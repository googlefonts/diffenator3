//! Test harness for difference-signature-driven word selection.
//!
//! Given two fonts, builds the static [`DifferenceSignature`], shapes every
//! word in a word list at the default location, and reports which words would
//! be selected for the behavioural comparison (and at which designspace
//! locations), versus the exhaustive baseline. This lets us validate that the
//! selection is correct and that it narrows the work down to words that
//! actually exercise the differences.
//!
//! This is an *independent* test component for the `wordselect` module, in the
//! same spirit as `staticdifftest` for the `staticdiff`/`gposdiff` modules.

use rustc_hash::FxHashSet as HashSet;
use std::{
    path::PathBuf,
    str::FromStr,
    time::{Duration, Instant},
};

use clap::Parser;
use diffenator3_lib::{
    dfont::DFont,
    render::{shaper::CachedShaper, wordlists::direction_from_script},
    staticdiff::{compute_signature, DifferenceSignature},
    wordselect::{select_buffer, word_is_encoded},
};
use fontdrasil::coords::NormalizedLocation;
use harfrust::Script;
use serde_json::{json, Value};
use skrifa::{GlyphId, MetadataProvider};
use static_lang_word_lists::{WordList, DIFFENATOR_LATIN};

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Select words to test based on the static difference signature"
)]
struct Cli {
    /// First font file
    font1: PathBuf,
    /// Second font file
    font2: PathBuf,
    /// Custom word list file (one word per line)
    #[clap(long)]
    wordlist: Option<PathBuf>,
    /// Script name for the built-in word list (only LATIN is supported)
    #[clap(long, default_value = "LATIN")]
    script: String,
    /// Emit the report as JSON
    #[clap(long)]
    json: bool,
    /// Show words that were NOT selected (text mode, truncated)
    #[clap(long)]
    show_rejected: bool,
    /// Maximum number of selected words to list (default 100)
    #[clap(long, default_value_t = 100)]
    limit: usize,
    /// Only process the first N words (useful for fast profiling)
    #[clap(long)]
    max_words: Option<usize>,
}

