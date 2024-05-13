use cfg_if::cfg_if;

pub mod dfont;
pub mod render;
pub mod ttj;

cfg_if! {
    if #[cfg(not(target_family = "wasm"))] {
        pub mod html;
    }
}

cfg_if! {
    if #[cfg(target_family = "wasm")] {
        use dfont::DFont;
        use render::{test_font_glyphs, test_font_words};
        use serde_json::json;
        use ttj::table_diff;

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
            for (axis_tag, values) in f_b.axis_info().iter() {
                let (our_min, _our_default, our_max) = values;
                axes.entry(axis_tag.clone()).and_modify(
                    |(their_min, _their_default, their_max)| {
                        // This looks upside-down but remember we are
                        // narrowing the axis ranges to the union of the
                        // two fonts.
                        *their_min = their_min.max(*our_min);
                        *their_max = their_max.min(*our_max);
                    },
                );
            }
            return serde_json::to_string(&json!({
                "axes": &axes
            })).unwrap_or("Couldn't do it".to_string());
        }

        #[wasm_bindgen]
        pub fn diff(font_a: &[u8], font_b: &[u8]) -> String {
            let f_a = DFont::new(font_a);
            let f_b = DFont::new(font_b);
            let val = json!({
                "tables": table_diff(&f_a.fontref(), &f_b.fontref()),
                "glyphs": test_font_glyphs(&f_a, &f_b),
                "words": test_font_words(&f_a, &f_b),
            });
            serde_json::to_string(&val)
                .unwrap_or("Couldn't do it".to_string())
        }

        #[wasm_bindgen]
        pub fn diff_tables(font_a: &[u8], font_b: &[u8], f: &js_sys::Function) {
            let f_a = DFont::new(font_a);
            let f_b = DFont::new(font_b);

            let val = json!({
                "tables": table_diff(&f_a.fontref(), &f_b.fontref())
            });
            f.call1(&JsValue::NULL, &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string()))).unwrap();
        }

        #[wasm_bindgen]
        pub fn diff_glyphs(font_a: &[u8], font_b: &[u8], location: &str, f: &js_sys::Function) {
            let mut f_a = DFont::new(font_a);
            let mut f_b = DFont::new(font_b);
            f_a.set_location(location);
            f_b.set_location(location);

            let val = json!({
                "glyphs": test_font_glyphs(&f_a, &f_b)
            });
            f.call1(&JsValue::NULL, &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string()))).unwrap();
        }

        #[wasm_bindgen]
        pub fn diff_words(font_a: &[u8], font_b: &[u8], location: &str, f: &js_sys::Function) {
            let mut f_a = DFont::new(font_a);
            let mut f_b = DFont::new(font_b);
            f_a.set_location(location);
            f_b.set_location(location);


            let val = json!({
                "words": test_font_words(&f_a, &f_b)
            });
            f.call1(&JsValue::NULL, &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string()))).unwrap();
        }

    }
}
