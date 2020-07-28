#![allow(dead_code)]

pub mod functions;
pub mod traits;
pub use functions::*;
pub use traits::*;

pub use std::env::var as getenv;
pub type JsonError = serde_json::error::Error;
