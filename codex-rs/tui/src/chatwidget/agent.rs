use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_app_server_client::ClientSurface;
use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessClientStartArgs;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::ApplyPatchApprovalResponse;
use codex_app_server_protocol::ChatgptAuthTokensRefreshResponse;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallResponse;
use codex_app_server_protocol::ExecCommandApprovalResponse;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::FileChangeRequestApprovalResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::McpServerRefreshResponse;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SkillsListResponse;
use codex_app_server_protocol::SkillsRemoteReadResponse;
use codex_app_server_protocol::SkillsRemoteWriteResponse;
use codex_app_server_protocol::ThreadBackgroundTerminalsCleanResponse;
use codex_app_server_protocol::ThreadCompactStartResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadRealtimeAppendAudioResponse;
use codex_app_server_protocol::ThreadRealtimeAppendTextResponse;
use codex_app_server_protocol::ThreadRealtimeStartResponse;
use codex_app_server_protocol::ThreadRealtimeStopResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadRollbackResponse;
use codex_app_server_protocol::ThreadSetNameResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ToolRequestUserInputAnswer;
use codex_app_server_protocol::ToolRequestUserInputResponse;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::auth::AuthManager;
use codex_core::config::Config;
use codex_core::config::types::HistoryPersistence;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::LoaderOverrides;
use codex_feedback::CodexFeedback;
use codex_protocol::ThreadId;
use codex_protocol::account::PlanType as AccountPlanType;
use codex_protocol::approvals::ApplyPatchApprovalRequestEvent;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::approvals::ExecApprovalRequestEvent;
use codex_protocol::dynamic_tools::DynamicToolCallRequest;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::MacOsPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::GetHistoryEntryResponseEvent;
use codex_protocol::protocol::ListCustomPromptsResponseEvent;
use codex_protocol::protocol::ListRemoteSkillsResponseEvent;
use codex_protocol::protocol::ListSkillsResponseEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RemoteSkillDownloadedEvent;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::WarningEvent;
use codex_protocol::request_user_input::RequestUserInputEvent;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::version::CODEX_CLI_VERSION;

const TUI_NOTIFY_CLIENT: &str = "codex-tui";
const HISTORY_FILENAME: &str = "history.jsonl";
const HISTORY_SOFT_CAP_RATIO: f64 = 0.8;
const HISTORY_LOCK_MAX_RETRIES: usize = 10;
const HISTORY_LOCK_RETRY_SLEEP: Duration = Duration::from_millis(100);

async fn initialize_app_server_client_name(thread: &CodexThread) {
    if let Err(err) = thread
        .set_app_server_client_name(Some(TUI_NOTIFY_CLIENT.to_string()))
        .await
    {
        tracing::error!("failed to set app server client name: {err}");
    }
}

fn in_process_start_args(config: &Config) -> InProcessClientStartArgs {
    let config_warnings: Vec<ConfigWarningNotification> = config
        .startup_warnings
        .iter()
        .map(|warning| ConfigWarningNotification {
            summary: warning.clone(),
            details: None,
            path: None,
            range: None,
        })
        .collect();

    InProcessClientStartArgs {
        arg0_paths: codex_arg0::Arg0DispatchPaths::default(),
        config: Arc::new(config.clone()),
        cli_overrides: Vec::new(),
        loader_overrides: LoaderOverrides::default(),
        cloud_requirements: CloudRequirementsLoader::default(),
        feedback: CodexFeedback::new(),
        config_warnings,
        surface: ClientSurface::Tui,
        client_name: Some(TUI_NOTIFY_CLIENT.to_string()),
        client_version: CODEX_CLI_VERSION.to_string(),
        experimental_api: true,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    }
}

struct RequestIdSequencer {
    next: i64,
}

impl RequestIdSequencer {
    fn new() -> Self {
        Self { next: 1 }
    }

    fn next(&mut self) -> RequestId {
        let id = self.next;
        self.next += 1;
        RequestId::Integer(id)
    }
}

#[derive(Debug, Clone)]
enum PendingExecApprovalRequest {
    V1(RequestId),
    V2(RequestId),
}

#[derive(Debug, Clone)]
enum PendingPatchApprovalRequest {
    V1(RequestId),
    V2(RequestId),
}

#[derive(Default)]
struct PendingServerRequests {
    exec_approvals: HashMap<String, PendingExecApprovalRequest>,
    patch_approvals: HashMap<String, PendingPatchApprovalRequest>,
    mcp_elicitations: HashMap<RequestId, (String, codex_protocol::mcp::RequestId)>,
    request_user_input: HashMap<String, VecDeque<RequestId>>,
    dynamic_tool_calls: HashMap<String, RequestId>,
    pending_file_changes: HashMap<String, HashMap<PathBuf, FileChange>>,
}

impl PendingServerRequests {
    fn clear_turn_scoped(&mut self) {
        self.exec_approvals.clear();
        self.patch_approvals.clear();
        // MCP elicitation requests can outlive turn boundaries (turn_id is best-effort),
        // so clear them only via resolve path or serverRequest/resolved notifications.
        self.request_user_input.clear();
        self.dynamic_tool_calls.clear();
        self.pending_file_changes.clear();
    }

    fn register_request_user_input(&mut self, turn_id: String, request_id: RequestId) {
        self.request_user_input
            .entry(turn_id)
            .or_default()
            .push_back(request_id);
    }

    fn pop_request_user_input_request_id(&mut self, turn_id: &str) -> Option<RequestId> {
        let request_id = self
            .request_user_input
            .get_mut(turn_id)
            .and_then(VecDeque::pop_front);
        if self
            .request_user_input
            .get(turn_id)
            .is_some_and(VecDeque::is_empty)
        {
            self.request_user_input.remove(turn_id);
        }
        request_id
    }

    fn register_mcp_elicitation(
        &mut self,
        pending_request_id: RequestId,
        server_name: String,
        request_id: codex_protocol::mcp::RequestId,
    ) {
        self.mcp_elicitations
            .insert(pending_request_id, (server_name, request_id));
    }

    fn pop_mcp_elicitation_request_id(
        &mut self,
        server_name: &str,
        request_id: &codex_protocol::mcp::RequestId,
    ) -> Option<RequestId> {
        let pending_request_id = self.mcp_elicitations.iter().find_map(
            |(pending_request_id, (pending_server_name, pending_request_id_value))| {
                if pending_server_name == server_name && pending_request_id_value == request_id {
                    Some(pending_request_id.clone())
                } else {
                    None
                }
            },
        )?;
        self.mcp_elicitations.remove(&pending_request_id);
        Some(pending_request_id)
    }

    fn clear_mcp_elicitation_by_request_id(&mut self, request_id: &RequestId) {
        self.mcp_elicitations.remove(request_id);
    }
}

fn command_text_to_tokens(command: Option<String>) -> Vec<String> {
    command
        .as_deref()
        .map(|text| {
            shlex::split(text)
                .filter(|parts| !parts.is_empty())
                .unwrap_or_else(|| vec![text.to_string()])
        })
        .unwrap_or_default()
}

fn command_actions_to_core(
    command_actions: Option<Vec<codex_app_server_protocol::CommandAction>>,
    command: Option<&str>,
) -> Vec<ParsedCommand> {
    match command_actions {
        Some(actions) if !actions.is_empty() => actions
            .into_iter()
            .map(codex_app_server_protocol::CommandAction::into_core)
            .collect(),
        _ => command
            .map(|cmd| {
                vec![ParsedCommand::Unknown {
                    cmd: cmd.to_string(),
                }]
            })
            .unwrap_or_default(),
    }
}

fn network_approval_context_to_core(
    value: codex_app_server_protocol::NetworkApprovalContext,
) -> codex_protocol::protocol::NetworkApprovalContext {
    codex_protocol::protocol::NetworkApprovalContext {
        host: value.host,
        protocol: value.protocol.to_core(),
    }
}

fn additional_permission_profile_to_core(
    value: codex_app_server_protocol::AdditionalPermissionProfile,
) -> PermissionProfile {
    PermissionProfile {
        network: value.network.map(|network| NetworkPermissions {
            enabled: network.enabled,
        }),
        file_system: value.file_system.map(|file_system| FileSystemPermissions {
            read: file_system.read,
            write: file_system.write,
        }),
        macos: value.macos.map(|macos| MacOsPermissions {
            preferences: macos.preferences,
            automations: macos.automations,
            accessibility: macos.accessibility,
            calendar: macos.calendar,
        }),
    }
}

fn command_execution_available_decisions_to_core(
    value: Option<Vec<CommandExecutionApprovalDecision>>,
) -> Option<Vec<codex_protocol::protocol::ReviewDecision>> {
    value.map(|decisions| {
        decisions
            .into_iter()
            .map(|decision| match decision {
                CommandExecutionApprovalDecision::Accept => {
                    codex_protocol::protocol::ReviewDecision::Approved
                }
                CommandExecutionApprovalDecision::AcceptForSession => {
                    codex_protocol::protocol::ReviewDecision::ApprovedForSession
                }
                CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
                    execpolicy_amendment,
                } => codex_protocol::protocol::ReviewDecision::ApprovedExecpolicyAmendment {
                    proposed_execpolicy_amendment: execpolicy_amendment.into_core(),
                },
                CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
                    network_policy_amendment,
                } => codex_protocol::protocol::ReviewDecision::NetworkPolicyAmendment {
                    network_policy_amendment: network_policy_amendment.into_core(),
                },
                CommandExecutionApprovalDecision::Decline => {
                    codex_protocol::protocol::ReviewDecision::Denied
                }
                CommandExecutionApprovalDecision::Cancel => {
                    codex_protocol::protocol::ReviewDecision::Abort
                }
            })
            .collect()
    })
}

fn file_update_changes_to_core(
    changes: Vec<codex_app_server_protocol::FileUpdateChange>,
) -> HashMap<PathBuf, FileChange> {
    changes
        .into_iter()
        .map(|change| {
            let file_change = match change.kind {
                PatchChangeKind::Add => FileChange::Add {
                    content: change.diff,
                },
                PatchChangeKind::Delete => FileChange::Delete {
                    content: change.diff,
                },
                PatchChangeKind::Update { move_path } => FileChange::Update {
                    unified_diff: change.diff,
                    move_path,
                },
            };
            (PathBuf::from(change.path), file_change)
        })
        .collect()
}

