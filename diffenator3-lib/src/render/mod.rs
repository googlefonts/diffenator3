/// Rendering and comparison of fonts
///
/// The routines in this file handle the rendering and comparison of text
/// strings; the actual rendering proper is done in the `renderer` module.
mod cachedoutlines;
pub mod encodedglyphs;
pub mod renderer;
pub mod utils;
pub mod wordlists;
pub use crate::structs::{Difference, GlyphDiff};
use crate::{
    dfont::DFont,
    render::{utils::count_differences, wordlists::direction_from_script},
};
use cfg_if::cfg_if;
use harfrust::Script;
use renderer::Renderer;
use static_lang_word_lists::WordList;
use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
};

cfg_if! {
    if #[cfg(not(target_family = "wasm"))] {
        use indicatif::ParallelProgressIterator;
        use rayon::iter::ParallelIterator;
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
/// The return value is a BTreeMap where each key is a script tag and the
/// value is a list of  [Difference] objects.
pub fn test_font_words(
    font_a: &DFont,
    font_b: &DFont,
    custom_inputs: &[WordList],
) -> BTreeMap<String, Vec<Difference>> {
    let mut map: BTreeMap<String, Vec<Difference>> = BTreeMap::new();
    let mut jobs: Vec<&WordList> = vec![];

    let shared_codepoints = font_a
        .codepoints
        .intersection(&font_b.codepoints)
        .copied()
        .collect();

    let supported_a = font_a.supported_scripts();
    let supported_b = font_b.supported_scripts();

    // Create the jobs
    for script in supported_a.intersection(&supported_b) {
        if let Some(wordlist) = wordlists::get_wordlist(script) {
            jobs.push(wordlist);
        }
    }
    jobs.extend(custom_inputs.iter());
    // Process the jobs
    for job in jobs.iter_mut() {
        let results = diff_many_words(
            font_a,
            font_b,
            DEFAULT_WORDS_FONT_SIZE,
            job,
            Some(&shared_codepoints),
            DEFAULT_WORDS_THRESHOLD,
        );
        if !results.is_empty() {
            map.insert(job.name().to_string(), results);
        }
    }
    map
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
    wordlist: &WordList,
    shared_codepoints: Option<&HashSet<u32>>,
    threshold: usize,
) -> Vec<Difference> {
    let tl_a = ThreadLocal::new();
    let tl_b = ThreadLocal::new();
    let script = wordlist.script().and_then(|x| Script::from_str(x).ok());
    let direction = script.and_then(direction_from_script);
    // The cache should not be thread local
    let seen_glyphs = RwLock::new(HashSet::new());
    let differences: Vec<Option<Difference>> = wordlist
        .par_iter()
        .progress()
        .filter(|word| {
            shared_codepoints
                .as_ref()
                .is_none_or(|scp| word.chars().all(|c| scp.contains(&(c as u32))))
        })
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
    wordlist: &WordList,
    shared_codepoints: Option<&HashSet<u32>>,
    threshold: usize,
) -> Vec<Difference> {
    let script = wordlist.script().and_then(|x| Script::from_str(x).ok());
    let direction = script.and_then(|s| direction_from_script(s));
    let mut renderer_a = Renderer::new(font_a, font_size, direction, script);
    let mut renderer_b = Renderer::new(font_b, font_size, direction, script);
    let mut seen_glyphs: HashSet<String> = HashSet::new();

    let mut differences: Vec<Difference> = vec![];
    for word in wordlist.iter() {
        if let Some(scp) = shared_codepoints {
            if !word.chars().all(|c| scp.contains(&(c as u32))) {
                continue;
            }
        }
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
        let differing_pixels = count_differences(img_a, img_b, DEFAULT_GRAY_FUZZ);
        if differing_pixels > threshold {
            differences.push(Difference {
                word: word.to_string(),
                buffer_a,
                buffer_b: if buffers_same { None } else { Some(buffer_b) },
                // diff_map,
                ot_features: "".to_string(),
                lang: "".to_string(),
                differing_pixels,
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
