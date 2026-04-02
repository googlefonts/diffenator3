use read_fonts::types::BoundingBox;
use skrifa::{
    color::{Brush, ColorPainter, CompositeMode, Transform},
    instance::Size,
    outline::OutlinePen,
    prelude::LocationRef,
    GlyphId,
};
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, PixmapPaint,
    RadialGradient, SpreadMode, SweepGradient, Transform as TsTransform,
};

#[derive(Clone, Copy)]
pub(crate) struct PaletteColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

fn resolve_color(palette: &[PaletteColor], palette_index: u16, alpha: f32) -> Color {
    let c = palette
        .get(palette_index as usize)
        .copied()
        .unwrap_or(PaletteColor {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        });
    Color::from_rgba8(c.r, c.g, c.b, (c.a as f32 * alpha) as u8)
}

fn to_spread(extend: skrifa::color::Extend) -> SpreadMode {
    match extend {
        skrifa::color::Extend::Pad => SpreadMode::Pad,
        skrifa::color::Extend::Repeat => SpreadMode::Repeat,
        skrifa::color::Extend::Reflect => SpreadMode::Reflect,
        _ => SpreadMode::Pad,
    }
}

fn to_ts_transform(t: &Transform) -> TsTransform {
    TsTransform::from_row(t.xx, t.yx, t.xy, t.yy, t.dx, t.dy)
}

pub(crate) struct SkiaPainter<'a> {
    palette: &'a [PaletteColor],
    pixmap: Pixmap,
    layer_stack: Vec<(Pixmap, CompositeMode)>,
    transform_stack: Vec<TsTransform>,
    transform: TsTransform,
    clip_stack: Vec<Pixmap>,
    outlines: skrifa::outline::OutlineGlyphCollection<'a>,
    location: LocationRef<'a>,
}

impl<'a> SkiaPainter<'a> {
    pub fn new(
        width: u32,
        height: u32,
        palette: &'a [PaletteColor],
        outlines: skrifa::outline::OutlineGlyphCollection<'a>,
        location: LocationRef<'a>,
    ) -> Self {
        Self {
            palette,
            pixmap: Pixmap::new(width, height).unwrap(),
            layer_stack: Vec::new(),
            transform_stack: Vec::new(),
            transform: TsTransform::identity(),
            clip_stack: Vec::new(),
            outlines,
            location,
        }
    }

    fn current_pixmap(&mut self) -> &mut Pixmap {
        if let Some((ref mut pm, _)) = self.layer_stack.last_mut() {
            pm
        } else {
            &mut self.pixmap
        }
    }

    fn width(&self) -> u32 {
        self.pixmap.width()
    }

    fn height(&self) -> u32 {
        self.pixmap.height()
    }

    fn glyph_path(&self, glyph_id: GlyphId) -> Option<tiny_skia::Path> {
        use skrifa::outline::DrawSettings;
        let glyph = self.outlines.get(glyph_id)?;
        let mut pen = TsPathPen::default();
        let settings = DrawSettings::unhinted(Size::unscaled(), self.location);
        glyph.draw(settings, &mut pen).ok()?;
        pen.builder.finish()
    }

    /// Consume the painter and return the raw RGBA pixmap.
    pub fn into_pixmap(self) -> Pixmap {
        self.pixmap
    }

    /// Draw an outline glyph as a solid opaque white shape on the pixmap.
    ///
    /// Used as fallback for non-color glyphs in a mixed color/outline string.
    pub fn draw_outline_glyph(&mut self, glyph_id: GlyphId) {
        if let Some(path) = self.glyph_path(glyph_id) {
            let mut paint = Paint::default();
            paint.set_color(Color::WHITE);
            let transform = self.transform;
            let pm = self.current_pixmap();
            pm.fill_path(&path, &paint, FillRule::Winding, transform, None);
        }
    }
}

#[derive(Default)]
struct TsPathPen {
    builder: PathBuilder,
}

impl OutlinePen for TsPathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.builder.quad_to(cx0, cy0, x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.builder.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }
    fn close(&mut self) {
        self.builder.close();
    }
}

impl<'a> ColorPainter for SkiaPainter<'a> {
    fn push_transform(&mut self, transform: Transform) {
        self.transform_stack.push(self.transform);
        self.transform = self.transform.post_concat(to_ts_transform(&transform));
    }

