use clap::{builder::ArgAction, Parser};
use colored::Colorize;
use diffenator3::{
    dfont::DFont,
    render::test_fonts,
    ttj::{jsondiff::Substantial, table_diff},
};
use serde_json::{json, Map};
use std::path::PathBuf;

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

    /// Show diffs as JSON
    #[clap(long = "json")]
    json: bool,

    /// The first font file to compare
    font1: PathBuf,
    /// The second font file to compare
    font2: PathBuf,
}

fn show_map_diff(fields: &Map<String, serde_json::Value>, indent: usize, succinct: bool) {
    for (field, diff) in fields.iter() {
        print!("{}", " ".repeat(indent * 2));
        if let Some(lr) = diff.as_array() {
            let (left, right) = (&lr[0], &lr[1]);
            if succinct && (left.is_something() && !right.is_something()) {
                println!(
                    "{}: {} => {}",
                    field,
                    format!("{}", left).green(),
                    "<absent>".red().italic()
                );
            } else if succinct && (right.is_something() && !left.is_something()) {
                println!(
                    "{}: {} => {}",
                    field,
                    "<absent>".green().italic(),
                    format!("{}", right).red()
                );
            } else {
                println!(
                    "{}: {} => {}",
                    field,
                    format!("{}", left).green(),
                    format!("{}", right).red()
                );
            }
        } else if let Some(fields) = diff.as_object() {
            println!("{}:", field);
            show_map_diff(fields, indent + 1, succinct)
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let font_binary_a = std::fs::read(cli.font1).expect("Couldn't open file");
    let font_binary_b = std::fs::read(cli.font2).expect("Couldn't open file");

    let font_a = DFont::new(&font_binary_a);
    let font_b = DFont::new(&font_binary_b);
    let mut diff = Map::new();
    if cli.tables {
        let table_diff = table_diff(&font_a.fontref(), &font_b.fontref());
        if table_diff.is_something() {
            diff.insert("tables".into(), table_diff);
        }
    }
    if cli.glyphs {
        let glyph_diff = test_fonts(&font_a, &font_b);
        if glyph_diff.is_something() {
            diff.insert("glyphs".into(), glyph_diff);
        }
    }
    // Handle strings here later
    if cli.json {
        println!("{}", serde_json::to_string_pretty(&diff).expect("foo"));
        std::process::exit(0);
    }

    if diff.contains_key("tables") {
        for (table_name, diff) in diff["tables"].as_object().unwrap().iter() {
            println!("\n# {}", table_name);
            if let Some(lr) = diff.as_array() {
                let (left, right) = (&lr[0], &lr[1]);
                if cli.succinct && (left.is_something() && !right.is_something()) {
                    println!("Table was present in LHS but absent in RHS");
                } else if cli.succinct && (right.is_something() && !left.is_something()) {
                    println!("Table was present in RHS but absent in LHS");
                } else {
                    println!("LHS had: {}", left);
                    println!("RHS had: {}", right);
                }
            } else if let Some(fields) = diff.as_object() {
                show_map_diff(fields, 0, cli.succinct);
            }
        }
    }
    if diff.contains_key("glyphs") {
        println!("\n# Glyphs");
        let map = diff["glyphs"].as_object().unwrap();
        if map["missing"].is_something() {
            println!("\nMissing glyphs:");
            for glyph in map["missing"].as_array().unwrap() {
                println!("  {}", glyph);
            }
        }
        if map["new"].is_something() {
            println!("\nNew glyphs:");
            for glyph in map["new"].as_array().unwrap() {
                println!("  {}", glyph);
            }
        }
        if map["modified"].is_something() {
            println!("\nModified glyphs:");
            for glyph in map["modified"].as_array().unwrap() {
                println!("  {}", glyph);
            }
        }
    }
}
