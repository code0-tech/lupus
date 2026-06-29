use crate::artifact::{Artifact, ArtifactKind};
use crate::error::ConvertError;
use crate::format::Format;
use crate::schema::{DecodeContext, EncodeContext};

pub trait Codec {
    fn format(&self) -> Format;
    fn artifact_kind(&self) -> ArtifactKind;
    fn decode(&self, input: &[u8], ctx: &DecodeContext) -> Result<Artifact, ConvertError>;
    fn encode(&self, artifact: &Artifact, ctx: &EncodeContext) -> Result<Vec<u8>, ConvertError>;
}
