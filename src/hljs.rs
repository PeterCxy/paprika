// Simple bindings for Highlight.js
// We don't have something equivalent in Rust
// and I don't really want to run these on client
use js_sys::Reflect;
use wasm_bindgen::prelude::*;

include!(concat!(env!("OUT_DIR"), "/load_hljs.rs"));

pub fn highlight_auto(code: &str) -> String {
    Reflect::get(&hljs_highlight_auto(code), &"value".into())
        .unwrap().as_string().unwrap()
}

pub fn highlight(lang: &str, code: &str) -> String {
    Reflect::get(&hljs_highlight(lang, code), &"value".into())
        .unwrap().as_string().unwrap()
}