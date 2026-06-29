use std::{error::Error, fmt};

use crate::artifact::ArtifactKind;

#[derive(Debug, Clone, PartialEq)]
pub enum ConvertError {
    UnsupportedFormat(String),
    WrongArtifact {
        expected: ArtifactKind,
        found: ArtifactKind,
    },
    InformationLoss(String),
    Validation(String),
    InvalidConversion(String),
    Decoding(String),
    Encoding(String),
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
            ConvertError::InformationLoss(message) => {
                write!(f, "conversion would lose information: {message}")
            }
            ConvertError::Validation(message) => write!(f, "validation failed: {message}"),
            ConvertError::InvalidConversion(message) => {
                write!(f, "invalid conversion: {message}")
            }
            ConvertError::Decoding(message) => write!(f, "decoding failed: {message}"),
            ConvertError::Encoding(message) => write!(f, "encoding failed: {message}"),
        }
    }
}

impl Error for ConvertError {}

impl From<serde_json::Error> for ConvertError {
    fn from(value: serde_json::Error) -> Self {
        ConvertError::Encoding(value.to_string())
    }
}
