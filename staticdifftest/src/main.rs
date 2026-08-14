//! A small test harness for the static difference analysis.
//!
//! Given two font files, builds the static [`DifferenceSignature`] -- the
//! shaping-free analysis of where the fonts' glyphs differ across the design
//! space -- and reports it to the user, in text or JSON.
//!
//! This is an *independent* test component for the `staticdiff` module: it
//! lets us validate that the analysis is correct and comprehensive across a
//! variety of fonts, before wiring the results into word selection and then
//! into `diffenator3` proper.

use rustc_hash::FxHashMap;
use std::{collections::HashMap, path::PathBuf};

use clap::Parser;
use diffenator3_lib::{
    dfont::DFont,
    staticdiff::{DifferenceSignature, GlyphChange, GlyphReport, LocationSet, MatchMethod},
};
use fontdrasil::coords::NormalizedLocation;
use serde_json::{json, Map, Value};
use skrifa::{GlyphId, MetadataProvider};

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Report static (shaping-free) differences between two fonts"
)]
struct Cli {
    /// First font file
    font1: PathBuf,
    /// Second font file
    font2: PathBuf,
    /// Emit the report as JSON
    #[clap(long)]
    json: bool,
    /// Don't truncate the lists in the report
    #[clap(long)]
    full: bool,
    /// Maximum number of changed glyphs to list (default 100)
    #[clap(long, default_value_t = 100)]
    limit: usize,
}

fn main() {
    let cli = Cli::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let font_a = DFont::new(&std::fs::read(&cli.font1).expect("Couldn't open font 1"));
    let font_b = DFont::new(&std::fs::read(&cli.font2).expect("Couldn't open font 2"));

    let start = std::time::Instant::now();
    let signature = diffenator3_lib::staticdiff::compute_signature(&font_a, &font_b);
    let elapsed = start.elapsed();

    if cli.json {
        let json = signature_to_json(&font_a, &font_b, &signature);
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    } else {
        report_text(&font_a, &font_b, &signature, elapsed, &cli);
    }
}

/// The user-space string for a location, from whichever font understands all
/// of the location's axes (falling back to font A).
fn best_font_user(font_a: &DFont, font_b: &DFont, loc: &NormalizedLocation) -> String {
    for font in [font_a, font_b] {
        let tags: Vec<_> = font
            .fontref()
            .axes()
            .iter()
            .map(|axis| axis.tag())
            .collect();
        if loc.iter().all(|(tag, _)| tags.contains(tag)) {
            let s = font.location_to_user(loc);
            if !s.is_empty() {
                return s;
            }
        }
    }
    font_a.location_to_user(loc)
}

/// A location rendered for display: normalized coords (unambiguous), plus the
/// user-space string. The two fonts can normalize differently, so a single
/// user-space string may correspond to several distinct normalized locations.
fn location_user(font_a: &DFont, font_b: &DFont, loc: &NormalizedLocation) -> String {
    let norm: Vec<String> = loc
        .iter()
        .map(|(tag, coord)| format!("{tag}={}", coord.to_f64()))
        .collect();
    let norm = norm.join(",");
    if norm.is_empty() {
        return "default".to_string();
    }
    let user = best_font_user(font_a, font_b, loc);
    format!("{norm} (user {user})")
}

/// A location as JSON: normalized coords plus the user-space label.
fn location_to_json(font_a: &DFont, font_b: &DFont, loc: &NormalizedLocation) -> Value {
    let mut normalized = Map::new();
    for (tag, coord) in loc.iter() {
        normalized.insert(tag.to_string(), json!(coord.to_f64()));
    }
    let user = best_font_user(font_a, font_b, loc);
    json!({
        "normalized": Value::Object(normalized),
        "user": if user.is_empty() { "default" } else { user.as_str() },
    })
}

fn glyph_report_json(report: &GlyphReport) -> Value {
    json!({
        "gid": report.gid.to_u32(),
        "name": report.name,
        "codepoint": report.codepoint.map(|c| format!("U+{:04X}", c)),
    })
}

fn matched_by_str(matched_by: &MatchMethod) -> &'static str {
    match matched_by {
        MatchMethod::Cmap(_) => "cmap",
        MatchMethod::DefaultOutline => "outline",
    }
}

/// Best-effort glyph name for a font A glyph.
fn gid_name(font: &DFont, gid: GlyphId) -> String {
    font.fontref()
        .glyph_names()
        .get(gid)
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("gid{}", gid.to_u32()))
}

/// JSON for a set of changed locations.
fn locations_json(font_a: &DFont, font_b: &DFont, locations: &LocationSet) -> Vec<Value> {
    locations
        .iter()
        .map(|loc| location_to_json(font_a, font_b, loc))
        .collect()
}

