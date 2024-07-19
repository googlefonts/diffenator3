use skrifa::setting::VariationSetting;

use crate::dfont::DFont;

/// A position across both fonts to test; could be an
/// instance, could be a location

#[derive(Debug, Clone)]
pub enum Setting {
    Instance(String),
    Location(Vec<VariationSetting>),
    Default,
}

pub fn parse_location(variations: &str) -> Result<Vec<VariationSetting>, String> {
    let mut settings: Vec<VariationSetting> = vec![];
    for variation in variations.split(',') {
        let mut parts = variation.split('=');
        let axis = parts.next().ok_or("Couldn't parse axis".to_string())?;
        let value = parts.next().ok_or("Couldn't parse value".to_string())?;
        let value = value
            .parse::<f32>()
            .map_err(|_| "Couldn't parse value".to_string())?;
        settings.push((axis, value).into());
    }
    Ok(settings)
}

impl Setting {
    pub fn from_instance(instance: String) -> Self {
        Setting::Instance(instance)
    }
    pub fn from_setting(location: Vec<VariationSetting>) -> Self {
        Setting::Location(location)
    }
    pub fn set_on_fonts(&self, font_a: &mut DFont, font_b: &mut DFont) -> Result<(), String> {
        match self {
            Setting::Instance(inst) => {
                font_a
                    .set_instance(inst)
                    .map_err(|_e| format!("Old font does not contain instance '{}'", inst))?;
                font_b
                    .set_instance(inst)
                    .map_err(|_e| format!("New font does not contain instance '{}'", inst))?;
            }
            Setting::Location(loc) => {
                font_a.location = loc.clone();
                font_a.normalize_location();
                font_b.location = loc.clone();
                font_b.normalize_location();
            }
            Setting::Default => {}
        }
        Ok(())
    }

    pub fn name(&self) -> String {
        match self {
            Setting::Instance(inst) => inst.clone(),
            Setting::Location(loc) => loc
                .iter()
                .map(|vs| format!("{}={}", vs.selector, vs.value))
                .collect::<Vec<String>>()
                .join(","),
            Setting::Default => "Default".to_string(),
        }
    }
}
