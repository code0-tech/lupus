use std::collections::BTreeMap;

use crate::artifact::{Artifact, ArtifactKind};
use crate::codec::Codec;
use crate::data::Data;
use crate::error::ConvertError;
use crate::format::Format;
use crate::formats::{CsvCodec, HttpFormCodec, JsonCodec, ProtobufCodec, TextCodec, XmlCodec};
use crate::schema::{DecodeContext, EncodeContext, JsonSchema};
use crate::transform::{data_into_markup, data_to_text, markup_into_data, markup_to_text};
use crate::validation::validate_json_schema;

#[derive(Default)]
pub struct Engine {
    codecs: BTreeMap<Format, Box<dyn Codec>>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_codecs() -> Self {
        let mut engine = Self::new();
        engine.register(JsonCodec);
        engine.register(XmlCodec);
        engine.register(TextCodec);
        engine.register(CsvCodec);
        engine.register(HttpFormCodec);
        engine.register(ProtobufCodec);
        engine
    }

    pub fn register<C>(&mut self, codec: C)
    where
        C: Codec + 'static,
    {
        self.codecs.insert(codec.format(), Box::new(codec));
    }

    pub fn convert(
        &self,
        input: &[u8],
        from: Format,
        to: Format,
        decode_ctx: &DecodeContext,
        encode_ctx: &EncodeContext,
    ) -> Result<Vec<u8>, ConvertError> {
        let decoder = self
            .codecs
            .get(&from)
            .ok_or_else(|| ConvertError::UnsupportedFormat(from.to_string()))?;
        let encoder = self
            .codecs
            .get(&to)
            .ok_or_else(|| ConvertError::UnsupportedFormat(to.to_string()))?;

        let artifact = decoder.decode(input, decode_ctx)?;
        let artifact = self.normalize_for_target(artifact, encoder.artifact_kind())?;

        encoder.encode(&artifact, encode_ctx)
    }

    pub fn validate(
        &self,
        input: &[u8],
        format: Format,
        schema: &JsonSchema,
        decode_ctx: &DecodeContext,
    ) -> Result<(), ConvertError> {
        let decoder = self
            .codecs
            .get(&format)
            .ok_or_else(|| ConvertError::UnsupportedFormat(format.to_string()))?;
        let artifact = decoder.decode(input, decode_ctx)?;
        let artifact = self.normalize_for_target(artifact, ArtifactKind::Data)?;
        let Artifact::Data(data) = artifact else {
            unreachable!("normalization to Data returned another artifact kind");
        };
        validate_json_schema(&data, schema)
    }

