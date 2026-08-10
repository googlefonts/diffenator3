/// Methods which other people's structs really should have but sadly don't.
use std::collections::BTreeSet;

use read_fonts::tables::{gsub::ClassDef, varc::CoverageTable};
use skrifa::GlyphId16;

pub trait MonkeyPatchClassDef {
    /// Return a list of glyphs in this class
    fn class_glyphs(&self, class: u16, coverage: Option<CoverageTable>) -> Vec<GlyphId16>;
}

impl MonkeyPatchClassDef for ClassDef<'_> {
    fn class_glyphs(&self, class: u16, coverage: Option<CoverageTable>) -> Vec<GlyphId16> {
        if class == 0 {
            // let coverage_map = coverage.unwrap().coverage_map();
            if let Some(coverage) = coverage {
                let all_glyphs: BTreeSet<GlyphId16> = coverage.iter().collect();
                let in_a_class: BTreeSet<GlyphId16> =
                    self.iter().map(|(gid, _a_class)| gid).collect();
                // Remove all the glyphs in assigned class
                all_glyphs.difference(&in_a_class).copied().collect()
            } else {
                panic!("ClassDef has no coverage table and class=0 was requested");
            }
        } else {
            self.iter()
                .filter(move |&(_gid, their_class)| their_class == class)
                .map(|(gid, _)| gid)
                .collect()
        }
    }
}