fn file_change_approval_decision_from_review(
    decision: codex_protocol::protocol::ReviewDecision,
) -> (FileChangeApprovalDecision, bool) {
    match decision {
        codex_protocol::protocol::ReviewDecision::Approved => {
            (FileChangeApprovalDecision::Accept, false)
        }
        codex_protocol::protocol::ReviewDecision::ApprovedForSession => {
            (FileChangeApprovalDecision::AcceptForSession, false)
        }
        codex_protocol::protocol::ReviewDecision::Denied => {
            (FileChangeApprovalDecision::Decline, false)
        }
        codex_protocol::protocol::ReviewDecision::Abort => {
            (FileChangeApprovalDecision::Cancel, false)
        }
        codex_protocol::protocol::ReviewDecision::ApprovedExecpolicyAmendment { .. }
        | codex_protocol::protocol::ReviewDecision::NetworkPolicyAmendment { .. } => {
            (FileChangeApprovalDecision::Accept, true)
        }
    }
}

fn request_user_input_questions_to_core(
    questions: Vec<codex_app_server_protocol::ToolRequestUserInputQuestion>,
) -> Vec<codex_protocol::request_user_input::RequestUserInputQuestion> {
    questions
        .into_iter()
        .map(
            |question| codex_protocol::request_user_input::RequestUserInputQuestion {
                id: question.id,
                header: question.header,
                question: question.question,
                is_other: question.is_other,
                is_secret: question.is_secret,
                options: question.options.map(|options| {
                    options
                        .into_iter()
                        .map(|option| {
                            codex_protocol::request_user_input::RequestUserInputQuestionOption {
                                label: option.label,
                                description: option.description,
                            }
                        })
                        .collect()
                }),
            },
        )
        .collect()
}

fn skill_scope_to_core(
    scope: codex_app_server_protocol::SkillScope,
) -> codex_protocol::protocol::SkillScope {
    match scope {
        codex_app_server_protocol::SkillScope::User => codex_protocol::protocol::SkillScope::User,
        codex_app_server_protocol::SkillScope::Repo => codex_protocol::protocol::SkillScope::Repo,
        codex_app_server_protocol::SkillScope::System => {
            codex_protocol::protocol::SkillScope::System
        }
        codex_app_server_protocol::SkillScope::Admin => codex_protocol::protocol::SkillScope::Admin,
    }
}

fn skill_interface_to_core(
    interface: codex_app_server_protocol::SkillInterface,
) -> codex_protocol::protocol::SkillInterface {
    codex_protocol::protocol::SkillInterface {
        display_name: interface.display_name,
        short_description: interface.short_description,
        icon_small: interface.icon_small,
        icon_large: interface.icon_large,
        brand_color: interface.brand_color,
        default_prompt: interface.default_prompt,
    }
}

fn skill_dependencies_to_core(
    dependencies: codex_app_server_protocol::SkillDependencies,
) -> codex_protocol::protocol::SkillDependencies {
    codex_protocol::protocol::SkillDependencies {
        tools: dependencies
            .tools
            .into_iter()
            .map(|tool| codex_protocol::protocol::SkillToolDependency {
                r#type: tool.r#type,
                value: tool.value,
                description: tool.description,
                transport: tool.transport,
                command: tool.command,
                url: tool.url,
            })
            .collect(),
    }
}

fn skill_metadata_to_core(
    metadata: codex_app_server_protocol::SkillMetadata,
) -> codex_protocol::protocol::SkillMetadata {
    codex_protocol::protocol::SkillMetadata {
        name: metadata.name,
        description: metadata.description,
        short_description: metadata.short_description,
        interface: metadata.interface.map(skill_interface_to_core),
        dependencies: metadata.dependencies.map(skill_dependencies_to_core),
        path: metadata.path,
        scope: skill_scope_to_core(metadata.scope),
        enabled: metadata.enabled,
    }
}

fn skills_list_entry_to_core(
    entry: codex_app_server_protocol::SkillsListEntry,
) -> codex_protocol::protocol::SkillsListEntry {
    codex_protocol::protocol::SkillsListEntry {
        cwd: entry.cwd,
        skills: entry
            .skills
            .into_iter()
            .map(skill_metadata_to_core)
            .collect(),
        errors: entry
            .errors
            .into_iter()
            .map(|error| codex_protocol::protocol::SkillErrorInfo {
                path: error.path,
                message: error.message,
            })
            .collect(),
    }
}

fn remote_skill_summary_to_core(
    summary: codex_app_server_protocol::RemoteSkillSummary,
) -> codex_protocol::protocol::RemoteSkillSummary {
    codex_protocol::protocol::RemoteSkillSummary {
        id: summary.id,
        name: summary.name,
        description: summary.description,
    }
}

fn remote_scope_to_protocol(
    scope: codex_protocol::protocol::RemoteSkillHazelnutScope,
) -> codex_app_server_protocol::HazelnutScope {
    match scope {
        codex_protocol::protocol::RemoteSkillHazelnutScope::WorkspaceShared => {
            codex_app_server_protocol::HazelnutScope::WorkspaceShared
        }
        codex_protocol::protocol::RemoteSkillHazelnutScope::AllShared => {
            codex_app_server_protocol::HazelnutScope::AllShared
        }
        codex_protocol::protocol::RemoteSkillHazelnutScope::Personal => {
            codex_app_server_protocol::HazelnutScope::Personal
        }
        codex_protocol::protocol::RemoteSkillHazelnutScope::Example => {
            codex_app_server_protocol::HazelnutScope::Example
        }
    }
}

fn product_surface_to_protocol(
    product_surface: codex_protocol::protocol::RemoteSkillProductSurface,
) -> codex_app_server_protocol::ProductSurface {
    match product_surface {
        codex_protocol::protocol::RemoteSkillProductSurface::Chatgpt => {
            codex_app_server_protocol::ProductSurface::Chatgpt
        }
        codex_protocol::protocol::RemoteSkillProductSurface::Codex => {
            codex_app_server_protocol::ProductSurface::Codex
        }
        codex_protocol::protocol::RemoteSkillProductSurface::Api => {
            codex_app_server_protocol::ProductSurface::Api
        }
        codex_protocol::protocol::RemoteSkillProductSurface::Atlas => {
            codex_app_server_protocol::ProductSurface::Atlas
        }
    }
}

async fn resolve_server_request(
    client: &InProcessAppServerClient,
    request_id: RequestId,
    value: serde_json::Value,
    method: &str,
    app_event_tx: &AppEventSender,
) {
    if let Err(err) = client.resolve_server_request(request_id, value).await {
        send_error_event(
            app_event_tx,
            format!("failed to resolve server request for `{method}`: {err}"),
        );
    }
}

async fn reject_server_request(
    client: &InProcessAppServerClient,
    request_id: RequestId,
    method: &str,
    reason: String,
    app_event_tx: &AppEventSender,
) {
    if let Err(err) = client
        .reject_server_request(
            request_id,
            JSONRPCErrorError {
                code: -32000,
                message: reason,
                data: None,
            },
        )
        .await
    {
        send_error_event(
            app_event_tx,
            format!("failed to reject `{method}` server request: {err}"),
        );
    }
}

fn local_only_deferred_message(action_name: &str) -> String {
    format!("{action_name} is temporarily unavailable in in-process local-only mode")
}

fn app_server_request_id_to_mcp(request_id: RequestId) -> codex_protocol::mcp::RequestId {
    // In this path the app-server request id is used as the TUI correlation id.
    // App-server translates the resolved server request back to the original MCP request.
    match request_id {
        RequestId::String(id) => codex_protocol::mcp::RequestId::String(id),
        RequestId::Integer(id) => codex_protocol::mcp::RequestId::Integer(id),
    }
}

fn mcp_elicitation_request_to_core(
    request: McpServerElicitationRequest,
) -> codex_protocol::approvals::ElicitationRequest {
    match request {
        McpServerElicitationRequest::Form {
            message,
            requested_schema,
        } => codex_protocol::approvals::ElicitationRequest::Form {
            message,
            requested_schema,
        },
        McpServerElicitationRequest::Url {
            message,
            url,
            elicitation_id,
        } => codex_protocol::approvals::ElicitationRequest::Url {
            message,
            url,
            elicitation_id,
        },
    }
}

fn mcp_elicitation_action_to_protocol(
    action: codex_protocol::approvals::ElicitationAction,
) -> McpServerElicitationAction {
    match action {
        codex_protocol::approvals::ElicitationAction::Accept => McpServerElicitationAction::Accept,
        codex_protocol::approvals::ElicitationAction::Decline => {
            McpServerElicitationAction::Decline
        }
        codex_protocol::approvals::ElicitationAction::Cancel => McpServerElicitationAction::Cancel,
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct StoredHistoryEntry {
    session_id: String,
    ts: u64,
    text: String,
}

fn history_file_path(config: &Config) -> PathBuf {
    config.codex_home.join(HISTORY_FILENAME)
}

fn now_unix_seconds() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|err| format!("system clock before unix epoch: {err}"))
}

fn history_entry_from_line(line: &str) -> Option<codex_protocol::message_history::HistoryEntry> {
    if let Ok(entry) = serde_json::from_str::<StoredHistoryEntry>(line) {
        return Some(codex_protocol::message_history::HistoryEntry {
            conversation_id: entry.session_id,
            ts: entry.ts,
            text: entry.text,
        });
    }

    serde_json::from_str::<codex_protocol::message_history::HistoryEntry>(line).ok()
}

#[cfg(unix)]
fn history_log_id(metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.ino()
}

#[cfg(windows)]
fn history_log_id(metadata: &std::fs::Metadata) -> u64 {
    use std::os::windows::fs::MetadataExt;
    metadata.creation_time()
}

#[cfg(not(any(unix, windows)))]
fn history_log_id(_metadata: &std::fs::Metadata) -> u64 {
    0
}

fn trim_target_bytes(max_bytes: u64, newest_entry_len: u64) -> u64 {
    let soft_cap_bytes = ((max_bytes as f64) * HISTORY_SOFT_CAP_RATIO)
        .floor()
        .clamp(1.0, max_bytes as f64) as u64;
    soft_cap_bytes.max(newest_entry_len)
}

