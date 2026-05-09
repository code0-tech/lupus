use crate::artifact::{Artifact, ArtifactKind};
use crate::codec::Codec;
use crate::error::ConvertError;
use crate::format::Format;
use crate::markup::{Markup, MarkupAttribute, MarkupElement, MarkupNode};
use crate::schema::{DecodeContext, EncodeContext};

pub struct XmlCodec;

impl Codec for XmlCodec {
    fn format(&self) -> Format {
        Format::Xml
    }

    fn artifact_kind(&self) -> ArtifactKind {
        ArtifactKind::Markup
    }

    fn decode(&self, input: &[u8], _ctx: &DecodeContext<'_>) -> Result<Artifact, ConvertError> {
        let input = std::str::from_utf8(input)
            .map_err(|err| ConvertError::Parse(format!("xml is not valid utf-8: {err}")))?;
        Ok(Artifact::Markup(parse_xml(input)?))
    }

    fn encode(
        &self,
        artifact: &Artifact,
        ctx: &EncodeContext<'_>,
    ) -> Result<Vec<u8>, ConvertError> {
        let Artifact::Markup(markup) = artifact else {
            return Err(ConvertError::WrongArtifact {
                expected: ArtifactKind::Markup,
                found: artifact.kind(),
            });
        };

        Ok(write_xml(markup, ctx.policy.pretty).into_bytes())
    }
}

fn parse_xml(input: &str) -> Result<Markup, ConvertError> {
    let mut parser = XmlParser::new(input);
    parser.skip_misc()?;
    let root = parser.parse_node()?;
    parser.skip_misc()?;

    if !parser.is_eof() {
        return Err(ConvertError::Parse(
            "xml can only have one root node".to_string(),
        ));
    }

    Ok(Markup { root })
}

