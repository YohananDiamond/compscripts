#![allow(dead_code)]

pub mod functions;
pub mod traits;
pub mod array_serialization;
pub use array_serialization::*;
pub use functions::*;
pub use traits::*;

pub use std::env::var as getenv;
pub type JsonError = serde_json::error::Error;
