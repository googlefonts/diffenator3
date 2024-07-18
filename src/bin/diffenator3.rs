use clap::{builder::ArgAction, Parser};
use diffenator3::{
    dfont::DFont,
    render::{modified_encoded_glyphs, new_missing_glyphs, test_font_words},
    reporters::{self, html::template_engine},
    ttj::{jsondiff::Substantial, table_diff},
};
use serde_json::{json, Map};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// If an entry is absent in one font, show the data anyway
    #[clap(long = "no-succinct", action = ArgAction::SetFalse)]
    succinct: bool,

    /// If an entry is absent in one font, just report it as absent
    #[clap(long = "succinct", overrides_with = "succinct")]
    _no_succinct: bool,

    /// Don't show diffs in font-tables
    #[clap(long = "no-tables", action = ArgAction::SetFalse)]
    tables: bool,

    /// Show diffs in font tables [default]
    #[clap(long = "tables", overrides_with = "tables")]
    _no_tables: bool,

    /// Don't show diffs in glyph images
    #[clap(long = "no-glyphs", action = ArgAction::SetFalse)]
    glyphs: bool,

    /// Show diffs in glyph images [default]
    #[clap(long = "glyphs", overrides_with = "glyphs")]
    _no_glyphs: bool,

    /// Don't show diffs in word images
    #[clap(long = "no-words", action = ArgAction::SetFalse)]
    words: bool,

    /// Show diffs in word images [default]
    #[clap(long = "words", overrides_with = "words")]
    _no_words: bool,

    /// Show diffs as JSON
    #[clap(long = "json")]
    json: bool,
    /// Show diffs as HTML
    #[clap(long = "html")]
    html: bool,

    /// Indent JSON
    #[clap(long = "pretty", requires = "json")]
    pretty: bool,

    /// Output directory for HTML
    #[clap(long = "output", default_value = "out", requires = "html")]
    output: String,

    /// Directory for custom templates
    #[clap(long = "templates", requires = "html")]
    templates: Option<String>,

    /// Location in design space, in the form axis=123,other=456
    #[clap(long = "location")]
    location: Option<String>,
    #[clap(long = "instance", conflicts_with = "location")]
    instance: Option<String>,

    /// The first font file to compare
    font1: PathBuf,
    /// The second font file to compare
    font2: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let font_binary_a = std::fs::read(&cli.font1).expect("Couldn't open file");
    let font_binary_b = std::fs::read(&cli.font2).expect("Couldn't open file");

    let tera = cli.html.then(|| template_engine(cli.templates.as_ref()));

    let mut font_a = DFont::new(&font_binary_a);
    let mut font_b = DFont::new(&font_binary_b);

    let mut result = Map::new();

    // Location-independent tests
    if cli.tables {
        let table_diff = table_diff(&font_a.fontref(), &font_b.fontref());
        if table_diff.is_something() {
            result.insert("tables".into(), table_diff);
        }
    }
    if cli.glyphs {
        let cmap_diff = new_missing_glyphs(&font_a, &font_b);
        result.insert("cmap_diff".into(), json!(cmap_diff));
    }

    // Location-specific tests
    let mut location_results = vec![];
    let mut this_location_value = Map::new();

    let loc_name: String = if let Some(ref loc) = cli.location {
        let _hack = font_a.set_location(loc);
        let _hack = font_b.set_location(loc);
        loc.clone()
    } else if let Some(ref inst) = cli.instance {
        font_a.set_instance(inst).expect("Couldn't find instance");
        font_b.set_instance(inst).expect("Couldn't find instance");
        inst.clone()
    } else {
        "default".into()
    };
    let loc_coords: HashMap<String, f32> = font_a
        .location
        .iter()
        .map(|v| (v.selector.to_string(), v.value))
        .collect();
    this_location_value.insert("location".into(), json!(loc_name));
    this_location_value.insert("coords".into(), json!(loc_coords));

    if cli.glyphs {
        let glyph_diff = modified_encoded_glyphs(&font_a, &font_b);
        if !glyph_diff.is_empty() {
            this_location_value.insert("glyphs".into(), json!(glyph_diff));
        }
    }
    if cli.words {
        let word_diff = test_font_words(&font_a, &font_b);
        this_location_value.insert("words".into(), word_diff);
    }

    location_results.push(this_location_value);
    result.insert("locations".into(), json!(location_results));

    // Report back
    if cli.html {
        reporters::html::report(
            &cli.font1,
            &cli.font2,
            Path::new(&cli.output),
            &font_a,
            &font_b,
            result,
            tera.unwrap(),
        );
    } else if cli.json {
        reporters::json::report(result.into(), cli.pretty);
    } else {
        reporters::text::report(result, cli.succinct);
    }
}
