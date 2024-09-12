mod cachedoutlines;
pub mod encodedglyphs;
pub mod renderer;
pub(crate) mod rustyruzz;
pub mod utils;
pub mod wordlists;
use crate::dfont::DFont;
use crate::render::rustyruzz::{Direction, Script};
use crate::render::utils::count_differences;
use cfg_if::cfg_if;
use renderer::Renderer;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;

cfg_if! {
    if #[cfg(not(target_family = "wasm"))] {
        use indicatif::ParallelProgressIterator;
        use rayon::{iter::ParallelIterator, prelude::IntoParallelRefIterator};
        use thread_local::ThreadLocal;
        use std::cell::RefCell;
        use std::sync::RwLock;
    }
}

pub const DEFAULT_WORDS_FONT_SIZE: f32 = 16.0;
pub const DEFAULT_GLYPHS_FONT_SIZE: f32 = 32.0;
/// Number of differing pixels after which two images are considered different
///
/// This is a count rather than a percentage, because a percentage would mean
/// that significant differences could "hide" inside a long word. This should
/// be adjusted to the size of the font and the expected differences.
pub const DEFAULT_WORDS_THRESHOLD: usize = 8;
pub const DEFAULT_GLYPHS_THRESHOLD: usize = 16;
/// Gray pixels which differ by less than this amount are considered the same
pub const DEFAULT_GRAY_FUZZ: u8 = 8;

/// Compare two fonts by rendering a list of words and comparing the images
///
/// Word lists are gathered for all scripts which are supported by both fonts.
/// The return value is a JSON object where each key is a script tag and the
/// value is a list of serialized [Difference] objects.
pub fn test_font_words(font_a: &DFont, font_b: &DFont) -> Value {
    let mut map = serde_json::Map::new();
    for script in font_a
        .supported_scripts()
        .intersection(&font_b.supported_scripts())
    {
        if let Some(wordlist) = wordlists::get_wordlist(script) {
            let direction = wordlists::get_script_direction(script);
            let script_tag = wordlists::get_script_tag(script);
            // Only bother rendering the words that have cmap entries in both fonts
            let wordlist = wordlist
                .iter()
                .filter(|word| {
                    word.chars().all(|c| {
                        font_a.codepoints.contains(&(c as u32))
                            && font_b.codepoints.contains(&(c as u32))
                    })
                })
                .map(|s| s.to_string())
                .collect();
            let results = diff_many_words(
                font_a,
                font_b,
                DEFAULT_WORDS_FONT_SIZE,
                wordlist,
                DEFAULT_WORDS_THRESHOLD,
                direction,
                script_tag,
            );
            if !results.is_empty() {
                map.insert(script.to_string(), serde_json::to_value(results).unwrap());
            }
        }
    }
    json!(map)
}

/// Represents a difference between two encoded glyphs
#[derive(Debug, Serialize)]
pub struct GlyphDiff {
    /// The string representation of the glyph
    pub string: String,
    /// The Unicode name of the glyph
    pub name: String,
    /// The Unicode codepoint of the glyph
    pub unicode: String,
    /// The number of differing pixels
    pub differing_pixels: usize,
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
                differing_pixels: diff.differing_pixels,
            }
        } else {
            GlyphDiff {
                string: "".to_string(),
                name: "".to_string(),
                unicode: "".to_string(),
                differing_pixels: 0,
            }
        }
    }
}

/// Represents a difference between two renderings, whether words or glyphs
#[derive(Debug, Serialize)]
pub struct Difference {
    /// The text string which was rendered
    pub word: String,
    /// A string representation of the shaped buffer in the first font
    pub buffer_a: String,
    /// A string representation of the shaped buffer in the second font, if different
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_b: Option<String>,
    /// The number of differing pixels
    pub differing_pixels: usize,
    /// The OpenType features applied to the text
    #[serde(skip_serializing_if = "String::is_empty")]
    pub ot_features: String,
    /// The OpenType language tag applied to the text
    #[serde(skip_serializing_if = "String::is_empty")]
    pub lang: String,
}

