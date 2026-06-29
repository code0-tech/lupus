use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Data {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Data>),
    Object(BTreeMap<String, Data>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Number {
    I64(i64),
    U64(u64),
    F64(f64),
}