fn trim_history_file(file: &mut std::fs::File, max_bytes: Option<usize>) -> Result<(), String> {
    let Some(max_bytes) = max_bytes else {
        return Ok(());
    };
    if max_bytes == 0 {
        return Ok(());
    }

    let max_bytes = u64::try_from(max_bytes)
        .map_err(|err| format!("invalid history max_bytes value: {err}"))?;
    let mut current_len = file
        .metadata()
        .map_err(|err| format!("failed to read history metadata: {err}"))?
        .len();
    if current_len <= max_bytes {
        return Ok(());
    }

    let mut reader_file = file
        .try_clone()
        .map_err(|err| format!("failed to clone history file: {err}"))?;
    reader_file
        .seek(SeekFrom::Start(0))
        .map_err(|err| format!("failed to seek history file: {err}"))?;
    let mut buf_reader = BufReader::new(reader_file);
    let mut line_buf = String::new();
    let mut line_lengths = Vec::new();
    loop {
        line_buf.clear();
        let bytes = buf_reader
            .read_line(&mut line_buf)
            .map_err(|err| format!("failed to read history line: {err}"))?;
        if bytes == 0 {
            break;
        }
        line_lengths.push(bytes as u64);
    }
    if line_lengths.is_empty() {
        return Ok(());
    }

    let last_index = line_lengths.len() - 1;
    let trim_target = trim_target_bytes(max_bytes, line_lengths[last_index]);
    let mut drop_bytes = 0u64;
    let mut idx = 0usize;
    while current_len > trim_target && idx < last_index {
        current_len = current_len.saturating_sub(line_lengths[idx]);
        drop_bytes += line_lengths[idx];
        idx += 1;
    }
    if drop_bytes == 0 {
        return Ok(());
    }

    let mut reader = buf_reader.into_inner();
    reader
        .seek(SeekFrom::Start(drop_bytes))
        .map_err(|err| format!("failed to seek trimmed history position: {err}"))?;
    let capacity = usize::try_from(current_len).unwrap_or(0);
    let mut tail = Vec::with_capacity(capacity);
    reader
        .read_to_end(&mut tail)
        .map_err(|err| format!("failed to read history tail: {err}"))?;

    file.set_len(0)
        .map_err(|err| format!("failed to truncate history file: {err}"))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|err| format!("failed to seek truncated history file: {err}"))?;
    file.write_all(&tail)
        .map_err(|err| format!("failed to write trimmed history file: {err}"))?;
    file.flush()
        .map_err(|err| format!("failed to flush trimmed history file: {err}"))?;
    Ok(())
}

fn append_history_entry_blocking(
    path: PathBuf,
    line: String,
    max_bytes: Option<usize>,
) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.append(true);
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|err| format!("failed to open history file: {err}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = file
            .metadata()
            .map_err(|err| format!("failed to stat history file: {err}"))?;
        let current_mode = metadata.permissions().mode() & 0o777;
        if current_mode != 0o600 {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o600);
            file.set_permissions(permissions)
                .map_err(|err| format!("failed to set history permissions: {err}"))?;
        }
    }

    for _ in 0..HISTORY_LOCK_MAX_RETRIES {
        match file.try_lock() {
            Ok(()) => {
                file.seek(SeekFrom::End(0))
                    .map_err(|err| format!("failed to seek history file: {err}"))?;
                file.write_all(line.as_bytes())
                    .map_err(|err| format!("failed to append history entry: {err}"))?;
                file.flush()
                    .map_err(|err| format!("failed to flush history entry: {err}"))?;
                trim_history_file(&mut file, max_bytes)?;
                return Ok(());
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                std::thread::sleep(HISTORY_LOCK_RETRY_SLEEP);
            }
            Err(err) => {
                return Err(format!("failed to acquire exclusive history lock: {err}"));
            }
        }
    }

    Err("could not acquire exclusive history lock after retries".to_string())
}

fn read_history_entry_blocking(
    path: PathBuf,
    requested_log_id: u64,
    offset: usize,
) -> Result<Option<codex_protocol::message_history::HistoryEntry>, String> {
    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|err| format!("failed to open history file: {err}"))?;
    let metadata = file
        .metadata()
        .map_err(|err| format!("failed to stat history file: {err}"))?;
    let current_log_id = history_log_id(&metadata);
    if requested_log_id != 0 && requested_log_id != current_log_id {
        return Ok(None);
    }

    for _ in 0..HISTORY_LOCK_MAX_RETRIES {
        match file.try_lock_shared() {
            Ok(()) => {
                let reader = BufReader::new(&file);
                for (idx, line_result) in reader.lines().enumerate() {
                    let line =
                        line_result.map_err(|err| format!("failed to read history line: {err}"))?;
                    if idx == offset {
                        return Ok(history_entry_from_line(&line));
                    }
                }
                return Ok(None);
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                std::thread::sleep(HISTORY_LOCK_RETRY_SLEEP);
            }
            Err(err) => {
                return Err(format!("failed to acquire shared history lock: {err}"));
            }
        }
    }

    Err("could not acquire shared history lock after retries".to_string())
}

async fn append_history_entry_local(
    config: &Config,
    session_id: &ThreadId,
    text: String,
) -> Result<(), String> {
    if config.history.persistence == HistoryPersistence::None {
        return Ok(());
    }

    let path = history_file_path(config);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| format!("failed to create history dir: {err}"))?;
    }

    let entry = StoredHistoryEntry {
        session_id: session_id.to_string(),
        ts: now_unix_seconds()?,
        text,
    };
    let mut line = serde_json::to_string(&entry)
        .map_err(|err| format!("failed to serialize history entry: {err}"))?;
    line.push('\n');
    let max_bytes = config.history.max_bytes;
    tokio::task::spawn_blocking(move || append_history_entry_blocking(path, line, max_bytes))
        .await
        .map_err(|err| format!("failed to join history append task: {err}"))?
}

async fn read_history_entry_local(
    config: &Config,
    requested_log_id: u64,
    offset: usize,
) -> Result<Option<codex_protocol::message_history::HistoryEntry>, String> {
    let path = history_file_path(config);
    if !tokio::fs::try_exists(&path)
        .await
        .map_err(|err| format!("failed to check history file existence: {err}"))?
    {
        return Ok(None);
    }
    tokio::task::spawn_blocking(move || read_history_entry_blocking(path, requested_log_id, offset))
        .await
        .map_err(|err| format!("failed to join history read task: {err}"))?
}

fn local_external_chatgpt_tokens(
    config: &Config,
) -> Result<ChatgptAuthTokensRefreshResponse, String> {
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );
    auth_manager.set_forced_chatgpt_workspace_id(config.forced_chatgpt_workspace_id.clone());
    auth_manager.reload();

    let auth = auth_manager
        .auth_cached()
        .ok_or_else(|| "no cached auth available for local token refresh".to_string())?;
    if !auth.is_external_chatgpt_tokens() {
        return Err("external ChatGPT token auth is not active".to_string());
    }

    let access_token = auth
        .get_token()
        .map_err(|err| format!("failed to read external access token: {err}"))?;
    let chatgpt_account_id = auth
        .get_account_id()
        .ok_or_else(|| "external token auth is missing chatgpt account id".to_string())?;
    let chatgpt_plan_type = auth.account_plan_type().map(|plan_type| match plan_type {
        AccountPlanType::Free => "free".to_string(),
        AccountPlanType::Go => "go".to_string(),
        AccountPlanType::Plus => "plus".to_string(),
        AccountPlanType::Pro => "pro".to_string(),
        AccountPlanType::Team => "team".to_string(),
        AccountPlanType::Business => "business".to_string(),
        AccountPlanType::Enterprise => "enterprise".to_string(),
        AccountPlanType::Edu => "edu".to_string(),
        AccountPlanType::Unknown => "unknown".to_string(),
    });

    Ok(ChatgptAuthTokensRefreshResponse {
        access_token,
        chatgpt_account_id,
        chatgpt_plan_type,
    })
}

fn send_codex_event(app_event_tx: &AppEventSender, msg: EventMsg) {
    app_event_tx.send(AppEvent::CodexEvent(Event {
        id: String::new(),
        msg,
    }));
}

fn send_warning_event(app_event_tx: &AppEventSender, message: String) {
    send_codex_event(app_event_tx, EventMsg::Warning(WarningEvent { message }));
}

fn send_error_event(app_event_tx: &AppEventSender, message: String) {
    send_codex_event(
        app_event_tx,
        EventMsg::Error(codex_protocol::protocol::ErrorEvent {
            message,
            codex_error_info: None,
        }),
    );
}

fn lagged_event_warning_message(skipped: usize) -> String {
    format!("in-process app-server event stream lagged; dropped {skipped} events")
}

async fn send_request_with_response<T>(
    client: &InProcessAppServerClient,
    request: ClientRequest,
    method: &str,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    client.request_typed(request).await.map_err(|err| {
        if method.is_empty() {
            err.to_string()
        } else {
            format!("{method}: {err}")
        }
    })
}

fn session_configured_from_thread_start_response(
    response: ThreadStartResponse,
) -> Result<SessionConfiguredEvent, String> {
    let session_id = ThreadId::from_string(&response.thread.id)
        .map_err(|err| format!("thread/start returned invalid thread id: {err}"))?;

    Ok(SessionConfiguredEvent {
        session_id,
        forked_from_id: None,
        thread_name: response.thread.name,
        model: response.model,
        model_provider_id: response.model_provider,
        service_tier: response.service_tier,
        approval_policy: response.approval_policy.to_core(),
        sandbox_policy: response.sandbox.to_core(),
        cwd: response.cwd,
        reasoning_effort: response.reasoning_effort,
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: response.thread.path,
    })
}

/// Enriches an early synthetic `SessionConfigured` with later authoritative
/// data from the event stream.
///
/// The TUI emits startup session state immediately so first paint does not wait
/// on the event stream. When app-server later sends a richer
/// `SessionConfigured` for the same session, this merges fields that were
/// unknown during bootstrap and suppresses no-op updates.
fn merge_session_configured_update(
    current: &SessionConfiguredEvent,
    update: SessionConfiguredEvent,
) -> Option<SessionConfiguredEvent> {
    if update.session_id != current.session_id {
        return None;
    }

    let merged = SessionConfiguredEvent {
        session_id: update.session_id,
        forked_from_id: update.forked_from_id.or(current.forked_from_id),
        thread_name: update.thread_name.or_else(|| current.thread_name.clone()),
        model: update.model,
        model_provider_id: update.model_provider_id,
        service_tier: update.service_tier,
        approval_policy: update.approval_policy,
        sandbox_policy: update.sandbox_policy,
        cwd: update.cwd,
        reasoning_effort: update.reasoning_effort,
        history_log_id: update.history_log_id,
        history_entry_count: update.history_entry_count,
        initial_messages: update
            .initial_messages
            .or_else(|| current.initial_messages.clone()),
        network_proxy: update
            .network_proxy
            .or_else(|| current.network_proxy.clone()),
        rollout_path: update.rollout_path.or_else(|| current.rollout_path.clone()),
    };

    let changed = merged.forked_from_id != current.forked_from_id
        || merged.thread_name != current.thread_name
        || merged.model != current.model
        || merged.model_provider_id != current.model_provider_id
        || merged.service_tier != current.service_tier
        || merged.approval_policy != current.approval_policy
        || merged.sandbox_policy != current.sandbox_policy
        || merged.cwd != current.cwd
        || merged.reasoning_effort != current.reasoning_effort
        || merged.history_log_id != current.history_log_id
        || merged.history_entry_count != current.history_entry_count
        || merged.initial_messages.is_some() != current.initial_messages.is_some()
        || merged.network_proxy != current.network_proxy
        || merged.rollout_path != current.rollout_path;

    changed.then_some(merged)
}

