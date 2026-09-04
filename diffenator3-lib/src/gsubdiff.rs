//! GSUB-based glyph correspondence and confidence analysis.
//!
//! The static difference signature in [`crate::staticdiff`] can compare two
//! glyphs with confidence only when both are reachable through a shared cmap
//! codepoint. Every other glyph -- ligatures, alternates, decomposition
//! components and other unencoded glyphs -- only ever appears in a shaped
//! buffer via a `GSUB` substitution, so the older analysis conservatively
//! marked them all `uncertain`, forcing the word selector to fall back to
//! exhaustive rendering for any word that contains one.
//!
//! This module replaces that blanket uncertainty with a *trace*: it walks the
//! `GSUB` substitution rules of both fonts and matches them against each other
//! to establish a glyph correspondence for the unencoded glyphs they produce.
//! For example, if both fonts decompose `aacute` (U+00E1) into `a` +
//! `acutecomb`, and `aacute`/`a` are matched through the shared cmap, then the
//! decomposition rule tells us `acutecomb` in font A corresponds to
//! `acutecomb` in font B even though neither is encoded. Once the
//! correspondence is established we run the same outline/advance comparison
//! the cmap-matched glyphs get, and classify the unencoded glyph as identical,
//! changed, or still unverifiable -- instead of reflexively treating it as
//! uncertain.
//!
//! The correspondence is bootstrapped to a fixed point: matching a rule's
//! inputs (via the already-known correspondence) establishes its outputs'
//! correspondence, which in turn lets later rules be matched.
//!
//! Several rules often share an input glyph (e.g. `zero -> zero.dnom` and
//! `zero -> zero.numr` both substitute from `zero`). In that case the rule's
//! *output* outlines are used to disambiguate: the candidate whose outputs
//! look like font A's is the one that fired in font B.
//!
//! Soundness notes (mirroring the GPOS analysis in [`crate::gposdiff`]):
//!
//! * A rule is only trusted when an *identical* rule (same input and output
//!   glyphs, under the correspondence) exists in the other font. If the fonts
//!   shape differently, the rule does not match and the glyphs it produces
//!   stay uncertain.
//! * Glyph-level identity (identical outline and advance at every tested
//!   location) is verified with [`crate::staticdiff::compare_glyphs`] before a
//!   glyph is trusted, so a glyph that actually changed is reported as changed
//!   (and its word rendered at the right locations) rather than silently
//!   trusted.
//! * Contextual and chain-contextual lookups are not modelled; their presence
//!   sets [`GsubAnalysis::unmodelled`] so the caller knows the trace may be
//!   incomplete.
//! * Like the existing cmap-based analysis this does not model feature gating
//!   or lookup order; a rule that exists in both fonts is assumed to fire in
//!   the same situations. This is the same approximation the glyph pass makes
//!   for encoded glyphs.

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use read_fonts::{
    tables::gsub::{
        AlternateSubstFormat1, LigatureSubstFormat1, MultipleSubstFormat1,
        ReverseChainSingleSubstFormat1, SingleSubst, SubstitutionSubtables,
    },
    ReadError, TableProvider,
};
use skrifa::{GlyphId, GlyphId16, MetadataProvider};

use crate::{
    dfont::DFont,
    staticdiff::{compare_glyphs, outline_hash, GlyphComparison, LocationSet},
};

/// A single substitution rule: a sequence of input glyphs replaced by a
/// sequence of output glyphs. Both sequences are in one font's glyph ids.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Edge {
    inputs: Vec<GlyphId>,
    outputs: Vec<GlyphId>,
}

