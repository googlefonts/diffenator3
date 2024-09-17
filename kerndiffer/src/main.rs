/// Stand-alone application for comparing kerning tables.
///
/// This is a simple command-line tool that compares the kerning tables of two fonts.
/// Normally you would use [diffenator3] instead, but this tool is useful for
/// focusing on the kerning specifically.
use clap::Parser;
use colored::Colorize;
use env_logger::Env;
use read_fonts::FontRef;
use serde_json::{Map, Value};
use std::path::PathBuf;
use ttj::jsondiff::Substantial;
use ttj::kern_diff;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Maximum number of changes to report before giving up
    #[clap(long = "max-changes", default_value = "128", help_heading = Some("Report format"))]
    max_changes: usize,

    /// Don't try to match glyph names between fonts
    #[clap(long = "no-match", help_heading = Some("Report format"))]
    no_match: bool,

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

    let font_a = FontRef::new(&font_binary_a).expect("Couldn't parse font");
    let font_b = FontRef::new(&font_binary_b).expect("Couldn't parse font");
    let diff = kern_diff(&font_a, &font_b, cli.max_changes, cli.no_match);
    if let Value::Object(diff) = &diff {
        show_map_diff(diff, 0, false);
    } else {
        println!("No differences found");
    }
}

pub fn show_map_diff(fields: &Map<String, serde_json::Value>, indent: usize, succinct: bool) {
    for (field, diff) in fields.iter() {
        print!("{}", " ".repeat(indent * 2));
        if field == "error" {
            println!("{}", diff.as_str().unwrap().red());
            continue;
        }
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
