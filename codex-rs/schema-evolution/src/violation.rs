use crate::TypeSet;
use crate::ValueSet;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::fmt;

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ViolationKind {
    AdditionalPropertiesNarrowed,
    ConstraintChanged,
    EnumNarrowed,
    MethodRemoved,
    PropertyRemoved,
    RequiredPropertyAdded,
    TypeNarrowed,
    UnionVariantRemoved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Violation {
    MethodRemoved {
        method: String,
    },
    PropertyRemoved {
        at: Location,
    },
    RequiredPropertyAdded {
        at: Location,
    },
    TypeNarrowed {
        at: Location,
        before: Option<TypeSet>,
        after: TypeSet,
    },
    EnumNarrowed {
        at: Location,
        before: Option<ValueSet>,
        after: ValueSet,
    },
    UnionVariantRemoved {
        at: Location,
        variant: VariantLabel,
    },
    AdditionalPropertiesNarrowed {
        at: Location,
        before: AdditionalPropertiesValue,
        after: AdditionalPropertiesValue,
    },
    ConstraintChanged {
        at: Location,
        before: SchemaSnapshot,
        after: SchemaSnapshot,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Location {
    pub scope: ViolationScope,
    pub path: SchemaPath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViolationScope {
    Method(String),
    SharedEnvelope,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct SchemaPath(Vec<PathSegment>);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum PathSegment {
    Property(String),
    Items,
    TupleItem(usize),
    AdditionalProperties,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantLabel(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdditionalPropertiesValue {
    Any,
    Forbidden,
    Schema(Value),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaSnapshot(pub Value);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaBreakage {
    pub kind: ViolationKind,
    pub method: String,
    pub path: String,
    pub before: Value,
    pub after: Value,
}

impl Location {
    pub(crate) fn method(method: &str, path: SchemaPath) -> Self {
        Self {
            scope: ViolationScope::Method(method.to_string()),
            path,
        }
    }
}

impl SchemaPath {
    pub(crate) fn property(&self, name: impl Into<String>) -> Self {
        let mut path = self.clone();
        path.0.push(PathSegment::Property(name.into()));
        path
    }

    pub(crate) fn items(&self) -> Self {
        let mut path = self.clone();
        path.0.push(PathSegment::Items);
        path
    }

    pub(crate) fn tuple_item(&self, index: usize) -> Self {
        let mut path = self.clone();
        path.0.push(PathSegment::TupleItem(index));
        path
    }

    pub(crate) fn additional_properties(&self) -> Self {
        let mut path = self.clone();
        path.0.push(PathSegment::AdditionalProperties);
        path
    }

    pub(crate) fn starts_with_params(&self) -> bool {
        matches!(self.0.first(), Some(PathSegment::Property(name)) if name == "params")
    }
}

impl fmt::Display for SchemaPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.0.iter().enumerate() {
            match segment {
                PathSegment::Property(name) if index == 0 => formatter.write_str(name)?,
                PathSegment::Property(name) => write!(formatter, ".{name}")?,
                PathSegment::Items => formatter.write_str("[]")?,
                PathSegment::TupleItem(index) => write!(formatter, "[{index}]")?,
                PathSegment::AdditionalProperties if index == 0 => formatter.write_str("*")?,
                PathSegment::AdditionalProperties => formatter.write_str(".*")?,
            }
        }
        Ok(())
    }
}

impl AdditionalPropertiesValue {
    fn to_json(&self) -> Value {
        match self {
            Self::Any => Value::Bool(true),
            Self::Forbidden => Value::Bool(false),
            Self::Schema(value) => value.clone(),
        }
    }
}

impl Violation {
    pub fn breakage(&self) -> SchemaBreakage {
        match self {
            Self::MethodRemoved { method } => SchemaBreakage {
                kind: ViolationKind::MethodRemoved,
                method: method.clone(),
                path: "request".to_string(),
                before: Value::Bool(true),
                after: Value::Bool(false),
            },
            Self::PropertyRemoved { at } => at_location(
                ViolationKind::PropertyRemoved,
                at,
                Value::Bool(true),
                Value::Bool(false),
            ),
            Self::RequiredPropertyAdded { at } => at_location(
                ViolationKind::RequiredPropertyAdded,
                at,
                Value::Bool(false),
                Value::Bool(true),
            ),
            Self::TypeNarrowed { at, before, after } => at_location(
                ViolationKind::TypeNarrowed,
                at,
                before.as_ref().map_or(Value::Null, TypeSet::to_json),
                after.to_json(),
            ),
            Self::EnumNarrowed { at, before, after } => at_location(
                ViolationKind::EnumNarrowed,
                at,
                before.as_ref().map_or(Value::Null, ValueSet::to_json),
                after.to_json(),
            ),
            Self::UnionVariantRemoved { at, variant } => at_location(
                ViolationKind::UnionVariantRemoved,
                at,
                Value::String(variant.0.clone()),
                Value::Null,
            ),
            Self::AdditionalPropertiesNarrowed { at, before, after } => at_location(
                ViolationKind::AdditionalPropertiesNarrowed,
                at,
                before.to_json(),
                after.to_json(),
            ),
            Self::ConstraintChanged { at, before, after } => at_location(
                ViolationKind::ConstraintChanged,
                at,
                before.0.clone(),
                after.0.clone(),
            ),
        }
    }

    pub(crate) fn location(&self) -> Option<&Location> {
        match self {
            Self::MethodRemoved { .. } => None,
            Self::PropertyRemoved { at }
            | Self::RequiredPropertyAdded { at }
            | Self::TypeNarrowed { at, .. }
            | Self::EnumNarrowed { at, .. }
            | Self::UnionVariantRemoved { at, .. }
            | Self::AdditionalPropertiesNarrowed { at, .. }
            | Self::ConstraintChanged { at, .. } => Some(at),
        }
    }

    pub(crate) fn set_shared_envelope(&mut self) {
        if let Some(location) = self.location_mut() {
            location.scope = ViolationScope::SharedEnvelope;
        }
    }

    fn location_mut(&mut self) -> Option<&mut Location> {
        match self {
            Self::MethodRemoved { .. } => None,
            Self::PropertyRemoved { at }
            | Self::RequiredPropertyAdded { at }
            | Self::TypeNarrowed { at, .. }
            | Self::EnumNarrowed { at, .. }
            | Self::UnionVariantRemoved { at, .. }
            | Self::AdditionalPropertiesNarrowed { at, .. }
            | Self::ConstraintChanged { at, .. } => Some(at),
        }
    }
}

fn at_location(kind: ViolationKind, at: &Location, before: Value, after: Value) -> SchemaBreakage {
    SchemaBreakage {
        kind,
        method: match &at.scope {
            ViolationScope::Method(method) => method.clone(),
            ViolationScope::SharedEnvelope => "*".to_string(),
        },
        path: match at.path.to_string() {
            path if path.is_empty() => "request".to_string(),
            path => path,
        },
        before,
        after,
    }
}

#[cfg(test)]
#[path = "violation_tests.rs"]
mod tests;
