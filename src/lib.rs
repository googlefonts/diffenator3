use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_family = "wasm")] {
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

        use dfont::DFont;
        use render::test_fonts;
        use serde_json::json;
        use ttj::table_diff;

        mod dfont;
        mod render;
        mod ttj;

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
                .to_string()
        }
    }
}
