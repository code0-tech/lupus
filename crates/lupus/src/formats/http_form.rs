use std::collections::BTreeMap;

use crate::artifact::{Artifact, ArtifactKind};
use crate::codec::Codec;
use crate::data::Data;
use crate::error::ConvertError;
use crate::format::Format;
use crate::schema::{DecodeContext, EncodeContext};

pub struct HttpFormCodec;

impl Codec for HttpFormCodec {
    fn format(&self) -> Format {
        Format::HttpForm
    }

    fn artifact_kind(&self) -> ArtifactKind {
        ArtifactKind::Data
    }

    fn decode(&self, input: &[u8], _ctx: &DecodeContext) -> Result<Artifact, ConvertError> {
        let input = std::str::from_utf8(input).map_err(|err| {
            ConvertError::Decoding(format!("HTTP form is not valid UTF-8: {err}"))
        })?;
        let mut fields = BTreeMap::new();

        if !input.is_empty() {
            for pair in input.split('&') {
                let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                let key = decode_component(key)?;
                let value = decode_component(value)?;
                if fields.insert(key.clone(), Data::String(value)).is_some() {
                    return Err(ConvertError::InformationLoss(format!(
                        "repeated HTTP form field {key:?} cannot round-trip as a scalar"
                    )));
                }
            }
        }

        Ok(Artifact::Data(Data::Object(fields)))
    }

    fn encode(&self, artifact: &Artifact, _ctx: &EncodeContext) -> Result<Vec<u8>, ConvertError> {
        let Artifact::Data(Data::Object(fields)) = artifact else {
            return Err(ConvertError::InformationLoss(
                "HTTP form encoding requires a flat object".to_string(),
            ));
        };

        fields
            .iter()
            .map(|(key, value)| {
                let Data::String(value) = value else {
                    return Err(ConvertError::InformationLoss(
                        "HTTP form fields must be strings; nested and typed values cannot round-trip"
                            .to_string(),
                    ));
                };
                Ok(format!(
                    "{}={}",
                    encode_component(key),
                    encode_component(value)
                ))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|pairs| pairs.join("&").into_bytes())
    }
}

fn encode_component(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'*' => {
                output.push(char::from(byte));
            }
            b' ' => output.push('+'),
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

fn decode_component(value: &str) -> Result<String, ConvertError> {
    let mut bytes = Vec::with_capacity(value.len());
    let input = value.as_bytes();
    let mut index = 0;

    while index < input.len() {
        match input[index] {
            b'+' => bytes.push(b' '),
            b'%' => {
                if index + 2 >= input.len() {
                    return Err(ConvertError::Decoding(
                        "incomplete percent escape in HTTP form".to_string(),
                    ));
                }
                let high = hex(input[index + 1])?;
                let low = hex(input[index + 2])?;
                bytes.push(high << 4 | low);
                index += 2;
            }
            byte => bytes.push(byte),
        }
        index += 1;
    }

    String::from_utf8(bytes)
        .map_err(|err| ConvertError::Decoding(format!("invalid UTF-8 in HTTP form: {err}")))
}

fn hex(byte: u8) -> Result<u8, ConvertError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(ConvertError::Decoding(format!(
            "invalid percent escape digit {:?}",
            char::from(byte)
        ))),
    }
}

#[cfg(test)]
mod tests {
    use crate::artifact::Artifact;
    use crate::codec::Codec;
    use crate::data::Data;
    use crate::formats::HttpFormCodec;
    use crate::schema::{DecodeContext, EncodeContext};

    #[test]
    fn form_round_trips_flat_string_objects() {
        let codec = HttpFormCodec;
        let encoded = b"email=grace%40example.com&name=Grace+Hopper";
        let artifact = codec.decode(encoded, &DecodeContext::default()).unwrap();
        assert!(matches!(artifact, Artifact::Data(Data::Object(_))));
        assert_eq!(
            codec.encode(&artifact, &EncodeContext::default()).unwrap(),
            encoded
        );
    }
}
