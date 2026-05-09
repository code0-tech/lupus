use std::{error::Error, fmt};

use crate::artifact::ArtifactKind;

#[derive(Debug, Clone, PartialEq)]
pub enum ConvertError {
    UnsupportedFormat(String),
    WrongArtifact {
        expected: ArtifactKind,
        found: ArtifactKind,
    },
    LossyConversionRefused(String),
    InvalidConversion(String),
    Parse(String),
    Serialization(String),
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvertError::UnsupportedFormat(format) => write!(f, "unsupported format: {format}"),
            ConvertError::WrongArtifact { expected, found } => {
                write!(
                    f,
                    "wrong artifact type: expected {expected:?}, found {found:?}"
                )
            }
            ConvertError::LossyConversionRefused(message) => {
                write!(f, "lossy conversion refused: {message}")
            }
            ConvertError::InvalidConversion(message) => {
                write!(f, "invalid conversion: {message}")
            }
            ConvertError::Parse(message) => write!(f, "parse error: {message}"),
            ConvertError::Serialization(message) => write!(f, "serialization error: {message}"),
        }
    }
}

impl Error for ConvertError {}

impl From<serde_json::Error> for ConvertError {
    fn from(value: serde_json::Error) -> Self {
        ConvertError::Serialization(value.to_string())
    }
}
