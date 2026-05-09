use crate::policy::ConvertPolicy;

pub struct DecodeContext<'a> {
    pub schema: Option<&'a Schema>,
    pub policy: ConvertPolicy,
}

impl Default for DecodeContext<'_> {
    fn default() -> Self {
        Self {
            schema: None,
            policy: ConvertPolicy::default(),
        }
    }
}

pub struct EncodeContext<'a> {
    pub schema: Option<&'a Schema>,
    pub policy: ConvertPolicy,
}

impl Default for EncodeContext<'_> {
    fn default() -> Self {
        Self {
            schema: None,
            policy: ConvertPolicy::default(),
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
