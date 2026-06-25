mod method;
mod object;
mod value;

#[cfg(test)]
pub(crate) use method::Argument;
pub(crate) use method::Arguments;
pub(crate) use method::Method;
pub(crate) use object::ObjectSchema;
pub(crate) use value::JsonType;
pub(crate) use value::TypeSet;
pub(crate) use value::ValueSet;

use crate::parse::Parser;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SchemaId(pub(crate) usize);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiSchema {
    pub(crate) methods: BTreeMap<String, Method>,
    pub(crate) nodes: Vec<SchemaNode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaNode {
    Any,
    Never,
    Reference(SchemaId),
    Rules(Box<SchemaRules>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaRules {
    pub types: Option<TypeSet>,
    pub values: Option<ValueSet>,
    pub object: Option<ObjectSchema>,
}

impl ApiSchema {
    /// Parses a generated client-request JSON Schema into the representation used for comparison.
    pub fn parse(root: &Value) -> Result<Self> {
        let definitions = root
            .get("definitions")
            .map(|value| {
                value
                    .as_object()
                    .ok_or_else(|| anyhow!("ClientRequest definitions must be an object"))
            })
            .transpose()?
            .cloned()
            .unwrap_or_default();
        let mut schema = Self {
            methods: BTreeMap::new(),
            nodes: vec![SchemaNode::Any; definitions.len()],
        };
        let references = definitions
            .keys()
            .enumerate()
            .map(|(index, name)| {
                (
                    format!("#/definitions/{}", pointer_escape(name)),
                    SchemaId(index),
                )
            })
            .collect();
        let definition_ids = (0..definitions.len()).map(SchemaId).collect::<Vec<_>>();
        let mut parser = Parser::new(&mut schema, references);
        for ((name, value), id) in definitions.iter().zip(&definition_ids) {
            parser.schema.nodes[id.0] = parser
                .parse_node(value)
                .with_context(|| format!("parse JSON Schema definition {name}"))?;
        }
        for id in definition_ids {
            parser.schema.resolve(id)?;
        }
        parser.schema.methods = Method::parse_all(&mut parser, root)?;
        Ok(schema)
    }

    pub(crate) fn push(&mut self, node: SchemaNode) -> SchemaId {
        let id = SchemaId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    pub(crate) fn resolve(&self, mut id: SchemaId) -> Result<(SchemaId, &SchemaNode)> {
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(id) {
                return Err(anyhow!("cyclic direct JSON Schema reference"));
            }
            match self
                .nodes
                .get(id.0)
                .ok_or_else(|| anyhow!("invalid schema node {}", id.0))?
            {
                SchemaNode::Reference(target) => id = *target,
                node => return Ok((id, node)),
            }
        }
    }
}

fn pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
