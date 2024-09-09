use clap::Parser;
use diffenator3::reporters::text::show_map_diff;
use diffenator3::ttj::font_to_json;
use diffenator3::ttj::jsondiff::diff;
use diffenator3::{dfont::DFont, ttj::namemap::NameMap};
use env_logger::Env;
use serde_json::{Map, Value};
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
    let glyphmap_a = NameMap::new(&font_a.fontref());
    let glyphmap_b = NameMap::new(&font_b.fontref());
    let big_difference = !cli.no_match && !glyphmap_a.compatible(&glyphmap_b);
    if big_difference {
        println!("Glyph names differ dramatically between fonts, using font names from font A");
    }
    let big_difference = false;
    let kerns_a = just_kerns(font_to_json(&font_a.fontref(), None));
    // println!("Font A flat kerning: {:#?}", kerns_a);
    let kerns_b = just_kerns(font_to_json(
        &font_b.fontref(),
        Some(if big_difference {
            &glyphmap_a
        } else {
            &glyphmap_b
        }),
    ));
    // println!("Font B flat kerning: {:#?}", kerns_a);

    let diff = diff(&kerns_a, &kerns_b, cli.max_changes);
    if let Value::Object(diff) = &diff {
        show_map_diff(diff, 0, false);
    } else {
        println!("No differences found");
    }
}

// Since we created the data structure, we're going to be unwrap()ping with gay abandon.
fn just_kerns(font: Value) -> Value {
    let mut flatkerns = Map::new();
    for lookup in font
        .get("GPOS")
        .and_then(|x| x.get("lookup_list"))
        .and_then(|x| x.as_object())
        .map(|x| x.values())
        .into_iter()
        .flatten()
        .flat_map(|x| x.as_array().unwrap().iter())
        .filter(|x| x.get("type").map(|x| x == "pair").unwrap_or(false))
        .map(|x| x.as_object().unwrap())
    {
        if lookup.contains_key("kerns") && lookup.contains_key("classes") {
            // Flatten class kerning
            let classes = lookup.get("classes").unwrap().as_object().unwrap();
            let kerns = lookup.get("kerns").unwrap().as_object().unwrap();
            for (left_class, value) in kerns.iter() {
                for (right_class, kern) in value.as_object().unwrap().iter() {
                    if kern == "0" {
                        continue;
                    }
                    for left_glyph in classes.get(left_class).unwrap().as_array().unwrap().iter() {
                        // println!("left (class): {:#?}", left_glyph);

                        for right_glyph in
                            classes.get(right_class).unwrap().as_array().unwrap().iter()
                        {
                            // println!("   right (class): {:#?}, kern: {:#?}", right_glyph, kern);
                            let key = left_glyph.as_str().unwrap().to_owned()
                                + "/"
                                + right_glyph.as_str().unwrap();

                            if let Some(existing) = flatkerns.get(&key) {
                                flatkerns.insert(
                                    key,
                                    Value::String(
                                        existing.as_str().unwrap().to_owned()
                                            + " + "
                                            + kern.as_str().unwrap(),
                                    ),
                                );
                            } else {
                                flatkerns.insert(key, kern.clone());
                            }
                        }
                    }
                }
            }
        } else {
            for (left, value_map) in lookup.iter() {
                // println!("left: {:#?}", left);
                if left == "type" {
                    continue;
                }
                for (right, value) in value_map.as_object().unwrap().iter() {
                    // println!(" right: {:#?}, value: {:?}", right, value);
                    let key = left.to_owned() + "/" + right.as_str();
                    if let Some(existing) = flatkerns.get(&key) {
                        flatkerns.insert(
                            key,
                            Value::String(
                                existing.as_str().unwrap().to_owned()
                                    + " + "
                                    + value.as_str().unwrap(),
                            ),
                        );
                    } else {
                        flatkerns.insert(key, value.clone());
                    }
                }
            }
        }
    }
    Value::Object(flatkerns)
}
