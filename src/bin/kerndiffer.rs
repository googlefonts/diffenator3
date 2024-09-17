/// Stand-alone application for comparing kerning tables.
///
/// This is a simple command-line tool that compares the kerning tables of two fonts.
/// Normally you would use [diffenator3] instead, but this tool is useful for
/// focusing on the kerning specifically.
use clap::Parser;
use diffenator3::dfont::DFont;
use diffenator3::reporters::text::show_map_diff;
use diffenator3::ttj::kern_diff;
use env_logger::Env;
use serde_json::Value;
use std::path::PathBuf;

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

    let font_a = DFont::new(&font_binary_a);
    let font_b = DFont::new(&font_binary_b);
    let diff = kern_diff(
        &font_a.fontref(),
        &font_b.fontref(),
        cli.max_changes,
        cli.no_match,
    );
    if let Value::Object(diff) = &diff {
        show_map_diff(diff, 0, false);
    } else {
        println!("No differences found");
    }
}
