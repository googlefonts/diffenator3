use super::{LocationResult, Report};

use crate::ttj::jsondiff::Substantial;
use colored::Colorize;
use serde_json::Map;

fn show_map_diff(fields: &Map<String, serde_json::Value>, indent: usize, succinct: bool) {
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

pub fn report(result: Report, succinct: bool) {
    if let Some(tables) = result.tables {
        for (table_name, diff) in tables.as_object().unwrap().iter() {
            if diff.is_something() {
                println!("\n# {}", table_name);
            }
            if let Some(lr) = diff.as_array() {
                let (left, right) = (&lr[0], &lr[1]);
                if succinct && (left.is_something() && !right.is_something()) {
                    println!("Table was present in LHS but absent in RHS");
                } else if succinct && (right.is_something() && !left.is_something()) {
                    println!("Table was present in RHS but absent in LHS");
                } else {
                    println!("LHS had: {}", left);
                    println!("RHS had: {}", right);
                }
            } else if let Some(fields) = diff.as_object() {
                show_map_diff(fields, 0, succinct);
            } else {
                println!("Unexpected diff format: {}", diff);
            }
        }
    }

    if let Some(cmap_diff) = result.cmap_diff {
        println!("\n# Encoded Glyphs");
        if !cmap_diff.missing.is_empty() {
            println!("\nMissing glyphs:");
            for glyph in cmap_diff.missing {
                println!(" - {} ", glyph);
            }
        }
        if !cmap_diff.new.is_empty() {
            println!("\nNew glyphs:");
            for glyph in cmap_diff.new {
                println!(" - {} ", glyph);
            }
        }
    }

    for locationresult in result.locations {
        if locationresult.is_some() {
            report_location(locationresult);
        }
    }
}

fn report_location(locationresult: LocationResult) {
    print!("# Differences at location {} ", locationresult.location);
    if !locationresult.coords.is_empty() {
        print!("( ");
        for (k, v) in locationresult.coords.iter() {
            print!("{}: {}, ", k, v);
        }
        print!(")");
    }
    println!();

    if !locationresult.glyphs.is_empty() {
        println!("\n## Glyphs");
        for glyph in locationresult.glyphs {
            println!(" - {} ({:.3}%)", glyph.string, glyph.percent);
        }
    }

    if let Some(words) = locationresult.words {
        println!("# Words");
        let map = words.as_object().unwrap();
        for (script, script_diff) in map.iter() {
            println!("\n## {}", script);
            for difference in script_diff.as_array().unwrap().iter() {
                println!(
                    "  - {} ({:.3}%)",
                    difference["word"].as_str().unwrap(),
                    difference["percent"].as_f64().unwrap()
                );
            }
        }
    }
}
