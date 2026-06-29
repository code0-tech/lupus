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

    fn decode(&self, input: &[u8], _ctx: &DecodeContext) -> Result<Artifact, ConvertError> {
        let input = std::str::from_utf8(input)
            .map_err(|err| ConvertError::Decoding(format!("xml is not valid utf-8: {err}")))?;
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

fn parse_xml(input: &str) -> Result<Markup, ConvertError> {
    let mut parser = XmlParser::new(input);
    parser.skip_misc()?;
    let root = parser.parse_node()?;
    parser.skip_misc()?;

    if !parser.is_eof() {
        return Err(ConvertError::Decoding(
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
                return Err(ConvertError::Decoding(format!(
                    "missing closing tag for <{name}>"
                )));
            }

            if self.starts_with("</") {
                self.bump(2);
                let closing = self.parse_name()?;
                self.skip_ws();
                self.expect(">")?;

                if closing != name {
                    return Err(ConvertError::Decoding(format!(
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
            .ok_or_else(|| ConvertError::Decoding("expected attribute quote".to_string()))?;
        if quote != '"' && quote != '\'' {
            return Err(ConvertError::Decoding(
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

        Err(ConvertError::Decoding(
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
            Err(ConvertError::Decoding("expected XML name".to_string()))
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
            Err(ConvertError::Decoding(format!("expected {expected:?}")))
        }
    }

    fn take_until(&mut self, pattern: &str) -> Result<String, ConvertError> {
        let start = self.pos;
        let Some(offset) = self.rest().find(pattern) else {
            return Err(ConvertError::Decoding(format!(
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
    write_node(&markup.root, 0, pretty, &mut output);
    output
}

fn write_node(node: &MarkupNode, depth: usize, pretty: bool, output: &mut String) {
    match node {
        MarkupNode::Element(element) => write_element(element, depth, pretty, output),
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
}

fn write_element(element: &MarkupElement, depth: usize, pretty: bool, output: &mut String) {
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
            .all(|child| matches!(child, MarkupNode::Element(_)));

    if multiline {
        output.push('\n');
        for child in &element.children {
            output.push_str(&"  ".repeat(depth + 1));
            write_node(child, depth + 1, pretty, output);
            output.push('\n');
        }
        output.push_str(&"  ".repeat(depth));
    } else {
        for child in &element.children {
            write_node(child, depth + 1, pretty, output);
        }
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
            return Err(ConvertError::Decoding(
                "unterminated XML entity".to_string(),
            ));
        };
        let entity = &rest[..end];
        match entity {
            "amp" => output.push('&'),
            "lt" => output.push('<'),
            "gt" => output.push('>'),
            "quot" => output.push('"'),
            "apos" => output.push('\''),
            _ => {
                return Err(ConvertError::Decoding(format!(
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

    #[test]
    fn xml_codec_decodes_and_encodes_elements() -> Result<(), Box<dyn std::error::Error>> {
        let codec = XmlCodec;
        let ctx = DecodeContext;
        let artifact = codec.decode(br#"<user id="7"><name>Ada</name></user>"#, &ctx)?;

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
        Ok(())
    }

    #[test]
    fn xml_codec_preserves_comments_when_encoding_markup() -> Result<(), Box<dyn std::error::Error>>
    {
        let codec = XmlCodec;
        let ctx = DecodeContext;
        let artifact = codec.decode(b"<root><!--x--></root>", &ctx)?;
        let encoded = codec.encode(&artifact, &EncodeContext { pretty: false })?;

        assert_eq!(String::from_utf8(encoded)?, "<root><!--x--></root>");
        Ok(())
    }
}
