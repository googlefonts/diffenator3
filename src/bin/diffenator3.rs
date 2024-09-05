use clap::builder::ArgAction;
use clap::Parser;
use diffenator3::dfont::DFont;
use diffenator3::render::encodedglyphs::{modified_encoded_glyphs, new_missing_glyphs};
use diffenator3::render::test_font_words;
use diffenator3::reporters::html::template_engine;
use diffenator3::reporters::{self, LocationResult, Report};
use diffenator3::setting::{parse_location, Setting};
use diffenator3::ttj::jsondiff::Substantial;
use diffenator3::ttj::table_diff;
use env_logger::Env;
use indexmap::IndexSet;
use itertools::Itertools;
use skrifa::{MetadataProvider, Tag};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Don't show diffs in font-tables
    #[clap(long = "no-tables", action = ArgAction::SetFalse, help_heading = Some("Tests to run"))]
    tables: bool,

    /// Show diffs in font tables [default]
    #[clap(long = "tables", overrides_with = "tables", help_heading = Some("Tests to run"))]
    _no_tables: bool,

    /// Don't show diffs in glyph images
    #[clap(long = "no-glyphs", action = ArgAction::SetFalse, help_heading = Some("Tests to run"))]
    glyphs: bool,

    /// Show diffs in glyph images [default]
    #[clap(long = "glyphs", overrides_with = "glyphs", help_heading = Some("Tests to run"))]
    _no_glyphs: bool,

    /// Don't show diffs in word images
    #[clap(long = "no-words", action = ArgAction::SetFalse, help_heading = Some("Tests to run"))]
    words: bool,

    /// Show diffs in word images [default]
    #[clap(long = "words", overrides_with = "words", help_heading = Some("Tests to run"))]
    _no_words: bool,

    /// Show diffs as JSON
    #[clap(long = "json", help_heading = Some("Report format"))]
    json: bool,
    /// Show diffs as HTML
    #[clap(long = "html", help_heading = Some("Report format"))]
    html: bool,
    /// If an entry is absent in one font, show the data anyway
    #[clap(long = "no-succinct", action = ArgAction::SetFalse, help_heading = Some("Report format"))]
    succinct: bool,

    /// If an entry is absent in one font, just report it as absent
    #[clap(long = "succinct", overrides_with = "succinct", help_heading = Some("Report format"))]
    _no_succinct: bool,

    /// Indent JSON
    #[clap(long = "pretty", requires = "json", help_heading = Some("Report format"))]
    pretty: bool,

    /// Output directory for HTML
    #[clap(long = "output", default_value = "out", requires = "html", help_heading = Some("Report format"))]
    output: String,

    /// Directory for custom templates
    #[clap(long = "templates", requires = "html", help_heading = Some("Report format"))]
    templates: Option<String>,

    /// Location in user space, in the form axis=123,other=456 (may be repeated)
    #[clap(long = "location", help_heading = "Locations to test")]
    location: Vec<String>,
    /// Instance to compare (may be repeated; use * for all instances)
    #[clap(
        long = "instances",
        help_heading = "Locations to test",
        default_value = "*"
    )]
    instances: Vec<String>,
    /// Cross-product (use min/default/max of all axes)
    #[clap(long = "cross-product", help_heading = "Locations to test")]
    cross_product: bool,
    /// Cross-product splits
    #[clap(
        long = "cross-product-splits",
        help_heading = "Locations to test",
        default_value = "1"
    )]
    splits: usize,

    /// The first font file to compare
    font1: PathBuf,
    /// The second font file to compare
    font2: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("warn")).init();

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
    let settings: Vec<Setting> = generate_settings(&cli, &font_a, &font_b);

    result.locations = settings
        .into_iter()
        .map(|setting| {
            println!("Testing {}", setting.name());
            if let Err(e) = setting.set_on_fonts(&mut font_a, &mut font_b) {
                LocationResult::from_error(setting.name(), e)
            } else {
                test_at_location(&font_a, setting.name(), &cli, &font_b)
            }
        })
        .collect();

    // If there's more than one, filter out the boring ones
    if result.locations.len() > 1 {
        result.locations.retain(|l| l.is_some());
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

fn generate_settings(args: &Cli, font_a: &DFont, font_b: &DFont) -> Vec<Setting> {
    let mut settings = vec![];
    for instance in &args.instances {
        if instance == "*" {
            // Add the union of instances from both fonts
            let mut instances: IndexSet<String> = font_a.instances().into_iter().collect();
            instances.extend(font_b.instances().into_iter());
            settings.extend(instances.into_iter().map(Setting::from_instance));
        } else {
            settings.push(Setting::from_instance(instance.clone()));
        }
    }

    for location in &args.location {
        let loc = parse_location(location).expect("Couldn't parse location");
        settings.push(Setting::from_setting(loc));
    }
    if args.cross_product {
        let mut axes: HashSet<Tag> = font_a.fontref().axes().iter().map(|a| a.tag()).collect();
        axes.extend(font_b.fontref().axes().iter().map(|a| a.tag()));
        let axes_min_max = axes
            .iter()
            .map(|tag| {
                let a = font_a.fontref().axes().iter().find(|a| a.tag() == *tag);
                let b = font_b.fontref().axes().iter().find(|a| a.tag() == *tag);
                let a_extents = a.map(|a| (a.min_value(), a.default_value(), a.max_value()));
                let b_extents = b.map(|a| (a.min_value(), a.default_value(), a.max_value()));
                match (a_extents, b_extents) {
                    (Some((a_min, a_default, a_max)), Some((b_min, _b_default, b_max))) => {
                        (*tag, (a_min.min(b_min), a_default, a_max.max(b_max)))
                    }
                    (Some((a_min, a_default, a_max)), None) => (*tag, (a_min, a_default, a_max)),
                    (None, Some((b_min, b_default, b_max))) => (*tag, (b_min, b_default, b_max)),
                    (None, None) => panic!("Couldn't find axis"),
                }
            })
            .collect::<HashMap<Tag, (f32, f32, f32)>>();
        let mut per_axis_splits: Vec<Vec<(Tag, f32)>> = vec![];

        for (axis, tuple) in axes_min_max.into_iter() {
            per_axis_splits.push(split_axis(&axis, tuple, args.splits))
        }
        per_axis_splits.dedup();
        // Find the cartesian product of all axis/value iterators
        for locations in per_axis_splits.into_iter().multi_cartesian_product() {
            settings.push(Setting::from_setting(
                locations
                    .into_iter()
                    .map(|(a, v)| skrifa::setting::Setting::new(a, v))
                    .collect(),
            ));
        }
    }
    if settings.is_empty() {
        // Add default setting
        settings.push(Setting::Default);
    }
    settings
}

fn split_axis(axis: &Tag, tuple: (f32, f32, f32), split_count: usize) -> Vec<(Tag, f32)> {
    let (min, default, max) = tuple;
    let step = (default - min) / split_count as f32;
    let mut splits: Vec<f32> = vec![min];
    for i in 1..split_count {
        splits.push(min + step * i as f32);
    }
    splits.push(default);
    let step = (max - default) / split_count as f32;
    for i in 1..split_count {
        splits.push(default + step * i as f32);
    }
    splits.push(max);
    splits.dedup();
    splits.into_iter().map(|v| (*axis, v)).collect()
}
