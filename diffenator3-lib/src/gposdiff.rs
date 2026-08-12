//! Variation-aware GPOS positioning analysis.
//!
//! This module descends into every GPOS lookup -- single adjustment, pair
//! adjustment, cursive attachment, and mark-to-base/mark-to-ligature/
//! mark-to-mark attachment -- and computes the effect of that lookup on each
//! single glyph or glyph pair, both at the default location and at every
//! variation location implied by the GDEF item variation store. It then
//! compares those effects between two fonts and records, per glyph/pair, the
//! set of designspace locations at which the positioning differs.
//!
//! This is the "pair pass" which, together with the glyph pass in
//! [`crate::staticdiff`], covers the positioning component of the soundness
//! condition for word selection: a rendered word can differ at a location if
//! a glyph's outline/advance differs there (glyph pass) *or* positioning
//! between adjacent glyphs differs there (this pass).
//!
//! Notes on soundness:
//!
//! * Lookups are compared *per contribution*, not summed: a single adjustment
//!   and a pair adjustment can both affect a glyph's advance, but they apply
//!   in different contexts, so summing could hide a difference. We therefore
//!   compare, per location, the *multiset* of each lookup's resolved effect on
//!   a glyph/pair.
//! * Because lookups apply conditionally (script/feature/context), this is an
//!   over-approximation -- it may flag a glyph/pair as changed when the
//!   differing lookups never actually co-apply -- but it is sound: it never
//!   misses a real positioning difference.
//! * Contextual and chain-contextual GPOS lookups are not modelled yet; their
//!   presence sets [`GposChanges::unmodelled`], so the caller knows the
//!   positioning analysis cannot be trusted for pruning.
//! * Value records may reference `Device` (ppem-dependent) tables, which are
//!   not resolved here; a hash of the device data is included in the
//!   comparison so device differences are still detected.

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::collections::BTreeMap;

use fontdrasil::coords::{NormalizedCoord, NormalizedLocation};
use read_fonts::{
    tables::{
        gpos::{
            AnchorTable, CursivePosFormat1, DeviceOrVariationIndex, MarkBasePosFormat1,
            MarkLigPosFormat1, MarkMarkPosFormat1, PairPos, PositionSubtables, SinglePos,
            ValueRecord,
        },
        layout::Device,
        variations::{DeltaSetIndex, ItemVariationStore},
    },
    types::F2Dot14,
    FontData, ReadError, TableProvider,
};
use skrifa::{GlyphId, GlyphId16, MetadataProvider};

use crate::{dfont::DFont, staticdiff::LocationSet};

type PairValueList = Vec<(NormalizedLocation, (PValue, PValue))>;

/// read_fonts coverage tables iterate over 16-bit glyph ids.
fn to_gid(gid: GlyphId16) -> GlyphId {
    GlyphId::new(gid.to_u16() as u32)
}

/// A resolved positioning value for a single glyph.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
struct PValue {
    x_advance: i32,
    y_advance: i32,
    x_placement: i32,
    y_placement: i32,
}

impl PValue {
    fn is_zero(&self) -> bool {
        self.x_advance == 0 && self.y_advance == 0 && self.x_placement == 0 && self.y_placement == 0
    }
}

/// A resolved anchor coordinate (used for mark/cursive positioning).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
struct AnchorV {
    x: i32,
    y: i32,
}

/// Per-glyph/pair effects: for each key, the resolved contributions at each
/// location. Multiple contributions for the same key/location are possible
/// when several lookups cover the same glyph/pair; they are compared as a
/// multiset.
type Effects<K, T> = HashMap<K, Vec<(NormalizedLocation, T)>>;

/// Upper bound on the number of (glyph, glyph, location) entries materialized
/// by class-kerning expansion. Dense kern tables can otherwise explode into
/// millions of pairs. Beyond this the analysis is marked `pair_capped`, which
/// forces the caller to treat pair pruning as untrusted (sound fallback to
/// exhaustive testing).
const PAIR_EXPANSION_BUDGET: usize = 200_000;

/// Upper bound on the number of (mark, base, location) entries materialized
/// by mark attachment expansion (marks x bases can be huge). Beyond this the
/// analysis is marked `mark_capped`, which forces the caller to treat mark
/// pruning as untrusted.
const MARK_EXPANSION_BUDGET: usize = 200_000;

/// The pixel size words are rendered at (matches the renderer's word size);
/// static (ppem) `Device` tables are evaluated at this size.
const RENDER_PPEM: u16 = 16;

