use crate::data::Data;
use crate::markup::Markup;

#[derive(Debug, Clone, PartialEq)]
pub enum Artifact {
    Data(Data),
    Markup(Markup),
    Text(String),
}

impl Artifact {
    pub fn kind(&self) -> ArtifactKind {
        match self {
            Artifact::Data(_) => ArtifactKind::Data,
            Artifact::Markup(_) => ArtifactKind::Markup,
            Artifact::Text(_) => ArtifactKind::Text,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Data,
    Markup,
    Text,
}
