use harfrust::{ShapePlan, ShaperData, ShaperInstance, UnicodeBuffer};
use read_fonts::TableProvider;
use skrifa::GlyphId;

#[derive(Default)]
pub struct DrawBuffer {
    glyphs: Vec<PositionedGlyph>,
    pub cursor_xmax: f32,
}

impl DrawBuffer {
    pub fn new_from_font(
        shaper_data: &ShaperData,
        font: &skrifa::FontRef,
        string: &str,
        instance: Option<&ShaperInstance>,
        scale: f32,
        plan: Option<&ShapePlan>,
    ) -> Self {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(string);
        let shaper = shaper_data.shaper(font).instance(instance).build();

        let output = if let Some(plan) = &plan {
            // If we have a shaping plan, we can use it to shape the string
            if let Some(script) = plan.script() {
                buffer.set_script(script);
            }
            buffer.set_direction(plan.direction());
            if let Some(lang) = plan.language() {
                buffer.set_language(lang.clone());
            }
            shaper.shape_with_plan(plan, buffer, &[])
        } else {
            // Otherwise, we guess segment properties
            buffer.guess_segment_properties();
            shaper.shape(buffer, &[])
        };
        let upem = font.head().unwrap().units_per_em();
        let factor = scale / upem as f32;

        let mut cursor = 0.0;

        // The results of the shaping operation are stored in the `output` buffer.
        let positions = output.glyph_positions();
        let infos = output.glyph_infos();

        let mut buffer = Self::default();

        for (position, info) in positions.iter().zip(infos) {
            let offset_x = cursor + (position.x_offset as f32 * factor);
            let offset_y = position.y_offset as f32 * factor;
            buffer.glyphs.push(PositionedGlyph {
                glyph_id: GlyphId::new(info.glyph_id),
                x_pos: offset_x,
                y_pos: offset_y,
            });
            cursor += position.x_advance as f32 * factor;
        }
        buffer
    }

    pub fn serialize(&self) -> String {
        let mut serialized_buffer = String::new();
        for glyph in &self.glyphs {
            serialized_buffer.push_str(&format!("{}", glyph.glyph_id));
            if glyph.x_pos != 0.0 || glyph.y_pos != 0.0 {
                serialized_buffer.push_str(&format!("@{},{}", glyph.x_pos, glyph.y_pos));
            }
            serialized_buffer.push('|');
        }
        serialized_buffer
    }

    pub fn iter(&self) -> impl Iterator<Item = &PositionedGlyph> {
        self.glyphs.iter()
    }
}

pub struct PositionedGlyph {
    pub glyph_id: GlyphId,
    pub x_pos: f32,
    pub y_pos: f32,
}
