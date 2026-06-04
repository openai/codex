use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::r#gen::SchemaSettings;
use schemars::schema::InstanceType;
use schemars::schema::RootSchema;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use std::path::Path;
use std::path::PathBuf;

use crate::events::common::SubagentHookContext;

const GENERATED_DIR: &str = "generated";
const POST_TOOL_USE_INPUT_FIXTURE: &str = "post-tool-use.command.input.schema.json";
const POST_TOOL_USE_OUTPUT_FIXTURE: &str = "post-tool-use.command.output.schema.json";
const PERMISSION_REQUEST_INPUT_FIXTURE: &str = "permission-request.command.input.schema.json";
const PERMISSION_REQUEST_OUTPUT_FIXTURE: &str = "permission-request.command.output.schema.json";
const POST_COMPACT_INPUT_FIXTURE: &str = "post-compact.command.input.schema.json";
const POST_COMPACT_OUTPUT_FIXTURE: &str = "post-compact.command.output.schema.json";
const PRE_TOOL_USE_INPUT_FIXTURE: &str = "pre-tool-use.command.input.schema.json";
const PRE_TOOL_USE_OUTPUT_FIXTURE: &str = "pre-tool-use.command.output.schema.json";
const PRE_COMPACT_INPUT_FIXTURE: &str = "pre-compact.command.input.schema.json";
const PRE_COMPACT_OUTPUT_FIXTURE: &str = "pre-compact.command.output.schema.json";
const SESSION_START_INPUT_FIXTURE: &str = "session-start.command.input.schema.json";
const SESSION_START_OUTPUT_FIXTURE: &str = "session-start.command.output.schema.json";
const USER_PROMPT_SUBMIT_INPUT_FIXTURE: &str = "user-prompt-submit.command.input.schema.json";
const USER_PROMPT_SUBMIT_OUTPUT_FIXTURE: &str = "user-prompt-submit.command.output.schema.json";
const SUBAGENT_START_INPUT_FIXTURE: &str = "subagent-start.command.input.schema.json";
const SUBAGENT_START_OUTPUT_FIXTURE: &str = "subagent-start.command.output.schema.json";
const SUBAGENT_STOP_INPUT_FIXTURE: &str = "subagent-stop.command.input.schema.json";
const SUBAGENT_STOP_OUTPUT_FIXTURE: &str = "subagent-stop.command.output.schema.json";
const STOP_INPUT_FIXTURE: &str = "stop.command.input.schema.json";
const STOP_OUTPUT_FIXTURE: &str = "stop.command.output.schema.json";

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub(crate) struct NullableString(Option<String>);

impl NullableString {
    pub(crate) fn from_path(path: Option<PathBuf>) -> Self {
        Self(path.map(|path| path.display().to_string()))
    }

    pub(crate) fn from_string(value: Option<String>) -> Self {
        Self(value)
    }
}

