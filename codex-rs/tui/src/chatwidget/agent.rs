//! In-process app-server agent for the TUI.
//!
//! This module owns the background task that bridges the TUI's `Op`-driven
//! command model and the app-server's JSON-RPC protocol. On startup it creates
//! an [`InProcessAppServerClient`], opens a thread via `thread/start`, and then
//! enters a `select!` loop that:
//!
//! 1. Receives `Op` values from the `ChatWidget` and translates them into
//!    app-server client requests (`turn/start`, `turn/interrupt`, approvals,
//!    etc.), while forwarding a small set of legacy thread ops directly to the
//!    backing `CodexThread` until app-server grows first-class equivalents.
//! 2. Receives server events (`ServerRequest`, `ServerNotification`, legacy
//!    `JSONRPCNotification`) from the app-server and converts them into
//!    `EventMsg` values that the TUI already knows how to render.
//!
//! The module also contains local history I/O, protocol-type conversion
//! helpers, and the `spawn_op_forwarder` used for resumed/forked threads that
//! bypass the in-process client.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::future::Future;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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
use codex_app_server_protocol::GrantedMacOsPermissions;
use codex_app_server_protocol::GrantedPermissionProfile;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::McpServerRefreshResponse;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::PermissionsRequestApprovalResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ReviewDelivery;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget as ApiReviewTarget;
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
use codex_protocol::models::MacOsSeatbeltProfileExtensions;
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
use codex_protocol::protocol::ReviewTarget as CoreReviewTarget;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::WarningEvent;
use codex_protocol::request_permissions::RequestPermissionsEvent;
use codex_protocol::request_user_input::RequestUserInputEvent;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;
use toml::Value as TomlValue;
use tracing::warn;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::InProcessAgentContext;
use crate::version::CODEX_CLI_VERSION;

#[cfg(test)]
use codex_app_server_protocol::ChatgptAuthTokensRefreshParams;
#[cfg(test)]
use codex_app_server_protocol::ChatgptAuthTokensRefreshReason;
#[cfg(test)]
use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
#[cfg(test)]
use codex_app_server_protocol::DynamicToolCallParams;
#[cfg(test)]
use codex_app_server_protocol::ExecCommandApprovalParams;
#[cfg(test)]
use codex_app_server_protocol::FileChangeRequestApprovalParams;
#[cfg(test)]
use codex_app_server_protocol::McpElicitationObjectType;
#[cfg(test)]
use codex_app_server_protocol::McpElicitationSchema;
#[cfg(test)]
use codex_app_server_protocol::McpServerElicitationRequestParams;
#[cfg(test)]
use codex_app_server_protocol::PermissionsRequestApprovalParams;
#[cfg(test)]
use codex_app_server_protocol::ToolRequestUserInputOption;
#[cfg(test)]
use codex_app_server_protocol::ToolRequestUserInputParams;
#[cfg(test)]
use codex_app_server_protocol::ToolRequestUserInputQuestion;

const TUI_NOTIFY_CLIENT: &str = "codex-tui";
const HISTORY_FILENAME: &str = "history.jsonl";
const HISTORY_SOFT_CAP_RATIO: f64 = 0.8;
const HISTORY_LOCK_MAX_RETRIES: usize = 10;
const HISTORY_LOCK_RETRY_SLEEP: Duration = Duration::from_millis(100);

/// Interactive request types that the in-process app-server delivers as typed
/// `ServerRequest` variants instead of legacy `codex/event/…` notifications.
///
/// This enum is the single source of truth for the opt-out list passed to the
/// app-server at startup. When a new interactive request type is promoted from
/// the legacy notification path to a typed request, add it here so the
/// app-server stops sending the duplicate legacy notification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InProcessTypedInteractiveRequest {
    ExecApproval,
    ApplyPatchApproval,
    RequestPermissions,
    RequestUserInput,
    McpServerElicitation,
    DynamicToolCall,
}

impl InProcessTypedInteractiveRequest {
    const ALL: [Self; 6] = [
        Self::ExecApproval,
        Self::ApplyPatchApproval,
        Self::RequestPermissions,
        Self::RequestUserInput,
        Self::McpServerElicitation,
        Self::DynamicToolCall,
    ];

    fn legacy_notification_method(self) -> &'static str {
        match self {
            Self::ExecApproval => "codex/event/exec_approval_request",
            Self::ApplyPatchApproval => "codex/event/apply_patch_approval_request",
            Self::RequestPermissions => "codex/event/request_permissions",
            Self::RequestUserInput => "codex/event/request_user_input",
            Self::McpServerElicitation => "codex/event/elicitation_request",
            Self::DynamicToolCall => "codex/event/dynamic_tool_call_request",
        }
    }
}

#[cfg(test)]
fn in_process_typed_interactive_request(
    request: &ServerRequest,
) -> Option<InProcessTypedInteractiveRequest> {
    match request {
        ServerRequest::CommandExecutionRequestApproval { .. }
        | ServerRequest::ExecCommandApproval { .. } => {
            Some(InProcessTypedInteractiveRequest::ExecApproval)
        }
        ServerRequest::FileChangeRequestApproval { .. }
        | ServerRequest::ApplyPatchApproval { .. } => {
            Some(InProcessTypedInteractiveRequest::ApplyPatchApproval)
        }
        ServerRequest::PermissionsRequestApproval { .. } => {
            Some(InProcessTypedInteractiveRequest::RequestPermissions)
        }
        ServerRequest::ToolRequestUserInput { .. } => {
            Some(InProcessTypedInteractiveRequest::RequestUserInput)
        }
        ServerRequest::McpServerElicitationRequest { .. } => {
            Some(InProcessTypedInteractiveRequest::McpServerElicitation)
        }
        ServerRequest::DynamicToolCall { .. } => {
            Some(InProcessTypedInteractiveRequest::DynamicToolCall)
        }
        ServerRequest::ChatgptAuthTokensRefresh { .. } => None,
    }
}

fn in_process_typed_event_legacy_opt_outs() -> Vec<String> {
    InProcessTypedInteractiveRequest::ALL
        .into_iter()
        .map(InProcessTypedInteractiveRequest::legacy_notification_method)
        .map(str::to_string)
        .collect()
}

async fn initialize_app_server_client_name(thread: &CodexThread) {
    if let Err(err) = thread
        .set_app_server_client_name(Some(TUI_NOTIFY_CLIENT.to_string()))
        .await
    {
        tracing::error!("failed to set app server client name: {err}");
    }
}

/// Build the initialization payload for an in-process app-server client from
/// the TUI's runtime state. The resulting client embeds its own app-server
/// instance and communicates over in-memory channels.
fn in_process_start_args(
    config: &Config,
    thread_manager: Arc<ThreadManager>,
    arg0_paths: codex_arg0::Arg0DispatchPaths,
    cli_overrides: Vec<(String, TomlValue)>,
    cloud_requirements: CloudRequirementsLoader,
) -> InProcessClientStartArgs {
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
        arg0_paths,
        config: Arc::new(config.clone()),
        thread_manager: Some(thread_manager),
        cli_overrides,
        loader_overrides: LoaderOverrides::default(),
        cloud_requirements,
        feedback: CodexFeedback::new(),
        config_warnings,
        session_source: SessionSource::Cli,
        enable_codex_api_key_env: false,
        client_name: TUI_NOTIFY_CLIENT.to_string(),
        client_version: CODEX_CLI_VERSION.to_string(),
        experimental_api: true,
        opt_out_notification_methods: in_process_typed_event_legacy_opt_outs(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    }
}

/// Monotonically increasing counter for JSON-RPC request IDs within a single
/// agent session. Each `InProcessAppServerClient` uses its own sequencer so IDs
/// are unique per session but not globally.
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

/// Tracks an outstanding exec-approval server request so the agent can resolve
/// it when the user decides. V1 and V2 correspond to the legacy and current
/// app-server request schemas; the response format differs between them.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingExecApprovalRequest {
    V1(RequestId),
    V2(RequestId),
}

/// Same as [`PendingExecApprovalRequest`] but for file-change (patch) approvals.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingPatchApprovalRequest {
    V1(RequestId),
    V2(RequestId),
}

/// Bookkeeping for server requests that are awaiting a user response.
///
/// When the app-server sends a `ServerRequest` (e.g. an exec approval prompt),
/// the agent records the request ID here. When the TUI user makes a decision
/// and the corresponding `Op` arrives, the agent looks up the request ID and
/// calls `resolve_server_request` / `reject_server_request` to unblock the
/// app-server.
///
/// All fields except `mcp_elicitations` are turn-scoped and cleared on turn
/// completion or abort via [`clear_turn_scoped`](Self::clear_turn_scoped).
#[derive(Default)]
struct PendingServerRequests {
    exec_approvals: HashMap<(String, String), PendingExecApprovalRequest>,
    patch_approvals: HashMap<(String, String), PendingPatchApprovalRequest>,
    mcp_elicitations: HashMap<RequestId, PendingMcpElicitationRequest>,
    request_permissions: HashMap<(String, String), RequestId>,
    request_user_input: HashMap<(String, String), VecDeque<RequestId>>,
    dynamic_tool_calls: HashMap<(String, String), RequestId>,
    pending_file_changes: HashMap<(String, String), HashMap<PathBuf, FileChange>>,
}

struct PendingMcpElicitationRequest {
    thread_id: String,
    server_name: String,
    request_id: codex_protocol::mcp::RequestId,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ThreadScopedOp {
    pub(crate) thread_id: ThreadId,
    pub(crate) op: Op,
    pub(crate) interrupt_turn_id: Option<String>,
}

impl PendingServerRequests {
    fn clear_turn_scoped(&mut self, thread_id: &str) {
        self.exec_approvals
            .retain(|(pending_thread_id, _), _| pending_thread_id != thread_id);
        self.patch_approvals
            .retain(|(pending_thread_id, _), _| pending_thread_id != thread_id);
        // MCP elicitation requests can outlive turn boundaries (turn_id is best-effort),
        // so clear them only via resolve path or serverRequest/resolved notifications.
        self.request_permissions
            .retain(|(pending_thread_id, _), _| pending_thread_id != thread_id);
        self.request_user_input
            .retain(|(pending_thread_id, _), _| pending_thread_id != thread_id);
        self.dynamic_tool_calls
            .retain(|(pending_thread_id, _), _| pending_thread_id != thread_id);
        self.pending_file_changes
            .retain(|(pending_thread_id, _), _| pending_thread_id != thread_id);
    }

    fn register_request_user_input(
        &mut self,
        thread_id: String,
        turn_id: String,
        request_id: RequestId,
    ) {
        self.request_user_input
            .entry((thread_id, turn_id))
            .or_default()
            .push_back(request_id);
    }

    fn note_file_changes(
        &mut self,
        thread_id: String,
        item_id: String,
        changes: HashMap<PathBuf, FileChange>,
    ) {
        self.pending_file_changes
            .insert((thread_id, item_id), changes);
    }

    fn take_file_changes(
        &mut self,
        thread_id: &str,
        item_id: &str,
    ) -> HashMap<PathBuf, FileChange> {
        self.pending_file_changes
            .remove(&(thread_id.to_string(), item_id.to_string()))
            .unwrap_or_default()
    }

    fn pop_request_user_input_request_id(
        &mut self,
        thread_id: &str,
        turn_id: &str,
    ) -> Option<RequestId> {
        let key = (thread_id.to_string(), turn_id.to_string());
        let request_id = self
            .request_user_input
            .get_mut(&key)
            .and_then(VecDeque::pop_front);
        if self
            .request_user_input
            .get(&key)
            .is_some_and(VecDeque::is_empty)
        {
            self.request_user_input.remove(&key);
        }
        request_id
    }

