use crate::setting::parse_location;
use font_types::NameId;
use read_fonts::FontRef;
use skrifa::instance::Location;
use skrifa::setting::VariationSetting;
use skrifa::MetadataProvider;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use ucd::Codepoint;

pub struct DFont {
    pub backing: Vec<u8>,
    pub location: Vec<VariationSetting>,
    pub normalized_location: Location,
    pub codepoints: HashSet<u32>,
}

impl DFont {
    pub fn new(string: &[u8]) -> Self {
        let backing: Vec<u8> = string.to_vec();

        let mut fnt = DFont {
            backing,
            codepoints: HashSet::new(),
            normalized_location: Location::default(),
            location: vec![],
        };
        let cmap = fnt.fontref().charmap();
        fnt.codepoints = cmap.mappings().map(|(cp, _)| cp).collect();
        fnt
    }

    /// Must be called after the location is set
    pub fn normalize_location(&mut self) {
        self.normalized_location = self.fontref().axes().location(&self.location);
    }

    pub fn set_location(&mut self, variations: &str) -> Result<(), String> {
        self.location = parse_location(variations)?;
        self.normalize_location();
        Ok(())
    }
    pub fn instances(&self) -> Vec<String> {
        self.fontref()
            .named_instances()
            .iter()
            .flat_map(|ni| {
                self.fontref()
                    .localized_strings(ni.subfamily_name_id())
                    .english_or_first()
            })
            .map(|s| s.to_string())
            .collect()
    }
    pub fn set_instance(&mut self, instance: &str) -> Result<(), String> {
        let instance = self
            .fontref()
            .named_instances()
            .iter()
            .find(|ni| {
                self.fontref()
                    .localized_strings(ni.subfamily_name_id())
                    .any(|s| instance == s.chars().collect::<Cow<str>>())
            })
            .ok_or_else(|| format!("No instance named {}", instance))?;
        let user_coords = instance.user_coords();
        let location = instance.location();
        self.location = self
            .fontref()
            .axes()
            .iter()
            .zip(user_coords)
            .map(|(a, v)| (a.tag(), v).into())
            .collect();
        self.normalized_location = location;
        Ok(())
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

    pub fn style_name(&self) -> String {
        self.fontref()
            .localized_strings(NameId::SUBFAMILY_NAME)
            .english_or_first()
            .map_or_else(|| "Regular".to_string(), |s| s.chars().collect())
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

    pub fn axis_info(&self) -> HashMap<String, (f32, f32, f32)> {
        self.fontref()
            .axes()
            .iter()
            .map(|axis| {
                (
                    axis.tag().to_string(),
                    (axis.min_value(), axis.default_value(), axis.max_value()),
                )
            })
            .collect()
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
