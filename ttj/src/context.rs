use read_fonts::{types::F2Dot14, ReadError, TableProvider};
use skrifa::FontRef;

use crate::monkeypatching::DenormalizeLocation;

use super::namemap::NameMap;

pub(crate) struct SerializationContext<'a> {
    pub(crate) font: &'a FontRef<'a>,
    pub(crate) names: NameMap,
    pub(crate) gdef_regions: Vec<Vec<F2Dot14>>,
    pub(crate) gdef_locations: Vec<String>,
}

impl<'a> SerializationContext<'a> {
    pub fn new(font: &'a FontRef<'a>, names: NameMap) -> Result<Self, ReadError> {
        let (gdef_regions, gdef_locations) = if let Ok(Some(ivs)) = font
            .gdef()
            .and_then(|gdef| gdef.item_var_store().transpose())
        {
            let regions = ivs.variation_region_list()?.variation_regions();

            // Find all the peaks
            let all_tuples: Vec<Vec<F2Dot14>> = regions
                .iter()
                .flatten()
                .map(|r| r.region_axes().iter().map(|x| x.peak_coord()).collect())
                .collect();
            // Let's turn these back to userspace
            let locations: Vec<String> = all_tuples
                .iter()
                .map(|tuple| {
                    let coords: Vec<f32> = tuple.iter().map(|x| x.to_f32()).collect();
                    if let Ok(location) = font.denormalize_location(&coords) {
                        let mut loc_str: Vec<String> = location
                            .iter()
                            .map(|setting| {
                                setting.selector.to_string() + "=" + &setting.value.to_string()
                            })
                            .collect();
                        loc_str.sort();
                        loc_str.join(",")
                    } else {
                        "Unknown".to_string()
                    }
                })
                .collect();
            (all_tuples, locations)
        } else {
            (Vec::new(), Vec::new())
        };

        Ok(SerializationContext {
            font,
            names,
            gdef_regions,
            gdef_locations,
        })
    }
}
