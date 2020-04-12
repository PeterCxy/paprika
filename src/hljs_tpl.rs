// Will be loaded by build.rs and add #[wasm_bindgen(inline_js = "...")] here
extern "C" {
    #[wasm_bindgen(js_name = "highlightAuto")]
    fn hljs_highlight_auto(code: &str) -> JsValue;
    #[wasm_bindgen(catch, js_name = "highlight")]
    fn hljs_highlight(lang: &str, code: &str) -> Result<JsValue, JsValue>;
}