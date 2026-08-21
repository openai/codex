use super::AllowDenyRequirement;
use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct ComputerUseConfig {
    pub default_app_access: Option<AllowDenyRequirement>,
    pub macos: Option<ComputerUseMacosConfig>,
    pub windows: Option<ComputerUseWindowsConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct ComputerUseMacosConfig {
    pub bundle_ids: Option<BTreeMap<String, AllowDenyRequirement>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct ComputerUseWindowsConfig {
    pub aumids: Option<BTreeMap<String, AllowDenyRequirement>>,
    pub exes: Option<Vec<ComputerUseWindowsExeConfig>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct ComputerUseWindowsExeConfig {
    pub publisher_name: String,
    pub product_name: String,
    pub binary_name: Option<String>,
    pub access: AllowDenyRequirement,
}
