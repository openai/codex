use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum JsonValue {
    Null,
    Boolean(bool),
    Str(String),
    Num(f64),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}
