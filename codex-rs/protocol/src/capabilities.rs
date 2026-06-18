use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// A user-selected root that can expose one or more runtime capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct SelectedCapabilityRoot {
    /// Stable identifier supplied by the capability selection platform.
    pub id: String,
    /// Where the selected root can be resolved.
    pub location: CapabilityRootLocation,
}

/// Location used to resolve a selected capability root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type")]
#[ts(export_to = "v2/")]
pub enum CapabilityRootLocation {
    /// A path owned by an execution environment.
    Environment {
        #[serde(rename = "environmentId")]
        #[ts(rename = "environmentId")]
        environment_id: String,
        path: String,
    },
}
