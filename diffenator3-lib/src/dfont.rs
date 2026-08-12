use fontdrasil::coords::{
    ConvertSpace, CoordConverter, DesignCoord, Location, NormalizedCoord, NormalizedLocation,
    NormalizedSpace, UserCoord,
};
use read_fonts::{types::NameId, FontRef, ReadError, TableProvider};
use skrifa::{GlyphId, MetadataProvider};
use std::collections::{HashMap, HashSet};
use ucd::Codepoint;

use crate::render::shaper::DrawBuffer;

fn fontdrasil_axes(font: &FontRef) -> Result<Option<fontdrasil::types::Axes>, ReadError> {
    let per_axis_maps = if let Ok(segments) = font.avar().map(|x| x.axis_segment_maps()) {
        segments.iter().collect::<Result<Vec<_>, _>>()?
    } else {
        vec![]
    };
    Ok(Some(
        font.axes()
            .iter()
            .enumerate()
            .map(|(ix, axis)| {
                let min = UserCoord::new(axis.min_value() as f64);
                let default = UserCoord::new(axis.default_value() as f64);
                let max = UserCoord::new(axis.max_value() as f64);
                #[allow(clippy::unwrap_used)]
                let mut fd_axis = fontdrasil::types::Axis {
                    converter: CoordConverter::default_normalization(min, default, max),
                    hidden: axis.is_hidden(),
                    tag: axis.tag(),
                    name: axis.tag().to_string(),
                    min,
                    default,
                    max,
                    localized_names: HashMap::new(), // Let's not
                };
                if let Some(map) = per_axis_maps.get(ix) {
                    let desired_mapping: Vec<(
                        fontdrasil::coords::Coord<fontdrasil::coords::UserSpace>,
                        fontdrasil::coords::Coord<fontdrasil::coords::DesignSpace>,
                    )> = map
                        .axis_value_maps
                        .iter()
                        .map(|mapping| {
                            let from = mapping.from_coordinate().to_f32();
                            let to = mapping.to_coordinate().to_f32();
                            // These are both normalized coordinates. Turn the `from` back into
                            // userspace using default normalization
                            let user_from =
                                NormalizedCoord::new(from as f64).to_user(&fd_axis.converter);
                            // Let's pretend design space is just normalized space
                            let design_to = DesignCoord::new(to as f64);
                            (user_from, design_to)
                        })
                        .collect();
                    let default_idx = desired_mapping
                        .iter()
                        .position(|(_, to)| to.to_f64() == 0.0)
                        .unwrap_or(0);
                    fd_axis.converter = CoordConverter::new(desired_mapping, default_idx)
                }
                fd_axis
            })
            .collect(),
    ))
}

/// A representation of everything we need to know about a font for diffenator purposes
#[derive(Debug, Clone)]
pub struct DFont {
    /// The font binary data
    pub backing: Vec<u8>,
    /// The set of encoded codepoints in the font
    pub codepoints: HashSet<u32>,
    /// Cached variation positions
    variation_positions: HashMap<GlyphId, Vec<NormalizedLocation>>,
    /// Fontdrasil axes, if the font has any. This is cached because building it is expensive.
    pub fontdrasil_axes: Option<fontdrasil::types::Axes>,
}

impl DFont {
    /// Create a new DFont from a byte slice
    pub fn new(string: &[u8]) -> Self {
        let backing: Vec<u8> = string.to_vec();

        let mut fnt = DFont {
            backing,
            codepoints: HashSet::new(),
            variation_positions: HashMap::new(),
            fontdrasil_axes: None,
        };
        fnt.fontdrasil_axes = fontdrasil_axes(&fnt.fontref()).unwrap_or_default();
        let cmap = fnt.fontref().charmap();
        fnt.codepoints = cmap.mappings().map(|(cp, _)| cp).collect();
        let max_glyphid = fnt.fontref().maxp().map(|x| x.num_glyphs()).unwrap_or(0);
        fnt.variation_positions = (0..max_glyphid)
            .map(|glyph| {
                let glyphid = GlyphId::new(glyph.into());
                let variations = fnt
                    ._variations_for_glyph_uncached(&glyphid)
                    .unwrap_or_default();
                (glyphid, variations)
            })
            .collect();
        fnt
    }

