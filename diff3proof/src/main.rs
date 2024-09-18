use std::collections::HashMap;
use std::path::Path;
use std::{collections::HashSet, path::PathBuf};

/// Create before/after HTML proofs of two fonts
// In a way this is not related to the core goal of diffenator3, but
// at the same time, we happen to have all the moving parts required
// to make this, and it would be a shame not to use them.
use clap::Parser;
use diffenator3_lib::dfont::{shared_axes, DFont};
use diffenator3_lib::html::{gen_html, template_engine};
use env_logger::Env;
use google_fonts_languages::{SampleTextProto, LANGUAGES, SCRIPTS};
use serde_json::json;

#[derive(Parser, Debug, clap::ValueEnum, Clone, PartialEq)]
enum SampleMode {
    /// Sample text emphasises real language input
    Context,
    /// Sample text optimizes for codepoint coverage
    Cover,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Output directory for HTML
    #[clap(long = "output", default_value = "out")]
    output: String,

    /// Directory for custom templates
    #[clap(long = "templates")]
    templates: Option<String>,

    /// Update diffenator3's stock templates
    #[clap(long = "update-templates")]
    update_templates: bool,

    /// Point size for sample text in pixels
    #[clap(long = "point-size", default_value = "25")]
    point_size: u32,

    /// Choice of sample text
    #[clap(long = "sample-mode", default_value = "context")]
    sample_mode: SampleMode,

    /// Update
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

    let tera = template_engine(cli.templates.as_ref(), cli.update_templates);
    let font_a = DFont::new(&font_binary_a);
    let font_b = DFont::new(&font_binary_b);

    let shared_codepoints: HashSet<u32> = font_a
        .codepoints
        .intersection(&font_b.codepoints)
        .copied()
        .collect();
    let (axes, instances) = shared_axes(&font_a, &font_b);
    let axes_instances = serde_json::to_string(&json!({
        "axes": axes,
        "instances": instances
    }))
    .unwrap();

    let mut variables = serde_json::Map::new();
    variables.insert("axes_instances".to_string(), axes_instances.into());
    match cli.sample_mode {
        SampleMode::Context => {
            let sample_texts = language_sample_texts(&shared_codepoints);
            variables.insert("language_samples".to_string(), json!(sample_texts));
        }
        SampleMode::Cover => {
            let sample_text = cover_sample_texts(&shared_codepoints);
            variables.insert("cover_sample".to_string(), json!(sample_text));
        }
    }

    gen_html(
        &cli.font1,
        &cli.font2,
        Path::new(&cli.output),
        tera,
        "diff3proof.html",
        &variables.into(),
        "diff3proof.html",
        cli.point_size,
    );
}

fn longest_sampletext(st: &SampleTextProto) -> &str {
    if let Some(text) = &st.specimen_16 {
        return text;
    }
    if let Some(text) = &st.specimen_21 {
        return text;
    }
    if let Some(text) = &st.specimen_32 {
        return text;
    }
    if let Some(text) = &st.specimen_36 {
        return text;
    }
    if let Some(text) = &st.specimen_48 {
        return text;
    }
    if let Some(text) = &st.tester {
        return text;
    }
    ""
}

fn language_sample_texts(codepoints: &HashSet<u32>) -> HashMap<String, Vec<(String, String)>> {
    let mut texts = HashMap::new();
    let re = fancy_regex::Regex::new(r"^(.{20,})(\1)").unwrap();
    let mut seen_cps = HashSet::new();
    // Sort languages by number of speakers
    let mut languages: Vec<_> = LANGUAGES.values().collect();
    languages.sort_by_key(|lang| -lang.population.unwrap_or(0));

    for lang in languages.iter() {
        if let Some(sample) = lang.sample_text.as_ref().map(longest_sampletext) {
            let mut sample = sample.replace('\n', " ");
            let sample_chars = sample.chars().map(|c| c as u32).collect::<HashSet<u32>>();

            // Can we render this text?
            if !sample_chars.is_subset(codepoints) {
                continue;
            }
            // Does this add anything new to the mix?
            if sample_chars.is_subset(&seen_cps) {
                continue;
            }
            seen_cps.extend(sample_chars);
            let script = lang.script();
            let script_name = SCRIPTS.get(script).unwrap().name();
            // Remove repeated phrases
            if let Ok(Some(captures)) = re.captures(&sample) {
                sample = captures.get(1).unwrap().as_str().to_string();
            }
            texts
                .entry(script_name.to_string())
                .or_insert_with(Vec::new)
                .push((lang.name().to_string(), sample.to_string()));
        }
    }
    texts
}

fn cover_sample_texts(codepoints: &HashSet<u32>) -> String {
    // Create a bag of shapable words
    let mut words = HashSet::new();
    let mut languages: Vec<_> = LANGUAGES.values().collect();
    languages.sort_by_key(|lang| -lang.population.unwrap_or(0));

    for lang in languages.iter() {
        if let Some(sample) = lang.sample_text.as_ref().map(longest_sampletext) {
            let sample = sample.replace('\n', " ");
            for a_word in sample.split_whitespace() {
                let word_chars = a_word.chars().map(|c| c as u32).collect::<HashSet<u32>>();
                // Can we render this text?
                if !word_chars.is_subset(codepoints) {
                    continue;
                }
                words.insert(a_word.to_string());
            }
        }
    }

    // Now do the greedy cover
    let mut uncovered_codepoints = codepoints.clone();
    let mut best_words = vec![];
    let mut prev_count = usize::MAX;
    while !uncovered_codepoints.is_empty() {
        if uncovered_codepoints.len() == prev_count {
            break;
        }
        prev_count = uncovered_codepoints.len();
        let best_word = words
            .iter()
            .max_by_key(|word| {
                let word_chars = word.chars().map(|c| c as u32).collect::<HashSet<u32>>();
                word_chars.intersection(&uncovered_codepoints).count()
            })
            .unwrap();
        for char in best_word.chars() {
            uncovered_codepoints.remove(&(char as u32));
        }
        best_words.push(best_word.to_string());
    }
    best_words.sort();
    best_words.join(" ")
}