impl JsonSchema for NullableString {
    fn schema_name() -> String {
        "NullableString".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            instance_type: Some(vec![InstanceType::String, InstanceType::Null].into()),
            ..Default::default()
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SubagentCommandInputFields {
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,
}

impl From<Option<&SubagentHookContext>> for SubagentCommandInputFields {
    fn from(value: Option<&SubagentHookContext>) -> Self {
        match value {
            Some(context) => Self {
                agent_id: Some(context.agent_id.clone()),
                agent_type: Some(context.agent_type.clone()),
            },
            None => Self::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct HookUniversalOutputWire {
    #[serde(default = "default_continue")]
    pub r#continue: bool,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub suppress_output: bool,
    #[serde(default)]
    pub system_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub(crate) enum HookEventNameWire {
    #[serde(rename = "PreToolUse")]
    PreToolUse,
    #[serde(rename = "PermissionRequest")]
    PermissionRequest,
    #[serde(rename = "PostToolUse")]
    PostToolUse,
    #[serde(rename = "PreCompact")]
    PreCompact,
    #[serde(rename = "PostCompact")]
    PostCompact,
    #[serde(rename = "SessionStart")]
    SessionStart,
    #[serde(rename = "UserPromptSubmit")]
    UserPromptSubmit,
    #[serde(rename = "SubagentStart")]
    SubagentStart,
    #[serde(rename = "SubagentStop")]
    SubagentStop,
    #[serde(rename = "Stop")]
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "pre-tool-use.command.output")]
pub(crate) struct PreToolUseCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub decision: Option<PreToolUseDecisionWire>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub hook_specific_output: Option<PreToolUseHookSpecificOutputWire>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "post-tool-use.command.output")]
pub(crate) struct PostToolUseCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub decision: Option<BlockDecisionWire>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub hook_specific_output: Option<PostToolUseHookSpecificOutputWire>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "permission-request.command.output")]
pub(crate) struct PermissionRequestCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub hook_specific_output: Option<PermissionRequestHookSpecificOutputWire>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "pre-compact.command.output")]
pub(crate) struct PreCompactCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "post-compact.command.output")]
pub(crate) struct PostCompactCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct PermissionRequestHookSpecificOutputWire {
    #[schemars(schema_with = "permission_request_hook_event_name_schema")]
    pub hook_event_name: HookEventNameWire,
    #[serde(default)]
    pub decision: Option<PermissionRequestDecisionWire>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct PermissionRequestDecisionWire {
    pub behavior: PermissionRequestBehaviorWire,
    /// Reserved for a future input-rewrite capability.
    ///
    /// PermissionRequest hooks currently fail closed if this field is present.
    #[serde(default)]
    pub updated_input: Option<Value>,
    /// Reserved for a future permission-rewrite capability.
    ///
    /// PermissionRequest hooks currently fail closed if this field is present.
    #[serde(default)]
    pub updated_permissions: Option<Value>,
    #[serde(default)]
    pub message: Option<String>,
    /// Reserved for future short-circuiting semantics.
    ///
    /// PermissionRequest hooks currently fail closed if this field is `true`.
    #[serde(default)]
    pub interrupt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub(crate) enum PermissionRequestBehaviorWire {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct PostToolUseHookSpecificOutputWire {
    #[schemars(schema_with = "post_tool_use_hook_event_name_schema")]
    pub hook_event_name: HookEventNameWire,
    #[serde(default)]
    pub additional_context: Option<String>,
    #[serde(default)]
    #[serde(rename = "updatedMCPToolOutput")]
    pub updated_mcp_tool_output: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct PreToolUseHookSpecificOutputWire {
    #[schemars(schema_with = "pre_tool_use_hook_event_name_schema")]
    pub hook_event_name: HookEventNameWire,
    #[serde(default)]
    pub permission_decision: Option<PreToolUsePermissionDecisionWire>,
    #[serde(default)]
    pub permission_decision_reason: Option<String>,
    #[serde(default)]
    pub updated_input: Option<Value>,
    #[serde(default)]
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub(crate) enum PreToolUsePermissionDecisionWire {
    #[serde(rename = "allow")]
    Allow,
    #[serde(rename = "deny")]
    Deny,
    #[serde(rename = "ask")]
    Ask,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub(crate) enum PreToolUseDecisionWire {
    #[serde(rename = "approve")]
    Approve,
    #[serde(rename = "block")]
    Block,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "pre-tool-use.command.input")]
pub(crate) struct PreToolUseCommandInput {
    pub session_id: String,
    /// Codex extension: expose the active turn id to internal turn-scoped hooks.
    pub turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "pre_tool_use_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub tool_use_id: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "permission-request.command.input")]
pub(crate) struct PermissionRequestCommandInput {
    pub session_id: String,
    /// Codex extension: expose the active turn id to internal turn-scoped hooks.
    pub turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "permission_request_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    pub tool_name: String,
    pub tool_input: Value,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "post-tool-use.command.input")]
pub(crate) struct PostToolUseCommandInput {
    pub session_id: String,
    /// Codex extension: expose the active turn id to internal turn-scoped hooks.
    pub turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "post_tool_use_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    pub tool_name: String,
    pub tool_input: Value,
    pub tool_response: Value,
    pub tool_use_id: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "pre-compact.command.input")]
pub(crate) struct PreCompactCommandInput {
    pub session_id: String,
    /// Codex extension: expose the active turn id to internal turn-scoped hooks.
    pub turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "pre_compact_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "compaction_trigger_schema")]
    pub trigger: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "post-compact.command.input")]
pub(crate) struct PostCompactCommandInput {
    pub session_id: String,
    /// Codex extension: expose the active turn id to internal turn-scoped hooks.
    pub turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "post_compact_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "compaction_trigger_schema")]
    pub trigger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "session-start.command.output")]
pub(crate) struct SessionStartCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub hook_specific_output: Option<SessionStartHookSpecificOutputWire>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct SessionStartHookSpecificOutputWire {
    #[schemars(schema_with = "session_start_hook_event_name_schema")]
    pub hook_event_name: HookEventNameWire,
    #[serde(default)]
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "subagent-start.command.output")]
pub(crate) struct SubagentStartCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub hook_specific_output: Option<SubagentStartHookSpecificOutputWire>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct SubagentStartHookSpecificOutputWire {
    #[schemars(schema_with = "subagent_start_hook_event_name_schema")]
    pub hook_event_name: HookEventNameWire,
    #[serde(default)]
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "user-prompt-submit.command.output")]
pub(crate) struct UserPromptSubmitCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub decision: Option<BlockDecisionWire>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub hook_specific_output: Option<UserPromptSubmitHookSpecificOutputWire>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub(crate) struct UserPromptSubmitHookSpecificOutputWire {
    #[schemars(schema_with = "user_prompt_submit_hook_event_name_schema")]
    pub hook_event_name: HookEventNameWire,
    #[serde(default)]
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "stop.command.output")]
pub(crate) struct StopCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub decision: Option<BlockDecisionWire>,
    /// Claude requires `reason` when `decision` is `block`; we enforce that
    /// semantic rule during output parsing rather than in the JSON schema.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[schemars(rename = "subagent-stop.command.output")]
pub(crate) struct SubagentStopCommandOutputWire {
    #[serde(flatten)]
    pub universal: HookUniversalOutputWire,
    #[serde(default)]
    pub decision: Option<BlockDecisionWire>,
    /// Claude requires `reason` when `decision` is `block`; we enforce that
    /// semantic rule during output parsing rather than in the JSON schema.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub(crate) enum BlockDecisionWire {
    #[serde(rename = "block")]
    Block,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "session-start.command.input")]
