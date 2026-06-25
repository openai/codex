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
            }
        }
        Ok(())
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
