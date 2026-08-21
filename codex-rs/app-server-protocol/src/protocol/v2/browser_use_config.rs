use super::AllowDenyRequirement;
use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct BrowserUseConfig {
    pub allow_history_access: Option<bool>,
    pub default_origin_policy: Option<BrowserUseOriginPolicyConfig>,
    pub origins: Option<BTreeMap<String, BrowserUseOriginPolicyConfig>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export_to = "v2/")]
pub struct BrowserUseOriginPolicyConfig {
    pub access: Option<AllowDenyRequirement>,
    pub downloads: Option<AllowDenyRequirement>,
    pub uploads: Option<AllowDenyRequirement>,
    pub full_cdp_access: Option<AllowDenyRequirement>,
}
