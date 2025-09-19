use serde::Serialize;

/// Represents a difference between two renderings, whether words or glyphs
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct Difference {
    /// The text string which was rendered
    pub word: String,
    /// A string representation of the shaped buffer in the first font
    pub buffer_a: String,
    /// A string representation of the shaped buffer in the second font, if different
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_b: Option<String>,
    /// The number of differing pixels
    pub differing_pixels: usize,
    /// The OpenType features applied to the text
    #[serde(skip_serializing_if = "String::is_empty")]
    pub ot_features: String,
    /// The OpenType language tag applied to the text
    #[serde(skip_serializing_if = "String::is_empty")]
    pub lang: String,
}

#[derive(Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct EncodedGlyph {
    /// The character, as a string
    pub string: String,
    /// Name of the character from the Unicode database, if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
/// Represents changes to the cmap table - added or removed glyphs
#[derive(Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct CmapDiff {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<EncodedGlyph>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub new: Vec<EncodedGlyph>,
}

/// Represents a difference between two encoded glyphs
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct GlyphDiff {
    /// The string representation of the glyph
    pub string: String,
    /// The Unicode name of the glyph
    pub name: String,
    /// The Unicode codepoint of the glyph
    pub unicode: String,
    /// The number of differing pixels
    pub differing_pixels: usize,
}

#[cfg(feature = "typescript")]
pub type Api = (Difference, GlyphDiff, CmapDiff);
