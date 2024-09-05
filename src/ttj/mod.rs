use crate::ttj::jsondiff::diff;
use crate::ttj::serializefont::ToValue;
use read_fonts::traversal::SomeTable;
use read_fonts::{FontRef, TableProvider};
use serde_json::{Map, Value};
use skrifa::charmap::Charmap;
use skrifa::string::StringId;
use skrifa::{GlyphId, GlyphId16, MetadataProvider};

mod gdef;
pub mod jsondiff;
mod layout;
mod serializefont;

fn serialize_name_table<'a>(font: &(impl MetadataProvider<'a> + TableProvider<'a>)) -> Value {
    let mut map = Map::new();
    if let Ok(name) = font.name() {
        let mut ids: Vec<StringId> = name.name_record().iter().map(|x| x.name_id()).collect();
        ids.sort_by_key(|id| id.to_u16());
        for id in ids {
            let strings = font.localized_strings(id);
            if strings.clone().next().is_some() {
                let mut localized = Map::new();
                for string in font.localized_strings(id) {
                    localized.insert(
                        string.language().unwrap_or("default").to_string(),
                        Value::String(string.to_string()),
                    );
                }
                map.insert(id.to_string(), Value::Object(localized));
            }
        }
    }
    Value::Object(map)
}

fn gid_to_name<'a>(font: &impl TableProvider<'a>, gid: GlyphId) -> String {
    if let Ok(gid16) = TryInto::<GlyphId16>::try_into(gid) {
        if let Ok(Some(name)) = font
            .post()
            .map(|post| post.glyph_name(gid16).map(|x| x.to_string()))
        {
            return name;
        }
    }
    format!("gid{:}", gid)
}

fn serialize_cmap_table<'a>(font: &impl TableProvider<'a>, names: &[String]) -> Value {
    let charmap = Charmap::new(font);
    let mut map: Map<String, Value> = Map::new();
    for (codepoint, gid) in charmap.mappings() {
        let name = names
            .get(gid.to_u32() as usize)
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("gid{}", gid));
        map.insert(format!("U+{:04X}", codepoint), Value::String(name));
    }
    Value::Object(map)
}

