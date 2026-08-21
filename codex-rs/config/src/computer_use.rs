use crate::AllowDenyRequirementToml;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ComputerUseConfigToml {
    pub default_app_access: Option<AllowDenyRequirementToml>,
    pub macos: Option<ComputerUseMacosConfigToml>,
    pub windows: Option<ComputerUseWindowsConfigToml>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ComputerUseMacosConfigToml {
    pub bundle_ids: Option<BTreeMap<String, AllowDenyRequirementToml>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ComputerUseWindowsConfigToml {
    pub aumids: Option<BTreeMap<String, AllowDenyRequirementToml>>,
    pub exes: Option<Vec<ComputerUseWindowsExeConfigToml>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ComputerUseWindowsExeConfigToml {
    pub publisher_name: String,
    pub product_name: String,
    pub binary_name: Option<String>,
    pub access: AllowDenyRequirementToml,
}

#[cfg(test)]
#[path = "computer_use_tests.rs"]
mod tests;
