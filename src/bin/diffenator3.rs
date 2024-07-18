use clap::{builder::ArgAction, Parser};
use diffenator3::{
    dfont::DFont,
    render::{
        encodedglyphs::{modified_encoded_glyphs, new_missing_glyphs},
        test_font_words,
    },
    reporters::{self, html::template_engine, LocationResult, Report},
    ttj::{jsondiff::Substantial, table_diff},
};
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
    /// List of instances to compare
    #[clap(long = "instances", conflicts_with = "location")]
    instances: Option<Vec<String>>,

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

    let mut result = Report::default();

    // Location-independent tests
    if cli.tables {
        let table_diff = table_diff(&font_a.fontref(), &font_b.fontref());
        if table_diff.is_something() {
            result.tables = Some(table_diff);
        }
    }
    if cli.glyphs {
        result.cmap_diff = Some(new_missing_glyphs(&font_a, &font_b));
    }

    // Location-specific tests

    // let loc_name: String = if let Some(ref loc) = cli.location {
    //     let _hack = font_a.set_location(loc);
    //     let _hack = font_b.set_location(loc);
    //     loc.clone()
    // } else if let Some(ref inst) = cli.instance {
    //     font_a.set_instance(inst).expect("Couldn't find instance");
    //     font_b.set_instance(inst).expect("Couldn't find instance");
    //     inst.clone()
    // } else {
    //     "default".into()
    // };

    for instance in font_a.instances() {
        font_a
            .set_instance(&instance)
            .expect("Couldn't find instance");
        font_b
            .set_instance(&instance)
            .expect("Couldn't find instance");
        let location_name = instance;

        let this_location_value = test_at_location(&font_a, location_name, &cli, &font_b);
        result.locations.push(this_location_value);
    }

    // Report back
    if cli.html {
        reporters::html::report(
            &cli.font1,
            &cli.font2,
            Path::new(&cli.output),
            result,
            tera.unwrap(),
        );
    } else if cli.json {
        reporters::json::report(result, cli.pretty);
    } else {
        reporters::text::report(result, cli.succinct);
    }
}

fn test_at_location(font_a: &DFont, loc_name: String, cli: &Cli, font_b: &DFont) -> LocationResult {
    let mut this_location_value = LocationResult::default();
    let loc_coords: HashMap<String, f32> = font_a
        .location
        .iter()
        .map(|v| (v.selector.to_string(), v.value))
        .collect();
    this_location_value.location = loc_name;
    this_location_value.coords = loc_coords;

    if cli.glyphs {
        this_location_value.glyphs = modified_encoded_glyphs(font_a, font_b);
    }
    if cli.words {
        this_location_value.words = Some(test_font_words(font_a, font_b));
    }
    this_location_value
}
