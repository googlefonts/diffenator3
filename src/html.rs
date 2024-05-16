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
    pub fn new(filename: &str, suffix: &str, dfont: &DFont) -> Self {
        let familyname = suffix.to_string() + " " + &dfont.family_name();
        let cssfamilyname = familyname.replace(' ', "-");
        let class_name = cssfamilyname.clone();
        let font_weight = "normal".to_string();
        let font_width = "normal".to_string();
        let font_style = "normal".to_string();

        CSSFontFace {
            suffix: suffix.to_string(),
            filename: filename.to_string(),
            familyname,
            cssfamilyname,
            class_name,
            font_weight,
            font_width,
            font_style,
        }
    }
}

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        match Tera::new("templates/*") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        }
    };
}

pub fn render_output(
    value: &serde_json::Value,
    font_face_old: CSSFontFace,
    font_face_new: CSSFontFace,
) -> Result<String, tera::Error> {
    TEMPLATES.render(
        "diffenator.html",
        &Context::from_serialize(json!({
            "diff": value,
            "font_faces_old": [font_face_old],
            "font_faces_new": [font_face_new],
            "font_faces": [],
            "font_styles_old": [],
            "font_styles_new": [],
            "font_styles": [],
            "pt_size": 20,
        }))?,
    )
}
