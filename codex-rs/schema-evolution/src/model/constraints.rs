use anyhow::Result;
use anyhow::anyhow;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LowerBound {
    ExclusiveMinimum,
    MinItems,
    MinLength,
    MinProperties,
    Minimum,
}

impl LowerBound {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExclusiveMinimum => "exclusiveMinimum",
            Self::MinItems => "minItems",
            Self::MinLength => "minLength",
            Self::MinProperties => "minProperties",
            Self::Minimum => "minimum",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UpperBound {
    ExclusiveMaximum,
    MaxItems,
    MaxLength,
    MaxProperties,
    Maximum,
}

impl UpperBound {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExclusiveMaximum => "exclusiveMaximum",
            Self::MaxItems => "maxItems",
            Self::MaxLength => "maxLength",
            Self::MaxProperties => "maxProperties",
            Self::Maximum => "maximum",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConstraintSet {
    pub lower_bounds: BTreeMap<LowerBound, Number>,
    pub upper_bounds: BTreeMap<UpperBound, Number>,
    pub unique_items: Option<bool>,
    pub opaque: BTreeMap<String, Value>,
}

impl ConstraintSet {
    pub(crate) fn parse(object: &Map<String, Value>) -> Result<Self> {
        let mut constraints = Self::default();
        for (key, value) in object {
            if annotation(key) || structural_keyword(key) {
                continue;
            }
            match key.as_str() {
                "minimum" => constraints.insert_lower(LowerBound::Minimum, value)?,
                "exclusiveMinimum" => {
                    constraints.insert_lower(LowerBound::ExclusiveMinimum, value)?
                }
                "minLength" => constraints.insert_lower(LowerBound::MinLength, value)?,
                "minItems" => constraints.insert_lower(LowerBound::MinItems, value)?,
                "minProperties" => constraints.insert_lower(LowerBound::MinProperties, value)?,
                "maximum" => constraints.insert_upper(UpperBound::Maximum, value)?,
                "exclusiveMaximum" => {
                    constraints.insert_upper(UpperBound::ExclusiveMaximum, value)?
                }
                "maxLength" => constraints.insert_upper(UpperBound::MaxLength, value)?,
                "maxItems" => constraints.insert_upper(UpperBound::MaxItems, value)?,
                "maxProperties" => constraints.insert_upper(UpperBound::MaxProperties, value)?,
                "uniqueItems" => {
                    constraints.unique_items = Some(
                        value
                            .as_bool()
                            .ok_or_else(|| anyhow!("JSON Schema uniqueItems must be a boolean"))?,
                    );
                }
                _ => {
                    if contains_reference(value) {
                        return Err(anyhow!(
                            "JSON Schema references inside {key} are not supported"
                        ));
                    }
                    constraints.opaque.insert(key.clone(), normalized(value));
                }
            }
        }
        Ok(constraints)
    }

    pub(crate) fn to_json(&self) -> Value {
        let mut values = serde_json::Map::new();
        for (kind, value) in &self.lower_bounds {
            values.insert(kind.as_str().to_string(), Value::Number(value.clone()));
        }
        for (kind, value) in &self.upper_bounds {
            values.insert(kind.as_str().to_string(), Value::Number(value.clone()));
        }
        if let Some(value) = self.unique_items {
            values.insert("uniqueItems".to_string(), Value::Bool(value));
        }
        values.extend(self.opaque.clone());
        Value::Object(values)
    }

    fn insert_lower(&mut self, kind: LowerBound, value: &Value) -> Result<()> {
        self.lower_bounds.insert(
            kind,
            value
                .as_number()
                .cloned()
                .ok_or_else(|| anyhow!("JSON Schema {} must be numeric", kind.as_str()))?,
        );
        Ok(())
    }

    fn insert_upper(&mut self, kind: UpperBound, value: &Value) -> Result<()> {
        self.upper_bounds.insert(
            kind,
            value
                .as_number()
                .cloned()
                .ok_or_else(|| anyhow!("JSON Schema {} must be numeric", kind.as_str()))?,
        );
        Ok(())
    }
}

pub(crate) fn normalized(value: &Value) -> Value {
    fn visit(value: &Value, parent: Option<&str>) -> Value {
        match value {
            Value::Array(values) => {
                let mut values = values
                    .iter()
                    .map(|value| visit(value, /*parent*/ None))
                    .collect::<Vec<_>>();
                if parent.is_some_and(|parent| {
                    matches!(
                        parent,
                        "allOf" | "anyOf" | "enum" | "oneOf" | "required" | "type"
                    )
                }) {
                    values.sort_by_key(|value| serde_json::to_string(value).unwrap_or_default());
                }
                Value::Array(values)
            }
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .filter(|(key, _)| !annotation(key))
                    .map(|(key, value)| (key.clone(), visit(value, Some(key))))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }
    visit(value, /*parent*/ None)
}

pub(crate) fn annotation(keyword: &str) -> bool {
    matches!(
        keyword,
        "$comment"
            | "$id"
            | "$schema"
            | "default"
            | "deprecated"
            | "description"
            | "examples"
            | "readOnly"
            | "title"
            | "writeOnly"
    )
}

fn contains_reference(value: &Value) -> bool {
    match value {
        Value::Array(values) => values.iter().any(contains_reference),
        Value::Object(object) => {
            object.contains_key("$ref") || object.values().any(contains_reference)
        }
        _ => false,
    }
}

fn structural_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "$ref"
            | "additionalItems"
            | "additionalProperties"
            | "anyOf"
            | "const"
            | "enum"
            | "items"
            | "oneOf"
            | "properties"
            | "required"
            | "type"
    )
}