pub(crate) struct SessionStartCommandInput {
    pub session_id: String,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "session_start_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    #[schemars(schema_with = "session_start_source_schema")]
    pub source: String,
}

impl SessionStartCommandInput {
    pub(crate) fn new(
        session_id: impl Into<String>,
        transcript_path: Option<PathBuf>,
        cwd: impl Into<String>,
        model: impl Into<String>,
        permission_mode: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            transcript_path: NullableString::from_path(transcript_path),
            cwd: cwd.into(),
            hook_event_name: "SessionStart".to_string(),
            model: model.into(),
            permission_mode: permission_mode.into(),
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "subagent-start.command.input")]
pub(crate) struct SubagentStartCommandInput {
    pub session_id: String,
    /// Codex extension: expose the active turn id to internal turn-scoped hooks.
    pub turn_id: String,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "subagent_start_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    pub agent_id: String,
    pub agent_type: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "user-prompt-submit.command.input")]
pub(crate) struct UserPromptSubmitCommandInput {
    pub session_id: String,
    /// Codex extension: expose the active turn id to internal turn-scoped hooks.
    pub turn_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "user_prompt_submit_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "stop.command.input")]
pub(crate) struct StopCommandInput {
    pub session_id: String,
    /// Codex extension: expose the active turn id to internal turn-scoped hooks.
    pub turn_id: String,
    pub transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "stop_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    pub stop_hook_active: bool,
    pub last_assistant_message: NullableString,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "subagent-stop.command.input")]
pub(crate) struct SubagentStopCommandInput {
    pub session_id: String,
    /// Codex extension: expose the active turn id to internal turn-scoped hooks.
    pub turn_id: String,
    pub transcript_path: NullableString,
    pub agent_transcript_path: NullableString,
    pub cwd: String,
    #[schemars(schema_with = "subagent_stop_hook_event_name_schema")]
    pub hook_event_name: String,
    pub model: String,
    #[schemars(schema_with = "permission_mode_schema")]
    pub permission_mode: String,
    pub stop_hook_active: bool,
    pub agent_id: String,
    pub agent_type: String,
    pub last_assistant_message: NullableString,
}

pub fn write_schema_fixtures(schema_root: &Path) -> anyhow::Result<()> {
    let generated_dir = schema_root.join(GENERATED_DIR);
    ensure_empty_dir(&generated_dir)?;

    write_schema(
        &generated_dir.join(POST_TOOL_USE_INPUT_FIXTURE),
        input_schema_json::<PostToolUseCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(POST_TOOL_USE_OUTPUT_FIXTURE),
        output_schema_json::<PostToolUseCommandOutputWire>(OutputSchemaKind::PostToolUse)?,
    )?;
    write_schema(
        &generated_dir.join(PERMISSION_REQUEST_INPUT_FIXTURE),
        input_schema_json::<PermissionRequestCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(PERMISSION_REQUEST_OUTPUT_FIXTURE),
        output_schema_json::<PermissionRequestCommandOutputWire>(
            OutputSchemaKind::PermissionRequest,
        )?,
    )?;
    write_schema(
        &generated_dir.join(POST_COMPACT_INPUT_FIXTURE),
        input_schema_json::<PostCompactCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(POST_COMPACT_OUTPUT_FIXTURE),
        output_schema_json::<PostCompactCommandOutputWire>(OutputSchemaKind::PostCompact)?,
    )?;
    write_schema(
        &generated_dir.join(PRE_COMPACT_INPUT_FIXTURE),
        input_schema_json::<PreCompactCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(PRE_COMPACT_OUTPUT_FIXTURE),
        output_schema_json::<PreCompactCommandOutputWire>(OutputSchemaKind::PreCompact)?,
    )?;
    write_schema(
        &generated_dir.join(PRE_TOOL_USE_INPUT_FIXTURE),
        input_schema_json::<PreToolUseCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(PRE_TOOL_USE_OUTPUT_FIXTURE),
        output_schema_json::<PreToolUseCommandOutputWire>(OutputSchemaKind::PreToolUse)?,
    )?;
    write_schema(
        &generated_dir.join(SESSION_START_INPUT_FIXTURE),
        input_schema_json::<SessionStartCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(SESSION_START_OUTPUT_FIXTURE),
        output_schema_json::<SessionStartCommandOutputWire>(OutputSchemaKind::SessionStart)?,
    )?;
    write_schema(
        &generated_dir.join(USER_PROMPT_SUBMIT_INPUT_FIXTURE),
        input_schema_json::<UserPromptSubmitCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(USER_PROMPT_SUBMIT_OUTPUT_FIXTURE),
        output_schema_json::<UserPromptSubmitCommandOutputWire>(
            OutputSchemaKind::UserPromptSubmit,
        )?,
    )?;
    write_schema(
        &generated_dir.join(SUBAGENT_START_INPUT_FIXTURE),
        input_schema_json::<SubagentStartCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(SUBAGENT_START_OUTPUT_FIXTURE),
        output_schema_json::<SubagentStartCommandOutputWire>(OutputSchemaKind::SubagentStart)?,
    )?;
    write_schema(
        &generated_dir.join(SUBAGENT_STOP_INPUT_FIXTURE),
        input_schema_json::<SubagentStopCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(SUBAGENT_STOP_OUTPUT_FIXTURE),
        output_schema_json::<SubagentStopCommandOutputWire>(OutputSchemaKind::SubagentStop)?,
    )?;
    write_schema(
        &generated_dir.join(STOP_INPUT_FIXTURE),
        input_schema_json::<StopCommandInput>()?,
    )?;
    write_schema(
        &generated_dir.join(STOP_OUTPUT_FIXTURE),
        output_schema_json::<StopCommandOutputWire>(OutputSchemaKind::Stop)?,
    )?;

    Ok(())
}