fn serialize_hmtx_table<'a>(font: &impl TableProvider<'a>, names: &[String]) -> Value {
    let mut map = Map::new();
    if let Ok(hmtx) = font.hmtx() {
        let widths = hmtx.h_metrics();
        let long_metrics = widths.len();
        for gid in 0..font.maxp().unwrap().num_glyphs() {
            let name = names
                .get(gid as usize)
                .map(|n| n.to_string())
                .unwrap_or_else(|| format!("gid{}", gid));
            if gid < (long_metrics as u16) {
                if let Some((width, lsb)) = widths
                    .get(gid as usize)
                    .map(|lm| (lm.advance(), lm.side_bearing()))
                {
                    map.insert(
                        name,
                        Value::Object(
                            vec![
                                ("width".to_string(), Value::Number(width.into())),
                                ("lsb".to_string(), Value::Number(lsb.into())),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                    );
                }
            } else {
                // XXX
            }
        }
    }
    Value::Object(map)
}

pub fn font_to_json(font: &FontRef, glyphmap: Option<&[String]>) -> Value {
    let glyphmap = if let Some(glyphmap) = glyphmap {
        glyphmap
    } else {
        &(0..font.maxp().unwrap().num_glyphs())
            .map(|gid| gid_to_name(font, GlyphId::new(gid as u32)))
            .collect::<Vec<String>>()
    };
    let mut map = Map::new();

    for table in font.table_directory.table_records().iter() {
        let key = table.tag().to_string();
        let value = match table.tag().into_bytes().as_ref() {
            b"head" => font.head().map(|t| <dyn SomeTable>::serialize(&t)),
            // b"name" => font.name().map(|t| serialize_name_table(&t)),
            b"hhea" => font.hhea().map(|t| <dyn SomeTable>::serialize(&t)),
            b"vhea" => font.vhea().map(|t| <dyn SomeTable>::serialize(&t)),
            // b"hmtx" => font.hmtx().map(|t| <dyn SomeTable>::serialize(&t)),
            b"vmtx" => font.vmtx().map(|t| <dyn SomeTable>::serialize(&t)),
            b"fvar" => font.fvar().map(|t| <dyn SomeTable>::serialize(&t)),
            b"avar" => font.avar().map(|t| <dyn SomeTable>::serialize(&t)),
            b"HVAR" => font.hvar().map(|t| <dyn SomeTable>::serialize(&t)),
            b"VVAR" => font.vvar().map(|t| <dyn SomeTable>::serialize(&t)),
            b"MVAR" => font.mvar().map(|t| <dyn SomeTable>::serialize(&t)),
            b"maxp" => font.maxp().map(|t| <dyn SomeTable>::serialize(&t)),
            b"OS/2" => font.os2().map(|t| <dyn SomeTable>::serialize(&t)),
            b"post" => font.post().map(|t| <dyn SomeTable>::serialize(&t)),
            b"loca" => font.loca(None).map(|t| <dyn SomeTable>::serialize(&t)),
            b"glyf" => font.glyf().map(|t| <dyn SomeTable>::serialize(&t)),
            b"gvar" => font.gvar().map(|t| <dyn SomeTable>::serialize(&t)),
            // b"gasp" => {
            //     let gasp: Result<tables::gasp::Gasp, _> = font.expect_table();
            //     gasp.map(|t| <dyn SomeTable>::serialize(&t))
            // }
            // b"cmap" => font.cmap().map(|t| <dyn SomeTable>::serialize(&t)),
            // b"GDEF" => font.gdef().map(|t| <dyn SomeTable>::serialize(&t)),
            // b"GPOS" => font.gpos().map(|t| <dyn SomeTable>::serialize(&t)),
            // b"GSUB" => font.gsub().map(|t| <dyn SomeTable>::serialize(&t)),
            b"COLR" => font.colr().map(|t| <dyn SomeTable>::serialize(&t)),
            b"CPAL" => font.cpal().map(|t| <dyn SomeTable>::serialize(&t)),
            b"STAT" => font.stat().map(|t| <dyn SomeTable>::serialize(&t)),
            _ => font.expect_data_for_tag(table.tag()).map(|tabledata| {
                Value::Array(
                    tabledata
                        .as_ref()
                        .iter()
                        .map(|&x| Value::Number(x.into()))
                        .collect(),
                )
            }),
        };
        map.insert(
            key,
            value.unwrap_or_else(|_| Value::String("Could not parse".to_string())),
        );
        // }
    }
    map.insert("name".to_string(), serialize_name_table(font));
    map.insert("cmap".to_string(), serialize_cmap_table(font, glyphmap));
    map.insert("hmtx".to_string(), serialize_hmtx_table(font, glyphmap));
    map.insert("GDEF".to_string(), gdef::serialize_gdef_table(font));
    map.insert("GPOS".to_string(), layout::serialize_gpos_table(font));
    map.insert("GSUB".to_string(), layout::serialize_gsub_table(font));
    Value::Object(map)
}

pub fn table_diff(font_a: &FontRef, font_b: &FontRef) -> Value {
    let glyphmap_a: Vec<String> = (0..font_a.maxp().unwrap().num_glyphs())
        .map(|gid| gid_to_name(font_a, GlyphId::new(gid as u32)))
        .collect();
    let glyphmap_b: Vec<String> = (0..font_b.maxp().unwrap().num_glyphs())
        .map(|gid| gid_to_name(font_b, GlyphId::new(gid as u32)))
        .collect();
    let count_glyphname_differences = glyphmap_a
        .iter()
        .zip(glyphmap_b.iter())
        .filter(|(a, b)| a != b)
        .count();
    let big_difference = count_glyphname_differences > glyphmap_a.len() / 2;
    if big_difference {
        println!("Glyph names differ dramatically between fonts, using font names from font A");
    }

    diff(
        &font_to_json(font_a, Some(&glyphmap_a)),
        &font_to_json(
            font_b,
            Some(if big_difference {
                &glyphmap_a
            } else {
                &glyphmap_b
            }),
        ),
    )
}

// fn main() {
//     let bytes = std::fs::read("Nunito[wght,ital].ttf").expect("Can't read");
//     let font1 = FontRef::new(&bytes).expect("Can't parse");
//     let bytes = std::fs::read("Nunito[wght].ttf").expect("Can't read");
//     let font2 = FontRef::new(&bytes).expect("Can't parse");
//     let left = font_to_json(&font1);
//     let right = font_to_json(&font2);
//     println!(
//         "{:}",
//         serde_json::to_string_pretty(&diff(&left, &right)).unwrap()
//     );
// }
