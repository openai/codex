use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use serde::de::Error as _;
use std::collections::BTreeMap;
use std::fmt;

use super::requirements_exec_policy::RequirementsExecPolicy;
use super::requirements_exec_policy::RequirementsExecPolicyToml;
use crate::Constrained;
use crate::ConstraintError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequirementSource {
    Unknown,
    MdmManagedPreferences { domain: String, key: String },
    CloudRequirements,
    SystemRequirementsToml { file: AbsolutePathBuf },
    LegacyManagedConfigTomlFromFile { file: AbsolutePathBuf },
    LegacyManagedConfigTomlFromMdm,
}

impl fmt::Display for RequirementSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequirementSource::Unknown => write!(f, "<unspecified>"),
            RequirementSource::MdmManagedPreferences { domain, key } => {
                write!(f, "MDM {domain}:{key}")
            }
            RequirementSource::CloudRequirements => {
                write!(f, "cloud requirements")
            }
            RequirementSource::SystemRequirementsToml { file } => {
                write!(f, "{}", file.as_path().display())
            }
            RequirementSource::LegacyManagedConfigTomlFromFile { file } => {
                write!(f, "{}", file.as_path().display())
            }
            RequirementSource::LegacyManagedConfigTomlFromMdm => {
                write!(f, "MDM managed_config.toml (legacy)")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstrainedWithSource<T> {
    pub value: Constrained<T>,
    pub source: Option<RequirementSource>,
}

impl<T> ConstrainedWithSource<T> {
    pub fn new(value: Constrained<T>, source: Option<RequirementSource>) -> Self {
        Self { value, source }
    }
}

impl<T> std::ops::Deref for ConstrainedWithSource<T> {
    type Target = Constrained<T>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> std::ops::DerefMut for ConstrainedWithSource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

/// Normalized version of [`ConfigRequirementsToml`] after deserialization and
/// normalization.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigRequirements {
    pub approval_policy: ConstrainedWithSource<AskForApproval>,
    pub approvals_reviewer: ConstrainedWithSource<ApprovalsReviewer>,
    pub sandbox_policy: ConstrainedWithSource<SandboxPolicy>,
    pub web_search_mode: ConstrainedWithSource<WebSearchMode>,
    pub feature_requirements: Option<Sourced<FeatureRequirementsToml>>,
    pub mcp_servers: Option<Sourced<BTreeMap<String, McpServerRequirement>>>,
    pub exec_policy: Option<Sourced<RequirementsExecPolicy>>,
    pub enforce_residency: ConstrainedWithSource<Option<ResidencyRequirement>>,
    /// Managed network constraints derived from requirements.
    pub network: Option<Sourced<NetworkConstraints>>,
    /// Managed filesystem constraints derived from requirements.
    pub filesystem: Option<Sourced<FilesystemConstraints>>,
}

impl Default for ConfigRequirements {
    fn default() -> Self {
        Self {
            approval_policy: ConstrainedWithSource::new(
                Constrained::allow_any_from_default(),
                /*source*/ None,
            ),
            approvals_reviewer: ConstrainedWithSource::new(
                Constrained::allow_any_from_default(),
                /*source*/ None,
            ),
            sandbox_policy: ConstrainedWithSource::new(
                Constrained::allow_any(SandboxPolicy::new_read_only_policy()),
                /*source*/ None,
            ),
            web_search_mode: ConstrainedWithSource::new(
                Constrained::allow_any(WebSearchMode::Cached),
                /*source*/ None,
            ),
            feature_requirements: None,
            mcp_servers: None,
            exec_policy: None,
            enforce_residency: ConstrainedWithSource::new(
                Constrained::allow_any(/*initial_value*/ None),
                /*source*/ None,
            ),
            network: None,
            filesystem: None,
        }
    }
}

impl ConfigRequirements {
    pub fn exec_policy_source(&self) -> Option<&RequirementSource> {
        self.exec_policy.as_ref().map(|policy| &policy.source)
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum McpServerIdentity {
    Command { command: String },
    Url { url: String },
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct McpServerRequirement {
    pub identity: McpServerIdentity,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkDomainPermissionsToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, NetworkDomainPermissionToml>,
}

impl NetworkDomainPermissionsToml {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn allowed_domains(&self) -> Option<Vec<String>> {
        let allowed_domains: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, permission)| matches!(permission, NetworkDomainPermissionToml::Allow))
            .map(|(pattern, _)| pattern.clone())
            .collect();
        (!allowed_domains.is_empty()).then_some(allowed_domains)
    }

    pub fn denied_domains(&self) -> Option<Vec<String>> {
        let denied_domains: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, permission)| matches!(permission, NetworkDomainPermissionToml::Deny))
            .map(|(pattern, _)| pattern.clone())
            .collect();
        (!denied_domains.is_empty()).then_some(denied_domains)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum NetworkDomainPermissionToml {
    Allow,
    Deny,
}

impl std::fmt::Display for NetworkDomainPermissionToml {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let permission = match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        };
        f.write_str(permission)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkUnixSocketPermissionsToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, NetworkUnixSocketPermissionToml>,
}

impl NetworkUnixSocketPermissionsToml {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn allow_unix_sockets(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, permission)| matches!(permission, NetworkUnixSocketPermissionToml::Allow))
            .map(|(path, _)| path.clone())
            .collect()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum NetworkUnixSocketPermissionToml {
    Allow,
    None,
}

impl std::fmt::Display for NetworkUnixSocketPermissionToml {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let permission = match self {
            Self::Allow => "allow",
            Self::None => "none",
        };
        f.write_str(permission)
    }
}

#[derive(Serialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkRequirementsToml {
    pub enabled: Option<bool>,
    pub http_port: Option<u16>,
    pub socks_port: Option<u16>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    pub domains: Option<NetworkDomainPermissionsToml>,
    /// When true, only managed `allowed_domains` are respected while managed
    /// network enforcement is active. User allowlist entries are ignored.
    pub managed_allowed_domains_only: Option<bool>,
    /// In danger-full-access mode, allow all network access and enforce managed deny entries.
    pub danger_full_access_denylist_only: Option<bool>,
    pub unix_sockets: Option<NetworkUnixSocketPermissionsToml>,
    pub allow_local_binding: Option<bool>,
}

#[derive(Deserialize)]
struct RawNetworkRequirementsToml {
    enabled: Option<bool>,
    http_port: Option<u16>,
    socks_port: Option<u16>,
    allow_upstream_proxy: Option<bool>,
    dangerously_allow_non_loopback_proxy: Option<bool>,
    dangerously_allow_all_unix_sockets: Option<bool>,
    domains: Option<NetworkDomainPermissionsToml>,
    #[serde(default)]
    allowed_domains: Option<Vec<String>>,
    /// When true, only managed `allowed_domains` are respected while managed
    /// network enforcement is active. User allowlist entries are ignored.
    managed_allowed_domains_only: Option<bool>,
    /// In danger-full-access mode, allow all network access and enforce managed deny entries.
    danger_full_access_denylist_only: Option<bool>,
    #[serde(default)]
    denied_domains: Option<Vec<String>>,
    unix_sockets: Option<NetworkUnixSocketPermissionsToml>,
    #[serde(default)]
    allow_unix_sockets: Option<Vec<String>>,
    allow_local_binding: Option<bool>,
}

impl<'de> Deserialize<'de> for NetworkRequirementsToml {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawNetworkRequirementsToml::deserialize(deserializer)?;
        let RawNetworkRequirementsToml {
            enabled,
            http_port,
            socks_port,
            allow_upstream_proxy,
            dangerously_allow_non_loopback_proxy,
            dangerously_allow_all_unix_sockets,
            domains,
            allowed_domains,
            managed_allowed_domains_only,
            danger_full_access_denylist_only,
            denied_domains,
            unix_sockets,
            allow_unix_sockets,
            allow_local_binding,
        } = raw;

        if domains.is_some() && (allowed_domains.is_some() || denied_domains.is_some()) {
            return Err(D::Error::custom(
                "`experimental_network.domains` cannot be combined with legacy `allowed_domains` or `denied_domains`",
            ));
        }

