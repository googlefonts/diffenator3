use std::hash::Hash;

use fontdrasil::coords::NormalizedCoord;
use harfrust::{ShapePlan, ShaperData, ShaperInstance, UnicodeBuffer};
use read_fonts::{types::F2Dot14, TableProvider};
use skrifa::GlyphId;

#[derive(Default, Clone, PartialEq, Debug)]
pub struct DrawBuffer {
    glyphs: Vec<PositionedGlyph>,
    pub cursor_xmax: f32,
}

impl DrawBuffer {
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

    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &PositionedGlyph> {
        self.glyphs.iter()
    }
}

#[derive(Clone, PartialEq, Debug, Copy)]
pub struct PositionedGlyph {
    pub glyph_id: GlyphId,
    pub x_pos: f32,
    pub y_pos: f32,
}
impl Eq for PositionedGlyph {}

impl Hash for PositionedGlyph {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.glyph_id.hash(state);
        F2Dot14::from_f32(self.x_pos).to_bits().hash(state);
        F2Dot14::from_f32(self.y_pos).to_bits().hash(state);
    }
}

pub struct CachedShaper<'a> {
    shaper_data: ShaperData,
    scale: f32,
    font: skrifa::FontRef<'a>,
    plan: Option<ShapePlan>,
}

impl<'a> CachedShaper<'a> {
    pub fn new(
        font: skrifa::FontRef<'a>,
        scale: f32,
        direction: Option<harfrust::Direction>,
        script: Option<harfrust::Script>,
    ) -> Self {
        let shaper_data = ShaperData::new(&font);

        let shaper = shaper_data.shaper(&font).instance(None).build();

        let plan = if let Some(direction) = direction {
            if script.is_some() {
                Some(ShapePlan::new(&shaper, direction, script, None, &[]))
            } else {
                None
            }
        } else {
            None
        };
        Self {
            shaper_data,
            scale,
            font,
            plan,
        }
    }

    pub fn shape(&self, string: &str, location: Option<Vec<NormalizedCoord>>) -> DrawBuffer {
        let mut buffer = UnicodeBuffer::new();
        let instance = location.map(|location| {
            ShaperInstance::from_coords(&self.font, location.iter().map(|x| x.to_f2dot14()))
        });
        buffer.push_str(string);
        let shaper = self
            .shaper_data
            .shaper(&self.font)
            .instance(instance.as_ref())
            .build();

        let output = if let Some(plan) = &self.plan {
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
        let upem = self.font.head().unwrap().units_per_em();
        let factor = self.scale / upem as f32;

        let mut cursor = 0.0;

        // The results of the shaping operation are stored in the `output` buffer.
        let positions = output.glyph_positions();
        let infos = output.glyph_infos();

        let mut buffer = DrawBuffer::default();

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
        buffer.cursor_xmax = cursor;
        buffer
    }
}
