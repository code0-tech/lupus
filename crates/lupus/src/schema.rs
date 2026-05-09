pub struct DecodeContext<'a> {
    pub schema: Option<&'a Schema>,
}

impl Default for DecodeContext<'_> {
    fn default() -> Self {
        Self {
            schema: None,
        }
    }
}

pub struct EncodeContext<'a> {
    pub schema: Option<&'a Schema>,
}

impl Default for EncodeContext<'_> {
    fn default() -> Self {
        Self {
            schema: None,
        }
    }
}

pub enum Schema {
    Protobuf(ProtobufSchema),
    JsonSchema(JsonSchema),
    XmlSchema(XmlSchema),
}

pub struct ProtobufSchema {
    pub message_name: String,
    pub descriptor_bytes: Vec<u8>,
}

pub struct JsonSchema {
    pub raw: String,
}

pub struct XmlSchema {
    pub raw: String,
}
