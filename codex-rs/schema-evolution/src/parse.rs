use crate::ApiSchema;
use crate::ArraySchema;
use crate::ConstraintSet;
use crate::ObjectSchema;
use crate::SchemaId;
use crate::SchemaNode;
use crate::SchemaRules;
use crate::TypeSet;
use crate::UnionKind;
use crate::UnionSchema;
use crate::ValueSet;
use crate::model::annotation;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use serde_json::Map;
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) struct Parser<'a> {
    pub(crate) schema: &'a mut ApiSchema,
    references: BTreeMap<String, SchemaId>,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(schema: &'a mut ApiSchema, references: BTreeMap<String, SchemaId>) -> Self {
        Self { schema, references }
    }

    pub(crate) fn parse_schema(&mut self, value: &Value) -> Result<SchemaId> {
        let node = self.parse_node(value)?;
        Ok(self.schema.push(node))
    }

    pub(crate) fn parse_node(&mut self, value: &Value) -> Result<SchemaNode> {
        match value {
            Value::Bool(true) => return Ok(SchemaNode::Any),
            Value::Bool(false) => return Ok(SchemaNode::Never),
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref") {
                    return self.parse_reference(reference);
                }
                if let Some(alias) = single_all_of(object)? {
                    return Ok(SchemaNode::Reference(self.parse_schema(alias)?));
                }
                if object.contains_key("allOf") {
                    bail!("only single-branch JSON Schema allOf aliases are supported");
                }
            }
            _ => bail!("JSON Schema must be a boolean or object"),
        }
        let object = value
            .as_object()
            .ok_or_else(|| anyhow!("JSON Schema must be a boolean or object"))?;
        let types = TypeSet::parse(object.get("type"))?;
        let values = ValueSet::parse(object)?;
        let object_schema = ObjectSchema::parse(self, object, types.as_ref())?;
        let array = ArraySchema::parse(self, object, types.as_ref())?;
        let any_of = UnionSchema::parse(self, object, "anyOf", UnionKind::AnyOf)?;
        let one_of = UnionSchema::parse(self, object, "oneOf", UnionKind::OneOf)?;
        let constraints = ConstraintSet::parse(object)?;
        Ok(SchemaNode::Rules(Box::new(SchemaRules {
            types,
            values,
            object: object_schema,
            array,
            any_of,
            one_of,
            constraints,
        })))
    }

    fn parse_reference(&self, value: &Value) -> Result<SchemaNode> {
        let reference = value
            .as_str()
            .ok_or_else(|| anyhow!("JSON Schema $ref must be a string"))?;
        let target =
            self.references.get(reference).copied().ok_or_else(|| {
                anyhow!("unsupported or missing JSON Schema reference {reference}")
            })?;
        Ok(SchemaNode::Reference(target))
    }
}

fn single_all_of(object: &Map<String, Value>) -> Result<Option<&Value>> {
    let Some(value) = object.get("allOf") else {
        return Ok(None);
    };
    let branches = value
        .as_array()
        .ok_or_else(|| anyhow!("JSON Schema allOf must be an array"))?;
    Ok(
        (branches.len() == 1 && object.keys().all(|key| key == "allOf" || annotation(key)))
            .then(|| &branches[0]),
    )
}

#[cfg(test)]
use crate::Arguments;
#[cfg(test)]
use crate::model::Argument;

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
