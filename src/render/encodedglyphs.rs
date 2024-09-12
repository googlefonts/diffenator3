use std::fmt::Display;

use crate::dfont::DFont;
use crate::render::{diff_many_words, GlyphDiff};
use rustybuzz::Direction;
use serde::Serialize;

use super::{DEFAULT_GLYPHS_FONT_SIZE, DEFAULT_GLYPHS_THRESHOLD};

#[derive(Serialize)]
pub struct EncodedGlyph {
    pub string: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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

#[derive(Serialize)]
pub struct CmapDiff {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<EncodedGlyph>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub new: Vec<EncodedGlyph>,
}

impl CmapDiff {
    pub fn is_some(&self) -> bool {
        !self.missing.is_empty() || !self.new.is_empty()
    }
}

fn chars_to_json_array(chars: impl Iterator<Item = u32>) -> impl Iterator<Item = EncodedGlyph> {
    chars
        .map(char::from_u32)
        .filter(|x| x.is_some())
        .map(|c| EncodedGlyph {
            string: c.unwrap().to_string(),
            name: unicode_names2::name(c.unwrap()).map(|n| n.to_string()),
        })
}

pub fn new_missing_glyphs(font_a: &DFont, font_b: &DFont) -> CmapDiff {
    let cmap_a = &font_a.codepoints;
    let cmap_b = &font_b.codepoints;
    let missing_glyphs = cmap_a.difference(cmap_b).copied();
    let new_glyphs = cmap_b.difference(cmap_a).copied();
    CmapDiff {
        missing: chars_to_json_array(missing_glyphs).collect(),
        new: chars_to_json_array(new_glyphs).collect(),
    }
}

pub fn modified_encoded_glyphs(font_a: &DFont, font_b: &DFont) -> Vec<GlyphDiff> {
    let cmap_a = &font_a.codepoints;
    let cmap_b = &font_b.codepoints;
    let same_glyphs = cmap_a.intersection(cmap_b);
    let word_list: Vec<String> = same_glyphs
        .map(|i| char::from_u32(*i))
        .filter(|x| x.is_some())
        .map(|c| c.unwrap().to_string())
        .collect();
    let mut result: Vec<GlyphDiff> = diff_many_words(
        font_a,
        font_b,
        DEFAULT_GLYPHS_FONT_SIZE,
        word_list,
        DEFAULT_GLYPHS_THRESHOLD,
        Direction::LeftToRight,
        None,
    )
    .into_iter()
    .map(|x| x.into())
    .collect();
    result.sort_by_key(|x| -(x.differing_pixels as i32));
    result
}
