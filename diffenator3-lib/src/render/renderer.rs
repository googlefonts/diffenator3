/// Turn some words into images
use std::any::Any;

use harfrust::{Direction, Script, ShapePlan, ShaperData, ShaperInstance, Variation};
use image::{DynamicImage, GrayImage, Luma};
use skrifa::{instance::Size, MetadataProvider};
use zeno::Command;

use super::{
    cachedoutlines::CachedOutlineGlyphCollection,
    utils::{terrible_bounding_box, RecordingPen},
};
use crate::{dfont::DFont, render::shaper::DrawBuffer};

pub trait AnyRenderer {
    /// Shape `string` and return a serialized glyph buffer plus an opaque handle
    /// to the renderer's intermediate data.
    ///
    /// The serialized buffer is used for de-duplication and for reporting; the
    /// opaque handle is passed back to [`Self::fast_equivalence_check`] and
    /// [`Self::final_rendering`] for this renderer to downcast.
    fn string_to_stage1_rendering(&mut self, string: &str) -> Option<(String, Box<dyn Any>)>;

    /// Cheap check for whether two intermediate renderings are equivalent, so that
    /// rasterization can be skipped when both fonts produce identical output.
    ///
    /// The arguments always originate from this same renderer's
    /// [`Self::string_to_stage1_rendering`], so implementations can safely
    /// downcast them to their concrete intermediate type.
    fn fast_equivalence_check(&self, data1: &dyn Any, data2: &dyn Any) -> bool;

    /// Rasterize intermediate data into a grayscale image.
    fn final_rendering(&mut self, data: &dyn Any) -> GrayImage;
}

pub struct Renderer<'a> {
    shaper_data: ShaperData,
    scale: f32,
    font: skrifa::FontRef<'a>,
    plan: Option<ShapePlan>,
    instance: ShaperInstance,
    outlines: CachedOutlineGlyphCollection<'a>,
}

impl<'a> Renderer<'a> {
    /// Create a new renderer for a font
    ///
    /// Direction and script are needed for correct shaping; no automatic detection is done.
    pub fn new(
        dfont: &'a DFont,
        font_size: f32,
        direction: Option<Direction>,
        script: Option<Script>,
    ) -> Self {
        let font = harfrust::FontRef::new(&dfont.backing).unwrap_or_else(|_| {
            panic!(
                "error constructing a Font from data for {:}",
                dfont.family_name()
            );
        });
        let shaper_data = ShaperData::new(&font);

        // Convert our location into a structure that rustybuzz/harfruzz can use
        let instance = ShaperInstance::from_variations(
            &font,
            dfont.location.iter().map(|setting| {
                let tag = setting.selector;
                let value = setting.value;
                Variation { tag, value }
            }),
        );
        let shaper = shaper_data.shaper(&font).instance(Some(&instance)).build();

        let plan = if let Some(direction) = direction {
            if script.is_some() {
                Some(ShapePlan::new(&shaper, direction, script, None, &[]))
            } else {
                None
            }
        } else {
            None
        };
        let location = (&dfont.normalized_location).into();
        let outlines = CachedOutlineGlyphCollection::new(
            font.outline_glyphs(),
            Size::new(font_size),
            location,
        );

        Self {
            shaper_data,
            font,
            plan,
            instance,
            scale: font_size,
            outlines,
        }
    }
}