/// The result of tracing unencoded glyphs through `GSUB`.
#[derive(Debug, Clone, Default)]
pub struct GsubAnalysis {
    /// Font A glyph ids proven to render identically in both fonts (same
    /// outline and advance at every tested location). These are unencoded
    /// glyphs whose correspondence was established by matching `GSUB`
    /// substitution rules between the fonts, and which then compared
    /// identical. They no longer need to be treated as `uncertain`.
    pub safe: HashSet<GlyphId>,
    /// Unencoded font A glyph ids whose outline/advance differs at some
    /// location, keyed to the `(outline_locations, advance_locations)` where
    /// they differ. These should be reported as glyph changes rather than
    /// being treated as `uncertain`.
    pub changed: HashMap<GlyphId, (LocationSet, LocationSet)>,
    /// The font A -> font B glyph correspondence: the cmap seed plus the
    /// unencoded glyphs matched through `GSUB` rules. The caller can use this
    /// to extend other per-glyph analyses (e.g. GPOS) to the unencoded glyphs.
    pub correspondence: HashMap<GlyphId, GlyphId>,
    /// True if either font has contextual/chain-contextual `GSUB` lookups,
    /// which this analysis does not model, so the trace may be incomplete.
    pub unmodelled: bool,
}

/// Trace the unencoded glyphs of font A through the `GSUB` substitution rules
/// of both fonts, establishing which of them correspond to a font B glyph and
/// whether that pair is identical.
///
/// `cmap_a2b`/`cmap_b2a` are the shared-codepoint correspondences (font A ->
/// font B and back); they seed the correspondence and are never disturbed.
pub fn analyze_gsub(
    font_a: &DFont,
    font_b: &DFont,
    cmap_a2b: &HashMap<GlyphId, GlyphId>,
    cmap_b2a: &HashMap<GlyphId, GlyphId>,
) -> GsubAnalysis {
    let (edges_a, unmodelled_a) = extract_edges(font_a);
    let (edges_b, unmodelled_b) = extract_edges(font_b);

    // --- Phase 1: bootstrap the glyph correspondence through matched rules.
    //
    // `corr_a2b`/`corr_b2a` start as the cmap correspondence and grow as rules
    // are matched. A glyph whose correspondence is ambiguous (a rule that
    // would match it disagrees with an already-established mapping) is removed
    // and never re-established; the trace cannot trust it.
    let mut corr_a2b = cmap_a2b.clone();
    let mut corr_b2a = cmap_b2a.clone();
    let mut ambiguous: HashSet<GlyphId> = HashSet::default();

    // Font B's edges indexed by their input sequence (in B glyph ids). An A
    // rule whose inputs are all corresponded maps onto exactly the key to look
    // up, so a pass is linear in the number of rules.
    let mut b_index: HashMap<Vec<GlyphId>, Vec<usize>> = HashMap::default();
    for (idx, edge) in edges_b.iter().enumerate() {
        b_index.entry(edge.inputs.clone()).or_default().push(idx);
    }

    // Default-outline hashes, used to disambiguate rules that share an input
    // glyph (e.g. `zero -> zero.dnom` and `zero -> zero.numr`): among the
    // candidates, the rule whose outputs look like font A's is the one that
    // fired in font B.
    let outlines_a = font_a.fontref().outline_glyphs();
    let outlines_b = font_b.fontref().outline_glyphs();
    let hash_a: HashMap<GlyphId, u64> = outlines_a
        .iter()
        .filter_map(|(gid, _)| outline_hash(&outlines_a, gid, &[]).map(|hash| (gid, hash)))
        .collect();
    let hash_b: HashMap<GlyphId, u64> = outlines_b
        .iter()
        .filter_map(|(gid, _)| outline_hash(&outlines_b, gid, &[]).map(|hash| (gid, hash)))
        .collect();

    loop {
        let mut changed = false;

        for edge_a in &edges_a {
            // The A rule's inputs must all be corresponded before it can match
            // anything in font B.
            let key: Vec<GlyphId> = match edge_a
                .inputs
                .iter()
                .map(|gid| corr_a2b.get(gid).copied())
                .collect::<Option<Vec<_>>>()
            {
                Some(key) => key,
                None => continue,
            };
            let Some(candidates) = b_index.get(&key) else {
                continue;
            };
            // Several rules with the same input sequence but different outputs
            // are ambiguous; try to disambiguate them by output outline.
            let mut matching: Vec<usize> = candidates
                .iter()
                .copied()
                .filter(|&idx| edges_b[idx].outputs.len() == edge_a.outputs.len())
                .collect();
            if matching.len() > 1 {
                let a_hashes: Option<Vec<u64>> = edge_a
                    .outputs
                    .iter()
                    .map(|gid| hash_a.get(gid).copied())
                    .collect();
                let outline_matches: Vec<usize> = matching
                    .iter()
                    .copied()
                    .filter(|&idx| {
                        let Some(a_hashes) = &a_hashes else {
                            return false;
                        };
                        let b_hashes: Option<Vec<u64>> = edges_b[idx]
                            .outputs
                            .iter()
                            .map(|gid| hash_b.get(gid).copied())
                            .collect();
                        b_hashes.as_ref() == Some(a_hashes)
                    })
                    .collect();
                if outline_matches.len() == 1 {
                    matching = outline_matches;
                }
            }
            if matching.len() != 1 {
                continue;
            }
            let edge_b = &edges_b[matching[0]];

            for (gid_a, gid_b) in edge_a.outputs.iter().zip(&edge_b.outputs) {
                if ambiguous.contains(gid_a) {
                    continue;
                }
                // Encoded glyphs: the cmap correspondence is ground truth. A
                // rule whose output disagrees with it is simply not trusted for
                // that output; the cmap mapping is never disturbed.
                if cmap_a2b.contains_key(gid_a) {
                    continue;
                }
                match corr_a2b.get(gid_a) {
                    Some(existing) if existing == gid_b => {}
                    _ => {
                        let reverse_ok = corr_b2a.get(gid_b).is_none_or(|other| other == gid_a);
                        if corr_a2b.contains_key(gid_a) || !reverse_ok {
                            // Conflict: this output is produced by rules that
                            // map it to different font B glyphs.
                            ambiguous.insert(*gid_a);
                            if let Some(old) = corr_a2b.remove(gid_a) {
                                corr_b2a.remove(&old);
                            }
                            changed = true;
                        } else {
                            corr_a2b.insert(*gid_a, *gid_b);
                            corr_b2a.insert(*gid_b, *gid_a);
                            changed = true;
                        }
                    }
                }
            }
        }

        if !changed {
            break;
        }
    }

    // --- Phase 2: classify each newly-corresponded unencoded glyph.
    //
    // The correspondence only tells us *which* font B glyph a font A glyph
    // should be compared against. Glyph-level identity is still verified by
    // the same outline/advance comparison the cmap-matched glyphs get.
    let mut safe = HashSet::default();
    let mut changed: HashMap<GlyphId, (LocationSet, LocationSet)> = HashMap::default();
    for (gid_a, gid_b) in &corr_a2b {
        if cmap_a2b.contains_key(gid_a) {
            continue;
        }
        match compare_glyphs(font_a, font_b, *gid_a, *gid_b) {
            GlyphComparison::Same => {
                safe.insert(*gid_a);
            }
            GlyphComparison::Changed {
                outline_locations,
                advance_locations,
            } => {
                changed.insert(*gid_a, (outline_locations, advance_locations));
            }
            GlyphComparison::Unavailable => {
                // Nothing drawable on either side; can't verify identity.
            }
        }
    }

    GsubAnalysis {
        safe,
        changed,
        correspondence: corr_a2b,
        unmodelled: unmodelled_a || unmodelled_b,
    }
}

