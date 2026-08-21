use crate::AllowDenyRequirementToml;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct BrowserUseConfigToml {
    pub allow_history_access: Option<bool>,
    pub default_origin_policy: Option<BrowserUseOriginPolicyConfigToml>,
    pub origins: Option<BTreeMap<String, BrowserUseOriginPolicyConfigToml>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct BrowserUseOriginPolicyConfigToml {
    pub access: Option<AllowDenyRequirementToml>,
    pub downloads: Option<AllowDenyRequirementToml>,
    pub uploads: Option<AllowDenyRequirementToml>,
    pub full_cdp_access: Option<AllowDenyRequirementToml>,
}

#[cfg(test)]
#[path = "browser_use_tests.rs"]
mod tests;