fn main() {
    let cli = Cli::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let font_a = DFont::new(&std::fs::read(&cli.font1).expect("Couldn't open font 1"));
    let font_b = DFont::new(&std::fs::read(&cli.font2).expect("Couldn't open font 2"));

    let mut words: Vec<String> = if let Some(path) = &cli.wordlist {
        std::fs::read_to_string(path)
            .expect("Couldn't read word list")
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    } else {
        let wl: &WordList = match cli.script.as_str() {
            "LATIN" => &DIFFENATOR_LATIN,
            other => panic!("unknown script {other}; only LATIN is supported"),
        };
        wl.iter().map(|word| word.to_string()).collect()
    };
    if let Some(max) = cli.max_words {
        words.truncate(max);
    }
    let wordlist = WordList::define("test", words.iter().cloned());

    let script = wordlist.script().and_then(|s| Script::from_str(s).ok());
    let direction = script.and_then(direction_from_script);
    let shaper_a = CachedShaper::new(
        harfrust::FontRef::new(&font_a.backing).expect("couldn't parse font A"),
        16.0,
        direction,
        script,
    );
    // Note: we only shape font A. Selection is driven purely by font A's
    // buffer and the static signature; font B only appears in the signature.

    let start = std::time::Instant::now();
    let signature = compute_signature(&font_a, &font_b);
    let analysis_time = start.elapsed();

    // Coarse per-phase timing so we can see where the selection time goes.
    let mut t_encoded = Duration::ZERO;
    let mut t_shape = Duration::ZERO;
    let mut t_select = Duration::ZERO;
    let mut t_post = Duration::ZERO;

    // Derived selection indexes, built once and reused for every word:
    // rebuilding these inside select_buffer per word was the dominant cost.
    let uncertain = signature.uncertain_glyphs();
    let marks = font_a.mark_glyphs();

    // Words that got selected, with reasons and locations.
    let mut selected: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    // Encoded-but-unselected words; only the first few are kept for display.
    let mut rejected: Vec<String> = Vec::new();
    let mut rejected_count = 0usize;
    // Words skipped before shaping because a character isn't encoded in both fonts.
    let mut skipped = 0usize;
    let mut exhaustive_count = 0usize;

    // Coverage tracking: which changed glyphs/pairs/mark pairs the selected
    // words actually exercise.
    let mut covered_glyphs: HashSet<GlyphId> = HashSet::default();
    let mut covered_pairs: HashSet<(GlyphId, GlyphId)> = HashSet::default();
    let mut covered_marks: HashSet<(GlyphId, GlyphId)> = HashSet::default();

    // Render-operation accounting.
    let mut selected_ops: usize = 0;
    let mut exhaustive_ops: usize = 0;

    for word in wordlist.iter() {
        let t0 = Instant::now();
        // Skip words whose characters aren't encoded in both fonts: they shape
        // to `.notdef` in one of them, which would otherwise force uncertain
        // glyphs and the exhaustive fallback. (Now handled by the
        // word-selection module, mirroring diffenator3's wordlist filter.)
        if !word_is_encoded(&font_a, &font_b, word) {
            skipped += 1;
            t_encoded += t0.elapsed();
            continue;
        }
        t_encoded += t0.elapsed();

        let t1 = Instant::now();
        let buffer_a = shaper_a.shape(word, None);
        t_shape += t1.elapsed();

        let t2 = Instant::now();
        let selection = select_buffer(
            &signature, &uncertain, &marks, &font_a, &font_b, word, &buffer_a,
        );
        t_select += t2.elapsed();

        let t3 = Instant::now();
        // Exhaustive baseline: default + every variation peak of the buffer.
        // Computed for every encoded word so the reduction metric stays honest.
        let mut baseline: HashSet<NormalizedLocation> = font_a.variations_for_buffer(&buffer_a);
        baseline.insert(NormalizedLocation::default());
        exhaustive_ops += baseline.len();

        // All the coverage/glyph-list work below only matters for words we
        // actually select, so keep it out of the hot path for the ~99% that
        // are rejected.
        if selection.selected {
            let glyphs: Vec<GlyphId> = buffer_a.iter().map(|g| g.glyph_id).collect();
            let demarked: Vec<GlyphId> = glyphs
                .iter()
                .copied()
                .filter(|gid| !marks.contains(gid))
                .collect();

            for gid in &glyphs {
                if signature.glyph_changes.contains_key(gid) {
                    covered_glyphs.insert(*gid);
                }
            }
            for pair in demarked.windows(2) {
                if signature
                    .pair_position_changes
                    .contains_key(&(pair[0], pair[1]))
                {
                    covered_pairs.insert((pair[0], pair[1]));
                }
            }
            for i in 1..glyphs.len() {
                if marks.contains(&glyphs[i]) {
                    for j in 0..i {
                        if signature
                            .mark_position_changes
                            .contains_key(&(glyphs[i], glyphs[j]))
                        {
                            covered_marks.insert((glyphs[i], glyphs[j]));
                        }
                    }
                }
            }
            if selection.exhaustive {
                exhaustive_count += 1;
            }
            selected_ops += selection.locations.len();
            let locs: Vec<String> = selection
                .locations
                .iter()
                .map(|loc| location_user(&font_a, loc))
                .collect();
            selected.push((word.to_string(), selection.reasons.clone(), locs));
        } else {
            rejected_count += 1;
            if rejected.len() < 20 {
                rejected.push(word.to_string());
            }
        }
        t_post += t3.elapsed();
    }

    let total = start.elapsed();
    eprintln!("-- timing breakdown --");
    eprintln!(
        "  signature analysis : {:>8.3}s",
        analysis_time.as_secs_f64()
    );
    eprintln!("  encoding filter    : {:>8.3}s", t_encoded.as_secs_f64());
    eprintln!("  shaping            : {:>8.3}s", t_shape.as_secs_f64());
    eprintln!("  select_buffer      : {:>8.3}s", t_select.as_secs_f64());
    eprintln!("  post/record        : {:>8.3}s", t_post.as_secs_f64());
    eprintln!("  total              : {:>8.3}s", total.as_secs_f64());

    if cli.json {
        let json = report_json(
            &font_a,
            &signature,
            wordlist.len(),
            skipped,
            &selected,
            &covered_glyphs,
            &covered_pairs,
            &covered_marks,
            exhaustive_count,
            selected_ops,
            exhaustive_ops,
            analysis_time,
        );
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    } else {
        report_text(
            &font_a,
            &signature,
            wordlist.len(),
            skipped,
            &selected,
            &rejected,
            rejected_count,
            &covered_glyphs,
            &covered_pairs,
            &covered_marks,
            exhaustive_count,
            selected_ops,
            exhaustive_ops,
            analysis_time,
            &cli,
        );
    }
}

/// A location rendered as a user-space string (from font A).
fn location_user(font_a: &DFont, loc: &NormalizedLocation) -> String {
    let user = font_a.location_to_user(loc);
    if user.is_empty() {
        "default".to_string()
    } else {
        user
    }
}

fn glyph_name(font_a: &DFont, gid: GlyphId) -> String {
    font_a
        .fontref()
        .glyph_names()
        .get(gid)
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("gid{}", gid.to_u32()))
}

fn uncovered_pairs(
    font_a: &DFont,
    signature: &DifferenceSignature,
    covered: &HashSet<(GlyphId, GlyphId)>,
) -> Vec<String> {
    let mut out: Vec<String> = signature
        .pair_position_changes
        .keys()
        .filter(|pair| !covered.contains(pair))
        .map(|(l, r)| format!("{}/{}", glyph_name(font_a, *l), glyph_name(font_a, *r)))
        .collect();
    out.sort();
    out
}

