use cfg_if::cfg_if;

pub mod dfont;
pub mod render;
pub mod ttj;

cfg_if! {
    if #[cfg(target_family = "wasm")] {
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

        use dfont::DFont;
        use render::test_fonts;
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
                "glyphs": test_fonts(&f_a, &f_b),
            });
            serde_json::to_string(&val)
                .unwrap_or("Couldn't do it".to_string())
        }

        #[wasm_bindgen]
        pub fn supported_scripts(font_a: &[u8]) -> String {
            let scripts: Vec<String> = DFont::new(font_a).supported_scripts().into_iter().collect();
            serde_json::to_string(&scripts)
                .unwrap_or("Couldn't do it".to_string())
        }

        #[wasm_bindgen]
        pub fn word_diff(font_a: &[u8], font_b: &[u8], wordlist: String) -> String {
            let f_a = DFont::new(font_a);
            let f_b = DFont::new(font_b);
            let lines: Vec<String> = wordlist.lines().map(|x| x.to_string()).collect();
            let diff = render::diff_many_words(&f_a, &f_b, 40.0, lines, 0.2);
            serde_json::to_string(&json!(diff))
                .unwrap_or("Couldn't do it".to_string())
        }

    }
}
