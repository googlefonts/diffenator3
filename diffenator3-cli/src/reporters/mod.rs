pub mod html;
pub mod json;
pub mod text;

use std::collections::HashMap;

use serde::Serialize;

use diffenator3_lib::render::encodedglyphs::CmapDiff;
use diffenator3_lib::render::GlyphDiff;
use ttj::jsondiff::Substantial;

#[derive(Serialize, Default)]
pub struct LocationResult {
    pub location: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub coords: HashMap<String, f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub glyphs: Vec<GlyphDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<serde_json::Value>,
}

impl LocationResult {
    pub fn is_some(&self) -> bool {
        self.error.is_some()
            || !self.glyphs.is_empty()
            || (self.words.is_some() && self.words.as_ref().unwrap().is_something())
    }

    pub fn from_error(location: String, error: String) -> Self {
        LocationResult {
            location,
            error: Some(error),
            ..Default::default()
        }
    }
}
#[derive(Serialize, Default)]
pub struct Report {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tables: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kerns: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmap_diff: Option<CmapDiff>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<LocationResult>,
}
