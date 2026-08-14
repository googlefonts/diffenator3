//! Static, shaping-free analysis of the differences between two fonts.
//!
//! The expensive part of `diffenator3` is shaping and rendering many words at
//! many designspace locations. This module tries to avoid as much of that work
//! as possible by statically analysing the two fonts *before* any shaping
//! happens: for every glyph the fonts have in common it works out, cheaply, at
//! which designspace locations the two fonts differ, and records those
//! locations.
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::collections::BTreeSet;

use fontdrasil::coords::{NormalizedCoord, NormalizedLocation};
use read_fonts::types::F2Dot14;
use skrifa::{
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlineGlyphCollection},
    GlyphId, MetadataProvider,
};
use zeno::Command;

use crate::{dfont::DFont, gposdiff::compute_gpos_changes, render::utils::RecordingPen};

/// A set of designspace locations, in normalized coordinates.
///
/// `NormalizedLocation` is a `BTreeMap<Tag, NormalizedCoord>`, which is
/// `Ord + Hash`, so this works directly as a set type.
pub type LocationSet = HashSet<NormalizedLocation>;

/// Sample size used for outline and metric comparison.
///
/// The absolute value doesn't matter for difference detection: both fonts are
/// sampled at the same size and the scaling is normalized by units-per-em. It
/// only needs to be identical for the two fonts.
const SAMPLE_SIZE: f32 = 16.0;

/// How a matched pair of glyphs was identified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMethod {
    /// Both fonts encode the same codepoint, and we compare the glyphs each
    /// maps to. This is high confidence.
    Cmap(u32),
    /// Unencoded glyphs matched by identical default-location outline hash.
    /// This is heuristic: the semantic correspondence is not guaranteed, so
    /// these glyphs are also reported in [`DifferenceSignature::uncertain`].
    DefaultOutline,
}

/// A lightweight, report-friendly description of a glyph in one of the fonts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphReport {
    /// Glyph id in the font.
    pub gid: GlyphId,
    /// Best-effort name (from `post`/CFF, or synthesized as `gidDDD`).
    pub name: String,
    /// The codepoint, if the glyph is encoded in that font's cmap.
    pub codepoint: Option<u32>,
}

/// The difference between a matched pair of glyphs.
#[derive(Debug, Clone)]
pub struct GlyphChange {
    /// The corresponding glyph id in font B.
    pub gid_b: GlyphId,
    /// Best-effort glyph name (from font A).
    pub name: String,
    /// The codepoint, if the glyph is encoded in font A's cmap.
    pub codepoint: Option<u32>,
    /// How the pair was matched.
    pub matched_by: MatchMethod,
    /// Locations at which the outline differs.
    pub outline_locations: LocationSet,
    /// Locations at which the advance width differs.
    pub advance_locations: LocationSet,
}

impl GlyphChange {
    /// All locations at which this glyph differs in any observable way.
    pub fn changed_locations(&self) -> LocationSet {
        self.outline_locations
            .union(&self.advance_locations)
            .cloned()
            .collect()
    }
}

/// The result of statically comparing two fonts.
#[derive(Debug, Clone, Default)]
pub struct DifferenceSignature {
    /// Glyphs which differ, keyed by glyph id in font A. Includes both
    /// high-confidence cmap-matched differences and (heuristic) unencoded
    /// differences, distinguished by [`GlyphChange::matched_by`].
    pub glyph_changes: HashMap<GlyphId, GlyphChange>,
    /// Number of high-confidence (cmap-matched) glyph pairs which were found
    /// identical everywhere.
    pub unchanged_count: usize,
    /// Encoded glyphs present in font A but missing from font B's cmap.
    pub missing: Vec<GlyphReport>,
    /// Encoded glyphs present in font B but missing from font A's cmap.
    pub new: Vec<GlyphReport>,
    /// Font A glyphs the analyzer could not confidently reason about: every
    /// unencoded glyph (only reachable through `GSUB`), plus ambiguous or
    /// undrawable glyphs. Word selection must fall back to exhaustive testing
    /// for any word whose shaped buffer contains one of these.
    pub uncertain: Vec<GlyphReport>,
    /// Single adjustment (GPOS lookup type 1) positioning differences, keyed
    /// by font A glyph id.
    pub single_position_changes: HashMap<GlyphId, LocationSet>,
    /// Pair adjustment (GPOS lookup type 2) positioning differences, keyed by
    /// `(left, right)` font A glyph ids.
    pub pair_position_changes: HashMap<(GlyphId, GlyphId), LocationSet>,
    /// Mark attachment (GPOS lookup types 4/5/6) differences, keyed by
    /// `(mark, base)` font A glyph ids.
    pub mark_position_changes: HashMap<(GlyphId, GlyphId), LocationSet>,
    /// Cursive attachment (GPOS lookup type 3) differences, keyed by font A
    /// glyph id.
    pub cursive_position_changes: HashMap<GlyphId, LocationSet>,
    /// True if either font has contextual GPOS lookups that this analysis does
    /// not model, so positioning pruning must not be trusted.
    pub positioning_unmodelled: bool,
    /// True when the fonts share no cmap codepoints, so matching is unreliable
    /// and the whole comparison must be treated as uncertain.
    pub mapping_failed: bool,
}

