pub mod html;
pub mod json;
pub mod text;

use diffenator3_lib::structs::{CmapDiff, Difference, GlyphDiff};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize, Default)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct Report {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tables: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kerns: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmap_diff: Option<CmapDiff>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<BTreeMap<String, crate::languages::LanguageDiff>>,
    /// Differences between glyphs
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub glyphs: Vec<GlyphDiff>,
    /// Differences between words
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub words: BTreeMap<String, Vec<Difference>>,
}

#[cfg(feature = "typescript")]
#[allow(dead_code)]
pub type Api = (Report);
