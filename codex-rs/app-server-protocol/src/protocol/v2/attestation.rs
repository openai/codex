use crate::JsonSchema;
use crate::TS;
use codex_utils_redacted_string::RedactedString;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AttestationGenerateParams {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct AttestationGenerateResponse {
    /// Opaque client attestation token.
    #[ts(type = "string")]
    pub token: RedactedString,
}
