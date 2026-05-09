use crate::data::Data;
use crate::markup::Markup;

#[derive(Debug, Clone, PartialEq)]
pub enum Artifact {
    Data(Data),
    Markup(Markup),
    Text(String),
    Binary(Vec<u8>),
}

impl Artifact {
    pub fn kind(&self) -> ArtifactKind {
        match self {
            Artifact::Data(_) => ArtifactKind::Data,
            Artifact::Markup(_) => ArtifactKind::Markup,
            Artifact::Text(_) => ArtifactKind::Text,
            Artifact::Binary(_) => ArtifactKind::Binary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Data,
    Markup,
    Text,
    Binary,
}
