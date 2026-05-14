use crate::FeatureConfig;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MultiAgentV2ConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub max_concurrent_threads_per_session: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 3600000))]
    pub min_wait_timeout_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 3600000))]
    pub max_wait_timeout_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 3600000))]
    pub default_wait_timeout_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_hint_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_hint_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_agent_usage_hint_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_usage_hint_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_spawn_agent_metadata: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub non_code_mode_only: Option<bool>,
}

impl FeatureConfig for MultiAgentV2ConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = Some(enabled);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AppsMcpPathOverrideConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl FeatureConfig for AppsMcpPathOverrideConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled.or(self.path.as_ref().map(|_| true))
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = Some(enabled);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NetworkProxyConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_socks5: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socks_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_socks5_udp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_upstream_proxy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<NetworkProxyModeToml>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domains: Option<BTreeMap<String, NetworkProxyDomainPermissionToml>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unix_sockets: Option<BTreeMap<String, NetworkProxyUnixSocketPermissionToml>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_local_binding: Option<bool>,
}

impl FeatureConfig for NetworkProxyConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = Some(enabled);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NetworkProxyModeToml {
    Limited,
    Full,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NetworkProxyDomainPermissionToml {
    Allow,
    Deny,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NetworkProxyUnixSocketPermissionToml {
    Allow,
    None,
}