    pub fn normalize_for_target(
        &self,
        artifact: Artifact,
        target: ArtifactKind,
    ) -> Result<Artifact, ConvertError> {
        if artifact.kind() == target {
            return Ok(artifact);
        }

        match (artifact, target) {
            (Artifact::Data(data), ArtifactKind::Markup) => {
                Ok(Artifact::Markup(data_into_markup(data)?))
            }
            (Artifact::Markup(markup), ArtifactKind::Data) => {
                Ok(Artifact::Data(markup_into_data(markup)?))
            }
            (Artifact::Markup(markup), ArtifactKind::Text) => {
                Ok(Artifact::Text(markup_to_text(&markup)))
            }
            (Artifact::Data(data), ArtifactKind::Text) => Ok(Artifact::Text(data_to_text(&data))),
            (Artifact::Text(text), ArtifactKind::Data) => Ok(Artifact::Data(Data::String(text))),
            (Artifact::Text(text), ArtifactKind::Markup) => {
                Ok(Artifact::Markup(data_into_markup(Data::String(text))?))
            }
            (Artifact::Binary(bytes), ArtifactKind::Text) => {
                let text = String::from_utf8(bytes).map_err(|err| {
                    ConvertError::InvalidConversion(format!("binary is not valid utf-8: {err}"))
                })?;
                Ok(Artifact::Text(text))
            }
            (artifact, target) => Err(ConvertError::WrongArtifact {
                expected: target,
                found: artifact.kind(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::artifact::{Artifact, ArtifactKind};
    use crate::data::Data;
    use crate::engine::Engine;
    use crate::format::Format;
    use crate::formats::CsvCodec;
    use crate::formats::JsonCodec;
    use crate::formats::TextCodec;
    use crate::formats::XmlCodec;
    use crate::schema::{DecodeContext, EncodeContext, JsonSchema};

    #[test]
    fn text_into_data_raw() {
        let mut engine = Engine::new();
        engine.register(JsonCodec);
        engine.register(TextCodec);

        let input = "This is just a long long string";
        let result = engine
            .convert(
                input.as_bytes(),
                Format::Text,
                Format::Json,
                &DecodeContext::default(),
                &EncodeContext::default(),
            )
            .unwrap();

        assert_eq!(result, br#""This is just a long long string""#);
    }

    #[test]
    fn normalizing_text_to_data_wraps_the_text_as_a_string() {
        let engine = Engine::new();

        assert_eq!(
            engine
                .normalize_for_target(
                    Artifact::Text("unstructured".to_string()),
                    ArtifactKind::Data,
                )
                .unwrap(),
            Artifact::Data(Data::String("unstructured".to_string()))
        );
    }

    #[test]
    fn json_xml_json_round_trip_does_not_add_formatting_text() {
        let mut engine = Engine::new();
        engine.register(JsonCodec);
        engine.register(XmlCodec);
        let decode_ctx = DecodeContext::default();
        let encode_ctx = EncodeContext::default();
        let json =
            br#"{"user":{"@id":"7","email":["ada@example.com","ada@work.example"],"name":"Ada"}}"#;

        let xml = engine
            .convert(json, Format::Json, Format::Xml, &decode_ctx, &encode_ctx)
            .unwrap();
        assert_eq!(
            xml,
            br#"<user id="7"><email>ada@example.com</email><email>ada@work.example</email><name>Ada</name></user>"#
        );

        let result = engine
            .convert(&xml, Format::Xml, Format::Json, &decode_ctx, &encode_ctx)
            .unwrap();
        assert_eq!(result, json);
    }

    #[test]
    fn codecs_pretty_print_structured_output_when_requested() {
        let mut engine = Engine::new();
        engine.register(JsonCodec);
        engine.register(XmlCodec);
        let decode_ctx = DecodeContext::default();
        let encode_ctx = EncodeContext { pretty: true };

        let xml = engine
            .convert(
                br#"{"user":{"name":"Ada","email":"ada@example.com"}}"#,
                Format::Json,
                Format::Xml,
                &decode_ctx,
                &encode_ctx,
            )
            .unwrap();
        assert_eq!(
            xml,
            b"<user>\n  <email>ada@example.com</email>\n  <name>Ada</name>\n</user>"
        );

        let json = engine
            .convert(&xml, Format::Xml, Format::Json, &decode_ctx, &encode_ctx)
            .unwrap();
        assert_eq!(
            json,
            b"{\n  \"user\": {\n    \"email\": \"ada@example.com\",\n    \"name\": \"Ada\"\n  }\n}"
        );
    }

    #[test]
    fn canonical_xml_array_converts_to_csv() {
        let mut engine = Engine::new();
        engine.register(XmlCodec);
        engine.register(CsvCodec);
        let xml = br#"<data>
  <item>
    <email>ada@example.com</email>
    <name>Ada Lovelace</name>
  </item>
  <item>
    <email>grace@example.com</email>
    <name>Grace Hopper</name>
  </item>
</data>"#;

        let csv = engine
            .convert(
                xml,
                Format::Xml,
                Format::Csv,
                &DecodeContext::default(),
                &EncodeContext::default(),
            )
            .unwrap();

        assert_eq!(
            csv,
            b"email,name\nada@example.com,Ada Lovelace\ngrace@example.com,Grace Hopper\n"
        );
    }

    #[test]
    fn validate_decodes_xml_before_applying_json_schema() {
        let mut engine = Engine::new();
        engine.register(XmlCodec);
        let schema = JsonSchema {
            raw: r#"{
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "required": ["user"],
                "properties": {
                    "user": {
                        "type": "object",
                        "required": ["name"],
                        "properties": {
                            "name": { "const": "Ada" }
                        }
                    }
                }
            }"#
            .to_string(),
        };

        engine
            .validate(
                b"<user><name>Ada</name></user>",
                Format::Xml,
                &schema,
                &DecodeContext::default(),
            )
            .unwrap();
    }
}