fn active_turn_id_from_turns(turns: &[codex_app_server_protocol::Turn]) -> Option<String> {
    turns.iter().rev().find_map(|turn| {
        if turn.status == TurnStatus::InProgress {
            Some(turn.id.clone())
        } else {
            None
        }
    })
}

fn server_request_method_name(request: &ServerRequest) -> String {
    serde_json::to_value(request)
        .ok()
        .and_then(|value| {
            value
                .get("method")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn normalize_legacy_notification_method(method: &str) -> &str {
    method.strip_prefix("codex/event/").unwrap_or(method)
}

fn legacy_notification_to_event(notification: JSONRPCNotification) -> Result<Event, String> {
    let value = notification
        .params
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let method = notification.method;
    let normalized_method = normalize_legacy_notification_method(&method).to_string();
    let serde_json::Value::Object(object) = value else {
        return Err(format!(
            "legacy notification `{}` params were not an object",
            method
        ));
    };
    let mut event_payload = if let Some(serde_json::Value::Object(msg_payload)) = object.get("msg")
    {
        serde_json::Value::Object(msg_payload.clone())
    } else {
        serde_json::Value::Object(object)
    };
    let serde_json::Value::Object(ref mut object) = event_payload else {
        return Err(format!(
            "legacy notification `{method}` event payload was not an object"
        ));
    };
    object.insert(
        "type".to_string(),
        serde_json::Value::String(normalized_method),
    );

    let msg: EventMsg = serde_json::from_value(event_payload)
        .map_err(|err| format!("failed to decode event: {err}"))?;
    Ok(Event {
        id: String::new(),
        msg,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "migration routing keeps dependencies explicit"
)]
async fn process_in_process_command(
    op: Op,
    thread_id: &str,
    session_id: &ThreadId,
    config: &Config,
    current_turn_id: &mut Option<String>,
    request_ids: &mut RequestIdSequencer,
    pending_server_requests: &mut PendingServerRequests,
    client: &InProcessAppServerClient,
    app_event_tx: &AppEventSender,
) -> bool {
    match op {
        Op::Interrupt => {
            let Some(turn_id) = current_turn_id.clone() else {
                send_warning_event(
                    app_event_tx,
                    "turn/interrupt skipped because there is no active turn".to_string(),
                );
                return false;
            };
            let request = ClientRequest::TurnInterrupt {
                request_id: request_ids.next(),
                params: TurnInterruptParams {
                    thread_id: thread_id.to_string(),
                    turn_id,
                },
            };
            if let Err(err) = send_request_with_response::<TurnInterruptResponse>(
                client,
                request,
                "turn/interrupt",
            )
            .await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::UserInput {
            items,
            final_output_json_schema,
        } => {
            let request = ClientRequest::TurnStart {
                request_id: request_ids.next(),
                params: TurnStartParams {
                    thread_id: thread_id.to_string(),
                    input: items.into_iter().map(Into::into).collect(),
                    output_schema: final_output_json_schema,
                    ..TurnStartParams::default()
                },
            };
            match send_request_with_response::<TurnStartResponse>(client, request, "turn/start")
                .await
            {
                Ok(response) => {
                    *current_turn_id = Some(response.turn.id);
                }
                Err(err) => send_error_event(app_event_tx, err),
            }
        }
        Op::UserTurn {
            items,
            cwd,
            approval_policy,
            sandbox_policy,
            model,
            effort,
            summary,
            service_tier,
            final_output_json_schema,
            collaboration_mode,
            personality,
        } => {
            let request = ClientRequest::TurnStart {
                request_id: request_ids.next(),
                params: TurnStartParams {
                    thread_id: thread_id.to_string(),
                    input: items.into_iter().map(Into::into).collect(),
                    cwd: Some(cwd),
                    approval_policy: Some(approval_policy.into()),
                    sandbox_policy: Some(sandbox_policy.into()),
                    model: Some(model),
                    service_tier,
                    effort,
                    summary,
                    personality,
                    output_schema: final_output_json_schema,
                    collaboration_mode,
                },
            };
            match send_request_with_response::<TurnStartResponse>(client, request, "turn/start")
                .await
            {
                Ok(response) => {
                    *current_turn_id = Some(response.turn.id);
                }
                Err(err) => send_error_event(app_event_tx, err),
            }
        }
        Op::RealtimeConversationStart(params) => {
            let request = ClientRequest::ThreadRealtimeStart {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::ThreadRealtimeStartParams {
                    thread_id: thread_id.to_string(),
                    prompt: params.prompt,
                    session_id: params.session_id,
                },
            };
            if let Err(err) = send_request_with_response::<ThreadRealtimeStartResponse>(
                client,
                request,
                "thread/realtime/start",
            )
            .await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::RealtimeConversationAudio(params) => {
            let request = ClientRequest::ThreadRealtimeAppendAudio {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::ThreadRealtimeAppendAudioParams {
                    thread_id: thread_id.to_string(),
                    audio: params.frame.into(),
                },
            };
            if let Err(err) = send_request_with_response::<ThreadRealtimeAppendAudioResponse>(
                client,
                request,
                "thread/realtime/appendAudio",
            )
            .await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::RealtimeConversationText(params) => {
            let request = ClientRequest::ThreadRealtimeAppendText {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::ThreadRealtimeAppendTextParams {
                    thread_id: thread_id.to_string(),
                    text: params.text,
                },
            };
            if let Err(err) = send_request_with_response::<ThreadRealtimeAppendTextResponse>(
                client,
                request,
                "thread/realtime/appendText",
            )
            .await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::RealtimeConversationClose => {
            let request = ClientRequest::ThreadRealtimeStop {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::ThreadRealtimeStopParams {
                    thread_id: thread_id.to_string(),
                },
            };
            if let Err(err) = send_request_with_response::<ThreadRealtimeStopResponse>(
                client,
                request,
                "thread/realtime/stop",
            )
            .await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::CleanBackgroundTerminals => {
            let request = ClientRequest::ThreadBackgroundTerminalsClean {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::ThreadBackgroundTerminalsCleanParams {
                    thread_id: thread_id.to_string(),
                },
            };
            if let Err(err) = send_request_with_response::<ThreadBackgroundTerminalsCleanResponse>(
                client,
                request,
                "thread/backgroundTerminals/clean",
            )
            .await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::ListModels => {
            let request = ClientRequest::ModelList {
                request_id: request_ids.next(),
                params: ModelListParams::default(),
            };
            if let Err(err) =
                send_request_with_response::<ModelListResponse>(client, request, "model/list").await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::RefreshMcpServers { config: _ } => {
            let request = ClientRequest::McpServerRefresh {
                request_id: request_ids.next(),
                params: None,
            };
            if let Err(err) = send_request_with_response::<McpServerRefreshResponse>(
                client,
                request,
                "config/mcpServer/reload",
            )
            .await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::ListSkills { cwds, force_reload } => {
            let request = ClientRequest::SkillsList {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::SkillsListParams {
                    cwds,
                    force_reload,
                    per_cwd_extra_user_roots: None,
                },
            };
            match send_request_with_response::<SkillsListResponse>(client, request, "skills/list")
                .await
            {
                Ok(response) => {
                    send_codex_event(
                        app_event_tx,
                        EventMsg::ListSkillsResponse(ListSkillsResponseEvent {
                            skills: response
                                .data
                                .into_iter()
                                .map(skills_list_entry_to_core)
                                .collect(),
                        }),
                    );
                }
                Err(err) => send_error_event(app_event_tx, err),
            }
        }
        Op::ListRemoteSkills {
            hazelnut_scope,
            product_surface,
            enabled,
        } => {
            let request = ClientRequest::SkillsRemoteList {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::SkillsRemoteReadParams {
                    hazelnut_scope: remote_scope_to_protocol(hazelnut_scope),
                    product_surface: product_surface_to_protocol(product_surface),
                    enabled: enabled.unwrap_or(false),
                },
            };
            match send_request_with_response::<SkillsRemoteReadResponse>(
                client,
                request,
                "skills/remote/list",
            )
            .await
            {
                Ok(response) => {
                    send_codex_event(
                        app_event_tx,
                        EventMsg::ListRemoteSkillsResponse(ListRemoteSkillsResponseEvent {
                            skills: response
                                .data
                                .into_iter()
                                .map(remote_skill_summary_to_core)
                                .collect(),
                        }),
                    );
                }
                Err(err) => send_error_event(app_event_tx, err),
            }
        }
        Op::DownloadRemoteSkill { hazelnut_id } => {
            let request = ClientRequest::SkillsRemoteExport {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::SkillsRemoteWriteParams { hazelnut_id },
            };
            match send_request_with_response::<SkillsRemoteWriteResponse>(
                client,
                request,
                "skills/remote/export",
            )
            .await
            {
                Ok(response) => {
                    let id = response.id;
                    send_codex_event(
                        app_event_tx,
                        EventMsg::RemoteSkillDownloaded(RemoteSkillDownloadedEvent {
                            id: id.clone(),
                            name: id,
                            path: response.path,
                        }),
                    );
                }
                Err(err) => send_error_event(app_event_tx, err),
            }
        }
        Op::Compact => {
            let request = ClientRequest::ThreadCompactStart {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::ThreadCompactStartParams {
                    thread_id: thread_id.to_string(),
                },
            };
            if let Err(err) = send_request_with_response::<ThreadCompactStartResponse>(
                client,
                request,
                "thread/compact/start",
            )
            .await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::ThreadRollback { num_turns } => {
            let request = ClientRequest::ThreadRollback {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::ThreadRollbackParams {
                    thread_id: thread_id.to_string(),
                    num_turns,
                },
            };
            match send_request_with_response::<ThreadRollbackResponse>(
                client,
                request,
                "thread/rollback",
            )
            .await
            {
                Ok(response) => {
                    *current_turn_id = active_turn_id_from_turns(&response.thread.turns);
                }
                Err(err) => {
                    send_codex_event(
                        app_event_tx,
                        EventMsg::Error(codex_protocol::protocol::ErrorEvent {
                            message: err,
                            codex_error_info: Some(CodexErrorInfo::ThreadRollbackFailed),
                        }),
                    );
                }
            }
        }
        Op::SetThreadName { name } => {
            let request = ClientRequest::ThreadSetName {
                request_id: request_ids.next(),
                params: codex_app_server_protocol::ThreadSetNameParams {
                    thread_id: thread_id.to_string(),
                    name,
                },
            };
            if let Err(err) = send_request_with_response::<ThreadSetNameResponse>(
                client,
                request,
                "thread/name/set",
            )
            .await
            {
                send_error_event(app_event_tx, err);
            }
        }
        Op::ExecApproval { id, decision, .. } => {
            let Some(pending_request) = pending_server_requests.exec_approvals.remove(&id) else {
                send_warning_event(
                    app_event_tx,
                    format!("exec approval ignored because request id `{id}` was not pending"),
                );
                return false;
            };

            let (request_id, result) = match pending_request {
                PendingExecApprovalRequest::V2(request_id) => {
                    let response = CommandExecutionRequestApprovalResponse {
                        decision: CommandExecutionApprovalDecision::from(decision),
                    };
                    let result = match serde_json::to_value(response) {
                        Ok(value) => value,
                        Err(err) => {
                            send_error_event(
                                app_event_tx,
                                format!("failed to encode exec approval response: {err}"),
                            );
                            return false;
                        }
                    };
                    (request_id, result)
                }
                PendingExecApprovalRequest::V1(request_id) => {
                    let response = ExecCommandApprovalResponse { decision };
                    let result = match serde_json::to_value(response) {
                        Ok(value) => value,
                        Err(err) => {
                            send_error_event(
                                app_event_tx,
                                format!("failed to encode legacy exec approval response: {err}"),
                            );
                            return false;
                        }
                    };
                    (request_id, result)
                }
            };

            resolve_server_request(
                client,
                request_id,
                result,
                "item/commandExecution/requestApproval",
                app_event_tx,
            )
            .await;
        }
        Op::PatchApproval { id, decision } => {
            let Some(pending_request) = pending_server_requests.patch_approvals.remove(&id) else {
                send_warning_event(
                    app_event_tx,
                    format!("patch approval ignored because request id `{id}` was not pending"),
                );
                return false;
            };

            let (request_id, result) = match pending_request {
                PendingPatchApprovalRequest::V2(request_id) => {
                    let (decision, lossy) = file_change_approval_decision_from_review(decision);
                    if lossy {
                        send_warning_event(
                            app_event_tx,
                            "mapped unsupported patch decision to `accept` for v2 file-change approval"
                                .to_string(),
                        );
                    }
                    let response = FileChangeRequestApprovalResponse { decision };
                    let result = match serde_json::to_value(response) {
                        Ok(value) => value,
                        Err(err) => {
                            send_error_event(
                                app_event_tx,
                                format!("failed to encode patch approval response: {err}"),
                            );
                            return false;
                        }
                    };
                    (request_id, result)
                }
                PendingPatchApprovalRequest::V1(request_id) => {
                    let response = ApplyPatchApprovalResponse { decision };
                    let result = match serde_json::to_value(response) {
                        Ok(value) => value,
                        Err(err) => {
                            send_error_event(
                                app_event_tx,
                                format!("failed to encode legacy patch approval response: {err}"),
                            );
                            return false;
                        }
                    };
                    (request_id, result)
                }
            };

            resolve_server_request(
                client,
                request_id,
                result,
                "item/fileChange/requestApproval",
                app_event_tx,
            )
            .await;
        }
        Op::UserInputAnswer { id, response } => {
            let Some(request_id) = pending_server_requests.pop_request_user_input_request_id(&id)
            else {
                send_warning_event(
                    app_event_tx,
                    format!(
                        "request_user_input response ignored because turn `{id}` has no pending request"
                    ),
                );
                return false;
            };

            let response = ToolRequestUserInputResponse {
                answers: response
                    .answers
                    .into_iter()
                    .map(|(question_id, answer)| {
                        (
                            question_id,
                            ToolRequestUserInputAnswer {
                                answers: answer.answers,
                            },
                        )
                    })
                    .collect(),
            };
            let result = match serde_json::to_value(response) {
                Ok(value) => value,
                Err(err) => {
                    send_error_event(
                        app_event_tx,
                        format!("failed to encode request_user_input response: {err}"),
                    );
                    return false;
                }
            };
            resolve_server_request(
                client,
                request_id,
                result,
                "item/tool/requestUserInput",
                app_event_tx,
            )
            .await;
        }
        Op::DynamicToolResponse { id, response } => {
            let Some(request_id) = pending_server_requests.dynamic_tool_calls.remove(&id) else {
                send_warning_event(
                    app_event_tx,
                    format!(
                        "dynamic tool response ignored because request id `{id}` was not pending"
                    ),
                );
                return false;
            };
            let response = DynamicToolCallResponse {
                content_items: response
                    .content_items
                    .into_iter()
                    .map(
                        |item| match item {
                            codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem::InputText {
                                text,
                            } => DynamicToolCallOutputContentItem::InputText { text },
                            codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem::InputImage {
                                image_url,
                            } => DynamicToolCallOutputContentItem::InputImage { image_url },
                        },
                    )
                    .collect(),
                success: response.success,
            };
            let result = match serde_json::to_value(response) {
                Ok(value) => value,
                Err(err) => {
                    send_error_event(
                        app_event_tx,
                        format!("failed to encode dynamic tool response: {err}"),
                    );
                    return false;
                }
            };
            resolve_server_request(client, request_id, result, "item/tool/call", app_event_tx)
                .await;
        }
        Op::AddToHistory { text } => {
            if let Err(err) = append_history_entry_local(config, session_id, text).await {
                send_warning_event(
                    app_event_tx,
                    format!("failed to append local history: {err}"),
                );
            }
        }
        Op::GetHistoryEntryRequest { offset, log_id } => {
            match read_history_entry_local(config, log_id, offset).await {
                Ok(entry) => {
                    send_codex_event(
                        app_event_tx,
                        EventMsg::GetHistoryEntryResponse(GetHistoryEntryResponseEvent {
                            offset,
                            log_id,
                            entry,
                        }),
                    );
                }
                Err(err) => {
                    send_warning_event(
                        app_event_tx,
                        format!("failed to read local history entry: {err}"),
                    );
                }
            }
        }
        Op::ListCustomPrompts => {
            let custom_prompts =
                if let Some(dir) = codex_core::custom_prompts::default_prompts_dir() {
                    codex_core::custom_prompts::discover_prompts_in(&dir).await
                } else {
                    Vec::new()
                };
            send_codex_event(
                app_event_tx,
                EventMsg::ListCustomPromptsResponse(ListCustomPromptsResponseEvent {
                    custom_prompts,
                }),
            );
        }
        Op::ReloadUserConfig => {
            tracing::debug!("reload_user_config handled locally in TUI in-process mode");
        }
        Op::Undo => {
            send_warning_event(app_event_tx, local_only_deferred_message("Undo"));
        }
        Op::OverrideTurnContext { .. } => {
            send_warning_event(
                app_event_tx,
                local_only_deferred_message("OverrideTurnContext"),
            );
        }
        Op::DropMemories => {
            send_warning_event(app_event_tx, local_only_deferred_message("DropMemories"));
        }
        Op::UpdateMemories => {
            send_warning_event(app_event_tx, local_only_deferred_message("UpdateMemories"));
        }
        Op::RunUserShellCommand { .. } => {
            send_warning_event(
                app_event_tx,
                local_only_deferred_message("RunUserShellCommand"),
            );
        }
        Op::ListMcpTools => {
            send_warning_event(app_event_tx, local_only_deferred_message("ListMcpTools"));
        }
        Op::ResolveElicitation {
            server_name,
            request_id,
            decision,
            content,
        } => {
            let Some(pending_request_id) =
                pending_server_requests.pop_mcp_elicitation_request_id(&server_name, &request_id)
            else {
                send_warning_event(
                    app_event_tx,
                    format!(
                        "mcp elicitation response ignored because `{server_name}` request `{request_id}` was not pending"
                    ),
                );
                return false;
            };

            let response = McpServerElicitationRequestResponse {
                action: mcp_elicitation_action_to_protocol(decision),
                content,
            };
            let result = match serde_json::to_value(response) {
                Ok(value) => value,
                Err(err) => {
                    send_error_event(
                        app_event_tx,
                        format!("failed to encode mcp elicitation response: {err}"),
                    );
                    return false;
                }
            };
            resolve_server_request(
                client,
                pending_request_id,
                result,
                "mcpServer/elicitation/request",
                app_event_tx,
            )
            .await;
        }
        Op::Shutdown => {
            let request = ClientRequest::ThreadUnsubscribe {
                request_id: request_ids.next(),
                params: ThreadUnsubscribeParams {
                    thread_id: thread_id.to_string(),
                },
            };
            if let Err(err) = send_request_with_response::<ThreadUnsubscribeResponse>(
                client,
                request,
                "thread/unsubscribe",
            )
            .await
            {
                send_warning_event(
                    app_event_tx,
                    format!("thread/unsubscribe failed during shutdown: {err}"),
                );
            }
            return true;
        }
        unsupported => {
            send_warning_event(
                app_event_tx,
                format!(
                    "op `{}` is not routed through in-process app-server yet",
                    serde_json::to_value(&unsupported)
                        .ok()
                        .and_then(|value| {
                            value
                                .get("type")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned)
                        })
                        .unwrap_or_else(|| "unknown".to_string())
                ),
            );
        }
    }

    false
}

#[expect(
    clippy::too_many_arguments,
    reason = "agent loop keeps runtime state explicit"
)]
/// Runs the in-process TUI agent loop for a single active thread.
///
/// This loop is responsible for keeping the TUI's existing `Op`-driven model
/// working on top of app-server. It forwards supported ops as typed
/// `ClientRequest`/`ClientNotification` messages, translates server requests
/// back into UI events, and preserves thread-local bookkeeping such as current
/// turn id and pending approval state.
async fn run_in_process_agent_loop(
    mut codex_op_rx: tokio::sync::mpsc::UnboundedReceiver<Op>,
    mut client: InProcessAppServerClient,
    config: Config,
    thread_id: String,
    mut session_configured: SessionConfiguredEvent,
    app_event_tx: AppEventSender,
    mut request_ids: RequestIdSequencer,
    mut current_turn_id: Option<String>,
) {
    let mut pending_shutdown_complete = false;
    let mut pending_server_requests = PendingServerRequests::default();
    let session_id = session_configured.session_id;
    loop {
        tokio::select! {
            maybe_op = codex_op_rx.recv() => {
                match maybe_op {
                    Some(op) => {
                        let should_shutdown = process_in_process_command(
                            op,
                            &thread_id,
                            &session_id,
                            &config,
                            &mut current_turn_id,
                            &mut request_ids,
                            &mut pending_server_requests,
                            &client,
                            &app_event_tx,
                        ).await;
                        if should_shutdown {
                            pending_shutdown_complete = true;
                            break;
                        }
                    }
                    None => break,
                }
            }
            maybe_event = client.next_event() => {
                let Some(event) = maybe_event else {
                    break;
                };

                match event {
                    InProcessServerEvent::ServerRequest(request) => {
                        let method = server_request_method_name(&request);
                        match request {
                            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                                if params.thread_id != thread_id {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!(
                                            "request targets thread `{}`, but active thread is `{thread_id}`",
                                            params.thread_id
                                        ),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                }

                                let command = command_text_to_tokens(params.command.clone());
                                let parsed_cmd = command_actions_to_core(
                                    params.command_actions,
                                    params.command.as_deref(),
                                );
                                let approval_id = params
                                    .approval_id
                                    .clone()
                                    .unwrap_or_else(|| params.item_id.clone());
                                pending_server_requests.exec_approvals.insert(
                                    approval_id,
                                    PendingExecApprovalRequest::V2(request_id),
                                );
                                send_codex_event(
                                    &app_event_tx,
                                    EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                                        call_id: params.item_id,
                                        approval_id: params.approval_id,
                                        turn_id: params.turn_id,
                                        command,
                                        cwd: params.cwd.unwrap_or_default(),
                                        reason: params.reason,
                                        network_approval_context: params
                                            .network_approval_context
                                            .map(network_approval_context_to_core),
                                        proposed_execpolicy_amendment: params
                                            .proposed_execpolicy_amendment
                                            .map(codex_app_server_protocol::ExecPolicyAmendment::into_core),
                                        proposed_network_policy_amendments: params
                                            .proposed_network_policy_amendments
                                            .map(|items| {
                                                items
                                                    .into_iter()
                                                    .map(codex_app_server_protocol::NetworkPolicyAmendment::into_core)
                                                    .collect()
                                            }),
                                        additional_permissions: params
                                            .additional_permissions
                                            .map(additional_permission_profile_to_core),
                                        available_decisions: command_execution_available_decisions_to_core(
                                            params.available_decisions,
                                        ),
                                        parsed_cmd,
                                    }),
                                );
                            }
                            ServerRequest::ExecCommandApproval { request_id, params } => {
                                if params.conversation_id.to_string() != thread_id {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!(
                                            "request targets thread `{}`, but active thread is `{thread_id}`",
                                            params.conversation_id
                                        ),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                }

                                let approval_id = params
                                    .approval_id
                                    .clone()
                                    .unwrap_or_else(|| params.call_id.clone());
                                pending_server_requests.exec_approvals.insert(
                                    approval_id,
                                    PendingExecApprovalRequest::V1(request_id),
                                );
                                send_codex_event(
                                    &app_event_tx,
                                    EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                                        call_id: params.call_id,
                                        approval_id: params.approval_id,
                                        turn_id: String::new(),
                                        command: params.command,
                                        cwd: params.cwd,
                                        reason: params.reason,
                                        network_approval_context: None,
                                        proposed_execpolicy_amendment: None,
                                        proposed_network_policy_amendments: None,
                                        additional_permissions: None,
                                        available_decisions: None,
                                        parsed_cmd: params.parsed_cmd,
                                    }),
                                );
                            }
                            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                                if params.thread_id != thread_id {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!(
                                            "request targets thread `{}`, but active thread is `{thread_id}`",
                                            params.thread_id
                                        ),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                }

                                let changes = pending_server_requests
                                    .pending_file_changes
                                    .remove(&params.item_id)
                                    .unwrap_or_default();
                                pending_server_requests.patch_approvals.insert(
                                    params.item_id.clone(),
                                    PendingPatchApprovalRequest::V2(request_id),
                                );
                                send_codex_event(
                                    &app_event_tx,
                                    EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                                        call_id: params.item_id,
                                        turn_id: params.turn_id,
                                        changes,
                                        reason: params.reason,
                                        grant_root: params.grant_root,
                                    }),
                                );
                            }
                            ServerRequest::ApplyPatchApproval { request_id, params } => {
                                if params.conversation_id.to_string() != thread_id {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!(
                                            "request targets thread `{}`, but active thread is `{thread_id}`",
                                            params.conversation_id
                                        ),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                }

                                pending_server_requests.patch_approvals.insert(
                                    params.call_id.clone(),
                                    PendingPatchApprovalRequest::V1(request_id),
                                );
                                send_codex_event(
                                    &app_event_tx,
                                    EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                                        call_id: params.call_id,
                                        turn_id: String::new(),
                                        changes: params.file_changes,
                                        reason: params.reason,
                                        grant_root: params.grant_root,
                                    }),
                                );
                            }
                            ServerRequest::ToolRequestUserInput { request_id, params } => {
                                if params.thread_id != thread_id {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!(
                                            "request targets thread `{}`, but active thread is `{thread_id}`",
                                            params.thread_id
                                        ),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                }

                                pending_server_requests
                                    .register_request_user_input(params.turn_id.clone(), request_id);
                                send_codex_event(
                                    &app_event_tx,
                                    EventMsg::RequestUserInput(RequestUserInputEvent {
                                        call_id: params.item_id,
                                        turn_id: params.turn_id,
                                        questions: request_user_input_questions_to_core(
                                            params.questions,
                                        ),
                                    }),
                                );
                            }
                            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                                if params.thread_id != thread_id {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!(
                                            "request targets thread `{}`, but active thread is `{thread_id}`",
                                            params.thread_id
                                        ),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                }

                                let elicitation_id = app_server_request_id_to_mcp(request_id.clone());
                                pending_server_requests.register_mcp_elicitation(
                                    request_id,
                                    params.server_name.clone(),
                                    elicitation_id.clone(),
                                );
                                send_codex_event(
                                    &app_event_tx,
                                    EventMsg::ElicitationRequest(ElicitationRequestEvent {
                                        server_name: params.server_name,
                                        id: elicitation_id,
                                        request: mcp_elicitation_request_to_core(params.request),
                                    }),
                                );
                            }
                            ServerRequest::DynamicToolCall { request_id, params } => {
                                if params.thread_id != thread_id {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!(
                                            "request targets thread `{}`, but active thread is `{thread_id}`",
                                            params.thread_id
                                        ),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                }

                                pending_server_requests
                                    .dynamic_tool_calls
                                    .insert(params.call_id.clone(), request_id);
                                send_codex_event(
                                    &app_event_tx,
                                    EventMsg::DynamicToolCallRequest(DynamicToolCallRequest {
                                        call_id: params.call_id,
                                        turn_id: params.turn_id,
                                        tool: params.tool,
                                        arguments: params.arguments,
                                    }),
                                );
                            }
                            ServerRequest::ChatgptAuthTokensRefresh { request_id, params } => {
                                let refresh_result = tokio::task::spawn_blocking({
                                    let config = config.clone();
                                    move || local_external_chatgpt_tokens(&config)
                                })
                                .await;

                                match refresh_result {
                                    Err(err) => {
                                        reject_server_request(
                                            &client,
                                            request_id,
                                            &method,
                                            format!(
                                                "local chatgpt auth refresh task failed in in-process TUI: {err}"
                                            ),
                                            &app_event_tx,
                                        )
                                        .await;
                                    }
                                    Ok(Err(reason)) => {
                                        reject_server_request(
                                            &client,
                                            request_id,
                                            &method,
                                            format!(
                                                "local chatgpt auth refresh failed in in-process TUI: {reason}"
                                            ),
                                            &app_event_tx,
                                        )
                                        .await;
                                    }
                                    Ok(Ok(response)) => {
                                        if let Some(previous_account_id) = params.previous_account_id.as_deref()
                                            && previous_account_id != response.chatgpt_account_id
                                        {
                                            send_warning_event(
                                                &app_event_tx,
                                                format!(
                                                    "local auth refresh account mismatch: expected `{previous_account_id}`, got `{}`",
                                                    response.chatgpt_account_id
                                                ),
                                            );
                                        }

                                        let value = match serde_json::to_value(response) {
                                            Ok(value) => value,
                                            Err(err) => {
                                                let reason = format!(
                                                    "failed to serialize chatgpt auth refresh response: {err}"
                                                );
                                                send_error_event(
                                                    &app_event_tx,
                                                    reason.clone(),
                                                );
                                                reject_server_request(
                                                    &client,
                                                    request_id,
                                                    &method,
                                                    reason,
                                                    &app_event_tx,
                                                )
                                                .await;
                                                continue;
                                            }
                                        };
                                        resolve_server_request(
                                            &client,
                                            request_id,
                                            value,
                                            "account/chatgptAuthTokens/refresh",
                                            &app_event_tx,
                                        )
                                        .await;
                                    }
                                }
                            }
                        }
                    }
                    InProcessServerEvent::ServerNotification(notification) => {
                        match notification {
                            ServerNotification::ItemStarted(notification)
                                if notification.thread_id == thread_id =>
                            {
                                if let ThreadItem::FileChange { id, changes, .. } = notification.item
                                {
                                    pending_server_requests
                                        .pending_file_changes
                                        .insert(id, file_update_changes_to_core(changes));
                                }
                            }
                            ServerNotification::ServerRequestResolved(notification)
                                if notification.thread_id == thread_id =>
                            {
                                pending_server_requests
                                    .clear_mcp_elicitation_by_request_id(&notification.request_id);
                            }
                            _ => {}
                        }
                    }
                    InProcessServerEvent::LegacyNotification(notification) => {
                        let event = match legacy_notification_to_event(notification) {
                            Ok(event) => event,
                            Err(err) => {
                                send_warning_event(&app_event_tx, err);
                                continue;
                            }
                        };
                        if let EventMsg::SessionConfigured(update) = event.msg {
                            if let Some(merged) =
                                merge_session_configured_update(&session_configured, update)
                            {
                                session_configured = merged.clone();
                                app_event_tx.send(AppEvent::CodexEvent(Event {
                                    id: event.id,
                                    msg: EventMsg::SessionConfigured(merged),
                                }));
                            }
                            continue;
                        }

                        match &event.msg {
                            EventMsg::TurnStarted(payload) => {
                                current_turn_id = Some(payload.turn_id.clone());
                            }
                            EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_) => {
                                current_turn_id = None;
                                pending_server_requests.clear_turn_scoped();
                            }
                            _ => {}
                        }

                        let shutdown_complete = matches!(event.msg, EventMsg::ShutdownComplete);
                        if shutdown_complete {
                            pending_shutdown_complete = true;
                            break;
                        }
                        app_event_tx.send(AppEvent::CodexEvent(event));
                    }
                    InProcessServerEvent::Lagged { skipped } => {
                        send_warning_event(&app_event_tx, lagged_event_warning_message(skipped));
                    }
                }
            }
        }
    }

    let shutdown_error = client.shutdown().await.err();
    if let Some(err) = &shutdown_error {
        send_warning_event(
            &app_event_tx,
            format!("in-process app-server shutdown failed: {err}"),
        );
    }
    if pending_shutdown_complete {
        if shutdown_error.is_some() {
            send_warning_event(
                &app_event_tx,
                "emitting shutdown complete after shutdown error to preserve TUI shutdown flow"
                    .to_string(),
            );
        }
        send_codex_event(&app_event_tx, EventMsg::ShutdownComplete);
    }
}

