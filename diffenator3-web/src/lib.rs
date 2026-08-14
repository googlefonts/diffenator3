use diffenator3_lib::{
    dfont::{parse_location, shared_axes, DFont},
    render::{encodedglyphs, encodedglyphs::CmapDiff, test_font_words},
    staticdiff::compute_signature,
    structs::{Difference, GlyphDiff},
    summary::summarize,
    WordList,
};
use fontdrasil::coords::NormalizedLocation;
use serde_json::json;
use ttj::{font_to_json as underlying_font_to_json, kern_diff, table_diff};
use wasm_bindgen::JsValue;

use wasm_bindgen::prelude::*;
extern crate console_error_panic_hook;
use std::{
    collections::{BTreeMap, HashMap},
    panic,
};
use web_sys::console;

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
    let f_a = DFont::new(font_a);
    let f_b = DFont::new(font_b);

    // Specific-location diff: no difference signature is computed (it can be
    // expensive for large fonts); every common glyph is compared at the
    // requested location.
    let location = parse_location_opt(location);
    let val = json!({
        "modified_glyphs": encodedglyphs::modified_encoded_glyphs(
            &f_a,
            &f_b,
            location.as_ref(),
            None,
        ).unwrap_or_default()
    });
    f.call1(
        &JsValue::NULL,
        &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string())),
    )
    .unwrap();
}

#[wasm_bindgen]
pub fn cmap_diff(font_a: &[u8], font_b: &[u8], f: &js_sys::Function) {
    let f_a = DFont::new(font_a);
    let f_b = DFont::new(font_b);
    let val = json!({
        "cmap_diff": CmapDiff::new(&f_a, &f_b)
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
    let f_a = DFont::new(font_a);
    let f_b = DFont::new(font_b);

    let custom_word_diff = if !custom_words.is_empty() {
        vec![WordList::define("Custom words".to_string(), custom_words)]
    } else {
        vec![]
    };

    // Specific-location diff: no difference signature is computed (it can be
    // expensive for large fonts); every word is rendered at the requested
    // location.
    let location = parse_location_opt(location);
    let val = json!({
        "words": test_font_words(&f_a, &f_b, None, &custom_word_diff, location.as_ref())
    });
    f.call1(
        &JsValue::NULL,
        &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string())),
    )
    .unwrap();
}

/// Helper: parse a location string into an optional `UserLocation`, treating
/// an empty or unparseable string as the default location.
fn parse_location_opt(location: &str) -> Option<fontdrasil::coords::UserLocation> {
    if location.is_empty() {
        return None;
    }
    parse_location(location).ok()
}

