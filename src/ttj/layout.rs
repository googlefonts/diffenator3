mod gpos;
pub(crate) mod variable_scalars;

use read_fonts::tables::gpos::{PositionLookup, PositionSubtables};
use read_fonts::tables::gsub::{
    ChainedSequenceContext, FeatureList, SequenceContext, SubstitutionLookup, SubstitutionSubtables,
};
use read_fonts::tables::layout::{self};
use read_fonts::tables::varc::CoverageTable;
use read_fonts::{FontRead, FontRef, ReadError, TableProvider};
use serde_json::{Map, Value};
use skrifa::GlyphId16;

use super::namemap::NameMap;

pub(crate) fn serialize_gpos_table(font: &FontRef, names: &NameMap) -> Value {
    let mut map = Map::new();
    if let Ok(gpos) = font.gpos() {
        if let Ok(script_list) = gpos.script_list() {
            map.insert(
                "script_list".to_string(),
                Value::Object(serialize_script_list(&script_list, font)),
            );
        }
        if let Ok(feature_list) = gpos.feature_list() {
            map.insert(
                "feature_list".to_string(),
                Value::Object(serialize_feature_list(&feature_list, font)),
            );
        }
        if let Ok(lookup_list) = gpos.lookup_list() {
            map.insert(
                "lookup_list".to_string(),
                serialize_lookup_list(lookup_list, font, names),
            );
        }
    }
    Value::Object(map)
}

pub(crate) fn serialize_gsub_table(font: &FontRef, names: &NameMap) -> Value {
    let mut map = Map::new();
    if let Ok(gsub) = font.gsub() {
        if let Ok(script_list) = gsub.script_list() {
            map.insert(
                "script_list".to_string(),
                Value::Object(serialize_script_list(&script_list, font)),
            );
        }
        if let Ok(feature_list) = gsub.feature_list() {
            map.insert(
                "feature_list".to_string(),
                Value::Object(serialize_feature_list(&feature_list, font)),
            );
        }
        if let Ok(lookup_list) = gsub.lookup_list() {
            map.insert(
                "lookup_list".to_string(),
                serialize_lookup_list(lookup_list, font, names),
            );
        }
    }
    Value::Object(map)
}

fn serialize_feature_list(feature_list: &FeatureList, _font: &FontRef) -> Map<String, Value> {
    let offsets = feature_list.offset_data();
    let mut map = Map::new();
    for featurerec in feature_list.feature_records().iter() {
        if let Ok(feature) = featurerec.feature(offsets) {
            map.insert(
                featurerec.feature_tag().to_string(),
                serde_json::Value::Array(
                    feature
                        .lookup_list_indices()
                        .iter()
                        .map(|x| serde_json::Value::Number(x.get().into()))
                        .collect(),
                ),
            );
        }
    }
    map
}

fn serialize_script_list(
    script_list: &read_fonts::tables::gpos::ScriptList,
    font: &FontRef,
) -> Map<String, Value> {
    let offsets = script_list.offset_data();
    let mut map = Map::new();
    for scriptrec in script_list.script_records().iter() {
        if let Ok(script) = scriptrec.script(offsets) {
            let script_offsets = script.offset_data();
            if let Some(Ok(dflt)) = script.default_lang_sys() {
                map.insert(
                    format!("{}/dflt", scriptrec.script_tag()),
                    Value::Object(serialize_langsys(&dflt, font)),
                );
            }
            for langsysrecord in script.lang_sys_records().iter() {
                if let Ok(langsys) = langsysrecord.lang_sys(script_offsets) {
                    map.insert(
                        format!(
                            "{}/{}",
                            scriptrec.script_tag(),
                            langsysrecord.lang_sys_tag()
                        ),
                        Value::Object(serialize_langsys(&langsys, font)),
                    );
                }
            }
        }
    }
    map
}

