use crate::artifact::{Artifact, ArtifactKind};
use crate::codec::Codec;
use crate::error::ConvertError;
use crate::format::Format;
use crate::schema::{DecodeContext, EncodeContext};

pub struct TextCodec;

impl Codec for TextCodec {
    fn format(&self) -> Format {
        Format::Text
    }

    fn artifact_kind(&self) -> ArtifactKind {
        ArtifactKind::Text
    }

    fn decode(&self, input: &[u8], _ctx: &DecodeContext) -> Result<Artifact, ConvertError> {
        let text = std::str::from_utf8(input)
            .map_err(|err| ConvertError::Decoding(format!("text is not valid utf-8: {err}")))?;
        Ok(Artifact::Text(text.to_string()))
    }

    fn encode(&self, artifact: &Artifact, _ctx: &EncodeContext) -> Result<Vec<u8>, ConvertError> {
        let Artifact::Text(text) = artifact else {
            return Err(ConvertError::WrongArtifact {
                expected: ArtifactKind::Text,
                found: artifact.kind(),
            });
        };

        Ok(text.as_bytes().to_vec())
    }
}