    pub fn fontref(&self) -> FontRef<'_> {
        FontRef::new(&self.backing).expect("Couldn't parse font")
    }
    pub fn family_name(&self) -> String {
        self.fontref()
            .localized_strings(NameId::FAMILY_NAME)
            .english_or_first()
            .map_or_else(|| "Unknown".to_string(), |s| s.chars().collect())
    }

    pub fn style_name(&self) -> String {
        self.fontref()
            .localized_strings(NameId::SUBFAMILY_NAME)
            .english_or_first()
            .map_or_else(|| "Regular".to_string(), |s| s.chars().collect())
    }

    /// The axes of the font
    ///
    /// Returns a map from axis tag to (min, default, max) values
    pub fn axis_info(&self) -> HashMap<String, (f32, f32, f32)> {
        self.fontref()
            .axes()
            .iter()
            .map(|axis| {
                (
                    axis.tag().to_string(),
                    (axis.min_value(), axis.default_value(), axis.max_value()),
                )
            })
            .collect()
    }

    /// Returns a list of scripts where the font has at least one encoded
    /// character from that script.
    pub fn supported_scripts(&self) -> HashSet<String> {
        let cmap = self.fontref().charmap();
        let mut strings = HashSet::new();
        for (codepoint, _glyphid) in cmap.mappings() {
            if let Some(script) = char::from_u32(codepoint).and_then(|c| c.script()) {
                // Would you believe, no Display, no .to_string(), we just have to grub around with Debug.
                strings.insert(format!("{:?}", script));
            }
        }
        strings
    }

    /// Returns a list of the master locations in the font
    ///
    /// This is derived heuristically from locations of shared tuples in the `gvar` table.
    /// This should work well enough for most "normal" fonts.
    pub fn masters(&self) -> Result<Vec<NormalizedLocation>, ReadError> {
        let gvar = self.fontref().gvar()?;
        let axes = self.fontref().axes();
        let tuples = gvar.shared_tuples()?.tuples();
        let peaks: Vec<NormalizedLocation> = tuples
            .iter()
            .flatten()
            .map(|tuple| {
                let coords = tuple
                    .values()
                    .iter()
                    .map(|x| x.get().to_f32())
                    .collect::<Vec<f32>>();
                let normalized_location: NormalizedLocation = axes
                    .iter()
                    .zip(coords.iter())
                    .map(|(axis, coord)| (axis.tag(), NormalizedCoord::new(*coord as f64)))
                    .collect();
                normalized_location
            })
            .collect();
        Ok(peaks)
    }

    pub fn variations_for_glyph(&self, glyphid: &GlyphId) -> Vec<NormalizedLocation> {
        self.variation_positions
            .get(glyphid)
            .cloned()
            .unwrap_or_default()
    }

    pub fn _variations_for_glyph_uncached(
        &self,
        glyphid: &GlyphId,
    ) -> Result<Vec<NormalizedLocation>, ReadError> {
        let gvar = self.fontref().gvar()?;
        let axes = self.fontref().axes();
        Ok(
            if let Some(variations) = gvar.glyph_variation_data(*glyphid)? {
                let variations: Vec<NormalizedLocation> = variations
                    .tuples()
                    .map(|tuple| {
                        let coords = tuple
                            .peak()
                            .values()
                            .iter()
                            .map(|x| x.get().to_f32())
                            .collect::<Vec<f32>>();
                        axes.iter()
                            .zip(coords.iter())
                            .map(|(axis, coord)| (axis.tag(), NormalizedCoord::new(*coord as f64)))
                            .collect()
                    })
                    .collect();
                variations
            } else {
                Vec::new()
            },
        )
    }

    pub fn variations_for_buffer(&self, buffer: &DrawBuffer) -> HashSet<NormalizedLocation> {
        buffer
            .iter()
            .fold(HashSet::new(), |mut acc, positioned_glyph| {
                // Borrow the cached per-glyph list directly instead of going
                // through variations_for_glyph (which clones a Vec per glyph);
                // this runs per (word, buffer) in the hot loops.
                if let Some(variations) = self.variation_positions.get(&positioned_glyph.glyph_id) {
                    acc.extend(variations.iter().cloned());
                }
                acc
            })
    }

    pub fn location_to_coords<T>(&self, location: &Location<T>) -> Vec<NormalizedCoord>
    where
        T: ConvertSpace<NormalizedSpace>,
    {
        let Some(axes) = self.fontdrasil_axes.as_ref() else {
            return vec![];
        };
        let normalized_location = location.to_normalized(axes);
        self.normalized_location_to_coords(&normalized_location)
    }

    pub fn normalized_location_to_coords(
        &self,
        location: &NormalizedLocation,
    ) -> Vec<NormalizedCoord> {
        let axes = self.fontref().axes();
        axes.iter()
            .map(|axis| {
                location
                    .get(axis.tag())
                    .unwrap_or_else(|| NormalizedCoord::new(0.0))
            })
            .collect()
    }

    pub fn location_to_user(&self, location: &NormalizedLocation) -> String {
        if let Some(fontdrasil_axes) = &self.fontdrasil_axes {
            // A location may contain axes which this font doesn't have (e.g. a
            // location built from the union of both fonts' variation peaks).
            // Convert only the axes this font knows about, otherwise fontdrasil
            // panics on the unknown axes.
            let mut location = location.clone();
            let tags: Vec<_> = self
                .fontref()
                .axes()
                .iter()
                .map(|axis| axis.tag())
                .collect();
            location.fit_to_axes(&tags);
            let user_location = location.to_user(fontdrasil_axes);
            let mut loc_str: Vec<String> = user_location
                .iter()
                .map(|(tag, coord)| format!("{}={}", tag, coord.to_f64()))
                .collect();
            loc_str.sort();
            loc_str.join(",")
        } else {
            "".to_string()
        }
    }

    /// The GDEF glyph class of a glyph: 0=unassigned, 1=base, 2=ligature,
    /// 3=mark, 4=component. Returns 0 if the font has no GDEF glyph class
    /// definition.
    pub fn glyph_class(&self, gid: GlyphId) -> u16 {
        self.fontref()
            .gdef()
            .ok()
            .and_then(|gdef| gdef.glyph_class_def())
            .and_then(|res| res.ok())
            .map(|class_def| class_def.get(gid))
            .unwrap_or(0)
    }

    /// Whether the glyph is a mark glyph (GDEF glyph class 3).
    pub fn glyph_is_mark(&self, gid: GlyphId) -> bool {
        self.glyph_class(gid) == 3
    }

    /// The set of glyph ids whose GDEF glyph class is 3 (mark). Build once
    /// and reuse: per-glyph [`Self::glyph_class`] lookups re-parse GDEF and
    /// are too slow to call inside a per-word hot loop.
    pub fn mark_glyphs(&self) -> HashSet<GlyphId> {
        let num = self.fontref().maxp().map(|x| x.num_glyphs()).unwrap_or(0);
        (0..num)
            .map(|gid| GlyphId::new(gid as u32))
            .filter(|gid| self.glyph_class(*gid) == 3)
            .collect()
    }
}

