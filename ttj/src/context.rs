use std::collections::HashMap;

use fontdrasil::coords::{
    CoordConverter, DesignCoord, NormalizedCoord, NormalizedLocation, UserCoord,
};
use read_fonts::{types::F2Dot14, ReadError, TableProvider};
use skrifa::{FontRef, MetadataProvider};

use super::namemap::NameMap;

pub(crate) struct SerializationContext<'a> {
    pub(crate) font: &'a FontRef<'a>,
    pub(crate) names: NameMap,
    pub(crate) gdef_regions: Vec<Vec<F2Dot14>>,
    pub(crate) gdef_locations: Vec<String>,
}

fn fontdrasil_axes(font: &FontRef) -> Result<Option<fontdrasil::types::Axes>, ReadError> {
    let per_axis_maps = if let Ok(segments) = font.avar().map(|x| x.axis_segment_maps()) {
        segments.iter().collect::<Result<Vec<_>, _>>()?
    } else {
        vec![]
    };
    Ok(Some(
        font.axes()
            .iter()
            .enumerate()
            .map(|(ix, axis)| {
                let min = UserCoord::new(axis.min_value() as f64);
                let default = UserCoord::new(axis.default_value() as f64);
                let max = UserCoord::new(axis.max_value() as f64);
                #[allow(clippy::unwrap_used)]
                let mut fd_axis = fontdrasil::types::Axis {
                    converter: CoordConverter::default_normalization(min, default, max),
                    hidden: axis.is_hidden(),
                    tag: fontdrasil::types::Tag::new(&axis.tag().into_bytes()),
                    name: axis.tag().to_string(),
                    min,
                    default,
                    max,
                    localized_names: HashMap::new(), // Let's not
                };
                if let Some(map) = per_axis_maps.get(ix) {
                    let desired_mapping: Vec<(
                        fontdrasil::coords::Coord<fontdrasil::coords::UserSpace>,
                        fontdrasil::coords::Coord<fontdrasil::coords::DesignSpace>,
                    )> = map
                        .axis_value_maps
                        .iter()
                        .map(|mapping| {
                            let from = mapping.from_coordinate().to_f32();
                            let to = mapping.to_coordinate().to_f32();
                            // These are both normalized coordinates. Turn the `from` back into
                            // userspace using default normalization
                            let user_from =
                                NormalizedCoord::new(from as f64).to_user(&fd_axis.converter);
                            // Let's pretend design space is just normalized space
                            let design_to = DesignCoord::new(to as f64);
                            (user_from, design_to)
                        })
                        .collect();
                    let default_idx = desired_mapping
                        .iter()
                        .position(|(_, to)| to.to_f64() == 0.0)
                        .unwrap_or(0);
                    fd_axis.converter = CoordConverter::new(desired_mapping, default_idx)
                        .unwrap_or_else(|_| {
                            // If we can't make a converter, just use the default normalization
                            CoordConverter::default_normalization(min, default, max)
                        });
                }
                fd_axis
            })
            .collect(),
    ))
}

impl<'a> SerializationContext<'a> {
    pub fn new(font: &'a FontRef<'a>, names: NameMap) -> Result<Self, ReadError> {
        let axes = font
            .axes()
            .iter()
            .map(|axis| axis.tag())
            .collect::<Vec<_>>();
        let fontdrasil_axes = fontdrasil_axes(font)?.unwrap_or_default();
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
                    let coords_norm: Vec<f32> = tuple.iter().map(|x| x.to_f32()).collect();
                    let normalized_location = axes
                        .iter()
                        .zip(coords_norm.iter())
                        .map(|(tag, coord)| {
                            (
                                fontdrasil::types::Tag::new(&tag.into_bytes()),
                                NormalizedCoord::new(*coord as f64),
                            )
                        })
                        .collect::<NormalizedLocation>();
                    if let Ok(user_location) = normalized_location.to_user(&fontdrasil_axes) {
                        let mut loc_str: Vec<String> = user_location
                            .iter()
                            .map(|(tag, coord)| format!("{}={}", tag, coord.to_f64()))
                            .collect();
                        loc_str.sort();
                        loc_str.join(",")
                    } else {
                        // If we can't convert to user space, just return the normalized location
                        let mut loc_str: Vec<String> = normalized_location
                            .iter()
                            .map(|(tag, coord)| format!("{}={}n", tag, coord.to_f64()))
                            .collect();
                        loc_str.sort();
                        loc_str.join(",")
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
