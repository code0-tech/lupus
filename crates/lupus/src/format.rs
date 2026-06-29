use std::fmt;

pub const TEXT: &str = "text";
pub const PROTOBUF: &str = "protobuf";
pub const CSV: &str = "csv";
pub const HTTP_FORM: &str = "http-form";
pub const XML: &str = "xml";
pub const JSON: &str = "json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    Text,
    Protobuf,
    Csv,
    HttpForm,
    Xml,
    Json,
}

impl Format {
    pub const ALL: [Self; 6] = [
        Self::Json,
        Self::Xml,
        Self::Text,
        Self::Csv,
        Self::HttpForm,
        Self::Protobuf,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Format::Text => TEXT,
            Format::Protobuf => PROTOBUF,
            Format::Csv => CSV,
            Format::HttpForm => HTTP_FORM,
            Format::Xml => XML,
            Format::Json => JSON,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Format::Json => "JSON",
            Format::Xml => "XML",
            Format::Text => "Text",
            Format::Csv => "CSV",
            Format::HttpForm => "HTTP Form",
            Format::Protobuf => "Tucana Value",
        }
    }
}

impl TryFrom<&str> for Format {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|format| format.as_str() == value)
            .ok_or(())
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
