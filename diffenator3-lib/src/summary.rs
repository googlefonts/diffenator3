//! A human-readable summary of a [`DifferenceSignature`].
//!
//! The static analysis knows *which* glyphs and glyph pairs differ and *where*
//! (at which designspace locations) without doing any shaping or rasterization.
//! This module turns that into a concise, plain-language summary suitable for
//! the text reporter, the JSON report (and therefore the HTML report), and the
//! web interface.

use crate::{dfont::DFont, staticdiff::DifferenceSignature};
use fontdrasil::coords::NormalizedLocation;
use serde::Serialize;
use skrifa::{GlyphId, MetadataProvider};

/// A plain-language summary of the static difference signature.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct SignatureSummary {
    /// Number of cmap-matched glyphs that differ in outline or advance width.
    pub changed_glyphs: usize,
    /// Number of cmap-matched glyphs that are identical everywhere.
    pub identical_glyphs: usize,
    /// Encoded glyphs present only in the "before" font's cmap.
    pub missing_glyphs: Vec<String>,
    /// Encoded glyphs present only in the "after" font's cmap.
    pub new_glyphs: Vec<String>,
    /// Number of glyphs whose correspondence could not be established.
    pub uncertain_glyphs: usize,
    /// Number of GPOS single-adjustment (lookup type 1) differences.
    pub single_adjustments: usize,
    /// Number of GPOS pair-adjustment (lookup type 2 / kerning) differences.
    pub pair_adjustments: usize,
    /// Number of GPOS mark-attachment (lookup types 4/5/6) differences.
    pub mark_adjustments: usize,
    /// Number of GPOS cursive-attachment (lookup type 3) differences.
    pub cursive_adjustments: usize,
    /// Whether either font has contextual GPOS that isn't modelled.
    pub positioning_unmodelled: bool,
    /// Whether the fonts share no cmap codepoints, making matching unreliable.
    pub mapping_failed: bool,
    /// Human-readable designspace locations where changes were recorded.
    pub changed_locations: Vec<String>,
    /// A one-line plain-language overview.
    pub overview: String,
    /// Plain-language bullet points describing the changes.
    pub points: Vec<String>,
}

/// Maximum number of names to list before eliding the rest.
const MAX_EXAMPLES: usize = 20;
/// Maximum number of example pairs to show for positioning differences.
const MAX_PAIR_EXAMPLES: usize = 8;

