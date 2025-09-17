use std::collections::BTreeMap;

use diffenator3_lib::dfont::DFont;
use serde_json::json;
use shaperglot::{Checker, Languages, SupportLevel};

fn support_label(level: &SupportLevel) -> &'static str {
    match level {
        SupportLevel::Complete => "Complete",
        SupportLevel::Supported => "Supported",
        SupportLevel::Incomplete => "Incomplete",
        SupportLevel::Unsupported => "Unsupported",
        SupportLevel::None => "None",
        SupportLevel::Indeterminate => "Indeterminate",
    }
}

pub fn diff_languages(font_a: &DFont, font_b: &DFont) -> serde_json::Value {
    let checker_a = Checker::new(&font_a.backing).expect("Failed to load font");
    let checker_b = Checker::new(&font_b.backing).expect("Failed to load font");
    let languages = Languages::new();
    let mut supported: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for language in languages.iter() {
        log::debug!("Checking language: {}", language.name());
        let results_a = checker_a.check(language);
        let results_b = checker_b.check(language);
        supported.insert(
            language.name().to_string(),
            json!({
                "level_a": support_label(&results_a.support_level()).to_string(),
                "score_a":  results_a.score(),
                "fixes_a": results_a.fixes_required(),
                "level_b": support_label(&results_b.support_level()).to_string(),
                "score_b":  results_b.score(),
                "fixes_b": results_b.fixes_required(),
            }),
        );
    }
    serde_json::to_value(&supported).expect("Failed to serialize language support")
}
