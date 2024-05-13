use lazy_static::lazy_static;
use tera::{Context, Tera};

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        match Tera::new("templates/*") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        }
    };
}

pub fn render_output(value: &serde_json::Value) -> Result<String, tera::Error> {
    TEMPLATES.render("diffenator.html", &Context::from_serialize(value)?)
}
