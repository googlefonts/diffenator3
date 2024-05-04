use clap::{Arg, Command, Parser};
use diffenator3::{dfont::DFont, render::test_fonts, ttj::table_diff};
use serde_json::json;
use std::{env, path::PathBuf};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The first font file to compare
    font1: PathBuf,
    /// The second font file to compare
    font2: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let font_binary_a = std::fs::read(cli.font1).expect("Couldn't open file");
    let font_binary_b = std::fs::read(cli.font2).expect("Couldn't open file");

    let font_a = DFont::new(&font_binary_a);
    let font_b = DFont::new(&font_binary_b);
    let output = test_fonts(&font_a, &font_b);
    let table_diff = table_diff(&font_a.fontref(), &font_b.fontref());
    let diff = json!({
        "glyph_diff": output,
        "strings": Vec::<String>::new(),
        "tables": table_diff,
    });
    println!("{}", serde_json::to_string_pretty(&diff).expect("foo"));
}
