use std::path::{Path, PathBuf};

use diffenator3_lib::html::{gen_html, Tera};

use super::Report;

pub fn report(
    font1_pb: &PathBuf,
    font2_pb: &PathBuf,
    output_dir: &Path,
    tera: Tera,
    report: &Report,
) -> ! {
    gen_html(
        font1_pb,
        font2_pb,
        output_dir,
        tera,
        "diffenator.html",
        &serde_json::to_value(report).expect("Couldn't serialize report"),
        "diffenator.html",
        40,
    );
}
