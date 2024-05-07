use font_types::NameId;
use read_fonts::{FontRef};
use skrifa::MetadataProvider;
use std::{collections::HashSet};
use ucd::Codepoint;

pub struct DFont {
    pub backing: Vec<u8>,
    variations: String,
}

impl DFont {
    pub fn new(string: &[u8]) -> Self {
        let backing: Vec<u8> = string.to_vec();
        DFont {
            backing,
            variations: "".to_string(),
        }
    }

    pub fn fontref(&self) -> FontRef {
        FontRef::new(&self.backing).expect("Couldn't parse font")
    }
    pub fn family_name(&self) -> String {
        self.fontref()
            .localized_strings(NameId::FAMILY_NAME)
            .english_or_first()
            .map_or_else(|| "Unknown".to_string(), |s| s.chars().collect())
    }

    pub fn is_color(&self) -> bool {
        self.fontref()
            .table_directory
            .table_records()
            .iter()
            .any(|tr| {
                let tag = tr.tag();
                tag == "SVG " || tag == "COLR" || tag == "CBDT"
            })
    }

    pub fn is_variable(&self) -> bool {
        self.fontref()
            .table_directory
            .table_records()
            .iter()
            .any(|tr| tr.tag() == "fvar")
    }

    pub fn codepoints(&self) -> HashSet<u32> {
        let cmap = self.fontref().charmap();
        let mut points = HashSet::new();
        for (codepoint, _glyphid) in cmap.mappings() {
            points.insert(codepoint);
        }
        points
    }

    pub fn supported_scripts(&self) -> HashSet<String> {
        let cmap = self.fontref().charmap();
        let mut strings = HashSet::new();
        for (codepoint, _glyphid) in cmap.mappings() {
            if let Some(script) = char::from_u32(codepoint).and_then(|c| c.script()) {
                strings.insert(format!("{:?}", script));
            }
        }
        strings
    }
}
