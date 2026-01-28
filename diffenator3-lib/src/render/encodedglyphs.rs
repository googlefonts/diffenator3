/// Find and represent differences between encoded glyphs in the fonts.
use std::fmt::Display;

use super::{DEFAULT_GLYPHS_FONT_SIZE, DEFAULT_GLYPHS_THRESHOLD};
pub use crate::structs::{CmapDiff, EncodedGlyph};
use crate::{
    dfont::DFont,
    render::{diff_many_words, GlyphDiff},
};
pub use harfrust::Direction;
use static_lang_word_lists::WordList;

impl From<char> for EncodedGlyph {
    fn from(c: char) -> Self {
        EncodedGlyph {
            string: c.to_string(),
            name: unicode_names2::name(c).map(|s| s.to_string()),
        }
    }
}

impl From<u32> for EncodedGlyph {
    fn from(c: u32) -> Self {
        char::from_u32(c).unwrap().into()
    }
}

impl Display for EncodedGlyph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (U+{:04X})",
            self.string,
            self.string.chars().next().unwrap() as u32
        )?;
        if let Some(name) = &self.name {
            write!(f, " {}", name)
        } else {
            Ok(())
        }
    }
}

impl CmapDiff {
    pub fn is_some(&self) -> bool {
        !self.missing.is_empty() || !self.new.is_empty()
    }

    /// Compare the encoded codepoints from two fonts and return the differences
    pub fn new(font_a: &DFont, font_b: &DFont) -> Self {
        let cmap_a = &font_a.codepoints;
        let cmap_b = &font_b.codepoints;
        Self {
            missing: cmap_a.difference(cmap_b).map(|&x| x.into()).collect(),
            new: cmap_b.difference(cmap_a).map(|&x| x.into()).collect(),
        }
    }
}

/// Render the encoded glyphs common to both fonts, and return any differences
pub fn modified_encoded_glyphs(font_a: &DFont, font_b: &DFont) -> Vec<GlyphDiff> {
    let cmap_a = &font_a.codepoints;
    let cmap_b = &font_b.codepoints;
    let same_glyphs = cmap_a.intersection(cmap_b);
    let word_list: Vec<String> = same_glyphs
        .filter_map(|i| char::from_u32(*i))
        .map(|c| c.to_string())
        .collect();
    let wl = WordList::define("Encoded glyphs", word_list);
    let mut result: Vec<GlyphDiff> = diff_many_words(
        font_a,
        font_b,
        DEFAULT_GLYPHS_FONT_SIZE,
        &wl,
        None,
        DEFAULT_GLYPHS_THRESHOLD,
    )
    .into_iter()
    .map(|x| x.into())
    .collect();
    result.sort_by_key(|x| -(x.differing_pixels as i32));
    result
}
