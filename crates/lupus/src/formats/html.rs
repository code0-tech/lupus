use crate::artifact::{Artifact, ArtifactKind};
use crate::codec::Codec;
use crate::error::ConvertError;
use crate::format::Format;
use crate::formats::xml::{parse_xml, write_xml};
use crate::schema::{DecodeContext, EncodeContext};

pub struct HtmlCodec;

impl Codec for HtmlCodec {
    fn format(&self) -> Format {
        Format::Html
    }

    fn artifact_kind(&self) -> ArtifactKind {
        ArtifactKind::Markup
    }

    fn decode(&self, input: &[u8], _ctx: &DecodeContext) -> Result<Artifact, ConvertError> {
        let input = std::str::from_utf8(input)
            .map_err(|err| ConvertError::Decoding(format!("html is not valid UTF-8: {err}")))?;
        Ok(Artifact::Markup(parse_xml(input)?))
    }

    fn encode(&self, artifact: &Artifact, ctx: &EncodeContext) -> Result<Vec<u8>, ConvertError> {
        let Artifact::Markup(markup) = artifact else {
            return Err(ConvertError::WrongArtifact {
                expected: ArtifactKind::Markup,
                found: artifact.kind(),
            });
        };

        Ok(write_xml(markup, ctx.pretty).into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::Codec;
    use crate::formats::HtmlCodec;
    use crate::schema::{DecodeContext, EncodeContext};

    #[test]
    fn html_codec_decodes_and_encodes_markup() -> Result<(), Box<dyn std::error::Error>> {
        let codec = HtmlCodec;
        let artifact = codec.decode(b"<article><h1>Hello</h1></article>", &DecodeContext)?;

        assert_eq!(
            codec.encode(&artifact, &EncodeContext::default())?,
            b"<article><h1>Hello</h1></article>"
        );
        Ok(())
    }
}