fn write_schema(path: &Path, json: Vec<u8>) -> anyhow::Result<()> {
    std::fs::write(path, json)?;
    Ok(())
}

fn ensure_empty_dir(dir: &Path) -> anyhow::Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(dir)?;
    Ok(())
}

// Parser wire types stay broad enough to recognize legacy and reserved output,
// while generated schemas advertise only output that the runtime supports.
#[derive(Clone, Copy)]
enum OutputSchemaKind {
    PostToolUse,
    PermissionRequest,
    PostCompact,
    PreCompact,
    PreToolUse,
    SessionStart,
    UserPromptSubmit,
    SubagentStart,
    SubagentStop,
    Stop,
}

fn input_schema_json<T>() -> anyhow::Result<Vec<u8>>
where
    T: JsonSchema,
{
    serialize_schema(schema_value::<T>(/*option_add_null_type*/ false)?)
}

fn output_schema_json<T>(kind: OutputSchemaKind) -> anyhow::Result<Vec<u8>>
where
    T: JsonSchema,
{
    serialize_schema(output_schema_value::<T>(kind)?)
}

fn output_schema_value<T>(kind: OutputSchemaKind) -> anyhow::Result<Value>
where
    T: JsonSchema,
{
    let mut value = schema_value::<T>(/*option_add_null_type*/ true)?;
    constrain_output_schema(&mut value, kind)?;
    Ok(value)
}

fn schema_value<T>(option_add_null_type: bool) -> anyhow::Result<Value>
where
    T: JsonSchema,
{
    let schema = schema_for_type::<T>(option_add_null_type);
    let value = serde_json::to_value(schema)?;
    Ok(value)
}

fn serialize_schema(value: Value) -> anyhow::Result<Vec<u8>> {
    let value = canonicalize_json(&value);
    Ok(serde_json::to_vec_pretty(&value)?)
}

fn schema_for_type<T>(option_add_null_type: bool) -> RootSchema
where
    T: JsonSchema,
{
    SchemaSettings::draft07()
        .with(|settings| {
            settings.option_add_null_type = option_add_null_type;
        })
        .into_generator()
        .into_root_schema_for::<T>()
}

fn constrain_output_schema(schema: &mut Value, kind: OutputSchemaKind) -> anyhow::Result<()> {
    match kind {
        OutputSchemaKind::PreToolUse => {
            constrain_bool_property(schema, "/properties/continue", /*expected*/ true)?;
            constrain_null_property(schema, "/properties/stopReason")?;
            constrain_bool_property(
                schema,
                "/properties/suppressOutput",
                /*expected*/ false,
            )?;
            constrain_enum(
                schema,
                "/definitions/PreToolUseDecisionWire/enum",
                &["block"],
            )?;
            constrain_enum(
                schema,
                "/definitions/PreToolUsePermissionDecisionWire/enum",
                &["allow", "deny"],
            )?;
            require_block_reason(schema)?;
            constrain_pre_tool_use_hook_specific_output(schema)?;
        }
        OutputSchemaKind::PermissionRequest => {
            constrain_bool_property(schema, "/properties/continue", /*expected*/ true)?;
            constrain_null_property(schema, "/properties/stopReason")?;
            constrain_bool_property(
                schema,
                "/properties/suppressOutput",
                /*expected*/ false,
            )?;
            constrain_null_property(
                schema,
                "/definitions/PermissionRequestDecisionWire/properties/updatedInput",
            )?;
            constrain_null_property(
                schema,
                "/definitions/PermissionRequestDecisionWire/properties/updatedPermissions",
            )?;
            constrain_bool_property(
                schema,
                "/definitions/PermissionRequestDecisionWire/properties/interrupt",
                /*expected*/ false,
            )?;
        }
        OutputSchemaKind::PostToolUse => {
            constrain_bool_property(
                schema,
                "/properties/suppressOutput",
                /*expected*/ false,
            )?;
            constrain_null_property(
                schema,
                "/definitions/PostToolUseHookSpecificOutputWire/properties/updatedMCPToolOutput",
            )?;
            require_block_reason(schema)?;
        }
        OutputSchemaKind::UserPromptSubmit
        | OutputSchemaKind::SubagentStop
        | OutputSchemaKind::Stop => require_block_reason(schema)?,
        OutputSchemaKind::PostCompact
        | OutputSchemaKind::PreCompact
        | OutputSchemaKind::SessionStart
        | OutputSchemaKind::SubagentStart => {}
    }
    Ok(())
}