    fn pop_transform(&mut self) {
        if let Some(t) = self.transform_stack.pop() {
            self.transform = t;
        }
    }

    fn push_clip_glyph(&mut self, glyph_id: GlyphId) {
        let w = self.width();
        let h = self.height();
        let mut mask = Pixmap::new(w, h).unwrap();
        if let Some(path) = self.glyph_path(glyph_id) {
            let mut paint = Paint::default();
            paint.set_color(Color::WHITE);
            mask.fill_path(&path, &paint, FillRule::Winding, self.transform, None);
        }
        self.clip_stack.push(mask);
    }

    fn push_clip_box(&mut self, clip_box: BoundingBox<f32>) {
        let w = self.width();
        let h = self.height();
        let mut mask = Pixmap::new(w, h).unwrap();
        let mut pb = PathBuilder::new();
        if let Some(rect) = tiny_skia::Rect::from_ltrb(
            clip_box.x_min,
            clip_box.y_min,
            clip_box.x_max,
            clip_box.y_max,
        ) {
            pb.push_rect(rect);
        }
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(Color::WHITE);
            mask.fill_path(&path, &paint, FillRule::Winding, self.transform, None);
        }
        self.clip_stack.push(mask);
    }

    fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    fn fill(&mut self, brush: Brush<'_>) {
        let w = self.width();
        let h = self.height();

        let mut tmp = Pixmap::new(w, h).unwrap();
        let full_rect = tiny_skia::Rect::from_xywh(0.0, 0.0, w as f32, h as f32).unwrap();
        let mut pb = PathBuilder::new();
        pb.push_rect(full_rect);
        let path = pb.finish().unwrap();

        match brush {
            Brush::Solid {
                palette_index,
                alpha,
            } => {
                let color = resolve_color(self.palette, palette_index, alpha);
                let mut paint = Paint::default();
                paint.set_color(color);
                tmp.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    TsTransform::identity(),
                    None,
                );
            }
            Brush::LinearGradient {
                p0,
                p1,
                color_stops,
                extend,
            } => {
                let stops: Vec<GradientStop> = color_stops
                    .iter()
                    .map(|cs| {
                        let c = resolve_color(self.palette, cs.palette_index, cs.alpha);
                        GradientStop::new(cs.offset, c)
                    })
                    .collect();
                if let Some(shader) = LinearGradient::new(
                    tiny_skia::Point::from_xy(p0.x, p0.y),
                    tiny_skia::Point::from_xy(p1.x, p1.y),
                    stops,
                    to_spread(extend),
                    self.transform,
                ) {
                    let paint = Paint {
                        shader,
                        ..Paint::default()
                    };
                    tmp.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        TsTransform::identity(),
                        None,
                    );
                }
            }
            Brush::RadialGradient {
                c0,
                r0,
                c1,
                r1,
                color_stops,
                extend,
            } => {
                let stops: Vec<GradientStop> = color_stops
                    .iter()
                    .map(|cs| {
                        let c = resolve_color(self.palette, cs.palette_index, cs.alpha);
                        GradientStop::new(cs.offset, c)
                    })
                    .collect();
                if let Some(shader) = RadialGradient::new(
                    tiny_skia::Point::from_xy(c0.x, c0.y),
                    r0,
                    tiny_skia::Point::from_xy(c1.x, c1.y),
                    r1,
                    stops,
                    to_spread(extend),
                    self.transform,
                ) {
                    let paint = Paint {
                        shader,
                        ..Paint::default()
                    };
                    tmp.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        TsTransform::identity(),
                        None,
                    );
                }
            }
            Brush::SweepGradient {
                c0,
                start_angle,
                end_angle,
                color_stops,
                extend,
            } => {
                let stops: Vec<GradientStop> = color_stops
                    .iter()
                    .map(|cs| {
                        let c = resolve_color(self.palette, cs.palette_index, cs.alpha);
                        GradientStop::new(cs.offset, c)
                    })
                    .collect();
                if let Some(shader) = SweepGradient::new(
                    tiny_skia::Point::from_xy(c0.x, c0.y),
                    start_angle,
                    end_angle,
                    stops,
                    to_spread(extend),
                    self.transform,
                ) {
                    let paint = Paint {
                        shader,
                        ..Paint::default()
                    };
                    tmp.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        TsTransform::identity(),
                        None,
                    );
                }
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }

        // Apply clip mask
        if let Some(clip_mask) = self.clip_stack.last() {
            let tmp_pixels = tmp.pixels_mut();
            let mask_pixels = clip_mask.pixels();
            for (pixel, mask) in tmp_pixels.iter_mut().zip(mask_pixels.iter()) {
                let mask_alpha = mask.alpha();
                if mask_alpha == 0 {
                    *pixel = tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, 0).unwrap();
                } else if mask_alpha < 255 {
                    let a = mask_alpha as f32 / 255.0;
                    *pixel = tiny_skia::PremultipliedColorU8::from_rgba(
                        (pixel.red() as f32 * a) as u8,
                        (pixel.green() as f32 * a) as u8,
                        (pixel.blue() as f32 * a) as u8,
                        (pixel.alpha() as f32 * a) as u8,
                    )
                    .unwrap();
                }
            }
        }

        let pm = self.current_pixmap();
        pm.draw_pixmap(
            0,
            0,
            tmp.as_ref(),
            &PixmapPaint::default(),
            TsTransform::identity(),
            None,
        );
    }

    fn push_layer(&mut self, composite_mode: CompositeMode) {
        let w = self.width();
        let h = self.height();
        let layer = Pixmap::new(w, h).unwrap();
        self.layer_stack.push((layer, composite_mode));
    }

    fn pop_layer(&mut self) {
        if let Some((layer, mode)) = self.layer_stack.pop() {
            let blend = match mode {
                CompositeMode::SrcOver => tiny_skia::BlendMode::SourceOver,
                CompositeMode::Screen => tiny_skia::BlendMode::Screen,
                CompositeMode::Overlay => tiny_skia::BlendMode::Overlay,
                CompositeMode::Darken => tiny_skia::BlendMode::Darken,
                CompositeMode::Lighten => tiny_skia::BlendMode::Lighten,
                CompositeMode::ColorDodge => tiny_skia::BlendMode::ColorDodge,
                CompositeMode::ColorBurn => tiny_skia::BlendMode::ColorBurn,
                CompositeMode::HardLight => tiny_skia::BlendMode::HardLight,
                CompositeMode::SoftLight => tiny_skia::BlendMode::SoftLight,
                CompositeMode::Difference => tiny_skia::BlendMode::Difference,
                CompositeMode::Exclusion => tiny_skia::BlendMode::Exclusion,
                CompositeMode::Multiply => tiny_skia::BlendMode::Multiply,
                CompositeMode::HslHue => tiny_skia::BlendMode::Hue,
                CompositeMode::HslSaturation => tiny_skia::BlendMode::Saturation,
                CompositeMode::HslColor => tiny_skia::BlendMode::Color,
                CompositeMode::HslLuminosity => tiny_skia::BlendMode::Luminosity,
                CompositeMode::Src => tiny_skia::BlendMode::Source,
                CompositeMode::Dest => tiny_skia::BlendMode::Destination,
                CompositeMode::Clear => tiny_skia::BlendMode::Clear,
                CompositeMode::SrcIn => tiny_skia::BlendMode::SourceIn,
                CompositeMode::DestIn => tiny_skia::BlendMode::DestinationIn,
                CompositeMode::SrcOut => tiny_skia::BlendMode::SourceOut,
                CompositeMode::DestOut => tiny_skia::BlendMode::DestinationOut,
                CompositeMode::SrcAtop => tiny_skia::BlendMode::SourceAtop,
                CompositeMode::DestAtop => tiny_skia::BlendMode::DestinationAtop,
                CompositeMode::DestOver => tiny_skia::BlendMode::DestinationOver,
                CompositeMode::Xor => tiny_skia::BlendMode::Xor,
                CompositeMode::Plus => tiny_skia::BlendMode::Plus,
                _ => tiny_skia::BlendMode::SourceOver,
            };
            let paint = PixmapPaint {
                blend_mode: blend,
                ..PixmapPaint::default()
            };
            let pm = self.current_pixmap();
            pm.draw_pixmap(0, 0, layer.as_ref(), &paint, TsTransform::identity(), None);
        }
    }
}