/// JSON for a set of changes keyed by a single font A glyph.
fn single_changes_json(
    font_a: &DFont,
    font_b: &DFont,
    changes: &FxHashMap<GlyphId, LocationSet>,
) -> Vec<Value> {
    let mut items: Vec<Value> = changes
        .iter()
        .map(|(gid, locations)| {
            json!({
                "gid": gid.to_u32(),
                "name": gid_name(font_a, *gid),
                "locations": locations_json(font_a, font_b, locations),
            })
        })
        .collect();
    items.sort_by(|a, b| a["gid"].as_u64().cmp(&b["gid"].as_u64()));
    items
}

/// JSON for a set of changes keyed by a font A glyph pair.
fn pair_changes_json(
    font_a: &DFont,
    font_b: &DFont,
    changes: &FxHashMap<(GlyphId, GlyphId), LocationSet>,
) -> Vec<Value> {
    let mut items: Vec<Value> = changes
        .iter()
        .map(|((left, right), locations)| {
            json!({
                "left_gid": left.to_u32(),
                "left_name": gid_name(font_a, *left),
                "right_gid": right.to_u32(),
                "right_name": gid_name(font_a, *right),
                "locations": locations_json(font_a, font_b, locations),
            })
        })
        .collect();
    items.sort_by(|a, b| {
        (a["left_gid"].as_u64(), a["right_gid"].as_u64())
            .cmp(&(b["left_gid"].as_u64(), b["right_gid"].as_u64()))
    });
    items
}

fn signature_to_json(font_a: &DFont, font_b: &DFont, signature: &DifferenceSignature) -> Value {
    let mut changes: Vec<Value> = signature
        .glyph_changes
        .iter()
        .map(|(gid_a, change)| {
            json!({
                "codepoint": change.codepoint.map(|c| format!("U+{:04X}", c)),
                "name": change.name,
                "gid_a": gid_a.to_u32(),
                "gid_b": change.gid_b.to_u32(),
                "matched_by": matched_by_str(&change.matched_by),
                "outline_locations": change.outline_locations.iter()
                    .map(|loc| location_to_json(font_a, font_b, loc)).collect::<Vec<_>>(),
                "advance_locations": change.advance_locations.iter()
                    .map(|loc| location_to_json(font_a, font_b, loc)).collect::<Vec<_>>(),
                "changed_locations": change.changed_locations().iter()
                    .map(|loc| location_to_json(font_a, font_b, loc)).collect::<Vec<_>>(),
            })
        })
        .collect();
    changes.sort_by(|a, b| a["gid_a"].as_u64().cmp(&b["gid_a"].as_u64()));

    let axes = |font: &DFont| -> Vec<Value> {
        let mut axes: Vec<(String, (f32, f32, f32))> = font.axis_info().into_iter().collect();
        axes.sort_by(|a, b| a.0.cmp(&b.0));
        axes
            .into_iter()
            .map(|(tag, (min, dflt, max))| {
                json!({ "tag": tag, "min": min, "default": dflt, "max": max })
            })
            .collect()
    };

    json!({
        "font_a": {
            "family": font_a.family_name(),
            "style": font_a.style_name(),
            "axes": axes(font_a),
        },
        "font_b": {
            "family": font_b.family_name(),
            "style": font_b.style_name(),
            "axes": axes(font_b),
        },
        "summary": {
            "matched": signature.unchanged_count + signature.glyph_changes.len(),
            "unchanged": signature.unchanged_count,
            "changed": signature.glyph_changes.len(),
            "missing": signature.missing.len(),
            "new": signature.new.len(),
            "uncertain": signature.uncertain.len(),
            "single_position_changes": signature.single_position_changes.len(),
            "pair_position_changes": signature.pair_position_changes.len(),
            "mark_position_changes": signature.mark_position_changes.len(),
            "cursive_position_changes": signature.cursive_position_changes.len(),
            "positioning_unmodelled": signature.positioning_unmodelled,
            "mapping_failed": signature.mapping_failed,
            "needs_testing": signature.needs_testing(),
            "identical": signature.is_identical(),
        },
        "glyph_changes": Value::Array(changes),
        "single_position_changes": Value::Array(single_changes_json(
            font_a,
            font_b,
            &signature.single_position_changes,
        )),
        "pair_position_changes": Value::Array(pair_changes_json(
            font_a,
            font_b,
            &signature.pair_position_changes,
        )),
        "mark_position_changes": Value::Array(pair_changes_json(
            font_a,
            font_b,
            &signature.mark_position_changes,
        )),
        "cursive_position_changes": Value::Array(single_changes_json(
            font_a,
            font_b,
            &signature.cursive_position_changes,
        )),
        "positioning_unmodelled": signature.positioning_unmodelled,
        "missing": signature.missing.iter().map(glyph_report_json).collect::<Vec<_>>(),
        "new": signature.new.iter().map(glyph_report_json).collect::<Vec<_>>(),
        "uncertain": signature.uncertain.iter().map(glyph_report_json).collect::<Vec<_>>(),
    })
}

