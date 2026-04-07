use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use codex_utils_image::PromptImageMode;
use codex_utils_image::load_for_prompt_bytes;
use codex_utils_template::Template;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::ser::Serializer;
use ts_rs::TS;

use crate::config_types::ApprovalsReviewer;
use crate::config_types::CollaborationMode;
use crate::config_types::SandboxMode;
use crate::protocol::AskForApproval;
use crate::protocol::COLLABORATION_MODE_CLOSE_TAG;
use crate::protocol::COLLABORATION_MODE_OPEN_TAG;
use crate::protocol::GranularApprovalConfig;
use crate::protocol::NetworkAccess;
use crate::protocol::REALTIME_CONVERSATION_CLOSE_TAG;
use crate::protocol::REALTIME_CONVERSATION_OPEN_TAG;
use crate::protocol::SandboxPolicy;
use crate::protocol::WritableRoot;
use crate::user_input::UserInput;
use codex_execpolicy::Policy;
use codex_git_utils::GhostCommit;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_image::ImageProcessingError;
use schemars::JsonSchema;

use crate::mcp::CallToolResult;

static SANDBOX_MODE_DANGER_FULL_ACCESS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(SANDBOX_MODE_DANGER_FULL_ACCESS.trim_end())
        .unwrap_or_else(|err| panic!("danger-full-access sandbox template must parse: {err}"))
});
static SANDBOX_MODE_WORKSPACE_WRITE_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(SANDBOX_MODE_WORKSPACE_WRITE.trim_end())
        .unwrap_or_else(|err| panic!("workspace-write sandbox template must parse: {err}"))
});
static SANDBOX_MODE_READ_ONLY_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(SANDBOX_MODE_READ_ONLY.trim_end())
        .unwrap_or_else(|err| panic!("read-only sandbox template must parse: {err}"))
});

/// Controls the per-command sandbox override requested by a shell-like tool call.
#[derive(
    Debug, Clone, Copy, Default, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "snake_case")]
pub enum SandboxPermissions {
    /// Run with the turn's configured sandbox policy unchanged.
    #[default]
    UseDefault,
    /// Request to run outside the sandbox.
    RequireEscalated,
    /// Request to stay in the sandbox while widening permissions for this
    /// command only.
    WithAdditionalPermissions,
}

impl SandboxPermissions {
    /// True if SandboxPermissions requires full unsandboxed execution (i.e. RequireEscalated)
    pub fn requires_escalated_permissions(self) -> bool {
        matches!(self, SandboxPermissions::RequireEscalated)
    }

    /// True if SandboxPermissions requests any explicit per-command override
    /// beyond `UseDefault`.
    pub fn requests_sandbox_override(self) -> bool {
        !matches!(self, SandboxPermissions::UseDefault)
    }

    /// True if SandboxPermissions uses the sandboxed per-command permission
    /// widening flow.
    pub fn uses_additional_permissions(self) -> bool {
        matches!(self, SandboxPermissions::WithAdditionalPermissions)
    }
}

#[derive(Debug, Clone, Default, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct FileSystemPermissions {
    pub read: Option<Vec<AbsolutePathBuf>>,
    pub write: Option<Vec<AbsolutePathBuf>>,
}

impl FileSystemPermissions {
    pub fn is_empty(&self) -> bool {
        self.read.is_none() && self.write.is_none()
    }
}

#[derive(Debug, Clone, Default, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct NetworkPermissions {
    pub enabled: Option<bool>,
}

impl NetworkPermissions {
    pub fn is_empty(&self) -> bool {
        self.enabled.is_none()
    }
}

#[derive(Debug, Clone, Default, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct PermissionProfile {
    pub network: Option<NetworkPermissions>,
    pub file_system: Option<FileSystemPermissions>,
}