fn constrain_pre_tool_use_hook_specific_output(schema: &mut Value) -> anyhow::Result<()> {
    let definition = schema_node_mut(schema, "/definitions/PreToolUseHookSpecificOutputWire")?;
    definition["allOf"] = serde_json::json!([
        {
            "if": {
                "properties": {
                    "permissionDecision": {"const": "allow"}
                },
                "required": ["permissionDecision"]
            },
            "then": {
                "properties": {
                    "updatedInput": {"not": {"type": "null"}}
                },
                "required": ["updatedInput"]
            }
        },
        {
            "if": {
                "properties": {
                    "permissionDecision": {"const": "deny"}
                },
                "required": ["permissionDecision"]
            },
            "then": {
                "properties": {
                    "permissionDecisionReason": {
                        "pattern": ".*\\S.*",
                        "type": "string"
                    }
                },
                "required": ["permissionDecisionReason"]
            }
        },
        {
            "if": {
                "properties": {
                    "updatedInput": {"not": {"type": "null"}}
                },
                "required": ["updatedInput"]
            },
            "then": {
                "properties": {
                    "permissionDecision": {"const": "allow"}
                },
                "required": ["permissionDecision"]
            }
        },
        {
            "if": {
                "properties": {
                    "permissionDecisionReason": {"not": {"type": "null"}}
                },
                "required": ["permissionDecisionReason"]
            },
            "then": {
                "required": ["permissionDecision"]
            }
        }
    ]);
    Ok(())
}

fn require_block_reason(schema: &mut Value) -> anyhow::Result<()> {
    schema["allOf"] = serde_json::json!([
        {
            "if": {
                "properties": {
                    "decision": {"const": "block"}
                },
                "required": ["decision"]
            },
            "then": {
                "properties": {
                    "reason": {
                        "pattern": ".*\\S.*",
                        "type": "string"
                    }
                },
                "required": ["reason"]
            }
        }
    ]);
    Ok(())
}

fn constrain_bool_property(
    schema: &mut Value,
    pointer: &str,
    expected: bool,
) -> anyhow::Result<()> {
    schema_node_mut(schema, pointer)?["const"] = Value::Bool(expected);
    Ok(())
}

fn constrain_null_property(schema: &mut Value, pointer: &str) -> anyhow::Result<()> {
    *schema_node_mut(schema, pointer)? = serde_json::json!({
        "default": null,
        "type": "null"
    });
    Ok(())
}

fn constrain_enum(schema: &mut Value, pointer: &str, supported: &[&str]) -> anyhow::Result<()> {
    *schema_node_mut(schema, pointer)? = serde_json::json!(supported);
    Ok(())
}

fn schema_node_mut<'a>(schema: &'a mut Value, pointer: &str) -> anyhow::Result<&'a mut Value> {
    schema
        .pointer_mut(pointer)
        .ok_or_else(|| anyhow::anyhow!("generated hook schema is missing {pointer}"))
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json).collect()),
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            let mut sorted = Map::with_capacity(map.len());
            for (key, child) in entries {
                sorted.insert(key.clone(), canonicalize_json(child));
            }
            Value::Object(sorted)
        }
        _ => value.clone(),
    }
}

fn session_start_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("SessionStart")
}

fn post_tool_use_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("PostToolUse")
}

fn pre_compact_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("PreCompact")
}

fn post_compact_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("PostCompact")
}

fn pre_tool_use_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("PreToolUse")
}

fn permission_request_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("PermissionRequest")
}

fn user_prompt_submit_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("UserPromptSubmit")
}

fn subagent_start_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("SubagentStart")
}

fn subagent_stop_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("SubagentStop")
}

fn stop_hook_event_name_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_const_schema("Stop")
}

fn permission_mode_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_enum_schema(&[
        "default",
        "acceptEdits",
        "plan",
        "dontAsk",
        "bypassPermissions",
    ])
}

fn session_start_source_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_enum_schema(&["startup", "resume", "clear", "compact"])
}

fn compaction_trigger_schema(_gen: &mut SchemaGenerator) -> Schema {
    string_enum_schema(&["manual", "auto"])
}

fn string_const_schema(value: &str) -> Schema {
    let mut schema = SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        ..Default::default()
    };
    schema.const_value = Some(Value::String(value.to_string()));
    Schema::Object(schema)
}

fn string_enum_schema(values: &[&str]) -> Schema {
    let mut schema = SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        ..Default::default()
    };
    schema.enum_values = Some(
        values
            .iter()
            .map(|value| Value::String((*value).to_string()))
            .collect(),
    );
    Schema::Object(schema)
}

