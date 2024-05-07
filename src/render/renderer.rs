use crate::render::DFont;
use ab_glyph::{point, Font, FontRef, Glyph, GlyphId, Outline, OutlinedGlyph, ScaleFont};
use image::{DynamicImage, GrayImage, Luma};
use rustybuzz::{shape, Face, UnicodeBuffer};
use std::collections::BTreeMap;

pub struct Renderer<'a> {
    face: Face<'a>,
    scale: f32,
    font: FontRef<'a>,
    outline_cache: BTreeMap<GlyphId, Option<Outline>>,
}

impl<'a> Renderer<'a> {
    pub fn new(font: &'a DFont, font_size: f32) -> Self {
        let face = Face::from_slice(&font.backing, 0).expect("Foo");
        let font = FontRef::try_from_slice(&font.backing).unwrap_or_else(|_| {
            panic!(
                "error constructing a Font from data for {:}",
                font.family_name()
            );
        });

        Self {
            face,
            font,
            scale: font_size,
            outline_cache: BTreeMap::new(),
        }
    }

    fn get_outline(&mut self, id: GlyphId) -> Option<Outline> {
        self.outline_cache
            .entry(id)
            .or_insert_with(|| self.font.outline(id))
            .clone()
    }

    pub fn render_string(&mut self, string: &str) -> Option<(String, GrayImage)> {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(string);
        let output = shape(&self.face, &[], buffer);
        let scaled_font = self.font.as_scaled(self.scale);

        // The results of the shaping operation are stored in the `output` buffer.
        let positions = output.glyph_positions();
        let mut serialized_buffer = String::new();
        let infos = output.glyph_infos();
        let mut glyphs: Vec<Glyph> = vec![];
        let factor = self.scale / self.font.height_unscaled();
        // LSB is LSB of first base glyph
        let mut cursor = positions
            .iter()
            .zip(infos)
            .find(|(p, _i)| p.x_advance > 0)
            .map(|(_p, i)| scaled_font.h_side_bearing(GlyphId(i.glyph_id as u16)))
            .unwrap_or(0.0);

        for (position, info) in positions.iter().zip(infos) {
            if info.glyph_id == 0 {
                return None;
            }
            let x = cursor + (position.x_offset as f32 * factor);
            let y = -position.y_offset as f32 * factor;
            glyphs.push(Glyph {
                id: GlyphId(info.glyph_id as u16),
                scale: scaled_font.scale(),
                position: point(x, y + factor * self.face.ascender() as f32),
            });
            serialized_buffer.push_str(&format!(
                "gid={},position={},{}|",
                info.glyph_id, position.x_offset, position.y_offset
            ));
            cursor += position.x_advance as f32 * factor;
        }
        if glyphs.is_empty() {
            return None;
        }
        let width = {
            let last_glyph = glyphs.last().unwrap();
            let max_x = scaled_font.glyph_bounds(last_glyph).max.x;
            max_x as u32
        };
        let height = (self.scale * 1.2) as u32;

        let mut image = DynamicImage::new_luma8(width, height).into_luma8();

        for glyph in glyphs {
            if let Some(outline) = self.get_outline(glyph.id) {
                // Unscaled and unpositioned
                let scale_factor = self.font.as_scaled(glyph.scale).scale_factor();
                let outlined = OutlinedGlyph::new(glyph, outline, scale_factor);
                let bounds = outlined.px_bounds();
                // Draw the glyph into the image per-pixel by using the draw closure
                outlined.draw(|x, y, v| {
                    // Offset the position by the glyph bounding box
                    if x + bounds.min.x as u32 >= width || y + bounds.min.y as u32 >= height {
                        return;
                    }
                    let px = image.get_pixel_mut(x + bounds.min.x as u32, y + bounds.min.y as u32);
                    // Turn the coverage into an alpha value (blended with any previous)
                    // let bitmap_pixel = (v * 255.0) as u8;
                    let bitmap_pixel = if v > 0.5 { 255 } else { 0 };
                    *px = Luma([px.0[0].saturating_add(bitmap_pixel)]);
                });
            }
        }
        serialized_buffer.pop();
        Some((serialized_buffer, image))
    }
}
