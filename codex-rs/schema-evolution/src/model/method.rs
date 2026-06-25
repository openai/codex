use crate::JsonType;
use crate::SchemaId;
use crate::SchemaNode;
use crate::parse::Parser;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Method {
    pub name: String,
    pub arguments: Arguments,
    pub request: SchemaId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Arguments {
    None,
    Map(Argument),
    Value(Argument),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Argument {
    pub required: bool,
    pub schema: SchemaId,
}

impl Method {
    pub(crate) fn parse_all(
        parser: &mut Parser<'_>,
        root: &Value,
    ) -> Result<BTreeMap<String, Self>> {
        let variants = root
            .get("oneOf")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("ClientRequest schema must contain a oneOf array"))?;
        let mut methods = BTreeMap::new();
        for variant in variants {
            let request_id = parser.parse_schema(variant)?;
            let (_, request) = parser.schema.resolve(request_id)?;
            let SchemaNode::Rules(request) = request else {
                bail!("ClientRequest variant must be an object schema");
            };
            if request
                .types
                .as_ref()
                .is_some_and(|types| !types.accepted_types().contains(&JsonType::Object))
            {
                bail!("ClientRequest variant must accept objects");
            }
            let request_object = request
                .object
                .as_ref()
                .ok_or_else(|| anyhow!("ClientRequest variant must contain properties"))?;
            let method_property = request_object
                .properties
                .get("method")
                .ok_or_else(|| anyhow!("ClientRequest variant is missing method"))?;
            let method = singleton_string(parser, method_property.schema, "ClientRequest method")?;

            let arguments = match request_object.properties.get("params") {
                Some(property) => Arguments::classify(
                    parser,
                    Argument {
                        required: request_object.required.contains("params"),
                        schema: property.schema,
                    },
                )?,
                None => Arguments::None,
            };
            if methods
                .insert(
                    method.clone(),
                    Self {
                        name: method.clone(),
                        arguments,
                        request: request_id,
                    },
                )
                .is_some()
            {
                bail!("ClientRequest contains duplicate method {method}");
            }
        }
        Ok(methods)
    }
}

impl Arguments {
    fn classify(parser: &Parser<'_>, argument: Argument) -> Result<Self> {
        let (_, node) = parser.schema.resolve(argument.schema)?;
        let SchemaNode::Rules(rules) = node else {
            return Ok(Self::Value(argument));
        };
        if rules.object.is_some() {
            Ok(Self::Map(argument))
        } else {
            Ok(Self::Value(argument))
        }
    }
}

fn singleton_string(parser: &Parser<'_>, id: SchemaId, label: &str) -> Result<String> {
    let (_, node) = parser.schema.resolve(id)?;
    let SchemaNode::Rules(rules) = node else {
        bail!("{label} must use enum or const");
    };
    let values = rules
        .values
        .as_ref()
        .ok_or_else(|| anyhow!("{label} must use enum or const"))?;
    let [value] = values.values.as_slice() else {
        bail!("{label} must contain exactly one value");
    };
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{label} must be a string"))
}
