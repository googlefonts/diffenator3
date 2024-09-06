use read_fonts::tables::gpos::DeviceOrVariationIndex::VariationIndex;
use read_fonts::FontData;
use read_fonts::ReadError;
use read_fonts::TableProvider;
use skrifa::FontRef;

use crate::monkeypatching::DenormalizeLocation;

pub(crate) trait SerializeValueRecordLike {
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

pub(crate) fn serialize_all_deltas(
    device: read_fonts::tables::layout::VariationIndex,
    font: &FontRef,
    current: i32,
) -> Result<String, ReadError> {
    if let Some(Ok(ivs)) = font.gdef()?.item_var_store() {
        let regions = ivs.variation_region_list()?.variation_regions();
        // Let's turn these back to userspace
        let locations: Vec<String> = regions
            .iter()
            .flatten()
            .map(|r| {
                let tuple: Vec<f32> = r
                    .region_axes()
                    .iter()
                    .map(|x| x.peak_coord().to_f32())
                    .collect();
                if let Ok(location) = font.denormalize_location(&tuple) {
                    let loc_str: Vec<String> = location
                        .iter()
                        .map(|setting| {
                            setting.selector.to_string() + "=" + &setting.value.to_string()
                        })
                        .collect();
                    loc_str.join(",")
                } else {
                    "Unknown".to_string()
                }
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
            return Ok("(".to_string() + deltas.join(" ").as_str() + ")");
        }
    }
    Ok(format!("{}", current))
}
