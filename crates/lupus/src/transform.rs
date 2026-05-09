use std::collections::BTreeMap;

use crate::data::{Data, Number};
use crate::error::ConvertError;
use crate::markup::{Markup, MarkupAttribute, MarkupElement, MarkupNode};

const TEXT_FIELD: &str = "#text";
const ATTR_PREFIX: char = '@';
const DEFAULT_ROOT: &str = "data";
const DEFAULT_ITEM: &str = "item";

pub fn data_into_markup(data: Data) -> Result<Markup, ConvertError> {
    if let Data::Object(fields) = &data
        && fields.len() == 1
        && let Some((key, value)) = fields.first_key_value()
        && !key.starts_with(ATTR_PREFIX)
        && key != TEXT_FIELD
    {
        return Ok(Markup {
            root: MarkupNode::Element(data_to_element(key, value)?),
        });
    }

    Ok(Markup {
        root: MarkupNode::Element(data_to_element(DEFAULT_ROOT, &data)?),
    })
}

pub fn markup_into_data(markup: Markup) -> Result<Data, ConvertError> {
    match &markup.root {
        MarkupNode::Element(element) => {
            let mut root = BTreeMap::new();
            root.insert(element.name.clone(), element_to_data(element)?);
            Ok(Data::Object(root))
        }
        node => node_to_data(node),
    }
}

pub fn data_to_text(data: &Data) -> String {
    match data {
        Data::Null => "null".to_string(),
        Data::Bool(value) => value.to_string(),
        Data::Number(Number::I64(value)) => value.to_string(),
        Data::Number(Number::U64(value)) => value.to_string(),
        Data::Number(Number::F64(value)) => value.to_string(),
        Data::String(value) => value.clone(),
        Data::Array(values) => values
            .iter()
            .map(data_to_text)
            .collect::<Vec<_>>()
            .join(","),
        Data::Object(fields) => fields
            .iter()
            .map(|(key, value)| format!("{key}:{}", data_to_text(value)))
            .collect::<Vec<_>>()
            .join(","),
    }
}

pub fn markup_to_text(markup: &Markup) -> String {
    node_text(&markup.root)
}
fn data_to_element(name: &str, data: &Data) -> Result<MarkupElement, ConvertError> {
    let name = markup_name(name)?;
    let mut attributes = Vec::new();
    let mut children = Vec::new();

    match data {
        Data::Null => {}
        Data::Bool(_) | Data::Number(_) | Data::String(_) => {
            children.push(MarkupNode::Text(data_to_text(data)));
        }
        Data::Array(items) => {
            for item in items {
                children.push(MarkupNode::Element(data_to_element(DEFAULT_ITEM, item)?));
            }
        }
        Data::Object(fields) => {
            for (key, value) in fields {
                if let Some(attribute_name) = key.strip_prefix(ATTR_PREFIX) {
                    attributes.push(MarkupAttribute {
                        name: markup_name(attribute_name)?,
                        value: scalar_attribute_value(value)?,
                    });
                } else if key == TEXT_FIELD {
                    children.push(MarkupNode::Text(data_to_text(value)));
                } else if let Data::Array(items) = value {
                    for item in items {
                        children.push(MarkupNode::Element(data_to_element(key, item)?));
                    }
                } else {
                    children.push(MarkupNode::Element(data_to_element(key, value)?));
                }
            }
        }
    }

    Ok(MarkupElement {
        name,
        attributes,
        children,
    })
}

fn node_to_data(node: &MarkupNode) -> Result<Data, ConvertError> {
    match node {
        MarkupNode::Element(element) => element_to_data(element),
        MarkupNode::Text(text) | MarkupNode::CData(text) => Ok(Data::String(text.clone())),
        MarkupNode::Comment(_) | MarkupNode::Doctype(_)  => Ok(Data::Null),
        MarkupNode::Comment(_) => Err(ConvertError::LossyConversionRefused(
            "comments are not represented in data".to_string(),
        )),
        MarkupNode::Doctype(_) => Err(ConvertError::LossyConversionRefused(
            "doctypes are not represented in data".to_string(),
        )),
    }
}