        if unix_sockets.is_some() && allow_unix_sockets.is_some() {
            return Err(D::Error::custom(
                "`experimental_network.unix_sockets` cannot be combined with legacy `allow_unix_sockets`",
            ));
        }

        Ok(Self {
            enabled,
            http_port,
            socks_port,
            allow_upstream_proxy,
            dangerously_allow_non_loopback_proxy,
            dangerously_allow_all_unix_sockets,
            domains: domains
                .or_else(|| legacy_domain_permissions_from_lists(allowed_domains, denied_domains)),
            managed_allowed_domains_only,
            danger_full_access_denylist_only,
            unix_sockets: unix_sockets
                .or_else(|| legacy_unix_socket_permissions_from_list(allow_unix_sockets)),
            allow_local_binding,
        })
    }
}

/// Legacy list normalization is intentionally lossy: explicit empty legacy
/// lists are treated as unset when converted to the canonical network
/// permission shape.
fn legacy_domain_permissions_from_lists(
    allowed_domains: Option<Vec<String>>,
    denied_domains: Option<Vec<String>>,
) -> Option<NetworkDomainPermissionsToml> {
    let mut entries = BTreeMap::new();

    for pattern in allowed_domains.unwrap_or_default() {
        entries.insert(pattern, NetworkDomainPermissionToml::Allow);
    }

    for pattern in denied_domains.unwrap_or_default() {
        entries.insert(pattern, NetworkDomainPermissionToml::Deny);
    }

    (!entries.is_empty()).then_some(NetworkDomainPermissionsToml { entries })
}

fn legacy_unix_socket_permissions_from_list(
    allow_unix_sockets: Option<Vec<String>>,
) -> Option<NetworkUnixSocketPermissionsToml> {
    let entries = allow_unix_sockets
        .unwrap_or_default()
        .into_iter()
        .map(|path| (path, NetworkUnixSocketPermissionToml::Allow))
        .collect::<BTreeMap<_, _>>();

    (!entries.is_empty()).then_some(NetworkUnixSocketPermissionsToml { entries })
}

/// Normalized network constraints derived from requirements TOML.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NetworkConstraints {
    pub enabled: Option<bool>,
    pub http_port: Option<u16>,
    pub socks_port: Option<u16>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    pub domains: Option<NetworkDomainPermissionsToml>,
    /// When true, only managed `allowed_domains` are respected while managed
    /// network enforcement is active. User allowlist entries are ignored.
    pub managed_allowed_domains_only: Option<bool>,
    /// In danger-full-access mode, allow all network access and enforce managed deny entries.
    pub danger_full_access_denylist_only: Option<bool>,
    pub unix_sockets: Option<NetworkUnixSocketPermissionsToml>,
    pub allow_local_binding: Option<bool>,
}

impl<'de> Deserialize<'de> for NetworkConstraints {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let requirements = NetworkRequirementsToml::deserialize(deserializer)?;
        Ok(requirements.into())
    }
}