fn serialize_langsys(
    langsys: &read_fonts::tables::layout::LangSys,
    _font: &FontRef,
) -> Map<String, Value> {
    let mut map = Map::new();
    let feature_index = langsys.required_feature_index();
    if feature_index != 65535 {
        map.insert(
            "required_feature_index".to_string(),
            Value::Number(feature_index.into()),
        );
    }
    map.insert(
        "lookups".to_string(),
        Value::Array(
            langsys
                .feature_indices()
                .iter()
                .map(|x| Value::Number(x.get().into()))
                .collect(),
        ),
    );
    map
}

fn serialize_lookup_list<'a, T: FontRead<'a> + SerializeLookup>(
    lookup_list: layout::LookupList<'a, T>,
    font: &FontRef,
    names: &NameMap,
) -> serde_json::Value {
    // I know it's an array, but when you're looking through it you want to know what index you're looking at.
    let mut arr = Map::new();
    for (ix, lookuprec) in lookup_list.lookups().iter().enumerate() {
        if let Ok(lookuprec) = lookuprec {
            arr.insert(format!("{}", ix), lookuprec.serialize_lookup(font, names));
        }
    }
    arr.into()
}

pub trait SerializeLookup {
    fn serialize_lookup(&self, font: &FontRef, names: &NameMap) -> Value;
}
pub trait SerializeSubtable {
    fn serialize_subtable(&self, font: &FontRef, names: &NameMap) -> Result<Value, ReadError>;
}

macro_rules! serialize_it {
    ($subtables: ident, $font: ident, $names: ident) => {
        $subtables
            .iter()
            .flatten()
            .map(|st| st.serialize_subtable($font, $names))
            .collect()
    };
}
impl SerializeLookup for PositionLookup<'_> {
    fn serialize_lookup(&self, font: &FontRef, names: &NameMap) -> Value {
        if let Ok(subtables) = self.subtables() {
            let serialized_tables: Vec<Result<Value, _>> = match subtables {
                PositionSubtables::Single(st) => serialize_it!(st, font, names),
                PositionSubtables::Pair(st) => serialize_it!(st, font, names),
                PositionSubtables::Cursive(st) => serialize_it!(st, font, names),
                PositionSubtables::MarkToBase(st) => serialize_it!(st, font, names),
                PositionSubtables::MarkToLig(st) => serialize_it!(st, font, names),
                PositionSubtables::MarkToMark(st) => serialize_it!(st, font, names),
                PositionSubtables::ChainContextual(st) => {
                    serialize_it!(st, font, names)
                }
                PositionSubtables::Contextual(st) => serialize_it!(st, font, names),
            };
            return Value::Array(
                serialized_tables
                    .into_iter()
                    .map(|x| x.unwrap_or_default())
                    .collect(),
            );
        }
        serde_json::Value::Null
    }
}

impl SerializeLookup for SubstitutionLookup<'_> {
    fn serialize_lookup(&self, font: &FontRef, names: &NameMap) -> Value {
        if let Ok(subtables) = self.subtables() {
            let serialized_tables: Vec<Result<Value, _>> = match subtables {
                // SubstitutionSubtables::Single(st) => serialize_it!(st, font, names),
                // SubstitutionSubtables::Multiple(st) => serialize_it!(st, font, names),
                // SubstitutionSubtables::Cursive(st) => serialize_it!(st, font, names),
                // SubstitutionSubtables::MarkToBase(st) => serialize_it!(st, font, names),
                // SubstitutionSubtables::MarkToLig(st) => serialize_it!(st, font, names),
                // SubstitutionSubtables::MarkToMark(st) => serialize_it!(st, font, names),
                SubstitutionSubtables::ChainContextual(st) => {
                    serialize_it!(st, font, names)
                }
                SubstitutionSubtables::Contextual(st) => serialize_it!(st, font, names),
                _ => vec![],
            };
            return Value::Array(
                serialized_tables
                    .into_iter()
                    .map(|x| x.unwrap_or_default())
                    .collect(),
            );
        }
        serde_json::Value::Null
    }
}

