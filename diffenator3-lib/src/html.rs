// Shared HTML templating code between diffenator3-cli and diff3proof
use serde_json::{json, Value};
use std::{
    error::Error,
    path::{Path, PathBuf},
};
use tera::Context;
pub use tera::Tera;
use walkdir::WalkDir;

pub(crate) fn die(doing: &str, err: impl Error) -> ! {
    eprintln!("Error {}: {}", doing, err);
    eprintln!();
    eprintln!("Caused by:");
    if let Some(cause) = err.source() {
        for (i, e) in std::iter::successors(Some(cause), |e| (*e).source()).enumerate() {
            eprintln!("   {}: {}", i, e);
        }
    }
    std::process::exit(1);
}

#[allow(clippy::too_many_arguments)]
pub fn gen_html(
    font1_pb: &PathBuf,
    font2_pb: &PathBuf,
    output_dir: &Path,
    tera: Tera,
    template_name: &str,
    template_variables: &Value,
    output_file: &str,
    point_size: u32,
) -> ! {
    // Make output directory
    if !output_dir.exists() {
        std::fs::create_dir(output_dir).expect("Couldn't create output directory");
    }

    // Copy old font to output/old-<existing name>
    let old_font = output_dir.join(format!(
        "old-{}",
        font1_pb.file_name().unwrap().to_str().unwrap()
    ));
    std::fs::copy(font1_pb, &old_font).expect("Couldn't copy old font");
    let new_font = output_dir.join(format!(
        "new-{}",
        font2_pb.file_name().unwrap().to_str().unwrap()
    ));
    std::fs::copy(font2_pb, &new_font).expect("Couldn't copy new font");

    let html = tera
        .render(
            template_name,
            &Context::from_serialize(json!({
                "report": template_variables,
                "old_filename": old_font.file_name().unwrap().to_str().unwrap(),
                "new_filename": new_font.file_name().unwrap().to_str().unwrap(),
                "pt_size": point_size,
            }))
            .unwrap_or_else(|err| die("creating context", err)),
        )
        .unwrap_or_else(|err| die("rendering HTML", err));

    // Write output
    let output_file = output_dir.join(output_file);
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
        // ["shared.js", include_str!("../../templates/shared.js")],
        ["style.css", include_str!("../../templates/style.css")],
        [
            "diffenator.html",
            include_str!("../../templates/diffenator.html"),
        ],
        [
            "diff3proof.html",
            include_str!("../../templates/diff3proof.html"),
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