impl PermissionProfile {
    pub fn is_empty(&self) -> bool {
        self.network.is_none() && self.file_system.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseInputItem {
    Message {
        role: String,
        content: Vec<ContentItem>,
    },
    FunctionCallOutput {
        call_id: String,
        #[ts(as = "FunctionCallOutputBody")]
        #[schemars(with = "FunctionCallOutputBody")]
        output: FunctionCallOutputPayload,
    },
    McpToolCallOutput {
        call_id: String,
        output: CallToolResult,
    },
    CustomToolCallOutput {
        call_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        name: Option<String>,
        #[ts(as = "FunctionCallOutputBody")]
        #[schemars(with = "FunctionCallOutputBody")]
        output: FunctionCallOutputPayload,
    },
    ToolSearchOutput {
        call_id: String,
        status: String,
        execution: String,
        #[ts(type = "unknown[]")]
        tools: Vec<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItem {
    InputText { text: String },
    InputImage { image_url: String },
    OutputText { text: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Auto,
    Low,
    High,
    Original,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
/// Classifies an assistant message as interim commentary or final answer text.
///
/// Providers do not emit this consistently, so callers must treat `None` as
/// "phase unknown" and keep compatibility behavior for legacy models.
pub enum MessagePhase {
    /// Mid-turn assistant text (for example preamble/progress narration).
    ///
    /// Additional tool calls or assistant output may follow before turn
    /// completion.
    Commentary,
    /// The assistant's terminal answer text for the current turn.
    FinalAnswer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseItem {
    Message {
        #[serde(default, skip_serializing)]
        #[ts(skip)]
        id: Option<String>,
        role: String,
        content: Vec<ContentItem>,
        // Do not use directly, no available consistently across all providers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        end_turn: Option<bool>,
        // Optional output-message phase (for example: "commentary", "final_answer").
        // Availability varies by provider/model, so downstream consumers must
        // preserve fallback behavior when this is absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        phase: Option<MessagePhase>,
    },
    Reasoning {
        #[serde(default, skip_serializing)]
        #[ts(skip)]
        #[schemars(skip)]
        id: String,
        summary: Vec<ReasoningItemReasoningSummary>,
        #[serde(default, skip_serializing_if = "should_serialize_reasoning_content")]
        #[ts(optional)]
        content: Option<Vec<ReasoningItemContent>>,
        encrypted_content: Option<String>,
    },
    LocalShellCall {
        /// Legacy id field retained for compatibility with older payloads.
        #[serde(default, skip_serializing)]
        #[ts(skip)]
        id: Option<String>,
        /// Set when using the Responses API.
        call_id: Option<String>,
        status: LocalShellStatus,
        action: LocalShellAction,
    },
    FunctionCall {
        #[serde(default, skip_serializing)]
        #[ts(skip)]
        id: Option<String>,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        namespace: Option<String>,
        // The Responses API returns the function call arguments as a *string* that contains
        // JSON, not as an already‑parsed object. We keep it as a raw string here and let
        // Session::handle_function_call parse it into a Value.
        arguments: String,
        call_id: String,
    },
    ToolSearchCall {
        #[serde(default, skip_serializing)]
        #[ts(skip)]
        id: Option<String>,
        call_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        status: Option<String>,
        execution: String,
        #[ts(type = "unknown")]
        arguments: serde_json::Value,
    },
    // NOTE: The `output` field for `function_call_output` uses a dedicated payload type with
    // custom serialization. On the wire it is either:
    //   - a plain string (`content`)
    //   - an array of structured content items (`content_items`)
    // We keep this behavior centralized in `FunctionCallOutputPayload`.
    FunctionCallOutput {
        call_id: String,
        #[ts(as = "FunctionCallOutputBody")]
        #[schemars(with = "FunctionCallOutputBody")]
        output: FunctionCallOutputPayload,
    },
    CustomToolCall {
        #[serde(default, skip_serializing)]
        #[ts(skip)]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        status: Option<String>,

        call_id: String,
        name: String,
        input: String,
    },
    // `custom_tool_call_output.output` uses the same wire encoding as
    // `function_call_output.output` so freeform tools can return either plain
    // text or structured content items.
    CustomToolCallOutput {
        call_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        name: Option<String>,
        #[ts(as = "FunctionCallOutputBody")]
        #[schemars(with = "FunctionCallOutputBody")]
        output: FunctionCallOutputPayload,
    },
    ToolSearchOutput {
        call_id: Option<String>,
        status: String,
        execution: String,
        #[ts(type = "unknown[]")]
        tools: Vec<serde_json::Value>,
    },
    // Emitted by the Responses API when the agent triggers a web search.
    // Example payload (from SSE `response.output_item.done`):
    // {
    //   "id":"ws_...",
    //   "type":"web_search_call",
    //   "status":"completed",
    //   "action": {"type":"search","query":"weather: San Francisco, CA"}
    // }
    WebSearchCall {
        #[serde(default, skip_serializing)]
        #[ts(skip)]
        id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        action: Option<WebSearchAction>,
    },
    // Emitted by the Responses API when the agent triggers image generation.
    // Example payload:
    // {
    //   "id":"ig_123",
    //   "type":"image_generation_call",
    //   "status":"completed",
    //   "revised_prompt":"A gray tabby cat hugging an otter...",
    //   "result":"..."
    // }
    ImageGenerationCall {
        id: String,
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        revised_prompt: Option<String>,
        result: String,
    },
    // Generated by the harness but considered exactly as a model response.
    GhostSnapshot {
        ghost_commit: GhostCommit,
    },
    #[serde(alias = "compaction_summary")]
    Compaction {
        encrypted_content: String,
    },
    #[serde(other)]
    Other,
}

pub const BASE_INSTRUCTIONS_DEFAULT: &str = include_str!("prompts/base_instructions/default.md");

/// Base instructions for the model in a thread. Corresponds to the `instructions` field in the ResponsesAPI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(rename = "base_instructions", rename_all = "snake_case")]
pub struct BaseInstructions {
    pub text: String,
}

impl Default for BaseInstructions {
    fn default() -> Self {
        Self {
            text: BASE_INSTRUCTIONS_DEFAULT.to_string(),
        }
    }
}

/// Developer-provided guidance that is injected into a turn as a developer role
/// message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(rename = "developer_instructions", rename_all = "snake_case")]
pub struct DeveloperInstructions {
    text: String,
}

const APPROVAL_POLICY_NEVER: &str = include_str!("prompts/permissions/approval_policy/never.md");
const APPROVAL_POLICY_UNLESS_TRUSTED: &str =
    include_str!("prompts/permissions/approval_policy/unless_trusted.md");
const APPROVAL_POLICY_ON_FAILURE: &str =
    include_str!("prompts/permissions/approval_policy/on_failure.md");
const APPROVAL_POLICY_ON_REQUEST_RULE: &str =
    include_str!("prompts/permissions/approval_policy/on_request.md");
const APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION: &str =
    include_str!("prompts/permissions/approval_policy/on_request_rule_request_permission.md");
const GUARDIAN_SUBAGENT_APPROVAL_SUFFIX: &str = "`approvals_reviewer` is `guardian_subagent`: Sandbox escalations with require_escalated will be reviewed for compliance with the policy. If a rejection happens, you should proceed only with a materially safer alternative, or inform the user of the risk and send a final message to ask for approval.";

const SANDBOX_MODE_DANGER_FULL_ACCESS: &str =
    include_str!("prompts/permissions/sandbox_mode/danger_full_access.md");
const SANDBOX_MODE_WORKSPACE_WRITE: &str =
    include_str!("prompts/permissions/sandbox_mode/workspace_write.md");
const SANDBOX_MODE_READ_ONLY: &str = include_str!("prompts/permissions/sandbox_mode/read_only.md");

const REALTIME_START_INSTRUCTIONS: &str = include_str!("prompts/realtime/realtime_start.md");
const REALTIME_END_INSTRUCTIONS: &str = include_str!("prompts/realtime/realtime_end.md");

struct PermissionsPromptConfig<'a> {
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    exec_policy: &'a Policy,
    exec_permission_approvals_enabled: bool,
    request_permissions_tool_enabled: bool,
}

impl DeveloperInstructions {
    pub fn new<T: Into<String>>(text: T) -> Self {
        Self { text: text.into() }
    }

    pub fn from(
        approval_policy: AskForApproval,
        approvals_reviewer: ApprovalsReviewer,
        exec_policy: &Policy,
        exec_permission_approvals_enabled: bool,
        request_permissions_tool_enabled: bool,
    ) -> DeveloperInstructions {
        let with_request_permissions_tool = |text: &str| {
            if request_permissions_tool_enabled {
                format!("{text}\n\n{}", request_permissions_tool_prompt_section())
            } else {
                text.to_string()
            }
        };
        let on_request_instructions = || {
            let on_request_rule = if exec_permission_approvals_enabled {
                APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION.to_string()
            } else {
                APPROVAL_POLICY_ON_REQUEST_RULE.to_string()
            };
            let mut sections = vec![on_request_rule];
            if request_permissions_tool_enabled {
                sections.push(request_permissions_tool_prompt_section().to_string());
            }
            if let Some(prefixes) = approved_command_prefixes_text(exec_policy) {
                sections.push(format!(
                    "## Approved command prefixes\nThe following prefix rules have already been approved: {prefixes}"
                ));
            }
            sections.join("\n\n")
        };
        let text = match approval_policy {
            AskForApproval::Never => APPROVAL_POLICY_NEVER.to_string(),
            AskForApproval::UnlessTrusted => {
                with_request_permissions_tool(APPROVAL_POLICY_UNLESS_TRUSTED)
            }
            AskForApproval::OnFailure => with_request_permissions_tool(APPROVAL_POLICY_ON_FAILURE),
            AskForApproval::OnRequest => on_request_instructions(),
            AskForApproval::Granular(granular_config) => granular_instructions(
                granular_config,
                exec_policy,
                exec_permission_approvals_enabled,
                request_permissions_tool_enabled,
            ),
        };

        let text = if approvals_reviewer == ApprovalsReviewer::GuardianSubagent
            && approval_policy != AskForApproval::Never
        {
            format!("{text}\n\n{GUARDIAN_SUBAGENT_APPROVAL_SUFFIX}")
        } else {
            text
        };

        DeveloperInstructions::new(text)
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub fn concat(self, other: impl Into<DeveloperInstructions>) -> Self {
        let mut text = self.text;
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&other.into().text);
        Self { text }
    }

    pub fn model_switch_message(model_instructions: String) -> Self {
        DeveloperInstructions::new(format!(
            "<model_switch>\nThe user was previously using a different model. Please continue the conversation according to the following instructions:\n\n{model_instructions}\n</model_switch>"
        ))
    }

    pub fn realtime_start_message() -> Self {
        Self::realtime_start_message_with_instructions(REALTIME_START_INSTRUCTIONS.trim())
    }

    pub fn realtime_start_message_with_instructions(instructions: &str) -> Self {
        DeveloperInstructions::new(format!(
            "{REALTIME_CONVERSATION_OPEN_TAG}\n{instructions}\n{REALTIME_CONVERSATION_CLOSE_TAG}"
        ))
    }

    pub fn realtime_end_message(reason: &str) -> Self {
        DeveloperInstructions::new(format!(
            "{REALTIME_CONVERSATION_OPEN_TAG}\n{}\n\nReason: {reason}\n{REALTIME_CONVERSATION_CLOSE_TAG}",
            REALTIME_END_INSTRUCTIONS.trim()
        ))
    }

    pub fn personality_spec_message(spec: String) -> Self {
        let message = format!(
            "<personality_spec> The user has requested a new communication style. Future messages should adhere to the following personality: \n{spec} </personality_spec>"
        );
        DeveloperInstructions::new(message)
    }

    pub fn from_policy(
        sandbox_policy: &SandboxPolicy,
        approval_policy: AskForApproval,
        approvals_reviewer: ApprovalsReviewer,
        exec_policy: &Policy,
        cwd: &Path,
        exec_permission_approvals_enabled: bool,
        request_permissions_tool_enabled: bool,
    ) -> Self {
        let network_access = if sandbox_policy.has_full_network_access() {
            NetworkAccess::Enabled
        } else {
            NetworkAccess::Restricted
        };

        let (sandbox_mode, writable_roots) = match sandbox_policy {
            SandboxPolicy::DangerFullAccess => (SandboxMode::DangerFullAccess, None),
            SandboxPolicy::ReadOnly { .. } => (SandboxMode::ReadOnly, None),
            SandboxPolicy::ExternalSandbox { .. } => (SandboxMode::DangerFullAccess, None),
            SandboxPolicy::WorkspaceWrite { .. } => {
                let roots = sandbox_policy.get_writable_roots_with_cwd(cwd);
                (SandboxMode::WorkspaceWrite, Some(roots))
            }
        };

        DeveloperInstructions::from_permissions_with_network(
            sandbox_mode,
            network_access,
            PermissionsPromptConfig {
                approval_policy,
                approvals_reviewer,
                exec_policy,
                exec_permission_approvals_enabled,
                request_permissions_tool_enabled,
            },
            writable_roots,
        )
    }

    /// Returns developer instructions from a collaboration mode if they exist and are non-empty.
    pub fn from_collaboration_mode(collaboration_mode: &CollaborationMode) -> Option<Self> {
        collaboration_mode
            .settings
            .developer_instructions
            .as_ref()
            .filter(|instructions| !instructions.is_empty())
            .map(|instructions| {
                DeveloperInstructions::new(format!(
                    "{COLLABORATION_MODE_OPEN_TAG}{instructions}{COLLABORATION_MODE_CLOSE_TAG}"
                ))
            })
    }

    fn from_permissions_with_network(
        sandbox_mode: SandboxMode,
        network_access: NetworkAccess,
        config: PermissionsPromptConfig<'_>,
        writable_roots: Option<Vec<WritableRoot>>,
    ) -> Self {
        let start_tag = DeveloperInstructions::new("<permissions instructions>");
        let end_tag = DeveloperInstructions::new("</permissions instructions>");
        start_tag
            .concat(DeveloperInstructions::sandbox_text(
                sandbox_mode,
                network_access,
            ))
            .concat(DeveloperInstructions::from(
                config.approval_policy,
                config.approvals_reviewer,
                config.exec_policy,
                config.exec_permission_approvals_enabled,
                config.request_permissions_tool_enabled,
            ))
            .concat(DeveloperInstructions::from_writable_roots(writable_roots))
            .concat(end_tag)
    }

    fn from_writable_roots(writable_roots: Option<Vec<WritableRoot>>) -> Self {
        let Some(roots) = writable_roots else {
            return DeveloperInstructions::new("");
        };

        if roots.is_empty() {
            return DeveloperInstructions::new("");
        }

        let roots_list: Vec<String> = roots
            .iter()
            .map(|r| format!("`{}`", r.root.to_string_lossy()))
            .collect();
        let text = if roots_list.len() == 1 {
            format!(" The writable root is {}.", roots_list[0])
        } else {
            format!(" The writable roots are {}.", roots_list.join(", "))
        };
        DeveloperInstructions::new(text)
    }

    fn sandbox_text(mode: SandboxMode, network_access: NetworkAccess) -> DeveloperInstructions {
        let template = match mode {
            SandboxMode::DangerFullAccess => &*SANDBOX_MODE_DANGER_FULL_ACCESS_TEMPLATE,
            SandboxMode::WorkspaceWrite => &*SANDBOX_MODE_WORKSPACE_WRITE_TEMPLATE,
            SandboxMode::ReadOnly => &*SANDBOX_MODE_READ_ONLY_TEMPLATE,
        };
        let network_access = network_access.to_string();
        let text = template
            .render([("network_access", network_access.as_str())])
            .unwrap_or_else(|err| panic!("sandbox template must render: {err}"));

        DeveloperInstructions::new(text)
    }
}

fn approved_command_prefixes_text(exec_policy: &Policy) -> Option<String> {
    format_allow_prefixes(exec_policy.get_allowed_prefixes())
        .filter(|prefixes| !prefixes.is_empty())
}

fn granular_prompt_intro_text() -> &'static str {
    "# Approval Requests\n\nApproval policy is `granular`. Categories set to `false` are automatically rejected instead of prompting the user."
}

fn request_permissions_tool_prompt_section() -> &'static str {
    "# request_permissions Tool\n\nThe built-in `request_permissions` tool is available in this session. Invoke it when you need to request additional `network` or `file_system` permissions before later shell-like commands need them. Request only the specific permissions required for the task."
}

fn granular_instructions(
    granular_config: GranularApprovalConfig,
    exec_policy: &Policy,
    exec_permission_approvals_enabled: bool,
    request_permissions_tool_enabled: bool,
) -> String {
    let sandbox_approval_prompts_allowed = granular_config.allows_sandbox_approval();
    let shell_permission_requests_available =
        exec_permission_approvals_enabled && sandbox_approval_prompts_allowed;
    let request_permissions_tool_prompts_allowed =
        request_permissions_tool_enabled && granular_config.allows_request_permissions();
    let categories = [
        Some((
            granular_config.allows_sandbox_approval(),
            "`sandbox_approval`",
        )),
        Some((granular_config.allows_rules_approval(), "`rules`")),
        Some((granular_config.allows_skill_approval(), "`skill_approval`")),
        request_permissions_tool_enabled.then_some((
            granular_config.allows_request_permissions(),
            "`request_permissions`",
        )),
        Some((
            granular_config.allows_mcp_elicitations(),
            "`mcp_elicitations`",
        )),
    ];
    let prompted_categories = categories
        .iter()
        .flatten()
        .filter(|&&(is_allowed, _)| is_allowed)
        .map(|&(_, category)| format!("- {category}"))
        .collect::<Vec<_>>();
    let rejected_categories = categories
        .iter()
        .flatten()
        .filter(|&&(is_allowed, _)| !is_allowed)
        .map(|&(_, category)| format!("- {category}"))
        .collect::<Vec<_>>();

    let mut sections = vec![granular_prompt_intro_text().to_string()];

    if !prompted_categories.is_empty() {
        sections.push(format!(
            "These approval categories may still prompt the user when needed:\n{}",
            prompted_categories.join("\n")
        ));
    }
    if !rejected_categories.is_empty() {
        sections.push(format!(
            "These approval categories are automatically rejected instead of prompting the user:\n{}",
            rejected_categories.join("\n")
        ));
    }

    if shell_permission_requests_available {
        sections.push(APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION.to_string());
    }

    if request_permissions_tool_prompts_allowed {
        sections.push(request_permissions_tool_prompt_section().to_string());
    }

    if let Some(prefixes) = approved_command_prefixes_text(exec_policy) {
        sections.push(format!(
            "## Approved command prefixes\nThe following prefix rules have already been approved: {prefixes}"
        ));
    }

    sections.join("\n\n")
}

const MAX_RENDERED_PREFIXES: usize = 100;
const MAX_ALLOW_PREFIX_TEXT_BYTES: usize = 5000;
const TRUNCATED_MARKER: &str = "...\n[Some commands were truncated]";

pub fn format_allow_prefixes(prefixes: Vec<Vec<String>>) -> Option<String> {
    let mut truncated = false;
    if prefixes.len() > MAX_RENDERED_PREFIXES {
        truncated = true;
    }

    let mut prefixes = prefixes;
    prefixes.sort_by(|a, b| {
        a.len()
            .cmp(&b.len())
            .then_with(|| prefix_combined_str_len(a).cmp(&prefix_combined_str_len(b)))
            .then_with(|| a.cmp(b))
    });

    let full_text = prefixes
        .into_iter()
        .take(MAX_RENDERED_PREFIXES)
        .map(|prefix| format!("- {}", render_command_prefix(&prefix)))
        .collect::<Vec<_>>()
        .join("\n");

    // truncate to last UTF8 char
    let mut output = full_text;
    let byte_idx = output
        .char_indices()
        .nth(MAX_ALLOW_PREFIX_TEXT_BYTES)
        .map(|(i, _)| i);
    if let Some(byte_idx) = byte_idx {
        truncated = true;
        output = output[..byte_idx].to_string();
    }

    if truncated {
        Some(format!("{output}{TRUNCATED_MARKER}"))
    } else {
        Some(output)
    }
}

fn prefix_combined_str_len(prefix: &[String]) -> usize {
    prefix.iter().map(String::len).sum()
}

fn render_command_prefix(prefix: &[String]) -> String {
    let tokens = prefix
        .iter()
        .map(|token| serde_json::to_string(token).unwrap_or_else(|_| format!("{token:?}")))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{tokens}]")
}

impl From<DeveloperInstructions> for ResponseItem {
    fn from(di: DeveloperInstructions) -> Self {
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: di.into_text(),
            }],
            end_turn: None,
            phase: None,
        }
    }
}

