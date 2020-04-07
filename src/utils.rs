use cfg_if::cfg_if;
use serde::Deserialize;

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

pub type MyResult<T> = Result<T, Error>;

pub enum Error {
    NotFound(String)
}

impl Error {
    pub fn status_code(&self) -> u16 {
        match self {
            Error::NotFound(_) => 404
        }
    }
}

impl Into<String> for Error {
    fn into(self) -> String {
        match self {
            Error::NotFound(reason) => {
                format!("Not Found, Reason: {}", reason)
            }
        }
    }
}

#[derive(Deserialize)]
pub struct Config {
    // The secret value used to authenticate the Standard Notes plugin link
    secret: String
}

pub fn get_config() -> Config {
    serde_json::from_str(std::include_str!("../config.json")).unwrap()
}