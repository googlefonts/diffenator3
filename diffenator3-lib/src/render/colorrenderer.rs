/// Color glyph rendering for fonts with COLR tables.
///
/// This module provides [`ColorRenderer`], a renderer for fonts containing a
/// COLR table (color glyphs). It renders each unique glyph once via
/// [`SkiaPainter`] and caches the resulting RGBA tile. Subsequent occurrences
/// of the same glyph are composited from the cache, avoiding repeated paint
/// graph traversals.
use std::{any::Any, collections::HashMap};

use harfrust::{Direction, Script, ShapePlan, ShaperData, ShaperInstance, Variation};
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
use crate::{
    dfont::DFont,
    render::{renderer::AnyRenderer, shaper::DrawBuffer},
};

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
        let mut painter = SkiaPainter::new(tile_w, tile_h, &self.palette, outlines, self.location);

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
}

impl AnyRenderer for ColorRenderer<'_> {
    fn fast_equivalence_check(&self, _data1: &dyn Any, _data2: &dyn Any) -> bool {
        false // We can't do one cheaply
    }

    fn string_to_stage1_rendering(&mut self, string: &str) -> Option<(String, Box<dyn Any>)> {
        let buffer = DrawBuffer::new_from_font(
            &self.shaper_data,
            &self.font,
            string,
            Some(&self.instance),
            self.scale,
            self.plan.as_ref(),
        );
        // Ensure all glyphs for this word are cached
        for glyph in buffer.iter() {
            self.ensure_cached(glyph.glyph_id.into());
        }
        Some((buffer.serialize(), Box::new(buffer)))
    }

    /// Render a string to a GrayImage using cached glyph tiles.
    ///
    /// Returns the serialized glyph buffer (for dedup) and the rendered image.
    fn final_rendering(&mut self, data: &dyn Any) -> GrayImage {
        let buffer = data
            .downcast_ref::<DrawBuffer>()
            .expect("final_rendering: expected DrawBuffer from string_to_stage1_rendering");

        // Image dimensions from font metrics
        let size = Size::new(self.scale);
        let metrics = self.font.metrics(size, self.location);
        let ascent = metrics.ascent;
        let descent = metrics.descent;
        let height = ((ascent - descent).ceil() as u32).max(1);
        let width = (buffer.cursor_xmax.ceil() as u32).max(1);

        let mut word_pixmap = Pixmap::new(width, height).unwrap();

        // Composite cached tiles onto the word pixmap
        for glyph in buffer.iter() {
            if let Some(tile) = self.cache.get(&glyph.glyph_id.into()) {
                // The tile was rendered with its origin at (-bearing_x, bearing_y).
                // In the word pixmap, the glyph origin is at (px_x, ascent - px_y).
                // So the tile's top-left corner goes at:
                let dest_x = (glyph.x_pos + tile.bearing_x).round() as i32;
                let dest_y = ((ascent - glyph.y_pos) - tile.bearing_y).round() as i32;

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

        img
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

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_font() -> Vec<u8> {
        std::fs::read("test-data/Nabla-subset.ttf").expect("missing test font")
    }

    #[test]
    fn colrv1_render_produces_non_empty_image() {
        let data = load_test_font();
        let dfont = DFont::new(&data);
        let mut renderer = ColorRenderer::new(&dfont, 32.0, None, None);
        let (buffer, data) = renderer
            .string_to_stage1_rendering("hello")
            .expect("string_to_stage1_rendering returned None");
        let img = renderer.final_rendering(&*data);

        assert!(!buffer.is_empty(), "serialized buffer should not be empty");
        assert!(img.width() > 0 && img.height() > 0, "image has zero size");

        let non_zero = img.pixels().filter(|p| p.0[0] > 0).count();
        assert!(non_zero > 0, "image is completely blank");
    }

    #[test]
    fn colrv1_glyph_cache_is_reused() {
        let data = load_test_font();
        let dfont = DFont::new(&data);
        let mut renderer = ColorRenderer::new(&dfont, 32.0, None, None);

        // "ll" shares the same glyph; after rendering, the cache should contain it
        renderer.string_to_stage1_rendering("hello").unwrap();
        let cache_size_after_hello = renderer.cache.len();

        // "lo" reuses 'l' and 'o' which are already cached
        renderer.string_to_stage1_rendering("lo").unwrap();
        let cache_size_after_lo = renderer.cache.len();

        assert_eq!(
            cache_size_after_hello, cache_size_after_lo,
            "cache grew when all glyphs should already have been cached"
        );
    }

    #[test]
    fn colrv1_cached_tiles_contain_color() {
        let data = load_test_font();
        let dfont = DFont::new(&data);
        let mut renderer = ColorRenderer::new(&dfont, 32.0, None, None);

        renderer.string_to_stage1_rendering("hello").unwrap();

        // At least one cached tile should have pixels where the RGB channels
        // differ from each other, proving we're rendering actual color, not
        // just grayscale alpha.
        let has_color = renderer.cache.values().any(|tile| {
            tile.pixmap.pixels().iter().any(|px| {
                let (r, g, b) = (px.red(), px.green(), px.blue());
                px.alpha() > 0 && !(r == g && g == b)
            })
        });
        assert!(has_color, "no color pixels found in cached glyph tiles");
    }

    #[test]
    fn colrv1_same_font_has_zero_diff() {
        let data = load_test_font();
        let dfont = DFont::new(&data);
        let mut renderer_a = ColorRenderer::new(&dfont, 32.0, None, None);
        let mut renderer_b = ColorRenderer::new(&dfont, 32.0, None, None);

        let (_, data_a) = renderer_a.string_to_stage1_rendering("world").unwrap();
        let img_a = renderer_a.final_rendering(&*data_a);
        let (_, data_b) = renderer_b.string_to_stage1_rendering("world").unwrap();
        let img_b = renderer_b.final_rendering(&*data_b);

        let diff = crate::render::utils::count_differences(img_a, img_b, 0);
        assert_eq!(diff, 0, "same font should produce identical images");
    }

    #[test]
    fn scratch_old_render_string_path() {
        use crate::render::shaper::DrawBuffer;
        let data = load_test_font();
        let dfont = DFont::new(&data);
        let mut renderer = ColorRenderer::new(&dfont, 32.0, None, None);

        // Exactly what the removed `render_string` used to do:
        let buffer = DrawBuffer::new_from_font(
            &renderer.shaper_data,
            &renderer.font,
            "hello",
            Some(&renderer.instance),
            renderer.scale,
            renderer.plan.as_ref(),
        );
        for glyph in buffer.iter() {
            renderer.ensure_cached(glyph.glyph_id.into());
        }
        let img = renderer.final_rendering(&buffer as &dyn Any);
        let non_zero = img.pixels().filter(|p| p.0[0] > 0).count();
        panic!(
            "old-path: serialized='{}' dims={}x{} non_zero={}",
            buffer.serialize(),
            img.width(),
            img.height(),
            non_zero
        );
    }
}
