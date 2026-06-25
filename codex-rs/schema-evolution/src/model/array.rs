use crate::JsonType;
use crate::SchemaId;
use crate::TypeSet;
use crate::parse::Parser;
use anyhow::Result;
use anyhow::bail;
use serde_json::Map;
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArraySchema {
    pub items: Items,
    pub additional_items: AdditionalItems,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Items {
    Any,
    Each(SchemaId),
    Tuple(Vec<SchemaId>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdditionalItems {
    Any,
    Forbidden,
    Schema(SchemaId),
}

impl ArraySchema {
    pub(crate) fn parse(
        parser: &mut Parser<'_>,
        object: &Map<String, Value>,
        types: Option<&TypeSet>,
    ) -> Result<Option<Self>> {
        let items = match object.get("items") {
            None if !types.is_some_and(|types| types.declared.contains(&JsonType::Array)) => {
                return Ok(None);
            }
            None | Some(Value::Bool(true)) => Items::Any,
            Some(value @ (Value::Bool(false) | Value::Object(_))) => {
                Items::Each(parser.parse_schema(value)?)
            }
            Some(Value::Array(values)) => Items::Tuple(
                values
                    .iter()
                    .map(|value| parser.parse_schema(value))
                    .collect::<Result<_>>()?,
            ),
            Some(_) => bail!("JSON Schema items must be a schema or array of schemas"),
        };
        let additional_items = match object.get("additionalItems") {
            None | Some(Value::Bool(true)) => AdditionalItems::Any,
            Some(Value::Bool(false)) => AdditionalItems::Forbidden,
            Some(value @ Value::Object(_)) => AdditionalItems::Schema(parser.parse_schema(value)?),
            Some(_) => bail!("JSON Schema additionalItems must be a boolean or object"),
        };
        Ok(Some(Self {
            items,
            additional_items,
        }))
    }
}