    fn register_mcp_elicitation(
        &mut self,
        thread_id: String,
        pending_request_id: RequestId,
        server_name: String,
        request_id: codex_protocol::mcp::RequestId,
    ) {
        self.mcp_elicitations.insert(
            pending_request_id,
            PendingMcpElicitationRequest {
                thread_id,
                server_name,
                request_id,
            },
        );
    }

    fn pop_mcp_elicitation_request_id(
        &mut self,
        thread_id: &str,
        server_name: &str,
        request_id: &codex_protocol::mcp::RequestId,
    ) -> Option<RequestId> {
        let pending_request_id = self.mcp_elicitations.iter().find_map(
            |(pending_request_id, pending_elicitation)| {
                if pending_elicitation.thread_id == thread_id
                    && pending_elicitation.server_name == server_name
                    && pending_elicitation.request_id == *request_id
                {
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

    fn clear_resolved_request_id(&mut self, thread_id: &str, request_id: &RequestId) {
        self.exec_approvals
            .retain(|(pending_thread_id, _), pending| {
                pending_thread_id != thread_id || pending.request_id() != request_id
            });
        self.patch_approvals
            .retain(|(pending_thread_id, _), pending| {
                pending_thread_id != thread_id || pending.request_id() != request_id
            });
        self.request_permissions
            .retain(|(pending_thread_id, _), pending_request_id| {
                pending_thread_id != thread_id || pending_request_id != request_id
            });
        self.dynamic_tool_calls
            .retain(|(pending_thread_id, _), pending_request_id| {
                pending_thread_id != thread_id || pending_request_id != request_id
            });
        self.request_user_input
            .retain(|(pending_thread_id, _), pending_request_ids| {
                if pending_thread_id != thread_id {
                    return true;
                }
                pending_request_ids.retain(|pending_request_id| pending_request_id != request_id);
                !pending_request_ids.is_empty()
            });
        let remaining_patch_items = self.patch_approvals.keys().cloned().collect::<HashSet<_>>();
        self.pending_file_changes
            .retain(|key, _| key.0 != thread_id || remaining_patch_items.contains(key));
        self.clear_mcp_elicitation_by_request_id(request_id);
    }
}

impl PendingExecApprovalRequest {
    fn request_id(&self) -> &RequestId {
        match self {
            Self::V1(request_id) | Self::V2(request_id) => request_id,
        }
    }
}

impl PendingPatchApprovalRequest {
    fn request_id(&self) -> &RequestId {
        match self {
            Self::V1(request_id) | Self::V2(request_id) => request_id,
        }
    }
}

fn note_primary_legacy_event(
    session_id: ThreadId,
    conversation_id: Option<ThreadId>,
    event: &Event,
    current_turn_ids: &mut HashMap<String, String>,
    pending_server_requests: &mut PendingServerRequests,
) -> bool {
    let event_thread_id = conversation_id.unwrap_or(session_id);
    let event_thread_id_string = event_thread_id.to_string();

    match &event.msg {
        EventMsg::TurnStarted(payload) => {
            current_turn_ids.insert(event_thread_id_string.clone(), payload.turn_id.clone());
        }
        EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_) => {
            current_turn_ids.remove(&event_thread_id_string);
            pending_server_requests.clear_turn_scoped(&event_thread_id_string);
        }
        _ => {}
    }

    event_thread_id == session_id && matches!(event.msg, EventMsg::ShutdownComplete)
}

async fn finalize_in_process_shutdown<Shutdown>(
    shutdown: Shutdown,
    app_event_tx: AppEventSender,
    pending_shutdown_complete: bool,
) where
    Shutdown: Future<Output = std::io::Result<()>> + Send + 'static,
{
    if pending_shutdown_complete {
        let shutdown_app_event_tx = app_event_tx.clone();
        tokio::spawn(async move {
            if let Err(err) = shutdown.await {
                send_warning_event(
                    &shutdown_app_event_tx,
                    format!("in-process app-server shutdown failed: {err}"),
                );
            }
        });
        send_codex_event(&app_event_tx, EventMsg::ShutdownComplete);
        return;
    }

    if let Err(err) = shutdown.await {
        send_warning_event(
            &app_event_tx,
            format!("in-process app-server shutdown failed: {err}"),
        );
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
        macos: value.macos.map(|macos| MacOsSeatbeltProfileExtensions {
            macos_preferences: macos.preferences,
            macos_automation: macos.automations,
            macos_accessibility: macos.accessibility,
            macos_calendar: macos.calendar,
        }),
    }
}

fn granted_permission_profile_from_core(value: PermissionProfile) -> GrantedPermissionProfile {
    let network = value.network.and_then(|network| {
        if network.enabled.unwrap_or(false) {
            Some(codex_app_server_protocol::AdditionalNetworkPermissions {
                enabled: Some(true),
            })
        } else {
            None
        }
    });
    let file_system = value.file_system.and_then(|file_system| {
        if file_system.is_empty() {
            None
        } else {
            Some(codex_app_server_protocol::AdditionalFileSystemPermissions {
                read: file_system.read,
                write: file_system.write,
            })
        }
    });
    let macos = value.macos.and_then(|macos| {
        let preferences = match macos.macos_preferences {
            codex_protocol::models::MacOsPreferencesPermission::None => None,
            preferences => Some(preferences),
        };
        let automations = match macos.macos_automation {
            codex_protocol::models::MacOsAutomationPermission::None => None,
            automations => Some(automations),
        };
        let accessibility = macos.macos_accessibility.then_some(true);
        let calendar = macos.macos_calendar.then_some(true);
        if preferences.is_none()
            && automations.is_none()
            && accessibility.is_none()
            && calendar.is_none()
        {
            None
        } else {
            Some(GrantedMacOsPermissions {
                preferences,
                automations,
                accessibility,
                calendar,
            })
        }
    });

    GrantedPermissionProfile {
        network,
        file_system,
        macos,
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

async fn forward_op_to_thread(
    thread_manager: &ThreadManager,
    thread_id: &str,
    op: Op,
    app_event_tx: &AppEventSender,
) {
    let op_type = serde_json::to_value(&op)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_string());
    let thread_id = match ThreadId::from_string(thread_id) {
        Ok(thread_id) => thread_id,
        Err(err) => {
            send_error_event(
                app_event_tx,
                format!("failed to parse in-process thread id `{thread_id}`: {err}"),
            );
            return;
        }
    };

    if let Err(err) = thread_manager.send_op(thread_id, op).await {
        send_error_event(
            app_event_tx,
            format!("failed to forward `{op_type}` to in-process thread: {err}"),
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
            meta,
            message,
            requested_schema,
        } => codex_protocol::approvals::ElicitationRequest::Form {
            meta,
            message,
            requested_schema: serde_json::to_value(requested_schema).unwrap_or_else(|err| {
                warn!("failed to serialize MCP elicitation schema for local adapter: {err}");
                serde_json::Value::Null
            }),
        },
        McpServerElicitationRequest::Url {
            meta,
            message,
            url,
            elicitation_id,
        } => codex_protocol::approvals::ElicitationRequest::Url {
            meta,
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
    // Open directly and treat NotFound as empty history (no TOCTOU pre-check).
    let file = match OpenOptions::new().read(true).open(path) {
        Ok(f) => f,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("failed to open history file: {err}")),
    };
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
    tokio::task::spawn_blocking(move || read_history_entry_blocking(path, requested_log_id, offset))
        .await
        .map_err(|err| format!("failed to join history read task: {err}"))?
}

async fn local_external_chatgpt_tokens(
    auth_manager: Arc<AuthManager>,
) -> Result<ChatgptAuthTokensRefreshResponse, String> {
    let auth = auth_manager
        .auth_cached()
        .ok_or_else(|| "no cached auth available for local token refresh".to_string())?;
    if !auth.is_external_chatgpt_tokens() {
        return Err("external ChatGPT token auth is not active".to_string());
    }

    let base_external_auth_refresher = auth_manager.replace_external_auth_refresher(None);
    let refresh_result = match base_external_auth_refresher.clone() {
        Some(refresher) => {
            let _override_guard = auth_manager
                .push_external_auth_override(refresher, auth_manager.forced_chatgpt_workspace_id());
            auth_manager.refresh_token_from_authority().await
        }
        None => Err(std::io::Error::other("external auth refresher is not configured").into()),
    };
    let _ = auth_manager.replace_external_auth_refresher(base_external_auth_refresher);
    refresh_result.map_err(|err| format!("failed to refresh external ChatGPT auth: {err}"))?;

    let auth = auth_manager
        .auth_cached()
        .ok_or_else(|| "no cached auth available after local token refresh".to_string())?;
    if !auth.is_external_chatgpt_tokens() {
        return Err("external ChatGPT token auth is not active after refresh".to_string());
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

fn validate_refreshed_chatgpt_account(
    previous_account_id: Option<&str>,
    refreshed_account_id: &str,
) -> Result<(), String> {
    if let Some(previous_account_id) = previous_account_id
        && previous_account_id != refreshed_account_id
    {
        return Err(format!(
            "local auth refresh account mismatch: expected `{previous_account_id}`, got `{refreshed_account_id}`"
        ));
    }
    Ok(())
}

fn send_codex_event(app_event_tx: &AppEventSender, msg: EventMsg) {
    app_event_tx.send(AppEvent::CodexEvent(Event {
        id: String::new(),
        msg,
    }));
}

fn send_routed_codex_event(
    app_event_tx: &AppEventSender,
    session_id: ThreadId,
    event_thread_id: ThreadId,
    msg: EventMsg,
) {
    let event = Event {
        id: String::new(),
        msg,
    };
    if event_thread_id == session_id {
        app_event_tx.send(AppEvent::CodexEvent(event));
    } else {
        app_event_tx.send(AppEvent::ThreadEvent {
            thread_id: event_thread_id,
            event,
        });
    }
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

async fn local_history_metadata(config: &Config) -> (u64, usize) {
    if config.history.persistence == HistoryPersistence::None {
        return (0, 0);
    }

    let path = history_file_path(config);
    let log_id = match tokio::fs::metadata(&path).await {
        Ok(metadata) => history_log_id(&metadata),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return (0, 0),
        Err(_) => return (0, 0),
    };
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(_) => return (log_id, 0),
    };
    let mut buf = [0u8; 8192];
    let mut count = 0usize;
    loop {
        match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                count += buf[..n].iter().filter(|&&b| b == b'\n').count();
            }
            Err(_) => return (log_id, 0),
        }
    }

    (log_id, count)
}

async fn session_configured_from_thread_start_response(
    config: &Config,
    response: ThreadStartResponse,
) -> Result<SessionConfiguredEvent, String> {
    let session_id = ThreadId::from_string(&response.thread.id)
        .map_err(|err| format!("thread/start returned invalid thread id: {err}"))?;
    let (history_log_id, history_entry_count) = local_history_metadata(config).await;

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
        history_log_id,
        history_entry_count,
        initial_messages: None,
        network_proxy: None,
        rollout_path: response.thread.path,
    })
}

fn thread_sandbox_mode(
    sandbox_policy: &codex_protocol::protocol::SandboxPolicy,
) -> codex_app_server_protocol::SandboxMode {
    match sandbox_policy {
        codex_protocol::protocol::SandboxPolicy::DangerFullAccess
        | codex_protocol::protocol::SandboxPolicy::ExternalSandbox { .. } => {
            codex_app_server_protocol::SandboxMode::DangerFullAccess
        }
        codex_protocol::protocol::SandboxPolicy::ReadOnly { .. } => {
            codex_app_server_protocol::SandboxMode::ReadOnly
        }
        codex_protocol::protocol::SandboxPolicy::WorkspaceWrite { .. } => {
            codex_app_server_protocol::SandboxMode::WorkspaceWrite
        }
    }
}

fn thread_start_params_from_config(config: &Config) -> ThreadStartParams {
    ThreadStartParams {
        model: config.model.clone(),
        model_provider: Some(config.model_provider_id.clone()),
        service_tier: Some(config.service_tier),
        cwd: Some(config.cwd.to_string_lossy().into_owned()),
        approval_policy: Some(config.permissions.approval_policy.value().into()),
        sandbox: Some(thread_sandbox_mode(config.permissions.sandbox_policy.get())),
        config: None,
        service_name: None,
        base_instructions: config.base_instructions.clone(),
        developer_instructions: config.developer_instructions.clone(),
        personality: config.personality,
        ephemeral: None,
        dynamic_tools: None,
        mock_experimental_field: None,
        experimental_raw_events: false,
        persist_extended_history: false,
    }
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

struct DecodedLegacyNotification {
    conversation_id: Option<ThreadId>,
    event: Event,
}

fn decode_legacy_notification(
    notification: JSONRPCNotification,
) -> Result<DecodedLegacyNotification, String> {
    let value = notification
        .params
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let method = notification.method;
    let normalized_method = normalize_legacy_notification_method(&method).to_string();
    let serde_json::Value::Object(mut object) = value else {
        return Err(format!(
            "legacy notification `{method}` params were not an object"
        ));
    };
    let conversation_id = object
        .get("conversationId")
        .and_then(serde_json::Value::as_str)
        .map(ThreadId::from_string)
        .transpose()
        .map_err(|err| {
            format!("legacy notification `{method}` has invalid conversationId: {err}")
        })?;
    let event_id = object
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_default();
    let mut event_payload = if let Some(serde_json::Value::Object(msg_payload)) = object.get("msg")
    {
        serde_json::Value::Object(msg_payload.clone())
    } else {
        object.remove("conversationId");
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
    Ok(DecodedLegacyNotification {
        conversation_id,
        event: Event { id: event_id, msg },
    })
}

#[cfg(test)]
fn legacy_notification_to_event(notification: JSONRPCNotification) -> Result<Event, String> {
    decode_legacy_notification(notification).map(|decoded| decoded.event)
}

/// Translate a single TUI `Op` into the corresponding app-server client
/// request. Returns `true` when the op was `Op::Shutdown`, signalling the
/// caller to exit the agent loop.
#[expect(
    clippy::too_many_arguments,
    reason = "migration routing keeps dependencies explicit"
)]
async fn process_in_process_command(
    op: Op,
    thread_id: &str,
    primary_thread_id: &str,
    interrupt_turn_id: Option<&str>,
    session_id: &ThreadId,
    config: &Config,
    current_turn_ids: &mut HashMap<String, String>,
    request_ids: &mut RequestIdSequencer,
    pending_server_requests: &mut PendingServerRequests,
    client: &InProcessAppServerClient,
    thread_manager: &ThreadManager,
    app_event_tx: &AppEventSender,
) -> bool {
    match op {
        Op::Interrupt => {
            let Some(turn_id) = interrupt_turn_id
                .map(str::to_owned)
                .or_else(|| current_turn_ids.get(thread_id).cloned())
            else {
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
        Op::Review { review_request } => {
            let target = match review_request.target {
                CoreReviewTarget::UncommittedChanges => ApiReviewTarget::UncommittedChanges,
                CoreReviewTarget::BaseBranch { branch } => ApiReviewTarget::BaseBranch { branch },
                CoreReviewTarget::Commit { sha, title } => ApiReviewTarget::Commit { sha, title },
                CoreReviewTarget::Custom { instructions } => {
                    ApiReviewTarget::Custom { instructions }
                }
            };
            let request = ClientRequest::ReviewStart {
                request_id: request_ids.next(),
                params: ReviewStartParams {
                    thread_id: thread_id.to_string(),
                    target,
                    delivery: Some(ReviewDelivery::Inline),
                },
            };
            match send_request_with_response::<ReviewStartResponse>(client, request, "review/start")
                .await
            {
                Ok(response) => {
                    current_turn_ids.insert(thread_id.to_string(), response.turn.id);
                }
                Err(err) => send_error_event(app_event_tx, err),
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
                    current_turn_ids.insert(thread_id.to_string(), response.turn.id);
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
                    current_turn_ids.insert(thread_id.to_string(), response.turn.id);
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
                    if let Some(current_turn_id) = active_turn_id_from_turns(&response.thread.turns)
                    {
                        current_turn_ids.insert(thread_id.to_string(), current_turn_id);
                    } else {
                        current_turn_ids.remove(thread_id);
                    }
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
            let Some(pending_request) = pending_server_requests
                .exec_approvals
                .remove(&(thread_id.to_string(), id.clone()))
            else {
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
            let Some(pending_request) = pending_server_requests
                .patch_approvals
                .remove(&(thread_id.to_string(), id.clone()))
            else {
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
        Op::RequestPermissionsResponse { id, response } => {
            let Some(request_id) = pending_server_requests
                .request_permissions
                .remove(&(thread_id.to_string(), id.clone()))
            else {
                send_warning_event(
                    app_event_tx,
                    format!(
                        "request_permissions response ignored because request id `{id}` was not pending"
                    ),
                );
                return false;
            };

            let response = PermissionsRequestApprovalResponse {
                permissions: granted_permission_profile_from_core(response.permissions),
                scope: response.scope.into(),
            };
            let result = match serde_json::to_value(response) {
                Ok(value) => value,
                Err(err) => {
                    send_error_event(
                        app_event_tx,
                        format!("failed to encode request_permissions response: {err}"),
                    );
                    return false;
                }
            };
            resolve_server_request(
                client,
                request_id,
                result,
                "item/permissions/requestApproval",
                app_event_tx,
            )
            .await;
        }
        Op::UserInputAnswer { id, response } => {
            let Some(request_id) =
                pending_server_requests.pop_request_user_input_request_id(thread_id, &id)
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
            let Some(request_id) = pending_server_requests
                .dynamic_tool_calls
                .remove(&(thread_id.to_string(), id.clone()))
            else {
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
            forward_op_to_thread(
                thread_manager,
                thread_id,
                Op::ReloadUserConfig,
                app_event_tx,
            )
            .await;
        }
        Op::Undo => {
            send_warning_event(app_event_tx, local_only_deferred_message("Undo"));
        }
        Op::OverrideTurnContext {
            cwd,
            approval_policy,
            sandbox_policy,
            windows_sandbox_level,
            model,
            effort,
            summary,
            service_tier,
            collaboration_mode,
            personality,
        } => {
            forward_op_to_thread(
                thread_manager,
                thread_id,
                Op::OverrideTurnContext {
                    cwd,
                    approval_policy,
                    sandbox_policy,
                    windows_sandbox_level,
                    model,
                    effort,
                    summary,
                    service_tier,
                    collaboration_mode,
                    personality,
                },
                app_event_tx,
            )
            .await;
        }
        Op::DropMemories => {
            forward_op_to_thread(thread_manager, thread_id, Op::DropMemories, app_event_tx).await;
        }
        Op::UpdateMemories => {
            forward_op_to_thread(thread_manager, thread_id, Op::UpdateMemories, app_event_tx).await;
        }
        Op::RunUserShellCommand { command } => {
            forward_op_to_thread(
                thread_manager,
                thread_id,
                Op::RunUserShellCommand { command },
                app_event_tx,
            )
            .await;
        }
        Op::ListMcpTools => {
            forward_op_to_thread(thread_manager, thread_id, Op::ListMcpTools, app_event_tx).await;
        }
        Op::ResolveElicitation {
            server_name,
            request_id,
            decision,
            content,
            meta,
        } => {
            let Some(pending_request_id) = pending_server_requests.pop_mcp_elicitation_request_id(
                thread_id,
                &server_name,
                &request_id,
            ) else {
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
                meta,
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
            current_turn_ids.remove(thread_id);
            pending_server_requests.clear_turn_scoped(thread_id);
            return thread_id == primary_thread_id;
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
/// `ClientRequest`/`ClientNotification` messages, routes a small legacy subset
/// straight to the backing thread manager, translates server requests back
/// into UI events, and preserves thread-local bookkeeping such as current turn
/// id and pending approval state.
async fn run_in_process_agent_loop(
    mut codex_op_rx: tokio::sync::mpsc::UnboundedReceiver<Op>,
    mut thread_scoped_op_rx: tokio::sync::mpsc::UnboundedReceiver<ThreadScopedOp>,
    mut client: InProcessAppServerClient,
    thread_manager: Arc<ThreadManager>,
    config: Config,
    thread_id: String,
    mut session_configured: SessionConfiguredEvent,
    app_event_tx: AppEventSender,
    mut request_ids: RequestIdSequencer,
    mut current_turn_ids: HashMap<String, String>,
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
                            &thread_id,
                            None,
                            &session_id,
                            &config,
                            &mut current_turn_ids,
                            &mut request_ids,
                            &mut pending_server_requests,
                            &client,
                            thread_manager.as_ref(),
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
            maybe_thread_scoped_op = thread_scoped_op_rx.recv() => {
                match maybe_thread_scoped_op {
                    Some(ThreadScopedOp { thread_id: scoped_thread_id, op, interrupt_turn_id }) => {
                        let scoped_thread_id = scoped_thread_id.to_string();
                        let should_shutdown = process_in_process_command(
                            op,
                            &scoped_thread_id,
                            &thread_id,
                            interrupt_turn_id.as_deref(),
                            &session_id,
                            &config,
                            &mut current_turn_ids,
                            &mut request_ids,
                            &mut pending_server_requests,
                            &client,
                            thread_manager.as_ref(),
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
                                let Ok(request_thread_id) = ThreadId::from_string(&params.thread_id) else {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!("request carried invalid thread id `{}`", params.thread_id),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                };

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
                                    (params.thread_id.clone(), approval_id),
                                    PendingExecApprovalRequest::V2(request_id),
                                );
                                send_routed_codex_event(
                                    &app_event_tx,
                                    session_id,
                                    request_thread_id,
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
                                        skill_metadata: params
                                            .skill_metadata
                                            .map(|metadata| {
                                                codex_protocol::protocol::ExecApprovalRequestSkillMetadata {
                                                    path_to_skills_md: metadata.path_to_skills_md,
                                                }
                                            }),
                                        available_decisions: command_execution_available_decisions_to_core(
                                            params.available_decisions,
                                        ),
                                        parsed_cmd,
                                    }),
                                );
                            }
                            ServerRequest::ExecCommandApproval { request_id, params } => {
                                let approval_id = params
                                    .approval_id
                                    .clone()
                                    .unwrap_or_else(|| params.call_id.clone());
                                pending_server_requests.exec_approvals.insert(
                                    (params.conversation_id.to_string(), approval_id),
                                    PendingExecApprovalRequest::V1(request_id),
                                );
                                send_routed_codex_event(
                                    &app_event_tx,
                                    session_id,
                                    params.conversation_id,
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
                                        skill_metadata: None,
                                        available_decisions: None,
                                        parsed_cmd: params.parsed_cmd,
                                    }),
                                );
                            }
                            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                                let Ok(request_thread_id) = ThreadId::from_string(&params.thread_id) else {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!("request carried invalid thread id `{}`", params.thread_id),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                };

                                let changes = pending_server_requests
                                    .take_file_changes(&params.thread_id, &params.item_id);
                                pending_server_requests.patch_approvals.insert(
                                    (params.thread_id.clone(), params.item_id.clone()),
                                    PendingPatchApprovalRequest::V2(request_id),
                                );
                                send_routed_codex_event(
                                    &app_event_tx,
                                    session_id,
                                    request_thread_id,
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
                                pending_server_requests.patch_approvals.insert(
                                    (params.conversation_id.to_string(), params.call_id.clone()),
                                    PendingPatchApprovalRequest::V1(request_id),
                                );
                                send_routed_codex_event(
                                    &app_event_tx,
                                    session_id,
                                    params.conversation_id,
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
                                let Ok(request_thread_id) = ThreadId::from_string(&params.thread_id) else {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!("request carried invalid thread id `{}`", params.thread_id),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                };

                                pending_server_requests
                                    .register_request_user_input(
                                        params.thread_id.clone(),
                                        params.turn_id.clone(),
                                        request_id,
                                    );
                                send_routed_codex_event(
                                    &app_event_tx,
                                    session_id,
                                    request_thread_id,
                                    EventMsg::RequestUserInput(RequestUserInputEvent {
                                        call_id: params.item_id,
                                        turn_id: params.turn_id,
                                        questions: request_user_input_questions_to_core(
                                            params.questions,
                                        ),
                                    }),
                                );
                            }
                            ServerRequest::PermissionsRequestApproval { request_id, params } => {
                                let Ok(request_thread_id) = ThreadId::from_string(&params.thread_id) else {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!("request carried invalid thread id `{}`", params.thread_id),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                };

                                pending_server_requests
                                    .request_permissions
                                    .insert((params.thread_id.clone(), params.item_id.clone()), request_id);
                                send_routed_codex_event(
                                    &app_event_tx,
                                    session_id,
                                    request_thread_id,
                                    EventMsg::RequestPermissions(RequestPermissionsEvent {
                                        call_id: params.item_id,
                                        turn_id: params.turn_id,
                                        reason: params.reason,
                                        permissions: additional_permission_profile_to_core(
                                            params.permissions,
                                        ),
                                    }),
                                );
                            }
                            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                                let Ok(request_thread_id) = ThreadId::from_string(&params.thread_id) else {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!("request carried invalid thread id `{}`", params.thread_id),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                };

                                let elicitation_id = app_server_request_id_to_mcp(request_id.clone());
                                pending_server_requests.register_mcp_elicitation(
                                    params.thread_id.clone(),
                                    request_id,
                                    params.server_name.clone(),
                                    elicitation_id.clone(),
                                );
                                send_routed_codex_event(
                                    &app_event_tx,
                                    session_id,
                                    request_thread_id,
                                    EventMsg::ElicitationRequest(ElicitationRequestEvent {
                                        turn_id: params.turn_id,
                                        server_name: params.server_name,
                                        id: elicitation_id,
                                        request: mcp_elicitation_request_to_core(params.request),
                                    }),
                                );
                            }
                            ServerRequest::DynamicToolCall { request_id, params } => {
                                let Ok(request_thread_id) = ThreadId::from_string(&params.thread_id) else {
                                    reject_server_request(
                                        &client,
                                        request_id,
                                        &method,
                                        format!("request carried invalid thread id `{}`", params.thread_id),
                                        &app_event_tx,
                                    )
                                    .await;
                                    continue;
                                };

                                pending_server_requests
                                    .dynamic_tool_calls
                                    .insert((params.thread_id.clone(), params.call_id.clone()), request_id);
                                send_routed_codex_event(
                                    &app_event_tx,
                                    session_id,
                                    request_thread_id,
                                    EventMsg::DynamicToolCallRequest(DynamicToolCallRequest {
                                        call_id: params.call_id,
                                        turn_id: params.turn_id,
                                        tool: params.tool,
                                        arguments: params.arguments,
                                    }),
                                );
                            }
                            ServerRequest::ChatgptAuthTokensRefresh { request_id, params } => {
                                match local_external_chatgpt_tokens(thread_manager.auth_manager())
                                    .await
                                {
                                    Err(reason) => {
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
                                    Ok(response) => {
                                        if let Err(reason) = validate_refreshed_chatgpt_account(
                                            params.previous_account_id.as_deref(),
                                            &response.chatgpt_account_id,
                                        ) {
                                            send_warning_event(&app_event_tx, reason.clone());
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
                            ServerNotification::ItemStarted(notification) => {
                                if let ThreadItem::FileChange { id, changes, .. } = notification.item
                                {
                                    pending_server_requests
                                        .note_file_changes(
                                            notification.thread_id,
                                            id,
                                            file_update_changes_to_core(changes),
                                        );
                                }
                            }
                            ServerNotification::ServerRequestResolved(notification) => {
                                pending_server_requests.clear_resolved_request_id(
                                    &notification.thread_id,
                                    &notification.request_id,
                                );
                            }
                            ServerNotification::ThreadClosed(notification) => {
                                let Ok(closed_thread_id) =
                                    ThreadId::from_string(&notification.thread_id)
                                else {
                                    send_warning_event(
                                        &app_event_tx,
                                        format!(
                                            "thread/closed carried invalid thread id `{}`",
                                            notification.thread_id
                                        ),
                                    );
                                    continue;
                                };
                                current_turn_ids.remove(&notification.thread_id);
                                pending_server_requests.clear_turn_scoped(&notification.thread_id);
                                if closed_thread_id == session_id {
                                    pending_shutdown_complete = true;
                                    break;
                                }
                                app_event_tx.send(AppEvent::ThreadEvent {
                                    thread_id: closed_thread_id,
                                    event: Event {
                                        id: String::new(),
                                        msg: EventMsg::ShutdownComplete,
                                    },
                                });
                            }
                            _ => {}
                        }
                    }
                    InProcessServerEvent::LegacyNotification(notification) => {
                        let decoded = match decode_legacy_notification(notification) {
                            Ok(decoded) => decoded,
                            Err(err) => {
                                send_warning_event(&app_event_tx, err);
                                continue;
                            }
                        };
                        let event = decoded.event;
                        if let EventMsg::SessionConfigured(update) = event.msg {
                            match decoded.conversation_id {
                                Some(thread_id) if thread_id != session_id => {
                                    app_event_tx.send(AppEvent::ThreadEvent {
                                        thread_id,
                                        event: Event {
                                            id: event.id,
                                            msg: EventMsg::SessionConfigured(update),
                                        },
                                    });
                                }
                                _ => {
                                    if let Some(merged) =
                                        merge_session_configured_update(&session_configured, update)
                                    {
                                        session_configured = merged.clone();
                                        app_event_tx.send(AppEvent::CodexEvent(Event {
                                            id: event.id,
                                            msg: EventMsg::SessionConfigured(merged),
                                        }));
                                    }
                                }
                            }
                            continue;
                        }

                        let shutdown_complete = note_primary_legacy_event(
                            session_id,
                            decoded.conversation_id,
                            &event,
                            &mut current_turn_ids,
                            &mut pending_server_requests,
                        );
                        if shutdown_complete {
                            pending_shutdown_complete = true;
                            break;
                        }
                        match decoded.conversation_id {
                            Some(thread_id) if thread_id != session_id => {
                                app_event_tx.send(AppEvent::ThreadEvent { thread_id, event });
                            }
                            _ => {
                                app_event_tx.send(AppEvent::CodexEvent(event));
                            }
                        }
                    }
                    InProcessServerEvent::Lagged { skipped } => {
                        send_warning_event(&app_event_tx, lagged_event_warning_message(skipped));
                    }
                }
            }
        }
    }

    finalize_in_process_shutdown(client.shutdown(), app_event_tx, pending_shutdown_complete).await;
}

/// Spawn the agent bootstrapper and op forwarding loop, returning the
/// `UnboundedSender<Op>` used by the UI to submit operations.
pub(crate) fn spawn_agent(
    config: Config,
    app_event_tx: AppEventSender,
    server: Arc<ThreadManager>,
    in_process_context: InProcessAgentContext,
) -> (UnboundedSender<Op>, UnboundedSender<ThreadScopedOp>) {
    let (codex_op_tx, codex_op_rx) = unbounded_channel::<Op>();
    let (thread_scoped_op_tx, thread_scoped_op_rx) = unbounded_channel::<ThreadScopedOp>();

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        let mut request_ids = RequestIdSequencer::new();
        let thread_manager = Arc::clone(&server);
        let client = match InProcessAppServerClient::start(in_process_start_args(
            &config,
            server,
            in_process_context.arg0_paths,
            in_process_context.cli_kv_overrides,
            in_process_context.cloud_requirements,
        ))
        .await
        {
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
                params: thread_start_params_from_config(&config),
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

        let session_configured =
            match session_configured_from_thread_start_response(&config, thread_start).await {
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
            thread_scoped_op_rx,
            client,
            thread_manager,
            config,
            thread_id,
            session_configured,
            app_event_tx_clone,
            request_ids,
            HashMap::new(),
        )
        .await;
    });

    (codex_op_tx, thread_scoped_op_tx)
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
    use async_trait::async_trait;
    use base64::Engine;
    use codex_core::auth::ExternalAuthRefreshContext;
    use codex_core::auth::ExternalAuthRefresher;
    use codex_core::auth::ExternalAuthTokens;
    use codex_core::auth::login_with_chatgpt_auth_tokens;
    use codex_core::config::ConfigBuilder;
    use codex_protocol::protocol::ConversationAudioParams;
    use codex_protocol::protocol::ConversationStartParams;
    use codex_protocol::protocol::ConversationTextParams;
    use codex_protocol::protocol::RealtimeAudioFrame;
    use codex_protocol::protocol::TurnCompleteEvent;
    use codex_protocol::protocol::TurnStartedEvent;
    use pretty_assertions::assert_eq;
    use std::sync::Mutex;
    use tempfile::TempDir;
    use tokio::sync::mpsc::unbounded_channel;
    use tokio::sync::oneshot;
    use tokio::time::Duration;
    use tokio::time::timeout;

    async fn test_config() -> Config {
        ConfigBuilder::default()
            .codex_home(std::env::temp_dir())
            .build()
            .await
            .expect("config")
    }

    fn test_thread_manager(config: &Config) -> Arc<ThreadManager> {
        Arc::new(
            codex_core::test_support::thread_manager_with_models_provider_and_home(
                codex_core::CodexAuth::from_api_key("test"),
                config.model_provider.clone(),
                config.codex_home.clone(),
            ),
        )
    }

    struct RecordingExternalAuthRefresher {
        refreshed: ExternalAuthTokens,
        contexts: Mutex<Vec<ExternalAuthRefreshContext>>,
    }

    #[async_trait]
    impl ExternalAuthRefresher for RecordingExternalAuthRefresher {
        async fn refresh(
            &self,
            context: ExternalAuthRefreshContext,
        ) -> std::io::Result<ExternalAuthTokens> {
            self.contexts.lock().expect("contexts mutex").push(context);
            Ok(self.refreshed.clone())
        }
    }

    struct FailingExternalAuthRefresher;

    #[async_trait]
    impl ExternalAuthRefresher for FailingExternalAuthRefresher {
        async fn refresh(
            &self,
            _context: ExternalAuthRefreshContext,
        ) -> std::io::Result<ExternalAuthTokens> {
            Err(std::io::Error::other(
                "override refresher should not be used during local refresh",
            ))
        }
    }

    async fn assert_realtime_op_reports_expected_method(op: Op, expected_method: &str) {
        let config = test_config().await;
        let session_id = ThreadId::new();
        let thread_manager = test_thread_manager(&config);
        let client = InProcessAppServerClient::start(in_process_start_args(
            &config,
            Arc::clone(&thread_manager),
            codex_arg0::Arg0DispatchPaths::default(),
            Vec::new(),
            CloudRequirementsLoader::default(),
        ))
        .await
        .expect("in-process app-server client");
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let mut current_turn_ids = HashMap::new();
        let mut request_ids = RequestIdSequencer::new();
        let mut pending_server_requests = PendingServerRequests::default();

        let should_shutdown = process_in_process_command(
            op,
            "missing-thread-id",
            "missing-thread-id",
            None,
            &session_id,
            &config,
            &mut current_turn_ids,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            thread_manager.as_ref(),
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
        Arc<ThreadManager>,
        ThreadId,
    ) {
        let thread_manager = test_thread_manager(config);
        let client = InProcessAppServerClient::start(in_process_start_args(
            config,
            Arc::clone(&thread_manager),
            codex_arg0::Arg0DispatchPaths::default(),
            Vec::new(),
            CloudRequirementsLoader::default(),
        ))
        .await
        .expect("in-process app-server client");
        let (tx, rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let mut current_turn_ids = HashMap::new();
        let mut request_ids = RequestIdSequencer::new();
        let mut pending_server_requests = PendingServerRequests::default();
        let thread_start = send_request_with_response::<ThreadStartResponse>(
            &client,
            ClientRequest::ThreadStart {
                request_id: request_ids.next(),
                params: ThreadStartParams::default(),
            },
            "thread/start",
        )
        .await
        .expect("thread/start");
        let thread_id = thread_start.thread.id;
        let session_id = ThreadId::from_string(&thread_id).expect("valid thread id");
        let should_shutdown = process_in_process_command(
            op,
            &thread_id,
            &thread_id,
            None,
            &session_id,
            config,
            &mut current_turn_ids,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            thread_manager.as_ref(),
            &app_event_tx,
        )
        .await;
        (should_shutdown, rx, client, thread_manager, session_id)
    }

    #[tokio::test]
    async fn pending_shutdown_complete_emits_shutdown_complete_before_shutdown_finishes() {
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let (shutdown_started_tx, shutdown_started_rx) = oneshot::channel();
        let (shutdown_release_tx, shutdown_release_rx) = oneshot::channel();

        finalize_in_process_shutdown(
            async move {
                let _ = shutdown_started_tx.send(());
                let _ = shutdown_release_rx.await;
                Ok(())
            },
            app_event_tx,
            true,
        )
        .await;

        timeout(Duration::from_secs(2), shutdown_started_rx)
            .await
            .expect("timed out waiting for background shutdown to start")
            .expect("expected background shutdown start signal");

        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for shutdown complete event")
            .expect("expected app event");
        let AppEvent::CodexEvent(event) = event else {
            panic!("expected codex event");
        };
        assert!(
            matches!(event.msg, EventMsg::ShutdownComplete),
            "expected shutdown complete event"
        );
        assert!(rx.try_recv().is_err(), "expected no additional app events");

        let _ = shutdown_release_tx.send(());
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

    #[test]
    fn send_routed_codex_event_keeps_primary_thread_events_on_primary_bus() {
        let session_id = ThreadId::new();
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);

        send_routed_codex_event(
            &app_event_tx,
            session_id,
            session_id,
            EventMsg::Warning(WarningEvent {
                message: "primary".to_string(),
            }),
        );

        let event = rx.try_recv().expect("expected app event");
        let AppEvent::CodexEvent(event) = event else {
            panic!("expected primary codex event");
        };
        let EventMsg::Warning(warning) = event.msg else {
            panic!("expected warning event");
        };
        assert_eq!(warning.message, "primary".to_string());
    }

    #[test]
    fn send_routed_codex_event_routes_child_thread_events() {
        let session_id = ThreadId::new();
        let child_thread_id = ThreadId::new();
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);

        send_routed_codex_event(
            &app_event_tx,
            session_id,
            child_thread_id,
            EventMsg::Warning(WarningEvent {
                message: "child".to_string(),
            }),
        );

        let event = rx.try_recv().expect("expected app event");
        let AppEvent::ThreadEvent { thread_id, event } = event else {
            panic!("expected routed thread event");
        };
        assert_eq!(thread_id, child_thread_id);
        let EventMsg::Warning(warning) = event.msg else {
            panic!("expected warning event");
        };
        assert_eq!(warning.message, "child".to_string());
    }

    #[test]
    fn send_routed_codex_event_routes_child_dynamic_tool_call_requests() {
        let session_id = ThreadId::new();
        let child_thread_id = ThreadId::new();
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);

        send_routed_codex_event(
            &app_event_tx,
            session_id,
            child_thread_id,
            EventMsg::DynamicToolCallRequest(DynamicToolCallRequest {
                call_id: "call-1".to_string(),
                turn_id: "turn-1".to_string(),
                tool: "demo".to_string(),
                arguments: serde_json::json!({ "value": 1 }),
            }),
        );

        let event = rx.try_recv().expect("expected app event");
        let AppEvent::ThreadEvent { thread_id, event } = event else {
            panic!("expected routed thread event");
        };
        assert_eq!(thread_id, child_thread_id);
        let EventMsg::DynamicToolCallRequest(request) = event.msg else {
            panic!("expected dynamic tool call request event");
        };
        assert_eq!(request.call_id, "call-1".to_string());
        assert_eq!(request.turn_id, "turn-1".to_string());
        assert_eq!(request.tool, "demo".to_string());
        assert_eq!(request.arguments, serde_json::json!({ "value": 1 }));
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
        let thread_id = "thread-1".to_string();
        let pending_request_id = RequestId::Integer(42);
        let server_name = "test-server".to_string();
        let elicitation_id = codex_protocol::mcp::RequestId::Integer(7);
        pending.register_mcp_elicitation(
            thread_id.clone(),
            pending_request_id.clone(),
            server_name.clone(),
            elicitation_id.clone(),
        );

        pending.clear_turn_scoped(&thread_id);

        assert_eq!(
            pending.pop_mcp_elicitation_request_id(&thread_id, &server_name, &elicitation_id),
            Some(pending_request_id)
        );
    }

    #[test]
    fn clear_turn_scoped_only_clears_requests_for_target_thread() {
        let mut pending = PendingServerRequests::default();
        pending.exec_approvals.insert(
            ("thread-a".to_string(), "exec-a".to_string()),
            PendingExecApprovalRequest::V2(RequestId::Integer(1)),
        );
        pending.exec_approvals.insert(
            ("thread-b".to_string(), "exec-b".to_string()),
            PendingExecApprovalRequest::V2(RequestId::Integer(2)),
        );
        pending.request_permissions.insert(
            ("thread-a".to_string(), "perm-a".to_string()),
            RequestId::Integer(3),
        );
        pending.request_permissions.insert(
            ("thread-b".to_string(), "perm-b".to_string()),
            RequestId::Integer(4),
        );
        pending.register_request_user_input(
            "thread-a".to_string(),
            "turn-a".to_string(),
            RequestId::Integer(5),
        );
        pending.register_request_user_input(
            "thread-b".to_string(),
            "turn-b".to_string(),
            RequestId::Integer(6),
        );
        pending.dynamic_tool_calls.insert(
            ("thread-a".to_string(), "tool-a".to_string()),
            RequestId::Integer(7),
        );
        pending.dynamic_tool_calls.insert(
            ("thread-b".to_string(), "tool-b".to_string()),
            RequestId::Integer(8),
        );

        pending.clear_turn_scoped("thread-a");

        assert_eq!(
            pending.exec_approvals,
            HashMap::from([(
                ("thread-b".to_string(), "exec-b".to_string()),
                PendingExecApprovalRequest::V2(RequestId::Integer(2)),
            )])
        );
        assert_eq!(
            pending.request_permissions,
            HashMap::from([(
                ("thread-b".to_string(), "perm-b".to_string()),
                RequestId::Integer(4),
            )])
        );
        assert_eq!(
            pending.request_user_input,
            HashMap::from([(
                ("thread-b".to_string(), "turn-b".to_string()),
                VecDeque::from([RequestId::Integer(6)]),
            )])
        );
        assert_eq!(
            pending.dynamic_tool_calls,
            HashMap::from([(
                ("thread-b".to_string(), "tool-b".to_string()),
                RequestId::Integer(8),
            )])
        );
    }

    #[test]
    fn child_legacy_turn_events_do_not_mutate_primary_turn_state() {
        let session_id = ThreadId::new();
        let child_thread_id = ThreadId::new();
        let mut current_turn_ids =
            HashMap::from([(session_id.to_string(), "primary-turn".to_string())]);
        let mut pending_server_requests = PendingServerRequests::default();
        pending_server_requests.exec_approvals.insert(
            (session_id.to_string(), "exec-1".to_string()),
            PendingExecApprovalRequest::V2(RequestId::Integer(1)),
        );
        pending_server_requests.request_permissions.insert(
            (session_id.to_string(), "perm-1".to_string()),
            RequestId::Integer(2),
        );
        pending_server_requests.register_request_user_input(
            session_id.to_string(),
            "primary-turn".to_string(),
            RequestId::Integer(3),
        );
        pending_server_requests.dynamic_tool_calls.insert(
            (session_id.to_string(), "tool-1".to_string()),
            RequestId::Integer(4),
        );

        let child_turn_started = Event {
            id: "child-turn-started".to_string(),
            msg: EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "child-turn".to_string(),
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            }),
        };
        assert_eq!(
            note_primary_legacy_event(
                session_id,
                Some(child_thread_id),
                &child_turn_started,
                &mut current_turn_ids,
                &mut pending_server_requests,
            ),
            false
        );
        assert_eq!(
            current_turn_ids.get(&session_id.to_string()),
            Some(&"primary-turn".to_string())
        );
        assert_eq!(
            current_turn_ids.get(&child_thread_id.to_string()),
            Some(&"child-turn".to_string())
        );
        assert_eq!(
            pending_server_requests.exec_approvals.len(),
            1,
            "child turn start should not clear primary exec approvals"
        );
        assert_eq!(
            pending_server_requests
                .request_permissions
                .get(&(session_id.to_string(), "perm-1".to_string())),
            Some(&RequestId::Integer(2))
        );
        assert_eq!(
            pending_server_requests
                .request_user_input
                .get(&(session_id.to_string(), "primary-turn".to_string()))
                .map(VecDeque::len),
            Some(1)
        );
        assert_eq!(
            pending_server_requests
                .dynamic_tool_calls
                .get(&(session_id.to_string(), "tool-1".to_string())),
            Some(&RequestId::Integer(4))
        );

        let child_turn_complete = Event {
            id: "child-turn-complete".to_string(),
            msg: EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: "child-turn".to_string(),
                last_agent_message: None,
            }),
        };
        assert_eq!(
            note_primary_legacy_event(
                session_id,
                Some(child_thread_id),
                &child_turn_complete,
                &mut current_turn_ids,
                &mut pending_server_requests,
            ),
            false
        );
        assert_eq!(
            current_turn_ids.get(&session_id.to_string()),
            Some(&"primary-turn".to_string())
        );
        assert_eq!(current_turn_ids.get(&child_thread_id.to_string()), None);
        assert_eq!(
            pending_server_requests.exec_approvals.len(),
            1,
            "child turn completion should not clear primary exec approvals"
        );
        assert_eq!(
            pending_server_requests
                .request_permissions
                .get(&(session_id.to_string(), "perm-1".to_string())),
            Some(&RequestId::Integer(2))
        );
        assert_eq!(
            pending_server_requests
                .request_user_input
                .get(&(session_id.to_string(), "primary-turn".to_string()))
                .map(VecDeque::len),
            Some(1)
        );
        assert_eq!(
            pending_server_requests
                .dynamic_tool_calls
                .get(&(session_id.to_string(), "tool-1".to_string())),
            Some(&RequestId::Integer(4))
        );
    }

    #[test]
    fn primary_turn_completion_preserves_child_pending_requests() {
        let session_id = ThreadId::new();
        let child_thread_id = ThreadId::new();
        let mut current_turn_ids = HashMap::from([
            (session_id.to_string(), "primary-turn".to_string()),
            (child_thread_id.to_string(), "child-turn".to_string()),
        ]);
        let mut pending_server_requests = PendingServerRequests::default();
        pending_server_requests.exec_approvals.insert(
            (session_id.to_string(), "exec-primary".to_string()),
            PendingExecApprovalRequest::V2(RequestId::Integer(1)),
        );
        pending_server_requests.exec_approvals.insert(
            (child_thread_id.to_string(), "exec-child".to_string()),
            PendingExecApprovalRequest::V2(RequestId::Integer(2)),
        );
        pending_server_requests.request_permissions.insert(
            (session_id.to_string(), "perm-primary".to_string()),
            RequestId::Integer(3),
        );
        pending_server_requests.request_permissions.insert(
            (child_thread_id.to_string(), "perm-child".to_string()),
            RequestId::Integer(4),
        );
        pending_server_requests.register_request_user_input(
            session_id.to_string(),
            "primary-turn".to_string(),
            RequestId::Integer(5),
        );
        pending_server_requests.register_request_user_input(
            child_thread_id.to_string(),
            "child-turn".to_string(),
            RequestId::Integer(6),
        );

        assert_eq!(
            note_primary_legacy_event(
                session_id,
                None,
                &Event {
                    id: "primary-turn-complete".to_string(),
                    msg: EventMsg::TurnComplete(TurnCompleteEvent {
                        turn_id: "primary-turn".to_string(),
                        last_agent_message: None,
                    }),
                },
                &mut current_turn_ids,
                &mut pending_server_requests,
            ),
            false
        );

        assert_eq!(current_turn_ids.get(&session_id.to_string()), None);
        assert_eq!(
            current_turn_ids.get(&child_thread_id.to_string()),
            Some(&"child-turn".to_string())
        );
        assert_eq!(
            pending_server_requests.exec_approvals,
            HashMap::from([(
                (child_thread_id.to_string(), "exec-child".to_string()),
                PendingExecApprovalRequest::V2(RequestId::Integer(2)),
            )])
        );
        assert_eq!(
            pending_server_requests.request_permissions,
            HashMap::from([(
                (child_thread_id.to_string(), "perm-child".to_string()),
                RequestId::Integer(4),
            )])
        );
        assert_eq!(
            pending_server_requests.request_user_input,
            HashMap::from([(
                (child_thread_id.to_string(), "child-turn".to_string()),
                VecDeque::from([RequestId::Integer(6)]),
            )])
        );
    }

    #[test]
    fn pending_file_changes_are_scoped_by_thread() {
        let mut pending_server_requests = PendingServerRequests::default();
        let item_id = "patch-1";
        let main_thread_id = "thread-main";
        let child_thread_id = "thread-child";
        let main_changes = HashMap::from([(
            PathBuf::from("main.txt"),
            FileChange::Add {
                content: "main".to_string(),
            },
        )]);
        let child_changes = HashMap::from([(
            PathBuf::from("child.txt"),
            FileChange::Add {
                content: "child".to_string(),
            },
        )]);

        pending_server_requests.note_file_changes(
            main_thread_id.to_string(),
            item_id.to_string(),
            main_changes.clone(),
        );
        pending_server_requests.note_file_changes(
            child_thread_id.to_string(),
            item_id.to_string(),
            child_changes.clone(),
        );

        assert_eq!(
            pending_server_requests.take_file_changes(child_thread_id, item_id),
            child_changes
        );
        assert_eq!(
            pending_server_requests.take_file_changes(main_thread_id, item_id),
            main_changes
        );
    }

    #[test]
    fn server_request_resolved_clears_pending_mcp_elicitation_request() {
        let mut pending = PendingServerRequests::default();
        let thread_id = "thread-1".to_string();
        let pending_request_id = RequestId::Integer(5);
        let server_name = "test-server".to_string();
        let elicitation_id = codex_protocol::mcp::RequestId::String("abc".to_string());
        pending.register_mcp_elicitation(
            thread_id.clone(),
            pending_request_id.clone(),
            server_name.clone(),
            elicitation_id.clone(),
        );

        pending.clear_mcp_elicitation_by_request_id(&pending_request_id);

        assert_eq!(
            pending.pop_mcp_elicitation_request_id(&thread_id, &server_name, &elicitation_id),
            None
        );
    }

    #[test]
    fn server_request_resolved_clears_pending_request_permissions_and_user_input() {
        let mut pending = PendingServerRequests::default();
        pending.request_permissions.insert(
            ("thread-1".to_string(), "perm-1".to_string()),
            RequestId::Integer(5),
        );
        pending.register_request_user_input(
            "thread-1".to_string(),
            "turn-1".to_string(),
            RequestId::Integer(6),
        );
        pending.register_request_user_input(
            "thread-1".to_string(),
            "turn-1".to_string(),
            RequestId::Integer(7),
        );

        pending.clear_resolved_request_id("thread-1", &RequestId::Integer(5));
        pending.clear_resolved_request_id("thread-1", &RequestId::Integer(6));

        assert_eq!(pending.request_permissions.len(), 0);
        assert_eq!(
            pending.pop_request_user_input_request_id("thread-1", "turn-1"),
            Some(RequestId::Integer(7))
        );
    }

    #[test]
    fn pending_request_lookups_are_scoped_by_thread() {
        let mut pending = PendingServerRequests::default();

        pending.exec_approvals.insert(
            ("thread-a".to_string(), "exec-1".to_string()),
            PendingExecApprovalRequest::V2(RequestId::Integer(1)),
        );
        pending.exec_approvals.insert(
            ("thread-b".to_string(), "exec-1".to_string()),
            PendingExecApprovalRequest::V2(RequestId::Integer(2)),
        );
        pending.patch_approvals.insert(
            ("thread-a".to_string(), "patch-1".to_string()),
            PendingPatchApprovalRequest::V2(RequestId::Integer(3)),
        );
        pending.patch_approvals.insert(
            ("thread-b".to_string(), "patch-1".to_string()),
            PendingPatchApprovalRequest::V2(RequestId::Integer(4)),
        );
        pending.request_permissions.insert(
            ("thread-a".to_string(), "perm-1".to_string()),
            RequestId::Integer(5),
        );
        pending.request_permissions.insert(
            ("thread-b".to_string(), "perm-1".to_string()),
            RequestId::Integer(6),
        );
        pending.register_request_user_input(
            "thread-a".to_string(),
            "turn-1".to_string(),
            RequestId::Integer(7),
        );
        pending.register_request_user_input(
            "thread-b".to_string(),
            "turn-1".to_string(),
            RequestId::Integer(8),
        );
        pending.dynamic_tool_calls.insert(
            ("thread-a".to_string(), "tool-1".to_string()),
            RequestId::Integer(9),
        );
        pending.dynamic_tool_calls.insert(
            ("thread-b".to_string(), "tool-1".to_string()),
            RequestId::Integer(10),
        );
        pending.register_mcp_elicitation(
            "thread-a".to_string(),
            RequestId::Integer(11),
            "server".to_string(),
            codex_protocol::mcp::RequestId::Integer(12),
        );
        pending.register_mcp_elicitation(
            "thread-b".to_string(),
            RequestId::Integer(13),
            "server".to_string(),
            codex_protocol::mcp::RequestId::Integer(12),
        );

        assert_eq!(
            pending
                .exec_approvals
                .remove(&("thread-b".to_string(), "exec-1".to_string())),
            Some(PendingExecApprovalRequest::V2(RequestId::Integer(2)))
        );
        assert_eq!(
            pending
                .patch_approvals
                .remove(&("thread-b".to_string(), "patch-1".to_string())),
            Some(PendingPatchApprovalRequest::V2(RequestId::Integer(4)))
        );
        assert_eq!(
            pending
                .request_permissions
                .remove(&("thread-b".to_string(), "perm-1".to_string())),
            Some(RequestId::Integer(6))
        );
        assert_eq!(
            pending.pop_request_user_input_request_id("thread-b", "turn-1"),
            Some(RequestId::Integer(8))
        );
        assert_eq!(
            pending
                .dynamic_tool_calls
                .remove(&("thread-b".to_string(), "tool-1".to_string())),
            Some(RequestId::Integer(10))
        );
        assert_eq!(
            pending.pop_mcp_elicitation_request_id(
                "thread-b",
                "server",
                &codex_protocol::mcp::RequestId::Integer(12),
            ),
            Some(RequestId::Integer(13))
        );
        assert_eq!(
            pending
                .exec_approvals
                .remove(&("thread-a".to_string(), "exec-1".to_string())),
            Some(PendingExecApprovalRequest::V2(RequestId::Integer(1)))
        );
        assert_eq!(
            pending
                .patch_approvals
                .remove(&("thread-a".to_string(), "patch-1".to_string())),
            Some(PendingPatchApprovalRequest::V2(RequestId::Integer(3)))
        );
        assert_eq!(
            pending
                .request_permissions
                .remove(&("thread-a".to_string(), "perm-1".to_string())),
            Some(RequestId::Integer(5))
        );
        assert_eq!(
            pending.pop_request_user_input_request_id("thread-a", "turn-1"),
            Some(RequestId::Integer(7))
        );
        assert_eq!(
            pending
                .dynamic_tool_calls
                .remove(&("thread-a".to_string(), "tool-1".to_string())),
            Some(RequestId::Integer(9))
        );
        assert_eq!(
            pending.pop_mcp_elicitation_request_id(
                "thread-a",
                "server",
                &codex_protocol::mcp::RequestId::Integer(12),
            ),
            Some(RequestId::Integer(11))
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

    #[tokio::test]
    async fn spawn_agent_bootstrap_preserves_local_history_metadata() {
        let codex_home = TempDir::new().expect("create temp dir");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");
        append_history_entry_local(&config, &ThreadId::new(), "first".to_string())
            .await
            .expect("append first history entry");
        append_history_entry_local(&config, &ThreadId::new(), "second".to_string())
            .await
            .expect("append second history entry");

        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let thread_manager = Arc::new(
            codex_core::test_support::thread_manager_with_models_provider_and_home(
                codex_core::CodexAuth::from_api_key("test"),
                config.model_provider.clone(),
                config.codex_home.clone(),
            ),
        );

        let (codex_op_tx, _thread_scoped_op_tx) = spawn_agent(
            config.clone(),
            app_event_tx,
            thread_manager,
            InProcessAgentContext {
                arg0_paths: codex_arg0::Arg0DispatchPaths::default(),
                cli_kv_overrides: Vec::new(),
                cloud_requirements: CloudRequirementsLoader::default(),
            },
        );

        let maybe_event = timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("timed out waiting for bootstrap app event");
        let event = maybe_event.expect("expected bootstrap app event");
        let AppEvent::CodexEvent(event) = event else {
            panic!("expected bootstrap codex event");
        };
        let EventMsg::SessionConfigured(session) = event.msg else {
            panic!("expected SessionConfigured");
        };
        assert_ne!(session.history_log_id, 0);
        assert_eq!(session.history_entry_count, 2);

        drop(codex_op_tx);
    }

    #[tokio::test]
    async fn thread_start_params_from_config_preserves_effective_overrides() {
        let mut config = test_config().await;
        config.model = Some("gpt-5-mini".to_string());
        config.model_provider_id = "test-provider".to_string();
        config.service_tier = Some(codex_protocol::config_types::ServiceTier::Flex);
        config.base_instructions = Some("base instructions".to_string());
        config.developer_instructions = Some("developer instructions".to_string());
        config.personality = Some(codex_protocol::config_types::Personality::Friendly);
        config
            .permissions
            .approval_policy
            .set(codex_protocol::protocol::AskForApproval::OnRequest)
            .expect("set approval policy");
        config
            .permissions
            .sandbox_policy
            .set(codex_protocol::protocol::SandboxPolicy::new_workspace_write_policy())
            .expect("set sandbox policy");

        let params = thread_start_params_from_config(&config);

        assert_eq!(
            params,
            ThreadStartParams {
                model: Some("gpt-5-mini".to_string()),
                model_provider: Some("test-provider".to_string()),
                service_tier: Some(Some(codex_protocol::config_types::ServiceTier::Flex)),
                cwd: Some(config.cwd.to_string_lossy().into_owned()),
                approval_policy: Some(codex_app_server_protocol::AskForApproval::OnRequest),
                sandbox: Some(codex_app_server_protocol::SandboxMode::WorkspaceWrite),
                config: None,
                service_name: None,
                base_instructions: Some("base instructions".to_string()),
                developer_instructions: Some("developer instructions".to_string()),
                personality: Some(codex_protocol::config_types::Personality::Friendly),
                ephemeral: None,
                dynamic_tools: None,
                mock_experimental_field: None,
                experimental_raw_events: false,
                persist_extended_history: false,
            }
        );
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
        let thread_id = ThreadId::new();
        let notification = JSONRPCNotification {
            method: "codex/event/warning".to_string(),
            params: Some(serde_json::json!({
                "conversationId": thread_id.to_string(),
                "id": "submission-1",
                "msg": {
                    "message": "wrapped warning",
                    "type": "warning",
                },
            })),
        };

        let event = legacy_notification_to_event(notification).expect("decode wrapped warning");
        assert_eq!(event.id, "submission-1");
        let EventMsg::Warning(warning) = event.msg else {
            panic!("expected warning event");
        };
        assert_eq!(warning.message, "wrapped warning".to_string());
    }

    #[test]
    fn decode_legacy_notification_preserves_conversation_id() {
        let thread_id = ThreadId::new();
        let decoded = decode_legacy_notification(JSONRPCNotification {
            method: "codex/event/warning".to_string(),
            params: Some(serde_json::json!({
                "conversationId": thread_id.to_string(),
                "msg": {
                    "message": "wrapped warning",
                    "type": "warning",
                },
            })),
        })
        .expect("decode wrapped warning");

        assert_eq!(decoded.conversation_id, Some(thread_id));
        let EventMsg::Warning(warning) = decoded.event.msg else {
            panic!("expected warning event");
        };
        assert_eq!(warning.message, "wrapped warning".to_string());
    }

    #[test]
    fn decode_legacy_notification_defaults_missing_event_id() {
        let decoded = decode_legacy_notification(JSONRPCNotification {
            method: "codex/event/warning".to_string(),
            params: Some(serde_json::json!({
                "msg": {
                    "message": "wrapped warning",
                    "type": "warning",
                },
            })),
        })
        .expect("decode wrapped warning");

        assert!(decoded.event.id.is_empty());
        let EventMsg::Warning(warning) = decoded.event.msg else {
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
    async fn in_process_start_args_opt_outs_cover_typed_interactive_requests() {
        let config = test_config().await;
        let args = in_process_start_args(
            &config,
            test_thread_manager(&config),
            codex_arg0::Arg0DispatchPaths::default(),
            Vec::new(),
            CloudRequirementsLoader::default(),
        );

        assert_eq!(
            args.opt_out_notification_methods,
            in_process_typed_event_legacy_opt_outs()
        );
    }

    #[test]
    fn typed_interactive_server_requests_and_legacy_opt_outs_stay_in_sync() {
        let requests = [
            ServerRequest::CommandExecutionRequestApproval {
                request_id: RequestId::Integer(1),
                params: CommandExecutionRequestApprovalParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "item-1".to_string(),
                    approval_id: Some("approval-1".to_string()),
                    reason: Some("needs approval".to_string()),
                    network_approval_context: None,
                    command: Some("echo hello".to_string()),
                    cwd: Some(PathBuf::from("/tmp/project")),
                    command_actions: None,
                    additional_permissions: None,
                    skill_metadata: None,
                    proposed_execpolicy_amendment: None,
                    proposed_network_policy_amendments: None,
                    available_decisions: None,
                },
            },
            ServerRequest::ExecCommandApproval {
                request_id: RequestId::Integer(2),
                params: ExecCommandApprovalParams {
                    conversation_id: ThreadId::new(),
                    call_id: "call-1".to_string(),
                    approval_id: Some("approval-legacy-1".to_string()),
                    command: vec!["echo".to_string(), "hello".to_string()],
                    cwd: PathBuf::from("/tmp/project"),
                    reason: Some("legacy approval".to_string()),
                    parsed_cmd: Vec::new(),
                },
            },
            ServerRequest::FileChangeRequestApproval {
                request_id: RequestId::Integer(3),
                params: FileChangeRequestApprovalParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "patch-1".to_string(),
                    reason: Some("write access".to_string()),
                    grant_root: None,
                },
            },
            ServerRequest::ApplyPatchApproval {
                request_id: RequestId::Integer(4),
                params: codex_app_server_protocol::ApplyPatchApprovalParams {
                    conversation_id: ThreadId::new(),
                    call_id: "patch-legacy-1".to_string(),
                    file_changes: HashMap::new(),
                    reason: Some("legacy patch".to_string()),
                    grant_root: None,
                },
            },
            ServerRequest::ToolRequestUserInput {
                request_id: RequestId::Integer(5),
                params: ToolRequestUserInputParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "input-1".to_string(),
                    questions: vec![ToolRequestUserInputQuestion {
                        id: "q1".to_string(),
                        header: "Header".to_string(),
                        question: "Question?".to_string(),
                        is_other: false,
                        is_secret: false,
                        options: Some(vec![ToolRequestUserInputOption {
                            label: "Option".to_string(),
                            description: "Description".to_string(),
                        }]),
                    }],
                },
            },
            ServerRequest::McpServerElicitationRequest {
                request_id: RequestId::Integer(6),
                params: McpServerElicitationRequestParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: Some("turn-1".to_string()),
                    server_name: "server-1".to_string(),
                    request: McpServerElicitationRequest::Form {
                        meta: None,
                        message: "Allow this request?".to_string(),
                        requested_schema: McpElicitationSchema {
                            schema_uri: None,
                            type_: McpElicitationObjectType::Object,
                            properties: Default::default(),
                            required: None,
                        },
                    },
                },
            },
            ServerRequest::DynamicToolCall {
                request_id: RequestId::Integer(7),
                params: DynamicToolCallParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    call_id: "dynamic-1".to_string(),
                    tool: "tool".to_string(),
                    arguments: serde_json::json!({ "arg": 1 }),
                },
            },
            ServerRequest::PermissionsRequestApproval {
                request_id: RequestId::Integer(8),
                params: PermissionsRequestApprovalParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "permissions-1".to_string(),
                    reason: Some("Select a root".to_string()),
                    permissions: codex_app_server_protocol::AdditionalPermissionProfile {
                        network: None,
                        file_system: None,
                        macos: None,
                    },
                },
            },
            ServerRequest::ChatgptAuthTokensRefresh {
                request_id: RequestId::Integer(9),
                params: ChatgptAuthTokensRefreshParams {
                    reason: ChatgptAuthTokensRefreshReason::Unauthorized,
                    previous_account_id: None,
                },
            },
        ];

        let mut mapped_methods = requests
            .iter()
            .filter_map(in_process_typed_interactive_request)
            .map(InProcessTypedInteractiveRequest::legacy_notification_method)
            .collect::<Vec<_>>();
        mapped_methods.sort_unstable();
        mapped_methods.dedup();

        let mut expected_methods = InProcessTypedInteractiveRequest::ALL
            .into_iter()
            .map(InProcessTypedInteractiveRequest::legacy_notification_method)
            .collect::<Vec<_>>();
        expected_methods.sort_unstable();

        assert_eq!(mapped_methods, expected_methods);
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
        let (should_shutdown, mut rx, client, _thread_manager, _session_id) =
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
        let thread_manager = test_thread_manager(&config);
        let client = InProcessAppServerClient::start(in_process_start_args(
            &config,
            Arc::clone(&thread_manager),
            codex_arg0::Arg0DispatchPaths::default(),
            Vec::new(),
            CloudRequirementsLoader::default(),
        ))
        .await
        .expect("in-process app-server client");
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let mut current_turn_ids = HashMap::new();
        let mut request_ids = RequestIdSequencer::new();
        let mut pending_server_requests = PendingServerRequests::default();

        let should_shutdown = process_in_process_command(
            Op::AddToHistory {
                text: "hello history".to_string(),
            },
            &thread_id,
            &thread_id,
            None,
            &session_id,
            &config,
            &mut current_turn_ids,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            thread_manager.as_ref(),
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
            &thread_id,
            None,
            &session_id,
            &config,
            &mut current_turn_ids,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            thread_manager.as_ref(),
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

    async fn assert_forwarded_op(config: &Config, op: Op) {
        let (should_shutdown, _rx, client, thread_manager, session_id) =
            process_single_op(config, op.clone()).await;
        assert_eq!(should_shutdown, false);
        assert_eq!(
            thread_manager.captured_ops_for_testing(),
            vec![(session_id, op)]
        );

        client.shutdown().await.expect("shutdown in-process client");
    }

    #[tokio::test]
    async fn reload_user_config_is_forwarded_to_thread() {
        let config = test_config().await;
        assert_forwarded_op(&config, Op::ReloadUserConfig).await;
    }

    async fn assert_local_only_warning_for_op(config: &Config, op: Op, expected_message: &str) {
        let (should_shutdown, mut rx, client, _thread_manager, _session_id) =
            process_single_op(config, op).await;
        assert_eq!(should_shutdown, false);

        let event = next_codex_event(&mut rx).await;
        let warning = warning_from_event(event);
        assert_eq!(warning.message, expected_message.to_string());

        client.shutdown().await.expect("shutdown in-process client");
    }

    #[tokio::test]
    async fn review_op_sets_current_turn_id_for_follow_up_interrupts() {
        let config = test_config().await;
        let thread_manager = test_thread_manager(&config);
        let client = InProcessAppServerClient::start(in_process_start_args(
            &config,
            Arc::clone(&thread_manager),
            codex_arg0::Arg0DispatchPaths::default(),
            Vec::new(),
            CloudRequirementsLoader::default(),
        ))
        .await
        .expect("in-process app-server client");
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let mut current_turn_ids = HashMap::new();
        let mut request_ids = RequestIdSequencer::new();
        let mut pending_server_requests = PendingServerRequests::default();

        let thread_start = send_request_with_response::<ThreadStartResponse>(
            &client,
            ClientRequest::ThreadStart {
                request_id: request_ids.next(),
                params: ThreadStartParams::default(),
            },
            "thread/start",
        )
        .await
        .expect("thread/start");
        let thread_id = thread_start.thread.id;
        let session_id = ThreadId::from_string(&thread_id).expect("valid thread id");

        let should_shutdown = process_in_process_command(
            Op::Review {
                review_request: codex_protocol::protocol::ReviewRequest {
                    target: CoreReviewTarget::Custom {
                        instructions: "check current changes".to_string(),
                    },
                    user_facing_hint: None,
                },
            },
            &thread_id,
            &thread_id,
            None,
            &session_id,
            &config,
            &mut current_turn_ids,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            thread_manager.as_ref(),
            &app_event_tx,
        )
        .await;
        assert_eq!(should_shutdown, false);
        let turn_id = current_turn_ids
            .get(&thread_id)
            .expect("review/start should set the active turn id");
        assert_eq!(turn_id.is_empty(), false);

        if let Ok(Some(event)) = timeout(Duration::from_millis(200), rx.recv()).await {
            panic!("did not expect an app event after review/start: {event:?}");
        }

        let should_shutdown = process_in_process_command(
            Op::Interrupt,
            &thread_id,
            &thread_id,
            None,
            &session_id,
            &config,
            &mut current_turn_ids,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            thread_manager.as_ref(),
            &app_event_tx,
        )
        .await;
        assert_eq!(should_shutdown, false);
        if let Ok(Some(event)) = timeout(Duration::from_millis(200), rx.recv()).await {
            panic!("did not expect an app event after successful turn/interrupt: {event:?}");
        }

        client.shutdown().await.expect("shutdown in-process client");
    }

    #[tokio::test]
    async fn child_shutdown_does_not_request_in_process_loop_exit() {
        let config = test_config().await;
        let thread_manager = test_thread_manager(&config);
        let client = InProcessAppServerClient::start(in_process_start_args(
            &config,
            Arc::clone(&thread_manager),
            codex_arg0::Arg0DispatchPaths::default(),
            Vec::new(),
            CloudRequirementsLoader::default(),
        ))
        .await
        .expect("in-process app-server client");
        let (tx, _rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let mut current_turn_ids = HashMap::from([
            ("primary-thread".to_string(), "primary-turn".to_string()),
            ("child-thread".to_string(), "child-turn".to_string()),
        ]);
        let mut request_ids = RequestIdSequencer::new();
        let mut pending_server_requests = PendingServerRequests::default();
        pending_server_requests.register_request_user_input(
            "child-thread".to_string(),
            "child-turn".to_string(),
            RequestId::Integer(5),
        );
        let session_id = ThreadId::new();

        let should_shutdown = process_in_process_command(
            Op::Shutdown,
            "child-thread",
            "primary-thread",
            None,
            &session_id,
            &config,
            &mut current_turn_ids,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            thread_manager.as_ref(),
            &app_event_tx,
        )
        .await;

        assert_eq!(should_shutdown, false);
        assert_eq!(
            current_turn_ids,
            HashMap::from([("primary-thread".to_string(), "primary-turn".to_string())])
        );
        assert!(pending_server_requests.request_user_input.is_empty());

        client.shutdown().await.expect("shutdown in-process client");
    }

    #[tokio::test]
    async fn primary_thread_closed_notification_requests_in_process_loop_shutdown() {
        let config = test_config().await;
        let thread_manager = test_thread_manager(&config);
        let client = InProcessAppServerClient::start(in_process_start_args(
            &config,
            Arc::clone(&thread_manager),
            codex_arg0::Arg0DispatchPaths::default(),
            Vec::new(),
            CloudRequirementsLoader::default(),
        ))
        .await
        .expect("in-process app-server client");
        let mut request_ids = RequestIdSequencer::new();
        let thread_start = send_request_with_response::<ThreadStartResponse>(
            &client,
            ClientRequest::ThreadStart {
                request_id: request_ids.next(),
                params: ThreadStartParams::default(),
            },
            "thread/start",
        )
        .await
        .expect("thread/start");
        let session_configured =
            session_configured_from_thread_start_response(&config, thread_start)
                .await
                .expect("session configured");
        let thread_id = session_configured.session_id.to_string();
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let (codex_op_tx, codex_op_rx) = unbounded_channel();
        let (thread_scoped_op_tx, thread_scoped_op_rx) = unbounded_channel();

        let run_loop = tokio::spawn(run_in_process_agent_loop(
            codex_op_rx,
            thread_scoped_op_rx,
            client,
            Arc::clone(&thread_manager),
            config,
            thread_id,
            session_configured,
            app_event_tx,
            RequestIdSequencer::new(),
            HashMap::new(),
        ));

        thread_manager
            .remove_and_close_all_threads()
            .await
            .expect("close all threads");

        let shutdown_complete = timeout(Duration::from_secs(2), async {
            loop {
                let event = next_codex_event(&mut rx).await;
                if matches!(event.msg, EventMsg::ShutdownComplete) {
                    break event;
                }
            }
        })
        .await
        .expect("timed out waiting for shutdown complete event");
        assert!(matches!(shutdown_complete.msg, EventMsg::ShutdownComplete));

        timeout(Duration::from_secs(2), run_loop)
            .await
            .expect("timed out waiting for run loop to exit")
            .expect("run loop task should not panic");

        drop(codex_op_tx);
        drop(thread_scoped_op_tx);
    }

    #[tokio::test]
    async fn interrupt_uses_active_turn_for_target_thread_only() {
        let config = test_config().await;
        let session_id = ThreadId::new();
        let thread_manager = test_thread_manager(&config);
        let client = InProcessAppServerClient::start(in_process_start_args(
            &config,
            Arc::clone(&thread_manager),
            codex_arg0::Arg0DispatchPaths::default(),
            Vec::new(),
            CloudRequirementsLoader::default(),
        ))
        .await
        .expect("in-process app-server client");
        let (tx, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx);
        let mut current_turn_ids =
            HashMap::from([("child-thread".to_string(), "child-turn".to_string())]);
        let mut request_ids = RequestIdSequencer::new();
        let mut pending_server_requests = PendingServerRequests::default();

        let should_shutdown = process_in_process_command(
            Op::Interrupt,
            "primary-thread",
            "primary-thread",
            None,
            &session_id,
            &config,
            &mut current_turn_ids,
            &mut request_ids,
            &mut pending_server_requests,
            &client,
            thread_manager.as_ref(),
            &app_event_tx,
        )
        .await;

        assert_eq!(should_shutdown, false);
        let event = next_codex_event(&mut rx).await;
        let warning = warning_from_event(event);
        assert_eq!(
            warning.message,
            "turn/interrupt skipped because there is no active turn".to_string()
        );

        client.shutdown().await.expect("shutdown in-process client");
    }

    #[tokio::test]
    async fn undo_still_emits_explicit_local_only_warning() {
        let config = test_config().await;
        assert_local_only_warning_for_op(
            &config,
            Op::Undo,
            "Undo is temporarily unavailable in in-process local-only mode",
        )
        .await;
    }

    #[tokio::test]
    async fn override_turn_context_is_forwarded_to_thread() {
        let config = test_config().await;
        assert_forwarded_op(
            &config,
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
        )
        .await;
    }

    #[tokio::test]
    async fn legacy_core_ops_are_forwarded_to_thread() {
        let config = test_config().await;
        let forwarded_ops = vec![
            Op::DropMemories,
            Op::UpdateMemories,
            Op::RunUserShellCommand {
                command: "echo hello".to_string(),
            },
            Op::ListMcpTools,
        ];

        for op in forwarded_ops {
            assert_forwarded_op(&config, op).await;
        }
    }

    #[tokio::test]
    async fn resolve_elicitation_without_pending_request_warns() {
        let config = test_config().await;
        let (should_shutdown, mut rx, client, _thread_manager, _session_id) = process_single_op(
            &config,
            Op::ResolveElicitation {
                server_name: "test-server".to_string(),
                request_id: codex_protocol::mcp::RequestId::Integer(1),
                decision: codex_protocol::approvals::ElicitationAction::Cancel,
                content: None,
                meta: None,
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
    async fn local_external_chatgpt_refresh_uses_base_refresher_over_in_process_override() {
        let codex_home = TempDir::new().expect("create temp dir");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");
        let stale_access_token = fake_external_access_token("pro");
        login_with_chatgpt_auth_tokens(
            &config.codex_home,
            &stale_access_token,
            "workspace-1",
            Some("pro"),
        )
        .expect("write external auth token");

        let auth_manager = AuthManager::shared(
            config.codex_home.clone(),
            false,
            config.cli_auth_credentials_store_mode,
        );
        auth_manager.reload();
        let base_refresher = Arc::new(RecordingExternalAuthRefresher {
            refreshed: ExternalAuthTokens {
                access_token: fake_external_access_token("enterprise"),
                chatgpt_account_id: "workspace-1".to_string(),
                chatgpt_plan_type: Some("enterprise".to_string()),
            },
            contexts: Mutex::new(Vec::new()),
        });
        auth_manager.set_external_auth_refresher(base_refresher.clone());
        let _override_guard = auth_manager.push_external_auth_override(
            Arc::new(FailingExternalAuthRefresher),
            auth_manager.forced_chatgpt_workspace_id(),
        );

        let response = local_external_chatgpt_tokens(Arc::clone(&auth_manager))
            .await
            .expect("local token refresh response");
        assert_eq!(
            response.access_token,
            fake_external_access_token("enterprise")
        );
        assert_eq!(response.chatgpt_account_id, "workspace-1".to_string());
        assert_eq!(response.chatgpt_plan_type, Some("enterprise".to_string()));
        assert_eq!(
            base_refresher
                .contexts
                .lock()
                .expect("contexts mutex")
                .as_slice(),
            &[ExternalAuthRefreshContext {
                reason: codex_core::auth::ExternalAuthRefreshReason::Unauthorized,
                previous_account_id: Some("workspace-1".to_string()),
            }]
        );
    }

    #[tokio::test]
    async fn local_external_chatgpt_refresh_fails_without_external_auth() {
        let codex_home = TempDir::new().expect("create temp dir");
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");
        let auth_manager = AuthManager::shared(
            config.codex_home.clone(),
            false,
            config.cli_auth_credentials_store_mode,
        );
        let error = local_external_chatgpt_tokens(auth_manager)
            .await
            .expect_err("expected local refresh error");
        assert!(
            error.contains("no cached auth available")
                || error.contains("external ChatGPT token auth is not active"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn refreshed_chatgpt_account_mismatch_is_rejected() {
        let error = validate_refreshed_chatgpt_account(Some("workspace-1"), "workspace-2")
            .expect_err("expected account mismatch to fail");
        assert_eq!(
            error,
            "local auth refresh account mismatch: expected `workspace-1`, got `workspace-2`"
                .to_string()
        );
    }
}
