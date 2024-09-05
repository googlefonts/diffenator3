use read_fonts::tables::fvar::VariationAxisRecord;
use read_fonts::tables::gpos::DeviceOrVariationIndex::VariationIndex;
use read_fonts::tables::gpos::{
    CursivePosFormat1, MarkBasePosFormat1, MarkLigPosFormat1, MarkMarkPosFormat1, PairPos,
    PositionLookup, PositionSubtables, SinglePos,
};
use read_fonts::tables::gsub::{FeatureList, SubstitutionLookup};
use read_fonts::tables::layout::{self};
use read_fonts::{FontData, FontRead, FontRef, ReadError, TableProvider};
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
impl SerializeLookup for PositionLookup<'_> {
    fn serialize_lookup(&self, font: &FontRef, names: &NameMap) -> Value {
        if let Ok(subtables) = self.subtables() {
            let serialized_tables = match subtables {
                PositionSubtables::Single(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font, names))
                    .collect(),
                PositionSubtables::Pair(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font, names))
                    .collect(),
                PositionSubtables::Cursive(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font, names))
                    .collect(),
                PositionSubtables::MarkToBase(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font, names))
                    .collect(),
                PositionSubtables::MarkToLig(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font, names))
                    .collect(),
                PositionSubtables::MarkToMark(subtables) => subtables
                    .iter()
                    .flatten()
                    .map(|st| st.serialize_subtable(font, names))
                    .collect(),

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

impl SerializeLookup for SubstitutionLookup<'_> {
    fn serialize_lookup(&self, _font: &FontRef, _names: &NameMap) -> Value {
        let map = Map::new();
        Value::Object(map)
    }
}

