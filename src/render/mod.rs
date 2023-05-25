use std::io::BufRead;
mod renderer;
use renderer::Renderer;
use std::{fs::File, io::BufReader};

use image::{GenericImage, GrayImage, ImageBuffer};
use serde::Serialize;
use serde_json::{json, Value};
use std::{cell::RefCell, collections::HashSet};

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(not(target_family = "wasm"))] {
        use indicatif::ParallelProgressIterator;
        use rayon::{iter::ParallelIterator, prelude::IntoParallelRefIterator};
        use thread_local::ThreadLocal;
    }
}

use crate::dfont::DFont;

const FUZZ: u8 = 10;

pub(crate) fn test_fonts(font_a: &DFont, font_b: &DFont) -> Value {
    // let words = test_font_words(font_a, font_b);
    // eprintln!("{}", serde_json::to_string_pretty(&words).unwrap());

    // json!({
    //     "glyphs": test_font_glyphs(font_a, font_b),
    // "words": words
    // })
    json!(test_font_glyphs(font_a, font_b))
}

fn chars_to_json_array<'a>(chars: impl Iterator<Item = &'a u32>) -> Value {
    let array: Vec<Value> = chars
        .map(|i| char::from_u32(*i as u32))
        .filter(|x| x.is_some())
        .map(|c| {
            json!({
                "string": c.unwrap().to_string(),
                "name": unicode_names2::name(c.unwrap())
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                "unicode": format!("U+{:04X}", c.unwrap() as u32),
            })
        })
        .collect();
    Value::Array(array)
}

fn test_font_glyphs(font_a: &DFont, font_b: &DFont) -> Value {
    let cmap_a = font_a.codepoints();
    let cmap_b = font_b.codepoints();
    let missing_glyphs = cmap_a.difference(&cmap_b);
    let new_glyphs = cmap_b.difference(&cmap_a);
    let same_glyphs = cmap_a.intersection(&cmap_b);
    let threshold = 0.1;
    let word_list: Vec<String> = same_glyphs
        .map(|i| char::from_u32(*i as u32))
        .filter(|x| x.is_some())
        .map(|c| c.unwrap().to_string())
        .collect();
    let mut result: Vec<GlyphDiff> = diff_many_words(font_a, font_b, 40.0, word_list, threshold)
        .into_iter()
        .map(|x| x.into())
        .collect();
    result.sort_by_key(|x| (-x.percent * 10_000.0) as i32);

    json!({
        "missing": chars_to_json_array(missing_glyphs),
        "new": chars_to_json_array(new_glyphs),
        "modified": result
    })
}

fn test_font_words(font_a: &DFont, font_b: &DFont) -> Value {
    let file = File::open("test-data/Arabic.txt").expect("no such file");
    let buf = BufReader::new(file);
    let wordlist: Vec<String> = buf
        .lines()
        .map(|l| l.expect("Could not parse line"))
        .collect();

    json!({
        "Arabic": diff_many_words(
        font_a, font_b, 40.0, wordlist, 0.2
    )})
}

fn make_same_size(image_a: GrayImage, image_b: GrayImage) -> (GrayImage, GrayImage) {
    let max_width = image_a.width().max(image_b.width());
    let max_height = image_a.height().max(image_b.height());
    let mut a = ImageBuffer::new(max_width, max_height);
    a.copy_from(&image_a, 0, 0).unwrap();
    let mut b = ImageBuffer::new(max_width, max_height);
    b.copy_from(&image_b, 0, 0).unwrap();
    (a, b)
}

fn count_differences(img_a: GrayImage, img_b: GrayImage) -> f32 {
    let (img_a, img_b) = make_same_size(img_a, img_b);
    let img_a_vec = img_a.to_vec();

    let differing_pixels = img_a_vec
        .iter()
        .zip(img_b.to_vec())
        .filter(|(&cha, chb)| cha.abs_diff(*chb) > FUZZ)
        .count();
    differing_pixels as f32 / (img_a.width() as f32 * img_a.height() as f32) * 100.0
}

#[derive(Debug, Serialize)]
pub struct GlyphDiff {
    pub string: String,
    pub name: String,
    pub unicode: String,
    pub percent: f32,
}