/// Walk every non-contextual `GSUB` lookup and extract its substitution rules
/// as [`Edge`]s. Contextual/chain-contextual lookups are not modelled; their
/// presence sets the returned `unmodelled` flag.
fn extract_edges(font: &DFont) -> (Vec<Edge>, bool) {
    let mut edges: HashSet<Edge> = HashSet::default();
    let mut unmodelled = false;

    if let Ok(gsub) = font.fontref().gsub() {
        if let Ok(lookup_list) = gsub.lookup_list() {
            for lookuprec in lookup_list.lookups().iter() {
                let Ok(lookup) = lookuprec else {
                    continue;
                };
                let Ok(subtables) = lookup.subtables() else {
                    continue;
                };
                match subtables {
                    SubstitutionSubtables::Single(st) => {
                        for sub in st.iter().flatten() {
                            let _ = single_edges(&sub, &mut edges);
                        }
                    }
                    SubstitutionSubtables::Multiple(st) => {
                        for sub in st.iter().flatten() {
                            let _ = multiple_edges(&sub, &mut edges);
                        }
                    }
                    SubstitutionSubtables::Alternate(st) => {
                        for sub in st.iter().flatten() {
                            let _ = alternate_edges(&sub, &mut edges);
                        }
                    }
                    SubstitutionSubtables::Ligature(st) => {
                        for sub in st.iter().flatten() {
                            let _ = ligature_edges(&sub, &mut edges);
                        }
                    }
                    SubstitutionSubtables::Reverse(st) => {
                        for sub in st.iter().flatten() {
                            let _ = reverse_edges(&sub, &mut edges);
                        }
                    }
                    SubstitutionSubtables::Contextual(_)
                    | SubstitutionSubtables::ChainContextual(_)
                    | SubstitutionSubtables::EmptyExtension => {
                        unmodelled = true;
                    }
                }
            }
        }
    }

    (edges.into_iter().collect(), unmodelled)
}