impl DifferenceSignature {
    /// Whether the static analysis found anything that needs behavioural
    /// testing (changed glyphs, uncertain glyphs, or positioning differences).
    pub fn needs_testing(&self) -> bool {
        !self.glyph_changes.is_empty()
            || !self.uncertain.is_empty()
            || !self.single_position_changes.is_empty()
            || !self.pair_position_changes.is_empty()
            || !self.mark_position_changes.is_empty()
            || !self.cursive_position_changes.is_empty()
            || self.positioning_unmodelled
    }

    /// Whether the fonts appear identical at the outline/advance level for
    /// every glyph they can be matched on.
    ///
    /// Note this is a *partial* statement only: kerning (pair positioning),
    /// shaping (`GSUB`) and unencoded glyphs are not covered yet, so
    /// `is_identical()` returning `true` does **not** mean the fonts are
    /// behaviourally identical. It must not, on its own, be used to skip word
    /// testing.
    pub fn is_identical(&self) -> bool {
        self.glyph_changes.is_empty()
            && self.single_position_changes.is_empty()
            && self.pair_position_changes.is_empty()
            && self.mark_position_changes.is_empty()
            && self.cursive_position_changes.is_empty()
            && !self.positioning_unmodelled
            && self.missing.is_empty()
            && self.new.is_empty()
            && !self.mapping_failed
    }

    /// The set of glyph ids flagged `uncertain` (these force the exhaustive
    /// fallback when they appear in a word). Precompute once and reuse:
    /// rebuilding this per word dominates selection runtime on large word
    /// lists.
    pub fn uncertain_glyphs(&self) -> HashSet<GlyphId> {
        self.uncertain.iter().map(|report| report.gid).collect()
    }

    /// The set of designspace locations at which anything changed: the
    /// default location plus every location where a glyph outline/advance or
    /// positioning difference was recorded. This is what "auto" mode uses to
    /// decide which locations to report, instead of the font's static
    /// instances.
    pub fn changed_locations(&self) -> LocationSet {
        let mut set = LocationSet::default();
        set.insert(NormalizedLocation::default());
        for change in self.glyph_changes.values() {
            set.extend(change.changed_locations());
        }
        set.extend(
            self.single_position_changes
                .values()
                .chain(self.cursive_position_changes.values())
                .flat_map(|locs| locs.iter().cloned()),
        );
        set.extend(
            self.pair_position_changes
                .values()
                .chain(self.mark_position_changes.values())
                .flat_map(|locs| locs.iter().cloned()),
        );
        set
    }
}