impl AnyRenderer for Renderer<'_> {
    /// Render a string to a series of commands
    ///
    /// The commands can be used to render the string to an image. This routine also returns a
    /// serialized buffer that can be used both for debugging purposes and also to detect
    /// glyph sequences which have been rendered already (which helps to speed up the comparison).
    fn string_to_stage1_rendering(&mut self, string: &str) -> Option<(String, Box<dyn Any>)> {
        let draw_buffer = DrawBuffer::new_from_font(
            &self.shaper_data,
            &self.font,
            string,
            Some(&self.instance),
            self.scale,
            self.plan.as_ref(),
        );
        let mut pen = RecordingPen::default();
        for glyph in draw_buffer.iter() {
            pen.offset_x = glyph.x_pos;
            pen.offset_y = glyph.y_pos;
            self.outlines.draw(glyph.glyph_id, &mut pen);
        }
        let serialized_buffer = draw_buffer.serialize();
        if serialized_buffer.is_empty() {
            return None;
        }
        Some((serialized_buffer, Box::new(pen.buffer)))
    }

    fn fast_equivalence_check(&self, data1: &dyn Any, data2: &dyn Any) -> bool {
        // The data always comes from this renderer, so the downcast should succeed; if it
        // somehow doesn't, conservatively fall back to rasterizing.
        match (
            data1.downcast_ref::<Vec<Command>>(),
            data2.downcast_ref::<Vec<Command>>(),
        ) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Render a series of commands to an image
    ///
    /// This routine takes a series of commands returned from [string_to_stage1_rendering]
    /// and renders them to an image.
    fn final_rendering(&mut self, data: &dyn Any) -> GrayImage {
        let pen_buffer = data
            .downcast_ref::<Vec<Command>>()
            .expect("final_rendering: expected Vec<Command> from string_to_stage1_rendering");
        let (min_x, min_y, max_x, max_y) = terrible_bounding_box(pen_buffer);
        let x_origin = min_x.min(0.0);
        let y_origin = min_y.min(0.0);
        let x_size = (max_x - x_origin).ceil() as usize;
        let y_size = (max_y - y_origin).ceil() as usize;

        let mut rasterizer = ab_glyph_rasterizer::Rasterizer::new(x_size, y_size);

        let mut cursor = ab_glyph::Point { x: 0.0, y: 0.0 };
        let v2p = |v: &zeno::Vector| ab_glyph::Point {
            x: v.x - x_origin.ceil(),
            y: v.y - y_origin.ceil(),
        };
        let mut home = v2p(&zeno::Vector::new(0.0, 0.0));
        for command in pen_buffer {
            match command {
                Command::MoveTo(to) => {
                    cursor = v2p(to);
                    home = cursor;
                }
                Command::LineTo(to) => {
                    let newpt = v2p(to);
                    rasterizer.draw_line(cursor, newpt);
                    cursor = newpt;
                }
                Command::QuadTo(ctrl, to) => {
                    let ctrlpt = v2p(ctrl);
                    let newpt = v2p(to);
                    rasterizer.draw_quad(cursor, ctrlpt, newpt);
                    cursor = newpt;
                }
                Command::CurveTo(ctrl0, ctrl1, to) => {
                    let ctrl0pt = v2p(ctrl0);
                    let ctrl1pt = v2p(ctrl1);
                    let newpt = v2p(to);
                    rasterizer.draw_cubic(cursor, ctrl0pt, ctrl1pt, newpt);
                    cursor = newpt;
                }
                Command::Close => {
                    if cursor != home {
                        rasterizer.draw_line(cursor, home);
                    }
                }
            };
        }
        let mut image = DynamicImage::new_luma8(x_size as u32, y_size as u32).into_luma8();
        rasterizer.for_each_pixel_2d(|x, y, alpha| {
            image.put_pixel(x, y, Luma([(alpha * 255.0) as u8]));
        });
        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use harfruzz::script;
    use harfrust::script;

    #[test]
    fn test_zeno_path() {
        let path = "NotoSansArabic-NewRegular.ttf";
        let data = std::fs::read(path).unwrap();
        let font = DFont::new(&data);
        let mut renderer = Renderer::new(
            &font,
            40.0,
            Some(Direction::RightToLeft),
            Some(script::ARABIC),
        );
        let (_serialized_buffer, commands) =
            renderer.string_to_stage1_rendering("السلام عليكم").unwrap();
        let image = renderer.final_rendering(&*commands);
        image.save("test.png").unwrap();
    }
}