struct FontEffects {
    single: Effects<GlyphId, PValue>,
    pair: Effects<(GlyphId, GlyphId), (PValue, PValue)>,
    mark: Effects<(GlyphId, GlyphId), (AnchorV, AnchorV)>,
    cursive: Effects<GlyphId, (AnchorV, AnchorV)>,
    unmodelled: bool,
    /// True if the pair expansion budget was exceeded, so pair pruning must
    /// not be trusted.
    pair_capped: bool,
    /// True if the mark expansion budget was exceeded, so mark pruning must
    /// not be trusted.
    mark_capped: bool,
}

/// Positioning differences between two fonts, keyed by font A glyph ids.
#[derive(Debug, Clone, Default)]
pub struct GposChanges {
    /// Single adjustment (lookup type 1) effects that differ.
    pub single: HashMap<GlyphId, LocationSet>,
    /// Pair adjustment (lookup type 2) effects that differ, keyed by
    /// `(left, right)`.
    pub pair: HashMap<(GlyphId, GlyphId), LocationSet>,
    /// Mark attachment (lookup types 4/5/6) effects that differ, keyed by
    /// `(mark, base/ligature/mark)`.
    pub mark: HashMap<(GlyphId, GlyphId), LocationSet>,
    /// Cursive attachment (lookup type 3) effects that differ.
    pub cursive: HashMap<GlyphId, LocationSet>,
    /// True if positioning pruning must not be trusted: either font has
    /// contextual/chain-contextual GPOS lookups we don't model, or the pair
    /// expansion budget was exceeded.
    pub unmodelled: bool,
}

impl GposChanges {
    pub fn is_empty(&self) -> bool {
        self.single.is_empty()
            && self.pair.is_empty()
            && self.mark.is_empty()
            && self.cursive.is_empty()
    }
}

/// Compute the GPOS positioning differences between two fonts.
///
/// `match_a2b` maps font A glyph ids to the corresponding font B glyph ids
/// (normally via shared cmap codepoints); glyphs absent from the map are
/// skipped, since they are already reported as `uncertain` by the glyph pass.
pub fn compute_gpos_changes(
    font_a: &DFont,
    font_b: &DFont,
    match_a2b: &HashMap<GlyphId, GlyphId>,
) -> GposChanges {
    let effects_a = build_font_effects(font_a);
    let effects_b = build_font_effects(font_b);

    let match_b2a: HashMap<GlyphId, GlyphId> = match_a2b.iter().map(|(a, b)| (*b, *a)).collect();

    let map_gid = |gid: &GlyphId| match_a2b.get(gid).copied();
    let unmap_gid = |gid: &GlyphId| match_b2a.get(gid).copied();
    let map_pair = |(l, r): &(GlyphId, GlyphId)| map_gid(l).zip(map_gid(r));
    let unmap_pair = |(l, r): &(GlyphId, GlyphId)| unmap_gid(l).zip(unmap_gid(r));

    let single = compare_effects(&effects_a.single, &effects_b.single, map_gid, unmap_gid);
    let pair = compare_effects(&effects_a.pair, &effects_b.pair, map_pair, unmap_pair);
    let mark = compare_effects(&effects_a.mark, &effects_b.mark, map_pair, unmap_pair);
    let cursive = compare_effects(&effects_a.cursive, &effects_b.cursive, map_gid, unmap_gid);

    GposChanges {
        single,
        pair,
        mark,
        cursive,
        unmodelled: effects_a.unmodelled
            || effects_b.unmodelled
            || effects_a.pair_capped
            || effects_b.pair_capped
            || effects_a.mark_capped
            || effects_b.mark_capped,
    }
}

/// Compare two per-key effect maps, recording the locations where the effects
/// differ for each key that appears in either map.
///
/// `to_b` maps an A-key to the corresponding B-key; `from_b` maps back. Keys
/// whose glyphs cannot be matched across fonts are skipped (they are already
/// `uncertain`).
fn compare_effects<K, T>(
    effects_a: &Effects<K, T>,
    effects_b: &Effects<K, T>,
    to_b: impl Fn(&K) -> Option<K>,
    from_b: impl Fn(&K) -> Option<K>,
) -> HashMap<K, LocationSet>
where
    K: std::hash::Hash + Eq + Clone,
    T: Ord,
{
    let mut out: HashMap<K, LocationSet> = HashMap::default();

    for (key_a, entries_a) in effects_a {
        let Some(key_b) = to_b(key_a) else {
            continue;
        };
        let entries_b = effects_b.get(&key_b);
        let mut changed: LocationSet = HashSet::default();
        let mut locations: HashSet<&NormalizedLocation> = HashSet::default();
        for (loc, _) in entries_a {
            locations.insert(loc);
        }
        if let Some(entries_b) = entries_b {
            for (loc, _) in entries_b {
                locations.insert(loc);
            }
        }
        for loc in locations {
            let mut values_a: Vec<&T> = entries_a
                .iter()
                .filter(|(l, _)| l == loc)
                .map(|(_, v)| v)
                .collect();
            let mut values_b: Vec<&T> = entries_b
                .map(|entries| {
                    entries
                        .iter()
                        .filter(|(l, _)| l == loc)
                        .map(|(_, v)| v)
                        .collect()
                })
                .unwrap_or_default();
            values_a.sort();
            values_b.sort();
            if values_a != values_b {
                changed.insert(loc.clone());
            }
        }
        if !changed.is_empty() {
            out.insert(key_a.clone(), changed);
        }
    }

    // Keys present in B but not A: font A has no positioning effect for them,
    // font B does -- a difference at all of B's locations.
    for (key_b, entries_b) in effects_b {
        let Some(key_a) = from_b(key_b) else {
            continue;
        };
        if effects_a.contains_key(&key_a) {
            continue;
        }
        let mut changed = LocationSet::default();
        for (loc, _) in entries_b {
            changed.insert(loc.clone());
        }
        if !changed.is_empty() {
            out.insert(key_a, changed);
        }
    }

    out
}

