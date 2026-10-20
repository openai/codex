//! Configuration for runtime assertions; enabling the feature is separate from selecting its mode.

use crate::FeatureConfig;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeAuthMode {
    /// Send the actor credential and a runtime assertion together during migration.
    #[default]
    DualHeader,
    /// Send only the runtime assertion as the request credential.
    AssertionOnly,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeAgentInstancesConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<RuntimeAuthMode>,
}

impl FeatureConfig for RuntimeAgentInstancesConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }
}
