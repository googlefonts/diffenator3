use super::variable_scalars::SerializeValueRecordLike;
use super::SerializeSubtable;
use crate::context::SerializationContext;
use crate::monkeypatching::MonkeyPatchClassDef;
use read_fonts::tables::gpos::CursivePosFormat1;
use read_fonts::tables::gpos::MarkBasePosFormat1;
use read_fonts::tables::gpos::MarkLigPosFormat1;
use read_fonts::tables::gpos::MarkMarkPosFormat1;
use read_fonts::tables::gpos::PairPos;
use read_fonts::tables::gpos::SinglePos;
use read_fonts::ReadError;
use serde_json::json;
use serde_json::Map;
use serde_json::Value;

impl SerializeSubtable for SinglePos<'_> {
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "single".into());
        let coverage = match self {
            SinglePos::Format1(s) => s.coverage()?,
            SinglePos::Format2(s) => s.coverage()?,
        };
        match self {
            SinglePos::Format1(s) => {
                let value = s.value_record().serialize(self.offset_data(), context)?;
                for glyph in coverage.iter() {
                    let name = context.names.get(glyph);
                    map.insert(name, value.clone());
                }
            }
            SinglePos::Format2(s) => {
                for (vr, glyph) in s.value_records().iter().flatten().zip(coverage.iter()) {
                    let name = context.names.get(glyph);
                    map.insert(name, vr.serialize(self.offset_data(), context)?);
                }
            }
        }
        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for PairPos<'_> {
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "pair".into());
        match self {
            PairPos::Format1(s) => {
                for (left_glyph, pairs) in s.coverage()?.iter().zip(s.pair_sets().iter()) {
                    let left_name = context.names.get(left_glyph);
                    let pairs = pairs?;
                    for pair in pairs.pair_value_records().iter() {
                        let pair = pair?;
                        let right_name = context.names.get(pair.second_glyph());
                        let value_1 = pair
                            .value_record1()
                            .serialize(pairs.offset_data(), context)?;
                        let value_2 = pair
                            .value_record2()
                            .serialize(pairs.offset_data(), context)?;
                        let pair_value =
                            if value_2.as_object().map(|x| x.is_empty()).unwrap_or(false) {
                                value_1
                            } else {
                                json!({
                                    "first": value_1,
                                    "second": value_2
                                })
                            };
                        map.entry(left_name.clone())
                            .or_insert_with(|| Value::Object(Map::new()))
                            .as_object_mut()
                            .unwrap()
                            .insert(right_name, pair_value);
                    }
                }
            }
            PairPos::Format2(s) => {
                let class1 = s.class_def1()?;
                let class2 = s.class_def2()?;
                let mut classes = Map::new();
                let mut kerns = Map::new();
                for left_class in 0..s.class1_count() {
                    let mut left_class_glyphs =
                        class1.class_glyphs(left_class, Some(s.coverage()?));
                    left_class_glyphs.sort();
                    classes.insert(
                        format!("@CLASS_L_{}", left_class),
                        Value::Array(
                            left_class_glyphs
                                .into_iter()
                                .map(|gid| Value::String(context.names.get(gid)))
                                .collect(),
                        ),
                    );
                }
                for right_class in 1..s.class2_count() {
                    let mut right_class_glyphs = class2.class_glyphs(right_class, None);
                    right_class_glyphs.sort();
                    classes.insert(
                        format!("@CLASS_R_{}", right_class),
                        Value::Array(
                            right_class_glyphs
                                .into_iter()
                                .map(|gid| Value::String(context.names.get(gid)))
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
                            .serialize(s.offset_data(), context)?;
                        let value_2 = class2_record
                            .value_record2()
                            .serialize(s.offset_data(), context)?;
                        let pair_value =
                            if value_2.as_object().map(|x| x.is_empty()).unwrap_or(false) {
                                value_1
                            } else {
                                json!({
                                    "first": value_1,
                                    "second": value_2
                                })
                            };
                        kerns
                            .entry(left_name.clone())
                            .or_insert_with(|| Value::Object(Map::new()))
                            .as_object_mut()
                            .unwrap()
                            .insert(right_name, pair_value);
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
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "cursive".into());
        for (glyph_id, record) in self.coverage()?.iter().zip(self.entry_exit_record()) {
            let name = context.names.get(glyph_id);
            let mut entrymap = Map::new();
            if let Some(entry) = record
                .entry_anchor(self.offset_data())
                .map(|a| a?.serialize(self.offset_data(), context))
                .transpose()?
            {
                entrymap.insert("entry".to_string(), entry);
            }
            if let Some(exit) = record
                .exit_anchor(self.offset_data())
                .map(|a| a?.serialize(self.offset_data(), context))
                .transpose()?
            {
                entrymap.insert("exit".to_string(), exit);
            }
            map.insert(name, Value::Object(entrymap));
        }

        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for MarkBasePosFormat1<'_> {
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_base".into());
        let mark_array = self.mark_array()?;
        let mut marks = Map::new();
        for (mark_glyph, mark_record) in self.mark_coverage()?.iter().zip(mark_array.mark_records())
        {
            let mark_name = context.names.get(mark_glyph);
            let class_name = format!("anchor_{}", mark_record.mark_class());
            // "marks": {
            //    "anchor_0": {
            //         "glyph": "anchor",
            //         "glyph": "anchor",
            //    }, ...
            // }
            marks
                .entry(class_name)
                .or_insert_with(|| Map::new().into())
                .as_object_mut()
                .unwrap()
                .insert(
                    mark_name,
                    mark_record
                        .mark_anchor(mark_array.offset_data())?
                        .serialize(self.offset_data(), context)?,
                );
        }
        map.insert("marks".to_string(), marks.into());

        let mut bases = Map::new();
        let base_array = self.base_array()?;
        for (base_glyph, base_record) in self
            .base_coverage()?
            .iter()
            .zip(base_array.base_records().iter().flatten())
        {
            let mut anchors = Map::new();
            for (class, anchor) in base_record
                .base_anchors(base_array.offset_data())
                .iter()
                .enumerate()
            {
                if let Some(Ok(anchor)) = anchor {
                    anchors.insert(
                        format!("anchor_{}", class),
                        anchor.serialize(self.offset_data(), context)?,
                    );
                }
            }

            let base_name = context.names.get(base_glyph);
            bases.insert(base_name, anchors.into());
        }
        map.insert("bases".to_string(), bases.into());

        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for MarkLigPosFormat1<'_> {
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_lig".into());
        let mark_array = self.mark_array()?;
        let mut marks = Map::new();
        for (mark_glyph, mark_record) in self.mark_coverage()?.iter().zip(mark_array.mark_records())
        {
            let mark_name = context.names.get(mark_glyph);
            let class_name = format!("anchor_{}", mark_record.mark_class());
            // "marks": {
            //    "anchor_0": {
            //         "glyph": "anchor",
            //         "glyph": "anchor",
            //    }, ...
            // }
            marks
                .entry(class_name)
                .or_insert_with(|| Map::new().into())
                .as_object_mut()
                .unwrap()
                .insert(
                    mark_name,
                    mark_record
                        .mark_anchor(mark_array.offset_data())?
                        .serialize(self.offset_data(), context)?,
                );
        }
        map.insert("marks".to_string(), marks.into());

        let mut ligatures = Map::new();
        let ligature_array = self.ligature_array()?;
        for (ligature_glyph, ligature_attach_record) in self
            .ligature_coverage()?
            .iter()
            .zip(ligature_array.ligature_attaches().iter().flatten())
        {
            let mut anchors = Map::new();
            for (component_id, component_record) in ligature_attach_record
                .component_records()
                .iter()
                .flatten()
                .enumerate()
            {
                for (class, anchor) in component_record
                    .ligature_anchors(ligature_attach_record.offset_data())
                    .iter()
                    .enumerate()
                {
                    if let Some(Ok(anchor)) = anchor {
                        anchors.insert(
                            format!("anchor_{}_{}", class, component_id + 1),
                            anchor.serialize(self.offset_data(), context)?,
                        );
                    }
                }
            }

            let ligature_name = context.names.get(ligature_glyph);
            ligatures.insert(ligature_name, anchors.into());
        }
        map.insert("ligatures".to_string(), ligatures.into());

        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for MarkMarkPosFormat1<'_> {
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_mark".into());
        let mark_array = self.mark1_array()?;
        let mut marks = Map::new();
        for (mark_glyph, mark_record) in
            self.mark1_coverage()?.iter().zip(mark_array.mark_records())
        {
            let mark_name = context.names.get(mark_glyph);
            let class_name = format!("anchor_{}", mark_record.mark_class());
            // "marks": {
            //    "anchor_0": {
            //         "glyph": "anchor",
            //         "glyph": "anchor",
            //    }, ...
            // }
            marks
                .entry(class_name)
                .or_insert_with(|| Map::new().into())
                .as_object_mut()
                .unwrap()
                .insert(
                    mark_name,
                    mark_record
                        .mark_anchor(mark_array.offset_data())?
                        .serialize(self.offset_data(), context)?,
                );
        }
        map.insert("marks".to_string(), marks.into());

        let mut bases = Map::new();
        let base_array = self.mark2_array()?;
        for (base_glyph, base_record) in self
            .mark2_coverage()?
            .iter()
            .zip(base_array.mark2_records().iter().flatten())
        {
            let mut anchors = Map::new();
            for (class, anchor) in base_record
                .mark2_anchors(base_array.offset_data())
                .iter()
                .enumerate()
            {
                if let Some(Ok(anchor)) = anchor {
                    anchors.insert(
                        format!("anchor_{}", class),
                        anchor.serialize(self.offset_data(), context)?,
                    );
                }
            }

            let base_name = context.names.get(base_glyph);
            bases.insert(base_name, anchors.into());
        }
        map.insert("basemarks".to_string(), bases.into());

        Ok(Value::Object(map))
    }
}

fn wrap_in_map(v: Value) -> Map<String, Value> {
    if let Some(v) = v.as_object() {
        return v.clone();
    }
    let mut map = Map::new();
    map.insert("default".to_string(), v);
    map
}

fn insert_or_merge(m: &mut Map<String, Value>, key: String, new: Map<String, Value>) {
    if let Some(existing) = m.get_mut(&key).map(|x| x.as_object_mut().unwrap()) {
        // existing and new are both either {x: {loc: 123}, y: {loc: 456}} or {x: 123, y: 456}
        for (k, v) in new.into_iter() {
            let new_obj = wrap_in_map(v);
            let mut old_value = existing
                .get(&k)
                .map(|e| wrap_in_map(e.clone()))
                .unwrap_or_default();
            for (kk, vv) in new_obj.iter() {
                let old = old_value.get(kk).map(|x| x.as_i64().unwrap()).unwrap_or(0);
                old_value.insert(
                    kk.to_string(),
                    Value::Number((vv.as_i64().unwrap() + old).into()),
                );
            }
        }
    } else {
        m.insert(key, new.into());
    }
}
// Since we created the data structure, we're going to be unwrap()ping with gay abandon.
pub fn just_kerns(font: Value) -> Value {
    let mut flatkerns = Map::new();
    for lookup in font
        .get("GPOS")
        .and_then(|x| x.get("lookup_list"))
        .and_then(|x| x.as_object())
        .map(|x| x.values())
        .into_iter()
        .flatten()
        .flat_map(|x| x.as_array().unwrap().iter())
        .filter(|x| x.get("type").map(|x| x == "pair").unwrap_or(false))
        .map(|x| x.as_object().unwrap())
    {
        if lookup.contains_key("kerns") && lookup.contains_key("classes") {
            // Flatten class kerning
            let classes = lookup.get("classes").unwrap().as_object().unwrap();
            let kerns = lookup.get("kerns").unwrap().as_object().unwrap();
            for (left_class, value) in kerns.iter() {
                for (right_class, kern) in value.as_object().unwrap().iter() {
                    if kern == &json!({})
                        || kern == &json!({"x": 0})
                        || kern == &json!({"x": 0, "x_placement": 0})
                    {
                        continue;
                    }
                    let kern = kern.as_object().unwrap();
                    for left_glyph in classes
                        .get(left_class)
                        .unwrap_or(&json!(["@All"]))
                        .as_array()
                        .unwrap()
                        .iter()
                    {
                        // println!("left (class): {:#?} (ID {})", left_glyph, left_class);
                        // println!(
                        //     "right class: {:#?} ({:#?})",
                        //     right_class,
                        //     classes.get(right_class)
                        // );

                        for right_glyph in classes
                            .get(right_class)
                            .unwrap_or(&json!(["@All"]))
                            .as_array()
                            .unwrap()
                            .iter()
                        {
                            // println!("   right (class): {:#?}, kern: {:#?}", right_glyph, kern);
                            let key = left_glyph.as_str().unwrap().to_owned()
                                + "/"
                                + right_glyph.as_str().unwrap();
                            insert_or_merge(&mut flatkerns, key, kern.clone());
                        }
                    }
                }
            }
        } else {
            for (left, value_map) in lookup.iter() {
                // println!("left: {:#?}", left);
                if left == "type" {
                    continue;
                }
                for (right, value) in value_map.as_object().unwrap().iter() {
                    if value == &json!({"x": 0}) {
                        continue;
                    }
                    // println!(" right: {:#?}, value: {:?}", right, value);

                    let kern = value.as_object().unwrap();
                    let key = left.to_owned() + "/" + right.as_str();
                    insert_or_merge(&mut flatkerns, key, kern.clone());
                }
            }
        }
    }
    Value::Object(flatkerns)
}
