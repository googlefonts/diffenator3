/// Create before/after HTML proofs of two fonts
// In a way this is not related to the core goal of diffenator3, but
// at the same time, we happen to have all the moving parts required
// to make this, and it would be a shame not to use them.
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use clap::Parser;
use diffenator3_lib::{
    dfont::{shared_axes, DFont},
    html::{gen_html, template_engine},
};
use env_logger::Env;
use google_fonts_languages::{SampleTextProto, LANGUAGES, SCRIPTS};
use serde_json::json;

#[derive(Parser, Debug, clap::ValueEnum, Clone, PartialEq)]
pub enum SampleMode {
    /// Sample text emphasises real language input
    Context,
    /// Sample text optimizes for codepoint coverage
    Cover,
    /// A ladder of point sizes, to spot hinting/rasterization jumps
    Waterfall,
    /// Every encoded glyph in the font, laid out in a grid
    Glyphs,
    /// Curated strings that stress sidebearings and kerning pairs
    Spacing,
}

#[derive(Parser, Debug)]
#[command(version, about = "Create before/after HTML proofs of fonts", long_about = None)]
pub struct Cli {
    /// Output directory for HTML
    #[clap(long = "output", default_value = "out")]
    pub output: String,

    /// Directory for custom templates
    #[clap(long = "templates")]
    pub templates: Option<String>,

    /// Update diffenator3's stock templates
    #[clap(long = "update-templates")]
    pub update_templates: bool,

    /// Point size for sample text in pixels
    #[clap(long = "point-size", default_value = "25")]
    pub point_size: u32,

    /// Choice of sample text. Repeatable: --sample-mode context,waterfall,glyphs,spacing
    #[clap(long = "sample-mode", value_delimiter = ',', default_value = "context")]
    pub sample_mode: Vec<SampleMode>,

    /// Point sizes for the waterfall proof, in px
    #[clap(
        long = "waterfall-sizes",
        value_delimiter = ',',
        default_value = "7,10,11,12,14,16,18,21,27,32"
    )]
    pub waterfall_sizes: Vec<u32>,

    /// The first font file to compare
    pub font1: PathBuf,
    /// The second font file to compare
    pub font2: Option<PathBuf>,
}

/// Entry point for the diff3proof CLI. Call this from main().
pub fn cli_main() {
    let cli = Cli::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("warn")).init();
    run(&cli);
}

pub fn run(cli: &Cli) {
    let font_binary_a = std::fs::read(&cli.font1).expect("Couldn't open file");

    let tera = template_engine(cli.templates.as_ref(), cli.update_templates);
    let font_a = DFont::new(&font_binary_a);

    let (shared_codepoints, axes, instances) = if let Some(font2) = &cli.font2 {
        let font_binary_b = std::fs::read(font2).expect("Couldn't open file");
        let font_b = DFont::new(&font_binary_b);

        let shared_codepoints: HashSet<u32> = font_a
            .codepoints
            .intersection(&font_b.codepoints)
            .copied()
            .collect();
        let (axes, instances) = shared_axes(&font_a, &font_b);
        (shared_codepoints, axes, instances)
    } else {
        let shared_codepoints = font_a.codepoints.clone();
        let (axes, instances) = shared_axes(&font_a, &font_a);
        (shared_codepoints, axes, instances)
    };

    let axes_instances = serde_json::to_string(&json!({
        "axes": axes,
        "instances": instances
    }))
    .unwrap();

    let mut variables = serde_json::Map::new();
    variables.insert("axes_instances".to_string(), axes_instances.into());
    for mode in &cli.sample_mode {
        match mode {
            SampleMode::Context => {
                let sample_texts = language_sample_texts(&shared_codepoints);
                variables.insert("language_samples".to_string(), json!(sample_texts));
            }
            SampleMode::Cover => {
                let sample_text = cover_sample_texts(&shared_codepoints);
                variables.insert("cover_sample".to_string(), json!(sample_text));
            }
            SampleMode::Waterfall => {
                let sample_text = cover_sample_texts(&shared_codepoints);
                variables.insert("waterfall_sample".to_string(), json!(sample_text));
                variables.insert(
                    "waterfall_sizes".to_string(),
                    json!(cli.waterfall_sizes),
                );
            }
            SampleMode::Glyphs => {
                let mut glyphs: Vec<char> = shared_codepoints
                    .iter()
                    .filter_map(|&cp| char::from_u32(cp))
                    .collect();
                glyphs.sort();
                variables.insert("glyphs".to_string(), json!(glyphs));
            }
            SampleMode::Spacing => {
                let sections = spacing_kerning_strings(&shared_codepoints);
                variables.insert("spacing_kerning".to_string(), json!(sections));
            }
        }
    }

    gen_html(
        &cli.font1,
        cli.font2.as_ref().unwrap_or(&cli.font1),
        Path::new(&cli.output),
        tera,
        "diff3proof.html",
        &variables.into(),
        "diff3proof.html",
        cli.point_size,
    );
}

