//! Word selection driven by the static difference signature.
//!
//! Given the shaped buffer of a word (in font A), decide whether the word
//! needs to be rendered as part of the behavioural comparison, and -- if so --
//! at which designspace locations.
//!
//! A word is selected if its buffer intersects the signature in any way:
//!
//! * a glyph whose outline/advance differs ([`DifferenceSignature::glyph_changes`]);
//! * a glyph with a single-adjustment or cursive-positioning change;
//! * a changed kern pair (pair positioning);
//! * a changed mark attachment (mark-to-base/lig/mark);
//! * an `uncertain` glyph, which forces the exhaustive fallback.
//!
//! A word whose characters are not all encoded (have cmap mappings) in both
//! fonts would shape to `.notdef` in at least one of them, so it is rejected
//! up front by [`word_is_encoded`] -- mirroring the wordlist filter diffenator3
//! applies before rendering.
//!
//! ## Marks are transparent for pairs
//!
//! Pair positioning (kerning) and mark attachment apply *across* mark glyphs:
//! a kern lookup with the `IgnoreMarks` flag kerns `A` and `B` even when the
//! shaped buffer is `A <marks> B`. So a changed pair `A/B` selects any word
//! whose buffer contains `A` followed by any number of mark glyphs followed by
//! `B`. This is deliberately conservative: we cannot tell from the static
//! analysis whether a particular lookup ignores marks, so we treat marks as
//! transparent for every pair.

use rustc_hash::FxHashSet as HashSet;

use fontdrasil::coords::NormalizedLocation;
use skrifa::{GlyphId, MetadataProvider};

use crate::{
    dfont::DFont,
    render::shaper::DrawBuffer,
    staticdiff::{DifferenceSignature, LocationSet},
};

/// The result of deciding whether a word needs rendering.
#[derive(Debug, Clone, Default)]
pub struct WordSelection {
    /// Whether the word needs to be rendered at all.
    pub selected: bool,
    /// The designspace locations at which the word should be rendered,
    /// including the default location.
    pub locations: LocationSet,
    /// True if an uncertain glyph is present in the buffer, so the word must
    /// be rendered exhaustively (at all of its variation peaks) rather than
    /// only at the specific changed locations.
    pub exhaustive: bool,
    /// Human-readable reasons for the selection (empty if not selected).
    pub reasons: Vec<String>,
}

/// Whether every character of `word` is encoded (has a cmap mapping) in both
/// fonts. A word containing an unencoded character shapes to `.notdef` in at
/// least one font, so it can be rejected before any shaping/analysis work.
/// This mirrors diffenator3's wordlist filter and keeps `.notdef` glyphs out
/// of the selection (and out of the exhaustive fallback).
pub fn word_is_encoded(font_a: &DFont, font_b: &DFont, word: &str) -> bool {
    word.chars()
        .all(|c| font_a.codepoints.contains(&(c as u32)) && font_b.codepoints.contains(&(c as u32)))
}

/// Decide whether a word (given its shaped buffer) needs rendering, and at
/// which locations.
///
/// `uncertain` and `marks` are derived indexes (see
/// [`DifferenceSignature::uncertain_glyphs`] and [`DFont::mark_glyphs`]) that
/// must be built once and reused across words: rebuilding them on every call
/// dominates the runtime on large word lists. `font_a` is used for glyph
/// names, and `font_b`/`word` for the encoding pre-check
/// ([`word_is_encoded`]), so words that would shape to `.notdef` in either
/// font are rejected before any analysis.
pub fn select_buffer(
    signature: &DifferenceSignature,
    uncertain: &HashSet<GlyphId>,
    marks: &HashSet<GlyphId>,
    font_a: &DFont,
    font_b: &DFont,
    word: &str,
    buffer: &DrawBuffer,
) -> WordSelection {
    let glyphs: Vec<GlyphId> = buffer.iter().map(|glyph| glyph.glyph_id).collect();
    let mut selection = WordSelection::default();
    if !word_is_encoded(font_a, font_b, word) {
        return selection;
    }
    if glyphs.is_empty() {
        return selection;
    }

    for gid in &glyphs {
        if let Some(change) = signature.glyph_changes.get(gid) {
            selection.locations.extend(change.changed_locations());
            selection
                .reasons
                .push(format!("glyph '{}' outline/advance changed", change.name));
        }
        if let Some(locations) = signature.single_position_changes.get(gid) {
            selection.locations.extend(locations.iter().cloned());
            selection.reasons.push(format!(
                "glyph '{}' single-adjustment changed",
                glyph_name(font_a, *gid)
            ));
        }
        if let Some(locations) = signature.cursive_position_changes.get(gid) {
            selection.locations.extend(locations.iter().cloned());
            selection.reasons.push(format!(
                "glyph '{}' cursive-attachment changed",
                glyph_name(font_a, *gid)
            ));
        }
        if uncertain.contains(gid) {
            selection.exhaustive = true;
            selection.reasons.push(format!(
                "glyph '{}' uncertain (needs exhaustive fallback)",
                glyph_name(font_a, *gid)
            ));
        }
    }

    // Pair positioning applies across mark glyphs: build the de-marked
    // sequence (drop GDEF class 3 glyphs) and check consecutive pairs.
    let demarked: Vec<GlyphId> = glyphs
        .iter()
        .copied()
        .filter(|gid| !marks.contains(gid))
        .collect();
    for pair in demarked.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        if let Some(locations) = signature.pair_position_changes.get(&(left, right)) {
            selection.locations.extend(locations.iter().cloned());
            selection.reasons.push(format!(
                "pair '{}'/'{}' kern changed",
                glyph_name(font_a, left),
                glyph_name(font_a, right)
            ));
        }
    }

    // Mark attachment: a mark attaches to a preceding glyph (base, ligature
    // or another mark). Conservatively check every preceding glyph.
    for i in 1..glyphs.len() {
        if marks.contains(&glyphs[i]) {
            for j in 0..i {
                if let Some(locations) =
                    signature.mark_position_changes.get(&(glyphs[i], glyphs[j]))
                {
                    selection.locations.extend(locations.iter().cloned());
                    selection.reasons.push(format!(
                        "mark '{}' on '{}' attachment changed",
                        glyph_name(font_a, glyphs[i]),
                        glyph_name(font_a, glyphs[j])
                    ));
                }
            }
        }
    }

    if !selection.reasons.is_empty() {
        selection.selected = true;
        // Always render the default location: the shaped buffer itself may
        // differ between the fonts even when no per-location value does.
        selection.locations.insert(NormalizedLocation::default());
    }
    selection
}

fn glyph_name(font: &DFont, gid: GlyphId) -> String {
    font.fontref()
        .glyph_names()
        .get(gid)
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("gid{}", gid.to_u32()))
}