impl From<NetworkRequirementsToml> for NetworkConstraints {
    fn from(value: NetworkRequirementsToml) -> Self {
        let NetworkRequirementsToml {
            enabled,
            http_port,
            socks_port,
            allow_upstream_proxy,
            dangerously_allow_non_loopback_proxy,
            dangerously_allow_all_unix_sockets,
            domains,
            managed_allowed_domains_only,
            danger_full_access_denylist_only,
            unix_sockets,
            allow_local_binding,
        } = value;
        Self {
            enabled,
            http_port,
            socks_port,
            allow_upstream_proxy,
            dangerously_allow_non_loopback_proxy,
            dangerously_allow_all_unix_sockets,
            domains,
            managed_allowed_domains_only,
            danger_full_access_denylist_only,
            unix_sockets,
            allow_local_binding,
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct FilesystemRequirementsToml {
    pub deny_read: Option<Vec<AbsolutePathBuf>>,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct PermissionsRequirementsToml {
    pub filesystem: Option<FilesystemRequirementsToml>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemConstraints {
    pub deny_read: Vec<AbsolutePathBuf>,
}

impl From<PermissionsRequirementsToml> for FilesystemConstraints {
    fn from(value: PermissionsRequirementsToml) -> Self {
        let deny_read = value
            .filesystem
            .and_then(|filesystem| filesystem.deny_read)
            .unwrap_or_default();
        Self { deny_read }
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum WebSearchModeRequirement {
    Disabled,
    Cached,
    Live,
}

impl From<WebSearchMode> for WebSearchModeRequirement {
    fn from(mode: WebSearchMode) -> Self {
        match mode {
            WebSearchMode::Disabled => WebSearchModeRequirement::Disabled,
            WebSearchMode::Cached => WebSearchModeRequirement::Cached,
            WebSearchMode::Live => WebSearchModeRequirement::Live,
        }
    }
}

impl From<WebSearchModeRequirement> for WebSearchMode {
    fn from(mode: WebSearchModeRequirement) -> Self {
        match mode {
            WebSearchModeRequirement::Disabled => WebSearchMode::Disabled,
            WebSearchModeRequirement::Cached => WebSearchMode::Cached,
            WebSearchModeRequirement::Live => WebSearchMode::Live,
        }
    }
}

impl fmt::Display for WebSearchModeRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebSearchModeRequirement::Disabled => write!(f, "disabled"),
            WebSearchModeRequirement::Cached => write!(f, "cached"),
            WebSearchModeRequirement::Live => write!(f, "live"),
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct FeatureRequirementsToml {
    #[serde(flatten)]
    pub entries: BTreeMap<String, bool>,
}

impl FeatureRequirementsToml {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct AppRequirementToml {
    pub enabled: Option<bool>,
}

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct AppsRequirementsToml {
    #[serde(default, flatten)]
    pub apps: BTreeMap<String, AppRequirementToml>,
}

impl AppsRequirementsToml {
    pub fn is_empty(&self) -> bool {
        self.apps.values().all(|app| app.enabled.is_none())
    }
}

/// Merge `enabled` configs from a lower-precedence source into an existing higher-precedence set.
/// This lets managed sources (for example Cloud/MDM) enforce setting disablement across layers.
/// Implemented with AppsRequirementsToml for now, could be abstracted if we have more enablement-style configs in the future.
pub(crate) fn merge_enablement_settings_descending(
    base: &mut AppsRequirementsToml,
    incoming: AppsRequirementsToml,
) {
    for (app_id, incoming_requirement) in incoming.apps {
        let base_requirement = base.apps.entry(app_id).or_default();
        let higher_precedence = base_requirement.enabled;
        let lower_precedence = incoming_requirement.enabled;
        base_requirement.enabled =
            if higher_precedence == Some(false) || lower_precedence == Some(false) {
                Some(false)
            } else {
                higher_precedence.or(lower_precedence)
            };
    }
}

/// Base config deserialized from system `requirements.toml` or MDM.
#[derive(Deserialize, Debug, Clone, Default, PartialEq)]
pub struct ConfigRequirementsToml {
    pub allowed_approval_policies: Option<Vec<AskForApproval>>,
    pub allowed_approvals_reviewers: Option<Vec<ApprovalsReviewer>>,
    pub allowed_sandbox_modes: Option<Vec<SandboxModeRequirement>>,
    pub allowed_web_search_modes: Option<Vec<WebSearchModeRequirement>>,
    #[serde(rename = "features", alias = "feature_requirements")]
    pub feature_requirements: Option<FeatureRequirementsToml>,
    pub mcp_servers: Option<BTreeMap<String, McpServerRequirement>>,
    pub apps: Option<AppsRequirementsToml>,
    pub rules: Option<RequirementsExecPolicyToml>,
    pub enforce_residency: Option<ResidencyRequirement>,
    #[serde(rename = "experimental_network")]
    pub network: Option<NetworkRequirementsToml>,
    pub permissions: Option<PermissionsRequirementsToml>,
    pub guardian_policy_config: Option<String>,
}

/// Value paired with the requirement source it came from, for better error
/// messages.
#[derive(Debug, Clone, PartialEq)]
pub struct Sourced<T> {
    pub value: T,
    pub source: RequirementSource,
}

impl<T> Sourced<T> {
    pub fn new(value: T, source: RequirementSource) -> Self {
        Self { value, source }
    }
}

impl<T> std::ops::Deref for Sourced<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigRequirementsWithSources {
    pub allowed_approval_policies: Option<Sourced<Vec<AskForApproval>>>,
    pub allowed_approvals_reviewers: Option<Sourced<Vec<ApprovalsReviewer>>>,
    pub allowed_sandbox_modes: Option<Sourced<Vec<SandboxModeRequirement>>>,
    pub allowed_web_search_modes: Option<Sourced<Vec<WebSearchModeRequirement>>>,
    pub feature_requirements: Option<Sourced<FeatureRequirementsToml>>,
    pub mcp_servers: Option<Sourced<BTreeMap<String, McpServerRequirement>>>,
    pub apps: Option<Sourced<AppsRequirementsToml>>,
    pub rules: Option<Sourced<RequirementsExecPolicyToml>>,
    pub enforce_residency: Option<Sourced<ResidencyRequirement>>,
    pub network: Option<Sourced<NetworkRequirementsToml>>,
    pub permissions: Option<Sourced<PermissionsRequirementsToml>>,
    pub guardian_policy_config: Option<Sourced<String>>,
}

impl ConfigRequirementsWithSources {
    pub fn merge_unset_fields(&mut self, source: RequirementSource, other: ConfigRequirementsToml) {
        // For every field in `other` that is `Some`, if the corresponding field
        // in `self` is `None`, copy the value from `other` into `self`.
        macro_rules! fill_missing_take {
            ($base:expr, $other:expr, $source:expr, { $($field:ident),+ $(,)? }) => {
                $(
                    if $base.$field.is_none()
                        && let Some(value) = $other.$field.take()
                    {
                        $base.$field = Some(Sourced::new(value, $source.clone()));
                    }
                )+
            };
        }

        // Destructure without `..` so adding fields to `ConfigRequirementsToml`
        // forces this merge logic to be updated.
        let ConfigRequirementsToml {
            allowed_approval_policies: _,
            allowed_approvals_reviewers: _,
            allowed_sandbox_modes: _,
            allowed_web_search_modes: _,
            feature_requirements: _,
            mcp_servers: _,
            apps: _,
            rules: _,
            enforce_residency: _,
            network: _,
            permissions: _,
            guardian_policy_config: _,
        } = &other;

        let mut other = other;
        if other
            .guardian_policy_config
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            other.guardian_policy_config = None;
        }
        fill_missing_take!(
            self,
            other,
            source,
            {
                allowed_approval_policies,
                allowed_approvals_reviewers,
                allowed_sandbox_modes,
                allowed_web_search_modes,
                feature_requirements,
                mcp_servers,
                rules,
                enforce_residency,
                network,
                permissions,
                guardian_policy_config,
            }
        );

        if let Some(incoming_apps) = other.apps.take() {
            if let Some(existing_apps) = self.apps.as_mut() {
                merge_enablement_settings_descending(&mut existing_apps.value, incoming_apps);
            } else {
                self.apps = Some(Sourced::new(incoming_apps, source));
            }
        }
    }

    pub fn into_toml(self) -> ConfigRequirementsToml {
        let ConfigRequirementsWithSources {
            allowed_approval_policies,
            allowed_approvals_reviewers,
            allowed_sandbox_modes,
            allowed_web_search_modes,
            feature_requirements,
            mcp_servers,
            apps,
            rules,
            enforce_residency,
            network,
            permissions,
            guardian_policy_config,
        } = self;
        ConfigRequirementsToml {
            allowed_approval_policies: allowed_approval_policies.map(|sourced| sourced.value),
            allowed_approvals_reviewers: allowed_approvals_reviewers.map(|sourced| sourced.value),
            allowed_sandbox_modes: allowed_sandbox_modes.map(|sourced| sourced.value),
            allowed_web_search_modes: allowed_web_search_modes.map(|sourced| sourced.value),
            feature_requirements: feature_requirements.map(|sourced| sourced.value),
            mcp_servers: mcp_servers.map(|sourced| sourced.value),
            apps: apps.map(|sourced| sourced.value),
            rules: rules.map(|sourced| sourced.value),
            enforce_residency: enforce_residency.map(|sourced| sourced.value),
            network: network.map(|sourced| sourced.value),
            permissions: permissions.map(|sourced| sourced.value),
            guardian_policy_config: guardian_policy_config.map(|sourced| sourced.value),
        }
    }
}

/// Currently, `external-sandbox` is not supported in config.toml, but it is
/// supported through programmatic use.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum SandboxModeRequirement {
    #[serde(rename = "read-only")]
    ReadOnly,

    #[serde(rename = "workspace-write")]
    WorkspaceWrite,

    #[serde(rename = "danger-full-access")]
    DangerFullAccess,

    #[serde(rename = "external-sandbox")]
    ExternalSandbox,
}

impl From<SandboxMode> for SandboxModeRequirement {
    fn from(mode: SandboxMode) -> Self {
        match mode {
            SandboxMode::ReadOnly => SandboxModeRequirement::ReadOnly,
            SandboxMode::WorkspaceWrite => SandboxModeRequirement::WorkspaceWrite,
            SandboxMode::DangerFullAccess => SandboxModeRequirement::DangerFullAccess,
        }
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResidencyRequirement {
    Us,
}

impl ConfigRequirementsToml {
    pub fn is_empty(&self) -> bool {
        self.allowed_approval_policies.is_none()
            && self.allowed_approvals_reviewers.is_none()
            && self.allowed_sandbox_modes.is_none()
            && self.allowed_web_search_modes.is_none()
            && self
                .feature_requirements
                .as_ref()
                .is_none_or(FeatureRequirementsToml::is_empty)
            && self.mcp_servers.is_none()
            && self
                .apps
                .as_ref()
                .is_none_or(AppsRequirementsToml::is_empty)
            && self.rules.is_none()
            && self.enforce_residency.is_none()
            && self.network.is_none()
            && self.permissions.is_none()
            && self
                .guardian_policy_config
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
    }
}

impl TryFrom<ConfigRequirementsWithSources> for ConfigRequirements {
    type Error = ConstraintError;

    fn try_from(toml: ConfigRequirementsWithSources) -> Result<Self, Self::Error> {
        let ConfigRequirementsWithSources {
            allowed_approval_policies,
            allowed_approvals_reviewers,
            allowed_sandbox_modes,
            allowed_web_search_modes,
            feature_requirements,
            mcp_servers,
            apps: _apps,
            rules,
            enforce_residency,
            network,
            permissions,
            guardian_policy_config: _guardian_policy_config,
        } = toml;

        let approval_policy = match allowed_approval_policies {
            Some(Sourced {
                value: policies,
                source: requirement_source,
            }) => {
                let Some(initial_value) = policies.first().copied() else {
                    return Err(ConstraintError::empty_field("allowed_approval_policies"));
                };

                let requirement_source_for_error = requirement_source.clone();
                let constrained = Constrained::new(initial_value, move |candidate| {
                    if policies.contains(candidate) {
                        Ok(())
                    } else {
                        Err(ConstraintError::InvalidValue {
                            field_name: "approval_policy",
                            candidate: format!("{candidate:?}"),
                            allowed: format!("{policies:?}"),
                            requirement_source: requirement_source_for_error.clone(),
                        })
                    }
                })?;
                ConstrainedWithSource::new(constrained, Some(requirement_source))
            }
            None => ConstrainedWithSource::new(
                Constrained::allow_any_from_default(),
                /*source*/ None,
            ),
        };

        let approvals_reviewer = match allowed_approvals_reviewers {
            Some(Sourced {
                value: reviewers,
                source: requirement_source,
            }) => {
                let Some(initial_value) = reviewers.first().copied() else {
                    return Err(ConstraintError::empty_field("allowed_approvals_reviewers"));
                };

                let requirement_source_for_error = requirement_source.clone();
                let constrained = Constrained::new(initial_value, move |candidate| {
                    if reviewers.contains(candidate) {
                        Ok(())
                    } else {
                        Err(ConstraintError::InvalidValue {
                            field_name: "approvals_reviewer",
                            candidate: format!("{candidate:?}"),
                            allowed: format!("{reviewers:?}"),
                            requirement_source: requirement_source_for_error.clone(),
                        })
                    }
                })?;
                ConstrainedWithSource::new(constrained, Some(requirement_source))
            }
            None => ConstrainedWithSource::new(
                Constrained::allow_any_from_default(),
                /*source*/ None,
            ),
        };

        // TODO(gt): `ConfigRequirementsToml` should let the author specify the
        // default `SandboxPolicy`? Should do this for `AskForApproval` too?
        //
        // Currently, we force ReadOnly as the default policy because two of
        // the other variants (WorkspaceWrite, ExternalSandbox) require
        // additional parameters. Ultimately, we should expand the config
        // format to allow specifying those parameters.
        let default_sandbox_policy = SandboxPolicy::new_read_only_policy();
        let sandbox_policy = match allowed_sandbox_modes {
            Some(Sourced {
                value: modes,
                source: requirement_source,
            }) => {
                if !modes.contains(&SandboxModeRequirement::ReadOnly) {
                    return Err(ConstraintError::InvalidValue {
                        field_name: "allowed_sandbox_modes",
                        candidate: format!("{modes:?}"),
                        allowed: "must include 'read-only' to allow any SandboxPolicy".to_string(),
                        requirement_source,
                    });
                };

                let requirement_source_for_error = requirement_source.clone();
                let constrained = Constrained::new(default_sandbox_policy, move |candidate| {
                    let mode = match candidate {
                        SandboxPolicy::ReadOnly { .. } => SandboxModeRequirement::ReadOnly,
                        SandboxPolicy::WorkspaceWrite { .. } => {
                            SandboxModeRequirement::WorkspaceWrite
                        }
                        SandboxPolicy::DangerFullAccess => SandboxModeRequirement::DangerFullAccess,
                        SandboxPolicy::ExternalSandbox { .. } => {
                            SandboxModeRequirement::ExternalSandbox
                        }
                    };
                    if modes.contains(&mode) {
                        Ok(())
                    } else {
                        Err(ConstraintError::InvalidValue {
                            field_name: "sandbox_mode",
                            candidate: format!("{mode:?}"),
                            allowed: format!("{modes:?}"),
                            requirement_source: requirement_source_for_error.clone(),
                        })
                    }
                })?;
                ConstrainedWithSource::new(constrained, Some(requirement_source))
            }
            None => {
                ConstrainedWithSource::new(
                    Constrained::allow_any(default_sandbox_policy),
                    /*source*/ None,
                )
            }
        };
        let exec_policy = match rules {
            Some(Sourced { value, source }) => {
                let policy = value.to_requirements_policy().map_err(|err| {
                    ConstraintError::ExecPolicyParse {
                        requirement_source: source.clone(),
                        reason: err.to_string(),
                    }
                })?;
                Some(Sourced::new(policy, source))
            }
            None => None,
        };
        let web_search_mode = match allowed_web_search_modes {
            Some(Sourced {
                value: modes,
                source: requirement_source,
            }) => {
                let mut accepted = modes.into_iter().collect::<std::collections::BTreeSet<_>>();
                accepted.insert(WebSearchModeRequirement::Disabled);
                let allowed_for_error = format!(
                    "{:?}",
                    accepted
                        .iter()
                        .copied()
                        .map(WebSearchMode::from)
                        .collect::<Vec<_>>()
                );

                let initial_value = if accepted.contains(&WebSearchModeRequirement::Cached) {
                    WebSearchMode::Cached
                } else if accepted.contains(&WebSearchModeRequirement::Live) {
                    WebSearchMode::Live
                } else {
                    WebSearchMode::Disabled
                };
                let requirement_source_for_error = requirement_source.clone();
                let constrained = Constrained::new(initial_value, move |candidate| {
                    if accepted.contains(&(*candidate).into()) {
                        Ok(())
                    } else {
                        Err(ConstraintError::InvalidValue {
                            field_name: "web_search_mode",
                            candidate: format!("{candidate:?}"),
                            allowed: allowed_for_error.clone(),
                            requirement_source: requirement_source_for_error.clone(),
                        })
                    }
                })?;
                ConstrainedWithSource::new(constrained, Some(requirement_source))
            }
            None => ConstrainedWithSource::new(
                Constrained::allow_any(WebSearchMode::Cached),
                /*source*/ None,
            ),
        };
        let feature_requirements =
            feature_requirements.filter(|requirements| !requirements.value.is_empty());

        let enforce_residency = match enforce_residency {
            Some(Sourced {
                value: residency,
                source: requirement_source,
            }) => {
                let required = Some(residency);
                let requirement_source_for_error = requirement_source.clone();
                let constrained = Constrained::new(required, move |candidate| {
                    if candidate == &required {
                        Ok(())
                    } else {
                        Err(ConstraintError::InvalidValue {
                            field_name: "enforce_residency",
                            candidate: format!("{candidate:?}"),
                            allowed: format!("{required:?}"),
                            requirement_source: requirement_source_for_error.clone(),
                        })
                    }
                })?;
                ConstrainedWithSource::new(constrained, Some(requirement_source))
            }
            None => ConstrainedWithSource::new(
                Constrained::allow_any(/*initial_value*/ None),
                /*source*/ None,
            ),
        };
        let network = network.map(|sourced_network| {
            let Sourced { value, source } = sourced_network;
            Sourced::new(NetworkConstraints::from(value), source)
        });
        let filesystem = permissions.map(|sourced_permissions| {
            let Sourced { value, source } = sourced_permissions;
            Sourced::new(FilesystemConstraints::from(value), source)
        });
        Ok(ConfigRequirements {
            approval_policy,
            approvals_reviewer,
            sandbox_policy,
            web_search_mode,
            feature_requirements,
            mcp_servers,
            exec_policy,
            enforce_residency,
            network,
            filesystem,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use codex_execpolicy::Decision;
    use codex_execpolicy::Evaluation;
    use codex_execpolicy::RuleMatch;
    use codex_protocol::protocol::NetworkAccess;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use toml::from_str;

    fn tokens(cmd: &[&str]) -> Vec<String> {
        cmd.iter().map(std::string::ToString::to_string).collect()
    }

    fn system_requirements_toml_file_for_test() -> Result<AbsolutePathBuf> {
        Ok(AbsolutePathBuf::try_from(
            std::env::temp_dir().join("requirements.toml"),
        )?)
    }

    fn with_unknown_source(toml: ConfigRequirementsToml) -> ConfigRequirementsWithSources {
        let ConfigRequirementsToml {
            allowed_approval_policies,
            allowed_approvals_reviewers,
            allowed_sandbox_modes,
            allowed_web_search_modes,
            feature_requirements,
            mcp_servers,
            apps,
            rules,
            enforce_residency,
            network,
            permissions,
            guardian_policy_config,
        } = toml;
        ConfigRequirementsWithSources {
            allowed_approval_policies: allowed_approval_policies
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
            allowed_approvals_reviewers: allowed_approvals_reviewers
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
            allowed_sandbox_modes: allowed_sandbox_modes
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
            allowed_web_search_modes: allowed_web_search_modes
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
            feature_requirements: feature_requirements
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
            mcp_servers: mcp_servers.map(|value| Sourced::new(value, RequirementSource::Unknown)),
            apps: apps.map(|value| Sourced::new(value, RequirementSource::Unknown)),
            rules: rules.map(|value| Sourced::new(value, RequirementSource::Unknown)),
            enforce_residency: enforce_residency
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
            network: network.map(|value| Sourced::new(value, RequirementSource::Unknown)),
            permissions: permissions.map(|value| Sourced::new(value, RequirementSource::Unknown)),
            guardian_policy_config: guardian_policy_config
                .map(|value| Sourced::new(value, RequirementSource::Unknown)),
        }
    }

    #[test]
    fn merge_unset_fields_copies_every_field_and_sets_sources() {
        let mut target = ConfigRequirementsWithSources::default();
        let source = RequirementSource::LegacyManagedConfigTomlFromMdm;

        let allowed_approval_policies = vec![AskForApproval::UnlessTrusted, AskForApproval::Never];
        let allowed_approvals_reviewers =
            vec![ApprovalsReviewer::GuardianSubagent, ApprovalsReviewer::User];
        let allowed_sandbox_modes = vec![
            SandboxModeRequirement::WorkspaceWrite,
            SandboxModeRequirement::DangerFullAccess,
        ];
        let allowed_web_search_modes = vec![
            WebSearchModeRequirement::Cached,
            WebSearchModeRequirement::Live,
        ];
        let feature_requirements = FeatureRequirementsToml {
            entries: BTreeMap::from([("personality".to_string(), true)]),
        };
        let enforce_residency = ResidencyRequirement::Us;
        let enforce_source = source.clone();
        let guardian_policy_config = "Use the company-managed guardian policy.".to_string();

        // Intentionally constructed without `..Default::default()` so adding a new field to
        // `ConfigRequirementsToml` forces this test to be updated.
        let other = ConfigRequirementsToml {
            allowed_approval_policies: Some(allowed_approval_policies.clone()),
            allowed_approvals_reviewers: Some(allowed_approvals_reviewers.clone()),
            allowed_sandbox_modes: Some(allowed_sandbox_modes.clone()),
            allowed_web_search_modes: Some(allowed_web_search_modes.clone()),
            feature_requirements: Some(feature_requirements.clone()),
            mcp_servers: None,
            apps: None,
            rules: None,
            enforce_residency: Some(enforce_residency),
            network: None,
            permissions: None,
            guardian_policy_config: Some(guardian_policy_config.clone()),
        };

        target.merge_unset_fields(source.clone(), other);

        assert_eq!(
            target,
            ConfigRequirementsWithSources {
                allowed_approval_policies: Some(Sourced::new(
                    allowed_approval_policies,
                    source.clone()
                )),
                allowed_approvals_reviewers: Some(Sourced::new(
                    allowed_approvals_reviewers,
                    source.clone(),
                )),
                allowed_sandbox_modes: Some(Sourced::new(allowed_sandbox_modes, source.clone(),)),
                allowed_web_search_modes: Some(Sourced::new(
                    allowed_web_search_modes,
                    enforce_source.clone(),
                )),
                feature_requirements: Some(Sourced::new(
                    feature_requirements,
                    enforce_source.clone(),
                )),
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: Some(Sourced::new(enforce_residency, enforce_source)),
                network: None,
                permissions: None,
                guardian_policy_config: Some(Sourced::new(guardian_policy_config, source)),
            }
        );
    }

    #[test]
    fn merge_unset_fields_fills_missing_values() -> Result<()> {
        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
            "#,
        )?;

        let source_location = RequirementSource::MdmManagedPreferences {
            domain: "com.codex".to_string(),
            key: "allowed_approval_policies".to_string(),
        };

        let mut empty_target = ConfigRequirementsWithSources::default();
        empty_target.merge_unset_fields(source_location.clone(), source);
        assert_eq!(
            empty_target,
            ConfigRequirementsWithSources {
                allowed_approval_policies: Some(Sourced::new(
                    vec![AskForApproval::OnRequest],
                    source_location,
                )),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                feature_requirements: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
                guardian_policy_config: None,
            }
        );
        Ok(())
    }

    #[test]
    fn merge_unset_fields_does_not_overwrite_existing_values() -> Result<()> {
        let existing_source = RequirementSource::LegacyManagedConfigTomlFromMdm;
        let mut populated_target = ConfigRequirementsWithSources::default();
        let populated_requirements: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["never"]
            "#,
        )?;
        populated_target.merge_unset_fields(existing_source.clone(), populated_requirements);

        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
            "#,
        )?;
        let source_location = RequirementSource::MdmManagedPreferences {
            domain: "com.codex".to_string(),
            key: "allowed_approval_policies".to_string(),
        };
        populated_target.merge_unset_fields(source_location, source);

        assert_eq!(
            populated_target,
            ConfigRequirementsWithSources {
                allowed_approval_policies: Some(Sourced::new(
                    vec![AskForApproval::Never],
                    existing_source,
                )),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                feature_requirements: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
                guardian_policy_config: None,
            }
        );
        Ok(())
    }

    #[test]
    fn merge_unset_fields_ignores_blank_guardian_override() {
        let mut target = ConfigRequirementsWithSources::default();
        target.merge_unset_fields(
            RequirementSource::CloudRequirements,
            ConfigRequirementsToml {
                guardian_policy_config: Some("   \n\t".to_string()),
                ..Default::default()
            },
        );
        target.merge_unset_fields(
            RequirementSource::SystemRequirementsToml {
                file: system_requirements_toml_file_for_test()
                    .expect("system requirements.toml path"),
            },
            ConfigRequirementsToml {
                guardian_policy_config: Some("Use the system guardian policy.".to_string()),
                ..Default::default()
            },
        );

        assert_eq!(
            target.guardian_policy_config,
            Some(Sourced::new(
                "Use the system guardian policy.".to_string(),
                RequirementSource::SystemRequirementsToml {
                    file: system_requirements_toml_file_for_test()
                        .expect("system requirements.toml path"),
                },
            )),
        );
    }

    #[test]
    fn deserialize_guardian_policy_config() -> Result<()> {
        let requirements: ConfigRequirementsToml = from_str(
            r#"
guardian_policy_config = """
Use the cloud-managed guardian policy.
"""
"#,
        )?;

        assert_eq!(
            requirements.guardian_policy_config.as_deref(),
            Some("Use the cloud-managed guardian policy.\n")
        );
        Ok(())
    }

    #[test]
    fn blank_guardian_policy_config_is_empty() -> Result<()> {
        let requirements: ConfigRequirementsToml = from_str(
            r#"
guardian_policy_config = """

"""
"#,
        )?;

        assert!(requirements.is_empty());
        Ok(())
    }

    #[test]
    fn allowed_approvals_reviewers_is_not_empty() -> Result<()> {
        let requirements: ConfigRequirementsToml = from_str(
            r#"
allowed_approvals_reviewers = ["user"]
"#,
        )?;

        assert!(!requirements.is_empty());
        Ok(())
    }

    #[test]
    fn deserialize_filesystem_deny_read_requirements() -> Result<()> {
        let deny_read_0 = if cfg!(windows) {
            r"C:\Users\viyatb\.gitconfig"
        } else {
            "/home/viyatb/.gitconfig"
        };
        let deny_read_1 = if cfg!(windows) {
            r"C:\Users\viyatb\.ssh"
        } else {
            "/home/viyatb/.ssh"
        };
        let toml_str = format!(
            r#"
            [permissions.filesystem]
            deny_read = [{deny_read_0:?}, {deny_read_1:?}]
        "#
        );

        let config: ConfigRequirementsToml = from_str(&toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;

        assert_eq!(
            requirements.filesystem,
            Some(Sourced::new(
                FilesystemConstraints {
                    deny_read: vec![
                        AbsolutePathBuf::from_absolute_path(deny_read_0)?,
                        AbsolutePathBuf::from_absolute_path(deny_read_1)?,
                    ],
                },
                RequirementSource::Unknown,
            ))
        );
        Ok(())
    }

    #[test]
    fn deserialize_apps_requirements() -> Result<()> {
        let toml_str = r#"
            [apps.connector_123123]
            enabled = false
        "#;
        let requirements: ConfigRequirementsToml = from_str(toml_str)?;

        assert_eq!(
            requirements.apps,
            Some(AppsRequirementsToml {
                apps: BTreeMap::from([(
                    "connector_123123".to_string(),
                    AppRequirementToml {
                        enabled: Some(false),
                    },
                )]),
            })
        );
        Ok(())
    }

    fn apps_requirements(entries: &[(&str, Option<bool>)]) -> AppsRequirementsToml {
        AppsRequirementsToml {
            apps: entries
                .iter()
                .map(|(app_id, enabled)| {
                    (
                        (*app_id).to_string(),
                        AppRequirementToml { enabled: *enabled },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn merge_enablement_settings_descending_unions_distinct_apps() {
        let mut merged = apps_requirements(&[("connector_high", Some(false))]);
        let lower = apps_requirements(&[("connector_low", Some(true))]);

        merge_enablement_settings_descending(&mut merged, lower);

        assert_eq!(
            merged,
            apps_requirements(&[
                ("connector_high", Some(false)),
                ("connector_low", Some(true))
            ]),
        );
    }

    #[test]
    fn merge_enablement_settings_descending_prefers_false_from_lower_precedence() {
        let mut merged = apps_requirements(&[("connector_123123", Some(true))]);
        let lower = apps_requirements(&[("connector_123123", Some(false))]);

        merge_enablement_settings_descending(&mut merged, lower);

        assert_eq!(
            merged,
            apps_requirements(&[("connector_123123", Some(false))]),
        );
    }

    #[test]
    fn merge_enablement_settings_descending_keeps_higher_true_when_lower_is_unset() {
        let mut merged = apps_requirements(&[("connector_123123", Some(true))]);
        let lower = apps_requirements(&[("connector_123123", None)]);

        merge_enablement_settings_descending(&mut merged, lower);

        assert_eq!(
            merged,
            apps_requirements(&[("connector_123123", Some(true))]),
        );
    }

    #[test]
    fn merge_enablement_settings_descending_uses_lower_value_when_higher_missing() {
        let mut merged = apps_requirements(&[]);
        let lower = apps_requirements(&[("connector_123123", Some(true))]);

        merge_enablement_settings_descending(&mut merged, lower);

        assert_eq!(
            merged,
            apps_requirements(&[("connector_123123", Some(true))]),
        );
    }

    #[test]
    fn merge_enablement_settings_descending_preserves_higher_false_when_lower_missing_app() {
        let mut merged = apps_requirements(&[("connector_123123", Some(false))]);
        let lower = apps_requirements(&[]);

        merge_enablement_settings_descending(&mut merged, lower);

        assert_eq!(
            merged,
            apps_requirements(&[("connector_123123", Some(false))]),
        );
    }

    #[test]
    fn merge_unset_fields_merges_apps_across_sources_with_enabled_evaluation() {
        let higher_source = RequirementSource::CloudRequirements;
        let lower_source = RequirementSource::LegacyManagedConfigTomlFromMdm;
        let mut target = ConfigRequirementsWithSources::default();

        target.merge_unset_fields(
            higher_source.clone(),
            ConfigRequirementsToml {
                apps: Some(apps_requirements(&[
                    ("connector_high", Some(true)),
                    ("connector_shared", Some(true)),
                ])),
                ..Default::default()
            },
        );
        target.merge_unset_fields(
            lower_source,
            ConfigRequirementsToml {
                apps: Some(apps_requirements(&[
                    ("connector_low", Some(false)),
                    ("connector_shared", Some(false)),
                ])),
                ..Default::default()
            },
        );

        let apps = target.apps.expect("apps should be present");
        assert_eq!(
            apps.value,
            apps_requirements(&[
                ("connector_high", Some(true)),
                ("connector_low", Some(false)),
                ("connector_shared", Some(false)),
            ])
        );
        assert_eq!(apps.source, higher_source);
    }

    #[test]
    fn merge_unset_fields_apps_empty_higher_source_does_not_block_lower_disables() {
        let mut target = ConfigRequirementsWithSources::default();

        target.merge_unset_fields(
            RequirementSource::CloudRequirements,
            ConfigRequirementsToml {
                apps: Some(apps_requirements(&[])),
                ..Default::default()
            },
        );
        target.merge_unset_fields(
            RequirementSource::LegacyManagedConfigTomlFromMdm,
            ConfigRequirementsToml {
                apps: Some(apps_requirements(&[("connector_123123", Some(false))])),
                ..Default::default()
            },
        );

        assert_eq!(
            target.apps.map(|apps| apps.value),
            Some(apps_requirements(&[("connector_123123", Some(false))])),
        );
    }

    #[test]
    fn constraint_error_includes_requirement_source() -> Result<()> {
        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
                allowed_approvals_reviewers = ["guardian_subagent"]
                allowed_sandbox_modes = ["read-only"]
            "#,
        )?;

        let requirements_toml_file = system_requirements_toml_file_for_test()?;
        let source_location = RequirementSource::SystemRequirementsToml {
            file: requirements_toml_file,
        };

        let mut target = ConfigRequirementsWithSources::default();
        target.merge_unset_fields(source_location.clone(), source);
        let requirements = ConfigRequirements::try_from(target)?;

        assert_eq!(
            requirements.approval_policy.can_set(&AskForApproval::Never),
            Err(ConstraintError::InvalidValue {
                field_name: "approval_policy",
                candidate: "Never".into(),
                allowed: "[OnRequest]".into(),
                requirement_source: source_location.clone(),
            })
        );
        assert_eq!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::DangerFullAccess),
            Err(ConstraintError::InvalidValue {
                field_name: "sandbox_mode",
                candidate: "DangerFullAccess".into(),
                allowed: "[ReadOnly]".into(),
                requirement_source: source_location.clone(),
            })
        );
        assert_eq!(
            requirements
                .approvals_reviewer
                .can_set(&ApprovalsReviewer::User),
            Err(ConstraintError::InvalidValue {
                field_name: "approvals_reviewer",
                candidate: "User".into(),
                allowed: "[GuardianSubagent]".into(),
                requirement_source: source_location,
            })
        );

        Ok(())
    }

    #[test]
    fn constraint_error_includes_cloud_requirements_source() -> Result<()> {
        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
            "#,
        )?;

        let source_location = RequirementSource::CloudRequirements;

        let mut target = ConfigRequirementsWithSources::default();
        target.merge_unset_fields(source_location.clone(), source);
        let requirements = ConfigRequirements::try_from(target)?;

        assert_eq!(
            requirements.approval_policy.can_set(&AskForApproval::Never),
            Err(ConstraintError::InvalidValue {
                field_name: "approval_policy",
                candidate: "Never".into(),
                allowed: "[OnRequest]".into(),
                requirement_source: source_location,
            })
        );

        Ok(())
    }

    #[test]
    fn constrained_fields_store_requirement_source() -> Result<()> {
        let source: ConfigRequirementsToml = from_str(
            r#"
                allowed_approval_policies = ["on-request"]
                allowed_approvals_reviewers = ["guardian_subagent"]
                allowed_sandbox_modes = ["read-only"]
                allowed_web_search_modes = ["cached"]
                enforce_residency = "us"
                [features]
                personality = true
            "#,
        )?;

        let source_location = RequirementSource::CloudRequirements;
        let mut target = ConfigRequirementsWithSources::default();
        target.merge_unset_fields(source_location.clone(), source);
        let requirements = ConfigRequirements::try_from(target)?;

        assert_eq!(
            requirements.approval_policy.source,
            Some(source_location.clone())
        );
        assert_eq!(
            requirements.approvals_reviewer.source,
            Some(source_location.clone())
        );
        assert_eq!(
            requirements.sandbox_policy.source,
            Some(source_location.clone())
        );
        assert_eq!(
            requirements.web_search_mode.source,
            Some(source_location.clone())
        );
        assert_eq!(
            requirements
                .feature_requirements
                .as_ref()
                .map(|requirements| requirements.source.clone()),
            Some(source_location.clone())
        );
        assert_eq!(requirements.enforce_residency.source, Some(source_location));

        Ok(())
    }

    #[test]
    fn deserialize_allowed_approval_policies() -> Result<()> {
        let toml_str = r#"
            allowed_approval_policies = ["untrusted", "on-request"]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;

        assert_eq!(
            requirements.approval_policy.value(),
            AskForApproval::UnlessTrusted,
            "currently, there is no way to specify the default value for approval policy in the toml, so it picks the first allowed value"
        );
        assert!(
            requirements
                .approval_policy
                .can_set(&AskForApproval::UnlessTrusted)
                .is_ok()
        );
        assert_eq!(
            requirements
                .approval_policy
                .can_set(&AskForApproval::OnFailure),
            Err(ConstraintError::InvalidValue {
                field_name: "approval_policy",
                candidate: "OnFailure".into(),
                allowed: "[UnlessTrusted, OnRequest]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );
        assert!(
            requirements
                .approval_policy
                .can_set(&AskForApproval::OnRequest)
                .is_ok()
        );
        assert_eq!(
            requirements.approval_policy.can_set(&AskForApproval::Never),
            Err(ConstraintError::InvalidValue {
                field_name: "approval_policy",
                candidate: "Never".into(),
                allowed: "[UnlessTrusted, OnRequest]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );
        assert!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::new_read_only_policy())
                .is_ok()
        );

        Ok(())
    }

    #[test]
    fn deserialize_allowed_approvals_reviewers() -> Result<()> {
        let toml_str = r#"
            allowed_approvals_reviewers = ["guardian_subagent", "user"]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;

        assert_eq!(
            requirements.approvals_reviewer.value(),
            ApprovalsReviewer::GuardianSubagent,
            "currently, there is no way to specify the default value for approvals reviewer in the toml, so it picks the first allowed value"
        );
        assert!(
            requirements
                .approvals_reviewer
                .can_set(&ApprovalsReviewer::GuardianSubagent)
                .is_ok()
        );
        assert!(
            requirements
                .approvals_reviewer
                .can_set(&ApprovalsReviewer::User)
                .is_ok()
        );

        Ok(())
    }

    #[test]
    fn empty_allowed_approvals_reviewers_is_rejected() -> Result<()> {
        let toml_str = r#"
            allowed_approvals_reviewers = []
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let err = ConfigRequirements::try_from(with_unknown_source(config))
            .expect_err("empty approvals reviewer allow-list should be rejected");

        assert_eq!(
            err,
            ConstraintError::EmptyField {
                field_name: "allowed_approvals_reviewers".to_string(),
            }
        );

        Ok(())
    }

    #[test]
    fn deserialize_allowed_sandbox_modes() -> Result<()> {
        let toml_str = r#"
            allowed_sandbox_modes = ["read-only", "workspace-write"]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;

        let root = if cfg!(windows) { "C:\\repo" } else { "/repo" };
        assert!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::new_read_only_policy())
                .is_ok()
        );
        assert!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::WorkspaceWrite {
                    writable_roots: vec![AbsolutePathBuf::from_absolute_path(root)?],
                    read_only_access: Default::default(),
                    network_access: false,
                    exclude_tmpdir_env_var: false,
                    exclude_slash_tmp: false,
                })
                .is_ok()
        );
        assert_eq!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::DangerFullAccess),
            Err(ConstraintError::InvalidValue {
                field_name: "sandbox_mode",
                candidate: "DangerFullAccess".into(),
                allowed: "[ReadOnly, WorkspaceWrite]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );
        assert_eq!(
            requirements
                .sandbox_policy
                .can_set(&SandboxPolicy::ExternalSandbox {
                    network_access: NetworkAccess::Restricted,
                }),
            Err(ConstraintError::InvalidValue {
                field_name: "sandbox_mode",
                candidate: "ExternalSandbox".into(),
                allowed: "[ReadOnly, WorkspaceWrite]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );

        Ok(())
    }

    #[test]
    fn deserialize_allowed_web_search_modes() -> Result<()> {
        let toml_str = r#"
            allowed_web_search_modes = ["cached"]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;

        assert_eq!(requirements.web_search_mode.value(), WebSearchMode::Cached);
        assert!(
            requirements
                .web_search_mode
                .can_set(&WebSearchMode::Disabled)
                .is_ok()
        );
        assert_eq!(
            requirements.web_search_mode.can_set(&WebSearchMode::Live),
            Err(ConstraintError::InvalidValue {
                field_name: "web_search_mode",
                candidate: "Live".into(),
                allowed: "[Disabled, Cached]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );
        assert!(
            requirements
                .web_search_mode
                .can_set(&WebSearchMode::Cached)
                .is_ok()
        );

        Ok(())
    }

    #[test]
    fn allowed_web_search_modes_allows_disabled() -> Result<()> {
        let toml_str = r#"
            allowed_web_search_modes = ["disabled"]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;

        assert_eq!(
            requirements.web_search_mode.value(),
            WebSearchMode::Disabled
        );
        assert!(
            requirements
                .web_search_mode
                .can_set(&WebSearchMode::Disabled)
                .is_ok()
        );
        assert_eq!(
            requirements.web_search_mode.can_set(&WebSearchMode::Cached),
            Err(ConstraintError::InvalidValue {
                field_name: "web_search_mode",
                candidate: "Cached".into(),
                allowed: "[Disabled]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );
        Ok(())
    }

    #[test]
    fn allowed_web_search_modes_empty_restricts_to_disabled() -> Result<()> {
        let toml_str = r#"
            allowed_web_search_modes = []
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;

        assert_eq!(
            requirements.web_search_mode.value(),
            WebSearchMode::Disabled
        );
        assert!(
            requirements
                .web_search_mode
                .can_set(&WebSearchMode::Disabled)
                .is_ok()
        );
        assert_eq!(
            requirements.web_search_mode.can_set(&WebSearchMode::Cached),
            Err(ConstraintError::InvalidValue {
                field_name: "web_search_mode",
                candidate: "Cached".into(),
                allowed: "[Disabled]".into(),
                requirement_source: RequirementSource::Unknown,
            })
        );
        Ok(())
    }

    #[test]
    fn deserialize_feature_requirements() -> Result<()> {
        let toml_str = r#"
            [features]
            apps = false
            personality = true
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;

        assert_eq!(
            requirements.feature_requirements,
            Some(Sourced::new(
                FeatureRequirementsToml {
                    entries: BTreeMap::from([
                        ("apps".to_string(), false),
                        ("personality".to_string(), true),
                    ]),
                },
                RequirementSource::Unknown,
            ))
        );

        Ok(())
    }

    #[test]
    fn network_requirements_are_preserved_as_constraints_with_source() -> Result<()> {
        let toml_str = r#"
            [experimental_network]
            enabled = true
            allow_upstream_proxy = false
            dangerously_allow_all_unix_sockets = true
            managed_allowed_domains_only = true
            danger_full_access_denylist_only = true
            allow_local_binding = false

            [experimental_network.domains]
            "api.example.com" = "allow"
            "*.openai.com" = "allow"
            "blocked.example.com" = "deny"

            [experimental_network.unix_sockets]
            "/tmp/example.sock" = "allow"
        "#;

        let source = RequirementSource::CloudRequirements;
        let mut requirements_with_sources = ConfigRequirementsWithSources::default();
        requirements_with_sources.merge_unset_fields(source.clone(), from_str(toml_str)?);

        let requirements = ConfigRequirements::try_from(requirements_with_sources)?;
        let sourced_network = requirements
            .network
            .expect("network requirements should be preserved as constraints");

        assert_eq!(sourced_network.source, source);
        assert_eq!(sourced_network.value.enabled, Some(true));
        assert_eq!(sourced_network.value.allow_upstream_proxy, Some(false));
        assert_eq!(
            sourced_network.value.dangerously_allow_all_unix_sockets,
            Some(true)
        );
        assert_eq!(
            sourced_network.value.domains.as_ref(),
            Some(&NetworkDomainPermissionsToml {
                entries: BTreeMap::from([
                    (
                        "*.openai.com".to_string(),
                        NetworkDomainPermissionToml::Allow,
                    ),
                    (
                        "api.example.com".to_string(),
                        NetworkDomainPermissionToml::Allow,
                    ),
                    (
                        "blocked.example.com".to_string(),
                        NetworkDomainPermissionToml::Deny,
                    ),
                ]),
            })
        );
        assert_eq!(
            sourced_network.value.managed_allowed_domains_only,
            Some(true)
        );
        assert_eq!(
            sourced_network.value.danger_full_access_denylist_only,
            Some(true)
        );
        assert_eq!(
            sourced_network.value.unix_sockets.as_ref(),
            Some(&NetworkUnixSocketPermissionsToml {
                entries: BTreeMap::from([(
                    "/tmp/example.sock".to_string(),
                    NetworkUnixSocketPermissionToml::Allow,
                )]),
            })
        );
        assert_eq!(sourced_network.value.allow_local_binding, Some(false));

        Ok(())
    }

    #[test]
    fn legacy_network_requirements_are_preserved_as_constraints_with_source() -> Result<()> {
        let toml_str = r#"
            [experimental_network]
            enabled = true
            allow_upstream_proxy = false
            dangerously_allow_all_unix_sockets = true
            allowed_domains = ["api.example.com", "*.openai.com"]
            managed_allowed_domains_only = true
            danger_full_access_denylist_only = true
            denied_domains = ["blocked.example.com"]
            allow_unix_sockets = ["/tmp/example.sock"]
            allow_local_binding = false
        "#;

        let source = RequirementSource::CloudRequirements;
        let mut requirements_with_sources = ConfigRequirementsWithSources::default();
        requirements_with_sources.merge_unset_fields(source.clone(), from_str(toml_str)?);

        let requirements = ConfigRequirements::try_from(requirements_with_sources)?;
        let sourced_network = requirements
            .network
            .expect("network requirements should be preserved as constraints");

        assert_eq!(sourced_network.source, source);
        assert_eq!(sourced_network.value.enabled, Some(true));
        assert_eq!(sourced_network.value.allow_upstream_proxy, Some(false));
        assert_eq!(
            sourced_network.value.dangerously_allow_all_unix_sockets,
            Some(true)
        );
        assert_eq!(
            sourced_network.value.domains.as_ref(),
            Some(&NetworkDomainPermissionsToml {
                entries: BTreeMap::from([
                    (
                        "*.openai.com".to_string(),
                        NetworkDomainPermissionToml::Allow,
                    ),
                    (
                        "api.example.com".to_string(),
                        NetworkDomainPermissionToml::Allow,
                    ),
                    (
                        "blocked.example.com".to_string(),
                        NetworkDomainPermissionToml::Deny,
                    ),
                ]),
            })
        );
        assert_eq!(
            sourced_network.value.managed_allowed_domains_only,
            Some(true)
        );
        assert_eq!(
            sourced_network.value.danger_full_access_denylist_only,
            Some(true)
        );
        assert_eq!(
            sourced_network.value.unix_sockets.as_ref(),
            Some(&NetworkUnixSocketPermissionsToml {
                entries: BTreeMap::from([(
                    "/tmp/example.sock".to_string(),
                    NetworkUnixSocketPermissionToml::Allow,
                )]),
            })
        );
        assert_eq!(sourced_network.value.allow_local_binding, Some(false));

        Ok(())
    }

    #[test]
    fn mixed_legacy_and_canonical_network_requirements_are_rejected() {
        let err = from_str::<ConfigRequirementsToml>(
            r#"
                [experimental_network]
                allowed_domains = ["api.example.com"]

                [experimental_network.domains]
                "*.openai.com" = "allow"
            "#,
        )
        .expect_err("mixed network domain shapes should fail");

        assert!(
            err.to_string()
                .contains("`experimental_network.domains` cannot be combined"),
            "unexpected error: {err:#}"
        );

        let err = from_str::<ConfigRequirementsToml>(
            r#"
                [experimental_network]
                allow_unix_sockets = ["/tmp/example.sock"]

                [experimental_network.unix_sockets]
                "/tmp/another.sock" = "allow"
            "#,
        )
        .expect_err("mixed network unix socket shapes should fail");

        assert!(
            err.to_string()
                .contains("`experimental_network.unix_sockets` cannot be combined"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn network_permission_containers_project_allowed_and_denied_entries() {
        let domains = NetworkDomainPermissionsToml {
            entries: BTreeMap::from([
                (
                    "*.openai.com".to_string(),
                    NetworkDomainPermissionToml::Allow,
                ),
                (
                    "api.example.com".to_string(),
                    NetworkDomainPermissionToml::Allow,
                ),
                (
                    "blocked.example.com".to_string(),
                    NetworkDomainPermissionToml::Deny,
                ),
            ]),
        };
        let unix_sockets = NetworkUnixSocketPermissionsToml {
            entries: BTreeMap::from([
                (
                    "/tmp/example.sock".to_string(),
                    NetworkUnixSocketPermissionToml::Allow,
                ),
                (
                    "/tmp/ignored.sock".to_string(),
                    NetworkUnixSocketPermissionToml::None,
                ),
            ]),
        };

        assert_eq!(
            domains.allowed_domains(),
            Some(vec![
                "*.openai.com".to_string(),
                "api.example.com".to_string()
            ])
        );
        assert_eq!(
            domains.denied_domains(),
            Some(vec!["blocked.example.com".to_string()])
        );
        assert_eq!(
            NetworkDomainPermissionsToml {
                entries: BTreeMap::from([(
                    "api.example.com".to_string(),
                    NetworkDomainPermissionToml::Allow,
                )]),
            }
            .denied_domains(),
            None
        );
        assert_eq!(
            unix_sockets.allow_unix_sockets(),
            vec!["/tmp/example.sock".to_string()]
        );
    }

    #[test]
    fn deserialize_mcp_server_requirements() -> Result<()> {
        let toml_str = r#"
            [mcp_servers.docs.identity]
            command = "codex-mcp"

            [mcp_servers.remote.identity]
            url = "https://example.com/mcp"
        "#;
        let requirements: ConfigRequirements =
            with_unknown_source(from_str(toml_str)?).try_into()?;

        assert_eq!(
            requirements.mcp_servers,
            Some(Sourced::new(
                BTreeMap::from([
                    (
                        "docs".to_string(),
                        McpServerRequirement {
                            identity: McpServerIdentity::Command {
                                command: "codex-mcp".to_string(),
                            },
                        },
                    ),
                    (
                        "remote".to_string(),
                        McpServerRequirement {
                            identity: McpServerIdentity::Url {
                                url: "https://example.com/mcp".to_string(),
                            },
                        },
                    ),
                ]),
                RequirementSource::Unknown,
            ))
        );
        Ok(())
    }

    #[test]
    fn deserialize_exec_policy_requirements() -> Result<()> {
        let toml_str = r#"
            [rules]
            prefix_rules = [
                { pattern = [{ token = "rm" }], decision = "forbidden" },
            ]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements: ConfigRequirements = with_unknown_source(config).try_into()?;
        let policy = requirements.exec_policy.expect("exec policy").value;

        assert_eq!(
            policy.as_ref().check(&tokens(&["rm", "-rf"]), &|_| {
                panic!("rule should match so heuristic should not be called");
            }),
            Evaluation {
                decision: Decision::Forbidden,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["rm"]),
                    decision: Decision::Forbidden,
                    resolved_program: None,
                    justification: None,
                }],
            }
        );

        Ok(())
    }

    #[test]
    fn exec_policy_error_includes_requirement_source() -> Result<()> {
        let toml_str = r#"
            [rules]
            prefix_rules = [
                { pattern = [{ token = "rm" }] },
            ]
        "#;
        let config: ConfigRequirementsToml = from_str(toml_str)?;
        let requirements_toml_file = system_requirements_toml_file_for_test()?;
        let source_location = RequirementSource::SystemRequirementsToml {
            file: requirements_toml_file,
        };

        let mut requirements_with_sources = ConfigRequirementsWithSources::default();
        requirements_with_sources.merge_unset_fields(source_location.clone(), config);
        let err = ConfigRequirements::try_from(requirements_with_sources)
            .expect_err("invalid exec policy");

        assert_eq!(
            err,
            ConstraintError::ExecPolicyParse {
                requirement_source: source_location,
                reason: "rules prefix_rule at index 0 is missing a decision".to_string(),
            }
        );

        Ok(())
    }
}
