/// Turn some words into images
use crate::render::rustyruzz::{
    shape_with_plan, Direction, Face, Script, ShapePlan, UnicodeBuffer, Variation,
};
use image::{DynamicImage, GrayImage, Luma};
use skrifa::instance::Size;
use skrifa::raw::TableProvider;
use skrifa::{GlyphId, MetadataProvider};
use zeno::Command;

use super::cachedoutlines::CachedOutlineGlyphCollection;
use super::utils::{terrible_bounding_box, RecordingPen};
use crate::dfont::DFont;

pub struct Renderer<'a> {
    face: Face<'a>,
    scale: f32,
    font: skrifa::FontRef<'a>,
    plan: ShapePlan,
    outlines: CachedOutlineGlyphCollection<'a>,
}

impl<'a> Renderer<'a> {
    /// Create a new renderer for a font
    ///
    /// Direction and script are needed for correct shaping; no automatic detection is done.
    pub fn new(
        dfont: &'a DFont,
        font_size: f32,
        direction: Direction,
        script: Option<Script>,
    ) -> Self {
        let mut face = Face::from_slice(&dfont.backing, 0).expect("Foo");
        let font = skrifa::FontRef::new(&dfont.backing).unwrap_or_else(|_| {
            panic!(
                "error constructing a Font from data for {:}",
                dfont.family_name()
            );
        });

        // Convert our location into a structure that rustybuzz/harfruzz can use
        let variations: Vec<_> = dfont
            .location
            .iter()
            .map(|setting| {
                let tag = setting.selector;
                let tag = rustybuzz::ttf_parser::Tag::from_bytes(&tag.into_bytes());
                let value = setting.value;
                Variation { tag, value }
            })
            .collect();
        face.set_variations(&variations);
        let plan = ShapePlan::new(&face, direction, script, None, &[]);
        let location = (&dfont.normalized_location).into();
        let outlines = CachedOutlineGlyphCollection::new(
            font.outline_glyphs(),
            Size::new(font_size),
            location,
        );

        Self {
            face,
            font,
            plan,
            scale: font_size,
            outlines,
        }
    }

    /// Render a string to a series of commands
    ///
    /// The commands can be used to render the string to an image. This routine also returns a
    /// serialized buffer that can be used both for debugging purposes and also to detect
    /// glyph sequences which have been rendered already (which helps to speed up the comparison).
    pub fn string_to_positioned_glyphs(&mut self, string: &str) -> Option<(String, Vec<Command>)> {
        let mut pen = RecordingPen::default();

        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(string);
        let output = shape_with_plan(&self.face, &self.plan, buffer);
        let upem = self.font.head().unwrap().units_per_em();
        let factor = self.scale / upem as f32;

        let mut cursor = 0.0;

        // The results of the shaping operation are stored in the `output` buffer.
        let positions = output.glyph_positions();
        let infos = output.glyph_infos();

        let mut serialized_buffer = String::new();

        for (position, info) in positions.iter().zip(infos) {
            pen.offset_x = cursor + (position.x_offset as f32 * factor);
            pen.offset_y = position.y_offset as f32 * factor;
            self.outlines.draw(GlyphId::new(info.glyph_id), &mut pen);
            serialized_buffer.push_str(&format!("{}", info.glyph_id,));
            if position.x_offset != 0 || position.y_offset != 0 {
                serialized_buffer
                    .push_str(&format!("@{},{}", position.x_offset, position.y_offset));
            }
            serialized_buffer.push('|');
            cursor += position.x_advance as f32 * factor;
        }
        if serialized_buffer.is_empty() {
            return None;
        }
        Some((serialized_buffer, pen.buffer))
    }

    /// Render a series of commands to an image
    ///
    /// This routine takes a series of commands returned from [string_to_positioned_glyphs]
    /// and renders them to an image.
    pub fn render_positioned_glyphs(&mut self, pen_buffer: &[Command]) -> GrayImage {
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
    use rustybuzz::script;

    #[test]
    fn test_zeno_path() {
        let path = "NotoSansArabic-NewRegular.ttf";
        let data = std::fs::read(path).unwrap();
        let font = DFont::new(&data);
        let mut renderer = Renderer::new(&font, 40.0, Direction::RightToLeft, Some(script::ARABIC));
        let (_serialized_buffer, commands) = renderer
            .string_to_positioned_glyphs("السلام عليكم")
            .unwrap();
        let image = renderer.render_positioned_glyphs(&commands);
        image.save("test.png").unwrap();
    }
}