/// Spawn the agent bootstrapper and op forwarding loop, returning the
/// `UnboundedSender<Op>` used by the UI to submit operations.
pub(crate) fn spawn_agent(
    config: Config,
    app_event_tx: AppEventSender,
    _server: Arc<ThreadManager>,
) -> UnboundedSender<Op> {
    let (codex_op_tx, codex_op_rx) = unbounded_channel::<Op>();

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        let mut request_ids = RequestIdSequencer::new();
        let client = match InProcessAppServerClient::start(in_process_start_args(&config)).await {
            Ok(client) => client,
            Err(err) => {
                let message = format!("Failed to initialize in-process app-server client: {err}");
                tracing::error!("{message}");
                send_error_event(&app_event_tx_clone, message.clone());
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                return;
            }
        };

        let thread_start = match send_request_with_response::<ThreadStartResponse>(
            &client,
            ClientRequest::ThreadStart {
                request_id: request_ids.next(),
                params: ThreadStartParams::default(),
            },
            "thread/start",
        )
        .await
        {
            Ok(response) => response,
            Err(err) => {
                send_error_event(&app_event_tx_clone, err.clone());
                app_event_tx_clone.send(AppEvent::FatalExitRequest(err));
                let _ = client.shutdown().await;
                return;
            }
        };

        let session_configured = match session_configured_from_thread_start_response(thread_start) {
            Ok(event) => event,
            Err(message) => {
                send_error_event(&app_event_tx_clone, message.clone());
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                let _ = client.shutdown().await;
                return;
            }
        };

        let thread_id = session_configured.session_id.to_string();
        send_codex_event(
            &app_event_tx_clone,
            EventMsg::SessionConfigured(session_configured.clone()),
        );

        run_in_process_agent_loop(
            codex_op_rx,
            client,
            config,
            thread_id,
            session_configured,
            app_event_tx_clone,
            request_ids,
            None,
        )
        .await;
    });

    codex_op_tx
}

