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

pub fn report(result: Map<String, tera::Value>, succinct: bool) {
    if result.contains_key("tables") {
        for (table_name, diff) in result["tables"].as_object().unwrap().iter() {
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

    if result.contains_key("glyphs") {
        println!("\n# Glyphs");
        let display_glyph = |glyph: &serde_json::Value| {
            println!(
                "  - {} ({}: {}) {:.3}%",
                glyph["string"].as_str().unwrap(),
                glyph["unicode"].as_str().unwrap(),
                glyph["name"].as_str().unwrap(),
                glyph["percent"].as_f64().unwrap()
            );
        };
        let map = result["glyphs"].as_object().unwrap();
        if map["missing"].is_something() {
            println!("\nMissing glyphs:");
            for glyph in map["missing"].as_array().unwrap() {
                display_glyph(glyph);
            }
        }
        if map["new"].is_something() {
            println!("\nNew glyphs:");
            for glyph in map["new"].as_array().unwrap() {
                display_glyph(glyph);
            }
        }
        if map["modified"].is_something() {
            println!("\nModified glyphs:");
            for glyph in map["modified"].as_array().unwrap() {
                display_glyph(glyph);
            }
        }
    }
    if result.contains_key("words") {
        println!("# Words");
        let map = result["words"].as_object().unwrap();
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
