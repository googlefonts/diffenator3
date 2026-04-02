/// Color glyph rendering for fonts with COLR tables.
///
/// This module provides [`ColorRenderer`], a renderer for fonts containing a
/// COLR table (color glyphs). It renders each unique glyph once via
/// [`SkiaPainter`] and caches the resulting RGBA tile. Subsequent occurrences
/// of the same glyph are composited from the cache, avoiding repeated paint
/// graph traversals.
use std::collections::HashMap;

use harfrust::{
    Direction, Script, ShapePlan, ShaperData, ShaperInstance, UnicodeBuffer, Variation,
};
use image::{GrayImage, Luma};
use skrifa::{
    color::{ColorPainter, Transform},
    instance::Size,
    prelude::LocationRef,
    raw::TableProvider,
    GlyphId, MetadataProvider,
};
use tiny_skia::{Pixmap, PixmapPaint, Transform as TsTransform};

use super::colorpainter::{PaletteColor, SkiaPainter};
use crate::dfont::DFont;

/// A pre-rendered glyph tile cached for reuse across words.
struct CachedColorGlyph {
    /// The rendered RGBA bitmap of this glyph.
    pixmap: Pixmap,
    /// Pixel offset from the glyph origin to the left edge of the bitmap.
    bearing_x: f32,
    /// Pixel offset from the baseline to the top edge of the bitmap.
    bearing_y: f32,
}

pub struct ColorRenderer<'a> {
    shaper_data: ShaperData,
    scale: f32,
    font: skrifa::FontRef<'a>,
    plan: Option<ShapePlan>,
    instance: ShaperInstance,
    palette: Vec<PaletteColor>,
    location: LocationRef<'a>,
    cache: HashMap<u32, CachedColorGlyph>,
}

impl<'a> ColorRenderer<'a> {
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

        let location: LocationRef = (&dfont.normalized_location).into();
        let palette = read_cpal_palette(&font);