type InstancePositions = Vec<(String, HashMap<String, f32>)>;
type AxisDescription = HashMap<String, (f32, f32, f32)>;

/// Compare two fonts and return the axes and instances they have in common
pub fn shared_axes(f_a: &DFont, f_b: &DFont) -> (AxisDescription, InstancePositions) {
    let mut axes = f_a.axis_info();
    let b_axes = f_b.axis_info();
    let a_axes_names: Vec<String> = axes.keys().cloned().collect();
    for axis_tag in a_axes_names.iter() {
        if !b_axes.contains_key(axis_tag) {
            axes.remove(axis_tag);
        }
    }
    for (axis_tag, values) in b_axes.iter() {
        let (our_min, _our_default, our_max) = values;
        axes.entry(axis_tag.clone())
            .and_modify(|(their_min, _their_default, their_max)| {
                // This looks upsidedown but remember we are
                // narrowing the axis ranges to the union of the
                // two fonts.
                *their_min = their_min.max(*our_min);
                *their_max = their_max.min(*our_max);
            });
    }
    let axis_names: Vec<String> = f_a
        .fontref()
        .axes()
        .iter()
        .map(|axis| axis.tag().to_string())
        .collect();
    let instances = f_a
        .fontref()
        .named_instances()
        .iter()
        .map(|ni| {
            let name = f_a
                .fontref()
                .localized_strings(ni.subfamily_name_id())
                .english_or_first()
                .map_or_else(|| "Unknown".to_string(), |s| s.chars().collect());
            let location_map = axis_names.iter().cloned().zip(ni.user_coords()).collect();
            (name, location_map)
        })
        .collect::<Vec<(String, HashMap<String, f32>)>>();
    (axes, instances)
}