impl From<SandboxMode> for DeveloperInstructions {
    fn from(mode: SandboxMode) -> Self {
        let network_access = match mode {
            SandboxMode::DangerFullAccess => NetworkAccess::Enabled,
            SandboxMode::WorkspaceWrite | SandboxMode::ReadOnly => NetworkAccess::Restricted,
        };

        DeveloperInstructions::sandbox_text(mode, network_access)
    }
}

fn should_serialize_reasoning_content(content: &Option<Vec<ReasoningItemContent>>) -> bool {
    match content {
        Some(content) => !content
            .iter()
            .any(|c| matches!(c, ReasoningItemContent::ReasoningText { .. })),
        None => false,
    }
}

fn local_image_error_placeholder(
    path: &std::path::Path,
    error: impl std::fmt::Display,
) -> ContentItem {
    ContentItem::InputText {
        text: format!(
            "Codex could not read the local image at `{}`: {}",
            path.display(),
            error
        ),
    }
}

pub const VIEW_IMAGE_TOOL_NAME: &str = "view_image";

const IMAGE_OPEN_TAG: &str = "<image>";
const IMAGE_CLOSE_TAG: &str = "</image>";
const LOCAL_IMAGE_OPEN_TAG_PREFIX: &str = "<image name=";
const LOCAL_IMAGE_OPEN_TAG_SUFFIX: &str = ">";
const LOCAL_IMAGE_CLOSE_TAG: &str = IMAGE_CLOSE_TAG;

pub fn image_open_tag_text() -> String {
    IMAGE_OPEN_TAG.to_string()
}

pub fn image_close_tag_text() -> String {
    IMAGE_CLOSE_TAG.to_string()
}

pub fn local_image_label_text(label_number: usize) -> String {
    format!("[Image #{label_number}]")
}

pub fn local_image_open_tag_text(label_number: usize) -> String {
    let label = local_image_label_text(label_number);
    format!("{LOCAL_IMAGE_OPEN_TAG_PREFIX}{label}{LOCAL_IMAGE_OPEN_TAG_SUFFIX}")
}

pub fn is_local_image_open_tag_text(text: &str) -> bool {
    text.strip_prefix(LOCAL_IMAGE_OPEN_TAG_PREFIX)
        .is_some_and(|rest| rest.ends_with(LOCAL_IMAGE_OPEN_TAG_SUFFIX))
}

pub fn is_local_image_close_tag_text(text: &str) -> bool {
    is_image_close_tag_text(text)
}

pub fn is_image_open_tag_text(text: &str) -> bool {
    text == IMAGE_OPEN_TAG
}

pub fn is_image_close_tag_text(text: &str) -> bool {
    text == IMAGE_CLOSE_TAG
}

fn invalid_image_error_placeholder(
    path: &std::path::Path,
    error: impl std::fmt::Display,
) -> ContentItem {
    ContentItem::InputText {
        text: format!(
            "Image located at `{}` is invalid: {}",
            path.display(),
            error
        ),
    }
}

fn unsupported_image_error_placeholder(path: &std::path::Path, mime: &str) -> ContentItem {
    ContentItem::InputText {
        text: format!(
            "Codex cannot attach image at `{}`: unsupported image `{}`.",
            path.display(),
            mime
        ),
    }
}

