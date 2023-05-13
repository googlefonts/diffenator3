use read_fonts::traversal::{FieldType, SomeArray, SomeTable};
use serde_json::{Map, Number, Value};

pub(crate) trait ToValue {
    fn serialize(&self) -> Value;
}

impl<'a> ToValue for FieldType<'a> {
    fn serialize(&self) -> Value {
        match self {
            Self::I8(arg0) => Value::Number((*arg0).into()),
            Self::U8(arg0) => Value::Number((*arg0).into()),
            Self::I16(arg0) => Value::Number((*arg0).into()),
            Self::U16(arg0) => Value::Number((*arg0).into()),
            Self::I32(arg0) => Value::Number((*arg0).into()),
            Self::U32(arg0) => Value::Number((*arg0).into()),
            Self::U24(arg0) => {
                let u: u32 = (*arg0).into();
                Value::Number(u.into())
            }
            Self::Tag(arg0) => Value::String(arg0.to_string()),
            Self::FWord(arg0) => Value::Number(arg0.to_i16().into()),
            Self::UfWord(arg0) => Value::Number(arg0.to_u16().into()),
            Self::MajorMinor(arg0) => Value::String(format!("{}.{}", arg0.major, arg0.minor)),
            Self::Version16Dot16(arg0) => Value::String(format!("{}", *arg0)),
            Self::F2Dot14(arg0) => Value::Number(Number::from_f64(arg0.to_f32() as f64).unwrap()),
            Self::Fixed(arg0) => Value::Number(Number::from(arg0.to_i32())),
            Self::LongDateTime(arg0) => Value::Number(arg0.as_secs().into()),
            Self::GlyphId(arg0) => Value::String(format!("g{}", arg0.to_u16())),
            Self::NameId(arg0) => Value::String(arg0.to_string()),
            Self::StringOffset(string) => match &string.target {
                Ok(arg0) => Value::String(arg0.as_ref().iter_chars().collect()),
                Err(_) => Value::Null,
            },
            Self::ArrayOffset(array) => match &array.target {
                Ok(arg0) => arg0.as_ref().serialize(),
                Err(_) => Value::Null,
            },
            Self::BareOffset(arg0) => Value::String(format!("0x{:04X}", arg0.to_u32())),
            Self::ResolvedOffset(arg0) => {
                arg0.target.as_ref().map_or(Value::Null, |t| t.serialize())
            }
            Self::Record(arg0) => (arg0 as &(dyn SomeTable<'a> + 'a)).serialize(),
            Self::Array(arg0) => arg0.serialize(),
            Self::Unknown => Value::String("no repr available".to_string()),
        }
    }
}

impl<'a> ToValue for dyn SomeArray<'a> + 'a {
    fn serialize(&self) -> Value {
        let mut out = vec![];
        let mut idx = 0;
        while let Some(val) = self.get(idx) {
            out.push(val.serialize());
            idx += 1;
        }
        Value::Array(out)
    }
}

impl<'a> ToValue for dyn SomeTable<'a> + 'a {
    fn serialize(&self) -> Value {
        let mut field_num = 0;
        let mut map = Map::new();
        while let Some(field) = self.get_field(field_num) {
            map.insert(field.name.to_string(), field.value.serialize());
            field_num += 1;
        }
        Value::Object(map)
    }
}
