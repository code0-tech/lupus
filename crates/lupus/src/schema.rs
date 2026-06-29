#[derive(Default)]
pub struct DecodeContext;

#[derive(Default)]
pub struct EncodeContext {
    pub pretty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonSchema {
    pub raw: String,
}