pub fn local_image_content_items_with_label_number(
    path: &std::path::Path,
    file_bytes: Vec<u8>,
    label_number: Option<usize>,
    mode: PromptImageMode,
) -> Vec<ContentItem> {
    match load_for_prompt_bytes(path, file_bytes, mode) {
        Ok(image) => {
            let mut items = Vec::with_capacity(3);
            if let Some(label_number) = label_number {
                items.push(ContentItem::InputText {
                    text: local_image_open_tag_text(label_number),
                });
            }
            items.push(ContentItem::InputImage {
                image_url: image.into_data_url(),
            });
            if label_number.is_some() {
                items.push(ContentItem::InputText {
                    text: LOCAL_IMAGE_CLOSE_TAG.to_string(),
                });
            }
            items
        }
        Err(err) => match &err {
            ImageProcessingError::Read { .. } | ImageProcessingError::Encode { .. } => {
                vec![local_image_error_placeholder(path, &err)]
            }
            ImageProcessingError::Decode { .. } if err.is_invalid_image() => {
                vec![invalid_image_error_placeholder(path, &err)]
            }
            ImageProcessingError::Decode { .. } => {
                vec![local_image_error_placeholder(path, &err)]
            }
            ImageProcessingError::UnsupportedImageFormat { mime } => {
                vec![unsupported_image_error_placeholder(path, mime)]
            }
        },
    }
}

