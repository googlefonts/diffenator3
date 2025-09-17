use super::{LocationResult, Report};

use colored::Colorize;
use serde_json::Map;
use tabled::{settings::Style, Table, Tabled};
use ttj::jsondiff::Substantial;

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

    if let Some(lang) = result.languages.as_ref() {
        println!("\n# Language Support Differences\n");
        if let Some(map) = lang.as_object() {
            report_language_support(map, succinct);
        } else {
            println!("\nNo differences found");
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
            println!(" - {} ({:.3} pixels)", glyph.string, glyph.differing_pixels);
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
                    difference["differing_pixels"].as_i64().unwrap()
                );
            }
        }
    }
}

#[derive(Tabled)]
struct DetailsRow {
    #[tabled(rename = "Language")]
    language: String,
    #[tabled(rename = "Support before")]
    support_a: String,
    #[tabled(rename = "Support after")]
    support_b: String,
    #[tabled(rename = "Same?")]
    same: String,
    #[tabled(rename = "Fixes needed")]
    glyphs_needed: u64,
}

fn report_language_support(map: &Map<String, serde_json::Value>, succinct: bool) {
    // Supported status table
    let mut builder = tabled::builder::Builder::default();
    builder.push_record(vec!["Support Level", "Font A", "Font B"]);

    for level in [
        "Complete",
        "Supported",
        "Incomplete",
        "Unsupported",
        "None",
        "Indeterminate",
    ] {
        let count_a = map
            .values()
            .filter(|v| v.get("level_a").and_then(|x| x.as_str()) == Some(level))
            .count();
        let count_b = map
            .values()
            .filter(|v| v.get("level_b").and_then(|x| x.as_str()) == Some(level))
            .count();
        if count_a == count_b && succinct {
            continue;
        }
        builder.push_record(vec![level, &count_a.to_string(), &count_b.to_string()]);
    }
    let mut table = builder.build();
    table.with(Style::markdown());
    println!("{}", table);
    // Detailed differences
    println!("\nLanguage differences:\n");
    let mut rows: Vec<DetailsRow> = vec![];
    for (lang, details) in map.iter() {
        if succinct && details["level_a"] == details["level_b"] {
            continue;
        }
        if details["level_a"] == "None" && details["level_b"] == "None" {
            continue;
        }
        if details["level_a"] == "Indeterminate" && details["level_b"] == "Indeterminate" {
            continue;
        }
        let level_a = details["level_a"].as_str().unwrap();
        let level_b = details["level_b"].as_str().unwrap();
        let score_a = details["score_a"].as_f64().unwrap_or_default();
        let score_b = details["score_b"].as_f64().unwrap_or_default();
        rows.push(DetailsRow {
            language: lang.clone(),
            support_a: level_a.to_string(),
            support_b: level_b.to_string(),
            same: if score_a == score_b {
                "Same"
            } else if score_b > score_a {
                "Better"
            } else {
                "Worse"
            }
            .to_string(),
            glyphs_needed: details["fixes_b"].as_u64().unwrap(),
        });
    }
    let mut table = Table::new(rows);
    table.with(Style::markdown());
    println!("{}", table);
}
