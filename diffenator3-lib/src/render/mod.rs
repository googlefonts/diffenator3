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
    staticdiff::DifferenceSignature,
    wordselect::{select_buffer, word_is_encoded},
};
use colorrenderer::ColorRenderer;
use fontdrasil::coords::{NormalizedCoord, NormalizedLocation, UserLocation};
use harfrust::{Direction, Script};
use read_fonts::ReadError;
use renderer::{AnyRenderer, Renderer};
use rustc_hash::FxHashSet;
use skrifa::{raw::TableProvider, GlyphId};
use static_lang_word_lists::WordList;
use std::{collections::BTreeMap, str::FromStr};

use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(feature = "rayon")] {
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
    signature: Option<&DifferenceSignature>,
    custom_inputs: &[WordList],
    location: Option<&UserLocation>,
) -> BTreeMap<String, Vec<Difference>> {
    let mut map: BTreeMap<String, Vec<Difference>> = BTreeMap::new();
    let mut jobs: Vec<&WordList> = vec![];

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
            signature,
            DEFAULT_WORDS_THRESHOLD,
            location,
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
                location: diff.location,
            }
        } else {
            GlyphDiff {
                string: "".to_string(),
                name: "".to_string(),
                unicode: "".to_string(),
                differing_pixels: 0,
                location: "".to_string(),
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

// A fast but complicated version
#[cfg(feature = "rayon")]
/// Compare two fonts by rendering a list of words and comparing the images
///
/// This function is parallelized and uses rayon to speed up the process.
pub(crate) fn diff_many_words(
    font_a: &DFont,
    font_b: &DFont,
    font_size: f32,
    wordlist: &WordList,
    signature: Option<&DifferenceSignature>,
    threshold: usize,
    base_location: Option<&UserLocation>,
) -> Result<Vec<Difference>, ReadError> {
    let script = wordlist.script().and_then(|x| Script::from_str(x).ok());
    let direction = script.and_then(direction_from_script);
    let use_color = font_has_colr(font_a) || font_has_colr(font_b);

    // The static difference signature (outlines, advances, GPOS positioning)
    // is computed by the caller and passed in; it drives word selection.
    // `uncertain` and `marks` are derived indexes, shared read-only across the
    // worker threads. When no signature is supplied (a specific-location
    // diff) every word is rendered.
    let selection_idx: Option<(FxHashSet<GlyphId>, FxHashSet<GlyphId>)> =
        signature.map(|sig| (sig.uncertain_glyphs(), font_a.mark_glyphs()));

    let seen_glyphs = RwLock::new(FxHashSet::default());

    let tl_a: ThreadLocal<RefCell<Box<dyn AnyRenderer + Send + '_>>> = ThreadLocal::new();
    let tl_b: ThreadLocal<RefCell<Box<dyn AnyRenderer + Send + '_>>> = ThreadLocal::new();
    let coords_a = base_location.map(|loc| font_a.location_to_coords(loc));
    let coords_b = base_location.map(|loc| font_b.location_to_coords(loc));

    let differences: Vec<Difference> = wordlist
        .par_iter()
        .progress()
        .filter(|word| word_is_encoded(font_a, font_b, word))
        .flat_map(|word| {
            let renderer_a = tl_a.get_or(|| {
                RefCell::new(make_renderer(
                    font_a, font_size, direction, script, use_color,
                ))
            });
            let renderer_b = tl_b.get_or(|| {
                RefCell::new(make_renderer(
                    font_b, font_size, direction, script, use_color,
                ))
            });

            // Shape at the default location, then ask the static analysis
            // whether this word needs behavioural testing at all -- but only
            // when a signature was supplied (auto mode). For a specific
            // location diff every word is rendered.
            let buffer_a = renderer_a.borrow_mut().shape(word, coords_a.as_ref());
            let selection = match (&signature, &selection_idx) {
                (Some(sig), Some((uncertain, marks))) => {
                    let sel = select_buffer(sig, uncertain, marks, font_a, font_b, word, &buffer_a);
                    if !sel.selected {
                        // Nothing in this word's buffer intersects the
                        // difference signature, so skip it entirely.
                        return vec![];
                    }
                    Some(sel)
                }
                _ => None,
            };

            // Deduplication under an RwLock: read-check then write-insert. If
            // every glyph in A's buffer was already rendered, skip this word.
            let is_dup = {
                let seen = seen_glyphs.read().unwrap();
                buffer_a.iter().all(|g| seen.contains(g))
            };
            if is_dup {
                return vec![];
            }
            {
                let mut seen = seen_glyphs.write().unwrap();
                for g in buffer_a.iter() {
                    seen.insert(*g);
                }
            }

            let buffer_b = renderer_b.borrow_mut().shape(word, coords_b.as_ref());

            // Locations to test: the selected locations (default + the
            // designspace points where something changed), or -- when the
            // fallback is exhaustive -- every variation peak of either font.
            // With no signature, fall back to every variation peak.
            let locations: Vec<NormalizedLocation> = match &selection {
                Some(sel) if sel.exhaustive => {
                    let mut locs = font_a.variations_for_buffer(&buffer_a);
                    locs.extend(font_b.variations_for_buffer(&buffer_b));
                    locs.into_iter().collect()
                }
                Some(sel) => sel.locations.iter().cloned().collect(),
                None => {
                    let mut locs = font_a.variations_for_buffer(&buffer_a);
                    locs.extend(font_b.variations_for_buffer(&buffer_b));
                    locs.into_iter().collect()
                }
            };

            let mut results = Vec::with_capacity(locations.len() + 1);

            // Render at the default location
            if let Some(diff) = render_word(
                threshold,
                &mut renderer_a.borrow_mut(),
                &mut renderer_b.borrow_mut(),
                word,
                buffer_a,
                buffer_b,
                "default location",
                coords_a.as_ref().unwrap_or(&vec![]),
                coords_b.as_ref().unwrap_or(&vec![]),
            ) {
                results.push(diff);
            }

            if base_location.is_some() {
                // If the caller asked for an explicit location, don't render any
                // variations: the caller is responsible for rendering at the
                // other locations.
                return results;
            }

            // Render at each selected/variation location (default is already
            // rendered above, so skip it here).
            for variation in locations {
                if variation == NormalizedLocation::default() {
                    continue;
                }
                let coords_a = font_a.normalized_location_to_coords(&variation);
                let coords_b = font_b.normalized_location_to_coords(&variation);
                let buffer_a = renderer_a.borrow_mut().shape(word, Some(&coords_a));
                let buffer_b = renderer_b.borrow_mut().shape(word, Some(&coords_b));
                let user_space = font_a.location_to_user(&variation);
                if let Some(diff) = render_word(
                    threshold,
                    &mut renderer_a.borrow_mut(),
                    &mut renderer_b.borrow_mut(),
                    word,
                    buffer_a,
                    buffer_b,
                    &user_space,
                    &coords_a,
                    &coords_b,
                ) {
                    results.push(diff);
                }
            }

            results
        })
        .collect();

    let mut diffs = differences;
    diffs.retain(|diff| diff.differing_pixels > threshold);
    diffs.sort_by_key(|x| -(x.differing_pixels as i32));
    Ok(diffs)
}

// A slow and simple version (wasm; the parallel version above is used on
// native targets)
#[cfg(not(feature = "rayon"))]
pub(crate) fn diff_many_words(
    font_a: &DFont,
    font_b: &DFont,
    font_size: f32,
    wordlist: &WordList,
    signature: Option<&DifferenceSignature>,
    threshold: usize,
    base_location: Option<&UserLocation>,
) -> Result<Vec<Difference>, ReadError> {
    let script = wordlist.script().and_then(|x| Script::from_str(x).ok());
    let direction = script.and_then(direction_from_script);
    let use_color = font_has_colr(font_a) || font_has_colr(font_b);

    // The static difference signature (outlines, advances, GPOS positioning)
    // is computed by the caller and passed in; it drives word selection
    // below: a word is shaped once at the default location, then only
    // rendered at the designspace points where its glyphs/pairs actually
    // changed. `uncertain` and `marks` are derived indexes, built once and
    // reused for every word. When no signature is supplied (a specific-
    // location diff) every word is rendered.
    let selection_idx: Option<(FxHashSet<GlyphId>, FxHashSet<GlyphId>)> =
        signature.map(|sig| (sig.uncertain_glyphs(), font_a.mark_glyphs()));

    let mut seen_glyphs: FxHashSet<shaper::PositionedGlyph> = FxHashSet::default();
    let mut differences: Vec<Difference> = vec![];

    let mut renderer_a = make_renderer(font_a, font_size, direction, script, use_color);
    let mut renderer_b = make_renderer(font_b, font_size, direction, script, use_color);
    let coords_a = base_location.map(|loc| font_a.location_to_coords(loc));
    let coords_b = base_location.map(|loc| font_b.location_to_coords(loc));

    // No timings in wasm!

    // let time_before = std::time::Instant::now();
    // let mut first_shape_time = Duration::ZERO;
    // let mut var_shape_time = Duration::ZERO;
    // let mut first_other_time = Duration::ZERO;
    // let mut var_other_time = Duration::ZERO;
    // let mut variations_processed = 0;

    for word in wordlist.iter() {
        if !word_is_encoded(font_a, font_b, word) {
            continue;
        }
        // Shape at the default location to discover the buffer, then ask the
        // static analysis whether this word needs behavioural testing at all
        // -- but only when a signature was supplied (auto mode). For a
        // specific location diff every word is rendered.
        let buffer_a = renderer_a.shape(word, coords_a.as_ref());
        let selection = match (&signature, &selection_idx) {
            (Some(sig), Some((uncertain, marks))) => {
                let sel = select_buffer(sig, uncertain, marks, font_a, font_b, word, &buffer_a);
                if !sel.selected {
                    // Nothing in this word's buffer intersects the difference
                    // signature, so skip it entirely (no second shape, no
                    // render).
                    continue;
                }
                Some(sel)
            }
            _ => None,
        };
        let buffer_b = renderer_b.shape(word, coords_b.as_ref());

        // Locations to test: the selected locations (default + the designspace
        // points where something changed), or -- when the fallback is
        // exhaustive -- every variation peak of either font. Computed before
        // the default render below moves the buffers. With no signature, fall
        // back to every variation peak.
        let locations: Vec<NormalizedLocation> = match &selection {
            Some(sel) if sel.exhaustive => {
                let mut locs = font_a.variations_for_buffer(&buffer_a);
                locs.extend(font_b.variations_for_buffer(&buffer_b));
                locs.into_iter().collect()
            }
            Some(sel) => sel.locations.iter().cloned().collect(),
            None => {
                let mut locs = font_a.variations_for_buffer(&buffer_a);
                locs.extend(font_b.variations_for_buffer(&buffer_b));
                locs.into_iter().collect()
            }
        };

        // Deduplicate once per word, mirroring the parallel path: if every
        // glyph in A's default buffer was already rendered by an earlier
        // word, skip this word entirely. Deliberately NOT applied per
        // variation render -- PositionedGlyph carries no location, so a glyph
        // whose outline changes only at a non-default location (e.g. wght=700)
        // would be deduped away and its variation diff missed.
        if buffer_a.iter().all(|glyph| seen_glyphs.contains(glyph)) {
            continue;
        }
        for glyph in buffer_a.iter() {
            seen_glyphs.insert(*glyph);
        }

        if let Some(diff) = render_word(
            threshold,
            &mut renderer_a,
            &mut renderer_b,
            word,
            buffer_a,
            buffer_b,
            "default location",
            coords_a.as_ref().unwrap_or(&vec![]),
            coords_b.as_ref().unwrap_or(&vec![]),
        ) {
            differences.push(diff);
        }

        if base_location.is_some() {
            // If the caller asked for an explicit location, don't render any
            // variations: the caller is responsible for rendering at the
            // other locations.
            continue;
        }

        // first_other_time += other.elapsed();

        for variation in locations {
            if variation == NormalizedLocation::default() {
                // The default location was already rendered above.
                continue;
            }
            // let shaping = std::time::Instant::now();
            let coords_a = font_a.normalized_location_to_coords(&variation);
            let coords_b = font_b.normalized_location_to_coords(&variation);
            let buffer_a = renderer_a.shape(word, Some(coords_a.as_ref()));
            let buffer_b = renderer_b.shape(word, Some(coords_b.as_ref()));
            // var_shape_time += shaping.elapsed();
            // let other = std::time::Instant::now();
            let user_space = font_a.location_to_user(&variation);
            if let Some(diff) = render_word(
                threshold,
                &mut renderer_a,
                &mut renderer_b,
                word,
                buffer_a,
                buffer_b,
                &user_space,
                &coords_a,
                &coords_b,
            ) {
                differences.push(diff);
            }
            // var_other_time += other.elapsed();
            // variations_processed += 1;
        }
    }

    // log::info!(
    //     "Processed {} words in {:?} with {} variations",
    //     wordlist.len(),
    //     time_before.elapsed(),
    //     variations_processed
    // );
    // log::info!(
    //     "First shape time: {:?}, first other time: {:?}",
    //     first_shape_time,
    //     first_other_time
    // );
    // log::info!(
    //     "Variation shape time: {:?}, variation other time: {:?}",
    //     var_shape_time,
    //     var_other_time
    // );

    // renderer_a.log_stats();
    // renderer_b.log_stats();

    differences.sort_by_key(|x| -(x.differing_pixels as i32));
    Ok(differences)
}

/// Stage-1 render, fast-equivalence check, rasterize, and pixel-compare a word
/// pair.  Deduplication (the [`HashSet`] guard) is the caller's responsibility.
#[allow(clippy::too_many_arguments)]
fn render_word<'a>(
    threshold: usize,
    renderer_a: &mut Box<dyn AnyRenderer + Send + 'a>,
    renderer_b: &mut Box<dyn AnyRenderer + Send + 'a>,
    word: &str,
    buffer_a: shaper::DrawBuffer,
    buffer_b: shaper::DrawBuffer,
    location: &str,
    coords_a: &Vec<NormalizedCoord>,
    coords_b: &Vec<NormalizedCoord>,
) -> Option<Difference> {
    let data_a = renderer_a.buffer_to_stage1_rendering(&buffer_a, Some(coords_a))?;
    let data_b = renderer_b.buffer_to_stage1_rendering(&buffer_b, Some(coords_b))?;
    if renderer_a.fast_equivalence_check(&*data_a, &*data_b) {
        return None;
    }
    let buffers_same = buffer_a == buffer_b;
    let img_a = renderer_a.final_rendering(&*data_a, Some(coords_a));
    let img_b = renderer_b.final_rendering(&*data_b, Some(coords_b));
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
            location: location.to_string(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dfont::DFont, staticdiff::compute_signature};

    /// Helper: build a DFont if the test font exists, else None.
    fn dfont(path: &str) -> Option<DFont> {
        let data = std::fs::read(path).ok()?;
        Some(DFont::new(&data))
    }

    /// `diff_many_words` now drives its rendering from the static difference
    /// signature: changed words must still be reported (at the locations
    /// where they change) and unchanged words must be skipped entirely.
    #[test]
    fn diff_many_words_reports_kern_changes_with_selection() {
        let Some(font_a) = dfont("test-data/Martel-Original.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let Some(font_b) = dfont("test-data/Martel-KernChange.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let words = ["To", "Tory", "AV", "Tg", "hello", "world", "zip"];
        let wl = WordList::define("kern", words.iter().cloned());
        // Threshold 0: any differing pixel counts, so the assertions are
        // purely about which words are reported (and where).
        let signature = compute_signature(&font_a, &font_b);
        let diffs = diff_many_words(&font_a, &font_b, 16.0, &wl, Some(&signature), 0, None)
            .expect("diff_many_words failed");

        let reported: Vec<&str> = diffs.iter().map(|d| d.word.as_str()).collect();

        // "AV" has a *real* added kern (-30 at regular, -70 at bold): the
        // selection must catch it, and it must be reported at the bold
        // (wght=900) variation location where the pixels differ.
        assert!(
            reported.contains(&"AV"),
            "expected AV to be reported, got {reported:?}"
        );
        let av_locations: Vec<&str> = diffs
            .iter()
            .filter(|d| d.word == "AV")
            .map(|d| d.location.as_str())
            .collect();
        assert!(
            av_locations.iter().any(|l| l.contains("wght=900")),
            "expected a wght=900 location for AV, got {av_locations:?}"
        );

        // Words whose buffers don't intersect the difference signature must be
        // skipped entirely by the static-analysis selection.
        for skipped in ["hello", "world", "zip"] {
            assert!(
                !reported.contains(&skipped),
                "expected {skipped} to be skipped, got {reported:?}"
            );
        }

        // Note: the static analysis also flags the T/o and T/g pairs, so "To",
        // "Tory" and "Tg" ARE selected and rendered -- but the flagged change
        // does not manifest in the rendered output (the lookup isn't active in
        // default shaping), so they are correctly *not* reported as pixel
        // diffs. The selection is conservative (renders them) and sound (no
        // false positives).
    }
}
