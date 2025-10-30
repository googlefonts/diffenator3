// Stolen from fontc/otl-normalizer

use std::collections::{BTreeMap, HashMap};

use read_fonts::{
    tables::cmap::{CmapSubtable, EncodingRecord, PlatformId},
    TableProvider,
};
use skrifa::{FontRef, GlyphId, GlyphId16};

/// Given a `char`, returns the postscript name for that `char`s glyph,
/// if one exists in the aglfn.
fn glyph_name_for_char(chr: char) -> Option<String> {
    fontdrasil::agl::agl_name_for_char(chr).map(Into::into)
}

/// A map from glyph IDs to glyph names
#[derive(Debug, Clone)]
pub struct NameMap(BTreeMap<GlyphId, String>);

impl NameMap {
    /// Generate a new NameMap from a font
    pub fn new(font: &FontRef) -> Self {
        let num_glyphs = font.maxp().map(|x| x.num_glyphs()).unwrap_or(0);
        let reverse_cmapping = reverse_cmap(font);
        let post = font.post().ok();
        let name_map = (1..num_glyphs)
            .map(move |gid| {
                let gid = GlyphId16::new(gid);
                // first check post, then do fallback
                if let Some(name) = post
                    .as_ref()
                    .and_then(|post| post.glyph_name(gid).map(|x| x.to_string()))
                {
                    return (gid.into(), name);
                }
                // fallback to unicode or gid
                let name = match reverse_cmapping
                    .as_ref()
                    .and_then(|cmap| cmap.get(&gid))
                    .and_then(|cp| char::from_u32(*cp))
                {
                    Some(codepoint) => match glyph_name_for_char(codepoint) {
                        Some(name) => name.to_string(),
                        // we have a codepoint but it doesn't have a name:
                        None => {
                            let raw = codepoint as u32;
                            if raw <= 0xFFFF {
                                format!("uni{raw:04X}")
                            } else {
                                format!("u{raw:X}")
                            }
                        }
                    },
                    // we have no codepoint, just use glyph ID
                    None => format!("glyph.{:05}", gid.to_u16()),
                };
                (GlyphId::from(gid), name)
            })
            .collect::<BTreeMap<GlyphId, _>>();
        Self(name_map)
    }

    /// Get the name of a glyph
    pub fn get(&self, gid: impl Into<GlyphId>) -> String {
        let gid: GlyphId = gid.into();
        self.0
            .get(&gid)
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("gid{}", gid))
    }

    /// Check if two NameMaps are compatible
    ///
    /// Two NameMaps are compatible if they have the same names for most of the glyphs;
    /// that is, if less than 25% of the names are different.
    pub fn compatible(&self, other: &Self) -> bool {
        let count_glyphname_differences = self
            .0
            .iter()
            .zip(other.0.iter())
            .filter(|(a, b)| a != b)
            .count();
        count_glyphname_differences < self.0.len() / 4
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

fn reverse_cmap(font: &FontRef) -> Option<HashMap<GlyphId16, u32>> {
    // <https://github.com/fonttools/fonttools/blob/6fa1a76e061c2e84243d8cac/Lib/fontTools/ttLib/tables/_c_m_a_p.py#L334>
    fn is_unicode(record: &&EncodingRecord) -> bool {
        record.platform_id() == PlatformId::Unicode
            || record.platform_id() == PlatformId::Unicode
                && [0, 1, 10].contains(&record.encoding_id())
    }

    let cmap = font.cmap().ok()?;
    let offset_data = cmap.offset_data();

    let mut reverse_cmap = HashMap::new();

    let mut add_to_map = |args: (u32, GlyphId16)| {
        // because multiple glyphs may map to the same codepoint,
        // we always use the lowest codepoint to determine the name.
        let val = reverse_cmap.entry(args.1).or_insert(args.0);
        *val = args.0.min(*val);
    };

    for subtable in cmap
        .encoding_records()
        .iter()
        .filter(is_unicode)
        .map(|rec| rec.subtable(offset_data).unwrap())
    {
        match subtable {
            CmapSubtable::Format4(subtable) => subtable
                .iter()
                .map(|(unicode, gid)| (unicode, GlyphId16::try_from(gid).unwrap()))
                .for_each(&mut add_to_map),
            CmapSubtable::Format12(subtable) => subtable
                .iter()
                .map(|(unicode, gid)| (unicode, GlyphId16::try_from(gid).unwrap()))
                .for_each(&mut add_to_map),
            _ => (),
        }
    }

    Some(reverse_cmap)
}