/// Auto mode: compute the static difference signature once, then render the
/// glyph/word diffs across every location the signature says has changed, and
/// return a per-location report. This lists only the locations with actual
/// changes (instead of the font's static instances), which may include
/// non-instance locations where differences occur.
#[wasm_bindgen]
pub fn diff_all(font_a: &[u8], font_b: &[u8], custom_words: Vec<String>, f: &js_sys::Function) {
    let f_a = DFont::new(font_a);
    let f_b = DFont::new(font_b);
    console_error_panic_hook::set_once();

    let custom_word_diff = if !custom_words.is_empty() {
        vec![WordList::define("Custom words".to_string(), custom_words)]
    } else {
        vec![]
    };

    // Push a progressive payload to the JS callback: the interesting locations
    // first (so the location nav can be built immediately), then the glyph
    // diffs, then the word diffs -- each as soon as it is ready, so the page
    // can render incrementally instead of waiting for the whole auto diff.
    let emit = |payload: &serde_json::Value| {
        let _ = f.call1(
            &JsValue::NULL,
            &JsValue::from_str(
                &serde_json::to_string(payload)
                    .unwrap_or_else(|_| "{\"kind\":\"error\"}".to_string()),
            ),
        );
    };

    // Static analysis of the two fonts (outlines, advances, GPOS positioning).
    let signature = compute_signature(&f_a, &f_b);
    console::log_1(&"We've computed the signature".into());

    // Post the human-readable summary back first so the page can show it while
    // the glyph and word diffs are still being computed.
    emit(&json!({
        "kind": "summary",
        "summary": summarize(&signature, &f_a),
    }));

    // The interesting locations, straight from the signature. These seed the
    // location nav; any that end up with no rendered diffs are pruned later.
    let mut locations: Vec<(String, NormalizedLocation)> = signature
        .changed_locations()
        .into_iter()
        .map(|loc| {
            let is_default = loc == NormalizedLocation::default();
            let loc_str = if is_default {
                "default location".to_string()
            } else {
                f_a.location_to_user(&loc)
            };
            (loc_str, loc)
        })
        .collect();
    locations.sort_by(|a, b| a.0.cmp(&b.0));

    emit(&json!({
        "kind": "locations",
        "locations": locations.iter().map(|(loc_str, _)| json!({
            "location": loc_str,
            "coords": parse_coords(loc_str),
        })).collect::<Vec<_>>(),
    }));

    // Render glyph diffs across all the changed locations and send them back
    // grouped by location, as soon as they are available.
    let glyphs = encodedglyphs::modified_encoded_glyphs(&f_a, &f_b, None, Some(&signature))
        .unwrap_or_default();
    console::log_1(&format!("We've found {} modified glyphs", glyphs.len()).into());
    let mut glyphs_by_loc: HashMap<String, Vec<GlyphDiff>> = HashMap::new();
    for glyph in glyphs {
        glyphs_by_loc
            .entry(glyph.location.clone())
            .or_default()
            .push(glyph);
    }
    emit(&json!({
        "kind": "glyphs",
        "locations": locations.iter().filter_map(|(loc_str, _)| {
            let glyphs = glyphs_by_loc.remove(loc_str).unwrap_or_default();
            if glyphs.is_empty() {
                return None;
            }
            Some(json!({
                "location": loc_str,
                "coords": parse_coords(loc_str),
                "glyphs": glyphs,
            }))
        }).collect::<Vec<_>>(),
    }));

    // Render word diffs and send them back grouped by location.
    let words = test_font_words(&f_a, &f_b, Some(&signature), &custom_word_diff, None);
    console::log_1(&format!("We've found {} modified wordlists", words.len()).into());
    let mut words_by_loc: HashMap<String, BTreeMap<String, Vec<Difference>>> = HashMap::new();
    for (wordlist_name, diffs) in words {
        for diff in diffs {
            words_by_loc
                .entry(diff.location.clone())
                .or_default()
                .entry(wordlist_name.clone())
                .or_default()
                .push(diff);
        }
    }
    emit(&json!({
        "kind": "words",
        "locations": locations.iter().filter_map(|(loc_str, _)| {
            let words = words_by_loc.remove(loc_str).unwrap_or_default();
            if words.is_empty() {
                return None;
            }
            Some(json!({
                "location": loc_str,
                "coords": parse_coords(loc_str),
                "words": words,
            }))
        }).collect::<Vec<_>>(),
    }));
}

/// Parse a location string like `"wght=700,wdth=100"` into a coords map.
fn parse_coords(location: &str) -> HashMap<String, f32> {
    location
        .split(',')
        .filter_map(|coord| {
            let mut parts = coord.split('=');
            let axis = parts.next()?;
            let value = parts.next()?;
            let value = value.parse::<f32>().ok()?;
            Some((axis.to_string(), value))
        })
        .collect()
}

#[wasm_bindgen]
pub fn use_auto_by_default(font_a: &[u8], font_b: &[u8], f: &js_sys::Function) {
    let f_a = DFont::new(font_a);
    let f_b = DFont::new(font_b);
    // Let's think about how hard it is going to be to compute the static
    // difference. This is a function of font axes x number of glyphs.
    // Picking a number out of the air, if that is more than 10,000 for either font,
    // we won't use auto, and instead will require the user to pick a location.
    let complexity_a = f_a.glyph_count() as usize * f_a.axis_info().len().min(1);
    let complexity_b = f_b.glyph_count() as usize * f_b.axis_info().len().min(1);
    if complexity_a > 10_000 || complexity_b > 10_000 {
        f.call1(&JsValue::NULL, &JsValue::from_bool(false)).unwrap();
    } else {
        f.call1(&JsValue::NULL, &JsValue::from_bool(true)).unwrap();
    }
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
