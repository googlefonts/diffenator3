mod gpos;
pub(crate) mod variable_scalars;

use read_fonts::tables::gpos::{PositionLookup, PositionSubtables};
use read_fonts::tables::gsub::{
    ChainedSequenceContext, FeatureList, SequenceContext, SubstitutionLookup,
};
use read_fonts::tables::layout::{self};
use read_fonts::{FontRead, FontRef, ReadError, TableProvider};
use serde_json::{Map, Value};

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
    fn serialize_lookup(&self, _font: &FontRef, _names: &NameMap) -> Value {
        let map = Map::new();
        Value::Object(map)
    }
}

impl SerializeSubtable for SequenceContext<'_> {
    fn serialize_subtable(&self, _font: &FontRef, _names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "sequence_context".into());
        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for ChainedSequenceContext<'_> {
    fn serialize_subtable(&self, _font: &FontRef, _names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "chained_sequence_context".into());
        Ok(Value::Object(map))
    }
}