fn to_gid(gid: GlyphId16) -> GlyphId {
    GlyphId::new(gid.to_u16() as u32)
}

/// Lookup type 1: single substitution (`a -> a.alt`).
fn single_edges(sub: &SingleSubst, edges: &mut HashSet<Edge>) -> Result<(), ReadError> {
    match sub {
        SingleSubst::Format1(format1) => {
            let coverage = format1.coverage()?;
            let delta = format1.delta_glyph_id();
            for input in coverage.iter() {
                let output = (input.to_u16() as i16 + delta) as u16;
                edges.insert(Edge {
                    inputs: vec![to_gid(input)],
                    outputs: vec![GlyphId::new(output as u32)],
                });
            }
        }
        SingleSubst::Format2(format2) => {
            let coverage = format2.coverage()?;
            for (input, output) in coverage.iter().zip(format2.substitute_glyph_ids()) {
                edges.insert(Edge {
                    inputs: vec![to_gid(input)],
                    outputs: vec![to_gid(output.get())],
                });
            }
        }
    }
    Ok(())
}

/// Lookup type 2: multiple substitution (decomposition,
/// `aacute -> a acutecomb`).
fn multiple_edges(sub: &MultipleSubstFormat1, edges: &mut HashSet<Edge>) -> Result<(), ReadError> {
    let coverage = sub.coverage()?;
    for (input, sequence) in coverage.iter().zip(sub.sequences().iter().flatten()) {
        edges.insert(Edge {
            inputs: vec![to_gid(input)],
            outputs: sequence
                .substitute_glyph_ids()
                .iter()
                .map(|gid| to_gid(gid.get()))
                .collect(),
        });
    }
    Ok(())
}

/// Lookup type 3: alternate substitution (`a -> a.swash`), one edge per
/// alternate glyph.
fn alternate_edges(
    sub: &AlternateSubstFormat1,
    edges: &mut HashSet<Edge>,
) -> Result<(), ReadError> {
    let coverage = sub.coverage()?;
    for (input, alternate_set) in coverage.iter().zip(sub.alternate_sets().iter().flatten()) {
        for alternate in alternate_set.alternate_glyph_ids().iter() {
            edges.insert(Edge {
                inputs: vec![to_gid(input)],
                outputs: vec![to_gid(alternate.get())],
            });
        }
    }
    Ok(())
}

/// Lookup type 4: ligature substitution (`f i -> fi`).
fn ligature_edges(sub: &LigatureSubstFormat1, edges: &mut HashSet<Edge>) -> Result<(), ReadError> {
    let coverage = sub.coverage()?;
    for (first, ligature_set) in coverage.iter().zip(sub.ligature_sets().iter().flatten()) {
        for ligature in ligature_set.ligatures().iter().flatten() {
            let mut inputs = vec![to_gid(first)];
            inputs.extend(
                ligature
                    .component_glyph_ids()
                    .iter()
                    .map(|gid| to_gid(gid.get())),
            );
            edges.insert(Edge {
                inputs,
                outputs: vec![to_gid(ligature.ligature_glyph())],
            });
        }
    }
    Ok(())
}

