use dfont::DFont;
use render::test_fonts;
use serde_json::json;
use ttj::table_diff;

mod dfont;
mod render;
mod ttj;
fn main() {
    let font_binary_a = std::fs::read("NotoSansArabic-OldRegular.ttf").expect("Couldn't open file");
    let font_binary_b = std::fs::read("NotoSansArabic-NewRegular.ttf").expect("Couldn't open file");

    let font_a = DFont::new(&font_binary_a);
    let font_b = DFont::new(&font_binary_b);
    let output = test_fonts(&font_a, &font_b);
    let diff = json!({
        "glyph_diff": output,
        "strings": Vec::<String>::new(),
        "tables": table_diff(&font_a.fontref(), &font_b.fontref())
    });
    println!("{}", serde_json::to_string_pretty(&diff).expect("foo"));
    println!("{:?}", font_a.supported_scripts());
}
