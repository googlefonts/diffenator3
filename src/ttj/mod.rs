use crate::ttj::{jsondiff::diff, serializefont::ToValue};
use read_fonts::{traversal::SomeTable, FontRef, TableProvider};
use serde_json::{Map, Value};
use skrifa::{charmap::Charmap, string::StringId, GlyphId, MetadataProvider};

pub mod jsondiff;
mod serializefont;

fn serialize_name_table<'a>(font: &impl MetadataProvider<'a>) -> Value {
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
    if let Ok(Some(name)) = font
        .post()
        .map(|post| post.glyph_name(gid).map(|x| x.to_string()))
    {
        name
    } else {
        format!("gid{:}", gid)
    }
}

fn serialize_cmap_table<'a>(font: &impl TableProvider<'a>) -> Value {
    let charmap = Charmap::new(font);
    let mut map = Map::new();
    for (codepoint, glyph_id) in charmap.mappings() {
        let name = gid_to_name(font, glyph_id);
        map.insert(format!("U+{:04X}", codepoint), Value::String(name));
    }
    Value::Object(map)
}

fn serialize_hmtx_table<'a>(font: &impl TableProvider<'a>) -> Value {
    let mut map = Map::new();
    if let Ok(hmtx) = font.hmtx() {
        let widths = hmtx.h_metrics();
        let long_metrics = widths.len();
        for gid in 0..font.maxp().unwrap().num_glyphs() {
            let name = gid_to_name(font, GlyphId::new(gid));
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

pub fn font_to_json(font: &FontRef) -> Value {
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
            // b"cmap" => font.cmap().map(|t| <dyn SomeTable>::serialize(&t)),
            b"GDEF" => font.gdef().map(|t| <dyn SomeTable>::serialize(&t)),
            b"GPOS" => font.gpos().map(|t| <dyn SomeTable>::serialize(&t)),
            b"GSUB" => font.gsub().map(|t| <dyn SomeTable>::serialize(&t)),
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
    map.insert("cmap".to_string(), serialize_cmap_table(font));
    map.insert("hmtx".to_string(), serialize_hmtx_table(font));
    Value::Object(map)
}

pub fn table_diff(font_a: &FontRef, font_b: &FontRef) -> Value {
    diff(&font_to_json(font_a), &font_to_json(font_b))
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
