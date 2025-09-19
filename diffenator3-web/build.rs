use diffenator3::reporters::Api as ReportApi;
use diffenator3_lib::structs::Api;

use typescript_type_def::{write_definition_file, DefinitionFileOptions};

fn main() {
    let ts_module = {
        let mut buf = Vec::new();
        let options = DefinitionFileOptions {
            root_namespace: None,
            ..DefinitionFileOptions::default()
        };
        write_definition_file::<_, (ReportApi, Api)>(&mut buf, options).unwrap();
        String::from_utf8(buf).unwrap()
    };
    std::fs::write("www/ts/api.d.ts", ts_module).unwrap();

    // Run npm install / npm build
    let status = std::process::Command::new("npm")
        .arg("install")
        .current_dir("www")
        .status()
        .expect("failed to run npm install");
    assert!(status.success());
    let status = std::process::Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir("www")
        .status()
        .expect("failed to run npm run build");
    assert!(status.success());
}
