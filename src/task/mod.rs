#[allow(dead_code)]
// This module contains some kanged code from Tokio
// because we cannot import the entire Tokio
mod task_local;
pub use task_local::LocalKey;