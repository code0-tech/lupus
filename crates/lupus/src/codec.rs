use crate::artifact::{Artifact, ArtifactKind};
use crate::error::ConvertError;
use crate::format::Format;
use crate::schema::{DecodeContext, EncodeContext};

pub trait Codec {
    fn format(&self) -> Format;
    fn artifact_kind(&self) -> ArtifactKind;
    fn decode(&self, input: &[u8], ctx: &DecodeContext<'_>) -> Result<Artifact, ConvertError>;
    fn encode(&self, artifact: &Artifact, ctx: &EncodeContext<'_>)
    -> Result<Vec<u8>, ConvertError>;
}