/// Compute the static difference signature between two fonts.
///
/// This is a pure function of the two fonts: it performs no shaping, no
/// rasterization and no pixel comparison. It *does* draw each glyph's outline
/// once per relevant location, which is the same cheap operation the renderer
/// performs during its stage-1 pass.
pub fn compute_signature(font_a: &DFont, font_b: &DFont) -> DifferenceSignature {
    let fontref_a = font_a.fontref();
    let fontref_b = font_b.fontref();
    let names_a = fontref_a.glyph_names();
    let names_b = fontref_b.glyph_names();

    // Codepoint -> glyph id, per font.
    let cmap_a: HashMap<u32, GlyphId> = fontref_a.charmap().mappings().collect();
    let cmap_b: HashMap<u32, GlyphId> = fontref_b.charmap().mappings().collect();

    let shared: BTreeSet<u32> = cmap_a
        .keys()
        .filter(|cp| cmap_b.contains_key(cp))
        .copied()
        .collect();

    // If there are no shared codepoints, we have no reliable way to match
    // glyphs between the fonts. Report that and give up.
    if shared.is_empty() {
        return DifferenceSignature {
            mapping_failed: true,
            ..DifferenceSignature::default()
        };
    }

    let mut signature = DifferenceSignature::default();

    // Run the GPOS positioning pass, keyed by the cmap glyph matches.
    let mut match_a2b: HashMap<GlyphId, GlyphId> = HashMap::default();
    for cp in &shared {
        match_a2b.insert(cmap_a[cp], cmap_b[cp]);
    }
    let gpos = compute_gpos_changes(font_a, font_b, &match_a2b);
    signature.single_position_changes = gpos.single;
    signature.pair_position_changes = gpos.pair;
    signature.mark_position_changes = gpos.mark;
    signature.cursive_position_changes = gpos.cursive;
    signature.positioning_unmodelled = gpos.unmodelled;

    // Glyph ids already matched through the cmap, per font.
    let mut claimed_a: HashSet<GlyphId> = HashSet::default();
    let mut claimed_b: HashSet<GlyphId> = HashSet::default();

    // 1. Compare every glyph reachable through a shared codepoint. This is the
    //    high-confidence part of the analysis.
    for cp in &shared {
        let gid_a = cmap_a[cp];
        let gid_b = cmap_b[cp];
        claimed_a.insert(gid_a);
        claimed_b.insert(gid_b);
        match compare_glyphs(font_a, font_b, gid_a, gid_b) {
            GlyphComparison::Same => signature.unchanged_count += 1,
            GlyphComparison::Changed {
                outline_locations,
                advance_locations,
            } => {
                signature.glyph_changes.insert(
                    gid_a,
                    GlyphChange {
                        gid_b,
                        name: name_for(&names_a, gid_a),
                        codepoint: Some(*cp),
                        matched_by: MatchMethod::Cmap(*cp),
                        outline_locations,
                        advance_locations,
                    },
                );
            }
            GlyphComparison::Unavailable => signature.uncertain.push(GlyphReport {
                gid: gid_a,
                name: name_for(&names_a, gid_a),
                codepoint: Some(*cp),
            }),
        }
    }

    // 2. Report cmap-level differences (informational; the word rendering
    //    already filters words to codepoints shared by both fonts).
    for (cp, gid) in &cmap_a {
        if !cmap_b.contains_key(cp) {
            signature.missing.push(GlyphReport {
                gid: *gid,
                name: name_for(&names_a, *gid),
                codepoint: Some(*cp),
            });
        }
    }
    for (cp, gid) in &cmap_b {
        if !cmap_a.contains_key(cp) {
            signature.new.push(GlyphReport {
                gid: *gid,
                name: name_for(&names_b, *gid),
                codepoint: Some(*cp),
            });
        }
    }

    // 3. Best-effort analysis of unencoded glyphs (ligatures, alternates,
    //    components, ...), matched between the fonts by their default outline.
    //    The match is heuristic, so the *report* includes any differences we
    //    find, but every unencoded glyph is also added to `uncertain` to force
    //    the exhaustive fallback during word selection.
    let outlines_a = fontref_a.outline_glyphs();
    let outlines_b = fontref_b.outline_glyphs();

    // Index font B's unclaimed glyphs by default outline hash.
    let mut hash_index_b: HashMap<u64, Vec<GlyphId>> = HashMap::default();
    for (gid, _) in outlines_b.iter() {
        if claimed_b.contains(&gid) {
            continue;
        }
        if let Some(hash) = outline_hash(&outlines_b, gid, &[]) {
            hash_index_b.entry(hash).or_default().push(gid);
        }
    }

    for (gid_a, _) in outlines_a.iter() {
        if claimed_a.contains(&gid_a) {
            continue;
        }
        let report = GlyphReport {
            gid: gid_a,
            name: name_for(&names_a, gid_a),
            codepoint: None,
        };
        let Some(hash) = outline_hash(&outlines_a, gid_a, &[]) else {
            signature.uncertain.push(report);
            continue;
        };
        let candidates = hash_index_b.get(&hash).cloned().unwrap_or_default();
        if let [gid_b] = candidates.as_slice() {
            // Unique default-outline match: worth reporting any differences.
            if let GlyphComparison::Changed {
                outline_locations,
                advance_locations,
            } = compare_glyphs(font_a, font_b, gid_a, *gid_b)
            {
                signature.glyph_changes.insert(
                    gid_a,
                    GlyphChange {
                        gid_b: *gid_b,
                        name: name_for(&names_a, gid_a),
                        codepoint: None,
                        matched_by: MatchMethod::DefaultOutline,
                        outline_locations,
                        advance_locations,
                    },
                );
            }
        }
        // Either way, the pairing is heuristic: mark it as needing fallback.
        signature.uncertain.push(report);
    }

    signature
}