/// Print the positioning (GPOS) difference sections of the report.
fn report_positioning(font_a: &DFont, font_b: &DFont, signature: &DifferenceSignature, cli: &Cli) {
    let shown = |total: usize| {
        if cli.full {
            total
        } else {
            total.min(cli.limit)
        }
    };
    let mut printed = false;

    if signature.positioning_unmodelled {
        println!("\nNOTE: positioning analysis is partial and pruning must not be trusted:");
        println!("  - contextual/chain-contextual GPOS lookups are not modelled, and/or");
        println!("  - the pair/mark expansion budget was exceeded.");
    }

    if !signature.single_position_changes.is_empty() {
        printed = true;
        println!(
            "\nSingle adjustment (GPOS type 1) differences ({}):",
            signature.single_position_changes.len()
        );
        let mut items: Vec<(u32, &LocationSet)> = signature
            .single_position_changes
            .iter()
            .map(|(g, l)| (g.to_u32(), l))
            .collect();
        items.sort_by_key(|(g, _)| *g);
        for (gid, locations) in items.iter().take(shown(items.len())) {
            let locs = locations
                .iter()
                .map(|loc| location_user(font_a, font_b, loc))
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "  {} (gid {}) at: {}",
                gid_name(font_a, GlyphId::new(*gid)),
                gid,
                locs
            );
        }
        if items.len() > shown(items.len()) {
            println!("  ... and {} more", items.len() - shown(items.len()));
        }
    }

    if !signature.pair_position_changes.is_empty() {
        printed = true;
        println!(
            "\nPair adjustment (GPOS type 2) differences ({}):",
            signature.pair_position_changes.len()
        );
        let mut items: Vec<((u32, u32), &LocationSet)> = signature
            .pair_position_changes
            .iter()
            .map(|((l, r), ls)| ((l.to_u32(), r.to_u32()), ls))
            .collect();
        items.sort_by_key(|(p, _)| *p);
        for ((left, right), locations) in items.iter().take(shown(items.len())) {
            let locs = locations
                .iter()
                .map(|loc| location_user(font_a, font_b, loc))
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "  {}/{} (gid {}/{}) at: {}",
                gid_name(font_a, GlyphId::new(*left)),
                gid_name(font_a, GlyphId::new(*right)),
                left,
                right,
                locs
            );
        }
        if items.len() > shown(items.len()) {
            println!("  ... and {} more", items.len() - shown(items.len()));
        }
    }

    if !signature.mark_position_changes.is_empty() {
        printed = true;
        println!(
            "\nMark attachment (GPOS types 4/5/6) differences ({}):",
            signature.mark_position_changes.len()
        );
        let mut items: Vec<((u32, u32), &LocationSet)> = signature
            .mark_position_changes
            .iter()
            .map(|((l, r), ls)| ((l.to_u32(), r.to_u32()), ls))
            .collect();
        items.sort_by_key(|(p, _)| *p);
        for ((mark, base), locations) in items.iter().take(shown(items.len())) {
            let locs = locations
                .iter()
                .map(|loc| location_user(font_a, font_b, loc))
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "  {} (mark) on {} (base) at: {}",
                gid_name(font_a, GlyphId::new(*mark)),
                gid_name(font_a, GlyphId::new(*base)),
                locs
            );
        }
        if items.len() > shown(items.len()) {
            println!("  ... and {} more", items.len() - shown(items.len()));
        }
    }

    if !signature.cursive_position_changes.is_empty() {
        printed = true;
        println!(
            "\nCursive attachment (GPOS type 3) differences ({}):",
            signature.cursive_position_changes.len()
        );
        let mut items: Vec<(u32, &LocationSet)> = signature
            .cursive_position_changes
            .iter()
            .map(|(g, l)| (g.to_u32(), l))
            .collect();
        items.sort_by_key(|(g, _)| *g);
        for (gid, locations) in items.iter().take(shown(items.len())) {
            let locs = locations
                .iter()
                .map(|loc| location_user(font_a, font_b, loc))
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "  {} (gid {}) at: {}",
                gid_name(font_a, GlyphId::new(*gid)),
                gid,
                locs
            );
        }
        if items.len() > shown(items.len()) {
            println!("  ... and {} more", items.len() - shown(items.len()));
        }
    }

    if !printed && !signature.positioning_unmodelled {
        println!("\nNo positioning differences found.");
    }
}

