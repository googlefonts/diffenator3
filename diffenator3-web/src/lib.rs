use diffenator3_lib::{
    dfont::{shared_axes, DFont},
    render::{encodedglyphs, encodedglyphs::CmapDiff, test_font_words},
    WordList,
};
use serde_json::json;
use ttj::{font_to_json as underlying_font_to_json, kern_diff, table_diff};
use wasm_bindgen::JsValue;

use wasm_bindgen::prelude::*;
extern crate console_error_panic_hook;
use std::{collections::BTreeMap, panic};

use shaperglot::{Checker, Languages, SupportLevel};

fn support_label(level: &SupportLevel) -> &'static str {
    match level {
        SupportLevel::Complete => "Complete",
        SupportLevel::Supported => "Supported",
        SupportLevel::Incomplete => "Incomplete",
        SupportLevel::Unsupported => "Unsupported",
        SupportLevel::None => "None",
        SupportLevel::Indeterminate => "Indeterminate",
    }
}

pub fn lang_diff(font_a: &DFont, font_b: &DFont) -> serde_json::Value {
    let checker_a = Checker::new(&font_a.backing).expect("Failed to load font");
    let checker_b = Checker::new(&font_b.backing).expect("Failed to load font");
    let languages = Languages::new();
    let mut supported: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for language in languages.iter() {
        let results_a = checker_a.check(language);
        let results_b = checker_b.check(language);
        supported.insert(
            language.name().to_string(),
            json!({
                "level_a": support_label(&results_a.support_level()).to_string(),
                "score_a":  results_a.score(),
                "fixes_a": results_a.fixes_required(),
                "level_b": support_label(&results_b.support_level()).to_string(),
                "score_b":  results_b.score(),
                "fixes_b": results_b.fixes_required(),
            }),
        );
    }
    serde_json::to_value(&supported).expect("Failed to serialize language support")
}

#[wasm_bindgen]
pub fn debugging() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
}

#[wasm_bindgen]
pub fn axes(font_a: &[u8], font_b: &[u8]) -> String {
    let (axes, instances) = shared_axes(&DFont::new(font_a), &DFont::new(font_b));
    serde_json::to_string(&json!({
        "axes": axes,
        "instances": instances
    }))
    .unwrap_or("Couldn't do it".to_string())
}

#[wasm_bindgen]
pub fn diff_tables(font_a: &[u8], font_b: &[u8], f: &js_sys::Function) {
    let f_a = DFont::new(font_a);
    let f_b = DFont::new(font_b);

    let val = json!({
        "tables": table_diff(&f_a.fontref(), &f_b.fontref(), 128, true)
    });
    f.call1(
        &JsValue::NULL,
        &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string())),
    )
    .unwrap();
}

#[wasm_bindgen]
pub fn diff_kerns(font_a: &[u8], font_b: &[u8], f: &js_sys::Function) {
    let f_a = DFont::new(font_a);
    let f_b = DFont::new(font_b);

    let val = json!({
        "kerns": kern_diff(&f_a.fontref(), &f_b.fontref(), 1000, true)
    });
    f.call1(
        &JsValue::NULL,
        &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string())),
    )
    .unwrap();
}

#[wasm_bindgen]
pub fn modified_glyphs(font_a: &[u8], font_b: &[u8], location: &str, f: &js_sys::Function) {
    let mut f_a = DFont::new(font_a);
    let mut f_b = DFont::new(font_b);
    let _hack = f_a.set_location(location);
    let _hack = f_b.set_location(location);

    let val = json!({
        "modified_glyphs": encodedglyphs::modified_encoded_glyphs(&f_a, &f_b)
    });
    f.call1(
        &JsValue::NULL,
        &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string())),
    )
    .unwrap();
}

#[wasm_bindgen]
pub fn new_missing_glyphs(font_a: &[u8], font_b: &[u8], f: &js_sys::Function) {
    let f_a = DFont::new(font_a);
    let f_b = DFont::new(font_b);
    let val = json!({
        "new_missing_glyphs": CmapDiff::new(&f_a, &f_b)
    });
    f.call1(
        &JsValue::NULL,
        &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string())),
    )
    .unwrap();
}

#[wasm_bindgen]
pub fn diff_words(
    font_a: &[u8],
    font_b: &[u8],
    custom_words: Vec<String>,
    location: &str,
    f: &js_sys::Function,
) {
    let mut f_a = DFont::new(font_a);
    let mut f_b = DFont::new(font_b);
    let _hack = f_a.set_location(location);
    let _hack = f_b.set_location(location);

    let custom_word_diff = if !custom_words.is_empty() {
        vec![WordList::define("Custom words".to_string(), custom_words)]
    } else {
        vec![]
    };

    let val = json!({
        "words": test_font_words(&f_a, &f_b, &custom_word_diff)
    });
    f.call1(
        &JsValue::NULL,
        &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string())),
    )
    .unwrap();
}

#[wasm_bindgen]
pub fn diff_languages(font_a: &[u8], font_b: &[u8], f: &js_sys::Function) {
    let f_a = DFont::new(font_a);
    let f_b = DFont::new(font_b);

    let val = json!({
        "languages": lang_diff(&f_a, &f_b)
    });
    f.call1(
        &JsValue::NULL,
        &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string())),
    )
    .unwrap();
}

#[wasm_bindgen]
pub fn font_to_json(font_a: &[u8]) -> JsValue {
    let f_a = DFont::new(font_a);
    let val = underlying_font_to_json(&f_a.fontref(), None);
    serde_wasm_bindgen::to_value(&val).unwrap_or_else(|e| {
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&obj, &"error".into(), &e.to_string().into());
        obj.into()
    })
}