struct XmlParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> XmlParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn starts_with(&self, pattern: &str) -> bool {
        self.rest().starts_with(pattern)
    }

    fn bump(&mut self, bytes: usize) {
        self.pos += bytes;
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.rest().chars().next()
            && ch.is_whitespace()
        {
            self.bump(ch.len_utf8());
        }
    }

    fn skip_misc(&mut self) -> Result<(), ConvertError> {
        loop {
            self.skip_ws();

            if self.starts_with("<?") {
                self.take_until("?>")?;
                continue;
            }

            if self.starts_with("<!--") {
                self.take_until("-->")?;
                continue;
            }

            break;
        }

        Ok(())
    }

    fn parse_node(&mut self) -> Result<MarkupNode, ConvertError> {
        if self.starts_with("<![CDATA[") {
            return self.parse_cdata();
        }

        if self.starts_with("<!--") {
            return self.parse_comment();
        }

        if self.starts_with("<!DOCTYPE") || self.starts_with("<!doctype") {
            return self.parse_doctype();
        }

        if self.starts_with("<") {
            return self.parse_element();
        }

        self.parse_text()
    }

    fn parse_element(&mut self) -> Result<MarkupNode, ConvertError> {
        self.expect("<")?;
        let name = self.parse_name()?;
        let mut attributes = Vec::new();

        loop {
            self.skip_ws();

            if self.starts_with("/>") {
                self.bump(2);
                return Ok(MarkupNode::Element(MarkupElement {
                    name,
                    attributes,
                    children: Vec::new(),
                }));
            }

            if self.starts_with(">") {
                self.bump(1);
                break;
            }

            attributes.push(self.parse_attribute()?);
        }

        let mut children = Vec::new();
        loop {
            if self.is_eof() {
                return Err(ConvertError::Parse(format!(
                    "missing closing tag for <{name}>"
                )));
            }

            if self.starts_with("</") {
                self.bump(2);
                let closing = self.parse_name()?;
                self.skip_ws();
                self.expect(">")?;

                if closing != name {
                    return Err(ConvertError::Parse(format!(
                        "expected closing tag </{name}>, found </{closing}>"
                    )));
                }

                break;
            }

            children.push(self.parse_node()?);
        }

        Ok(MarkupNode::Element(MarkupElement {
            name,
            attributes,
            children,
        }))
    }

    fn parse_attribute(&mut self) -> Result<MarkupAttribute, ConvertError> {
        let name = self.parse_name()?;
        self.skip_ws();
        self.expect("=")?;
        self.skip_ws();

        let quote = self
            .rest()
            .chars()
            .next()
            .ok_or_else(|| ConvertError::Parse("expected attribute quote".to_string()))?;
        if quote != '"' && quote != '\'' {
            return Err(ConvertError::Parse(
                "attribute value must be quoted".to_string(),
            ));
        }
        self.bump(quote.len_utf8());

        let start = self.pos;
        while let Some(ch) = self.rest().chars().next() {
            if ch == quote {
                let value = unescape_xml(&self.input[start..self.pos])?;
                self.bump(quote.len_utf8());
                return Ok(MarkupAttribute { name, value });
            }
            self.bump(ch.len_utf8());
        }

        Err(ConvertError::Parse(
            "unterminated attribute value".to_string(),
        ))
    }

    fn parse_name(&mut self) -> Result<String, ConvertError> {
        let start = self.pos;
        while let Some(ch) = self.rest().chars().next() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':') {
                self.bump(ch.len_utf8());
            } else {
                break;
            }
        }

        if self.pos == start {
            Err(ConvertError::Parse("expected XML name".to_string()))
        } else {
            Ok(self.input[start..self.pos].to_string())
        }
    }

    fn parse_text(&mut self) -> Result<MarkupNode, ConvertError> {
        let start = self.pos;
        while !self.is_eof() && !self.starts_with("<") {
            let Some(ch) = self.rest().chars().next() else {
                break;
            };
            self.bump(ch.len_utf8());
        }

        Ok(MarkupNode::Text(unescape_xml(
            &self.input[start..self.pos],
        )?))
    }

    fn parse_cdata(&mut self) -> Result<MarkupNode, ConvertError> {
        self.expect("<![CDATA[")?;
        let value = self.take_until("]]>")?;
        Ok(MarkupNode::CData(value))
    }

    fn parse_comment(&mut self) -> Result<MarkupNode, ConvertError> {
        self.expect("<!--")?;
        let value = self.take_until("-->")?;
        Ok(MarkupNode::Comment(value))
    }

    fn parse_doctype(&mut self) -> Result<MarkupNode, ConvertError> {
        self.expect("<!")?;
        let value = self.take_until(">")?;
        Ok(MarkupNode::Doctype(value.trim().to_string()))
    }

    fn expect(&mut self, expected: &str) -> Result<(), ConvertError> {
        if self.starts_with(expected) {
            self.bump(expected.len());
            Ok(())
        } else {
            Err(ConvertError::Parse(format!("expected {expected:?}")))
        }
    }

    fn take_until(&mut self, pattern: &str) -> Result<String, ConvertError> {
        let start = self.pos;
        let Some(offset) = self.rest().find(pattern) else {
            return Err(ConvertError::Parse(format!(
                "expected terminator {pattern:?}"
            )));
        };
        let end = self.pos + offset;
        let value = self.input[start..end].to_string();
        self.pos = end + pattern.len();
        Ok(value)
    }
}

fn write_xml(markup: &Markup, pretty: bool) -> String {
    let mut output = String::new();
    write_node(&markup.root, pretty, 0, &mut output);
    output
}

