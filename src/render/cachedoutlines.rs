/// Speed up the drawing process by caching the outlines of glyphs.
use std::collections::HashMap;

use skrifa::{
    instance::Size, outline::DrawSettings, prelude::LocationRef, GlyphId, OutlineGlyphCollection,
};
use zeno::Command;

use super::utils::RecordingPen;

pub(crate) struct CachedOutlineGlyphCollection<'a> {
    source: OutlineGlyphCollection<'a>,
    cache: HashMap<GlyphId, Vec<Command>>,
    size: Size,
    location: LocationRef<'a>,
}

impl<'a> CachedOutlineGlyphCollection<'a> {
    pub fn new(source: OutlineGlyphCollection<'a>, size: Size, location: LocationRef<'a>) -> Self {
        Self {
            source,
            size,
            location,
            cache: HashMap::new(),
        }
    }

    pub fn get(&mut self, glyph_id: GlyphId) -> Option<&Vec<Command>> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.cache.entry(glyph_id) {
            let outlined = self.source.get(glyph_id).unwrap();
            let mut pen = RecordingPen::default();
            let settings = DrawSettings::unhinted(self.size, self.location);
            let _ = outlined.draw(settings, &mut pen);
            e.insert(pen.buffer);
        }
        self.cache.get(&glyph_id)
    }

    pub fn draw(&mut self, glyph_id: GlyphId, pen: &mut RecordingPen) {
        let commands = self.get(glyph_id).unwrap();
        pen.buffer.extend(commands.iter().cloned());
    }
}
