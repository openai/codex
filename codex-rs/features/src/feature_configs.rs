use crate::FeatureConfig;
use crate::FeatureToml;
use codex_protocol::openai_models::ReasoningEffort;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolRegistryConfigToml {
    /// Fail the turn when multiple tools share the same effective name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_on_tool_collisions: Option<bool>,
    /// Include authoritative tool information in per-turn request metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_metadata_includes_tool_info: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodeModeConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Default yield timeout for code-mode exec calls, in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_exec_yield_time_ms: Option<u64>,
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
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodeModeHostConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Keep code mode fail-closed when the standalone host is unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_in_process_fallback: Option<bool>,
}

impl FeatureConfig for CodeModeHostConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NonPrefixedMcpToolNamesConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// MCP servers whose tools should omit the legacy `mcp__` namespace prefix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_names: Option<Vec<String>>,
}

impl FeatureConfig for NonPrefixedMcpToolNamesConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }
}

/// Optional conversation sources available to the Guardian v2 classifier.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GuardianV2TranscriptSource {
    ToolCalls,
    ToolOutputs,
    Reasoning,
}

/// Bounds and optional sources for the Guardian v2 conversation transcript.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuardianV2TranscriptConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<GuardianV2TranscriptSource>>,
    /// Include recent screenshots from messages and configured tool outputs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_images: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 100, max = 100000))]
    pub max_message_entry_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 100, max = 100000))]
    pub max_tool_entry_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 100, max = 100000))]
    pub max_message_transcript_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 100, max = 100000))]
    pub max_tool_transcript_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1))]
    pub max_recent_non_user_entries: Option<usize>,
}

/// Optional tool-call categories available to the Guardian v2 classifier.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuardianV2ReviewScopeConfigToml {
    /// Restrict asynchronous classification and fast approvals to browser and computer-use tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computer_use_only: Option<bool>,
    /// Include sandboxed shell command calls in Guardian v2 classification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandboxed_exec_commands: Option<bool>,
}

/// User-configurable prompt, approval, and context settings for Guardian v2.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuardianV2ConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classifier_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub review_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_call_lag: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 100, max = 100000))]
    pub max_action_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 100, max = 100000))]
    pub max_classifier_instruction_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reuse_parent_compaction: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 100, max = 100000))]
    pub max_parent_compaction_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_scope: Option<GuardianV2ReviewScopeConfigToml>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript: Option<GuardianV2TranscriptConfigToml>,
}

impl FeatureConfig for GuardianV2ConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
    }
}

pub(crate) fn deserialize_guardian_v2_feature<'de, D>(
    deserializer: D,
) -> Result<Option<FeatureToml<GuardianV2ConfigToml>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    const MIN_TOKENS: usize = 100;
    const MAX_TOKENS: usize = 100_000;

    let feature = Option::<FeatureToml<GuardianV2ConfigToml>>::deserialize(deserializer)?;
    let Some(FeatureToml::Config(config)) = feature.as_ref() else {
        return Ok(feature);
    };

    if let Some(threshold) = config.review_threshold
        && (!threshold.is_finite() || !(0.0..=1.0).contains(&threshold))
    {
        return Err(serde::de::Error::custom(
            "Guardian v2 review_threshold must be between 0.0 and 1.0",
        ));
    }

    let transcript = config.transcript.as_ref();
    let message_entry = transcript.and_then(|value| value.max_message_entry_tokens);
    let tool_entry = transcript.and_then(|value| value.max_tool_entry_tokens);
    let message_total = transcript.and_then(|value| value.max_message_transcript_tokens);
    let tool_total = transcript.and_then(|value| value.max_tool_transcript_tokens);
    for (field, tokens) in [
        ("max_action_tokens", config.max_action_tokens),
        (
            "max_classifier_instruction_tokens",
            config.max_classifier_instruction_tokens,
        ),
        (
            "max_parent_compaction_tokens",
            config.max_parent_compaction_tokens,
        ),
        ("transcript max_message_entry_tokens", message_entry),
        ("transcript max_tool_entry_tokens", tool_entry),
        ("transcript max_message_transcript_tokens", message_total),
        ("transcript max_tool_transcript_tokens", tool_total),
    ] {
        if let Some(tokens) = tokens
            && !(MIN_TOKENS..=MAX_TOKENS).contains(&tokens)
        {
            return Err(serde::de::Error::custom(format!(
                "Guardian v2 {field} must be between {MIN_TOKENS} and {MAX_TOKENS} tokens"
            )));
        }
    }

    if transcript
        .and_then(|value| value.max_recent_non_user_entries)
        .is_some_and(|count| count == 0)
    {
        return Err(serde::de::Error::custom(
            "Guardian v2 transcript max_recent_non_user_entries must be positive",
        ));
    }

    for (kind, entry, total) in [
        ("message", message_entry, message_total),
        ("tool", tool_entry, tool_total),
    ] {
        if let (Some(entry), Some(total)) = (entry, total)
            && entry > total
        {
            return Err(serde::de::Error::custom(format!(
                "Guardian v2 transcript max_{kind}_entry_tokens must not exceed max_{kind}_transcript_tokens"
            )));
        }
    }

    Ok(feature)
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
    /// Overrides inherited developer instructions for subagents without role-specific instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_developer_instructions: Option<String>,
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
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TokenBudgetConfigToml {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Whether to expose the built-in history and notes extension.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_history_notes_extension: Option<bool>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_broker: Option<bool>,
}

impl FeatureConfig for NetworkProxyConfigToml {
    fn enabled(&self) -> Option<bool> {
        self.enabled
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
