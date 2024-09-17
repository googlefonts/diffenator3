use super::SerializeSubtable;
use crate::context::SerializationContext;
use read_fonts::tables::gsub::AlternateSubstFormat1;
use read_fonts::tables::gsub::LigatureSubstFormat1;
use read_fonts::tables::gsub::MultipleSubstFormat1;
use read_fonts::tables::gsub::ReverseChainSingleSubstFormat1;
use read_fonts::tables::gsub::SingleSubst;
use read_fonts::tables::varc::CoverageTable;
use read_fonts::ReadError;
use serde_json::Map;
use serde_json::Value;
use skrifa::GlyphId16;

impl SerializeSubtable for SingleSubst<'_> {
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "single".into());
        let coverage = match self {
            SingleSubst::Format1(s) => s.coverage()?,
            SingleSubst::Format2(s) => s.coverage()?,
        };
        match self {
            SingleSubst::Format1(s) => {
                let delta = s.delta_glyph_id();
                for glyph in coverage.iter() {
                    let name_before = context.names.get(glyph);
                    let name_after = context
                        .names
                        .get(GlyphId16::new((glyph.to_u16() as i16 + delta) as u16)); // Good heavens
                    map.insert(name_before, Value::String(name_after));
                }
            }
            SingleSubst::Format2(s) => {
                for (before, after) in coverage.iter().zip(s.substitute_glyph_ids()) {
                    let name_before = context.names.get(before);
                    let name_after = context.names.get(after.get());
                    map.insert(name_before, Value::String(name_after));
                }
            }
        }
        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for MultipleSubstFormat1<'_> {
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "multiple".into());
        let coverage = self.coverage()?;
        for (before, after) in coverage.iter().zip(self.sequences().iter().flatten()) {
            let name_before = context.names.get(before);
            let names_after = after
                .substitute_glyph_ids()
                .iter()
                .map(|gid| Value::String(context.names.get(gid.get())));
            map.insert(name_before, Value::Array(names_after.collect()));
        }
        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for AlternateSubstFormat1<'_> {
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "alternate".into());
        let coverage = self.coverage()?;
        for (before, after) in coverage.iter().zip(self.alternate_sets().iter().flatten()) {
            let name_before = context.names.get(before);
            let names_after = after
                .alternate_glyph_ids()
                .iter()
                .map(|gid| Value::String(context.names.get(gid.get())));
            map.insert(name_before, Value::Array(names_after.collect()));
        }
        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for LigatureSubstFormat1<'_> {
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "ligature".into());
        let coverage = self.coverage()?;
        for (first_glyph, ligset) in coverage.iter().zip(self.ligature_sets().iter().flatten()) {
            let first_glyph_name = context.names.get(first_glyph);
            for ligature in ligset.ligatures().iter().flatten() {
                let mut before = vec![first_glyph_name.clone()];

                before.extend(
                    ligature
                        .component_glyph_ids()
                        .iter()
                        .map(|gid| context.names.get(gid.get())),
                );
                let before_sequence = before.join(" ");

                map.insert(
                    before_sequence,
                    Value::String(context.names.get(ligature.ligature_glyph())),
                );
            }
        }
        Ok(Value::Object(map))
    }
}

impl SerializeSubtable for ReverseChainSingleSubstFormat1<'_> {
    fn serialize_subtable(&self, context: &SerializationContext) -> Result<Value, ReadError> {
        let mut map = Map::new();
        map.insert("type".to_string(), "reverse".into());
        let coverage_to_array = |coverage: CoverageTable<'_>| {
            Value::Array(
                coverage
                    .iter()
                    .map(|gid| Value::String(context.names.get(gid)))
                    .collect::<Vec<_>>(),
            )
        };
        let mut backtrack = self
            .backtrack_coverages()
            .iter()
            .flatten()
            .map(coverage_to_array)
            .collect::<Vec<Value>>();
        backtrack.reverse();
        map.insert("pre_context".to_string(), backtrack.into());
        let lookahead = self
            .lookahead_coverages()
            .iter()
            .flatten()
            .map(coverage_to_array)
            .collect::<Vec<Value>>();
        map.insert("post_context".to_string(), lookahead.into());
        let coverage = self.coverage()?;
        for (before, after) in coverage.iter().zip(self.substitute_glyph_ids().iter()) {
            let name_before = context.names.get(before);
            let name_after = context.names.get(after.get());
            map.insert(name_before, Value::String(name_after));
        }
        Ok(Value::Object(map))
    }
}
