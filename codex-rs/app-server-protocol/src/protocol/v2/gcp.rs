//! Shared setup parameters for the built-in Google Cloud Vertex AI provider.

use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GcpSetupParams {
    pub project_id: String,
    /// Vertex AI location. Omission or null selects `global`.
    #[ts(optional = nullable)]
    pub location: Option<String>,
    /// Absolute path to a credential file on the app-server host.
    /// Omit or pass `null` to clear the active user configuration's override and use ADC.
    /// If another configuration layer would still supply a credential file, setup fails
    /// without saving changes.
    #[ts(optional = nullable)]
    pub credential_file: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct GcpSetupResponse {}
