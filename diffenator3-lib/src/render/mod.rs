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
    render::{shaper::PositionedGlyph, utils::count_differences, wordlists::direction_from_script},
};
use cfg_if::cfg_if;
use colorrenderer::ColorRenderer;
use fontdrasil::coords::NormalizedLocation;
use harfrust::{Direction, Script};
use indicatif::ProgressIterator as _;
use read_fonts::ReadError;
use renderer::{AnyRenderer, Renderer};
use skrifa::raw::TableProvider;
use static_lang_word_lists::WordList;
use std::{
    collections::{BTreeMap, HashSet},
    ops::ControlFlow,
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
        )
        .unwrap_or_default();
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

//             if buffer_a
//             let (buffer_b, data_b) = renderer_b.borrow_mut().string_to_stage1_rendering(word)?;
//                 .borrow()
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
) -> Result<Vec<Difference>, ReadError> {
    let script = wordlist.script().and_then(|x| Script::from_str(x).ok());
    let direction = script.and_then(direction_from_script);
    let use_color = font_has_colr(font_a) || font_has_colr(font_b);
    let mut seen_glyphs: HashSet<PositionedGlyph> = HashSet::new();
    let mut differences: Vec<Difference> = vec![];

    let mut renderer_a = make_renderer(font_a, font_size, direction, script, use_color);
    let mut renderer_b = make_renderer(font_b, font_size, direction, script, use_color);

    let time_before = std::time::Instant::now();

    for word in wordlist.iter() {
        if let Some(scp) = shared_codepoints {
            if !word.chars().all(|c| scp.contains(&(c as u32))) {
                continue;
            }
        }
        // Shape it at the default location and render to a buffer
        let buffer_a = renderer_a.shape(word, None);
        let mut variation_positions = font_a.variations_for_buffer(&buffer_a);
        let buffer_b = renderer_b.shape(word, None);
        variation_positions.extend(font_b.variations_for_buffer(&buffer_b));

        if let Some(diff) = process_word(
            threshold,
            &mut seen_glyphs,
            &mut renderer_a,
            &mut renderer_b,
            word,
            buffer_a,
            buffer_b,
            "".to_string(),
        ) {
            differences.push(diff);
        }

        // // Now let's look at the variations!
        // for variation in variation_positions {
        //     let buffer_a = renderer_a.shape(word, Some(font_a.location_to_coords(&variation)));
        //     let buffer_b = renderer_b.shape(word, Some(font_b.location_to_coords(&variation)));
        //     let user_space = font_a.location_to_user(&variation);
        //     // println!("also checking {} at {}", word, user_space);
        //     if let Some(diff) = process_word(
        //         threshold,
        //         &mut seen_glyphs,
        //         &mut renderer_a,
        //         &mut renderer_b,
        //         word,
        //         buffer_a,
        //         buffer_b,
        //         user_space,
        //     ) {
        //         differences.push(diff);
        //     }
        // }
    }

    log::info!(
        "Processed {} words in {:?}",
        wordlist.len(),
        time_before.elapsed()
    );

    differences.sort_by_key(|x| -(x.differing_pixels as i32));
    Ok(differences)
}

fn process_word<'a>(
    threshold: usize,
    seen_glyphs: &mut HashSet<PositionedGlyph>,
    renderer_a: &mut Box<dyn AnyRenderer + Send + 'a>,
    renderer_b: &mut Box<dyn AnyRenderer + Send + 'a>,
    word: &str,
    buffer_a: shaper::DrawBuffer,
    buffer_b: shaper::DrawBuffer,
    location: String,
) -> Option<Difference> {
    let data_a = renderer_a.buffer_to_stage1_rendering(&buffer_a)?;
    if buffer_a.iter().all(|glyph| seen_glyphs.contains(glyph)) {
        return None;
    }
    for glyph in buffer_a.iter() {
        seen_glyphs.insert(*glyph);
    }
    let data_b = renderer_b.buffer_to_stage1_rendering(&buffer_b)?;
    if renderer_a.fast_equivalence_check(&*data_a, &*data_b) {
        return None;
    }
    let buffers_same = buffer_a == buffer_b;
    let img_a = renderer_a.final_rendering(&*data_a);
    let img_b = renderer_b.final_rendering(&*data_b);
    let differing_pixels = count_differences(img_a, img_b, DEFAULT_GRAY_FUZZ);

    if differing_pixels > threshold {
        return Some(Difference {
            word: word.to_string(),
            buffer_a: buffer_a.serialize(),
            buffer_b: if buffers_same {
                None
            } else {
                Some(buffer_b.serialize())
            },
            ot_features: "".to_string(),
            lang: "".to_string(),
            differing_pixels,
            location,
        });
    }
    None
}