fn default_continue() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::NullableString;
    use super::OutputSchemaKind;
    use super::PERMISSION_REQUEST_INPUT_FIXTURE;
    use super::PERMISSION_REQUEST_OUTPUT_FIXTURE;
    use super::POST_COMPACT_INPUT_FIXTURE;
    use super::POST_COMPACT_OUTPUT_FIXTURE;
    use super::POST_TOOL_USE_INPUT_FIXTURE;
    use super::POST_TOOL_USE_OUTPUT_FIXTURE;
    use super::PRE_COMPACT_INPUT_FIXTURE;
    use super::PRE_COMPACT_OUTPUT_FIXTURE;
    use super::PRE_TOOL_USE_INPUT_FIXTURE;
    use super::PRE_TOOL_USE_OUTPUT_FIXTURE;
    use super::PermissionRequestCommandInput;
    use super::PermissionRequestCommandOutputWire;
    use super::PostCompactCommandInput;
    use super::PostToolUseCommandInput;
    use super::PostToolUseCommandOutputWire;
    use super::PreCompactCommandInput;
    use super::PreToolUseCommandInput;
    use super::PreToolUseCommandOutputWire;
    use super::SESSION_START_INPUT_FIXTURE;
    use super::SESSION_START_OUTPUT_FIXTURE;
    use super::STOP_INPUT_FIXTURE;
    use super::STOP_OUTPUT_FIXTURE;
    use super::SUBAGENT_START_INPUT_FIXTURE;
    use super::SUBAGENT_START_OUTPUT_FIXTURE;
    use super::SUBAGENT_STOP_INPUT_FIXTURE;
    use super::SUBAGENT_STOP_OUTPUT_FIXTURE;
    use super::SessionStartCommandOutputWire;
    use super::StopCommandInput;
    use super::SubagentCommandInputFields;
    use super::SubagentStartCommandInput;
    use super::SubagentStartCommandOutputWire;
    use super::SubagentStopCommandInput;
    use super::USER_PROMPT_SUBMIT_INPUT_FIXTURE;
    use super::USER_PROMPT_SUBMIT_OUTPUT_FIXTURE;
    use super::UserPromptSubmitCommandInput;
    use super::UserPromptSubmitCommandOutputWire;
    use super::input_schema_json;
    use super::output_schema_value;
    use super::write_schema_fixtures;
    use crate::events::common::SubagentHookContext;
    use pretty_assertions::assert_eq;
    use schemars::JsonSchema;
    use serde_json::Value;
    use serde_json::json;
    use tempfile::TempDir;

    fn expected_fixture(name: &str) -> &'static str {
        match name {
            POST_TOOL_USE_INPUT_FIXTURE => {
                include_str!("../schema/generated/post-tool-use.command.input.schema.json")
            }
            POST_TOOL_USE_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/post-tool-use.command.output.schema.json")
            }
            PERMISSION_REQUEST_INPUT_FIXTURE => {
                include_str!("../schema/generated/permission-request.command.input.schema.json")
            }
            PERMISSION_REQUEST_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/permission-request.command.output.schema.json")
            }
            POST_COMPACT_INPUT_FIXTURE => {
                include_str!("../schema/generated/post-compact.command.input.schema.json")
            }
            POST_COMPACT_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/post-compact.command.output.schema.json")
            }
            PRE_COMPACT_INPUT_FIXTURE => {
                include_str!("../schema/generated/pre-compact.command.input.schema.json")
            }
            PRE_COMPACT_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/pre-compact.command.output.schema.json")
            }
            PRE_TOOL_USE_INPUT_FIXTURE => {
                include_str!("../schema/generated/pre-tool-use.command.input.schema.json")
            }
            PRE_TOOL_USE_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/pre-tool-use.command.output.schema.json")
            }
            SESSION_START_INPUT_FIXTURE => {
                include_str!("../schema/generated/session-start.command.input.schema.json")
            }
            SESSION_START_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/session-start.command.output.schema.json")
            }
            USER_PROMPT_SUBMIT_INPUT_FIXTURE => {
                include_str!("../schema/generated/user-prompt-submit.command.input.schema.json")
            }
            USER_PROMPT_SUBMIT_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/user-prompt-submit.command.output.schema.json")
            }
            SUBAGENT_START_INPUT_FIXTURE => {
                include_str!("../schema/generated/subagent-start.command.input.schema.json")
            }
            SUBAGENT_START_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/subagent-start.command.output.schema.json")
            }
            SUBAGENT_STOP_INPUT_FIXTURE => {
                include_str!("../schema/generated/subagent-stop.command.input.schema.json")
            }
            SUBAGENT_STOP_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/subagent-stop.command.output.schema.json")
            }
            STOP_INPUT_FIXTURE => {
                include_str!("../schema/generated/stop.command.input.schema.json")
            }
            STOP_OUTPUT_FIXTURE => {
                include_str!("../schema/generated/stop.command.output.schema.json")
            }
            _ => panic!("unexpected fixture name: {name}"),
        }
    }

    fn normalize_newlines(value: &str) -> String {
        value.replace("\r\n", "\n")
    }

    fn assert_output_hook_event_name_const<T: JsonSchema>(
        kind: OutputSchemaKind,
        definition: &str,
        expected: &str,
    ) {
        let schema = output_schema_value::<T>(kind).expect("generate hook output schema");

        assert_eq!(
            schema["definitions"][definition]["properties"]["hookEventName"],
            json!({
                "const": expected,
                "type": "string",
            })
        );
    }

    #[test]
    fn generated_hook_schemas_match_fixtures() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let schema_root = temp_dir.path().join("schema");
        write_schema_fixtures(&schema_root).expect("write generated hook schemas");

        for fixture in [
            POST_TOOL_USE_INPUT_FIXTURE,
            POST_TOOL_USE_OUTPUT_FIXTURE,
            PERMISSION_REQUEST_INPUT_FIXTURE,
            PERMISSION_REQUEST_OUTPUT_FIXTURE,
            POST_COMPACT_INPUT_FIXTURE,
            POST_COMPACT_OUTPUT_FIXTURE,
            PRE_COMPACT_INPUT_FIXTURE,
            PRE_COMPACT_OUTPUT_FIXTURE,
            PRE_TOOL_USE_INPUT_FIXTURE,
            PRE_TOOL_USE_OUTPUT_FIXTURE,
            SESSION_START_INPUT_FIXTURE,
            SESSION_START_OUTPUT_FIXTURE,
            USER_PROMPT_SUBMIT_INPUT_FIXTURE,
            USER_PROMPT_SUBMIT_OUTPUT_FIXTURE,
            SUBAGENT_START_INPUT_FIXTURE,
            SUBAGENT_START_OUTPUT_FIXTURE,
            SUBAGENT_STOP_INPUT_FIXTURE,
            SUBAGENT_STOP_OUTPUT_FIXTURE,
            STOP_INPUT_FIXTURE,
            STOP_OUTPUT_FIXTURE,
        ] {
            let expected = normalize_newlines(expected_fixture(fixture));
            let actual = std::fs::read_to_string(schema_root.join("generated").join(fixture))
                .unwrap_or_else(|err| panic!("read generated schema {fixture}: {err}"));
            let actual = normalize_newlines(&actual);
            assert_eq!(expected, actual, "fixture should match generated schema");
        }
    }

    #[test]
    fn hook_specific_output_event_names_are_event_specific_in_output_schemas() {
        assert_output_hook_event_name_const::<PermissionRequestCommandOutputWire>(
            OutputSchemaKind::PermissionRequest,
            "PermissionRequestHookSpecificOutputWire",
            "PermissionRequest",
        );
        assert_output_hook_event_name_const::<PostToolUseCommandOutputWire>(
            OutputSchemaKind::PostToolUse,
            "PostToolUseHookSpecificOutputWire",
            "PostToolUse",
        );
        assert_output_hook_event_name_const::<PreToolUseCommandOutputWire>(
            OutputSchemaKind::PreToolUse,
            "PreToolUseHookSpecificOutputWire",
            "PreToolUse",
        );
        assert_output_hook_event_name_const::<SessionStartCommandOutputWire>(
            OutputSchemaKind::SessionStart,
            "SessionStartHookSpecificOutputWire",
            "SessionStart",
        );
        assert_output_hook_event_name_const::<SubagentStartCommandOutputWire>(
            OutputSchemaKind::SubagentStart,
            "SubagentStartHookSpecificOutputWire",
            "SubagentStart",
        );
        assert_output_hook_event_name_const::<UserPromptSubmitCommandOutputWire>(
            OutputSchemaKind::UserPromptSubmit,
            "UserPromptSubmitHookSpecificOutputWire",
            "UserPromptSubmit",
        );
    }

    #[test]
    fn output_schemas_allow_explicit_null_for_optional_fields() {
        let schema =
            output_schema_value::<SessionStartCommandOutputWire>(OutputSchemaKind::SessionStart)
                .expect("generate session start output schema");

        assert_eq!(
            schema["properties"]["systemMessage"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(
            schema["properties"]["hookSpecificOutput"]["anyOf"][1]["type"],
            "null"
        );
    }

    #[test]
    fn pre_tool_use_output_schema_excludes_unsupported_values() {
        let schema =
            output_schema_value::<PreToolUseCommandOutputWire>(OutputSchemaKind::PreToolUse)
                .expect("generate pre tool use output schema");

        assert_eq!(schema["properties"]["continue"]["const"], true);
        assert_eq!(schema["properties"]["stopReason"]["type"], "null");
        assert_eq!(schema["properties"]["suppressOutput"]["const"], false);
        assert_eq!(
            schema["definitions"]["PreToolUseDecisionWire"]["enum"],
            json!(["block"])
        );
        assert_eq!(
            schema["definitions"]["PreToolUsePermissionDecisionWire"]["enum"],
            json!(["allow", "deny"])
        );
    }

    #[test]
    fn reserved_output_fields_only_allow_neutral_values() {
        let permission = output_schema_value::<PermissionRequestCommandOutputWire>(
            OutputSchemaKind::PermissionRequest,
        )
        .expect("generate permission request output schema");
        let post_tool =
            output_schema_value::<PostToolUseCommandOutputWire>(OutputSchemaKind::PostToolUse)
                .expect("generate post tool use output schema");

        assert_eq!(
            permission["definitions"]["PermissionRequestDecisionWire"]["properties"]["updatedInput"]
                ["type"],
            "null"
        );
        assert_eq!(
            permission["definitions"]["PermissionRequestDecisionWire"]["properties"]["interrupt"]["const"],
            false
        );
        assert_eq!(
            post_tool["definitions"]["PostToolUseHookSpecificOutputWire"]["properties"]["updatedMCPToolOutput"]
                ["type"],
            "null"
        );
    }

    #[test]
    fn turn_scoped_hook_inputs_include_codex_turn_id_extension() {
        // Codex intentionally diverges from Claude's public hook docs here so
        // internal hook consumers can key off the active turn.
        let pre_tool_use: Value = serde_json::from_slice(
            &input_schema_json::<PreToolUseCommandInput>()
                .expect("serialize pre tool use input schema"),
        )
        .expect("parse pre tool use input schema");
        let post_tool_use: Value = serde_json::from_slice(
            &input_schema_json::<PostToolUseCommandInput>()
                .expect("serialize post tool use input schema"),
        )
        .expect("parse post tool use input schema");
        let pre_compact: Value = serde_json::from_slice(
            &input_schema_json::<PreCompactCommandInput>()
                .expect("serialize pre compact input schema"),
        )
        .expect("parse pre compact input schema");
        let post_compact: Value = serde_json::from_slice(
            &input_schema_json::<PostCompactCommandInput>()
                .expect("serialize post compact input schema"),
        )
        .expect("parse post compact input schema");
        let permission_request: Value = serde_json::from_slice(
            &input_schema_json::<PermissionRequestCommandInput>()
                .expect("serialize permission request input schema"),
        )
        .expect("parse permission request input schema");
        let user_prompt_submit: Value = serde_json::from_slice(
            &input_schema_json::<UserPromptSubmitCommandInput>()
                .expect("serialize user prompt submit input schema"),
        )
        .expect("parse user prompt submit input schema");
        let subagent_start: Value = serde_json::from_slice(
            &input_schema_json::<SubagentStartCommandInput>()
                .expect("serialize subagent start input schema"),
        )
        .expect("parse subagent start input schema");
        let subagent_stop: Value = serde_json::from_slice(
            &input_schema_json::<SubagentStopCommandInput>()
                .expect("serialize subagent stop input schema"),
        )
        .expect("parse subagent stop input schema");
        let stop: Value = serde_json::from_slice(
            &input_schema_json::<StopCommandInput>().expect("serialize stop input schema"),
        )
        .expect("parse stop input schema");

        for schema in [
            &pre_tool_use,
            &permission_request,
            &post_tool_use,
            &pre_compact,
            &post_compact,
            &user_prompt_submit,
            &subagent_start,
            &subagent_stop,
            &stop,
        ] {
            assert_eq!(schema["properties"]["turn_id"]["type"], "string");
            assert!(
                schema["required"]
                    .as_array()
                    .expect("schema required fields")
                    .contains(&Value::String("turn_id".to_string()))
            );
        }
    }

    #[test]
    fn subagent_context_fields_are_optional_for_hooks_that_run_inside_subagents() {
        let schemas = [
            input_schema_json::<PreToolUseCommandInput>()
                .expect("serialize pre tool use input schema"),
            input_schema_json::<PermissionRequestCommandInput>()
                .expect("serialize permission request input schema"),
            input_schema_json::<PostToolUseCommandInput>()
                .expect("serialize post tool use input schema"),
            input_schema_json::<PreCompactCommandInput>()
                .expect("serialize pre compact input schema"),
            input_schema_json::<PostCompactCommandInput>()
                .expect("serialize post compact input schema"),
            input_schema_json::<UserPromptSubmitCommandInput>()
                .expect("serialize user prompt submit input schema"),
        ];

        for schema in schemas {
            let schema: Value = serde_json::from_slice(&schema).expect("parse hook input schema");
            assert_eq!(schema["properties"]["agent_id"]["type"], "string");
            assert_eq!(schema["properties"]["agent_type"]["type"], "string");
            let required = schema["required"]
                .as_array()
                .expect("schema required fields");
            assert!(!required.contains(&Value::String("agent_id".to_string())));
            assert!(!required.contains(&Value::String("agent_type".to_string())));
        }
    }

    #[test]
    fn subagent_context_fields_serialize_flat_and_omit_when_absent() {
        let subagent = SubagentCommandInputFields::from(Some(&SubagentHookContext {
            agent_id: "agent-1".to_string(),
            agent_type: "worker".to_string(),
        }));
        let input = PreToolUseCommandInput {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            agent_id: subagent.agent_id,
            agent_type: subagent.agent_type,
            transcript_path: NullableString::from_path(/*path*/ None),
            cwd: "/tmp".to_string(),
            hook_event_name: "PreToolUse".to_string(),
            model: "gpt-test".to_string(),
            permission_mode: "default".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: json!({ "command": "echo hello" }),
            tool_use_id: "tool-1".to_string(),
        };

        assert_eq!(
            serde_json::to_value(input).expect("serialize subagent hook input"),
            json!({
                "session_id": "session-1",
                "turn_id": "turn-1",
                "agent_id": "agent-1",
                "agent_type": "worker",
                "transcript_path": null,
                "cwd": "/tmp",
                "hook_event_name": "PreToolUse",
                "model": "gpt-test",
                "permission_mode": "default",
                "tool_name": "Bash",
                "tool_input": { "command": "echo hello" },
                "tool_use_id": "tool-1",
            })
        );

        let root_input = PreToolUseCommandInput {
            session_id: "session-1".to_string(),
            turn_id: "turn-1".to_string(),
            agent_id: None,
            agent_type: None,
            transcript_path: NullableString::from_path(/*path*/ None),
            cwd: "/tmp".to_string(),
            hook_event_name: "PreToolUse".to_string(),
            model: "gpt-test".to_string(),
            permission_mode: "default".to_string(),
            tool_name: "Bash".to_string(),
            tool_input: json!({ "command": "echo hello" }),
            tool_use_id: "tool-1".to_string(),
        };
        let root_input = serde_json::to_value(root_input).expect("serialize root hook input");
        assert_eq!(root_input.get("agent_id"), None);
        assert_eq!(root_input.get("agent_type"), None);
    }
}