fn report_text(
    font_a: &DFont,
    font_b: &DFont,
    signature: &DifferenceSignature,
    elapsed: std::time::Duration,
    cli: &Cli,
) {
    println!("== Static difference analysis ==");
    println!(
        "Font A: {} {} ({})",
        font_a.family_name(),
        font_a.style_name(),
        font_a.axis_info().len()
    );
    println!(
        "Font B: {} {} ({})",
        font_b.family_name(),
        font_b.style_name(),
        font_b.axis_info().len()
    );
    println!("Analysis completed in {:?}", elapsed);
    println!();

    println!("Axes:");
    for (label, font) in [("A", font_a), ("B", font_b)] {
        let mut axes: Vec<(String, (f32, f32, f32))> = font.axis_info().into_iter().collect();
        axes.sort_by(|a, b| a.0.cmp(&b.0));
        for (tag, (min, dflt, max)) in axes {
            println!("  {label} {tag}: {min} - {max} (default {dflt})");
        }
    }
    println!();

    if signature.mapping_failed {
        println!("WARNING: fonts share no codepoints; the analysis is unreliable.");
        println!();
    }

    let matched = signature.unchanged_count + signature.glyph_changes.len();
    println!("Summary:");
    println!("  matched glyphs:  {matched}");
    println!("    unchanged:     {}", signature.unchanged_count);
    println!("    changed:       {}", signature.glyph_changes.len());
    println!("  codepoints only in A: {}", signature.missing.len());
    println!("  codepoints only in B: {}", signature.new.len());
    println!("  uncertain (fallback): {}", signature.uncertain.len());
    println!();

    if signature.glyph_changes.is_empty() {
        println!("No changed glyphs found.");
    } else {
        println!("Changed glyphs:");
        let mut sorted: Vec<(u32, &GlyphChange)> = signature
            .glyph_changes
            .iter()
            .map(|(gid, change)| (gid.to_u32(), change))
            .collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let shown = if cli.full {
            sorted.len()
        } else {
            sorted.len().min(cli.limit)
        };
        for (gid_a, change) in sorted.iter().take(shown) {
            let cp = change
                .codepoint
                .map(|c| format!("U+{:04X} ", c))
                .unwrap_or_default();
            println!(
                "  {cp}{} (gid {} -> gid {}) [matched by {}]",
                change.name,
                gid_a,
                change.gid_b.to_u32(),
                matched_by_str(&change.matched_by)
            );
            if !change.outline_locations.is_empty() {
                let locs = change
                    .outline_locations
                    .iter()
                    .map(|loc| location_user(font_a, font_b, loc))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("    outline differs at: {locs}");
            }
            if !change.advance_locations.is_empty() {
                let locs = change
                    .advance_locations
                    .iter()
                    .map(|loc| location_user(font_a, font_b, loc))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("    advance differs at: {locs}");
            }
        }
        if shown < sorted.len() {
            println!(
                "  ... and {} more (use --full to list them all)",
                sorted.len() - shown
            );
        }
    }

    report_positioning(font_a, font_b, signature, cli);

    if !signature.missing.is_empty() {
        println!("\nCodepoints missing from B ({}):", signature.missing.len());
        for report in signature.missing.iter().take(20) {
            println!(
                "  U+{:04X} {} (gid {})",
                report.codepoint.unwrap_or_default(),
                report.name,
                report.gid.to_u32()
            );
        }
        if signature.missing.len() > 20 {
            println!("  ... and {} more", signature.missing.len() - 20);
        }
    }

    if !signature.new.is_empty() {
        println!("\nCodepoints new in B ({}):", signature.new.len());
        for report in signature.new.iter().take(20) {
            println!(
                "  U+{:04X} {} (gid {})",
                report.codepoint.unwrap_or_default(),
                report.name,
                report.gid.to_u32()
            );
        }
        if signature.new.len() > 20 {
            println!("  ... and {} more", signature.new.len() - 20);
        }
    }

    if !signature.uncertain.is_empty() {
        println!(
            "\nUncertain -- need exhaustive fallback ({} glyphs):",
            signature.uncertain.len()
        );
        println!("  (unencoded glyphs -- ligatures, alternates, components -- cannot be");
        println!("   matched statically yet; GSUB-aware matching is planned.)");
        for report in signature.uncertain.iter().take(20) {
            println!("  gid {} {}", report.gid.to_u32(), report.name);
        }
        if signature.uncertain.len() > 20 {
            println!("  ... and {} more", signature.uncertain.len() - 20);
        }
    }
    println!();
}