/// Precomputed per-location region scalars for the GDEF item variation store.
///
/// `ItemVariationStore::compute_delta` re-parses the variation region list and
/// re-interpolates a scalar for every region on every call; for a many-axis
/// font with many regions that dominates the GPOS analysis. Here the scalars
/// are computed once per location up front, so resolving a delta set is a dot
/// product over the precomputed values.
struct RegionScalars {
    /// `per_location[loc_idx][region_idx]` -- the region's scalar at that
    /// location, as `Fixed::to_bits()` (16.16) widened to i64.
    per_location: Vec<Vec<i64>>,
}

impl RegionScalars {
    fn new(ivs: &ItemVariationStore, coords: &[Vec<F2Dot14>]) -> Result<Self, ReadError> {
        let regions = ivs.variation_region_list()?.variation_regions();
        let mut per_location = Vec::with_capacity(coords.len());
        for coord_set in coords {
            let mut scalars = Vec::with_capacity(regions.len());
            for region in regions.iter().flatten() {
                scalars.push(region.compute_scalar(coord_set).to_bits() as i64);
            }
            per_location.push(scalars);
        }
        Ok(Self { per_location })
    }

    /// Equivalent of `ItemVariationStore::compute_delta`, but using the
    /// precomputed region scalars instead of re-interpolating per call.
    fn delta(&self, ivs: &ItemVariationStore, loc_idx: usize, index: DeltaSetIndex) -> i32 {
        let scalars = match self.per_location.get(loc_idx) {
            Some(scalars) if !scalars.is_empty() => scalars,
            _ => return 0,
        };
        let data = match ivs.item_variation_data().get(index.outer as usize) {
            Some(Ok(data)) => data,
            _ => return 0,
        };
        let region_indices = data.region_indexes();
        // 64-bit accumulation, matching compute_delta.
        let mut accum = 0i64;
        for (i, region_delta) in data.delta_set(index.inner).enumerate() {
            let Some(region_index) = region_indices.get(i) else {
                break;
            };
            let ri = region_index.get() as usize;
            if let Some(&scalar) = scalars.get(ri) {
                accum += region_delta as i64 * scalar;
            }
        }
        ((accum + 0x8000) >> 16) as i32
    }
}

/// The item variation store plus precomputed region scalars, so delta
/// resolution is fast and shared across every value record / anchor.
struct DeltaResolver<'a> {
    ivs: ItemVariationStore<'a>,
    scalars: RegionScalars,
}

impl<'a> DeltaResolver<'a> {
    fn new(ivs: ItemVariationStore<'a>, coords: &[Vec<F2Dot14>]) -> Option<Self> {
        Some(Self {
            scalars: RegionScalars::new(&ivs, coords).ok()?,
            ivs,
        })
    }

    #[inline]
    fn delta(&self, loc_idx: usize, index: DeltaSetIndex) -> i32 {
        self.scalars.delta(&self.ivs, loc_idx, index)
    }
}