/// Spawn agent loops for an existing thread (e.g., a forked thread).
/// Sends the provided `SessionConfiguredEvent` immediately, then forwards subsequent
/// events and accepts Ops for submission.
pub(crate) fn spawn_agent_from_existing(
    config: Config,
    mut session_configured: codex_protocol::protocol::SessionConfiguredEvent,
    app_event_tx: AppEventSender,
) -> UnboundedSender<Op> {
    let (codex_op_tx, codex_op_rx) = unbounded_channel::<Op>();

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        let mut request_ids = RequestIdSequencer::new();
        let client = match InProcessAppServerClient::start(in_process_start_args(&config)).await {
            Ok(client) => client,
            Err(err) => {
                let message = format!("failed to initialize in-process app-server client: {err}");
                send_error_event(&app_event_tx_clone, message.clone());
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                return;
            }
        };

        let expected_thread_id = session_configured.session_id.to_string();
        let thread_resume = match send_request_with_response::<ThreadResumeResponse>(
            &client,
            ClientRequest::ThreadResume {
                request_id: request_ids.next(),
                params: ThreadResumeParams {
                    thread_id: expected_thread_id.clone(),
                    path: session_configured.rollout_path.clone(),
                    ..ThreadResumeParams::default()
                },
            },
            "thread/resume",
        )
        .await
        {
            Ok(response) => response,
            Err(err) => {
                let message = format!("in-process thread resume failed: {err}");
                send_error_event(&app_event_tx_clone, message.clone());
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                let _ = client.shutdown().await;
                return;
            }
        };

        if thread_resume.thread.id != expected_thread_id {
            match ThreadId::from_string(&thread_resume.thread.id) {
                Ok(parsed) => {
                    send_warning_event(
                        &app_event_tx_clone,
                        format!(
                            "in-process thread/resume returned `{}` instead of `{expected_thread_id}`; using resumed id",
                            thread_resume.thread.id
                        ),
                    );
                    session_configured.session_id = parsed;
                }
                Err(err) => {
                    let message = format!(
                        "in-process thread/resume returned invalid thread id `{}` ({err})",
                        thread_resume.thread.id
                    );
                    send_error_event(&app_event_tx_clone, message.clone());
                    app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                    let _ = client.shutdown().await;
                    return;
                }
            }
        }

        if session_configured.thread_name.is_none() {
            session_configured.thread_name = thread_resume.thread.name;
        }
        if session_configured.rollout_path.is_none() {
            session_configured.rollout_path = thread_resume.thread.path;
        }

        let thread_id = session_configured.session_id.to_string();
        let current_turn_id = active_turn_id_from_turns(&thread_resume.thread.turns);
        send_codex_event(
            &app_event_tx_clone,
            EventMsg::SessionConfigured(session_configured.clone()),
        );

        run_in_process_agent_loop(
            codex_op_rx,
            client,
            config,
            thread_id,
            session_configured,
            app_event_tx_clone,
            request_ids,
            current_turn_id,
        )
        .await;
    });

    codex_op_tx
}

