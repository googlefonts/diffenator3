use std::collections::HashMap;

use crate::context::SerializationContext;
use read_fonts::{
    tables::{gpos::DeviceOrVariationIndex::VariationIndex, variations::DeltaSetIndex},
    FontData, ReadError, TableProvider,
};
use serde_json::{Map, Value};

pub(crate) trait SerializeValueRecordLike {
    fn serialize(
        &self,
        offset_data: FontData<'_>,
        context: &SerializationContext,
    ) -> Result<Value, ReadError>;
}

pub(crate) fn hashmap_to_value(hashmap: HashMap<String, i32>) -> Value {
    let delta_map: Map<String, Value> = hashmap
        .iter()
        .map(|(k, v)| (k.clone(), Value::Number((*v).into())))
        .collect();
    Value::Object(delta_map)
}
impl SerializeValueRecordLike for read_fonts::tables::gpos::ValueRecord {
    fn serialize(
        &self,
        offset_data: FontData<'_>,
        context: &SerializationContext,
    ) -> Result<Value, ReadError> {
        let mut vr = Map::new();
        if let Some(x) = self.x_advance() {
            if let Some(Ok(VariationIndex(device))) = self.x_advance_device(offset_data) {
                vr.insert(
                    "x".to_string(),
                    hashmap_to_value(serialize_all_deltas(device, context, x.into())?),
                );
            } else {
                vr.insert("x".to_string(), Value::Number(x.into()));
            }
        }

        if let Some(y) = self.y_advance() {
            if let Some(Ok(VariationIndex(device))) = self.x_advance_device(offset_data) {
                vr.insert(
                    "y".to_string(),
                    hashmap_to_value(serialize_all_deltas(device, context, y.into())?),
                );
            } else {
                vr.insert("y".to_string(), Value::Number(y.into()));
            }
        }

        if let Some(x) = self.x_placement() {
            if let Some(Ok(VariationIndex(device))) = self.x_placement_device(offset_data) {
                vr.insert(
                    "x_placement".to_string(),
                    hashmap_to_value(serialize_all_deltas(device, context, x.into())?),
                );
            } else {
                vr.insert("x_placement".to_string(), Value::Number(x.into()));
            }
        }

        if let Some(y) = self.y_placement() {
            if let Some(Ok(VariationIndex(device))) = self.y_placement_device(offset_data) {
                vr.insert(
                    "y_placement".to_string(),
                    hashmap_to_value(serialize_all_deltas(device, context, y.into())?),
                );
            } else {
                vr.insert("y_placement".to_string(), Value::Number(y.into()));
            }
        }

        Ok(Value::Object(vr))
    }
}

impl SerializeValueRecordLike for read_fonts::tables::gpos::AnchorTable<'_> {
    fn serialize(
        &self,
        _offset_data: FontData<'_>,
        context: &SerializationContext,
    ) -> Result<Value, ReadError> {
        let mut vr = Map::new();
        let x = self.x_coordinate();
        if let Some(Ok(VariationIndex(device))) = self.x_device() {
            vr.insert(
                "x".to_string(),
                hashmap_to_value(serialize_all_deltas(device, context, x.into())?),
            );
        } else {
            vr.insert("x".to_string(), Value::Number(x.into()));
        }
        let y = self.y_coordinate();
        if let Some(Ok(VariationIndex(device))) = self.y_device() {
            vr.insert(
                "y".to_string(),
                hashmap_to_value(serialize_all_deltas(device, context, y.into())?),
            );
        } else {
            vr.insert("y".to_string(), Value::Number(y.into()));
        }

        Ok(Value::Object(vr))
    }
}

pub(crate) fn serialize_all_deltas(
    device: read_fonts::tables::layout::VariationIndex,
    context: &SerializationContext,
    current: i32,
) -> Result<HashMap<String, i32>, ReadError> {
    let d: DeltaSetIndex = device.into();
    let mut result = HashMap::new();
    result.insert("default".to_string(), current);
    if let Some(Ok(ivs)) = context.font.gdef()?.item_var_store() {
        let deltas: Vec<i32> = context
            .gdef_regions
            .iter()
            .map(|coords| ivs.compute_delta(d, coords).unwrap_or(0))
            .collect();
        // println!("Deltas: {:?}", deltas);

        for (location, delta) in context.gdef_locations.iter().zip(deltas.iter()) {
            if *delta == 0 {
                continue;
            }
            result.insert(location.clone(), current + delta);
        }
    }
    Ok(result)
}
