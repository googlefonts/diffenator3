use read_fonts::tables::gpos::{
    self, CursivePosFormat1, MarkBasePosFormat1, MarkLigPosFormat1, MarkMarkPosFormat1, PairPos,
    PositionLookup, PositionSubtables, SinglePos,
};
// use super::gid_to_name;
use read_fonts::tables::gsub::{FeatureList, SubstitutionLookup};
use read_fonts::tables::layout;
use read_fonts::{FontRead, FontRef, TableProvider};
use serde_json::{Map, Value};
use skrifa::GlyphId;

use super::gid_to_name;

pub(crate) fn serialize_gpos_table(font: &FontRef) -> Value {
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
                serialize_lookup_list(lookup_list, font),
            );
        }
    }
    Value::Object(map)
}

pub(crate) fn serialize_gsub_table(font: &FontRef) -> Value {
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
                serialize_lookup_list(lookup_list, font),
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
) -> serde_json::Value {
    let offsets = lookup_list.offset_data();
    let mut arr: Vec<Value> = vec![];
    for lookuprec in lookup_list.lookups().iter() {
        if let Ok(lookuprec) = lookuprec {
            arr.push(lookuprec.serialize_lookup(font));
        } else {
            arr.push(Value::Null);
        }
    }
    arr.into()
}

type GlyphNameMap = dyn Fn(GlyphId) -> String;

pub trait SerializeLookup {
    fn serialize_lookup(&self, font: &FontRef) -> Value;
}
pub trait SerializeSubtable {
    fn serialize_subtable(&self, font: &FontRef) -> Value;
}
impl SerializeLookup for PositionLookup<'_> {
    fn serialize_lookup(&self, font: &FontRef) -> Value {
        let mut map = Map::new();
        if let Ok(subtables) = self.subtables() {
            let serialized_tables = match subtables {
                PositionSubtables::Single(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font))
                    .collect(),
                PositionSubtables::Pair(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font))
                    .collect(),
                PositionSubtables::Cursive(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font))
                    .collect(),
                PositionSubtables::MarkToBase(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font))
                    .collect(),
                PositionSubtables::MarkToLig(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font))
                    .collect(),
                PositionSubtables::MarkToMark(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font))
                    .collect(),

                _ => vec![],
            };
            map.insert("subtables".to_string(), Value::Array(serialized_tables));
        }
        serde_json::Value::Object(map)
    }
}

impl SerializeLookup for SubstitutionLookup<'_> {
    fn serialize_lookup(&self, font: &FontRef) -> Value {
        let mut map = Map::new();
        Value::Object(map)
    }
}

impl SerializeSubtable for SinglePos<'_> {
    fn serialize_subtable(&self, font: &FontRef) -> Value {
        let mut map = Map::new();
        map.insert("type".to_string(), "single".into());
        if let Ok(coverage) = (match self {
            SinglePos::Format1(s) => s.coverage(),
            SinglePos::Format2(s) => s.coverage(),
        }) {
            let glyphs: Vec<Value> = coverage
                .iter()
                .map(|gid| gid_to_name(font, gid.into()).into())
                .collect();
            map.insert("glyphs".to_string(), Value::Array(glyphs));
        }
        Value::Object(map)
    }
}

impl SerializeSubtable for PairPos<'_> {
    fn serialize_subtable(&self, font: &FontRef) -> Value {
        let mut map = Map::new();
        map.insert("type".to_string(), "pair".into());
        map.insert(
            "format".to_string(),
            Value::Number(self.pos_format().into()),
        );
        if let Ok(coverage) = (match self {
            PairPos::Format1(s) => s.coverage(),
            PairPos::Format2(s) => s.coverage(),
        }) {
            let glyphs: Vec<Value> = coverage
                .iter()
                .map(|gid| gid_to_name(font, gid.into()).into())
                .collect();
            map.insert("glyphs".to_string(), Value::Array(glyphs));
        }

        Value::Object(map)
    }
}

impl SerializeSubtable for CursivePosFormat1<'_> {
    fn serialize_subtable(&self, font: &FontRef) -> Value {
        let mut map = Map::new();
        map.insert("type".to_string(), "cursive".into());
        map.insert(
            "format".to_string(),
            Value::Number(self.pos_format().into()),
        );

        Value::Object(map)
    }
}

impl SerializeSubtable for MarkBasePosFormat1<'_> {
    fn serialize_subtable(&self, font: &FontRef) -> Value {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_base".into());
        map.insert(
            "format".to_string(),
            Value::Number(self.pos_format().into()),
        );

        Value::Object(map)
    }
}

impl SerializeSubtable for MarkLigPosFormat1<'_> {
    fn serialize_subtable(&self, font: &FontRef) -> Value {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_lig".into());
        map.insert(
            "format".to_string(),
            Value::Number(self.pos_format().into()),
        );

        Value::Object(map)
    }
}

impl SerializeSubtable for MarkMarkPosFormat1<'_> {
    fn serialize_subtable(&self, font: &FontRef) -> Value {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_Mark".into());
        map.insert(
            "format".to_string(),
            Value::Number(self.pos_format().into()),
        );

        Value::Object(map)
    }
}
