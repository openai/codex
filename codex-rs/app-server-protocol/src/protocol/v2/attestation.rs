use schemars::JsonSchema;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Deserialize;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, JsonSchema, TS, Default)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct AttestationGenerateParams {}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct AttestationGenerateResponse {
    /// Opaque client attestation token.
    pub token: String,
}
