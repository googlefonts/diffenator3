use indexmap::IndexSet;
use serde_json::{json, Map, Value};

pub trait Substantial {
    fn is_something(&self) -> bool;
}
impl Substantial for Value {
    fn is_something(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(x) => *x,
            Value::Number(x) => x.as_f64().unwrap_or(0.0).abs() > f64::EPSILON,
            Value::String(x) => !x.is_empty(),
            Value::Array(x) => !x.is_empty(),
            Value::Object(x) => !x.is_empty(),
        }
    }
}

pub(crate) fn diff(this: &Value, other: &Value) -> Value {
    match (this, other) {
        (Value::Null, Value::Null) => Value::Null,
        (Value::Number(l), Value::Number(r)) => {
            if l == r {
                Value::Null
            } else {
                Value::Array(vec![this.clone(), other.clone()])
            }
        }
        (Value::Bool(l), Value::Bool(r)) => {
            if l == r {
                Value::Null
            } else {
                Value::Array(vec![this.clone(), other.clone()])
            }
        }
        (Value::String(l), Value::String(r)) => {
            if l == r {
                Value::Null
            } else {
                Value::Array(vec![this.clone(), other.clone()])
            }
        }
        (Value::Array(l), Value::Array(r)) => {
            let mut res = Map::new();
            for i in 0..(l.len().max(r.len())) {
                let difference = diff(
                    l.get(i).unwrap_or(&Value::Null),
                    r.get(i).unwrap_or(&Value::Null),
                );
                if difference.is_something() {
                    res.insert(i.to_string(), difference);
                }
            }
            if res.len() > 133 {
                json!({ "error": format!("There are {} changes, check manually!", res.len()) })
            } else {
                Value::Object(res)
            }
        }
        (Value::Object(l), Value::Object(r)) => {
            let mut res = Map::new();
            let mut all_keys = IndexSet::new();
            all_keys.extend(l.keys());
            all_keys.extend(r.keys());
            for key in all_keys {
                let difference = diff(
                    l.get(key).unwrap_or(&Value::Null),
                    r.get(key).unwrap_or(&Value::Null),
                );
                if difference.is_something() {
                    res.insert(key.to_string(), difference);
                }
            }
            if res.is_empty() {
                Value::Null
            } else if res.len() > 133 {
                json!({ "error": format!("There are {} changes, check manually!", res.len()) })
            } else {
                Value::Object(res)
            }
        }
        (_, _) => Value::Array(vec![this.clone(), other.clone()]),
    }
}