/// Lookup type 8: reverse chain single substitution (`a -> a.fina`).
///
/// The backtrack/lookahead context is not modelled, so a rule is matched
/// solely on its input/output glyphs. This is the same approximation the other
/// edge types make about feature gating.
fn reverse_edges(
    sub: &ReverseChainSingleSubstFormat1,
    edges: &mut HashSet<Edge>,
) -> Result<(), ReadError> {
    let coverage = sub.coverage()?;
    for (input, output) in coverage.iter().zip(sub.substitute_glyph_ids().iter()) {
        edges.insert(Edge {
            inputs: vec![to_gid(input)],
            outputs: vec![to_gid(output.get())],
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a DFont if the test font exists, else None.
    fn dfont(path: &str) -> Option<DFont> {
        let data = std::fs::read(path).ok()?;
        Some(DFont::new(&data))
    }

    /// The identity correspondence for a font compared with itself.
    fn self_cmap(font: &DFont) -> HashMap<GlyphId, GlyphId> {
        font.fontref()
            .charmap()
            .mappings()
            .map(|(_cp, gid)| (gid, gid))
            .collect()
    }

    /// The shared-codepoint correspondence between two fonts.
    fn cmap_pair(
        font_a: &DFont,
        font_b: &DFont,
    ) -> (HashMap<GlyphId, GlyphId>, HashMap<GlyphId, GlyphId>) {
        let mut a2b = HashMap::default();
        let mut b2a = HashMap::default();
        let cmap_a: HashMap<u32, GlyphId> = font_a.fontref().charmap().mappings().collect();
        let cmap_b: HashMap<u32, GlyphId> = font_b.fontref().charmap().mappings().collect();
        for (cp, gid_a) in &cmap_a {
            if let Some(gid_b) = cmap_b.get(cp) {
                a2b.insert(*gid_a, *gid_b);
                b2a.insert(*gid_b, *gid_a);
            }
        }
        (a2b, b2a)
    }

    /// A font traced against itself must find no changes, and must prove some
    /// of its unencoded glyphs safe (otherwise the trace is useless).
    #[test]
    fn self_trace_proves_unencoded_glyphs_safe() {
        let Some(font) = dfont("../test-fonts/MavenPro-Regular.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let cmap = self_cmap(&font);
        let analysis = analyze_gsub(&font, &font, &cmap, &cmap);
        assert!(
            analysis.changed.is_empty(),
            "self-trace must not report changes, got {:?}",
            analysis.changed.keys().collect::<Vec<_>>()
        );
        let safe_unencoded: Vec<GlyphId> = analysis
            .safe
            .iter()
            .copied()
            .filter(|gid| !cmap.contains_key(gid))
            .collect();
        assert!(
            !safe_unencoded.is_empty(),
            "expected the trace to prove some unencoded glyphs safe"
        );
    }

    /// A pair whose outlines are identical (only kerning changed) must not
    /// produce any GSUB changes -- the trace reports outline/advance identity,
    /// not positioning.
    #[test]
    fn trace_with_kern_change_only_reports_no_glyph_changes() {
        let Some(font_a) = dfont("test-data/Martel-Original.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let Some(font_b) = dfont("test-data/Martel-KernChange.ttf") else {
            eprintln!("skipping: test font not present");
            return;
        };
        let (a2b, b2a) = cmap_pair(&font_a, &font_b);
        let analysis = analyze_gsub(&font_a, &font_b, &a2b, &b2a);
        assert!(
            analysis.changed.is_empty(),
            "identical outlines should yield no GSUB changes, got {:?}",
            analysis.changed.keys().collect::<Vec<_>>()
        );
        // The correspondence should extend beyond the cmap seed to unencoded
        // glyphs (Martel decomposes/precomposes heavily), proving the trace
        // actually matched substitution rules between the two fonts.
        assert!(
            analysis.correspondence.len() > a2b.len(),
            "expected the trace to add unencoded glyphs to the correspondence"
        );
    }
}
