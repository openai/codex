use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use serde_json::Map;
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum JsonType {
    Array,
    Boolean,
    Integer,
    Null,
    Number,
    Object,
    String,
}

impl JsonType {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "array" => Ok(Self::Array),
            "boolean" => Ok(Self::Boolean),
            "integer" => Ok(Self::Integer),
            "null" => Ok(Self::Null),
            "number" => Ok(Self::Number),
            "object" => Ok(Self::Object),
            "string" => Ok(Self::String),
            _ => Err(anyhow!("unsupported JSON Schema type {value}")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeSet {
    pub declared: BTreeSet<JsonType>,
}

impl TypeSet {
    pub(crate) fn parse(value: Option<&Value>) -> Result<Option<Self>> {
        let Some(value) = value else {
            return Ok(None);
        };
        let declared = match value {
            Value::String(value) => [JsonType::parse(value)?].into_iter().collect(),
            Value::Array(values) => values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| anyhow!("JSON Schema types must be strings"))
                        .and_then(JsonType::parse)
                })
                .collect::<Result<_>>()?,
            _ => bail!("JSON Schema type must be a string or array"),
        };
        Ok(Some(Self { declared }))
    }

    pub(crate) fn accepted_types(&self) -> BTreeSet<JsonType> {
        let mut accepted = self.declared.clone();
        if accepted.contains(&JsonType::Number) {
            accepted.insert(JsonType::Integer);
        }
        accepted
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueSet {
    pub values: Vec<Value>,
}

impl ValueSet {
    pub(crate) fn parse(object: &Map<String, Value>) -> Result<Option<Self>> {
        let values = object
            .get("enum")
            .map(|values| {
                values
                    .as_array()
                    .cloned()
                    .ok_or_else(|| anyhow!("JSON Schema enum must be an array"))
            })
            .transpose()?;
        let constant = object.get("const");
        let mut values = match (values, constant) {
            (None, None) => return Ok(None),
            (None, Some(value)) => vec![value.clone()],
            (Some(values), None) => values,
            (Some(mut values), Some(constant)) => {
                values.retain(|value| value == constant);
                values
            }
        };
        values.sort_by_key(|value| serde_json::to_string(value).unwrap_or_default());
        values.dedup();
        Ok(Some(Self { values }))
    }
}
