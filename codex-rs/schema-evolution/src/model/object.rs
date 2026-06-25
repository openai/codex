use crate::SchemaId;
use crate::TypeSet;
use crate::parse::Parser;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use serde_json::Map;
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectSchema {
    pub properties: BTreeMap<String, Property>,
    pub required: BTreeSet<String>,
    pub additional_properties: AdditionalProperties,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Property {
    pub schema: SchemaId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdditionalProperties {
    Any,
    Forbidden,
    Schema(SchemaId),
}

impl ObjectSchema {
    pub(crate) fn parse(
        parser: &mut Parser<'_>,
        object: &Map<String, Value>,
        types: Option<&TypeSet>,
    ) -> Result<Option<Self>> {
        let properties = object
            .get("properties")
            .map(|value| {
                value
                    .as_object()
                    .ok_or_else(|| anyhow!("JSON Schema properties must be an object"))
            })
            .transpose()?;
        let required = parse_string_set(object.get("required"), "required")?;
        let has_object_keywords = properties.is_some()
            || object.contains_key("required")
            || object.contains_key("additionalProperties");
        if !has_object_keywords
            && !types.is_some_and(|types| types.declared.contains(&crate::JsonType::Object))
        {
            return Ok(None);
        }
        let properties = properties
            .into_iter()
            .flatten()
            .map(|(name, value)| {
                parser
                    .parse_schema(value)
                    .map(|schema| (name.clone(), Property { schema }))
            })
            .collect::<Result<_>>()?;
        let additional_properties = match object.get("additionalProperties") {
            None | Some(Value::Bool(true)) => AdditionalProperties::Any,
            Some(Value::Bool(false)) => AdditionalProperties::Forbidden,
            Some(value @ Value::Object(_)) => {
                AdditionalProperties::Schema(parser.parse_schema(value)?)
            }
            Some(_) => bail!("JSON Schema additionalProperties must be a boolean or object"),
        };
        Ok(Some(Self {
            properties,
            required,
            additional_properties,
        }))
    }
}

fn parse_string_set(value: Option<&Value>, keyword: &str) -> Result<BTreeSet<String>> {
    let Some(value) = value else {
        return Ok(BTreeSet::new());
    };
    value
        .as_array()
        .ok_or_else(|| anyhow!("JSON Schema {keyword} must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow!("JSON Schema {keyword} values must be strings"))
        })
        .collect()
}
