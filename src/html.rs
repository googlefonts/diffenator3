use std::{collections::HashMap, path::Path};

use lazy_static::lazy_static;
use serde::Serialize;
use serde_json::json;
use tera::{Context, Tera};

use crate::dfont::DFont;

#[derive(Debug, Serialize)]
pub struct CSSFontFace {
    suffix: String,
    filename: String,
    familyname: String,
    cssfamilyname: String,
    class_name: String,
    font_weight: String,
    font_width: String,
    font_style: String,
}

impl CSSFontFace {
    pub fn new(filename: &Path, suffix: &str, dfont: &DFont) -> Self {
        let familyname = suffix.to_string() + " " + &dfont.family_name();
        let cssfamilyname = familyname.clone();
        let class_name = cssfamilyname.replace(' ', "-");
        let font_weight = "normal".to_string();
        let font_width = "normal".to_string();
        let font_style = "normal".to_string();

        CSSFontFace {
            suffix: suffix.to_string(),
            filename: filename
                .file_name()
                .unwrap()
                .to_str()
                .unwrap_or_default()
                .to_string(),
            familyname,
            cssfamilyname,
            class_name,
            font_weight,
            font_width,
            font_style,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CSSFontStyle {
    suffix: String,
    coords: HashMap<String, f32>,
    familyname: String,
    style_name: String,
    cssfamilyname: String,
    class_name: String,
    font_variation_settings: String,
}

impl CSSFontStyle {
    pub fn new(dfont: &DFont, suffix: Option<&str>) -> Self {
        let familyname = dfont.family_name();
        let stylename = dfont.style_name();
        let coords = dfont
            .location
            .iter()
            .map(|setting| (setting.selector.clone().to_string(), setting.value))
            .collect();
        let font_variation_settings = dfont
            .location
            .iter()
            .map(|setting| format!("'{}' {}", setting.selector, setting.value))
            .collect::<Vec<String>>()
            .join(", ");
        let (cssfamilyname, class_name) = if let Some(suffix) = suffix {
            (
                format!("{} {}", suffix, familyname),
                format!("{}-{}", suffix, stylename).replace(' ', "-"),
            )
        } else {
            (familyname.to_string(), stylename.replace(' ', "-"))
        };

        CSSFontStyle {
            suffix: stylename.to_string(),
            familyname: familyname.to_string(),
            style_name: stylename.to_string(),
            cssfamilyname,
            class_name,
            coords,
            font_variation_settings,
        }
    }
}

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        match Tera::new("templates/*") {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        }
    };
}

pub fn render_output(
    value: &serde_json::Value,
    font_face_old: CSSFontFace,
    font_face_new: CSSFontFace,
    font_style_old: CSSFontStyle,
    font_style_new: CSSFontStyle,
) -> Result<String, tera::Error> {
    TEMPLATES.render(
        "diffenator.html",
        &Context::from_serialize(json!({
            "diff": {
                "tables": value.get("tables").unwrap_or(&json!({})),
                "glyphs": value.get("glyphs").unwrap_or(&serde_json::Value::Null),
                "words": value.get("words").unwrap_or(&serde_json::Value::Null),
            },
            "font_faces_old": [font_face_old],
            "font_faces_new": [font_face_new],
            "font_faces": [],
            "font_styles_old": [font_style_old],
            "font_styles_new": [font_style_new],
            "font_styles": [],
            "pt_size": 40,
            "include_ui": true,
        }))?,
    )
}