impl From<ResponseInputItem> for ResponseItem {
    fn from(item: ResponseInputItem) -> Self {
        match item {
            ResponseInputItem::Message { role, content } => Self::Message {
                role,
                content,
                id: None,
                end_turn: None,
                phase: None,
            },
            ResponseInputItem::FunctionCallOutput { call_id, output } => {
                Self::FunctionCallOutput { call_id, output }
            }
            ResponseInputItem::McpToolCallOutput { call_id, output } => {
                let output = output.into_function_call_output_payload();
                Self::FunctionCallOutput { call_id, output }
            }
            ResponseInputItem::CustomToolCallOutput {
                call_id,
                name,
                output,
            } => Self::CustomToolCallOutput {
                call_id,
                name,
                output,
            },
            ResponseInputItem::ToolSearchOutput {
                call_id,
                status,
                execution,
                tools,
            } => Self::ToolSearchOutput {
                call_id: Some(call_id),
                status,
                execution,
                tools,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum LocalShellStatus {
    Completed,
    InProgress,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LocalShellAction {
    Exec(LocalShellExecAction),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
pub struct LocalShellExecAction {
    pub command: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub working_directory: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[schemars(rename = "ResponsesApiWebSearchAction")]
pub enum WebSearchAction {
    Search {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        query: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        queries: Option<Vec<String>>,
    },
    OpenPage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        url: Option<String>,
    },
    FindInPage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        pattern: Option<String>,
    },

    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReasoningItemReasoningSummary {
    SummaryText { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReasoningItemContent {
    ReasoningText { text: String },
    Text { text: String },
}

impl From<Vec<UserInput>> for ResponseInputItem {
    fn from(items: Vec<UserInput>) -> Self {
        let mut image_index = 0;
        Self::Message {
            role: "user".to_string(),
            content: items
                .into_iter()
                .flat_map(|c| match c {
                    UserInput::Text { text, .. } => vec![ContentItem::InputText { text }],
                    UserInput::Image { image_url } => {
                        image_index += 1;
                        vec![
                            ContentItem::InputText {
                                text: image_open_tag_text(),
                            },
                            ContentItem::InputImage { image_url },
                            ContentItem::InputText {
                                text: image_close_tag_text(),
                            },
                        ]
                    }
                    UserInput::LocalImage { path } => {
                        image_index += 1;
                        match std::fs::read(&path) {
                            Ok(file_bytes) => local_image_content_items_with_label_number(
                                &path,
                                file_bytes,
                                Some(image_index),
                                PromptImageMode::ResizeToFit,
                            ),
                            Err(err) => vec![local_image_error_placeholder(&path, err)],
                        }
                    }
                    UserInput::Skill { .. } | UserInput::Mention { .. } => Vec::new(), // Tool bodies are injected later in core
                })
                .collect::<Vec<ContentItem>>(),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
pub struct SearchToolCallParams {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<usize>,
}

/// If the `name` of a `ResponseItem::FunctionCall` is either `container.exec`
/// or `shell`, the `arguments` field should deserialize to this struct.
#[derive(Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
pub struct ShellToolCallParams {
    pub command: Vec<String>,
    pub workdir: Option<String>,

    /// This is the maximum time in milliseconds that the command is allowed to run.
    #[serde(alias = "timeout")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sandbox_permissions: Option<SandboxPermissions>,
    /// Suggests a command prefix to persist for future sessions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prefix_rule: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub additional_permissions: Option<PermissionProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

/// If the `name` of a `ResponseItem::FunctionCall` is `shell_command`, the
/// `arguments` field should deserialize to this struct.
#[derive(Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
pub struct ShellCommandToolCallParams {
    pub command: String,
    pub workdir: Option<String>,

    /// Whether to run the shell with login shell semantics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<bool>,
    /// This is the maximum time in milliseconds that the command is allowed to run.
    #[serde(alias = "timeout")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sandbox_permissions: Option<SandboxPermissions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prefix_rule: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub additional_permissions: Option<PermissionProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

/// Responses API compatible content items that can be returned by a tool call.
/// This is a subset of ContentItem with the types we support as function call outputs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FunctionCallOutputContentItem {
    // Do not rename, these are serialized and used directly in the responses API.
    InputText {
        text: String,
    },
    // Do not rename, these are serialized and used directly in the responses API.
    InputImage {
        image_url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        detail: Option<ImageDetail>,
    },
}

/// Converts structured function-call output content into plain text for
/// human-readable surfaces.
///
/// This conversion is intentionally lossy:
/// - only `input_text` items are included
/// - image items are ignored
///
/// We use this helper where callers still need a string representation (for
/// example telemetry previews or legacy string-only output paths) while keeping
/// the original multimodal `content_items` as the authoritative payload sent to
/// the model.
pub fn function_call_output_content_items_to_text(
    content_items: &[FunctionCallOutputContentItem],
) -> Option<String> {
    let text_segments = content_items
        .iter()
        .filter_map(|item| match item {
            FunctionCallOutputContentItem::InputText { text } if !text.trim().is_empty() => {
                Some(text.as_str())
            }
            FunctionCallOutputContentItem::InputText { .. }
            | FunctionCallOutputContentItem::InputImage { .. } => None,
        })
        .collect::<Vec<_>>();

    if text_segments.is_empty() {
        None
    } else {
        Some(text_segments.join("\n"))
    }
}

impl From<crate::dynamic_tools::DynamicToolCallOutputContentItem>
    for FunctionCallOutputContentItem
{
    fn from(item: crate::dynamic_tools::DynamicToolCallOutputContentItem) -> Self {
        match item {
            crate::dynamic_tools::DynamicToolCallOutputContentItem::InputText { text } => {
                Self::InputText { text }
            }
            crate::dynamic_tools::DynamicToolCallOutputContentItem::InputImage { image_url } => {
                Self::InputImage {
                    image_url,
                    detail: None,
                }
            }
        }
    }
}

/// The payload we send back to OpenAI when reporting a tool call result.
///
/// `body` serializes directly as the wire value for `function_call_output.output`.
/// `success` remains internal metadata for downstream handling.
#[derive(Debug, Default, Clone, PartialEq, JsonSchema, TS)]
pub struct FunctionCallOutputPayload {
    pub body: FunctionCallOutputBody,
    pub success: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema, TS)]
#[serde(untagged)]
pub enum FunctionCallOutputBody {
    Text(String),
    ContentItems(Vec<FunctionCallOutputContentItem>),
}

impl FunctionCallOutputBody {
    /// Best-effort conversion of a function-call output body to plain text for
    /// human-readable surfaces.
    ///
    /// This conversion is intentionally lossy when the body contains content
    /// items: image entries are dropped and text entries are joined with
    /// newlines.
    pub fn to_text(&self) -> Option<String> {
        match self {
            Self::Text(content) => Some(content.clone()),
            Self::ContentItems(items) => function_call_output_content_items_to_text(items),
        }
    }
}

impl Default for FunctionCallOutputBody {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

impl FunctionCallOutputPayload {
    pub fn from_text(content: String) -> Self {
        Self {
            body: FunctionCallOutputBody::Text(content),
            success: None,
        }
    }

    pub fn from_content_items(content_items: Vec<FunctionCallOutputContentItem>) -> Self {
        Self {
            body: FunctionCallOutputBody::ContentItems(content_items),
            success: None,
        }
    }

    pub fn text_content(&self) -> Option<&str> {
        match &self.body {
            FunctionCallOutputBody::Text(content) => Some(content),
            FunctionCallOutputBody::ContentItems(_) => None,
        }
    }

    pub fn text_content_mut(&mut self) -> Option<&mut String> {
        match &mut self.body {
            FunctionCallOutputBody::Text(content) => Some(content),
            FunctionCallOutputBody::ContentItems(_) => None,
        }
    }

    pub fn content_items(&self) -> Option<&[FunctionCallOutputContentItem]> {
        match &self.body {
            FunctionCallOutputBody::Text(_) => None,
            FunctionCallOutputBody::ContentItems(items) => Some(items),
        }
    }

    pub fn content_items_mut(&mut self) -> Option<&mut Vec<FunctionCallOutputContentItem>> {
        match &mut self.body {
            FunctionCallOutputBody::Text(_) => None,
            FunctionCallOutputBody::ContentItems(items) => Some(items),
        }
    }
}

// `function_call_output.output` is encoded as either:
//   - an array of structured content items
//   - a plain string
impl Serialize for FunctionCallOutputPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.body {
            FunctionCallOutputBody::Text(content) => serializer.serialize_str(content),
            FunctionCallOutputBody::ContentItems(items) => items.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for FunctionCallOutputPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let body = FunctionCallOutputBody::deserialize(deserializer)?;
        Ok(FunctionCallOutputPayload {
            body,
            success: None,
        })
    }
}

impl CallToolResult {
    pub fn from_result(result: Result<Self, String>) -> Self {
        match result {
            Ok(result) => result,
            Err(error) => Self::from_error_text(error),
        }
    }

    pub fn from_error_text(text: String) -> Self {
        Self {
            content: vec![serde_json::json!({
                "type": "text",
                "text": text,
            })],
            structured_content: None,
            is_error: Some(true),
            meta: None,
        }
    }

    pub fn success(&self) -> bool {
        self.is_error != Some(true)
    }

    pub fn as_function_call_output_payload(&self) -> FunctionCallOutputPayload {
        if let Some(structured_content) = &self.structured_content
            && !structured_content.is_null()
        {
            match serde_json::to_string(structured_content) {
                Ok(serialized_structured_content) => {
                    return FunctionCallOutputPayload {
                        body: FunctionCallOutputBody::Text(serialized_structured_content),
                        success: Some(self.success()),
                    };
                }
                Err(err) => {
                    return FunctionCallOutputPayload {
                        body: FunctionCallOutputBody::Text(err.to_string()),
                        success: Some(false),
                    };
                }
            }
        }

        let serialized_content = match serde_json::to_string(&self.content) {
            Ok(serialized_content) => serialized_content,
            Err(err) => {
                return FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::Text(err.to_string()),
                    success: Some(false),
                };
            }
        };

        let content_items = convert_mcp_content_to_items(&self.content);

        let body = match content_items {
            Some(content_items) => FunctionCallOutputBody::ContentItems(content_items),
            None => FunctionCallOutputBody::Text(serialized_content),
        };

        FunctionCallOutputPayload {
            body,
            success: Some(self.success()),
        }
    }

    pub fn into_function_call_output_payload(self) -> FunctionCallOutputPayload {
        self.as_function_call_output_payload()
    }
}

fn convert_mcp_content_to_items(
    contents: &[serde_json::Value],
) -> Option<Vec<FunctionCallOutputContentItem>> {
    #[derive(serde::Deserialize)]
    #[serde(tag = "type")]
    enum McpContent {
        #[serde(rename = "text")]
        Text { text: String },
        #[serde(rename = "image")]
        Image {
            data: String,
            #[serde(rename = "mimeType", alias = "mime_type")]
            mime_type: Option<String>,
        },
        #[serde(other)]
        Unknown,
    }

    let mut saw_image = false;
    let mut items = Vec::with_capacity(contents.len());

    for content in contents {
        let item = match serde_json::from_value::<McpContent>(content.clone()) {
            Ok(McpContent::Text { text }) => FunctionCallOutputContentItem::InputText { text },
            Ok(McpContent::Image { data, mime_type }) => {
                saw_image = true;
                let image_url = if data.starts_with("data:") {
                    data
                } else {
                    let mime_type = mime_type.unwrap_or_else(|| "application/octet-stream".into());
                    format!("data:{mime_type};base64,{data}")
                };
                FunctionCallOutputContentItem::InputImage {
                    image_url,
                    detail: None,
                }
            }
            Ok(McpContent::Unknown) | Err(_) => FunctionCallOutputContentItem::InputText {
                text: serde_json::to_string(content).unwrap_or_else(|_| "<content>".to_string()),
            },
        };
        items.push(item);
    }

    if saw_image { Some(items) } else { None }
}

// Implement Display so callers can treat the payload like a plain string when logging or doing
// trivial substring checks in tests (existing tests call `.contains()` on the output). For
// `ContentItems`, Display emits a JSON representation.

impl std::fmt::Display for FunctionCallOutputPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.body {
            FunctionCallOutputBody::Text(content) => f.write_str(content),
            FunctionCallOutputBody::ContentItems(items) => {
                let content = serde_json::to_string(items).unwrap_or_default();
                f.write_str(content.as_str())
            }
        }
    }
}

// (Moved event mapping logic into codex-core to avoid coupling protocol to UI-facing events.)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_types::SandboxMode;
    use crate::protocol::AskForApproval;
    use crate::protocol::GranularApprovalConfig;
    use anyhow::Result;
    use codex_execpolicy::Policy;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn sandbox_permissions_helpers_match_documented_semantics() {
        let cases = [
            (SandboxPermissions::UseDefault, false, false, false),
            (SandboxPermissions::RequireEscalated, true, true, false),
            (
                SandboxPermissions::WithAdditionalPermissions,
                false,
                true,
                true,
            ),
        ];

        for (
            sandbox_permissions,
            requires_escalated_permissions,
            requests_sandbox_override,
            uses_additional_permissions,
        ) in cases
        {
            assert_eq!(
                sandbox_permissions.requires_escalated_permissions(),
                requires_escalated_permissions
            );
            assert_eq!(
                sandbox_permissions.requests_sandbox_override(),
                requests_sandbox_override
            );
            assert_eq!(
                sandbox_permissions.uses_additional_permissions(),
                uses_additional_permissions
            );
        }
    }

    #[test]
    fn convert_mcp_content_to_items_preserves_data_urls() {
        let contents = vec![serde_json::json!({
            "type": "image",
            "data": "data:image/png;base64,Zm9v",
            "mimeType": "image/png",
        })];

        let items = convert_mcp_content_to_items(&contents).expect("expected image items");
        assert_eq!(
            items,
            vec![FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,Zm9v".to_string(),
                detail: None,
            }]
        );
    }

    #[test]
    fn response_item_parses_image_generation_call() {
        let item = serde_json::from_value::<ResponseItem>(serde_json::json!({
            "id": "ig_123",
            "type": "image_generation_call",
            "status": "completed",
            "revised_prompt": "A small blue square",
            "result": "Zm9v",
        }))
        .expect("image generation item should deserialize");

        assert_eq!(
            item,
            ResponseItem::ImageGenerationCall {
                id: "ig_123".to_string(),
                status: "completed".to_string(),
                revised_prompt: Some("A small blue square".to_string()),
                result: "Zm9v".to_string(),
            }
        );
    }

    #[test]
    fn response_item_parses_image_generation_call_without_revised_prompt() {
        let item = serde_json::from_value::<ResponseItem>(serde_json::json!({
            "id": "ig_123",
            "type": "image_generation_call",
            "status": "completed",
            "result": "Zm9v",
        }))
        .expect("image generation item should deserialize");

        assert_eq!(
            item,
            ResponseItem::ImageGenerationCall {
                id: "ig_123".to_string(),
                status: "completed".to_string(),
                revised_prompt: None,
                result: "Zm9v".to_string(),
            }
        );
    }

    #[test]
    fn permission_profile_is_empty_when_all_fields_are_none() {
        assert_eq!(PermissionProfile::default().is_empty(), true);
    }

    #[test]
    fn permission_profile_is_not_empty_when_field_is_present_but_nested_empty() {
        let permission_profile = PermissionProfile {
            network: Some(NetworkPermissions { enabled: None }),
            file_system: None,
        };
        assert_eq!(permission_profile.is_empty(), false);
    }

    #[test]
    fn convert_mcp_content_to_items_builds_data_urls_when_missing_prefix() {
        let contents = vec![serde_json::json!({
            "type": "image",
            "data": "Zm9v",
            "mimeType": "image/png",
        })];

        let items = convert_mcp_content_to_items(&contents).expect("expected image items");
        assert_eq!(
            items,
            vec![FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,Zm9v".to_string(),
                detail: None,
            }]
        );
    }

    #[test]
    fn convert_mcp_content_to_items_returns_none_without_images() {
        let contents = vec![serde_json::json!({
            "type": "text",
            "text": "hello",
        })];

        assert_eq!(convert_mcp_content_to_items(&contents), None);
    }

    #[test]
    fn function_call_output_content_items_to_text_joins_text_segments() {
        let content_items = vec![
            FunctionCallOutputContentItem::InputText {
                text: "line 1".to_string(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: None,
            },
            FunctionCallOutputContentItem::InputText {
                text: "line 2".to_string(),
            },
        ];

        let text = function_call_output_content_items_to_text(&content_items);
        assert_eq!(text, Some("line 1\nline 2".to_string()));
    }

    #[test]
    fn function_call_output_content_items_to_text_ignores_blank_text_and_images() {
        let content_items = vec![
            FunctionCallOutputContentItem::InputText {
                text: "   ".to_string(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: None,
            },
        ];

        let text = function_call_output_content_items_to_text(&content_items);
        assert_eq!(text, None);
    }

    #[test]
    fn function_call_output_body_to_text_returns_plain_text_content() {
        let body = FunctionCallOutputBody::Text("ok".to_string());
        let text = body.to_text();
        assert_eq!(text, Some("ok".to_string()));
    }

    #[test]
    fn function_call_output_body_to_text_uses_content_item_fallback() {
        let body = FunctionCallOutputBody::ContentItems(vec![
            FunctionCallOutputContentItem::InputText {
                text: "line 1".to_string(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: None,
            },
        ]);

        let text = body.to_text();
        assert_eq!(text, Some("line 1".to_string()));
    }

    #[test]
    fn function_call_deserializes_optional_namespace() {
        let item: ResponseItem = serde_json::from_value(serde_json::json!({
            "type": "function_call",
            "name": "mcp__codex_apps__gmail_get_recent_emails",
            "namespace": "mcp__codex_apps__gmail",
            "arguments": "{\"top_k\":5}",
            "call_id": "call-1",
        }))
        .expect("function_call should deserialize");

        assert_eq!(
            item,
            ResponseItem::FunctionCall {
                id: None,
                name: "mcp__codex_apps__gmail_get_recent_emails".to_string(),
                namespace: Some("mcp__codex_apps__gmail".to_string()),
                arguments: "{\"top_k\":5}".to_string(),
                call_id: "call-1".to_string(),
            }
        );
    }

    #[test]
    fn converts_sandbox_mode_into_developer_instructions() {
        let workspace_write: DeveloperInstructions = SandboxMode::WorkspaceWrite.into();
        assert_eq!(
            workspace_write,
            DeveloperInstructions::new(
                "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `workspace-write`: The sandbox permits reading files, and editing files in `cwd` and `writable_roots`. Editing files in other directories requires approval. Network access is restricted."
            )
        );

        let read_only: DeveloperInstructions = SandboxMode::ReadOnly.into();
        assert_eq!(
            read_only,
            DeveloperInstructions::new(
                "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `read-only`: The sandbox only permits reading files. Network access is restricted."
            )
        );

        let danger_full_access: DeveloperInstructions = SandboxMode::DangerFullAccess.into();
        assert_eq!(
            danger_full_access,
            DeveloperInstructions::new(
                "Filesystem sandboxing defines which files can be read or written. `sandbox_mode` is `danger-full-access`: No filesystem sandboxing - all commands are permitted. Network access is enabled."
            )
        );
    }

    #[test]
    fn builds_permissions_with_network_access_override() {
        let instructions = DeveloperInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: false,
                request_permissions_tool_enabled: false,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.into_text();
        assert!(
            text.contains("Network access is enabled."),
            "expected network access to be enabled in message"
        );
        assert!(
            text.contains("How to request escalation"),
            "expected approval guidance to be included"
        );
    }

    #[test]
    fn builds_permissions_from_policy() {
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: true,
            allow_limited_git_writes: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        let instructions = DeveloperInstructions::from_policy(
            &policy,
            AskForApproval::UnlessTrusted,
            ApprovalsReviewer::User,
            &Policy::empty(),
            &PathBuf::from("/tmp"),
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        );
        let text = instructions.into_text();
        assert!(text.contains("Network access is enabled."));
        assert!(text.contains("`approval_policy` is `unless-trusted`"));
    }

    #[test]
    fn includes_request_rule_instructions_for_on_request() {
        let mut exec_policy = Policy::empty();
        exec_policy
            .add_prefix_rule(
                &["git".to_string(), "pull".to_string()],
                codex_execpolicy::Decision::Allow,
            )
            .expect("add rule");
        let instructions = DeveloperInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &exec_policy,
                exec_permission_approvals_enabled: false,
                request_permissions_tool_enabled: false,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.into_text();
        assert!(text.contains("prefix_rule"));
        assert!(text.contains("Approved command prefixes"));
        assert!(text.contains(r#"["git", "pull"]"#));
    }

    #[test]
    fn includes_request_permissions_tool_instructions_for_unless_trusted_when_enabled() {
        let instructions = DeveloperInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::UnlessTrusted,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: false,
                request_permissions_tool_enabled: true,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.into_text();
        assert!(text.contains("`approval_policy` is `unless-trusted`"));
        assert!(text.contains("# request_permissions Tool"));
    }

    #[test]
    fn includes_request_permissions_tool_instructions_for_on_failure_when_enabled() {
        let instructions = DeveloperInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnFailure,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: false,
                request_permissions_tool_enabled: true,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.into_text();
        assert!(text.contains("`approval_policy` is `on-failure`"));
        assert!(text.contains("# request_permissions Tool"));
    }

    #[test]
    fn includes_request_permission_rule_instructions_for_on_request_when_enabled() {
        let instructions = DeveloperInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: true,
                request_permissions_tool_enabled: false,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.into_text();
        assert!(text.contains("with_additional_permissions"));
        assert!(text.contains("additional_permissions"));
    }

    #[test]
    fn includes_request_permissions_tool_instructions_for_on_request_when_tool_is_enabled() {
        let instructions = DeveloperInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: false,
                request_permissions_tool_enabled: true,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.into_text();
        assert!(text.contains("# request_permissions Tool"));
        assert!(
            text.contains("The built-in `request_permissions` tool is available in this session.")
        );
    }

    #[test]
    fn on_request_includes_tool_guidance_alongside_inline_permission_guidance_when_both_exist() {
        let instructions = DeveloperInstructions::from_permissions_with_network(
            SandboxMode::WorkspaceWrite,
            NetworkAccess::Enabled,
            PermissionsPromptConfig {
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::User,
                exec_policy: &Policy::empty(),
                exec_permission_approvals_enabled: true,
                request_permissions_tool_enabled: true,
            },
            /*writable_roots*/ None,
        );

        let text = instructions.into_text();
        assert!(text.contains("with_additional_permissions"));
        assert!(text.contains("# request_permissions Tool"));
    }

    #[test]
    fn guardian_subagent_approvals_append_guardian_specific_guidance() {
        let text = DeveloperInstructions::from(
            AskForApproval::OnRequest,
            ApprovalsReviewer::GuardianSubagent,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        )
        .into_text();

        assert!(text.contains("`approvals_reviewer` is `guardian_subagent`"));
        assert!(text.contains("materially safer alternative"));
    }

    #[test]
    fn guardian_subagent_approvals_omit_guardian_specific_guidance_when_approval_is_never() {
        let text = DeveloperInstructions::from(
            AskForApproval::Never,
            ApprovalsReviewer::GuardianSubagent,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        )
        .into_text();

        assert!(!text.contains("`approvals_reviewer` is `guardian_subagent`"));
    }

    fn granular_categories_section(title: &str, categories: &[&str]) -> String {
        format!("{title}\n{}", categories.join("\n"))
    }

    fn granular_prompt_expected(
        prompted_categories: &[&str],
        rejected_categories: &[&str],
        include_shell_permission_request_instructions: bool,
        include_request_permissions_tool_section: bool,
    ) -> String {
        let mut sections = vec![granular_prompt_intro_text().to_string()];
        if !prompted_categories.is_empty() {
            sections.push(granular_categories_section(
                "These approval categories may still prompt the user when needed:",
                prompted_categories,
            ));
        }
        if !rejected_categories.is_empty() {
            sections.push(granular_categories_section(
                "These approval categories are automatically rejected instead of prompting the user:",
                rejected_categories,
            ));
        }
        if include_shell_permission_request_instructions {
            sections.push(APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION.to_string());
        }
        if include_request_permissions_tool_section {
            sections.push(request_permissions_tool_prompt_section().to_string());
        }
        sections.join("\n\n")
    }

    #[test]
    fn granular_policy_lists_prompted_and_rejected_categories_separately() {
        let text = DeveloperInstructions::from(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: true,
                skill_approval: false,
                request_permissions: true,
                mcp_elicitations: false,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ false,
        )
        .into_text();

        assert_eq!(
            text,
            [
                granular_prompt_intro_text().to_string(),
                granular_categories_section(
                    "These approval categories may still prompt the user when needed:",
                    &["- `rules`"],
                ),
                granular_categories_section(
                    "These approval categories are automatically rejected instead of prompting the user:",
                    &["- `sandbox_approval`", "- `skill_approval`", "- `mcp_elicitations`",],
                ),
            ]
            .join("\n\n")
        );
    }

    #[test]
    fn granular_policy_includes_command_permission_instructions_when_sandbox_approval_can_prompt() {
        let text = DeveloperInstructions::from(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ false,
        )
        .into_text();

        assert_eq!(
            text,
            granular_prompt_expected(
                &[
                    "- `sandbox_approval`",
                    "- `rules`",
                    "- `skill_approval`",
                    "- `mcp_elicitations`",
                ],
                &[],
                /*include_shell_permission_request_instructions*/ true,
                /*include_request_permissions_tool_section*/ false,
            )
        );
    }

    #[test]
    fn granular_policy_omits_shell_permission_instructions_when_inline_requests_are_disabled() {
        let text = DeveloperInstructions::from(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ false,
            /*request_permissions_tool_enabled*/ false,
        )
        .into_text();

        assert_eq!(
            text,
            granular_prompt_expected(
                &[
                    "- `sandbox_approval`",
                    "- `rules`",
                    "- `skill_approval`",
                    "- `mcp_elicitations`",
                ],
                &[],
                /*include_shell_permission_request_instructions*/ false,
                /*include_request_permissions_tool_section*/ false,
            )
        );
    }

    #[test]
    fn granular_policy_includes_request_permissions_tool_only_when_that_prompt_can_still_fire() {
        let allowed = DeveloperInstructions::from(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ true,
        )
        .into_text();
        assert!(allowed.contains("# request_permissions Tool"));

        let rejected = DeveloperInstructions::from(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: false,
                mcp_elicitations: true,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ true,
        )
        .into_text();
        assert!(!rejected.contains("# request_permissions Tool"));
    }

    #[test]
    fn granular_policy_lists_request_permissions_category_without_tool_section_when_tool_is_unavailable()
     {
        let text = DeveloperInstructions::from(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: false,
                skill_approval: false,
                request_permissions: true,
                mcp_elicitations: false,
            }),
            ApprovalsReviewer::User,
            &Policy::empty(),
            /*exec_permission_approvals_enabled*/ true,
            /*request_permissions_tool_enabled*/ false,
        )
        .into_text();

        assert!(!text.contains("- `request_permissions`"));
        assert!(!text.contains("# request_permissions Tool"));
    }

    #[test]
    fn render_command_prefix_list_sorts_by_len_then_total_len_then_alphabetical() {
        let prefixes = vec![
            vec!["b".to_string(), "zz".to_string()],
            vec!["aa".to_string()],
            vec!["b".to_string()],
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec!["a".to_string()],
            vec!["b".to_string(), "a".to_string()],
        ];

        let output = format_allow_prefixes(prefixes).expect("rendered list");
        assert_eq!(
            output,
            r#"- ["a"]
- ["b"]
- ["aa"]
- ["b", "a"]
- ["b", "zz"]
- ["a", "b", "c"]"#
                .to_string(),
        );
    }

    #[test]
    fn render_command_prefix_list_limits_output_to_max_prefixes() {
        let prefixes = (0..(MAX_RENDERED_PREFIXES + 5))
            .map(|i| vec![format!("{i:03}")])
            .collect::<Vec<_>>();

        let output = format_allow_prefixes(prefixes).expect("rendered list");
        assert_eq!(output.ends_with(TRUNCATED_MARKER), true);
        eprintln!("output: {output}");
        assert_eq!(output.lines().count(), MAX_RENDERED_PREFIXES + 1);
    }

    #[test]
    fn format_allow_prefixes_limits_output() {
        let mut exec_policy = Policy::empty();
        for i in 0..200 {
            exec_policy
                .add_prefix_rule(
                    &[format!("tool-{i:03}"), "x".repeat(500)],
                    codex_execpolicy::Decision::Allow,
                )
                .expect("add rule");
        }

        let output =
            format_allow_prefixes(exec_policy.get_allowed_prefixes()).expect("formatted prefixes");
        assert!(
            output.len() <= MAX_ALLOW_PREFIX_TEXT_BYTES + TRUNCATED_MARKER.len(),
            "output length exceeds expected limit: {output}",
        );
    }

    #[test]
    fn serializes_success_as_plain_string() -> Result<()> {
        let item = ResponseInputItem::FunctionCallOutput {
            call_id: "call1".into(),
            output: FunctionCallOutputPayload::from_text("ok".into()),
        };

        let json = serde_json::to_string(&item)?;
        let v: serde_json::Value = serde_json::from_str(&json)?;

        // Success case -> output should be a plain string
        assert_eq!(v.get("output").unwrap().as_str().unwrap(), "ok");
        Ok(())
    }

    #[test]
    fn serializes_failure_as_string() -> Result<()> {
        let item = ResponseInputItem::FunctionCallOutput {
            call_id: "call1".into(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text("bad".into()),
                success: Some(false),
            },
        };

        let json = serde_json::to_string(&item)?;
        let v: serde_json::Value = serde_json::from_str(&json)?;

        assert_eq!(v.get("output").unwrap().as_str().unwrap(), "bad");
        Ok(())
    }

    #[test]
    fn serializes_image_outputs_as_array() -> Result<()> {
        let call_tool_result = CallToolResult {
            content: vec![
                serde_json::json!({"type":"text","text":"caption"}),
                serde_json::json!({"type":"image","data":"BASE64","mimeType":"image/png"}),
            ],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let payload = call_tool_result.into_function_call_output_payload();
        assert_eq!(payload.success, Some(true));
        let Some(items) = payload.content_items() else {
            panic!("expected content items");
        };
        let items = items.to_vec();
        assert_eq!(
            items,
            vec![
                FunctionCallOutputContentItem::InputText {
                    text: "caption".into(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,BASE64".into(),
                    detail: None,
                },
            ]
        );

        let item = ResponseInputItem::FunctionCallOutput {
            call_id: "call1".into(),
            output: payload,
        };

        let json = serde_json::to_string(&item)?;
        let v: serde_json::Value = serde_json::from_str(&json)?;

        let output = v.get("output").expect("output field");
        assert!(output.is_array(), "expected array output");

        Ok(())
    }

    #[test]
    fn serializes_custom_tool_image_outputs_as_array() -> Result<()> {
        let item = ResponseInputItem::CustomToolCallOutput {
            call_id: "call1".into(),
            name: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,BASE64".into(),
                    detail: None,
                },
            ]),
        };

        let json = serde_json::to_string(&item)?;
        let v: serde_json::Value = serde_json::from_str(&json)?;

        let output = v.get("output").expect("output field");
        assert!(output.is_array(), "expected array output");

        Ok(())
    }

    #[test]
    fn preserves_existing_image_data_urls() -> Result<()> {
        let call_tool_result = CallToolResult {
            content: vec![serde_json::json!({
                "type": "image",
                "data": "data:image/png;base64,BASE64",
                "mimeType": "image/png"
            })],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let payload = call_tool_result.into_function_call_output_payload();
        let Some(items) = payload.content_items() else {
            panic!("expected content items");
        };
        let items = items.to_vec();
        assert_eq!(
            items,
            vec![FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,BASE64".into(),
                detail: None,
            }]
        );

        Ok(())
    }

    #[test]
    fn deserializes_array_payload_into_items() -> Result<()> {
        let json = r#"[
            {"type": "input_text", "text": "note"},
            {"type": "input_image", "image_url": "data:image/png;base64,XYZ"}
        ]"#;

        let payload: FunctionCallOutputPayload = serde_json::from_str(json)?;

        assert_eq!(payload.success, None);
        let expected_items = vec![
            FunctionCallOutputContentItem::InputText {
                text: "note".into(),
            },
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,XYZ".into(),
                detail: None,
            },
        ];
        assert_eq!(
            payload.body,
            FunctionCallOutputBody::ContentItems(expected_items.clone())
        );
        assert_eq!(
            serde_json::to_string(&payload)?,
            serde_json::to_string(&expected_items)?
        );

        Ok(())
    }

    #[test]
    fn deserializes_compaction_alias() -> Result<()> {
        let json = r#"{"type":"compaction_summary","encrypted_content":"abc"}"#;

        let item: ResponseItem = serde_json::from_str(json)?;

        assert_eq!(
            item,
            ResponseItem::Compaction {
                encrypted_content: "abc".into(),
            }
        );
        Ok(())
    }

    #[test]
    fn roundtrips_web_search_call_actions() -> Result<()> {
        let cases = vec![
            (
                r#"{
                    "type": "web_search_call",
                    "status": "completed",
                    "action": {
                        "type": "search",
                        "query": "weather seattle",
                        "queries": ["weather seattle", "seattle weather now"]
                    }
                }"#,
                None,
                Some(WebSearchAction::Search {
                    query: Some("weather seattle".into()),
                    queries: Some(vec!["weather seattle".into(), "seattle weather now".into()]),
                }),
                Some("completed".into()),
                true,
            ),
            (
                r#"{
                    "type": "web_search_call",
                    "status": "open",
                    "action": {
                        "type": "open_page",
                        "url": "https://example.com"
                    }
                }"#,
                None,
                Some(WebSearchAction::OpenPage {
                    url: Some("https://example.com".into()),
                }),
                Some("open".into()),
                true,
            ),
            (
                r#"{
                    "type": "web_search_call",
                    "status": "in_progress",
                    "action": {
                        "type": "find_in_page",
                        "url": "https://example.com/docs",
                        "pattern": "installation"
                    }
                }"#,
                None,
                Some(WebSearchAction::FindInPage {
                    url: Some("https://example.com/docs".into()),
                    pattern: Some("installation".into()),
                }),
                Some("in_progress".into()),
                true,
            ),
            (
                r#"{
                    "type": "web_search_call",
                    "status": "in_progress",
                    "id": "ws_partial"
                }"#,
                Some("ws_partial".into()),
                None,
                Some("in_progress".into()),
                false,
            ),
        ];

        for (json_literal, expected_id, expected_action, expected_status, expect_roundtrip) in cases
        {
            let parsed: ResponseItem = serde_json::from_str(json_literal)?;
            let expected = ResponseItem::WebSearchCall {
                id: expected_id.clone(),
                status: expected_status.clone(),
                action: expected_action.clone(),
            };
            assert_eq!(parsed, expected);

            let serialized = serde_json::to_value(&parsed)?;
            let mut expected_serialized: serde_json::Value = serde_json::from_str(json_literal)?;
            if !expect_roundtrip && let Some(obj) = expected_serialized.as_object_mut() {
                obj.remove("id");
            }
            assert_eq!(serialized, expected_serialized);
        }

        Ok(())
    }

    #[test]
    fn deserialize_shell_tool_call_params() -> Result<()> {
        let json = r#"{
            "command": ["ls", "-l"],
            "workdir": "/tmp",
            "timeout": 1000
        }"#;

        let params: ShellToolCallParams = serde_json::from_str(json)?;
        assert_eq!(
            ShellToolCallParams {
                command: vec!["ls".to_string(), "-l".to_string()],
                workdir: Some("/tmp".to_string()),
                timeout_ms: Some(1000),
                sandbox_permissions: None,
                prefix_rule: None,
                additional_permissions: None,
                justification: None,
            },
            params
        );
        Ok(())
    }