        Self {
            shaper_data,
            font,
            plan,
            instance,
            scale: font_size,
            palette,
            location,
            cache: HashMap::new(),
        }
    }

    /// Compute the tile dimensions and bearing for a glyph.
    ///
    /// Returns `(bearing_x, bearing_y, width, height)` in pixels.
    /// `bearing_x` is the horizontal offset from the glyph origin to the tile's left edge.
    /// `bearing_y` is the vertical offset from the baseline to the tile's top edge (Y-up).
    fn glyph_tile_bounds(&self, glyph_id: GlyphId) -> (f32, f32, u32, u32) {
        let size = Size::new(self.scale);
        let color_glyphs = self.font.color_glyphs();

        // COLRv1 glyphs may have a clip box that gives tight pixel bounds
        if let Some(color_glyph) = color_glyphs.get(glyph_id) {
            if let Some(bbox) = color_glyph.bounding_box(self.location, size) {
                let w = (bbox.x_max - bbox.x_min).ceil().max(1.0) as u32;
                let h = (bbox.y_max - bbox.y_min).ceil().max(1.0) as u32;
                return (bbox.x_min, bbox.y_max, w, h);
            }
        }

        // Fallback for COLRv0 or outline glyphs: use font-level metrics
        let glyph_metrics = self.font.glyph_metrics(size, self.location);
        let advance = glyph_metrics.advance_width(glyph_id).unwrap_or(self.scale);
        let metrics = self.font.metrics(size, self.location);
        let w = advance.ceil().max(1.0) as u32;
        let h = (metrics.ascent - metrics.descent).ceil().max(1.0) as u32;
        (0.0, metrics.ascent, w, h)
    }

    /// Render a single glyph into a tile for caching.
    fn render_glyph(&self, glyph_id: GlyphId) -> CachedColorGlyph {
        let upem = self.font.head().unwrap().units_per_em() as f32;
        let factor = self.scale / upem;

        let (bearing_x, bearing_y, tile_w, tile_h) = self.glyph_tile_bounds(glyph_id);

        let outlines = self.font.outline_glyphs();
        let mut painter =
            SkiaPainter::new(tile_w, tile_h, &self.palette, outlines, self.location);

        // Transform: maps font-unit origin (0,0) to pixel (-bearing_x, bearing_y)
        // within the tile, with Y-flip (font Y-up → pixel Y-down).
        let transform = Transform {
            xx: factor,
            yx: 0.0,
            xy: 0.0,
            yy: -factor,
            dx: -bearing_x,
            dy: bearing_y,
        };

        let color_glyphs = self.font.color_glyphs();
        if let Some(color_glyph) = color_glyphs.get(glyph_id) {
            painter.push_transform(transform);
            let _ = color_glyph.paint(self.location, &mut painter);
            painter.pop_transform();
        } else {
            painter.push_transform(transform);
            painter.draw_outline_glyph(glyph_id);
            painter.pop_transform();
        }

        CachedColorGlyph {
            pixmap: painter.into_pixmap(),
            bearing_x,
            bearing_y,
        }
    }

    /// Ensure a glyph is in the cache, rendering it if needed.
    fn ensure_cached(&mut self, glyph_id: u32) {
        if !self.cache.contains_key(&glyph_id) {
            let tile = self.render_glyph(GlyphId::new(glyph_id));
            self.cache.insert(glyph_id, tile);
        }
    }

    /// Render a string to a GrayImage using cached glyph tiles.
    ///
    /// Returns the serialized glyph buffer (for dedup) and the rendered image.
    pub fn render_string(&mut self, string: &str) -> Option<(String, GrayImage)> {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(string);
        let shaper = self
            .shaper_data
            .shaper(&self.font)
            .instance(Some(&self.instance))
            .build();

        let output = if let Some(plan) = &self.plan {
            if let Some(script) = plan.script() {
                buffer.set_script(script);
            }
            buffer.set_direction(plan.direction());
            if let Some(lang) = plan.language() {
                buffer.set_language(lang.clone());
            }
            shaper.shape_with_plan(plan, buffer, &[])
        } else {
            buffer.guess_segment_properties();
            shaper.shape(buffer, &[])
        };

        let upem = self.font.head().unwrap().units_per_em() as f32;
        let factor = self.scale / upem;

        let positions = output.glyph_positions();
        let infos = output.glyph_infos();

        let mut serialized_buffer = String::new();
        let mut glyphs: Vec<(u32, f32, f32)> = Vec::with_capacity(positions.len());
        let mut cursor = 0.0_f32;

        for (position, info) in positions.iter().zip(infos) {
            let px_x = cursor + (position.x_offset as f32 * factor);
            let px_y = position.y_offset as f32 * factor;
            glyphs.push((info.glyph_id, px_x, px_y));

            serialized_buffer.push_str(&format!("{}", info.glyph_id));
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

        // Ensure all glyphs for this word are cached
        for &(glyph_id, _, _) in &glyphs {
            self.ensure_cached(glyph_id);
        }

        // Image dimensions from font metrics
        let size = Size::new(self.scale);
        let metrics = self.font.metrics(size, self.location);
        let ascent = metrics.ascent;
        let descent = metrics.descent;
        let height = ((ascent - descent).ceil() as u32).max(1);
        let width = (cursor.ceil() as u32).max(1);

        let mut word_pixmap = Pixmap::new(width, height).unwrap();

        // Composite cached tiles onto the word pixmap
        for &(glyph_id, px_x, px_y) in &glyphs {
            if let Some(tile) = self.cache.get(&glyph_id) {
                // The tile was rendered with its origin at (-bearing_x, bearing_y).
                // In the word pixmap, the glyph origin is at (px_x, ascent - px_y).
                // So the tile's top-left corner goes at:
                let dest_x = (px_x + tile.bearing_x).round() as i32;
                let dest_y = ((ascent - px_y) - tile.bearing_y).round() as i32;

                word_pixmap.draw_pixmap(
                    dest_x,
                    dest_y,
                    tile.pixmap.as_ref(),
                    &PixmapPaint::default(),
                    TsTransform::identity(),
                    None,
                );
            }
        }

        // Convert premultiplied RGBA to grayscale via luminance
        let mut img = GrayImage::new(width, height);
        for (i, px) in word_pixmap.pixels().iter().enumerate() {
            let x = (i as u32) % width;
            let y = (i as u32) / width;
            let gray =
                0.299 * px.red() as f32 + 0.587 * px.green() as f32 + 0.114 * px.blue() as f32;
            img.put_pixel(x, y, Luma([gray.round().min(255.0) as u8]));
        }

        Some((serialized_buffer, img))
    }
}

/// Read the first CPAL palette from a font.
fn read_cpal_palette(font: &skrifa::FontRef) -> Vec<PaletteColor> {
    let cpal = match font.cpal() {
        Ok(cpal) => cpal,
        Err(_) => return vec![],
    };
    let num_entries = cpal.num_palette_entries();
    let color_records = match cpal.color_records_array() {
        Some(Ok(records)) => records,
        _ => return vec![],
    };
    (0..num_entries)
        .map(|i| {
            let rec = color_records[i as usize];
            PaletteColor {
                r: rec.red,
                g: rec.green,
                b: rec.blue,
                a: rec.alpha,
            }
        })
        .collect()
}
