use cfg_if::cfg_if;
use serde::Deserialize;
use js_sys::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::*;
use web_sys::*;

cfg_if! {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    if #[cfg(feature = "console_error_panic_hook")] {
        extern crate console_error_panic_hook;
        pub use self::console_error_panic_hook::set_once as set_panic_hook;
    } else {
        #[inline]
        pub fn set_panic_hook() {}
    }
}

#[macro_export]
macro_rules! cors {
    ($headers:ident) => {
        $headers.set("Access-Control-Allow-Origin", "*").unwrap();
        $headers.set("Access-Control-Allow-Headers", "*").unwrap();
    };
}

// Adapted from <https://stackoverflow.com/questions/27582739/how-do-i-create-a-hashmap-literal>
#[macro_export]
macro_rules! headers(
    { $($key:expr => $value:expr),+ } => {
        {
            let headers = ::web_sys::Headers::new().unwrap();
            $(
                headers.set($key, $value).unwrap();
            )+
            headers
        }
     };
     () => { ::web_sys::Headers::new().unwrap() };
);

// Remove all non-ascii characters from string
pub fn filter_non_ascii_alphanumeric(s: &str) -> String {
    s.chars().into_iter()
        .filter(|c| c.is_ascii_alphanumeric() || c.is_whitespace())
        .collect()
}

// A URL is "<uuid_first_four_chars>/<title_without_non_ascii>"
// The UUID involvement is to reduce the chance that two
// articles have the same URL by having the same title when
// all non-ASCII characters are removed
pub fn title_to_url(uuid: &str, title: &str) -> String {
    let title_part = filter_non_ascii_alphanumeric(title)
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join("-")
        .to_lowercase();
    format!("{}/{}", &uuid[0..4], title_part)
}

#[wasm_bindgen]
extern "C" {
    static crypto: Crypto;
}

// SHA-1 digest (hexed) via SubtleCrypto
pub async fn sha1(s: &str) -> String {
    let mut bytes: Vec<u8> = s.bytes().collect();
    let promise = crypto.subtle().digest_with_str_and_u8_array("SHA-1", &mut bytes).unwrap();
    let buffer: ArrayBuffer = JsFuture::from(promise).await.unwrap().into();
    let digest_arr = Uint8Array::new(&buffer).to_vec();
    hex::encode(digest_arr)
}

pub trait HeadersExt {
    fn add_cors(self) -> Self;
}

impl HeadersExt for Headers {
    fn add_cors(self) -> Self {
        self.set("Access-Control-Allow-Origin", "*").unwrap();
        self.set("Access-Control-Allow-Headers", "*").unwrap();
        self
    }
}

pub trait ResultExt<T, E> {
    // Ignore any error and return InternalError for them all
    // Used in place of ugly `.unwrap()`.
    fn internal_err(self) -> Result<T, Error>;
}

impl <T, E> ResultExt<T, E> for Result<T, E> {
    fn internal_err(self) -> Result<T, Error> {
        self.map_err(|_| crate::utils::Error::InternalError())
    }
}

pub type MyResult<T> = Result<T, Error>;

pub enum Error {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    InternalError()
}

impl Error {
    pub fn status_code(&self) -> u16 {
        match self {
            Error::NotFound(_) => 404,
            Error::BadRequest(_) => 400,
            Error::Unauthorized(_) => 401,
            Error::InternalError() => 500
        }
    }
}

impl Into<String> for Error {
    fn into(self) -> String {
        match self {
            Error::NotFound(reason) => {
                format!("Not Found, Reason: {}", reason)
            },
            Error::BadRequest(reason) => {
                format!("Bad Request, Reason: {}", reason)
            },
            Error::Unauthorized(reason) => {
                format!("Unauthorized, Reason: {}", reason)
            },
            Error::InternalError() => {
                format!("Internal Errror")
            }
        }
    }
}

#[derive(Deserialize)]
pub struct Config {
    // The secret value used to authenticate the Standard Notes plugin link
    pub secret: String,
    // Title of the blog
    pub title: String,
    // Language of blog
    pub lang: String,
    // Description of the blog
    pub description: String,
    // Plugin identifier used for Standard Notes
    pub plugin_identifier: String
}