/// The result of comparing a single matched pair of glyphs.
#[derive(Debug, Clone, PartialEq)]
enum GlyphComparison {
    /// Identical outlines and advances at every tested location.
    Same,
    /// Differs at the given locations.
    Changed {
        outline_locations: LocationSet,
        advance_locations: LocationSet,
    },
    /// Neither glyph can be drawn; we can't say anything useful.
    Unavailable,
}

/// Compare one glyph from each font across a set of locations.
///
/// The locations tested are the default location plus the union of both
/// fonts' variation peaks (`gvar` tuple peaks) for the glyph.
fn compare_glyphs(
    font_a: &DFont,
    font_b: &DFont,
    gid_a: GlyphId,
    gid_b: GlyphId,
) -> GlyphComparison {
    let fontref_a = font_a.fontref();
    let fontref_b = font_b.fontref();
    let outlines_a = fontref_a.outline_glyphs();
    let outlines_b = fontref_b.outline_glyphs();

    // Collect the locations to test: default + union of both fonts' peaks.
    let mut locations = LocationSet::default();
    locations.insert(NormalizedLocation::default());
    for loc in font_a.variations_for_glyph(&gid_a) {
        locations.insert(loc);
    }
    for loc in font_b.variations_for_glyph(&gid_b) {
        locations.insert(loc);
    }

    // If only one of them has an outline, that's a difference everywhere.
    let drawable_a = outlines_a.get(gid_a).is_some();
    let drawable_b = outlines_b.get(gid_b).is_some();
    match (drawable_a, drawable_b) {
        (false, false) => return GlyphComparison::Unavailable,
        (true, false) | (false, true) => {
            return GlyphComparison::Changed {
                outline_locations: locations,
                advance_locations: LocationSet::default(),
            };
        }
        (true, true) => {}
    }

    let mut outline_locations = LocationSet::default();
    let mut advance_locations = LocationSet::default();

    for loc in &locations {
        let coords_a = font_a.normalized_location_to_coords(loc);
        let coords_b = font_b.normalized_location_to_coords(loc);

        match (
            outline_hash(&outlines_a, gid_a, &coords_a),
            outline_hash(&outlines_b, gid_b, &coords_b),
        ) {
            (Some(hash_a), Some(hash_b)) => {
                if hash_a != hash_b {
                    outline_locations.insert(loc.clone());
                }
            }
            // One drawable, one not: a difference at this location.
            (None, None) => {}
            _ => {
                outline_locations.insert(loc.clone());
            }
        }

        if let (Some(adv_a), Some(adv_b)) = (
            advance_at(&fontref_a, gid_a, &coords_a),
            advance_at(&fontref_b, gid_b, &coords_b),
        ) {
            if adv_a != adv_b {
                advance_locations.insert(loc.clone());
            }
        }
    }

    if outline_locations.is_empty() && advance_locations.is_empty() {
        GlyphComparison::Same
    } else {
        GlyphComparison::Changed {
            outline_locations,
            advance_locations,
        }
    }
}