fn element_to_data(element: &MarkupElement) -> Result<Data, ConvertError> {
    let mut fields = BTreeMap::new();
    let mut text = String::new();

    for attribute in &element.attributes {
        fields.insert(
            format!("{ATTR_PREFIX}{}", attribute.name),
            Data::String(attribute.value.clone()),
        );
    }

    for child in &element.children {
        match child {
            MarkupNode::Element(child_element) => {
                let value = element_to_data(child_element)?;
                insert_repeated_field(&mut fields, child_element.name.clone(), value);
            }
            MarkupNode::Text(value) | MarkupNode::CData(value) => {
                text.push_str(value);
            }
            MarkupNode::Comment(_) | MarkupNode::Doctype(_)  => {}
            MarkupNode::Comment(_) => {
                return Err(ConvertError::LossyConversionRefused(
                    "comments are not represented in data".to_string(),
                ));
            }
            MarkupNode::Doctype(_) => {
                return Err(ConvertError::LossyConversionRefused(
                    "doctypes are not represented in data".to_string(),
                ));
            }
        }
    }

    if fields.is_empty() {
        if text.is_empty() {
            Ok(Data::Null)
        } else {
            Ok(Data::String(text))
        }
    } else {
        if !text.is_empty() {
            fields.insert(TEXT_FIELD.to_string(), Data::String(text));
        }
        Ok(Data::Object(fields))
    }
}

fn reject_lossy_markup(node: &MarkupNode) -> Result<(), ConvertError> {
    match node {
        MarkupNode::Element(element) => {
            let has_text = element
                .children
                .iter()
                .any(|child| matches!(child, MarkupNode::Text(_) | MarkupNode::CData(_)));
            let has_elements = element
                .children
                .iter()
                .any(|child| matches!(child, MarkupNode::Element(_)));

            if has_text && has_elements {
                return Err(ConvertError::LossyConversionRefused(
                    "data cannot preserve mixed child ordering".to_string(),
                ));
            }

            for child in &element.children {
                reject_lossy_markup(child)?;
            }
            Ok(())
        }
        MarkupNode::Text(_) => Ok(()),
        MarkupNode::CData(_) => Err(ConvertError::LossyConversionRefused(
            "data cannot preserve CDATA nodes".to_string(),
        )),
        MarkupNode::Comment(_) => Err(ConvertError::LossyConversionRefused(
            "data cannot preserve comments".to_string(),
        )),
        MarkupNode::Doctype(_) => Err(ConvertError::LossyConversionRefused(
            "data cannot preserve doctypes".to_string(),
        )),
    }
}

fn insert_repeated_field(fields: &mut BTreeMap<String, Data>, name: String, value: Data) {
    match fields.remove(&name) {
        Some(Data::Array(mut values)) => {
            values.push(value);
            fields.insert(name, Data::Array(values));
        }
        Some(existing) => {
            fields.insert(name, Data::Array(vec![existing, value]));
        }
        None => {
            fields.insert(name, value);
        }
    }
}

fn markup_name(name: &str) -> Result<String, ConvertError> {
    let sanitized = sanitize_markup_name(name);
    if sanitized != name  {
        return Err(ConvertError::LossyConversionRefused(format!(
            "name {name:?} must be sanitized to {sanitized:?}"
        )));
    }
    Ok(sanitized)
}

fn sanitize_markup_name(name: &str) -> String {
    let mut sanitized = String::new();
    for (index, ch) in name.chars().enumerate() {
        let valid = ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.');
        let ch = if valid { ch } else { '_' };
        if index == 0 && !(ch.is_ascii_alphabetic() || ch == '_') {
            sanitized.push('_');
        }
        sanitized.push(ch);
    }

    if sanitized.is_empty() {
        DEFAULT_ITEM.to_string()
    } else {
        sanitized
    }
}

fn scalar_attribute_value(data: &Data) -> Result<String, ConvertError> {
    match data {
        Data::Null | Data::Bool(_) | Data::Number(_) | Data::String(_) => Ok(data_to_text(data)),
        Data::Array(_) | Data::Object(_) => Ok(data_to_text(data)),
    }
}

fn node_text(node: &MarkupNode) -> String {
    match node {
        MarkupNode::Element(element) => element.children.iter().map(node_text).collect(),
        MarkupNode::Text(value) | MarkupNode::CData(value) => value.clone(),
        MarkupNode::Comment(_) | MarkupNode::Doctype(_) => String::new(),
    }
}
