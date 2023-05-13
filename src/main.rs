use dfont::DFont;
use render::test_fonts;
use serde_json::json;
use tera::{Context, Tera};
use ttj::table_diff;

mod dfont;
mod render;
mod ttj;

fn main() {
    let font_a = DFont::new("NotoSansArabic-OldRegular.ttf", "old");
    let font_b = DFont::new("NotoSansArabic-NewRegular.ttf", "new");
    let mut tera = Tera::default();
    tera.add_template_file("src/templates/_base.html", Some("_base.html"))
        .unwrap();
    tera.add_template_file(
        "src/templates/Glyph.partial.html",
        Some("Glyph.partial.html"),
    )
    .unwrap();
    tera.add_template_file(
        "src/templates/WordDiff.partial.html",
        Some("WordDiff.partial.html"),
    )
    .unwrap();
    tera.add_template_file(
        "src/templates/GlyphDiff.partial.html",
        Some("GlyphDiff.partial.html"),
    )
    .unwrap();
    tera.add_template_file("src/templates/diffenator.html", Some("main"))
        .unwrap();

    let output = test_fonts(&font_a, &font_b);
    let diff = json!({
        "glyph_diff": output,
        "strings": Vec::<String>::new(),
        "tables": table_diff(&font_a.fontref(), &font_b.fontref())
    });

    let context = Context::from_serialize(json!({
          "diff": diff,
          "font_faces_old": Vec::<String>::new(),
          "font_faces_new": Vec::<String>::new(),
          "font_faces": Vec::<String>::new(),
          "font_styles_old": Vec::<String>::new(),
          "font_styles_new": Vec::<String>::new(),
          "font_styles": Vec::<String>::new(),
          "pt_size": 20.0,

    }))
    .expect("Foo");
    println!("{}", tera.render("main", &context).unwrap());
}