fn write_node(node: &MarkupNode, pretty: bool, depth: usize, output: &mut String) {
    if pretty {
        output.push_str(&"  ".repeat(depth));
    }

    match node {
        MarkupNode::Element(element) => write_element(element, pretty, depth, output),
        MarkupNode::Text(value) => output.push_str(&escape_text(value)),
        MarkupNode::CData(value) => {
            output.push_str("<![CDATA[");
            output.push_str(value);
            output.push_str("]]>");
        }
        MarkupNode::Comment(value) => {
            output.push_str("<!--");
            output.push_str(value);
            output.push_str("-->");
        }
        MarkupNode::Doctype(value) => {
            output.push_str("<!DOCTYPE ");
            output.push_str(value.trim_start_matches("DOCTYPE").trim());
            output.push('>');
        }
    }

    if pretty {
        output.push('\n');
    }
}

fn write_element(element: &MarkupElement, pretty: bool, depth: usize, output: &mut String) {
    output.push('<');
    output.push_str(&element.name);
    for attribute in &element.attributes {
        output.push(' ');
        output.push_str(&attribute.name);
        output.push_str("=\"");
        output.push_str(&escape_attribute(&attribute.value));
        output.push('"');
    }

    if element.children.is_empty() {
        output.push_str("/>");
        return;
    }

    output.push('>');

    let multiline = pretty
        && element
            .children
            .iter()
            .any(|child| matches!(child, MarkupNode::Element(_)));
    if multiline {
        output.push('\n');
    }

    for child in &element.children {
        write_node(child, pretty && multiline, depth + 1, output);
    }

    if multiline {
        output.push_str(&"  ".repeat(depth));
    }
    output.push_str("</");
    output.push_str(&element.name);
    output.push('>');
}

fn escape_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attribute(input: &str) -> String {
    escape_text(input)
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn unescape_xml(input: &str) -> Result<String, ConvertError> {
    let mut output = String::new();
    let mut rest = input;

    while let Some(index) = rest.find('&') {
        output.push_str(&rest[..index]);
        rest = &rest[index + 1..];

        let Some(end) = rest.find(';') else {
            return Err(ConvertError::Parse("unterminated XML entity".to_string()));
        };
        let entity = &rest[..end];
        match entity {
            "amp" => output.push('&'),
            "lt" => output.push('<'),
            "gt" => output.push('>'),
            "quot" => output.push('"'),
            "apos" => output.push('\''),
            _ => {
                return Err(ConvertError::Parse(format!(
                    "unsupported XML entity: &{entity};"
                )));
            }
        }
        rest = &rest[end + 1..];
    }

    output.push_str(rest);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::ConvertPolicy;

    #[test]
    fn xml_codec_decodes_and_encodes_elements() {
        let codec = XmlCodec;
        let policy = ConvertPolicy::default();
        let ctx = DecodeContext {
            schema: None,
            policy,
        };
        let artifact = codec
            .decode(br#"<user id="7"><name>Ada</name></user>"#, &ctx)
            .unwrap();

        let Artifact::Markup(markup) = artifact else {
            panic!("expected markup");
        };

        assert_eq!(
            markup.root,
            MarkupNode::Element(MarkupElement {
                name: "user".to_string(),
                attributes: vec![MarkupAttribute {
                    name: "id".to_string(),
                    value: "7".to_string(),
                }],
                children: vec![MarkupNode::Element(MarkupElement {
                    name: "name".to_string(),
                    attributes: Vec::new(),
                    children: vec![MarkupNode::Text("Ada".to_string())],
                })],
            })
        );
    }

    #[test]
    fn xml_codec_preserves_comments_when_encoding_markup() {
        let codec = XmlCodec;
        let policy = ConvertPolicy {
            allow_lossy: false,
            pretty: false,
        };
        let ctx = DecodeContext {
            schema: None,
            policy: policy.clone(),
        };
        let artifact = codec.decode(b"<root><!--x--></root>", &ctx).unwrap();
        let encoded = codec
            .encode(
                &artifact,
                &EncodeContext {
                    schema: None,
                    policy,
                },
            )
            .unwrap();

        assert_eq!(String::from_utf8(encoded).unwrap(), "<root><!--x--></root>");
    }
}
