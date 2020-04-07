// Bindings to Cloudflare Workers KV
use crate::utils::*;
use js_sys::Promise;
use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = PAPRIKA, js_name = "get")]
    fn kv_get(key: &str) -> Promise;
    #[wasm_bindgen(js_namespace = PAPRIKA, js_name = "put")]
    fn kv_put_str(key: &str, value: &str) -> Promise;
}

// Returns empty string ("") if the key is not found
pub async fn get_str(key: &str) -> MyResult<String> {
    Ok(JsFuture::from(kv_get(key)).await.internal_err()?.as_string().unwrap_or("".into()))
}

pub async fn get_obj<T: DeserializeOwned>(key: &str) -> MyResult<T> {
    let res = get_str(key).await?;
    Ok(serde_json::from_str(&res).internal_err()?)
}

pub async fn put_str(key: &str, value: &str) -> MyResult<()> {
    JsFuture::from(kv_put_str(key, value)).await.internal_err()?;
    Ok(())
}

pub async fn put_obj<T: Serialize>(key: &str, value: T) -> MyResult<()> {
    put_str(key, &serde_json::to_string(&value).internal_err()?).await
}

// Some objects may be available for manual editing; thus making it pretty may be helpful
// For example, the user may want to manually edit the order in which posts appear
pub async fn put_obj_pretty<T: Serialize>(key: &str, value: T) -> MyResult<()> {
    put_str(key, &serde_json::to_string_pretty(&value).internal_err()?).await
}