// These chained lookups are a pain in the neck, putting them into common structures will help.
struct Slot {
    glyphs: Vec<GlyphId16>,
    lookups: Vec<u16>,
}

impl From<GlyphId16> for Slot {
    fn from(glyph: GlyphId16) -> Self {
        Slot {
            glyphs: vec![glyph],
            lookups: vec![],
        }
    }
}
impl From<CoverageTable<'_>> for Slot {
    fn from(glyph: CoverageTable<'_>) -> Self {
        Slot {
            glyphs: glyph.iter().collect(),
            lookups: vec![],
        }
    }
}

impl Slot {
    fn as_string(&self, names: &NameMap, marked: bool) -> String {
        let mut result = String::new();
        if self.glyphs.len() > 1 {
            result.push('[');
            result.push_str(
                &self
                    .glyphs
                    .iter()
                    .map(|x| names.get(*x))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            result.push(']');
        } else {
            result.push_str(&names.get(self.glyphs[0]));
        }
        if !marked {
            return result;
        }
        result.push_str("' ");
        result.push_str(
            &self
                .lookups
                .iter()
                .map(|x| format!("lookup lookup_{}", x))
                .collect::<Vec<_>>()
                .join(" "),
        );
        result
    }
}

struct ChainRule {
    backtrack: Vec<Slot>,
    input: Vec<Slot>,
    lookahead: Vec<Slot>,
}

impl ChainRule {
    fn as_string(&self, names: &NameMap) -> String {
        let mut result = String::new();
        result.push_str(
            self.backtrack
                .iter()
                .map(|x| x.as_string(names, false))
                .collect::<Vec<_>>()
                .join(" ")
                .as_str(),
        );
        if !self.backtrack.is_empty() {
            result.push(' ');
        }
        result.push_str(
            self.input
                .iter()
                .map(|x| x.as_string(names, true))
                .collect::<Vec<_>>()
                .join(" ")
                .as_str(),
        );
        if !self.lookahead.is_empty() {
            result.push(' ');
        }
        result.push_str(
            self.lookahead
                .iter()
                .map(|x| x.as_string(names, false))
                .collect::<Vec<_>>()
                .join(" ")
                .as_str(),
        );

        result
    }
}
impl SerializeSubtable for SequenceContext<'_> {
    fn serialize_subtable(&self, _font: &FontRef, names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        let rules = match self {
            SequenceContext::Format1(f1) => serialize_sequence_f1(f1),
            // SequenceContext::Format2(f2) => serialize_sequence_f2(&f2),
            SequenceContext::Format3(f3) => serialize_sequence_f3(&f3),
            _ => Ok(vec![]),
        }?;
        map.insert("type".to_string(), "sequence_context".into());
        map.insert(
            "rules".to_string(),
            Value::Array(
                rules
                    .into_iter()
                    .map(|x| x.as_string(names).into())
                    .collect(),
            ),
        );
        Ok(Value::Object(map))
    }
}

fn serialize_sequence_f1(f1: &layout::SequenceContextFormat1) -> Result<Vec<ChainRule>, ReadError> {
    let mut rules = vec![];
    for (first_glyph, ruleset) in f1.coverage()?.iter().zip(f1.seq_rule_sets().iter()) {
        if let Some(Ok(ruleset)) = ruleset {
            for sequencerule in ruleset.seq_rules().iter().flatten() {
                let mut glyphs = vec![Slot::from(first_glyph)];
                glyphs.extend(
                    sequencerule
                        .input_sequence()
                        .iter()
                        .map(|x| Slot::from(x.get())),
                );
                let mut rule = ChainRule {
                    backtrack: vec![],
                    input: glyphs,
                    lookahead: vec![],
                };
                for lookup_record in sequencerule.seq_lookup_records() {
                    if let Some(slot) = rule.input.get_mut(lookup_record.sequence_index() as usize)
                    {
                        slot.lookups.push(lookup_record.lookup_list_index());
                    }
                }
                rules.push(rule);
            }
        }
    }
    Ok(rules)
}

