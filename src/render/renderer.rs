use image::{DynamicImage, GrayImage, Luma};
use rustybuzz::{shape_with_plan, Direction, Face, ShapePlan, UnicodeBuffer};
use skrifa::{
    instance::{LocationRef, Size},
    outline::DrawSettings,
    raw::TableProvider,
    GlyphId, MetadataProvider, OutlineGlyphCollection,
};
use zeno::Command;

use super::utils::{terrible_bounding_box, RecordingPen};
use crate::dfont::DFont;

pub struct Renderer<'a> {
    face: Face<'a>,
    scale: f32,
    font: skrifa::FontRef<'a>,
    location: LocationRef<'a>,
    plan: ShapePlan,
    outlines: OutlineGlyphCollection<'a>,
}

impl<'a> Renderer<'a> {
    pub fn new(
        dfont: &'a DFont,
        font_size: f32,
        direction: Direction,
        script: Option<rustybuzz::Script>,
    ) -> Self {
        let face = Face::from_slice(&dfont.backing, 0).expect("Foo");
        let font = skrifa::FontRef::new(&dfont.backing).unwrap_or_else(|_| {
            panic!(
                "error constructing a Font from data for {:}",
                dfont.family_name()
            );
        });
        let plan = ShapePlan::new(&face, direction, script, None, &[]);
        let outlines = font.outline_glyphs();

        Self {
            face,
            font,
            plan,
            scale: font_size,
            location: (&dfont.location).into(),
            outlines,
        }
    }
    pub fn string_to_positioned_glyphs(&mut self, string: &str) -> Option<(String, Vec<Command>)> {
        let mut pen = RecordingPen::default();

        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(string);
        let output = shape_with_plan(&self.face, &self.plan, buffer);
        let upem = self.font.head().unwrap().units_per_em();

        // The results of the shaping operation are stored in the `output` buffer.
        let positions = output.glyph_positions();
        let mut serialized_buffer = String::new();
        let infos = output.glyph_infos();
        let mut cursor = 0.0;
        let factor = self.scale / upem as f32;
        for (position, info) in positions.iter().zip(infos) {
            if info.glyph_id == 0 {
                return None;
            }
            pen.offset_x = cursor + (position.x_offset as f32 * factor);
            pen.offset_y = -position.y_offset as f32 * factor;
            let settings = DrawSettings::unhinted(Size::new(self.scale), self.location);

            let _ = self
                .outlines
                .get(GlyphId::new(info.glyph_id as u16))
                .unwrap()
                .draw(settings, &mut pen);
            serialized_buffer.push_str(&format!(
                "gid={},position={},{}|",
                info.glyph_id, position.x_offset, position.y_offset
            ));
            cursor += position.x_advance as f32 * factor;
        }
        if serialized_buffer.is_empty() {
            return None;
        }
        Some((serialized_buffer, pen.buffer))
    }

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
    use rustybuzz::{script, Direction};

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
