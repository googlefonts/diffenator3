use crate::utils::die;
use serde_json::json;
use std::path::{Path, PathBuf};
use tera::{Context, Tera};
use walkdir::WalkDir;

use super::Report;

pub fn report(
    font1_pb: &PathBuf,
    font2_pb: &PathBuf,
    output_dir: &Path,
    diff: Report,
    tera: Tera,
) -> ! {
    // Make output directory
    if !output_dir.exists() {
        std::fs::create_dir(output_dir).expect("Couldn't create output directory");
    }

    // Copy old font to output/old-<existing name>
    let old_font = output_dir.join(format!(
        "old-{}",
        font2_pb.file_name().unwrap().to_str().unwrap()
    ));
    std::fs::copy(font1_pb, &old_font).expect("Couldn't copy old font");
    let new_font = output_dir.join(format!(
        "new-{}",
        font2_pb.file_name().unwrap().to_str().unwrap()
    ));
    std::fs::copy(font2_pb, &new_font).expect("Couldn't copy new font");

    let value = serde_json::to_value(diff).unwrap_or_else(|e| {
        die("serializing diff", e);
    });
    let html = render_output(
        &value,
        old_font.file_name().unwrap().to_str().unwrap(),
        new_font.file_name().unwrap().to_str().unwrap(),
        &tera,
    )
    .unwrap_or_else(|err| die("rendering HTML", err));

    // Write output
    let output_file = output_dir.join("diffenator.html");
    println!("Writing output to {}", output_file.to_str().unwrap());
    std::fs::write(output_file, html).expect("Couldn't write output file");
    std::process::exit(0);
}

/// Instantiate a Tera template engine
///
/// This function also takes care of working out which templates to use. If the user
/// passes a directory for their own templates, these are used. Otherwise, the
/// templates supplied in the binary are copied into the user's home directory,
/// and this directory is used as the template root.
pub fn template_engine(user_templates: Option<&String>, overwrite: bool) -> Tera {
    let homedir = create_user_home_templates_directory(overwrite);
    let mut tera = Tera::new(&format!("{}/*", homedir.to_str().unwrap())).unwrap_or_else(|e| {
        println!("Problem parsing templates: {:?}", e);
        std::process::exit(1)
    });
    if let Some(template_dir) = user_templates {
        for entry in WalkDir::new(template_dir) {
            if entry.as_ref().is_ok_and(|e| e.file_type().is_dir()) {
                continue;
            }
            let path = entry
                .as_ref()
                .unwrap_or_else(|e| {
                    println!("Problem reading template path: {:}", e);
                    std::process::exit(1)
                })
                .path();
            if let Err(e) =
                tera.add_template_file(path, path.strip_prefix(template_dir).unwrap().to_str())
            {
                println!("Problem adding template file: {:}", e);
                std::process::exit(1)
            }
        }
        if let Err(e) = tera.build_inheritance_chains() {
            println!("Problem building inheritance chains: {:}", e);
            std::process::exit(1)
        }
    }
    tera
}

pub fn create_user_home_templates_directory(force: bool) -> PathBuf {
    let home = homedir::my_home()
        .expect("Couldn't got home directory")
        .expect("No home directory found");
    let templates_dir = home.join(".diffenator3/templates");
    if !templates_dir.exists() {
        std::fs::create_dir_all(&templates_dir).unwrap_or_else(|e| {
            println!("Couldn't create {}: {}", templates_dir.to_str().unwrap(), e);
            std::process::exit(1);
        });
    }
    let all_templates = [
        ["script.js", include_str!("../../templates/script.js")],
        ["shared.js", include_str!("../../templates/shared.js")],
        ["style.css", include_str!("../../templates/style.css")],
        [
            "diffenator.html",
            include_str!("../../templates/diffenator.html"),
        ],
    ];
    for template in all_templates.iter() {
        let path = templates_dir.join(template[0]);
        if !path.exists() || force {
            std::fs::write(&path, template[1]).unwrap_or_else(|e| {
                println!(
                    "Couldn't write template file {}: {}",
                    path.to_str().unwrap(),
                    e
                );
                std::process::exit(1)
            });
        }
    }
    templates_dir
}

pub fn render_output(
    value: &serde_json::Value,
    old_filename: &str,
    new_filename: &str,
    tera: &Tera,
) -> Result<String, tera::Error> {
    tera.render(
        "diffenator.html",
        &Context::from_serialize(json!({
            "report": value,
            "old_filename": old_filename,
            "new_filename": new_filename,
            "pt_size": 40,
        }))?,
    )
}
