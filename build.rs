extern crate serde_json;

use std::fs;
use std::io::prelude::*;
use std::time::*;

fn main() {
    println!("cargo:rerun-if-changed=config.json");
    println!("cargo:rerun-if-changed=Cargo.toml");
    rerun_if_dir_changed("src");
    rerun_if_dir_changed("theme");
    // Load theme name from config.json and output code to load the theme via include_dir!
    let config: serde_json::Value = 
        serde_json::from_str(&fs::read_to_string("./config.json").unwrap()).unwrap();

    generate_build_timestamp();
    generate_theme_loader(&config);
    generate_hljs_loader(&config);
}

fn rerun_if_dir_changed(dir: &str) {
    for f in fs::read_dir(dir).unwrap() {
        let f = f.unwrap();
        let t = f.file_type().unwrap();
        let path = f.path();
        let path = path.to_str().unwrap();
        println!("cargo:rerun-if-changed={}", path);

        if t.is_dir() {
            rerun_if_dir_changed(path);
        }
    }
}

fn generate_build_timestamp() {
    let build_time = format!(
        "pub const BUILD_TIMESTAMP: u64 = {};",
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());
    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut out_file = fs::File::create(out_path.join("build_timestamp.rs")).unwrap();
    out_file.write(build_time.as_bytes()).unwrap();
    out_file.sync_data().unwrap();
}

fn generate_theme_loader(config: &serde_json::Value) {
    let theme_name = match config.get("theme") {
        Some(name) => name,
        None => panic!("Please define `theme` in `config.json`")
    };
    let theme_load_code = format!("const THEME_DIR: Dir = include_dir!(\"theme/{}\");", theme_name.as_str().unwrap());
    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut out_file = fs::File::create(out_path.join("load_theme.rs")).unwrap();
    out_file.write_all(theme_load_code.as_bytes()).unwrap();
    out_file.sync_data().unwrap();
}

fn generate_hljs_loader(config: &serde_json::Value) {
    let highlight_lang = match config.get("hljs") {
        Some(val) => val,
        None => panic!("Please specify what language for hljs to support in `config.json` with `hljs`")
    };

    if !highlight_lang.is_array() {
        panic!("`hljs` is not an array");
    }

    let highlight_lang = highlight_lang.as_array().unwrap().into_iter().map(|lang| {
        let lang = lang.as_str().unwrap();
        // Require only the needed language definition files
        format!("hljs.registerLanguage('{}', require('highlight.js/lib/languages/{}'));\n", lang, lang)
    }).collect::<String>();

    let js_code = format!(
        "const hljs = require(\\\"highlight.js/lib/core\\\");\n{}module.exports = hljs;",
        highlight_lang);
    let rs_code = fs::read_to_string("./src/hljs_tpl.rs").unwrap();
    let rs_code = format!("#[wasm_bindgen(inline_js = \"{}\")]\n{}", js_code, rs_code);
    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut out_file = fs::File::create(out_path.join("load_hljs.rs")).unwrap();
    out_file.write_all(rs_code.as_bytes()).unwrap();
    out_file.sync_data().unwrap();
}
