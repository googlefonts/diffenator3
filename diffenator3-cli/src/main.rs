/// Show differences between two font files
///
/// This software can analyze two OpenType files for differences in rendering,
/// and shaping. It does this by comparing images of glyphs and shaped text
/// and looking for differences between the renderings.
///
/// Additionally, it can compare kerning table information and binary tables.
mod args;
mod languages;
mod reporters;
use crate::{
    args::Cli,
    reporters::{LocationResult, Report},
};
use clap::Parser;
use diffenator3_lib::{
    dfont::{parse_location, DFont},
    html::template_engine,
    render::{
        encodedglyphs::{modified_encoded_glyphs, CmapDiff},
        test_font_words,
    },
    staticdiff::compute_signature,
    summary::summarize,
    WordList,
};
use env_logger::Env;
use std::{collections::HashMap, path::Path};
use ttj::{jsondiff::Substantial, table_diff};

fn main() {
    let cli = Cli::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or(if cli.quiet {
        "error"
    } else {
        "info"
    }))
    .init();

    if let Some(threads) = cli.jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .expect("Could not set thread count");
    }

    let font_binary_a = std::fs::read(&cli.font1).expect("Couldn't open file");
    let font_binary_b = std::fs::read(&cli.font2).expect("Couldn't open file");

    let tera = cli
        .html
        .then(|| template_engine(cli.templates.as_ref(), cli.update_templates));

    let font_a = DFont::new(&font_binary_a);
    let font_b = DFont::new(&font_binary_b);

    let mut result = Report::default();

    // Static analysis of the two fonts, computed once up here. It drives word
    // selection during the behavioural tests, and feeds the human-readable
    // summary of changes in the report.
    let signature = compute_signature(&font_a, &font_b);
    result.signature_summary = Some(summarize(&signature, &font_a));

    let custom_wordlist_inputs: Vec<WordList> = cli
        .custom_wordlists
        .iter()
        .map(|path| {
            let data = std::fs::read_to_string(path).expect("Couldn't read custom wordlist");
            let name: String = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("custom")
                .to_string();
            WordList::define(name, data.lines().map(String::from))
        })
        .collect();

    // Location-independent tests
    if cli.tables {
        log::info!("Diffing binary tables");
        let table_diff = table_diff(
            &font_a.fontref(),
            &font_b.fontref(),
            cli.max_changes,
            cli.no_match,
        );
        if table_diff.is_something() {
            result.tables = Some(table_diff);
        }
    }

    let location = cli
        .location
        .as_ref()
        .map(|s| parse_location(s).expect("Couldn't parse location"));
    if cli.glyphs {
        result.cmap_diff = Some(CmapDiff::new(&font_a, &font_b));
    }
    if cli.languages {
        log::info!("Diffing language support");
        result.languages = Some(languages::diff_languages(&font_a, &font_b));
    }

    let mut location_result_map: HashMap<String, LocationResult> = HashMap::new();

    if cli.glyphs {
        let glyphs = modified_encoded_glyphs(
            &font_a,
            &font_b,
            location.as_ref(),
            Some(&signature),
            cli.glyphs_threshold,
        )
        .expect("Error diffing glyphs");
        // Break out by location and add to locationresults
        for glyph in glyphs {
            let location_key = glyph.location.clone();
            let location_result = location_result_map
                .entry(location_key.clone())
                .or_insert_with(|| LocationResult {
                    location: location_key,
                    ..Default::default()
                });
            location_result.glyphs.push(glyph);
        }
    }
    if cli.words {
        let words = test_font_words(
            &font_a,
            &font_b,
            Some(&signature),
            &custom_wordlist_inputs,
            location.as_ref(),
            cli.words_threshold,
        );
        // Insert into location map, don't break glyphs!
        for (wordlist_name, word_diffs) in words.into_iter() {
            for diff in word_diffs.into_iter() {
                let location_key = diff.location.clone();
                let location_result = location_result_map
                    .entry(location_key.clone())
                    .or_insert_with(|| LocationResult {
                        location: location_key,
                        ..Default::default()
                    });
                let result_diff = location_result
                    .words
                    .entry(wordlist_name.clone())
                    .or_default();
                result_diff.push(diff);
            }
        }
    }
    // Convert location map to vector for serialization
    result.locations = location_result_map.into_values().collect();
    // Set .coords field of each location result based on the font's designspace
    for location_result in result.locations.iter_mut() {
        location_result.coords = location_result
            .location
            .split(',')
            .filter_map(|coord| {
                let mut parts = coord.split('=');
                let axis = parts.next()?;
                let value = parts.next()?;
                let value = value.parse::<f32>().ok()?;
                Some((axis.to_string(), value))
            })
            .collect();
    }
    // Report back
    if cli.html {
        reporters::html::report(
            &cli.font1,
            &cli.font2,
            Path::new(&cli.output),
            tera.unwrap(),
            &result,
        );
    } else if cli.json {
        reporters::json::report(result, cli.pretty);
    } else {
        reporters::text::report(result, cli.succinct);
    }
}
