use cfg_if::cfg_if;
use serde::Deserialize;
use web_sys::Headers;

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
    // Plugin identifier used for Standard Notes
    pub plugin_identifier: String
}