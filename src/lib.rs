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
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

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
        pub fn progressive_diff(font_a: &[u8], font_b: &[u8], f: &js_sys::Function) {
            let f_a = DFont::new(font_a);
            let f_b = DFont::new(font_b);

            let val = json!({
                "tables": table_diff(&f_a.fontref(), &f_b.fontref())
            });
            f.call1(&JsValue::NULL, &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string()))).unwrap();

            let val = json!({
                "glyphs": test_font_glyphs(&f_a, &f_b)
            });
            f.call1(&JsValue::NULL, &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string()))).unwrap();


            let val = json!({
                "words": test_font_words(&f_a, &f_b)
            });
            f.call1(&JsValue::NULL, &JsValue::from_str(&serde_json::to_string(&val).unwrap_or("Couldn't do it".to_string()))).unwrap();
        }

    }
}
