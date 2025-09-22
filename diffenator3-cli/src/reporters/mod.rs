pub mod html;
pub mod json;
pub mod text;

use diffenator3_lib::structs::{CmapDiff, Difference, GlyphDiff};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Default)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct LocationResult {
    /// Name of the location in designspace (named instance, or stringified coordinates)
    pub location: String,
    /// Coordinates of the location in designspace
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub coords: HashMap<String, f32>,
    /// An error message, if something went wrong
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Differences between glyphs
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub glyphs: Vec<GlyphDiff>,
    /// Differences between words
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub words: BTreeMap<String, Vec<Difference>>,
}

impl LocationResult {
    pub fn is_some(&self) -> bool {
        self.error.is_some() || !self.glyphs.is_empty() || !self.words.is_empty()
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
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct Report {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tables: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kerns: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmap_diff: Option<CmapDiff>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<LocationResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<BTreeMap<String, crate::languages::LanguageDiff>>,
}

#[cfg(feature = "typescript")]
#[allow(dead_code)]
pub type Api = (LocationResult, Report);
