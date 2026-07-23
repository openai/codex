use crate::FeatureConfig;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodeModeConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Exact tool namespaces to omit from the code-mode nested tool surface.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_tool_namespaces: Option<Vec<String>>,
    /// Exact tool namespaces to expose only as direct model tools.
    /// These tools bypass deferral, remain top-level in code-mode-only sessions, and are omitted
    /// from the nested code-mode tool surface.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_only_tool_namespaces: Option<Vec<String>>,
}

impl FeatureConfig for CodeModeConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = Some(enabled);
    }
}

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
    /// Deprecated compatibility field. Its value is ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_hint_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_hint_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_agent_usage_hint_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_usage_hint_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_agent_mode_hint_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 64), regex(pattern = r"^[a-zA-Z0-9_-]+$"))]
    pub tool_namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_spawn_agent_metadata: Option<bool>,
    /// Exposes `model` and `reasoning_effort` on the multi-agent v2 spawn tool and adds
    /// corresponding guidance to root and subagent usage hints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expose_spawn_agent_model_overrides: Option<bool>,
    /// Expose the multi-agent v2 `wait_agent` tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_agent_enabled: Option<bool>,
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
pub struct TokenBudgetConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Number of tokens remaining before auto-compaction when the wrap-up reminder is emitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub reminder_threshold_tokens: Option<i64>,
    /// Reminder template. `{n_remaining}` is replaced with the tokens remaining before
    /// auto-compaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 2000))]
    pub reminder_message_template: Option<String>,
    /// Guidance appended to the context-window metadata in a developer message.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 2000))]
    pub guidance_message: Option<String>,
    /// Developer message sampled before an automatic context-window rollover.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 2000))]
    pub auto_compact_fallback_prompt: Option<String>,
    /// Additional tokens available after the compaction threshold for fallback note-taking.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub auto_compact_fallback_buffer_tokens: Option<i64>,
}

impl FeatureConfig for TokenBudgetConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = Some(enabled);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RolloutBudgetConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub limit_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Remaining weighted-token values that trigger reminders when crossed.
    pub reminder_at_remaining_tokens: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub sampling_token_weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub prefill_token_weight: Option<f64>,
}

impl FeatureConfig for RolloutBudgetConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = Some(enabled);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CurrentTimeSource {
    #[default]
    System,
    External,
}

/// Which inference boundaries may receive current-time reminders.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CurrentTimeReminderDeliveryMode {
    /// Allow a reminder before any inference request once the interval is due.
    #[default]
    AnyInference,
    /// Allow reminders after user input or tool output; new context windows still force one.
    AfterUserOrToolOutput,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CurrentTimeReminderConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reminder_interval_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock_source: Option<CurrentTimeSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_mode: Option<CurrentTimeReminderDeliveryMode>,
    /// Expose the input-interruptible `clock.sleep` tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sleep_tool: Option<bool>,
}

impl FeatureConfig for CurrentTimeReminderConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = Some(enabled);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemovedAppsMcpPathOverrideConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
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
    Deny,
}