impl From<Difference> for GlyphDiff {
    fn from(diff: Difference) -> Self {
        if let Some(c) = diff.word.chars().next() {
            GlyphDiff {
                string: diff.word,
                name: unicode_names2::name(c)
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                unicode: format!("U+{:04X}", c as i32),
                percent: diff.percent,
            }
        } else {
            GlyphDiff {
                string: "".to_string(),
                name: "".to_string(),
                unicode: "".to_string(),
                percent: 0.0,
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Difference {
    pub word: String,
    pub buffer_a: String,
    pub buffer_b: String,
    // pub diff_map: Vec<i16>,
    pub percent: f32,
    pub ot_features: String,
    pub lang: String,
}

// A fast but complicated version
#[cfg(not(target_family = "wasm"))]
pub(crate) fn diff_many_words(
    font_a: &DFont,
    font_b: &DFont,
    font_size: f32,
    mut wordlist: Vec<String>,
    threshold: f32,
) -> Vec<Difference> {
    let tl_a = ThreadLocal::new();
    let tl_b = ThreadLocal::new();
    let tl_cache = ThreadLocal::new();
    wordlist.sort_by_key(|w| -(w.len() as i32));
    let differences: Vec<Option<Difference>> = wordlist
        .par_iter()
        .progress()
        .map(|word| {
            let renderer_a = tl_a.get_or(|| RefCell::new(Renderer::new(font_a, font_size)));
            let renderer_b = tl_b.get_or(|| RefCell::new(Renderer::new(font_b, font_size)));
            let seen_glyphs: &RefCell<HashSet<String>> =
                tl_cache.get_or(|| RefCell::new(HashSet::new()));

            let (buffer_a, img_a) = renderer_a.borrow_mut().render_string(word)?;
            if buffer_a
                .split('|')
                .all(|glyph| seen_glyphs.borrow().contains(glyph))
            {
                return None;
            }
            for glyph in buffer_a.split('|') {
                seen_glyphs.borrow_mut().insert(glyph.to_string());
            }
            let (buffer_b, img_b) = renderer_b.borrow_mut().render_string(word)?;
            let percent = count_differences(img_a, img_b) as f32;

            Some(Difference {
                word: word.to_string(),
                buffer_a,
                buffer_b,
                // diff_map,
                percent,
                ot_features: "".to_string(),
                lang: "".to_string(),
            })
        })
        .collect();
    let mut diffs: Vec<Difference> = differences
        .into_iter()
        .flatten()
        .filter(|diff| diff.percent > threshold)
        .collect();
    diffs.sort_by_key(|x| (-x.percent * 10_000.0) as i32);
    diffs
}

// A slow and simple version
#[cfg(target_family = "wasm")]
pub(crate) fn diff_many_words(
    font_a: &DFont,
    font_b: &DFont,
    font_size: f32,
    mut wordlist: Vec<String>,
    threshold: f32,
) -> Vec<Difference> {
    wordlist.sort_by_key(|w| -(w.len() as i32));
    let mut renderer_a = Renderer::new(font_a, font_size);
    let mut renderer_b = Renderer::new(font_b, font_size);
    let mut seen_glyphs: HashSet<String> = HashSet::new();

    let mut differences: Vec<Difference> = vec![];
    for word in wordlist {
        let result_a = renderer_a.render_string(&word);
        if result_a.is_none() {
            continue;
        }
        let (buffer_a, img_a) = result_a.unwrap();
        if buffer_a.split('|').all(|glyph| seen_glyphs.contains(glyph)) {
            continue;
        }
        for glyph in buffer_a.split('|') {
            seen_glyphs.insert(glyph.to_string());
        }
        let result_b = renderer_b.render_string(&word);
        if result_b.is_none() {
            continue;
        }
        let (buffer_b, img_b) = result_b.unwrap();
        let percent = count_differences(img_a, img_b) as f32;
        if percent > threshold {
            differences.push(Difference {
                word: word.to_string(),
                buffer_a,
                buffer_b,
                // diff_map,
                ot_features: "".to_string(),
                lang: "".to_string(),
                percent,
            })
        }
    }
    differences
}

// #[cfg(test)]
// mod tests {
//     use std::{
//         fs::File,
//         io::{BufRead, BufReader},
//     };

//     use super::*;

//     #[test]
//     fn test_it_works() {
//         let file = File::open("test-data/Latin.txt").expect("no such file");
//         let buf = BufReader::new(file);
//         let wordlist = buf
//             .lines()
//             .map(|l| l.expect("Could not parse line"))
//             .collect();
//         use std::time::Instant;
//         let now = Instant::now();

//         let mut results = diff_many_words_parallel(
//             "test-data/NotoSansArabic-Old.ttf",
//             "test-data/NotoSansArabic-New.ttf",
//             20.0,
//             wordlist,
//             10.0,
//         );
//         results.sort_by_key(|f| (f.percent * 100.0) as u32);
//         // for res in results {
//         //     println!("{}: {}%", res.word, res.percent)
//         // }
//         let elapsed = now.elapsed();
//         println!("Elapsed: {:.2?}", elapsed);
//     }

//     #[test]
//     fn test_render() {
//         let mut renderer_a = Renderer::new("test-data/NotoSansArabic-New.ttf", 40.0);
//         let mut renderer_b = Renderer::new("test-data/NotoSansArabic-Old.ttf", 40.0);
//         let (_, image_a) = renderer_a.render_string("پسے").unwrap(); // ď Ŭ
//         let (_, image_b) = renderer_b.render_string("پسے").unwrap();
//         let (image_a, image_b) = make_same_size(image_a, image_b);
//         image_a.save("image_a.png").expect("Can't save");
//         image_b.save("image_b.png").expect("Can't save");
//         let differing_pixels = count_differences(image_a, image_b);
//         println!("Ŏ Pixel differences: {:.2?}", differing_pixels);
//         let threshold = 0.5;
//         assert!(differing_pixels < threshold);
//     }

//     // #[test]
//     // fn test_ascent() {
//     //     let path = "Gulzar-Regular.ttf";
//     //     let font_size = 100.0;
//     //     let face = Face::from_file(path, 0).expect("No font");
//     //     let data = std::fs::read(path).unwrap();
//     //     let font = FontVec::try_from_vec(data).unwrap_or_else(|_| {
//     //         panic!("error constructing a Font from data at {:?}", path);
//     //     });
//     //     let mut hb_font = HBFont::new(face);
//     //     hb_font.set_scale(font_size as i32, font_size as i32);
//     //     let extents = hb_font.get_font_h_extents().unwrap();
//     //     let scaled_font = font.as_scaled(100.0);
//     //     println!("factor: {}", factor);
//     //     assert_eq!(scaled_font.ascent() / factor, extents.ascender as f32);
//     // }
// }