fn serialize_sequence_f3(
    subtable: &layout::SequenceContextFormat3,
) -> Result<Vec<ChainRule>, ReadError> {
    let mut rules = vec![];
    let glyphs = subtable
        .coverages()
        .iter()
        .flatten()
        .map(Slot::from)
        .collect();
    let mut rule = ChainRule {
        backtrack: vec![],
        input: glyphs,
        lookahead: vec![],
    };
    for lookup_record in subtable.seq_lookup_records() {
        if let Some(slot) = rule.input.get_mut(lookup_record.sequence_index() as usize) {
            slot.lookups.push(lookup_record.lookup_list_index());
        }
    }
    rules.push(rule);
    Ok(rules)
}

impl SerializeSubtable for ChainedSequenceContext<'_> {
    fn serialize_subtable(&self, _font: &FontRef, names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        let rules = match self {
            ChainedSequenceContext::Format1(f1) => serialize_chain_sequence_f1(f1),
            // SequenceContext::Format2(f2) => serialize_chain_sequence_f2(&f2),
            ChainedSequenceContext::Format3(f3) => serialize_chain_sequence_f3(&f3),
            _ => Ok(vec![]),
        }?;
        map.insert("type".to_string(), "chained_sequence_context".into());
        map.insert(
            "rules".to_string(),
            Value::Array(
                rules
                    .into_iter()
                    .map(|x| x.as_string(names).into())
                    .collect(),
            ),
        );
        Ok(Value::Object(map))
    }
}

fn serialize_chain_sequence_f1(
    f1: &layout::ChainedSequenceContextFormat1,
) -> Result<Vec<ChainRule>, ReadError> {
    let mut rules = vec![];
    for (first_glyph, ruleset) in f1.coverage()?.iter().zip(f1.chained_seq_rule_sets().iter()) {
        if let Some(Ok(ruleset)) = ruleset {
            for sequencerule in ruleset.chained_seq_rules().iter().flatten() {
                let mut glyphs = vec![Slot::from(first_glyph)];
                glyphs.extend(
                    sequencerule
                        .input_sequence()
                        .iter()
                        .map(|x| Slot::from(x.get())),
                );
                let backtrack = sequencerule
                    .backtrack_sequence()
                    .iter()
                    .map(|x| Slot::from(x.get()))
                    .collect();
                let lookahead = sequencerule
                    .lookahead_sequence()
                    .iter()
                    .map(|x| Slot::from(x.get()))
                    .collect();
                let mut rule = ChainRule {
                    backtrack,
                    input: glyphs,
                    lookahead,
                };
                for lookup_record in sequencerule.seq_lookup_records() {
                    if let Some(slot) = rule.input.get_mut(lookup_record.sequence_index() as usize)
                    {
                        slot.lookups.push(lookup_record.lookup_list_index());
                    }
                }
                rules.push(rule);
            }
        }
    }
    Ok(rules)
}

fn serialize_chain_sequence_f3(
    subtable: &layout::ChainedSequenceContextFormat3,
) -> Result<Vec<ChainRule>, ReadError> {
    let mut rules = vec![];
    let glyphs = subtable
        .input_coverages()
        .iter()
        .flatten()
        .map(Slot::from)
        .collect();
    let backtrack = subtable
        .backtrack_coverages()
        .iter()
        .flatten()
        .map(Slot::from)
        .collect();
    let lookahead = subtable
        .lookahead_coverages()
        .iter()
        .flatten()
        .map(Slot::from)
        .collect();
    let mut rule = ChainRule {
        backtrack,
        input: glyphs,
        lookahead,
    };
    for lookup_record in subtable.seq_lookup_records() {
        if let Some(slot) = rule.input.get_mut(lookup_record.sequence_index() as usize) {
            slot.lookups.push(lookup_record.lookup_list_index());
        }
    }
    rules.push(rule);
    Ok(rules)
}