impl SerializeSubtable for SinglePos<'_> {
    fn serialize_subtable(&self, font: &FontRef, names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "single".into());
        let coverage = match self {
            SinglePos::Format1(s) => s.coverage()?,
            SinglePos::Format2(s) => s.coverage()?,
        };
        match self {
            SinglePos::Format1(s) => {
                let value: String = s.value_record().serialize(self.offset_data(), font)?;
                for glyph in coverage.iter() {
                    let name = names.get(glyph);
                    map.insert(name, Value::String(value.clone()));
                }
            }
            SinglePos::Format2(s) => {
                for (vr, glyph) in s.value_records().iter().flatten().zip(coverage.iter()) {
                    let name = names.get(glyph);
                    map.insert(name, Value::String(vr.serialize(self.offset_data(), font)?));
                }
            }
        }
        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for PairPos<'_> {
    fn serialize_subtable(&self, font: &FontRef, names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "pair".into());
        match self {
            PairPos::Format1(s) => {
                for (left_glyph, pairs) in s.coverage()?.iter().zip(s.pair_sets().iter()) {
                    let left_name = names.get(left_glyph);
                    let pairs = pairs?;
                    for pair in pairs.pair_value_records().iter() {
                        let pair = pair?;
                        let right_name = names.get(pair.second_glyph());
                        let value_1 = pair.value_record1().serialize(pairs.offset_data(), font)?;
                        let value_2 = pair.value_record2().serialize(pairs.offset_data(), font)?;
                        map.entry(left_name.clone())
                            .or_insert_with(|| Value::Object(Map::new()))
                            .as_object_mut()
                            .unwrap()
                            .insert(
                                right_name,
                                Value::String(
                                    format!("{} {}", value_1, value_2).trim().to_string(),
                                ),
                            );
                    }
                }
            }
            PairPos::Format2(s) => {
                let class1 = s.class_def1()?;
                let class2 = s.class_def2()?;
                let mut classes = Map::new();
                let mut kerns = Map::new();
                for left_class in 0..s.class1_count() {
                    let left_class_glyphs: &mut dyn Iterator<Item = GlyphId16> = if left_class == 0
                    {
                        // use the coverage
                        &mut (s.coverage()?.iter())
                    } else {
                        &mut (class1
                            .iter()
                            .filter(|&(_gid, class)| class == left_class)
                            .map(|(gid, _)| gid))
                    };
                    classes.insert(
                        format!("@CLASS_L_{}", left_class),
                        Value::Array(
                            left_class_glyphs
                                .map(|gid| Value::String(names.get(gid)))
                                .collect(),
                        ),
                    );
                }
                for right_class in 1..s.class2_count() {
                    let right_class_glyphs = class2
                        .iter()
                        .filter(|&(_gid, class)| class == right_class)
                        .map(|(gid, _)| gid);
                    classes.insert(
                        format!("@CLASS_R_{}", right_class),
                        Value::Array(
                            right_class_glyphs
                                .map(|gid| Value::String(names.get(gid)))
                                .collect(),
                        ),
                    );
                }
                for (left_class, class1_record) in s.class1_records().iter().enumerate() {
                    let class1_record = class1_record?;
                    for (right_class, class2_record) in
                        class1_record.class2_records().iter().enumerate()
                    {
                        let class2_record = class2_record?;
                        let left_name = format!("@CLASS_L_{}", left_class);
                        let right_name = if right_class == 0 {
                            "@All".to_string()
                        } else {
                            format!("@CLASS_R_{}", right_class)
                        };
                        let value_1 = class2_record
                            .value_record1()
                            .serialize(s.offset_data(), font)?;
                        let value_2 = class2_record
                            .value_record2()
                            .serialize(s.offset_data(), font)?;
                        let valuerecords = format!("{} {}", value_1, value_2);
                        if valuerecords == "0 " && (left_class == 0 || right_class == 0) {
                            continue;
                        }
                        kerns
                            .entry(left_name.clone())
                            .or_insert_with(|| Value::Object(Map::new()))
                            .as_object_mut()
                            .unwrap()
                            .insert(right_name, Value::String(valuerecords.trim().to_string()));
                    }
                }
                map.insert("classes".to_string(), classes.into());
                map.insert("kerns".to_string(), kerns.into());
            }
        }
        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for CursivePosFormat1<'_> {
    fn serialize_subtable(&self, font: &FontRef, names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "cursive".into());
        for (glyph_id, record) in self.coverage()?.iter().zip(self.entry_exit_record()) {
            let name = names.get(glyph_id);
            let entry = record
                .entry_anchor(self.offset_data())
                .map(|a| a?.serialize(self.offset_data(), font))
                .transpose()?;
            let exit = record
                .exit_anchor(self.offset_data())
                .map(|a| a?.serialize(self.offset_data(), font))
                .transpose()?;
            map.insert(
                name,
                Value::Object(
                    vec![
                        (
                            "entry".to_string(),
                            Value::String(entry.unwrap_or("NONE".to_string())),
                        ),
                        (
                            "exit".to_string(),
                            Value::String(exit.unwrap_or("NONE".to_string())),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
            );
        }

        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for MarkBasePosFormat1<'_> {
    fn serialize_subtable(&self, _font: &FontRef, _names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_base".into());
        map.insert(
            "format".to_string(),
            Value::Number(self.pos_format().into()),
        );

        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for MarkLigPosFormat1<'_> {
    fn serialize_subtable(&self, _font: &FontRef, _names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_lig".into());
        map.insert(
            "format".to_string(),
            Value::Number(self.pos_format().into()),
        );

        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for MarkMarkPosFormat1<'_> {
    fn serialize_subtable(&self, _font: &FontRef, _names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_Mark".into());
        map.insert(
            "format".to_string(),
            Value::Number(self.pos_format().into()),
        );

        Ok(Value::Object(map))
    }
}

trait SerializeValueRecordLike {
    fn serialize(&self, offset_data: FontData<'_>, font: &FontRef) -> Result<String, ReadError>;
}

impl SerializeValueRecordLike for read_fonts::tables::gpos::ValueRecord {
    fn serialize(&self, offset_data: FontData<'_>, font: &FontRef) -> Result<String, ReadError> {
        let mut vr = String::new();
        if let Some(x) = self.x_advance() {
            if let Some(Ok(VariationIndex(device))) = self.x_advance_device(offset_data) {
                vr.push_str(&serialize_all_deltas(device, font, x.into())?)
            } else {
                vr.push_str(&format!("{}", x));
            }
        } else if self.y_advance().is_some() {
            vr.push('0');
        }

        if let Some(y) = self.y_advance() {
            vr.push(',');
            if let Some(Ok(VariationIndex(device))) = self.y_advance_device(offset_data) {
                vr.push_str(&serialize_all_deltas(device, font, y.into())?)
            } else {
                vr.push_str(&format!("{}", y));
            }
        }

        if self.x_placement().is_none() && self.y_placement().is_none() {
            return Ok(vr);
        }
        vr.push('@');
        if let Some(x) = self.x_placement() {
            if let Some(Ok(VariationIndex(device))) = self.x_placement_device(offset_data) {
                vr.push_str(&serialize_all_deltas(device, font, x.into())?)
            } else {
                vr.push_str(&format!("{}", x));
            }
        } else {
            vr.push('0');
        }
        vr.push(',');
        if let Some(y) = self.y_placement() {
            if let Some(Ok(VariationIndex(device))) = self.y_placement_device(offset_data) {
                vr.push_str(&serialize_all_deltas(device, font, y.into())?)
            } else {
                vr.push_str(&format!("{}", y));
            }
        } else {
            vr.push('0');
        }
        Ok(vr)
    }
}

impl SerializeValueRecordLike for read_fonts::tables::gpos::AnchorTable<'_> {
    fn serialize(&self, _offset_data: FontData<'_>, font: &FontRef) -> Result<String, ReadError> {
        let mut vr = String::new();
        let x = self.x_coordinate();
        if let Some(Ok(VariationIndex(device))) = self.x_device() {
            vr.push_str(&serialize_all_deltas(device, font, x.into())?)
        } else {
            vr.push_str(&format!("{}", x));
        }
        vr.push(',');
        let y = self.y_coordinate();
        if let Some(Ok(VariationIndex(device))) = self.y_device() {
            vr.push_str(&serialize_all_deltas(device, font, y.into())?)
        } else {
            vr.push_str(&format!("{}", y));
        }

        Ok(vr)
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn poor_mans_denormalize(peak: f32, axis: &VariationAxisRecord) -> f32 {
    // Insert avar here
    if peak > 0.0 {
        lerp(
            axis.default_value().to_f32(),
            axis.max_value().to_f32(),
            peak,
        )
    } else {
        lerp(
            axis.default_value().to_f32(),
            axis.min_value().to_f32(),
            -peak,
        )
    }
}

pub(crate) fn serialize_all_deltas(
    device: read_fonts::tables::layout::VariationIndex,
    font: &FontRef,
    current: i32,
) -> Result<String, ReadError> {
    let axes = font.fvar()?.axes()?;
    if let Some(Ok(ivs)) = font.gdef()?.item_var_store() {
        let regions = ivs.variation_region_list()?.variation_regions();
        // Let's turn these back to userspace
        let locations: Vec<String> = regions
            .iter()
            .flatten()
            .map(|r| {
                let location: Vec<String> = r
                    .region_axes()
                    .iter()
                    .enumerate()
                    .map(|(axis_ix, tuple)| {
                        let axis = axes.get(axis_ix).unwrap();
                        let peak = tuple.peak_coord().to_f32();
                        (axis, peak)
                    })
                    .filter(|&(_axis, peak)| peak != 0.0)
                    .map(|(axis, peak)| {
                        let value = poor_mans_denormalize(peak, axis);
                        format!("{}={}", axis.axis_tag(), value)
                    })
                    .collect();
                location.join(",")
            })
            .collect();

        if let Some(outer) = ivs
            .item_variation_data()
            .get(device.delta_set_outer_index() as usize)
            .transpose()?
        {
            let affected_regions = outer.region_indexes();
            let inner = outer.delta_set(device.delta_set_inner_index());
            let mut deltas = vec![format!("{}", current)];
            for (delta, region_index) in inner.zip(affected_regions.iter()) {
                if delta == 0 {
                    continue;
                }
                deltas.push(format!(
                    "{}@{}",
                    current + delta,
                    locations
                        .get(region_index.get() as usize)
                        .map(|x| x.as_str())
                        .unwrap_or("Unknown")
                ));
            }
            return Ok(deltas.join(" "));
        }
    }
    Ok(format!("{}", current))
}
