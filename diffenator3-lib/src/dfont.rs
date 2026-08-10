use read_fonts::{types::NameId, FontRef, ReadError, TableProvider};
use skrifa::{setting::VariationSetting, MetadataProvider};
use std::collections::{HashMap, HashSet};
use ttj::monkeypatching::DenormalizeLocation;
use ucd::Codepoint;

/// A representation of everything we need to know about a font for diffenator purposes
#[derive(Debug, Clone)]
pub struct DFont {
    /// The font binary data
    pub backing: Vec<u8>,
    /// The set of encoded codepoints in the font
    pub codepoints: HashSet<u32>,
}

impl DFont {
    /// Create a new DFont from a byte slice
    pub fn new(string: &[u8]) -> Self {
        let backing: Vec<u8> = string.to_vec();

        let mut fnt = DFont {
            backing,
            codepoints: HashSet::new(),
        };
        let cmap = fnt.fontref().charmap();
        fnt.codepoints = cmap.mappings().map(|(cp, _)| cp).collect();
        fnt
    }

    pub fn fontref(&self) -> FontRef<'_> {
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

    /// The axes of the font
    ///
    /// Returns a map from axis tag to (min, default, max) values
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

    /// Returns a list of scripts where the font has at least one encoded
    /// character from that script.
    pub fn supported_scripts(&self) -> HashSet<String> {
        let cmap = self.fontref().charmap();
        let mut strings = HashSet::new();
        for (codepoint, _glyphid) in cmap.mappings() {
            if let Some(script) = char::from_u32(codepoint).and_then(|c| c.script()) {
                // Would you believe, no Display, no .to_string(), we just have to grub around with Debug.
                strings.insert(format!("{:?}", script));
            }
        }
        strings
    }

    /// Returns a list of the master locations in the font
    ///
    /// This is derived heuristically from locations of shared tuples in the `gvar` table.
    /// This should work well enough for most "normal" fonts.
    pub fn masters(&self) -> Result<Vec<Vec<VariationSetting>>, ReadError> {
        let gvar = self.fontref().gvar()?;
        let tuples = gvar.shared_tuples()?.tuples();
        let peaks: Vec<Vec<VariationSetting>> = tuples
            .iter()
            .flatten()
            .flat_map(|tuple| {
                let location = tuple
                    .values()
                    .iter()
                    .map(|x| x.get().to_f32())
                    .collect::<Vec<f32>>();
                self.fontref().denormalize_location(&location)
            })
            .collect();
        Ok(peaks)
    }
}
