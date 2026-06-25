use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaBreakage {
    pub kind: ViolationKind,
    pub method: String,
    pub path: String,
    pub before: Value,
    pub after: Value,
}
