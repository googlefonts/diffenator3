use crate::{
    dfont::DFont,
    render::{diff_many_words, GlyphDiff},
};
use rustybuzz::Direction;
use serde::Serialize;

#[derive(Serialize)]
pub struct EncodedGlyph {
    pub string: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct CmapDiff {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<EncodedGlyph>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub new: Vec<EncodedGlyph>,
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
    let threshold = 0.1;
    let word_list: Vec<String> = same_glyphs
        .map(|i| char::from_u32(*i))
        .filter(|x| x.is_some())
        .map(|c| c.unwrap().to_string())
        .collect();
    let mut result: Vec<GlyphDiff> = diff_many_words(
        font_a,
        font_b,
        40.0,
        word_list,
        threshold,
        Direction::LeftToRight,
        None,
    )
    .into_iter()
    .map(|x| x.into())
    .collect();
    result.sort_by_key(|x| (-x.percent * 10_000.0) as i32);
    result
}