/// Draw a glyph's outline at the given location and hash the command stream.
///
/// Returns `None` if the glyph has no drawable outline. The hash is computed
/// from the exact same `zeno::Command` stream the renderer produces in its
/// stage-1 pass, so "same hash" means "same rendering" in the renderer's own
/// terms.
fn outline_hash(
    outlines: &OutlineGlyphCollection,
    gid: GlyphId,
    coords: &[NormalizedCoord],
) -> Option<u64> {
    let outlined = outlines.get(gid)?;
    let skrifa_coords: Vec<F2Dot14> = coords.iter().map(|coord| coord.to_f2dot14()).collect();
    let location = LocationRef::from(skrifa_coords.as_slice());
    let settings = DrawSettings::unhinted(Size::new(SAMPLE_SIZE), location);
    let mut pen = RecordingPen::default();
    outlined.draw(settings, &mut pen).ok()?;
    Some(hash_commands(&pen.buffer))
}

/// The advance width of a glyph at the given location, in the same (scaled)
/// space for both fonts.
fn advance_at(font: &skrifa::FontRef, gid: GlyphId, coords: &[NormalizedCoord]) -> Option<f32> {
    let skrifa_coords: Vec<F2Dot14> = coords.iter().map(|coord| coord.to_f2dot14()).collect();
    let location = LocationRef::from(skrifa_coords.as_slice());
    font.glyph_metrics(Size::new(SAMPLE_SIZE), location)
        .advance_width(gid)
}

/// Hash a stream of outline commands into a single u64.
///
/// This is a stable, exact hash: two command streams hash the same iff they
/// are byte-for-byte identical (same command discriminants and same f32
/// bit patterns).
fn hash_commands(commands: &[Command]) -> u64 {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    let mut hasher = DefaultHasher::new();
    for command in commands {
        match command {
            Command::MoveTo(to) => {
                0u8.hash(&mut hasher);
                to.x.to_bits().hash(&mut hasher);
                to.y.to_bits().hash(&mut hasher);
            }
            Command::LineTo(to) => {
                1u8.hash(&mut hasher);
                to.x.to_bits().hash(&mut hasher);
                to.y.to_bits().hash(&mut hasher);
            }
            Command::QuadTo(ctrl, to) => {
                2u8.hash(&mut hasher);
                ctrl.x.to_bits().hash(&mut hasher);
                ctrl.y.to_bits().hash(&mut hasher);
                to.x.to_bits().hash(&mut hasher);
                to.y.to_bits().hash(&mut hasher);
            }
            Command::CurveTo(ctrl0, ctrl1, to) => {
                3u8.hash(&mut hasher);
                ctrl0.x.to_bits().hash(&mut hasher);
                ctrl0.y.to_bits().hash(&mut hasher);
                ctrl1.x.to_bits().hash(&mut hasher);
                ctrl1.y.to_bits().hash(&mut hasher);
                to.x.to_bits().hash(&mut hasher);
                to.y.to_bits().hash(&mut hasher);
            }
            Command::Close => {
                4u8.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

/// Best-effort glyph name for reporting.
fn name_for(names: &skrifa::GlyphNames, gid: GlyphId) -> String {
    names
        .get(gid)
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("gid{}", gid.to_u32()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a DFont if the test font exists, else None.
    fn dfont(path: &str) -> Option<DFont> {
        let data = std::fs::read(path).ok()?;
        Some(DFont::new(&data))
    }

    /// A font compared with itself must produce no changes.
    #[test]
    fn self_comparison_is_identical() {
        let Some(font) = dfont("../../MavenPro-Regular.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let sig = compute_signature(&font, &font);
        assert!(
            sig.is_identical(),
            "font should be identical to itself, got {sig:?}"
        );
        assert!(sig.glyph_changes.is_empty());
        assert!(sig.uncertain.iter().all(|r| r.codepoint.is_none()));
    }

    /// MavenPro-Modified is a hand-modified version of MavenPro-Regular used
    /// as the canonical diffenator test pair: some glyphs should differ.
    #[test]
    fn mavenpro_regular_vs_modified() {
        let Some(font_a) = dfont("../../MavenPro-Regular.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let Some(font_b) = dfont("../../MavenPro-Modified.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let sig = compute_signature(&font_a, &font_b);
        assert!(
            sig.needs_testing(),
            "the modified MavenPro should differ from the regular one"
        );
        // All high-confidence changes should be cmap-matched.
        assert!(sig
            .glyph_changes
            .values()
            .all(|change| matches!(change.matched_by, MatchMethod::Cmap(_))));
    }
}
