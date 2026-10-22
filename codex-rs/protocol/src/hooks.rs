//! Shared hook handler configuration for local and owner-managed environment sources.

use crate::protocol::HookEventName;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HookEventsToml {
    #[serde(rename = "PreModelRequest", default)]
    pub pre_model_request: Vec<MatcherGroup>,
    #[serde(rename = "PostModelRequest", default)]
    pub post_model_request: Vec<MatcherGroup>,
    #[serde(rename = "PreToolUse", default)]
    pub pre_tool_use: Vec<MatcherGroup>,
    #[serde(rename = "PermissionRequest", default)]
    pub permission_request: Vec<MatcherGroup>,
    #[serde(rename = "PostToolUse", default)]
    pub post_tool_use: Vec<MatcherGroup>,
    #[serde(rename = "PreCompact", default)]
    pub pre_compact: Vec<MatcherGroup>,
    #[serde(rename = "PostCompact", default)]
    pub post_compact: Vec<MatcherGroup>,
    #[serde(rename = "SessionStart", default)]
    pub session_start: Vec<MatcherGroup>,
    #[serde(rename = "SessionEnd", default)]
    pub session_end: Vec<MatcherGroup>,
    #[serde(rename = "UserPromptSubmit", default)]
    pub user_prompt_submit: Vec<MatcherGroup>,
    #[serde(rename = "SubagentStart", default)]
    pub subagent_start: Vec<MatcherGroup>,
    #[serde(rename = "SubagentStop", default)]
    pub subagent_stop: Vec<MatcherGroup>,
    #[serde(rename = "Stop", default)]
    pub stop: Vec<MatcherGroup>,
    #[serde(rename = "Interrupt", default)]
    pub interrupt: Vec<MatcherGroup>,
}

impl HookEventsToml {
    pub fn is_empty(&self) -> bool {
        let Self {
            pre_model_request,
            post_model_request,
            pre_tool_use,
            permission_request,
            post_tool_use,
            pre_compact,
            post_compact,
            session_start,
            session_end,
            user_prompt_submit,
            subagent_start,
            subagent_stop,
            stop,
            interrupt,
        } = self;
        pre_model_request.is_empty()
            && post_model_request.is_empty()
            && pre_tool_use.is_empty()
            && permission_request.is_empty()
            && post_tool_use.is_empty()
            && pre_compact.is_empty()
            && post_compact.is_empty()
            && session_start.is_empty()
            && session_end.is_empty()
            && user_prompt_submit.is_empty()
            && subagent_start.is_empty()
            && subagent_stop.is_empty()
            && stop.is_empty()
            && interrupt.is_empty()
    }

    pub fn handler_count(&self) -> usize {
        let Self {
            pre_model_request,
            post_model_request,
            pre_tool_use,
            permission_request,
            post_tool_use,
            pre_compact,
            post_compact,
            session_start,
            session_end,
            user_prompt_submit,
            subagent_start,
            subagent_stop,
            stop,
            interrupt,
        } = self;
        [
            pre_model_request,
            post_model_request,
            pre_tool_use,
            permission_request,
            post_tool_use,
            pre_compact,
            post_compact,
            session_start,
            session_end,
            user_prompt_submit,
            subagent_start,
            subagent_stop,
            stop,
            interrupt,
        ]
        .into_iter()
        .flatten()
        .map(|group| group.hooks.len())
        .sum()
    }

    pub fn into_matcher_groups(mut self) -> [(HookEventName, Vec<MatcherGroup>); 14] {
        self.matcher_groups_mut()
            .map(|(event, groups)| (event, std::mem::take(groups)))
    }

    pub fn matcher_groups_mut(&mut self) -> [(HookEventName, &mut Vec<MatcherGroup>); 14] {
        use HookEventName as Event;

        [
            (Event::PreModelRequest, &mut self.pre_model_request),
            (Event::PostModelRequest, &mut self.post_model_request),
            (Event::PreToolUse, &mut self.pre_tool_use),
            (Event::PermissionRequest, &mut self.permission_request),
            (Event::PostToolUse, &mut self.post_tool_use),
            (Event::PreCompact, &mut self.pre_compact),
            (Event::PostCompact, &mut self.post_compact),
            (Event::SessionStart, &mut self.session_start),
            (Event::SessionEnd, &mut self.session_end),
            (Event::UserPromptSubmit, &mut self.user_prompt_submit),
            (Event::SubagentStart, &mut self.subagent_start),
            (Event::SubagentStop, &mut self.subagent_stop),
            (Event::Stop, &mut self.stop),
            (Event::Interrupt, &mut self.interrupt),
        ]
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MatcherGroup {
    #[serde(default)]
    pub matcher: Option<String>,
    #[serde(default)]
    pub hooks: Vec<HookHandlerConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum HookHandlerConfig {
    #[serde(rename = "command")]
    Command {
        command: String,
        #[serde(default, rename = "commandWindows", alias = "command_windows")]
        command_windows: Option<String>,
        #[serde(default, rename = "timeout")]
        timeout_sec: Option<u64>,
        #[serde(default)]
        r#async: bool,
        #[serde(default, rename = "statusMessage")]
        status_message: Option<String>,
        /// Approximate token threshold for spilling this hook's `additionalContext` to disk.
        /// Unset uses 2,500 tokens; `0` disables spilling for this hook. The threshold is
        /// evaluated against the original context; a spilled preview also includes recovery
        /// metadata.
        #[serde(
            default,
            rename = "additionalContextLimit",
            skip_serializing_if = "Option::is_none"
        )]
        additional_context_limit: Option<usize>,
    },
    #[serde(rename = "mcp_tool")]
    McpTool {
        server: String,
        tool: String,
        #[serde(default, deserialize_with = "deserialize_mcp_tool_input")]
        input: serde_json::Map<String, serde_json::Value>,
        #[serde(default, rename = "timeout")]
        timeout_sec: Option<u64>,
        #[serde(default, rename = "statusMessage")]
        status_message: Option<String>,
    },
    #[serde(rename = "prompt")]
    Prompt {},
    #[serde(rename = "agent")]
    Agent {},
}

// Reject values such as null that cannot be represented in TOML for trust hashing.
fn deserialize_mcp_tool_input<'de, D>(
    deserializer: D,
) -> Result<serde_json::Map<String, serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let input = serde_json::Map::deserialize(deserializer)?;
    fn representable(value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::Null => false,
            serde_json::Value::Number(n) => !n.is_u64() || n.as_i64().is_some(),
            serde_json::Value::Array(items) => items.iter().all(representable),
            serde_json::Value::Object(items) => items.values().all(representable),
            serde_json::Value::Bool(_) | serde_json::Value::String(_) => true,
        }
    }
    if !input.values().all(representable) {
        return Err(serde::de::Error::custom(
            "MCP hook input must be representable as TOML",
        ));
    }
    Ok(input)
}
