use std::fmt;

pub const BYTES: &str = "bytes";
pub const TEXT: &str = "text";
pub const PROTOBUF: &str = "protobuf";
pub const XML: &str = "xml";
pub const JSON: &str = "json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    Bytes,
    Text,
    Protobuf,
    Xml,
    Json,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Bytes => BYTES,
            Format::Text => TEXT,
            Format::Protobuf => PROTOBUF,
            Format::Xml => XML,
            Format::Json => JSON,
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
