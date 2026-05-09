#[derive(Debug, Clone, PartialEq)]
pub struct Markup {
    pub root: MarkupNode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarkupNode {
    Element(MarkupElement),
    Text(String),
    CData(String),
    Comment(String),
    Doctype(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkupElement {
    pub name: String,
    pub attributes: Vec<MarkupAttribute>,
    pub children: Vec<MarkupNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkupAttribute {
    pub name: String,
    pub value: String,
}
