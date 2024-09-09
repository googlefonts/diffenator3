use cfg_if::cfg_if;

pub mod dfont;
pub mod monkeypatching;
pub mod render;
pub mod setting;
pub mod ttj;

cfg_if! {
    if #[cfg(not(target_family = "wasm"))] {
        pub mod reporters;
        pub mod utils;
    }
}

cfg_if! {
    if #[cfg(target_family = "wasm")] {
        use std::collections::HashMap;
        use dfont::DFont;
        use render::{test_font_words, encodedglyphs};
        use serde_json::json;
        use ttj::table_diff;
        use skrifa::MetadataProvider;

        use wasm_bindgen::prelude::*;
        extern crate console_error_panic_hook;
        use std::panic;

        #[wasm_bindgen]
        pub fn debugging() {
            panic::set_hook(Box::new(console_error_panic_hook::hook));
        }

        #[wasm_bindgen]
        pub fn axes(font_a: &[u8], font_b: &[u8]) -> String {
            let f_a = DFont::new(font_a);
            let f_b = DFont::new(font_b);
            let mut axes = f_a.axis_info();
            let b_axes = f_b.axis_info();
            let a_axes_names: Vec<String> = axes.keys().cloned().collect();
            for axis_tag in a_axes_names.iter() {
                if !b_axes.contains_key(axis_tag) {
                    axes.remove(axis_tag);
                }
            }
            for (axis_tag, values) in b_axes.iter() {
                let (our_min, _our_default, our_max) = values;
                axes.entry(axis_tag.clone()).and_modify(
                    |(their_min, _their_default, their_max)| {
                        // This looks upside-down but remember we are
                        // narrowing the axis ranges to the union of the
                        // two fonts.
                        *their_min = their_min.max(*our_min);
                        *their_max = their_max.min(*our_max);
                    }
                );
            }
            let axis_names: Vec<String> = f_a.fontref()
            .axes()
            .iter()
            .map(|axis| {
                    axis.tag().to_string()
            }).collect();
            let instances = f_a
                .fontref()
                .named_instances()
                .iter()
                .map(|ni| {
                    let name = f_a
                        .fontref()
                        .localized_strings(ni.subfamily_name_id())
                        .english_or_first()
                        .map_or_else(|| "Unknown".to_string(), |s| s.chars().collect());
                    let location_map = axis_names.iter().cloned().zip(ni.user_coords()).collect();
                    (name, location_map)
                })
                .collect::<Vec<(String, HashMap<String, f32>)>>();
            return serde_json::to_string(&json!({
                "axes": &axes,
                "instances": instances
            })).unwrap_or("Couldn't do it".to_string());
        }

        #[wasm_bindgen]
        pub fn diff_tables(font_a: &[u8], font_b: &[u8], f: &js_sys::Function) {
            let f_a = DFont::new(font_a);
            let f_b = DFont::new(font_b);

            let val = json!({
                "tables": table_diff(&f_a.fontref(), &f_b.fontref(), 128)
            });
            f.call1(&JsValue::NULL, &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string()))).unwrap();
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
            f.call1(&JsValue::NULL, &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string()))).unwrap();
        }

        #[wasm_bindgen]
        pub fn new_missing_glyphs(font_a: &[u8], font_b: &[u8],f: &js_sys::Function) {
            let f_a = DFont::new(font_a);
            let f_b = DFont::new(font_b);
            let val = json!({
                "new_missing_glyphs": encodedglyphs::new_missing_glyphs(&f_a, &f_b)
            });
            f.call1(&JsValue::NULL, &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string()))).unwrap();
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
            f.call1(&JsValue::NULL, &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string()))).unwrap();
        }

    }
}
