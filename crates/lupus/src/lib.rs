#![forbid(unsafe_code)]

//! Format conversion and JSON Schema validation through owned intermediate
//! data and markup representations.

pub mod artifact;
pub mod codec;
pub mod data;
pub mod engine;
pub mod error;
pub mod format;
pub mod formats;
pub mod markup;
pub mod schema;
pub mod transform;
pub mod validation;

pub use engine::Engine;
pub use error::ConvertError;
pub use format::Format;
pub use schema::{DecodeContext, EncodeContext, JsonSchema};