/// Build a human-readable summary of a difference signature.
///
/// `font_a` is needed to render the changed locations in user space and to
/// look up glyph names for the examples.
pub fn summarize(signature: &DifferenceSignature, font_a: &DFont) -> SignatureSummary {
    let names = font_a.fontref().glyph_names();
    let mut points: Vec<String> = Vec::new();
    let mut overview_parts: Vec<String> = Vec::new();

    // Glyph outline / advance changes.
    let mut changed_names: Vec<String> = signature
        .glyph_changes
        .values()
        .map(|change| change.name.clone())
        .collect();
    changed_names.sort();
    changed_names.dedup();
    if !changed_names.is_empty() {
        points.push(format!(
            "{} glyph{} differ in outline or advance width: {}.",
            changed_names.len(),
            plural(changed_names.len()),
            join_capped(&changed_names, MAX_EXAMPLES)
        ));
        overview_parts.push(format!(
            "{} glyph{}",
            changed_names.len(),
            plural(changed_names.len())
        ));
    }

    if signature.unchanged_count > 0 {
        points.push(format!(
            "{} glyphs are identical everywhere.",
            signature.unchanged_count
        ));
    }

    // Missing / new glyphs.
    if !signature.missing.is_empty() || !signature.new.is_empty() {
        let mut parts: Vec<String> = Vec::new();
        if !signature.missing.is_empty() {
            parts.push(format!(
                "{} glyph{} missing from the new font",
                signature.missing.len(),
                plural(signature.missing.len())
            ));
        }
        if !signature.new.is_empty() {
            parts.push(format!(
                "{} glyph{} added in the new font",
                signature.new.len(),
                plural(signature.new.len())
            ));
        }
        points.push(format!("{}.", parts.join(", ")));
    }

    // Uncertain glyphs (need exhaustive behavioural testing).
    if !signature.uncertain.is_empty() {
        points.push(format!(
            "{} glyph{} could not be confidently matched and were tested exhaustively.",
            signature.uncertain.len(),
            plural(signature.uncertain.len())
        ));
    }

    // Positioning differences: clean count fragments for the overview, and
    // fragments with example pairs for the bullet point.
    let mut position_fragments: Vec<String> = Vec::new();
    let mut position_points: Vec<String> = Vec::new();

    if !signature.single_position_changes.is_empty() {
        let fragment = format!(
            "{} single adjustment{}",
            signature.single_position_changes.len(),
            plural(signature.single_position_changes.len())
        );
        position_fragments.push(fragment.clone());
        position_points.push(fragment);
    }
    if !signature.pair_position_changes.is_empty() {
        let fragment = format!(
            "{} kern pair{}",
            signature.pair_position_changes.len(),
            plural(signature.pair_position_changes.len())
        );
        let examples: Vec<String> = signature
            .pair_position_changes
            .keys()
            .take(MAX_PAIR_EXAMPLES)
            .map(|(a, b)| format!("({}, {})", glyph_name(&names, *a), glyph_name(&names, *b)))
            .collect();
        let suffix = if examples.is_empty() {
            String::new()
        } else {
            format!(" (e.g. {})", examples.join(", "))
        };
        position_fragments.push(fragment.clone());
        position_points.push(format!("{fragment}{suffix}"));
    }
    if !signature.mark_position_changes.is_empty() {
        let fragment = format!(
            "{} mark attachment{}",
            signature.mark_position_changes.len(),
            plural(signature.mark_position_changes.len())
        );
        let examples: Vec<String> = signature
            .mark_position_changes
            .keys()
            .take(MAX_PAIR_EXAMPLES)
            .map(|(m, b)| format!("({}, {})", glyph_name(&names, *m), glyph_name(&names, *b)))
            .collect();
        let suffix = if examples.is_empty() {
            String::new()
        } else {
            format!(" (e.g. {})", examples.join(", "))
        };
        position_fragments.push(fragment.clone());
        position_points.push(format!("{fragment}{suffix}"));
    }
    if !signature.cursive_position_changes.is_empty() {
        let fragment = format!(
            "{} cursive attachment{}",
            signature.cursive_position_changes.len(),
            plural(signature.cursive_position_changes.len())
        );
        position_fragments.push(fragment.clone());
        position_points.push(fragment);
    }
    if !position_points.is_empty() {
        points.push(format!(
            "Positioning differs: {}.",
            position_points.join(", ")
        ));
        overview_parts.extend(position_fragments);
    }

    if signature.positioning_unmodelled {
        points.push(
            "Both fonts contain contextual GPOS lookups, so positioning could not be fully modelled."
                .to_string(),
        );
    }
    if signature.mapping_failed {
        points.push(
            "The fonts share no common codepoints, so the comparison is unreliable.".to_string(),
        );
    }

    // Changed locations.
    let mut locations: Vec<String> = signature
        .changed_locations()
        .iter()
        .map(|loc| {
            if *loc == NormalizedLocation::default() {
                "default".to_string()
            } else {
                font_a.location_to_user(loc)
            }
        })
        .collect();
    locations.sort();
    locations.dedup();
    if !locations.is_empty() {
        points.push(format!(
            "Changes were recorded at: {}.",
            join_capped(&locations, MAX_EXAMPLES)
        ));
    }

    let overview = if overview_parts.is_empty() {
        "No differences were found in the static analysis.".to_string()
    } else {
        format!("{} differ.", overview_parts.join(", "))
    };

    SignatureSummary {
        changed_glyphs: signature.glyph_changes.len(),
        identical_glyphs: signature.unchanged_count,
        missing_glyphs: signature.missing.iter().map(|g| g.name.clone()).collect(),
        new_glyphs: signature.new.iter().map(|g| g.name.clone()).collect(),
        uncertain_glyphs: signature.uncertain.len(),
        single_adjustments: signature.single_position_changes.len(),
        pair_adjustments: signature.pair_position_changes.len(),
        mark_adjustments: signature.mark_position_changes.len(),
        cursive_adjustments: signature.cursive_position_changes.len(),
        positioning_unmodelled: signature.positioning_unmodelled,
        mapping_failed: signature.mapping_failed,
        changed_locations: locations,
        overview,
        points,
    }
}

/// Best-effort glyph name for reporting.
fn glyph_name(names: &skrifa::GlyphNames, gid: GlyphId) -> String {
    names
        .get(gid)
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("gid{}", gid.to_u32()))
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

/// Join a list, eliding beyond `max` entries.
fn join_capped(items: &[String], max: usize) -> String {
    if items.is_empty() {
        return String::new();
    }
    if items.len() <= max {
        items.join(", ")
    } else {
        format!(
            "{} (and {} more)",
            items[..max].join(", "),
            items.len() - max
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::staticdiff::compute_signature;

    /// Helper: build a DFont if the test font exists, else None.
    fn dfont(path: &str) -> Option<DFont> {
        let data = std::fs::read(path).ok()?;
        Some(DFont::new(&data))
    }

    #[test]
    fn mavenpro_summary_is_populated() {
        let Some(font_a) = dfont("../../MavenPro-Regular.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let Some(font_b) = dfont("../../MavenPro-Modified.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let signature = compute_signature(&font_a, &font_b);
        let summary = summarize(&signature, &font_a);
        assert!(
            !summary.points.is_empty(),
            "expected some summary points, got {:?}",
            summary.points
        );
        assert!(!summary.overview.is_empty(), "expected an overview");
        assert!(
            summary.changed_glyphs > 0,
            "MavenPro vs Modified should differ in glyphs"
        );
        assert!(
            summary.identical_glyphs > 0,
            "MavenPro vs Modified should share identical glyphs"
        );
    }

    #[test]
    fn self_comparison_summary_is_empty() {
        let Some(font) = dfont("../../MavenPro-Regular.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let signature = compute_signature(&font, &font);
        let summary = summarize(&signature, &font);
        assert_eq!(summary.changed_glyphs, 0);
        assert_eq!(summary.pair_adjustments, 0);
        assert!(summary.identical_glyphs > 0);
        assert!(
            summary.overview.contains("No differences"),
            "expected a no-difference overview, got {}",
            summary.overview
        );
    }
}