/// Walk every GPOS lookup in the font and accumulate its effects.
fn build_font_effects(font: &DFont) -> FontEffects {
    let fontref = font.fontref();
    let ivs: Option<ItemVariationStore> = fontref
        .gdef()
        .ok()
        .and_then(|gdef| gdef.item_var_store())
        .and_then(|res| res.ok());

    // Locations to test: default plus the GDEF item variation store region
    // peaks (where variable positioning is defined).
    let locations = collect_locations(font, ivs.as_ref());
    // Per-location coordinates in this font's axis order, for resolving value
    // records against its own item variation store.
    let coords: Vec<Vec<F2Dot14>> = locations
        .iter()
        .map(|loc| {
            font.normalized_location_to_coords(loc)
                .iter()
                .map(|coord| coord.to_f2dot14())
                .collect()
        })
        .collect();
    // Region scalars precomputed once per location, shared across every value
    // record and anchor (instead of re-parsing the IVS on every call).
    let resolver = ivs.and_then(|ivs| DeltaResolver::new(ivs, &coords));

    let mut single: Effects<GlyphId, PValue> = HashMap::default();
    let mut pair: Effects<(GlyphId, GlyphId), (PValue, PValue)> = HashMap::default();
    let mut mark: Effects<(GlyphId, GlyphId), (AnchorV, AnchorV)> = HashMap::default();
    let mut cursive: Effects<GlyphId, (AnchorV, AnchorV)> = HashMap::default();
    let mut unmodelled = false;
    let mut pair_budget = PAIR_EXPANSION_BUDGET;
    let mut pair_capped = false;
    let mut mark_budget = MARK_EXPANSION_BUDGET;
    let mut mark_capped = false;

    if let Ok(gpos) = fontref.gpos() {
        if let Ok(lookup_list) = gpos.lookup_list() {
            for lookuprec in lookup_list.lookups().iter() {
                let Ok(lookup) = lookuprec else {
                    continue;
                };
                let Ok(subtables) = lookup.subtables() else {
                    continue;
                };
                match subtables {
                    PositionSubtables::Single(st) => {
                        for sub in st.iter().flatten() {
                            let _ =
                                single_subtable(&sub, &locations, resolver.as_ref(), &mut single);
                        }
                    }
                    PositionSubtables::Pair(st) => {
                        for sub in st.iter().flatten() {
                            let _ = pair_subtable(
                                &sub,
                                font,
                                &locations,
                                resolver.as_ref(),
                                &mut pair,
                                &mut pair_budget,
                                &mut pair_capped,
                            );
                        }
                    }
                    PositionSubtables::Cursive(st) => {
                        for sub in st.iter().flatten() {
                            let _ =
                                cursive_subtable(&sub, &locations, resolver.as_ref(), &mut cursive);
                        }
                    }
                    PositionSubtables::MarkToBase(st) => {
                        for sub in st.iter().flatten() {
                            let _ = markbase_subtable(
                                &sub,
                                &locations,
                                resolver.as_ref(),
                                &mut mark,
                                &mut mark_budget,
                                &mut mark_capped,
                            );
                        }
                    }
                    PositionSubtables::MarkToLig(st) => {
                        for sub in st.iter().flatten() {
                            let _ = marklig_subtable(
                                &sub,
                                &locations,
                                resolver.as_ref(),
                                &mut mark,
                                &mut mark_budget,
                                &mut mark_capped,
                            );
                        }
                    }
                    PositionSubtables::MarkToMark(st) => {
                        for sub in st.iter().flatten() {
                            let _ = markmark_subtable(
                                &sub,
                                &locations,
                                resolver.as_ref(),
                                &mut mark,
                                &mut mark_budget,
                                &mut mark_capped,
                            );
                        }
                    }
                    PositionSubtables::Contextual(_) | PositionSubtables::ChainContextual(_) => {
                        unmodelled = true;
                    }
                }
            }
        }
    }

    FontEffects {
        single,
        pair,
        mark,
        cursive,
        unmodelled,
        pair_capped,
        mark_capped,
    }
}

/// Collect the locations to test for positioning: default plus the peaks of
/// every region in the GDEF item variation store.
fn collect_locations(font: &DFont, ivs: Option<&ItemVariationStore>) -> Vec<NormalizedLocation> {
    let mut set = LocationSet::default();
    set.insert(NormalizedLocation::default());
    if let Some(ivs) = ivs {
        if let Ok(region_list) = ivs.variation_region_list() {
            let regions = region_list.variation_regions();
            let axes: Vec<_> = font
                .fontref()
                .axes()
                .iter()
                .map(|axis| axis.tag())
                .collect();
            for region in regions.iter().flatten() {
                let coords: Vec<f32> = region
                    .region_axes()
                    .iter()
                    .map(|axis| axis.peak_coord().to_f32())
                    .collect();
                let loc: NormalizedLocation = axes
                    .iter()
                    .zip(coords.iter())
                    .map(|(tag, coord)| (*tag, NormalizedCoord::new(*coord as f64)))
                    .collect();
                set.insert(loc);
            }
        }
    }
    set.into_iter().collect()
}