    #[test]
    fn wraps_image_user_input_with_tags() -> Result<()> {
        let image_url = "data:image/png;base64,abc".to_string();

        let item = ResponseInputItem::from(vec![UserInput::Image {
            image_url: image_url.clone(),
        }]);

        match item {
            ResponseInputItem::Message { content, .. } => {
                let expected = vec![
                    ContentItem::InputText {
                        text: image_open_tag_text(),
                    },
                    ContentItem::InputImage { image_url },
                    ContentItem::InputText {
                        text: image_close_tag_text(),
                    },
                ];
                assert_eq!(content, expected);
            }
            other => panic!("expected message response but got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn tool_search_call_roundtrips() -> Result<()> {
        let parsed: ResponseItem = serde_json::from_str(
            r#"{
                "type": "tool_search_call",
                "call_id": "search-1",
                "execution": "client",
                "arguments": {
                    "query": "calendar create",
                    "limit": 1
                }
            }"#,
        )?;

        assert_eq!(
            parsed,
            ResponseItem::ToolSearchCall {
                id: None,
                call_id: Some("search-1".to_string()),
                status: None,
                execution: "client".to_string(),
                arguments: serde_json::json!({
                    "query": "calendar create",
                    "limit": 1,
                }),
            }
        );

        assert_eq!(
            serde_json::to_value(&parsed)?,
            serde_json::json!({
                "type": "tool_search_call",
                "call_id": "search-1",
                "execution": "client",
                "arguments": {
                    "query": "calendar create",
                    "limit": 1,
                }
            })
        );

        Ok(())
    }

    #[test]
    fn tool_search_output_roundtrips() -> Result<()> {
        let input = ResponseInputItem::ToolSearchOutput {
            call_id: "search-1".to_string(),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: vec![serde_json::json!({
                "type": "function",
                "name": "mcp__codex_apps__calendar_create_event",
                "description": "Create a calendar event.",
                "defer_loading": true,
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"}
                    },
                    "required": ["title"],
                    "additionalProperties": false,
                }
            })],
        };
        assert_eq!(
            ResponseItem::from(input.clone()),
            ResponseItem::ToolSearchOutput {
                call_id: Some("search-1".to_string()),
                status: "completed".to_string(),
                execution: "client".to_string(),
                tools: vec![serde_json::json!({
                    "type": "function",
                    "name": "mcp__codex_apps__calendar_create_event",
                    "description": "Create a calendar event.",
                    "defer_loading": true,
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "title": {"type": "string"}
                        },
                        "required": ["title"],
                        "additionalProperties": false,
                    }
                })],
            }
        );

        assert_eq!(
            serde_json::to_value(input)?,
            serde_json::json!({
                "type": "tool_search_output",
                "call_id": "search-1",
                "status": "completed",
                "execution": "client",
                "tools": [{
                    "type": "function",
                    "name": "mcp__codex_apps__calendar_create_event",
                    "description": "Create a calendar event.",
                    "defer_loading": true,
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "title": {"type": "string"}
                        },
                        "required": ["title"],
                        "additionalProperties": false,
                    }
                }]
            })
        );

        Ok(())
    }

    #[test]
    fn tool_search_server_items_allow_null_call_id() -> Result<()> {
        let parsed_call: ResponseItem = serde_json::from_str(
            r#"{
                "type": "tool_search_call",
                "execution": "server",
                "call_id": null,
                "status": "completed",
                "arguments": {
                    "paths": ["crm"]
                }
            }"#,
        )?;
        assert_eq!(
            parsed_call,
            ResponseItem::ToolSearchCall {
                id: None,
                call_id: None,
                status: Some("completed".to_string()),
                execution: "server".to_string(),
                arguments: serde_json::json!({
                    "paths": ["crm"],
                }),
            }
        );

        let parsed_output: ResponseItem = serde_json::from_str(
            r#"{
                "type": "tool_search_output",
                "execution": "server",
                "call_id": null,
                "status": "completed",
                "tools": []
            }"#,
        )?;
        assert_eq!(
            parsed_output,
            ResponseItem::ToolSearchOutput {
                call_id: None,
                status: "completed".to_string(),
                execution: "server".to_string(),
                tools: vec![],
            }
        );

        Ok(())
    }

    #[test]
    fn mixed_remote_and_local_images_share_label_sequence() -> Result<()> {
        let image_url = "data:image/png;base64,abc".to_string();
        let dir = tempdir()?;
        let local_path = dir.path().join("local.png");
        // A tiny valid PNG (1x1) so this test doesn't depend on cross-crate file paths, which
        // break under Bazel sandboxing.
        const TINY_PNG_BYTES: &[u8] = &[
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2,
            0, 0, 5, 0, 1, 122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ];
        std::fs::write(&local_path, TINY_PNG_BYTES)?;

        let item = ResponseInputItem::from(vec![
            UserInput::Image {
                image_url: image_url.clone(),
            },
            UserInput::LocalImage { path: local_path },
        ]);

        match item {
            ResponseInputItem::Message { content, .. } => {
                assert_eq!(
                    content.first(),
                    Some(&ContentItem::InputText {
                        text: image_open_tag_text(),
                    })
                );
                assert_eq!(content.get(1), Some(&ContentItem::InputImage { image_url }));
                assert_eq!(
                    content.get(2),
                    Some(&ContentItem::InputText {
                        text: image_close_tag_text(),
                    })
                );
                assert_eq!(
                    content.get(3),
                    Some(&ContentItem::InputText {
                        text: local_image_open_tag_text(/*label_number*/ 2),
                    })
                );
                assert!(matches!(
                    content.get(4),
                    Some(ContentItem::InputImage { .. })
                ));
                assert_eq!(
                    content.get(5),
                    Some(&ContentItem::InputText {
                        text: image_close_tag_text(),
                    })
                );
            }
            other => panic!("expected message response but got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn local_image_read_error_adds_placeholder() -> Result<()> {
        let dir = tempdir()?;
        let missing_path = dir.path().join("missing-image.png");

        let item = ResponseInputItem::from(vec![UserInput::LocalImage {
            path: missing_path.clone(),
        }]);

        match item {
            ResponseInputItem::Message { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    ContentItem::InputText { text } => {
                        let display_path = missing_path.display().to_string();
                        assert!(
                            text.contains(&display_path),
                            "placeholder should mention missing path: {text}"
                        );
                        assert!(
                            text.contains("could not read"),
                            "placeholder should mention read issue: {text}"
                        );
                    }
                    other => panic!("expected placeholder text but found {other:?}"),
                }
            }
            other => panic!("expected message response but got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn local_image_non_image_adds_placeholder() -> Result<()> {
        let dir = tempdir()?;
        let json_path = dir.path().join("example.json");
        std::fs::write(&json_path, br#"{"hello":"world"}"#)?;

        let item = ResponseInputItem::from(vec![UserInput::LocalImage {
            path: json_path.clone(),
        }]);

        match item {
            ResponseInputItem::Message { content, .. } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    ContentItem::InputText { text } => {
                        assert!(
                            text.contains("unsupported image `application/json`"),
                            "placeholder should mention unsupported image MIME: {text}"
                        );
                        assert!(
                            text.contains(&json_path.display().to_string()),
                            "placeholder should mention path: {text}"
                        );
                    }
                    other => panic!("expected placeholder text but found {other:?}"),
                }
            }
            other => panic!("expected message response but got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn local_image_unsupported_image_format_adds_placeholder() -> Result<()> {
        let dir = tempdir()?;
        let svg_path = dir.path().join("example.svg");
        std::fs::write(
            &svg_path,
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>"#,
        )?;

        let item = ResponseInputItem::from(vec![UserInput::LocalImage {
            path: svg_path.clone(),
        }]);

        match item {
            ResponseInputItem::Message { content, .. } => {
                assert_eq!(content.len(), 1);
                let expected = format!(
                    "Codex cannot attach image at `{}`: unsupported image `image/svg+xml`.",
                    svg_path.display()
                );
                match &content[0] {
                    ContentItem::InputText { text } => assert_eq!(text, &expected),
                    other => panic!("expected placeholder text but found {other:?}"),
                }
            }
            other => panic!("expected message response but got {other:?}"),
        }

        Ok(())
    }
}
