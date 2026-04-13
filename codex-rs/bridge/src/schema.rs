use serde::Deserialize;
use serde::Serialize;

/// Language-neutral schema manifest generated from Rust bridge DTO declarations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeSchema {
    /// Schema namespace.
    pub namespace: String,
    /// Schema version incremented when bridge DTOs change incompatibly.
    pub version: u32,
    /// Types exported to Python.
    pub types: Vec<BridgeType>,
    /// Methods exported to Python service implementations.
    pub methods: Vec<BridgeMethod>,
}

/// Exported bridge struct or enum.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeType {
    /// Type name.
    pub name: String,
    /// Either `struct` or `enum`.
    pub kind: String,
    /// Struct fields.
    pub fields: Vec<BridgeField>,
    /// Enum variant names.
    pub variants: Vec<String>,
}

/// Exported bridge struct field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeField {
    /// Field name.
    pub name: String,
    /// Python-facing type expression.
    pub python_type: String,
    /// Whether the field is optional.
    pub optional: bool,
    /// Opaque field descriptor when Python should see bytes instead of the Rust semantic type.
    pub opaque: Option<OpaqueField>,
}

/// Metadata for a field carried as opaque bytes in Python.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaqueField {
    /// Stable codec label.
    pub codec: String,
    /// Rust type hidden behind the opaque Python bytes.
    pub rust_type: String,
}

/// Exported bridge method.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeMethod {
    /// Stable method name.
    pub name: String,
    /// Request type name.
    pub request: String,
    /// Response type name.
    pub response: String,
}
