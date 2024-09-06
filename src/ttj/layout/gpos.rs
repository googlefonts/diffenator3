use super::super::namemap::NameMap;
use super::variable_scalars::SerializeValueRecordLike;
use super::SerializeSubtable;
use read_fonts::tables::gpos::CursivePosFormat1;
use read_fonts::tables::gpos::MarkBasePosFormat1;
use read_fonts::tables::gpos::MarkLigPosFormat1;
use read_fonts::tables::gpos::MarkMarkPosFormat1;
use read_fonts::tables::gpos::PairPos;
use read_fonts::tables::gpos::SinglePos;
use read_fonts::ReadError;
use serde_json::Map;
use serde_json::Value;
use skrifa::FontRef;
use skrifa::GlyphId16;

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
            let mut entrymap = Map::new();
            if let Some(entry) = record
                .entry_anchor(self.offset_data())
                .map(|a| a?.serialize(self.offset_data(), font))
                .transpose()?
            {
                entrymap.insert("entry".to_string(), Value::String(entry));
            }
            if let Some(exit) = record
                .exit_anchor(self.offset_data())
                .map(|a| a?.serialize(self.offset_data(), font))
                .transpose()?
            {
                entrymap.insert("exit".to_string(), Value::String(exit));
            }
            map.insert(name, Value::Object(entrymap));
        }

        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for MarkBasePosFormat1<'_> {
    fn serialize_subtable(&self, font: &FontRef, names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_base".into());
        let mark_array = self.mark_array()?;
        let mut marks = Map::new();
        for (mark_glyph, mark_record) in self.mark_coverage()?.iter().zip(mark_array.mark_records())
        {
            let mark_name = names.get(mark_glyph);
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
                        .serialize(self.offset_data(), font)?
                        .into(),
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
                        anchor.serialize(self.offset_data(), font)?.into(),
                    );
                }
            }

            let base_name = names.get(base_glyph);
            bases.insert(base_name, anchors.into());
        }
        map.insert("bases".to_string(), bases.into());

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
    fn serialize_subtable(&self, font: &FontRef, names: &NameMap) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "mark_to_mark".into());
        let mark_array = self.mark1_array()?;
        let mut marks = Map::new();
        for (mark_glyph, mark_record) in
            self.mark1_coverage()?.iter().zip(mark_array.mark_records())
        {
            let mark_name = names.get(mark_glyph);
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
                        .serialize(self.offset_data(), font)?
                        .into(),
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
                        anchor.serialize(self.offset_data(), font)?.into(),
                    );
                }
            }

            let base_name = names.get(base_glyph);
            bases.insert(base_name, anchors.into());
        }
        map.insert("basemarks".to_string(), bases.into());

        Ok(Value::Object(map))
    }
}