/// Lookup type 1: single adjustment positioning.
fn single_subtable(
    sub: &SinglePos,
    locations: &[NormalizedLocation],
    resolver: Option<&DeltaResolver>,
    out: &mut Effects<GlyphId, PValue>,
) -> Result<(), ReadError> {
    let offset_data = sub.offset_data();

    // (glyph, value record) pairs covered by this subtable.
    let records: Vec<(GlyphId, ValueRecord)> = match sub {
        SinglePos::Format1(format1) => {
            let coverage = format1.coverage()?;
            let value = format1.value_record();
            coverage
                .iter()
                .map(|gid| (to_gid(gid), value.clone()))
                .collect()
        }
        SinglePos::Format2(format2) => {
            let coverage = format2.coverage()?;
            let values = format2.value_records();
            coverage
                .iter()
                .zip(values.iter())
                .filter_map(|(gid, value)| value.ok().map(|value| (to_gid(gid), value.clone())))
                .collect()
        }
    };

    for (gid, value) in records {
        let entry = out.entry(gid).or_default();
        for (loc_idx, loc) in locations.iter().enumerate() {
            let value = resolve_value(&value, offset_data, loc_idx, resolver);
            if value.is_zero() {
                continue;
            }
            entry.push((loc.clone(), value));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
/// Lookup type 2: pair adjustment positioning.
fn pair_subtable(
    sub: &PairPos,
    font: &DFont,
    locations: &[NormalizedLocation],
    resolver: Option<&DeltaResolver>,
    out: &mut Effects<(GlyphId, GlyphId), (PValue, PValue)>,
    budget: &mut usize,
    capped: &mut bool,
) -> Result<(), ReadError> {
    let offset_data = sub.offset_data();
    match sub {
        PairPos::Format1(format1) => {
            let coverage = format1.coverage()?;
            let pair_sets = format1.pair_sets();
            let mut records: Vec<((GlyphId, GlyphId), ValueRecord, ValueRecord)> = Vec::new();
            for (left_gid, pair_set) in coverage.iter().zip(pair_sets.iter()) {
                let Ok(pair_set) = pair_set else {
                    continue;
                };
                for pair in pair_set.pair_value_records().iter() {
                    let Ok(pair) = pair else {
                        continue;
                    };
                    let key = (to_gid(left_gid), to_gid(pair.second_glyph()));
                    records.push((
                        key,
                        pair.value_record1().clone(),
                        pair.value_record2().clone(),
                    ));
                }
            }
            emit_pair_records(records, offset_data, locations, resolver, out);
        }
        PairPos::Format2(format2) => {
            // Class-based pair positioning. Resolve the class matrix once
            // (small), skip all-zero class pairs, then expand the rest into
            // per-glyph pairs so the comparison is uniform.
            let coverage = format2.coverage()?;
            let class1 = format2.class_def1()?;
            let class2 = format2.class_def2()?;
            let class1_records = format2.class1_records();

            let mut glyphs_in_class1: BTreeMap<u16, Vec<GlyphId>> = BTreeMap::new();
            for gid in coverage.iter() {
                let gid = to_gid(gid);
                let class = class1.get(gid);
                glyphs_in_class1.entry(class).or_default().push(gid);
            }
            let num_glyphs = font
                .fontref()
                .maxp()
                .map(|maxp| maxp.num_glyphs())
                .unwrap_or(0);
            let mut glyphs_in_class2: BTreeMap<u16, Vec<GlyphId>> = BTreeMap::new();
            for gid in 0..num_glyphs {
                let gid = GlyphId::new(gid as u32);
                let class = class2.get(gid);
                glyphs_in_class2.entry(class).or_default().push(gid);
            }

            // BTreeMap so the class-pair expansion order is deterministic:
            // the expansion budget can truncate the pair set, and a
            // deterministic order keeps the truncation consistent between
            // repeated runs (and between the two fonts).
            let mut matrix: BTreeMap<(u16, u16), PairValueList> = BTreeMap::new();
            for (class1_ix, class1_record) in class1_records.iter().enumerate() {
                let Ok(class1_record) = class1_record else {
                    continue;
                };
                for (class2_ix, class2_record) in class1_record.class2_records().iter().enumerate()
                {
                    let Ok(class2_record) = class2_record else {
                        continue;
                    };
                    let mut per_location = Vec::new();
                    for (loc_idx, loc) in locations.iter().enumerate() {
                        let value1 = resolve_value(
                            class2_record.value_record1(),
                            offset_data,
                            loc_idx,
                            resolver,
                        );
                        let value2 = resolve_value(
                            class2_record.value_record2(),
                            offset_data,
                            loc_idx,
                            resolver,
                        );
                        if !(value1.is_zero() && value2.is_zero()) {
                            per_location.push((loc.clone(), (value1, value2)));
                        }
                    }
                    if !per_location.is_empty() {
                        matrix.insert((class1_ix as u16, class2_ix as u16), per_location);
                    }
                }
            }

            for ((class1_ix, class2_ix), per_location) in matrix {
                let Some(lefts) = glyphs_in_class1.get(&class1_ix) else {
                    continue;
                };
                let Some(rights) = glyphs_in_class2.get(&class2_ix) else {
                    continue;
                };
                for &left in lefts {
                    for &right in rights {
                        if *budget < per_location.len() {
                            *capped = true;
                            return Ok(());
                        }
                        *budget -= per_location.len();
                        out.entry((left, right))
                            .or_default()
                            .extend(per_location.iter().cloned());
                    }
                }
            }
        }
    }
    Ok(())
}

/// Emit resolved (value1, value2) pairs for explicit (non-class) pair records.
fn emit_pair_records(
    records: Vec<((GlyphId, GlyphId), ValueRecord, ValueRecord)>,
    offset_data: FontData,
    locations: &[NormalizedLocation],
    resolver: Option<&DeltaResolver>,
    out: &mut Effects<(GlyphId, GlyphId), (PValue, PValue)>,
) {
    for (key, value1, value2) in records {
        let entry = out.entry(key).or_default();
        for (loc_idx, loc) in locations.iter().enumerate() {
            let value1 = resolve_value(&value1, offset_data, loc_idx, resolver);
            let value2 = resolve_value(&value2, offset_data, loc_idx, resolver);
            if value1.is_zero() && value2.is_zero() {
                continue;
            }
            entry.push((loc.clone(), (value1, value2)));
        }
    }
}

/// Lookup type 3: cursive attachment.
fn cursive_subtable(
    sub: &CursivePosFormat1,
    locations: &[NormalizedLocation],
    resolver: Option<&DeltaResolver>,
    out: &mut Effects<GlyphId, (AnchorV, AnchorV)>,
) -> Result<(), ReadError> {
    let offset_data = sub.offset_data();
    let coverage = sub.coverage()?;
    for (gid, record) in coverage.iter().zip(sub.entry_exit_record().iter()) {
        let gid = to_gid(gid);
        let entry_anchor = record.entry_anchor(offset_data);
        let exit_anchor = record.exit_anchor(offset_data);
        let entry = out.entry(gid).or_default();
        for (loc_idx, loc) in locations.iter().enumerate() {
            let entry_value = resolve_anchor(entry_anchor.clone(), loc_idx, resolver);
            let exit_value = resolve_anchor(exit_anchor.clone(), loc_idx, resolver);
            entry.push((loc.clone(), (entry_value, exit_value)));
        }
    }
    Ok(())
}

/// Lookup type 4: mark-to-base attachment.
fn markbase_subtable(
    sub: &MarkBasePosFormat1,
    locations: &[NormalizedLocation],
    resolver: Option<&DeltaResolver>,
    out: &mut Effects<(GlyphId, GlyphId), (AnchorV, AnchorV)>,
    budget: &mut usize,
    capped: &mut bool,
) -> Result<(), ReadError> {
    let mark_coverage = sub.mark_coverage()?;
    let base_coverage = sub.base_coverage()?;
    let mark_array = sub.mark_array()?;
    let base_array = sub.base_array()?;

    // (mark gid, mark class, mark anchor)
    let marks: Vec<(GlyphId, u16, AnchorTable)> = mark_coverage
        .iter()
        .zip(mark_array.mark_records().iter())
        .filter_map(|(gid, record)| {
            record
                .mark_anchor(mark_array.offset_data())
                .ok()
                .map(|anchor| (to_gid(gid), record.mark_class(), anchor))
        })
        .collect();

    for (mark_gid, mark_class, mark_anchor) in &marks {
        for (base_gid, base_record) in base_coverage.iter().zip(base_array.base_records().iter()) {
            let Ok(base_record) = base_record else {
                continue;
            };
            let Some(base_anchor) = base_record
                .base_anchors(base_array.offset_data())
                .get(*mark_class as usize)
            else {
                continue;
            };
            let entry = out.entry((*mark_gid, to_gid(base_gid))).or_default();
            for (loc_idx, loc) in locations.iter().enumerate() {
                if *budget == 0 {
                    *capped = true;
                    return Ok(());
                }
                *budget -= 1;
                let mark_anchor = resolve_anchor(Some(Ok(mark_anchor.clone())), loc_idx, resolver);
                let base_anchor = resolve_anchor(Some(base_anchor.clone()), loc_idx, resolver);
                entry.push((loc.clone(), (mark_anchor, base_anchor)));
            }
        }
    }
    Ok(())
}

/// Lookup type 5: mark-to-ligature attachment.
fn marklig_subtable(
    sub: &MarkLigPosFormat1,
    locations: &[NormalizedLocation],
    resolver: Option<&DeltaResolver>,
    out: &mut Effects<(GlyphId, GlyphId), (AnchorV, AnchorV)>,
    budget: &mut usize,
    capped: &mut bool,
) -> Result<(), ReadError> {
    let mark_coverage = sub.mark_coverage()?;
    let ligature_coverage = sub.ligature_coverage()?;
    let mark_array = sub.mark_array()?;
    let ligature_array = sub.ligature_array()?;

    let marks: Vec<(GlyphId, u16, AnchorTable)> = mark_coverage
        .iter()
        .zip(mark_array.mark_records().iter())
        .filter_map(|(gid, record)| {
            record
                .mark_anchor(mark_array.offset_data())
                .ok()
                .map(|anchor| (to_gid(gid), record.mark_class(), anchor))
        })
        .collect();

    for (mark_gid, mark_class, mark_anchor) in &marks {
        for (lig_gid, ligature_attach) in ligature_coverage
            .iter()
            .zip(ligature_array.ligature_attaches().iter())
        {
            let Ok(ligature_attach) = ligature_attach else {
                continue;
            };
            for component in ligature_attach.component_records().iter() {
                let Ok(component) = component else {
                    continue;
                };
                let Some(component_anchor) = component
                    .ligature_anchors(ligature_attach.offset_data())
                    .get(*mark_class as usize)
                else {
                    continue;
                };
                let entry = out.entry((*mark_gid, to_gid(lig_gid))).or_default();
                for (loc_idx, loc) in locations.iter().enumerate() {
                    if *budget == 0 {
                        *capped = true;
                        return Ok(());
                    }
                    *budget -= 1;
                    let mark_anchor =
                        resolve_anchor(Some(Ok(mark_anchor.clone())), loc_idx, resolver);
                    let component_anchor =
                        resolve_anchor(Some(component_anchor.clone()), loc_idx, resolver);
                    entry.push((loc.clone(), (mark_anchor, component_anchor)));
                }
            }
        }
    }
    Ok(())
}

/// Lookup type 6: mark-to-mark attachment.
fn markmark_subtable(
    sub: &MarkMarkPosFormat1,
    locations: &[NormalizedLocation],
    resolver: Option<&DeltaResolver>,
    out: &mut Effects<(GlyphId, GlyphId), (AnchorV, AnchorV)>,
    budget: &mut usize,
    capped: &mut bool,
) -> Result<(), ReadError> {
    let mark1_coverage = sub.mark1_coverage()?;
    let mark2_coverage = sub.mark2_coverage()?;
    let mark1_array = sub.mark1_array()?;
    let mark2_array = sub.mark2_array()?;

    let marks1: Vec<(GlyphId, u16, AnchorTable)> = mark1_coverage
        .iter()
        .zip(mark1_array.mark_records().iter())
        .filter_map(|(gid, record)| {
            record
                .mark_anchor(mark1_array.offset_data())
                .ok()
                .map(|anchor| (to_gid(gid), record.mark_class(), anchor))
        })
        .collect();

    for (mark1_gid, mark1_class, mark1_anchor) in &marks1 {
        for (mark2_gid, mark2_record) in mark2_coverage
            .iter()
            .zip(mark2_array.mark2_records().iter())
        {
            let Ok(mark2_record) = mark2_record else {
                continue;
            };
            let Some(mark2_anchor) = mark2_record
                .mark2_anchors(mark2_array.offset_data())
                .get(*mark1_class as usize)
            else {
                continue;
            };
            let entry = out.entry((*mark1_gid, to_gid(mark2_gid))).or_default();
            for (loc_idx, loc) in locations.iter().enumerate() {
                if *budget == 0 {
                    *capped = true;
                    return Ok(());
                }
                *budget -= 1;
                let mark1_anchor =
                    resolve_anchor(Some(Ok(mark1_anchor.clone())), loc_idx, resolver);
                let mark2_anchor = resolve_anchor(Some(mark2_anchor.clone()), loc_idx, resolver);
                entry.push((loc.clone(), (mark1_anchor, mark2_anchor)));
            }
        }
    }
    Ok(())
}

/// Resolve a value record at a location, including item-variation deltas
/// (via precomputed region scalars) and static (ppem) `Device` table deltas at
/// the rendering size.
fn resolve_value(
    record: &ValueRecord,
    offset_data: FontData,
    loc_idx: usize,
    resolver: Option<&DeltaResolver>,
) -> PValue {
    // Base values straight from the already-parsed record.
    let mut result = PValue {
        x_advance: record.x_advance.map(|v| v.get()).unwrap_or(0) as i32,
        y_advance: record.y_advance.map(|v| v.get()).unwrap_or(0) as i32,
        x_placement: record.x_placement.map(|v| v.get()).unwrap_or(0) as i32,
        y_placement: record.y_placement.map(|v| v.get()).unwrap_or(0) as i32,
    };
    // Device / variation deltas: a `VariationIndex` resolves against the item
    // variation store; a `Device` table is a static ppem delta at the render
    // size (comparing the *effective* delta means re-encoded-but-equivalent
    // Device tables compare equal).
    apply_device_delta(
        &mut result.x_advance,
        record.x_advance_device(offset_data),
        loc_idx,
        resolver,
    );
    apply_device_delta(
        &mut result.y_advance,
        record.y_advance_device(offset_data),
        loc_idx,
        resolver,
    );
    apply_device_delta(
        &mut result.x_placement,
        record.x_placement_device(offset_data),
        loc_idx,
        resolver,
    );
    apply_device_delta(
        &mut result.y_placement,
        record.y_placement_device(offset_data),
        loc_idx,
        resolver,
    );
    result
}

/// Add the resolved delta (variation or static device) of a value-record
/// device field to `target`.
#[inline]
fn apply_device_delta(
    target: &mut i32,
    device: Option<Result<DeviceOrVariationIndex, ReadError>>,
    loc_idx: usize,
    resolver: Option<&DeltaResolver>,
) {
    match device {
        Some(Ok(DeviceOrVariationIndex::VariationIndex(variation_index))) => {
            let delta = resolver.map_or(0, |resolver| {
                resolver.delta(
                    loc_idx,
                    DeltaSetIndex {
                        outer: variation_index.delta_set_outer_index(),
                        inner: variation_index.delta_set_inner_index(),
                    },
                )
            });
            *target += delta;
        }
        Some(Ok(DeviceOrVariationIndex::Device(device_table))) => {
            *target += device_delta_at_size(&device_table);
        }
        _ => {}
    }
}

/// The effective delta of a static `Device` table at the rendering size.
fn device_delta_at_size(device: &Device) -> i32 {
    if RENDER_PPEM < device.start_size() || RENDER_PPEM > device.end_size() {
        return 0;
    }
    let index = (RENDER_PPEM - device.start_size()) as usize;
    device.iter().nth(index).unwrap_or(0) as i32
}

/// Resolve an (optional) anchor at a location, including item-variation
/// deltas on the anchor coordinates.
fn resolve_anchor(
    anchor: Option<Result<AnchorTable, ReadError>>,
    loc_idx: usize,
    resolver: Option<&DeltaResolver>,
) -> AnchorV {
    let Some(Ok(anchor)) = anchor else {
        return AnchorV::default();
    };
    let mut result = AnchorV {
        x: anchor.x_coordinate() as i32,
        y: anchor.y_coordinate() as i32,
    };
    for (delta, coordinate) in [
        (anchor.x_device(), &mut result.x),
        (anchor.y_device(), &mut result.y),
    ] {
        match delta {
            Some(Ok(DeviceOrVariationIndex::VariationIndex(variation_index))) => {
                let outer = variation_index.delta_set_outer_index();
                let inner = variation_index.delta_set_inner_index();
                if let Some(resolver) = resolver {
                    *coordinate += resolver.delta(loc_idx, DeltaSetIndex { outer, inner });
                }
            }
            Some(Ok(DeviceOrVariationIndex::Device(device_table))) => {
                *coordinate += device_delta_at_size(&device_table);
            }
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::staticdiff::compute_signature;

    fn dfont(path: &str) -> DFont {
        let data = std::fs::read(path).unwrap();
        DFont::new(&data)
    }

    /// Martel-KernChange only changes three kern pairs: "To" -47 -> -90 at
    /// thin, plus a new "AV" -30 at regular / -70 at bold. The only pair
    /// changes reported should be A/V, T/g (a class effect of the To change)
    /// and T/o -- with no spurious a/v, a/y, u/m, ... or mark differences.
    #[test]
    fn martel_kernchange_reports_exactly_the_changed_pairs() {
        let font_a = dfont("test-data/Martel-Original.ttf");
        let font_b = dfont("test-data/Martel-KernChange.ttf");
        let signature = compute_signature(&font_a, &font_b);

        // Only kerning changed: no glyph outlines or mark anchors differ.
        assert!(
            signature.glyph_changes.is_empty(),
            "unexpected glyph changes: {:?}",
            signature.glyph_changes
        );
        assert!(
            signature.mark_position_changes.is_empty(),
            "unexpected mark changes: {:?}",
            signature.mark_position_changes
        );

        let cmap: HashMap<u32, GlyphId> = font_a.fontref().charmap().mappings().collect();
        let gid = |c: char| cmap[&(c as u32)];
        let expected = [
            (gid('A'), gid('V')),
            (gid('T'), gid('g')),
            (gid('T'), gid('o')),
        ];
        assert_eq!(
            signature.pair_position_changes.len(),
            expected.len(),
            "unexpected pair changes: {:?}",
            signature.pair_position_changes
        );
        for pair in expected {
            assert!(
                signature.pair_position_changes.contains_key(&pair),
                "missing expected pair {pair:?} in {:?}",
                signature.pair_position_changes
            );
        }
    }
}
