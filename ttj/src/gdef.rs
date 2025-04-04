use crate::layout::variable_scalars::{hashmap_to_value, serialize_all_deltas};

use super::context::SerializationContext;
use read_fonts::tables::gdef::{
    AttachList, CaretValue, ClassDef, GlyphClassDef, LigCaretList, MarkGlyphSets,
};
use read_fonts::tables::gpos::DeviceOrVariationIndex::VariationIndex;
use read_fonts::TableProvider;
use serde_json::{json, Map, Value};
use skrifa::GlyphId16;

pub(crate) fn serialize_gdef_table(context: &SerializationContext) -> Value {
    let mut map = Map::new();
    if let Ok(gdef) = context.font.gdef() {
        if let Some(Ok(classdef)) = gdef.glyph_class_def() {
            map.insert(
                "glyph_classes".to_string(),
                Value::Object(serialize_classdefs(&classdef, context, true)),
            );
        }
        if let Some(Ok(attachlist)) = gdef.attach_list() {
            serialize_attachlist(attachlist, context, &mut map);
        }
        if let Some(Ok(lig_caret_list)) = gdef.lig_caret_list() {
            serialize_ligcarets(lig_caret_list, context, &mut map);
        }
        if let Some(Ok(classdef)) = gdef.mark_attach_class_def() {
            map.insert(
                "mark_attach_classes".to_string(),
                Value::Object(serialize_classdefs(&classdef, context, false)),
            );
        }

        if let Some(Ok(markglyphsets)) = gdef.mark_glyph_sets_def() {
            map.insert(
                "mark_glyph_sets".to_string(),
                Value::Object(serialize_markglyphs(&markglyphsets, context)),
            );
        }
    }
    Value::Object(map)
}

fn serialize_ligcarets(
    lig_caret_list: LigCaretList,
    context: &SerializationContext,
    map: &mut Map<String, Value>,
) {
    let mut lig_carets = Map::new();
    if let Ok(coverage) = lig_caret_list.coverage() {
        for (ligature, gid) in lig_caret_list.lig_glyphs().iter().zip(coverage.iter()) {
            if let Ok(ligature) = ligature {
                let name = context.names.get(gid);
                lig_carets.insert(
                    name,
                    Value::Array(
                        ligature
                            .caret_values()
                            .iter()
                            .flatten()
                            .map(|x| match x {
                                CaretValue::Format1(c) => {
                                    json!({"coordinate": c.coordinate() })
                                }
                                CaretValue::Format2(c) => {
                                    json!({"point_index": c.caret_value_point_index() })
                                }
                                CaretValue::Format3(c) => {
                                    if let Ok(VariationIndex(device)) = c.device() {
                                        json!({"coordinate": serialize_all_deltas(device, context, c.coordinate().into())
                                                             .map(hashmap_to_value)
                                                             .unwrap_or_else(|_| Value::String(c.coordinate().to_string()))
                                            })
                                    } else {
                                        json!({"variable_coordinate": c.coordinate() })
                                    }
                                }
                            })
                            .collect::<Vec<Value>>(),
                    ),
                );
            }
        }
    }
    map.insert("lig_carets".to_string(), Value::Object(lig_carets));
}

fn serialize_attachlist(
    attachlist: AttachList,
    context: &SerializationContext,
    map: &mut Map<String, Value>,
) {
    let mut attachments = Map::new();
    if let Ok(coverage) = attachlist.coverage() {
        for (point, gid) in attachlist.attach_points().iter().zip(coverage.iter()) {
            if let Ok(point) = point {
                let name = context.names.get(gid);
                attachments.insert(
                    name,
                    Value::Array(
                        point
                            .point_indices()
                            .iter()
                            .map(|x| Value::Number(x.get().into()))
                            .collect::<Vec<Value>>(),
                    ),
                );
            }
        }
    }
    map.insert("attach_points".to_string(), Value::Object(attachments));
}

fn serialize_classdefs(
    classdef: &ClassDef<'_>,
    context: &SerializationContext,
    use_enum: bool,
) -> Map<String, Value> {
    let mut glyph_classes = Map::new();
    for gid in 0..context.names.len() {
        let name = context.names.get(gid as u32);
        let class = classdef.get(GlyphId16::new(gid as u16));
        if class == 0 {
            continue;
        }
        glyph_classes.insert(
            name,
            if use_enum {
                serde_json::value::to_value(GlyphClassDef::new(class)).unwrap_or_default()
            } else {
                Value::Number(class.into())
            },
        );
    }
    glyph_classes
}

fn serialize_markglyphs(
    markglyphsets: &MarkGlyphSets<'_>,
    context: &SerializationContext,
) -> Map<String, Value> {
    markglyphsets
        .coverages()
        .iter()
        .enumerate()
        .map(|(index, coverage)| {
            (
                format!("{}", index),
                if let Ok(coverage) = coverage {
                    let glyphnames = coverage
                        .iter()
                        .map(|gid| context.names.get(gid))
                        .collect::<Vec<String>>();
                    Value::Array(glyphnames.into_iter().map(Value::String).collect())
                } else {
                    Value::Null
                },
            )
        })
        .collect()
}