fn report_text(
    font_a: &DFont,
    signature: &DifferenceSignature,
    total: usize,
    skipped: usize,
    selected: &[(String, Vec<String>, Vec<String>)],
    rejected: &[String],
    rejected_count: usize,
    covered_glyphs: &HashSet<GlyphId>,
    covered_pairs: &HashSet<(GlyphId, GlyphId)>,
    covered_marks: &HashSet<(GlyphId, GlyphId)>,
    exhaustive_count: usize,
    selected_ops: usize,
    exhaustive_ops: usize,
    analysis_time: std::time::Duration,
    cli: &Cli,
) {
    println!("== Word selection ==");
    println!("Font A: {} {}", font_a.family_name(), font_a.style_name());
    println!("Signature analysis: {:?}", analysis_time);
    println!(
        "Signature: {} changed glyphs, {} changed pairs, {} mark changes, {} uncertain",
        signature.glyph_changes.len(),
        signature.pair_position_changes.len(),
        signature.mark_position_changes.len(),
        signature.uncertain.len()
    );
    println!();
    println!(
        "Words: {} total, {} selected ({:.1}%), {} exhaustive, {} skipped (not encoded)",
        total,
        selected.len(),
        if total > 0 {
            100.0 * selected.len() as f64 / total as f64
        } else {
            0.0
        },
        exhaustive_count,
        skipped
    );
    let reduction = if exhaustive_ops > 0 {
        100.0 * (exhaustive_ops - selected_ops) as f64 / exhaustive_ops as f64
    } else {
        0.0
    };
    println!(
        "Render ops: {} selected vs {} exhaustive ({:.1}% reduction)",
        selected_ops, exhaustive_ops, reduction
    );
    println!();

    println!("Selected words:");
    if selected.is_empty() {
        println!("  (none)");
    }
    for (word, reasons, locs) in selected.iter().take(cli.limit) {
        let mut reasons = reasons.clone();
        reasons.sort();
        reasons.dedup();
        println!(
            "  {word:>20}  [{}]  at: {}",
            reasons.join("; "),
            locs.join(", ")
        );
    }
    if selected.len() > cli.limit {
        println!(
            "  ... and {} more (use --limit to list more)",
            selected.len() - cli.limit
        );
    }
    println!();

    println!("Coverage (do the selected words exercise every difference?):");
    let uncovered = uncovered_pairs(font_a, signature, covered_pairs);
    println!(
        "  changed glyphs: {}/{} covered",
        covered_glyphs.len(),
        signature.glyph_changes.len()
    );
    println!(
        "  changed pairs: {}/{} covered{}",
        covered_pairs.len(),
        signature.pair_position_changes.len(),
        if uncovered.is_empty() {
            String::new()
        } else {
            format!("  -- NOT covered: {}", uncovered.join(", "))
        }
    );
    println!(
        "  mark pairs: {}/{} covered",
        covered_marks.len(),
        signature.mark_position_changes.len()
    );
    println!();

    if cli.show_rejected {
        println!("Rejected words ({}):", rejected_count);
        for word in rejected.iter() {
            println!("  {word}");
        }
        if rejected_count > 20 {
            println!("  ... and {} more", rejected_count - 20);
        }
    }
}

fn report_json(
    font_a: &DFont,
    signature: &DifferenceSignature,
    total: usize,
    skipped: usize,
    selected: &[(String, Vec<String>, Vec<String>)],
    covered_glyphs: &HashSet<GlyphId>,
    covered_pairs: &HashSet<(GlyphId, GlyphId)>,
    covered_marks: &HashSet<(GlyphId, GlyphId)>,
    exhaustive_count: usize,
    selected_ops: usize,
    exhaustive_ops: usize,
    analysis_time: std::time::Duration,
) -> Value {
    let uncovered: Vec<String> = uncovered_pairs(font_a, signature, covered_pairs);
    json!({
        "summary": {
            "words_total": total,
            "words_selected": selected.len(),
            "words_exhaustive": exhaustive_count,
            "words_skipped_not_encoded": skipped,
            "render_ops_selected": selected_ops,
            "render_ops_exhaustive": exhaustive_ops,
            "reduction_percent": if exhaustive_ops > 0 {
                100.0 * (exhaustive_ops - selected_ops) as f64 / exhaustive_ops as f64
            } else { 0.0 },
            "analysis_time_secs": analysis_time.as_secs_f64(),
        },
        "signature": {
            "changed_glyphs": signature.glyph_changes.len(),
            "changed_pairs": signature.pair_position_changes.len(),
            "mark_changes": signature.mark_position_changes.len(),
            "uncertain": signature.uncertain.len(),
            "positioning_unmodelled": signature.positioning_unmodelled,
        },
        "coverage": {
            "changed_glyphs_covered": covered_glyphs.len(),
            "changed_glyphs_total": signature.glyph_changes.len(),
            "changed_pairs_covered": covered_pairs.len(),
            "changed_pairs_total": signature.pair_position_changes.len(),
            "changed_pairs_uncovered": uncovered,
            "mark_pairs_covered": covered_marks.len(),
            "mark_pairs_total": signature.mark_position_changes.len(),
        },
        "selected_words": selected
            .iter()
            .map(|(word, reasons, locs)| {
                json!({ "word": word, "reasons": reasons, "locations": locs })
            })
            .collect::<Vec<_>>(),
    })
}
