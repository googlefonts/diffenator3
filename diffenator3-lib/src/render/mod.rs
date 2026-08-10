/// Rendering and comparison of fonts
///
/// The routines in this file handle the rendering and comparison of text
/// strings; the actual rendering proper is done in the `renderer` module.
mod cachedoutlines;
pub(crate) mod colorpainter;
pub(crate) mod colorrenderer;
pub mod encodedglyphs;
pub mod renderer;
pub mod shaper;
pub mod utils;
pub mod wordlists;
pub use crate::structs::{Difference, GlyphDiff};
use crate::{
    dfont::DFont,
    render::{utils::count_differences, wordlists::direction_from_script},
};
use cfg_if::cfg_if;
use colorrenderer::ColorRenderer;
use harfrust::{Direction, Script};
use renderer::{AnyRenderer, Renderer};
use skrifa::raw::TableProvider;
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

/// Returns true if the font has a COLR table.
fn font_has_colr(dfont: &DFont) -> bool {
    dfont.fontref().colr().is_ok()
}

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

/// Build a renderer appropriate for the fonts being compared, boxed as a trait
/// object so that the color and outline rendering paths share a single loop.
fn make_renderer<'a>(
    dfont: &'a DFont,
    font_size: f32,
    direction: Option<Direction>,
    script: Option<Script>,
    use_color: bool,
) -> Box<dyn AnyRenderer + Send + 'a> {
    if use_color {
        Box::new(ColorRenderer::new(dfont, font_size, direction, script))
    } else {
        Box::new(Renderer::new(dfont, font_size, direction, script))
    }
}

// Disable fast complicated version while we're refactoring. Get simple one working first.

// // A fast but complicated version
// #[cfg(not(target_family = "wasm"))]
// /// Compare two fonts by rendering a list of words and comparing the images
// ///
// /// This function is parallelized and uses rayon to speed up the process.
// pub(crate) fn diff_many_words(
//     font_a: &DFont,
//     font_b: &DFont,
//     font_size: f32,
//     wordlist: &WordList,
//     shared_codepoints: Option<&HashSet<u32>>,
//     threshold: usize,
// ) -> Vec<Difference> {
//     let script = wordlist.script().and_then(|x| Script::from_str(x).ok());
//     let direction = script.and_then(direction_from_script);
//     let seen_glyphs = RwLock::new(HashSet::new());
//     let use_color = font_has_colr(font_a) || font_has_colr(font_b);

//     let tl_a: ThreadLocal<RefCell<Box<dyn AnyRenderer + Send + '_>>> = ThreadLocal::new();
//     let tl_b: ThreadLocal<RefCell<Box<dyn AnyRenderer + Send + '_>>> = ThreadLocal::new();

//     let differences: Vec<Option<Difference>> = wordlist
//         .par_iter()
//         .progress()
//         .filter(|word| {
//             shared_codepoints
//                 .as_ref()
//                 .is_none_or(|scp| word.chars().all(|c| scp.contains(&(c as u32))))
//         })
//         .map(|word| {
//             let renderer_a = tl_a.get_or(|| {
//                 RefCell::new(make_renderer(
//                     font_a, font_size, direction, script, use_color,
//                 ))
//             });
//             let renderer_b = tl_b.get_or(|| {
//                 RefCell::new(make_renderer(
//                     font_b, font_size, direction, script, use_color,
//                 ))
//             });

//             let (buffer_a, data_a) = renderer_a.borrow_mut().string_to_stage1_rendering(word)?;
//             if buffer_a
//                 .split('|')
//                 .all(|glyph| seen_glyphs.read().unwrap().contains(glyph))
//             {
//                 return None;
//             }
//             for glyph in buffer_a.split('|') {
//                 seen_glyphs.write().unwrap().insert(glyph.to_string());
//             }
//             let (buffer_b, data_b) = renderer_b.borrow_mut().string_to_stage1_rendering(word)?;
//             if renderer_a
//                 .borrow()
//                 .fast_equivalence_check(&*data_a, &*data_b)
//             {
//                 return None;
//             }
//             let img_a = renderer_a.borrow_mut().final_rendering(&*data_a);
//             let img_b = renderer_b.borrow_mut().final_rendering(&*data_b);
//             let differing_pixels = count_differences(img_a, img_b, DEFAULT_GRAY_FUZZ);
//             let buffers_same = buffer_a == buffer_b;

//             Some(Difference {
//                 word: word.to_string(),
//                 buffer_a,
//                 buffer_b: if buffers_same { None } else { Some(buffer_b) },
//                 differing_pixels,
//                 ot_features: "".to_string(),
//                 lang: "".to_string(),
//             })
//         })
//         .collect();

//     let mut diffs: Vec<Difference> = differences
//         .into_iter()
//         .flatten()
//         .filter(|diff| diff.differing_pixels > threshold)
//         .collect();
//     diffs.sort_by_key(|x| -(x.differing_pixels as i32));
//     diffs
// }

// // A slow and simple version
// #[cfg(target_family = "wasm")]
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
    let use_color = font_has_colr(font_a) || font_has_colr(font_b);
    let mut seen_glyphs: HashSet<String> = HashSet::new();
    let mut differences: Vec<Difference> = vec![];

    let mut renderer_a = make_renderer(font_a, font_size, direction, script, use_color);
    let mut renderer_b = make_renderer(font_b, font_size, direction, script, use_color);

    for word in wordlist.iter() {
        if let Some(scp) = shared_codepoints {
            if !word.chars().all(|c| scp.contains(&(c as u32))) {
                continue;
            }
        }
        let Some((buffer_a, data_a)) = renderer_a.string_to_stage1_rendering(&word) else {
            continue;
        };
        if buffer_a.split('|').all(|glyph| seen_glyphs.contains(glyph)) {
            continue;
        }
        for glyph in buffer_a.split('|') {
            seen_glyphs.insert(glyph.to_string());
        }
        let Some((buffer_b, data_b)) = renderer_b.string_to_stage1_rendering(&word) else {
            continue;
        };
        if renderer_a.fast_equivalence_check(&*data_a, &*data_b) {
            continue;
        }
        let buffers_same = buffer_a == buffer_b;
        let img_a = renderer_a.final_rendering(&*data_a);
        let img_b = renderer_b.final_rendering(&*data_b);
        let differing_pixels = count_differences(img_a, img_b, DEFAULT_GRAY_FUZZ);
        if differing_pixels > threshold {
            differences.push(Difference {
                word: word.to_string(),
                buffer_a,
                buffer_b: if buffers_same { None } else { Some(buffer_b) },
                ot_features: "".to_string(),
                lang: "".to_string(),
                differing_pixels,
            });
        }
    }

    differences.sort_by_key(|x| -(x.differing_pixels as i32));
    differences
}
