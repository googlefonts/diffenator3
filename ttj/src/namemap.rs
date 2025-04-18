use read_fonts::TableProvider;
use skrifa::{FontRef, GlyphId, GlyphId16};

fn gid_to_name<'a>(font: &impl TableProvider<'a>, gid: GlyphId) -> String {
    if let Ok(gid16) = TryInto::<GlyphId16>::try_into(gid) {
        if let Ok(Some(name)) = font
            .post()
            .map(|post| post.glyph_name(gid16).map(|x| x.to_string()))
        {
            return name;
        }
    }
    format!("gid{:}", gid)
}

/// A map from glyph IDs to glyph names
#[derive(Debug, Clone)]
pub struct NameMap(Vec<String>);

impl NameMap {
    /// Generate a new NameMap from a font
    pub fn new(font: &FontRef) -> Self {
        let num_glyphs = font.maxp().unwrap().num_glyphs();
        let mut mapping = Vec::with_capacity(num_glyphs as usize);
        for gid in 0..num_glyphs {
            mapping.push(gid_to_name(font, GlyphId::new(gid as u32)));
        }
        Self(mapping)
    }

    /// Get the name of a glyph
    pub fn get(&self, gid: impl Into<GlyphId>) -> String {
        let gid: GlyphId = gid.into();
        self.0
            .get(gid.to_u32() as usize)
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
