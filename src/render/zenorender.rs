use image::{GrayImage, ImageBuffer};
use read_fonts::{FontRef, TableProvider};
use rustybuzz::{shape, Face, UnicodeBuffer};
use skrifa::{
    instance::Size,
    scale::{Context, Pen, Scaler},
    GlyphId,
};
use std::collections::BTreeMap;
use zeno::{Command, Mask, PathBuilder};

use crate::dfont::DFont;

#[derive(Default)]
struct RecordingPen {
    buffer: Vec<Command>,
    pub offset_x: f32,
    pub offset_y: f32,
}

// Implement the Pen trait for this type. This emits the appropriate
// SVG path commands for each element type.
impl Pen for RecordingPen {
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
    font: FontRef<'a>,
    context: Context,
}

impl<'a> Renderer<'a> {
    pub fn new(font: &'a DFont, font_size: f32) -> Self {
        let mut context = Context::new();

        let face = Face::from_slice(&font.backing, 0).expect("Foo");
        let font = FontRef::new(&font.backing).unwrap_or_else(|_| {
            panic!(
                "error constructing a Font from data for {:}",
                font.family_name()
            );
        });

        Self {
            face,
            font,
            context,
            scale: font_size,
        }
    }

    pub fn render_string(&mut self, string: &str) -> Option<(String, GrayImage)> {
        let mut scaler = self
            .context
            .new_scaler()
            .size(Size::new(self.scale))
            .build(&self.font);
        let mut pen = RecordingPen::default();

        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(string);
        let output = shape(&self.face, &[], buffer);
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
            scaler.outline(GlyphId::new(info.glyph_id as u16), &mut pen);
            serialized_buffer.push_str(&format!(
                "gid={},position={},{}|",
                info.glyph_id, position.x_offset, position.y_offset
            ));
            cursor += position.x_advance as f32 * factor;
        }
        if serialized_buffer.is_empty() {
            return None;
        }

        let (mask, placement) = Mask::new(&pen.buffer)
            .origin(zeno::Origin::BottomLeft)
            .render();
        let mut image = GrayImage::from_raw(placement.width, placement.height, mask).unwrap();
        Some((serialized_buffer, image))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use read_fonts::FontRef;
    use skrifa::{instance::Size, scale::Context, GlyphId};

    #[test]
    fn test_zeno_path() {
        let path = "NotoSansArabic-NewRegular.ttf";
        let data = std::fs::read(path).unwrap();
        let font = DFont::new(&data);
        let mut renderer = Renderer::new(&font, 40.0);
        if let Some((buffer, image)) = renderer.render_string("پپر") {
            image.save("zeno.png").unwrap();
        }
    }
}
