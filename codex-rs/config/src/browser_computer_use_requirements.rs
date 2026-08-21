use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AllowDenyRequirementToml {
    Allow,
    Deny,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct BrowserUseOriginPolicyToml {
    pub access: Option<AllowDenyRequirementToml>,
    pub downloads: Option<AllowDenyRequirementToml>,
    pub uploads: Option<AllowDenyRequirementToml>,
    pub full_cdp_access: Option<AllowDenyRequirementToml>,
    pub auto_review: Option<AllowDenyRequirementToml>,
    pub persistent_approval: Option<bool>,
    pub access_approval_lifetime: Option<BrowserUseAccessApprovalLifetimeToml>,
}

impl BrowserUseOriginPolicyToml {
    fn is_empty(&self) -> bool {
        self.access.is_none()
            && self.downloads.is_none()
            && self.uploads.is_none()
            && self.full_cdp_access.is_none()
            && self.auto_review.is_none()
            && self.persistent_approval.is_none()
            && self.access_approval_lifetime.is_none()
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BrowserUseAccessApprovalLifetimeToml {
    Turn,
    Thread,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct BrowserUseRequirementsToml {
    pub allow_history_access: Option<bool>,
    pub disable_auto_review: Option<bool>,
    pub allow_global_persistent_approval: Option<bool>,
    pub default_origin_policy: Option<BrowserUseOriginPolicyToml>,
    pub origins: Option<BTreeMap<String, BrowserUseOriginPolicyToml>>,
}

impl BrowserUseRequirementsToml {
    pub fn is_empty(&self) -> bool {
        self.allow_history_access.is_none()
            && self.disable_auto_review.is_none()
            && self.allow_global_persistent_approval.is_none()
            && self
                .default_origin_policy
                .as_ref()
                .is_none_or(BrowserUseOriginPolicyToml::is_empty)
            && self
                .origins
                .as_ref()
                .is_none_or(|origins| origins.values().all(BrowserUseOriginPolicyToml::is_empty))
    }
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct ComputerUseMacosRequirementsToml {
    pub bundle_ids: Option<BTreeMap<String, AllowDenyRequirementToml>>,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct ComputerUseWindowsRequirementsToml {
    pub aumids: Option<BTreeMap<String, AllowDenyRequirementToml>>,
    pub exes: Option<Vec<ComputerUseWindowsExeRequirementToml>>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ComputerUseWindowsExeRequirementToml {
    pub publisher_name: String,
    pub product_name: String,
    pub binary_name: Option<String>,
    pub access: AllowDenyRequirementToml,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct ComputerUseRequirementsToml {
    pub allow_locked_computer_use: Option<bool>,
    pub allow_persistent_approval: Option<bool>,
    pub default_app_access: Option<AllowDenyRequirementToml>,
    pub macos: Option<ComputerUseMacosRequirementsToml>,
    pub windows: Option<ComputerUseWindowsRequirementsToml>,
}

impl ComputerUseRequirementsToml {
    pub fn is_empty(&self) -> bool {
        self.allow_locked_computer_use.is_none()
            && self.allow_persistent_approval.is_none()
            && self.default_app_access.is_none()
            && self
                .macos
                .as_ref()
                .is_none_or(|macos| macos.bundle_ids.as_ref().is_none_or(BTreeMap::is_empty))
            && self.windows.as_ref().is_none_or(|windows| {
                windows.aumids.as_ref().is_none_or(BTreeMap::is_empty)
                    && windows.exes.as_ref().is_none_or(Vec::is_empty)
            })
    }
}