// A fast but complicated version
#[cfg(not(target_family = "wasm"))]
/// Compare two fonts by rendering a list of words and comparing the images
///
/// This function is parallelized and uses rayon to speed up the process.
///
/// # Arguments
///
/// * `font_a` - The first font to compare
/// * `font_b` - The second font to compare
/// * `font_size` - The size of the font to render
/// * `wordlist` - A list of words to render
/// * `threshold` - The percentage of differing pixels to consider a difference
/// * `direction` - The direction of the text
/// * `script` - The script of the text
///
/// # Returns
///
/// A list of [Difference] objects representing the differences between the two renderings.
pub(crate) fn diff_many_words(
    font_a: &DFont,
    font_b: &DFont,
    font_size: f32,
    wordlist: Vec<String>,
    threshold: usize,
    direction: Direction,
    script: Option<Script>,
) -> Vec<Difference> {
    let tl_a = ThreadLocal::new();
    let tl_b = ThreadLocal::new();
    // The cache should not be thread local
    let seen_glyphs = RwLock::new(HashSet::new());
    let differences: Vec<Option<Difference>> = wordlist
        .par_iter()
        .progress()
        .map(|word| {
            let renderer_a =
                tl_a.get_or(|| RefCell::new(Renderer::new(font_a, font_size, direction, script)));
            let renderer_b =
                tl_b.get_or(|| RefCell::new(Renderer::new(font_b, font_size, direction, script)));

            let (buffer_a, commands_a) =
                renderer_a.borrow_mut().string_to_positioned_glyphs(word)?;
            if buffer_a
                .split('|')
                .all(|glyph| seen_glyphs.read().unwrap().contains(glyph))
            {
                return None;
            }
            for glyph in buffer_a.split('|') {
                seen_glyphs.write().unwrap().insert(glyph.to_string());
            }
            let (buffer_b, commands_b) =
                renderer_b.borrow_mut().string_to_positioned_glyphs(word)?;
            if commands_a == commands_b {
                return None;
            }
            let img_a = renderer_a
                .borrow_mut()
                .render_positioned_glyphs(&commands_a);
            let img_b = renderer_b
                .borrow_mut()
                .render_positioned_glyphs(&commands_b);
            let differing_pixels = count_differences(img_a, img_b, DEFAULT_GRAY_FUZZ);
            let buffers_same = buffer_a == buffer_b;

            Some(Difference {
                word: word.to_string(),
                buffer_a,
                buffer_b: if buffers_same { None } else { Some(buffer_b) },
                // diff_map,
                differing_pixels,
                ot_features: "".to_string(),
                lang: "".to_string(),
            })
        })
        .collect();
    let mut diffs: Vec<Difference> = differences
        .into_iter()
        .flatten()
        .filter(|diff| diff.differing_pixels > threshold)
        .collect();
    diffs.sort_by_key(|x| -(x.differing_pixels as i32));
    diffs
}

// A slow and simple version
#[cfg(target_family = "wasm")]
pub(crate) fn diff_many_words(
    font_a: &DFont,
    font_b: &DFont,
    font_size: f32,
    wordlist: Vec<String>,
    threshold: usize,
    direction: Direction,
    script: Option<harfruzz::Script>,
) -> Vec<Difference> {
    let mut renderer_a = Renderer::new(font_a, font_size, direction, script);
    let mut renderer_b = Renderer::new(font_b, font_size, direction, script);
    let mut seen_glyphs: HashSet<String> = HashSet::new();

    let mut differences: Vec<Difference> = vec![];
    for word in wordlist {
        let result_a = renderer_a.string_to_positioned_glyphs(&word);
        if result_a.is_none() {
            continue;
        }
        let (buffer_a, commands_a) = result_a.unwrap();
        if buffer_a.split('|').all(|glyph| seen_glyphs.contains(glyph)) {
            continue;
        }
        for glyph in buffer_a.split('|') {
            seen_glyphs.insert(glyph.to_string());
        }
        let result_b = renderer_b.string_to_positioned_glyphs(&word);
        if result_b.is_none() {
            continue;
        }
        let (buffer_b, commands_b) = result_b.unwrap();
        if commands_a == commands_b {
            continue;
        }
        let buffers_same = buffer_a == buffer_b;
        let img_a = renderer_a.render_positioned_glyphs(&commands_a);
        let img_b = renderer_b.render_positioned_glyphs(&commands_b);
        let percent = count_differences(img_a, img_b, DEFAULT_GRAY_FUZZ);
        if percent > threshold {
            differences.push(Difference {
                word: word.to_string(),
                buffer_a,
                buffer_b: if buffers_same { None } else { Some(buffer_b) },
                // diff_map,
                ot_features: "".to_string(),
                lang: "".to_string(),
                percent,
            })
        }
    }
    differences.sort_by_key(|x| -(x.differing_pixels as i32));

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
