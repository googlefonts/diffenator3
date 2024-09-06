use read_fonts::{tables::fvar::VariationAxisRecord, ReadError, TableProvider};
use skrifa::{setting::VariationSetting, FontRef};

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

pub trait DenormalizeLocation {
    fn denormalize_location(
        &self,
        tuple: &[f32],
        // particular_axes: Option<&[usize]>,
    ) -> Result<Vec<VariationSetting>, ReadError>;
}

impl DenormalizeLocation for FontRef<'_> {
    fn denormalize_location(
        &self,
        tuple: &[f32],
        // particular_axes: Option<&[usize]>,
    ) -> Result<Vec<VariationSetting>, ReadError> {
        let all_axes = self.fvar()?.axes()?;
        // let axes: Vec<&VariationAxisRecord> = if let Some(these_axes) = particular_axes {
        //     these_axes
        //         .iter()
        //         .map(|i| all_axes.get(*i).unwrap())
        //         .collect()
        // } else {
        //     all_axes.into_iter().collect()
        // };
        Ok(all_axes
            .iter()
            .zip(tuple)
            .filter(|&(_axis, peak)| *peak != 0.0)
            .map(|(axis, peak)| {
                let value = poor_mans_denormalize(*peak, axis);
                (axis.axis_tag().to_string().as_str(), value).into()
            })
            .collect())
    }
}
