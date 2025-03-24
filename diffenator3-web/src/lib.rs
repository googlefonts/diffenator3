use diffenator3_lib::dfont::{shared_axes, DFont};
use diffenator3_lib::render::{encodedglyphs, encodedglyphs::CmapDiff, test_font_words};
use serde_json::json;
use ttj::font_to_json as underlying_font_to_json;
use ttj::{kern_diff, table_diff};
use wasm_bindgen::JsValue;

use wasm_bindgen::prelude::*;
extern crate console_error_panic_hook;
use std::panic;

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
pub fn diff_words(font_a: &[u8], font_b: &[u8], location: &str, f: &js_sys::Function) {
    let mut f_a = DFont::new(font_a);
    let mut f_b = DFont::new(font_b);
    let _hack = f_a.set_location(location);
    let _hack = f_b.set_location(location);

    let val = json!({
        "words": test_font_words(&f_a, &f_b)
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
