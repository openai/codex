use std::path::PathBuf;

use codex_protocol::approvals::ElicitationAction;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::mcp::RequestId as McpRequestId;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// TUI-side agent commands routed over the in-process app-server.
///
/// This replaces direct `codex_protocol::protocol::Op` usage in the TUI.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum AgentCommand {
    Interrupt,
    CleanBackgroundTerminals,
    UserTurn {
        items: Vec<UserInput>,
        cwd: PathBuf,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
        model: String,
        effort: Option<ReasoningEffortConfig>,
        summary: ReasoningSummaryConfig,
        final_output_json_schema: Option<Value>,
        collaboration_mode: Option<CollaborationMode>,
        personality: Option<Personality>,
    },
    OverrideTurnContext {
        cwd: Option<PathBuf>,
        approval_policy: Option<AskForApproval>,
        sandbox_policy: Option<SandboxPolicy>,
        windows_sandbox_level: Option<WindowsSandboxLevel>,
        model: Option<String>,
        effort: Option<Option<ReasoningEffortConfig>>,
        summary: Option<ReasoningSummaryConfig>,
        collaboration_mode: Option<CollaborationMode>,
        personality: Option<Personality>,
    },
    ExecApproval {
        id: String,
        turn_id: Option<String>,
        decision: ReviewDecision,
    },
    PatchApproval {
        id: String,
        decision: ReviewDecision,
    },
    ResolveElicitation {
        server_name: String,
        request_id: McpRequestId,
        decision: ElicitationAction,
    },
    UserInputAnswer {
        id: String,
        response: RequestUserInputResponse,
    },
    DynamicToolResponse {
        id: String,
        response: DynamicToolResponse,
    },
    AddToHistory {
        text: String,
    },
    GetHistoryEntryRequest {
        offset: usize,
        log_id: u64,
    },
    ListMcpTools,
    ReloadUserConfig,
    ListCustomPrompts,
    ListSkills {
        cwds: Vec<PathBuf>,
        force_reload: bool,
    },
    Compact,
    DropMemories,
    UpdateMemories,
    SetThreadName {
        name: String,
    },
    Undo,
    ThreadRollback {
        num_turns: u32,
    },
    Review {
        review_request: ReviewRequest,
    },
    Shutdown,
    RunUserShellCommand {
        command: String,
    },
}