/// Spawn an op-forwarding loop for an existing thread without subscribing to events.
pub(crate) fn spawn_op_forwarder(thread: std::sync::Arc<CodexThread>) -> UnboundedSender<Op> {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();

    tokio::spawn(async move {
        initialize_app_server_client_name(thread.as_ref()).await;
        while let Some(op) = codex_op_rx.recv().await {
            if let Err(e) = thread.submit(op).await {
                tracing::error!("failed to submit op: {e}");
            }
        }
    });

    codex_op_tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use codex_core::auth::login_with_chatgpt_auth_tokens;
    use codex_core::config::ConfigBuilder;
    use codex_protocol::protocol::ConversationAudioParams;
    use codex_protocol::protocol::ConversationStartParams;
    use codex_protocol::protocol::ConversationTextParams;
    use codex_protocol::protocol::RealtimeAudioFrame;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use tokio::sync::mpsc::unbounded_channel;
    use tokio::time::Duration;
    use tokio::time::timeout;

    async fn test_config() -> Config {
        ConfigBuilder::default()
            .codex_home(std::env::temp_dir())
            .build()
            .await
            .expect("config")
    }

    async fn assert_realtime_op_reports_expected_method(op: Op, expected_method: &str) {
        let config = test_config().await;
        let session_id = ThreadId::new();
        let client = InProcessAppServerClient::start(in_process_start_args(&config))
            .await
            .expect("in-process app-server client");
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let mut current_turn_id = None;
        let mut request_ids = RequestIdSequencer::new();
        let mut pending_server_requests = PendingServerRequests::default();

        let should_shutdown = process_in_process_command(
            op,
            "missing-thread-id",
            &session_id,
            &config,
            &mut current_turn_id,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            &app_event_tx,
        )
        .await;
        assert_eq!(should_shutdown, false);

        let maybe_event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for app event");
        let event = maybe_event.expect("expected app event");
        let AppEvent::CodexEvent(event) = event else {
            panic!("expected codex event");
        };
        let EventMsg::Error(error_event) = event.msg else {
            panic!("expected error event");
        };
        assert_eq!(error_event.codex_error_info, None);
        assert!(
            error_event.message.contains(expected_method),
            "expected error message to contain `{expected_method}`, got `{}`",
            error_event.message
        );

        client.shutdown().await.expect("shutdown in-process client");
    }

    async fn process_single_op(
        config: &Config,
        op: Op,
    ) -> (
        bool,
        tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
        InProcessAppServerClient,
    ) {
        let session_id = ThreadId::new();
        let thread_id = session_id.to_string();
        let client = InProcessAppServerClient::start(in_process_start_args(config))
            .await
            .expect("in-process app-server client");
        let (tx, rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let mut current_turn_id = None;
        let mut request_ids = RequestIdSequencer::new();
        let mut pending_server_requests = PendingServerRequests::default();
        let should_shutdown = process_in_process_command(
            op,
            &thread_id,
            &session_id,
            config,
            &mut current_turn_id,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            &app_event_tx,
        )
        .await;
        (should_shutdown, rx, client)
    }

    fn fake_external_access_token(plan_type: &str) -> String {
        #[derive(serde::Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }

        fn b64url_no_pad(bytes: &[u8]) -> String {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        }

        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = serde_json::json!({
            "https://api.openai.com/auth": {
                "chatgpt_plan_type": plan_type,
            }
        });

        let header_b64 = b64url_no_pad(
            &serde_json::to_vec(&header).expect("serialize fake jwt header for test"),
        );
        let payload_b64 = b64url_no_pad(
            &serde_json::to_vec(&payload).expect("serialize fake jwt payload for test"),
        );
        let signature_b64 = b64url_no_pad(b"sig");
        format!("{header_b64}.{payload_b64}.{signature_b64}")
    }

    async fn next_codex_event(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) -> codex_protocol::protocol::Event {
        let maybe_event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for app event");
        let event = maybe_event.expect("expected app event");
        let AppEvent::CodexEvent(event) = event else {
            panic!("expected codex event");
        };
        event
    }

    fn warning_from_event(event: codex_protocol::protocol::Event) -> WarningEvent {
        let EventMsg::Warning(warning) = event.msg else {
            panic!("expected warning event");
        };
        warning
    }

    #[test]
    fn clear_turn_scoped_preserves_pending_mcp_elicitation_requests() {
        let mut pending = PendingServerRequests::default();
        let pending_request_id = RequestId::Integer(42);
        let server_name = "test-server".to_string();
        let elicitation_id = codex_protocol::mcp::RequestId::Integer(7);
        pending.register_mcp_elicitation(
            pending_request_id.clone(),
            server_name.clone(),
            elicitation_id.clone(),
        );

        pending.clear_turn_scoped();

        assert_eq!(
            pending.pop_mcp_elicitation_request_id(&server_name, &elicitation_id),
            Some(pending_request_id)
        );
    }

    #[test]
    fn server_request_resolved_clears_pending_mcp_elicitation_request() {
        let mut pending = PendingServerRequests::default();
        let pending_request_id = RequestId::Integer(5);
        let server_name = "test-server".to_string();
        let elicitation_id = codex_protocol::mcp::RequestId::String("abc".to_string());
        pending.register_mcp_elicitation(
            pending_request_id.clone(),
            server_name.clone(),
            elicitation_id.clone(),
        );

        pending.clear_mcp_elicitation_by_request_id(&pending_request_id);

        assert_eq!(
            pending.pop_mcp_elicitation_request_id(&server_name, &elicitation_id),
            None
        );
    }

    #[test]
    fn lagged_event_warning_message_is_explicit() {
        assert_eq!(
            lagged_event_warning_message(7),
            "in-process app-server event stream lagged; dropped 7 events".to_string()
        );
    }

    fn session_configured_event() -> SessionConfiguredEvent {
        SessionConfiguredEvent {
            session_id: ThreadId::from_string("019cbf93-9ff5-7ac0-ac93-c8a36f0c98d3")
                .expect("valid thread id"),
            forked_from_id: None,
            thread_name: Some("thread".to_string()),
            model: "gpt-5".to_string(),
            model_provider_id: "openai".to_string(),
            service_tier: None,
            approval_policy: codex_protocol::protocol::AskForApproval::Never,
            sandbox_policy: codex_protocol::protocol::SandboxPolicy::DangerFullAccess,
            cwd: std::env::temp_dir(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::from("/tmp/thread.jsonl")),
        }
    }

    #[test]
    fn merge_session_configured_update_enriches_missing_metadata() {
        let current = session_configured_event();
        let mut update = session_configured_event();
        update.forked_from_id = Some(ThreadId::new());
        update.history_log_id = 41;
        update.history_entry_count = 9;

        let merged = merge_session_configured_update(&current, update)
            .expect("update should enrich session metadata");

        assert_eq!(merged.history_log_id, 41);
        assert_eq!(merged.history_entry_count, 9);
        assert!(merged.forked_from_id.is_some());
        assert_eq!(merged.rollout_path, current.rollout_path);
    }

    #[test]
    fn merge_session_configured_update_ignores_identical_payload() {
        let current = session_configured_event();

        let merged = merge_session_configured_update(&current, session_configured_event());

        assert_eq!(merged.is_none(), true);
    }

    #[test]
    fn legacy_notification_decodes_prefixed_warning_with_direct_payload() {
        let notification = JSONRPCNotification {
            method: "codex/event/warning".to_string(),
            params: Some(serde_json::json!({
                "message": "heads up",
            })),
        };

        let event = legacy_notification_to_event(notification).expect("decode warning");
        let EventMsg::Warning(warning) = event.msg else {
            panic!("expected warning event");
        };
        assert_eq!(warning.message, "heads up".to_string());
    }

    #[test]
    fn legacy_notification_decodes_prefixed_warning_with_event_wrapper_payload() {
        let notification = JSONRPCNotification {
            method: "codex/event/warning".to_string(),
            params: Some(serde_json::json!({
                "conversationId": "thread-1",
                "id": "submission-1",
                "msg": {
                    "message": "wrapped warning",
                    "type": "warning",
                },
            })),
        };

        let event = legacy_notification_to_event(notification).expect("decode wrapped warning");
        let EventMsg::Warning(warning) = event.msg else {
            panic!("expected warning event");
        };
        assert_eq!(warning.message, "wrapped warning".to_string());
    }

    #[test]
    fn legacy_notification_decodes_prefixed_mcp_startup_complete() {
        let notification = JSONRPCNotification {
            method: "codex/event/mcp_startup_complete".to_string(),
            params: Some(serde_json::json!({
                "ready": ["server-a"],
                "failed": [],
                "cancelled": [],
            })),
        };

        let event =
            legacy_notification_to_event(notification).expect("decode mcp startup complete");
        let EventMsg::McpStartupComplete(payload) = event.msg else {
            panic!("expected mcp startup complete event");
        };
        assert_eq!(payload.ready, vec!["server-a".to_string()]);
        assert!(payload.failed.is_empty());
        assert!(payload.cancelled.is_empty());
    }

    #[tokio::test]
    async fn realtime_start_op_routes_to_thread_realtime_start_method() {
        assert_realtime_op_reports_expected_method(
            Op::RealtimeConversationStart(ConversationStartParams {
                prompt: "hello".to_string(),
                session_id: None,
            }),
            "thread/realtime/start",
        )
        .await;
    }

    #[tokio::test]
    async fn realtime_audio_op_routes_to_thread_realtime_append_audio_method() {
        assert_realtime_op_reports_expected_method(
            Op::RealtimeConversationAudio(ConversationAudioParams {
                frame: RealtimeAudioFrame {
                    data: "aGVsbG8=".to_string(),
                    sample_rate: 24_000,
                    num_channels: 1,
                    samples_per_channel: Some(1),
                },
            }),
            "thread/realtime/appendAudio",
        )
        .await;
    }

    #[tokio::test]
    async fn realtime_text_op_routes_to_thread_realtime_append_text_method() {
        assert_realtime_op_reports_expected_method(
            Op::RealtimeConversationText(ConversationTextParams {
                text: "hello".to_string(),
            }),
            "thread/realtime/appendText",
        )
        .await;
    }

    #[tokio::test]
    async fn realtime_close_op_routes_to_thread_realtime_stop_method() {
        assert_realtime_op_reports_expected_method(
            Op::RealtimeConversationClose,
            "thread/realtime/stop",
        )
        .await;
    }

    #[tokio::test]
    async fn list_custom_prompts_emits_response_event_locally() {
        let config = test_config().await;
        let (should_shutdown, mut rx, client) =
            process_single_op(&config, Op::ListCustomPrompts).await;
        assert_eq!(should_shutdown, false);

        let event = next_codex_event(&mut rx).await;
        let EventMsg::ListCustomPromptsResponse(_) = event.msg else {
            panic!("expected ListCustomPromptsResponse");
        };

        client.shutdown().await.expect("shutdown in-process client");
    }

    #[tokio::test]
    async fn add_to_history_and_get_history_entry_work_locally() {
        let codex_home = TempDir::new().expect("create temp dir");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");
        let session_id = ThreadId::new();
        let thread_id = session_id.to_string();
        let client = InProcessAppServerClient::start(in_process_start_args(&config))
            .await
            .expect("in-process app-server client");
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let mut current_turn_id = None;
        let mut request_ids = RequestIdSequencer::new();
        let mut pending_server_requests = PendingServerRequests::default();

        let should_shutdown = process_in_process_command(
            Op::AddToHistory {
                text: "hello history".to_string(),
            },
            &thread_id,
            &session_id,
            &config,
            &mut current_turn_id,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            &app_event_tx,
        )
        .await;
        assert_eq!(should_shutdown, false);

        let should_shutdown = process_in_process_command(
            Op::GetHistoryEntryRequest {
                offset: 0,
                log_id: 0,
            },
            &thread_id,
            &session_id,
            &config,
            &mut current_turn_id,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            &app_event_tx,
        )
        .await;
        assert_eq!(should_shutdown, false);

        let event = next_codex_event(&mut rx).await;
        let EventMsg::GetHistoryEntryResponse(response) = event.msg else {
            panic!("expected GetHistoryEntryResponse");
        };
        let entry = response.entry.expect("expected history entry");
        assert_eq!(response.offset, 0);
        assert_eq!(response.log_id, 0);
        assert_eq!(entry.conversation_id, thread_id);
        assert_eq!(entry.text, "hello history".to_string());

        client.shutdown().await.expect("shutdown in-process client");
    }

    #[tokio::test]
    async fn reload_user_config_is_a_local_noop() {
        let config = test_config().await;
        let (should_shutdown, mut rx, client) =
            process_single_op(&config, Op::ReloadUserConfig).await;
        assert_eq!(should_shutdown, false);

        if let Ok(Some(event)) = timeout(Duration::from_millis(200), rx.recv()).await {
            panic!("did not expect an app event: {event:?}");
        }

        client.shutdown().await.expect("shutdown in-process client");
    }

    async fn assert_local_only_warning_for_op(config: &Config, op: Op, expected_message: &str) {
        let (should_shutdown, mut rx, client) = process_single_op(config, op).await;
        assert_eq!(should_shutdown, false);

        let event = next_codex_event(&mut rx).await;
        let warning = warning_from_event(event);
        assert_eq!(warning.message, expected_message.to_string());

        client.shutdown().await.expect("shutdown in-process client");
    }

    #[tokio::test]
    async fn deferred_op_emits_explicit_local_only_warning() {
        let config = test_config().await;
        let deferred_ops = vec![
            (
                Op::Undo,
                "Undo is temporarily unavailable in in-process local-only mode",
            ),
            (
                Op::OverrideTurnContext {
                    cwd: None,
                    approval_policy: None,
                    sandbox_policy: None,
                    windows_sandbox_level: None,
                    model: None,
                    effort: None,
                    summary: None,
                    service_tier: None,
                    collaboration_mode: None,
                    personality: None,
                },
                "OverrideTurnContext is temporarily unavailable in in-process local-only mode",
            ),
            (
                Op::DropMemories,
                "DropMemories is temporarily unavailable in in-process local-only mode",
            ),
            (
                Op::UpdateMemories,
                "UpdateMemories is temporarily unavailable in in-process local-only mode",
            ),
            (
                Op::RunUserShellCommand {
                    command: "echo hello".to_string(),
                },
                "RunUserShellCommand is temporarily unavailable in in-process local-only mode",
            ),
            (
                Op::ListMcpTools,
                "ListMcpTools is temporarily unavailable in in-process local-only mode",
            ),
        ];

        for (op, expected_warning) in deferred_ops {
            assert_local_only_warning_for_op(&config, op, expected_warning).await;
        }
    }

    #[tokio::test]
    async fn resolve_elicitation_without_pending_request_warns() {
        let config = test_config().await;
        let (should_shutdown, mut rx, client) = process_single_op(
            &config,
            Op::ResolveElicitation {
                server_name: "test-server".to_string(),
                request_id: codex_protocol::mcp::RequestId::Integer(1),
                decision: codex_protocol::approvals::ElicitationAction::Cancel,
                content: None,
            },
        )
        .await;
        assert_eq!(should_shutdown, false);

        let event = next_codex_event(&mut rx).await;
        let warning = warning_from_event(event);
        assert_eq!(
            warning.message,
            "mcp elicitation response ignored because `test-server` request `1` was not pending"
                .to_string()
        );

        client.shutdown().await.expect("shutdown in-process client");
    }

    #[tokio::test]
    async fn local_external_chatgpt_refresh_reads_tokens_from_auth_storage() {
        let codex_home = TempDir::new().expect("create temp dir");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");
        let access_token = fake_external_access_token("pro");
        login_with_chatgpt_auth_tokens(
            &config.codex_home,
            &access_token,
            "workspace-1",
            Some("pro"),
        )
        .expect("write external auth token");

        let response =
            local_external_chatgpt_tokens(&config).expect("local token refresh response");
        assert_eq!(response.access_token, access_token);
        assert_eq!(response.chatgpt_account_id, "workspace-1".to_string());
        assert_eq!(response.chatgpt_plan_type, Some("pro".to_string()));
    }

    #[tokio::test]
    async fn local_external_chatgpt_refresh_fails_without_external_auth() {
        let codex_home = TempDir::new().expect("create temp dir");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");
        let error =
            local_external_chatgpt_tokens(&config).expect_err("expected local refresh error");
        assert!(
            error.contains("no cached auth available")
                || error.contains("external ChatGPT token auth is not active"),
            "unexpected error: {error}"
        );
    }
}
