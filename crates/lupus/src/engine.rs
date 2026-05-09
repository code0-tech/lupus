use std::collections::BTreeMap;

use crate::artifact::{Artifact, ArtifactKind};
use crate::codec::Codec;
use crate::data::Data;
use crate::error::ConvertError;
use crate::format::Format;
use crate::markup::Markup;
use crate::schema::{DecodeContext, EncodeContext};
use crate::transform::{data_into_markup, data_to_text, markup_into_data, markup_to_text};

#[derive(Default)]
pub struct Engine {
    codecs: BTreeMap<Format, Box<dyn Codec>>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
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
        decode_ctx: &DecodeContext<'_>,
        encode_ctx: &EncodeContext<'_>,
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
                Ok(Artifact::Markup(self.data_into_markup(data)?))
            }
            (Artifact::Markup(markup), ArtifactKind::Data) => {
                Ok(Artifact::Data(self.markup_into_data(markup)?))
            }
            (Artifact::Markup(markup), ArtifactKind::Text) => {
                Ok(Artifact::Text(markup_to_text(&markup)))
            }
            (Artifact::Data(data), ArtifactKind::Text) => Ok(Artifact::Text(data_to_text(&data))),
            (Artifact::Text(text), ArtifactKind::Data) => Ok(Artifact::Data(Data::String(text))),
            (Artifact::Text(text), ArtifactKind::Markup) => {
                Ok(Artifact::Markup(self.data_into_markup(Data::String(text))?))
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

    pub fn data_into_markup(&self, data: Data) -> Result<Markup, ConvertError> {
        data_into_markup(data)
    }

    pub fn markup_into_data(&self, markup: Markup) -> Result<Data, ConvertError> {
        markup_into_data(markup)
    }
}
