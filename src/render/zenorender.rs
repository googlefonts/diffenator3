use std::sync::Arc;

use rasterize::{ActiveEdgeRasterizer, FillRule, Image, Layer, LinColor, Path, Scene, Transform};
use rustybuzz::{shape_with_plan, Direction, Face, ShapePlan, UnicodeBuffer};
use skrifa::{
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
    raw::TableProvider,
    GlyphId, MetadataProvider, OutlineGlyphCollection,
};

use zeno::{Command, Mask, PathBuilder};

use crate::dfont::DFont;

fn pt(v: &zeno::Vector) -> rasterize::Point {
    rasterize::Point::new(v.x as f64, v.y as f64)
}

#[derive(Default)]
struct RecordingPen {
    buffer: Vec<Command>,
    pub offset_x: f32,
    pub offset_y: f32,
}

// Implement the Pen trait for this type. This emits the appropriate
// SVG path commands for each element type.
impl OutlinePen for RecordingPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.buffer.move_to([self.offset_x + x, self.offset_y + y]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.buffer.line_to([self.offset_x + x, self.offset_y + y]);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.buffer.quad_to(
            [self.offset_x + cx0, self.offset_y + cy0],
            [self.offset_x + x, self.offset_y + y],
        );
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.buffer.curve_to(
            [self.offset_x + cx0, self.offset_y + cy0],
            [self.offset_x + cx1, self.offset_y + cy1],
            [self.offset_x + x, self.offset_y + y],
        );
    }

    fn close(&mut self) {
        self.buffer.close();
    }
}

pub struct Renderer<'a> {
    face: Face<'a>,
    scale: f32,
    font: skrifa::FontRef<'a>,
    plan: ShapePlan,
    outlines: OutlineGlyphCollection<'a>,
}

impl<'a> Renderer<'a> {
    pub fn new(
        font: &'a DFont,
        font_size: f32,
        direction: Direction,
        script: Option<rustybuzz::Script>,
    ) -> Self {
        let face = Face::from_slice(&font.backing, 0).expect("Foo");
        let font = skrifa::FontRef::new(&font.backing).unwrap_or_else(|_| {
            panic!(
                "error constructing a Font from data for {:}",
                font.family_name()
            );
        });
        let plan = ShapePlan::new(&face, direction, script, None, &[]);
        let outlines = font.outline_glyphs();

        Self {
            face,
            font,
            plan,
            scale: font_size,
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
            let settings = DrawSettings::unhinted(Size::new(self.scale), LocationRef::default());

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

    pub fn render_positioned_glyphs(&mut self, pen_buffer: &[Command]) -> Layer<LinColor> {
        let mut builder = Path::builder();
        for command in pen_buffer {
            match command {
                Command::MoveTo(to) => builder.move_to(pt(to)),
                Command::LineTo(to) => builder.line_to(pt(to)),
                Command::QuadTo(ctrl, to) => builder.quad_to(pt(ctrl), pt(to)),
                Command::CurveTo(ctrl0, ctrl1, to) => {
                    builder.cubic_to(pt(ctrl0), pt(ctrl1), pt(to))
                }
                Command::Close => builder.close(),
            };
        }
        let path = builder.build();
        let bbox = path
            .bbox(Transform::identity())
            .ok_or("Empty path")
            .unwrap();
        let scene = Scene::group(vec![Scene::fill(
            path.into(),
            Arc::new("#000000".parse::<LinColor>().unwrap()),
            FillRule::NonZero,
        )]);
        scene.render(
            &ActiveEdgeRasterizer::default(),
            Transform::identity(),
            None,
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufWriter, sync::Arc};

    use super::*;
    use rustybuzz::{script, Direction};

    #[test]
    fn test_zeno_path() {
        let path = "NotoSansArabic-NewRegular.ttf";
        let data = std::fs::read(path).unwrap();
        let font = DFont::new(&data);
        let mut renderer = Renderer::new(&font, 40.0, Direction::RightToLeft, Some(script::ARABIC));
        let (serialized_buffer, commands) = renderer
            .string_to_positioned_glyphs("السلام عليكم")
            .unwrap();
        static IMAGE_NAME: &str = "test.png";
        let mut output = BufWriter::new(File::create(IMAGE_NAME).unwrap());
        let image = renderer.render_positioned_glyphs(&commands);
        image.write_png(&mut output).unwrap();
    }
}
