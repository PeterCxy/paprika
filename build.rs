extern crate serde_json;

use std::io::prelude::*;

fn main() {
    println!("cargo:rerun-if-changed=config.json");
    // Load theme name from config.json and output code to load the theme via include_dir!
    let config: serde_json::Value = 
        serde_json::from_str(&std::fs::read_to_string("./config.json").unwrap()).unwrap();
    let theme_name = match config.get("theme") {
        Some(name) => name,
        None => panic!("Please define `theme` in `config.json`")
    };
    let theme_load_code = format!("const THEME_DIR: Dir = include_dir!(\"theme/{}\");", theme_name.as_str().unwrap());
    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut out_file = std::fs::File::create(out_path.join("load_theme.rs")).unwrap();
    out_file.write_all(theme_load_code.as_bytes()).unwrap();
    out_file.sync_data().unwrap();
}