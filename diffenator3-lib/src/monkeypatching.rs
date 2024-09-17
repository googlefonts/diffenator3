/// Methods which other people's structs really should have but sadly don't.
use std::collections::HashSet;

use read_fonts::{
    tables::{fvar::VariationAxisRecord, gsub::ClassDef, varc::CoverageTable},
    ReadError, TableProvider,
};
use skrifa::{setting::VariationSetting, FontRef, GlyphId16};

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
    /// Given a normalized location tuple, turn it back into a friendly representation in userspace
    fn denormalize_location(&self, tuple: &[f32]) -> Result<Vec<VariationSetting>, ReadError>;
}

impl DenormalizeLocation for FontRef<'_> {
    fn denormalize_location(&self, tuple: &[f32]) -> Result<Vec<VariationSetting>, ReadError> {
        let all_axes = self.fvar()?.axes()?;
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

pub trait MonkeyPatchClassDef {
    /// Return a list of glyphs in this class
    fn class_glyphs(&self, class: u16, coverage: Option<CoverageTable>) -> Vec<GlyphId16>;
}

impl MonkeyPatchClassDef for ClassDef<'_> {
    fn class_glyphs(&self, class: u16, coverage: Option<CoverageTable>) -> Vec<GlyphId16> {
        if class == 0 {
            // let coverage_map = coverage.unwrap().coverage_map();
            if let Some(coverage) = coverage {
                let all_glyphs: HashSet<GlyphId16> = coverage.iter().collect();
                let in_a_class: HashSet<GlyphId16> =
                    self.iter().map(|(gid, _a_class)| gid).collect();
                // Remove all the glyphs in assigned class
                all_glyphs.difference(&in_a_class).copied().collect()
            } else {
                panic!("ClassDef has no coverage table and class=0 was requested");
            }
        } else {
            self.iter()
                .filter(move |&(_gid, their_class)| their_class == class)
                .map(|(gid, _)| gid)
                .collect()
        }
    }
}
