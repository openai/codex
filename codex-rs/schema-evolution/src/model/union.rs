use crate::SchemaId;
use crate::parse::Parser;
use anyhow::Result;
use anyhow::anyhow;
use serde_json::Map;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnionKind {
    AnyOf,
    OneOf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnionSchema {
    pub kind: UnionKind,
    pub variants: Vec<SchemaId>,
}

impl UnionSchema {
    pub(crate) fn parse(
        parser: &mut Parser<'_>,
        object: &Map<String, Value>,
        keyword: &str,
        kind: UnionKind,
    ) -> Result<Option<Self>> {
        object
            .get(keyword)
            .map(|value| {
                let variants = value
                    .as_array()
                    .ok_or_else(|| anyhow!("JSON Schema {keyword} must be an array"))?
                    .iter()
                    .map(|value| parser.parse_schema(value))
                    .collect::<Result<_>>()?;
                Ok(Self { kind, variants })
            })
            .transpose()
    }
}