// Ported from GF_Latin_Core's "Spacing" and "Kerning" sections in
// googlefonts/diffenator2's src/diffenator2/data/test_strings.json.
// These strings specifically stress sidebearing consistency and kerning
// pairs and, unlike language_sample_texts, don't need a second font to be
// useful: they work standalone when onboarding a brand-new font.
//
// v1 is Latin-only, gated by codepoint coverage the same way
// language_sample_texts already is. Other scripts' equivalents can be
// added the same way as follow-up work.
const LATIN_CORE_SPACING: &[&str] = &[
    "HHOHHOOHOO HIHOIO",
    "uuonouonoo ninoioolonlniuo",
    "010 080 030 020 050 060 070",
    "3+4=7 5>2 2x²−(8×7)÷9≈1",
    "L·L l·l",
];

const LATIN_CORE_KERNING: &[&str] = &[
    "HHAVAHH OXOWAFAPATAUAYAÞYLYTJLVÆAVAOYO HH",
    "Ti Tí Tî Tï Tì Tī Vă Tä Tü Tõ Tŭ Tř",
    "HHTan oTo yTy vAv oVo oWo eYe Ly Ky Ac Ay gj",
    "nin non nvn ovo oxo ayn fl fi fj ft gj ko rento",
    "ďá ďu gj ľk włw yły ółr fħ ílïlîl",
    "H.Y-Y.T,F.P,V:Y„V-T-A*A’A“A’A-AL–L—L-",
    "n.r.r,y.y,(o)(i)(d)(j)(f)[j]{f}",
    "¿o,O? w@y «n•n» n*",
    "080 076 474 973 94 7078 54 24 272 679",
    "P4T47A .47,9.7-»4",
    "42°21′29″N 71°03′49″W",
    "d² m³ 15°C/26°F",
    "0/0\\0",
];

/// Filter the bundled spacing/kerning wordlists down to the strings this
/// font can actually render, grouped by section title (in the same shape
/// diffenator2's "Proofer" template consumed: an ordered list of
/// (section title, strings) pairs).
fn spacing_kerning_strings(codepoints: &HashSet<u32>) -> Vec<(String, Vec<String>)> {
    let sections: [(&str, &[&str]); 2] = [
        ("Spacing", LATIN_CORE_SPACING),
        ("Kerning", LATIN_CORE_KERNING),
    ];
    sections
        .iter()
        .filter_map(|(title, strings)| {
            let filtered: Vec<String> = strings
                .iter()
                .filter(|s| {
                    s.chars()
                        .all(|c| c.is_whitespace() || codepoints.contains(&(c as u32)))
                })
                .map(|s| s.to_string())
                .collect();
            if filtered.is_empty() {
                None
            } else {
                Some((title.to_string(), filtered))
            }
        })
        .collect()
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
