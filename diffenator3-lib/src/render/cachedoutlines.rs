/// Speed up the drawing process by caching the outlines of glyphs.
use std::collections::HashMap;

use fontdrasil::coords::NormalizedCoord;
use skrifa::{
    instance::Size, outline::DrawSettings, prelude::LocationRef, GlyphId, OutlineGlyphCollection,
};
use zeno::Command;

use super::utils::RecordingPen;

pub(crate) struct CachedOutlineGlyphCollection<'a> {
    source: OutlineGlyphCollection<'a>,
    cache: HashMap<(GlyphId, Vec<NormalizedCoord>), Vec<Command>>,
    size: Size,
    stats: (usize, usize), // (cache_hits, cache_misses)
}

impl<'a> CachedOutlineGlyphCollection<'a> {
    pub fn new(source: OutlineGlyphCollection<'a>, size: Size) -> Self {
        Self {
            source,
            size,
            cache: HashMap::new(),
            stats: (0, 0),
        }
    }

    pub fn get(
        &mut self,
        glyph_id: GlyphId,
        location: &[NormalizedCoord],
    ) -> Option<&Vec<Command>> {
        if let std::collections::hash_map::Entry::Vacant(e) =
            self.cache.entry((glyph_id, location.to_vec()))
        {
            let outlined = self.source.get(glyph_id).unwrap();
            self.stats.1 += 1;
            let mut pen = RecordingPen::default();
            let skrifa_norm_coords = location.iter().map(|x| x.to_f2dot14()).collect::<Vec<_>>();
            let location_ref = LocationRef::from(skrifa_norm_coords.as_slice());
            let settings = DrawSettings::unhinted(self.size, location_ref);
            let _ = outlined.draw(settings, &mut pen);
            e.insert(pen.buffer);
        } else {
            self.stats.0 += 1;
        }
        self.cache.get(&(glyph_id, location.to_vec()))
    }

    pub fn draw(
        &mut self,
        glyph_id: GlyphId,
        location: &[NormalizedCoord],
        pen: &mut RecordingPen,
    ) {
        let commands = self.get(glyph_id, location).unwrap();
        let matrix = zeno::Transform::translation(pen.offset_x, pen.offset_y);
        pen.buffer
            .extend(commands.iter().map(|c| c.transform(&matrix)));
    }

    pub fn log_stats(&self) {
        log::info!(
            "CachedOutlineGlyphCollection: cache hits: {}, cache misses: {}",
            self.stats.0,
            self.stats.1
        );
    }
}
