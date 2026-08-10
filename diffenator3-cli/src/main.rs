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
    dfont::DFont,
    html::template_engine,
    render::{
        encodedglyphs::{modified_encoded_glyphs, CmapDiff},
        test_font_words,
    },
    setting::Setting,
    WordList,
};
use env_logger::Env;
use std::{collections::HashMap, path::Path};
use ttj::{jsondiff::Substantial, kern_diff, table_diff};

fn main() {
    let mut cli = Cli::parse();
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

    let mut font_a = DFont::new(&font_binary_a);
    let mut font_b = DFont::new(&font_binary_b);

    let mut result = Report::default();

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
    if cli.kerns {
        log::info!("Diffing kerning");
        let kern_diff = kern_diff(
            &font_a.fontref(),
            &font_b.fontref(),
            cli.max_changes,
            cli.no_match,
        );
        if kern_diff.is_something() {
            result.kerns = Some(kern_diff);
        }
    }
    if cli.glyphs {
        result.cmap_diff = Some(CmapDiff::new(&font_a, &font_b));
    }
    if cli.languages {
        log::info!("Diffing language support");
        result.languages = Some(languages::diff_languages(&font_a, &font_b));
    }

    if cli.glyphs {
        result.glyphs = modified_encoded_glyphs(&font_a, &font_b);
    }
    if cli.words {
        result.words = test_font_words(&font_a, &font_b, wordlists);
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
