use crate::events::AppServerRpcTransport;
use crate::events::CodexAppMentionedEventRequest;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexAppUsedEventRequest;
use crate::events::CodexPluginEventRequest;
use crate::events::CodexPluginUsedEventRequest;
use crate::events::CodexRuntimeMetadata;
use crate::events::CodexToolCallEventParams;
use crate::events::CodexToolCallEventRequest;
use crate::events::SkillInvocationEventParams;
use crate::events::SkillInvocationEventRequest;
use crate::events::ThreadInitializationMode;
use crate::events::ThreadInitializedEvent;
use crate::events::ThreadInitializedEventParams;
use crate::events::ToolCallFailureKind;
use crate::events::ToolCallFinalReviewOutcome;
use crate::events::ToolCallTerminalStatus;
use crate::events::ToolKind;
use crate::events::TrackEventRequest;
use crate::events::codex_app_metadata;
use crate::events::codex_plugin_metadata;
use crate::events::codex_plugin_used_metadata;
use crate::events::plugin_state_event_type;
use crate::events::subagent_thread_started_event_request;
use crate::events::thread_source_name;
use crate::facts::AnalyticsFact;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::CustomAnalyticsFact;
use crate::facts::PluginState;
use crate::facts::PluginStateChangedInput;
use crate::facts::PluginUsedInput;
use crate::facts::SkillInvokedInput;
use crate::facts::SubAgentThreadStartedInput;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_git_utils::collect_git_info;
use codex_git_utils::get_git_repo_root;
use codex_login::default_client::originator;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use sha1::Digest;
use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[derive(Default)]
pub(crate) struct AnalyticsReducer {
    connections: HashMap<u64, ConnectionState>,
    tool_calls: HashMap<String, ToolCallState>,
}

struct ConnectionState {
    app_server_client: CodexAppServerClientMetadata,
    runtime: CodexRuntimeMetadata,
}

struct ToolCallState {
    connection_id: u64,
    started_at: u64,
}

struct ToolCallCompletedMetadata {
    tool_call_id: String,
    tool_name: String,
    tool_kind: ToolKind,
    terminal_status: ToolCallTerminalStatus,
    failure_kind: Option<ToolCallFailureKind>,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
}

impl AnalyticsReducer {
    pub(crate) async fn ingest(&mut self, input: AnalyticsFact, out: &mut Vec<TrackEventRequest>) {
        match input {
            AnalyticsFact::Initialize {
                connection_id,
                params,
                product_client_id,
                runtime,
                rpc_transport,
            } => {
                self.ingest_initialize(
                    connection_id,
                    params,
                    product_client_id,
                    runtime,
                    rpc_transport,
                );
            }
            AnalyticsFact::ClientRequest {
                connection_id: _connection_id,
                request_id: _request_id,
                request: _request,
            } => {}
            AnalyticsFact::ClientResponse {
                connection_id,
                response,
            } => {
                self.ingest_response(connection_id, *response, out);
            }
            AnalyticsFact::ServerRequest {
                connection_id: _connection_id,
                request: _request,
            } => {}
            AnalyticsFact::ServerResponse {
                response: _response,
            } => {}
            AnalyticsFact::Notification {
                connection_id,
                notification,
            } => {
                self.ingest_notification(connection_id, *notification, out);
            }
            AnalyticsFact::Custom(input) => match input {
                CustomAnalyticsFact::SubAgentThreadStarted(input) => {
                    self.ingest_subagent_thread_started(input, out);
                }
                CustomAnalyticsFact::SkillInvoked(input) => {
                    self.ingest_skill_invoked(input, out).await;
                }
                CustomAnalyticsFact::AppMentioned(input) => {
                    self.ingest_app_mentioned(input, out);
                }
                CustomAnalyticsFact::AppUsed(input) => {
                    self.ingest_app_used(input, out);
                }
                CustomAnalyticsFact::PluginUsed(input) => {
                    self.ingest_plugin_used(input, out);
                }
                CustomAnalyticsFact::PluginStateChanged(input) => {
                    self.ingest_plugin_state_changed(input, out);
                }
            },
        }
    }

    fn ingest_initialize(
        &mut self,
        connection_id: u64,
        params: InitializeParams,
        product_client_id: String,
        runtime: CodexRuntimeMetadata,
        rpc_transport: AppServerRpcTransport,
    ) {
        self.connections.insert(
            connection_id,
            ConnectionState {
                app_server_client: CodexAppServerClientMetadata {
                    product_client_id,
                    client_name: Some(params.client_info.name),
                    client_version: Some(params.client_info.version),
                    rpc_transport,
                    experimental_api_enabled: params
                        .capabilities
                        .map(|capabilities| capabilities.experimental_api),
                },
                runtime,
            },
        );
    }

    fn ingest_subagent_thread_started(
        &mut self,
        input: SubAgentThreadStartedInput,
        out: &mut Vec<TrackEventRequest>,
    ) {
        out.push(TrackEventRequest::ThreadInitialized(
            subagent_thread_started_event_request(input),
        ));
    }

    async fn ingest_skill_invoked(
        &mut self,
        input: SkillInvokedInput,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let SkillInvokedInput {
            tracking,
            invocations,
        } = input;
        for invocation in invocations {
            let skill_scope = match invocation.skill_scope {
                SkillScope::User => "user",
                SkillScope::Repo => "repo",
                SkillScope::System => "system",
                SkillScope::Admin => "admin",
            };
            let repo_root = get_git_repo_root(invocation.skill_path.as_path());
            let repo_url = if let Some(root) = repo_root.as_ref() {
                collect_git_info(root)
                    .await
                    .and_then(|info| info.repository_url)
            } else {
                None
            };
            let skill_id = skill_id_for_local_skill(
                repo_url.as_deref(),
                repo_root.as_deref(),
                invocation.skill_path.as_path(),
                invocation.skill_name.as_str(),
            );
            out.push(TrackEventRequest::SkillInvocation(
                SkillInvocationEventRequest {
                    event_type: "skill_invocation",
                    skill_id,
                    skill_name: invocation.skill_name.clone(),
                    event_params: SkillInvocationEventParams {
                        thread_id: Some(tracking.thread_id.clone()),
                        invoke_type: Some(invocation.invocation_type),
                        model_slug: Some(tracking.model_slug.clone()),
                        product_client_id: Some(originator().value),
                        repo_url,
                        skill_scope: Some(skill_scope.to_string()),
                    },
                },
            ));
        }
    }

    fn ingest_app_mentioned(&mut self, input: AppMentionedInput, out: &mut Vec<TrackEventRequest>) {
        let AppMentionedInput { tracking, mentions } = input;
        out.extend(mentions.into_iter().map(|mention| {
            let event_params = codex_app_metadata(&tracking, mention);
            TrackEventRequest::AppMentioned(CodexAppMentionedEventRequest {
                event_type: "codex_app_mentioned",
                event_params,
            })
        }));
    }

    fn ingest_app_used(&mut self, input: AppUsedInput, out: &mut Vec<TrackEventRequest>) {
        let AppUsedInput { tracking, app } = input;
        let event_params = codex_app_metadata(&tracking, app);
        out.push(TrackEventRequest::AppUsed(CodexAppUsedEventRequest {
            event_type: "codex_app_used",
            event_params,
        }));
    }

    fn ingest_plugin_used(&mut self, input: PluginUsedInput, out: &mut Vec<TrackEventRequest>) {
        let PluginUsedInput { tracking, plugin } = input;
        out.push(TrackEventRequest::PluginUsed(CodexPluginUsedEventRequest {
            event_type: "codex_plugin_used",
            event_params: codex_plugin_used_metadata(&tracking, plugin),
        }));
    }

    fn ingest_plugin_state_changed(
        &mut self,
        input: PluginStateChangedInput,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let PluginStateChangedInput { plugin, state } = input;
        let event = CodexPluginEventRequest {
            event_type: plugin_state_event_type(state),
            event_params: codex_plugin_metadata(plugin),
        };
        out.push(match state {
            PluginState::Installed => TrackEventRequest::PluginInstalled(event),
            PluginState::Uninstalled => TrackEventRequest::PluginUninstalled(event),
            PluginState::Enabled => TrackEventRequest::PluginEnabled(event),
            PluginState::Disabled => TrackEventRequest::PluginDisabled(event),
        });
    }

    fn ingest_response(
        &mut self,
        connection_id: u64,
        response: ClientResponse,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let (thread, model, initialization_mode) = match response {
            ClientResponse::ThreadStart { response, .. } => (
                response.thread,
                response.model,
                ThreadInitializationMode::New,
            ),
            ClientResponse::ThreadResume { response, .. } => (
                response.thread,
                response.model,
                ThreadInitializationMode::Resumed,
            ),
            ClientResponse::ThreadFork { response, .. } => (
                response.thread,
                response.model,
                ThreadInitializationMode::Forked,
            ),
            _ => return,
        };
        let thread_source: SessionSource = thread.source.into();
        let Some(connection_state) = self.connections.get(&connection_id) else {
            return;
        };
        out.push(TrackEventRequest::ThreadInitialized(
            ThreadInitializedEvent {
                event_type: "codex_thread_initialized",
                event_params: ThreadInitializedEventParams {
                    thread_id: thread.id,
                    app_server_client: connection_state.app_server_client.clone(),
                    runtime: connection_state.runtime.clone(),
                    model,
                    ephemeral: thread.ephemeral,
                    thread_source: thread_source_name(&thread_source),
                    initialization_mode,
                    subagent_source: None,
                    parent_thread_id: None,
                    created_at: u64::try_from(thread.created_at).unwrap_or_default(),
                },
            },
        ));
    }

    fn ingest_notification(
        &mut self,
        connection_id: u64,
        notification: ServerNotification,
        out: &mut Vec<TrackEventRequest>,
    ) {
        match notification {
            ServerNotification::ItemStarted(notification) => {
                if let Some(tool_call_id) = tool_call_id(&notification.item) {
                    self.tool_calls
                        .entry(tool_call_id.to_string())
                        .or_insert_with(|| ToolCallState {
                            connection_id,
                            started_at: now_unix_secs(),
                        });
                }
            }
            ServerNotification::ItemCompleted(notification) => {
                let Some(completed_metadata) = tool_call_completed_metadata(&notification.item)
                else {
                    return;
                };
                let Some(started) = self.tool_calls.remove(&completed_metadata.tool_call_id) else {
                    return;
                };
                let Some(connection_state) = self.connections.get(&started.connection_id) else {
                    return;
                };
                let completed_at = now_unix_secs();
                let duration_ms = completed_metadata.duration_ms.or_else(|| {
                    completed_at
                        .checked_sub(started.started_at)
                        .map(|duration_secs| duration_secs.saturating_mul(1000))
                });

                out.push(TrackEventRequest::ToolCall(CodexToolCallEventRequest {
                    event_type: "codex_tool_call_event",
                    event_params: CodexToolCallEventParams {
                        thread_id: notification.thread_id,
                        turn_id: notification.turn_id,
                        tool_call_id: completed_metadata.tool_call_id,
                        app_server_client: connection_state.app_server_client.clone(),
                        runtime: connection_state.runtime.clone(),
                        tool_name: completed_metadata.tool_name,
                        tool_kind: completed_metadata.tool_kind,
                        started_at: started.started_at,
                        completed_at: Some(completed_at),
                        duration_ms,
                        execution_started: true,
                        review_count: 0,
                        guardian_review_count: 0,
                        user_review_count: 0,
                        final_review_outcome: ToolCallFinalReviewOutcome::NotNeeded,
                        terminal_status: completed_metadata.terminal_status,
                        failure_kind: completed_metadata.failure_kind,
                        exit_code: completed_metadata.exit_code,
                        requested_additional_permissions: false,
                        requested_network_access: false,
                        retry_count: 0,
                    },
                }));
            }
            _ => {}
        }
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn tool_call_id(item: &ThreadItem) -> Option<&str> {
    match item {
        ThreadItem::CommandExecution { id, .. }
        | ThreadItem::FileChange { id, .. }
        | ThreadItem::McpToolCall { id, .. }
        | ThreadItem::DynamicToolCall { id, .. }
        | ThreadItem::CollabAgentToolCall { id, .. }
        | ThreadItem::WebSearch { id, .. }
        | ThreadItem::ImageGeneration { id, .. } => Some(id),
        _ => None,
    }
}

fn tool_call_completed_metadata(item: &ThreadItem) -> Option<ToolCallCompletedMetadata> {
    match item {
        ThreadItem::CommandExecution {
            id,
            source,
            status,
            exit_code,
            duration_ms,
            ..
        } => {
            let (terminal_status, failure_kind) = command_execution_outcome(status)?;
            let tool_kind = command_execution_tool_kind(source);
            Some(ToolCallCompletedMetadata {
                tool_call_id: id.clone(),
                tool_name: command_execution_tool_name(source).to_string(),
                tool_kind,
                terminal_status,
                failure_kind,
                exit_code: *exit_code,
                duration_ms: option_i64_to_u64(*duration_ms),
            })
        }
        ThreadItem::FileChange { id, status, .. } => {
            let (terminal_status, failure_kind) = patch_apply_outcome(status)?;
            Some(ToolCallCompletedMetadata {
                tool_call_id: id.clone(),
                tool_name: "apply_patch".to_string(),
                tool_kind: ToolKind::ApplyPatch,
                terminal_status,
                failure_kind,
                exit_code: None,
                duration_ms: None,
            })
        }
        ThreadItem::McpToolCall {
            id,
            tool,
            status,
            duration_ms,
            ..
        } => {
            let (terminal_status, failure_kind) = mcp_tool_call_outcome(status)?;
            Some(ToolCallCompletedMetadata {
                tool_call_id: id.clone(),
                tool_name: tool.clone(),
                tool_kind: ToolKind::Mcp,
                terminal_status,
                failure_kind,
                exit_code: None,
                duration_ms: option_i64_to_u64(*duration_ms),
            })
        }
        ThreadItem::DynamicToolCall {
            id,
            tool,
            status,
            duration_ms,
            ..
        } => {
            let (terminal_status, failure_kind) = dynamic_tool_call_outcome(status)?;
            Some(ToolCallCompletedMetadata {
                tool_call_id: id.clone(),
                tool_name: tool.clone(),
                tool_kind: ToolKind::Dynamic,
                terminal_status,
                failure_kind,
                exit_code: None,
                duration_ms: option_i64_to_u64(*duration_ms),
            })
        }
        ThreadItem::CollabAgentToolCall {
            id, tool, status, ..
        } => {
            let (terminal_status, failure_kind) = collab_tool_call_outcome(status)?;
            Some(ToolCallCompletedMetadata {
                tool_call_id: id.clone(),
                tool_name: collab_agent_tool_name(tool).to_string(),
                tool_kind: ToolKind::Other,
                terminal_status,
                failure_kind,
                exit_code: None,
                duration_ms: None,
            })
        }
        ThreadItem::WebSearch { id, .. } => Some(ToolCallCompletedMetadata {
            tool_call_id: id.clone(),
            tool_name: "web_search".to_string(),
            tool_kind: ToolKind::Other,
            terminal_status: ToolCallTerminalStatus::Completed,
            failure_kind: None,
            exit_code: None,
            duration_ms: None,
        }),
        ThreadItem::ImageGeneration { id, status, .. } => {
            let (terminal_status, failure_kind) = image_generation_outcome(status.as_str());
            Some(ToolCallCompletedMetadata {
                tool_call_id: id.clone(),
                tool_name: "image_generation".to_string(),
                tool_kind: ToolKind::Other,
                terminal_status,
                failure_kind,
                exit_code: None,
                duration_ms: None,
            })
        }
        _ => None,
    }
}

fn command_execution_tool_kind(source: &CommandExecutionSource) -> ToolKind {
    match source {
        CommandExecutionSource::UnifiedExecStartup
        | CommandExecutionSource::UnifiedExecInteraction => ToolKind::UnifiedExec,
        CommandExecutionSource::Agent | CommandExecutionSource::UserShell => ToolKind::Shell,
    }
}

fn command_execution_tool_name(source: &CommandExecutionSource) -> &'static str {
    match source {
        CommandExecutionSource::UnifiedExecStartup
        | CommandExecutionSource::UnifiedExecInteraction => "unified_exec",
        CommandExecutionSource::UserShell => "user_shell",
        CommandExecutionSource::Agent => "shell",
    }
}

fn command_execution_outcome(
    status: &CommandExecutionStatus,
) -> Option<(ToolCallTerminalStatus, Option<ToolCallFailureKind>)> {
    match status {
        CommandExecutionStatus::InProgress => None,
        CommandExecutionStatus::Completed => Some((ToolCallTerminalStatus::Completed, None)),
        CommandExecutionStatus::Failed => Some((
            ToolCallTerminalStatus::Failed,
            Some(ToolCallFailureKind::ToolError),
        )),
        CommandExecutionStatus::Declined => Some((
            ToolCallTerminalStatus::Rejected,
            Some(ToolCallFailureKind::ApprovalDenied),
        )),
    }
}

fn patch_apply_outcome(
    status: &PatchApplyStatus,
) -> Option<(ToolCallTerminalStatus, Option<ToolCallFailureKind>)> {
    match status {
        PatchApplyStatus::InProgress => None,
        PatchApplyStatus::Completed => Some((ToolCallTerminalStatus::Completed, None)),
        PatchApplyStatus::Failed => Some((
            ToolCallTerminalStatus::Failed,
            Some(ToolCallFailureKind::ToolError),
        )),
        PatchApplyStatus::Declined => Some((
            ToolCallTerminalStatus::Rejected,
            Some(ToolCallFailureKind::ApprovalDenied),
        )),
    }
}

fn mcp_tool_call_outcome(
    status: &McpToolCallStatus,
) -> Option<(ToolCallTerminalStatus, Option<ToolCallFailureKind>)> {
    match status {
        McpToolCallStatus::InProgress => None,
        McpToolCallStatus::Completed => Some((ToolCallTerminalStatus::Completed, None)),
        McpToolCallStatus::Failed => Some((
            ToolCallTerminalStatus::Failed,
            Some(ToolCallFailureKind::ToolError),
        )),
    }
}

fn dynamic_tool_call_outcome(
    status: &DynamicToolCallStatus,
) -> Option<(ToolCallTerminalStatus, Option<ToolCallFailureKind>)> {
    match status {
        DynamicToolCallStatus::InProgress => None,
        DynamicToolCallStatus::Completed => Some((ToolCallTerminalStatus::Completed, None)),
        DynamicToolCallStatus::Failed => Some((
            ToolCallTerminalStatus::Failed,
            Some(ToolCallFailureKind::ToolError),
        )),
    }
}

fn collab_tool_call_outcome(
    status: &CollabAgentToolCallStatus,
) -> Option<(ToolCallTerminalStatus, Option<ToolCallFailureKind>)> {
    match status {
        CollabAgentToolCallStatus::InProgress => None,
        CollabAgentToolCallStatus::Completed => Some((ToolCallTerminalStatus::Completed, None)),
        CollabAgentToolCallStatus::Failed => Some((
            ToolCallTerminalStatus::Failed,
            Some(ToolCallFailureKind::ToolError),
        )),
    }
}

fn image_generation_outcome(status: &str) -> (ToolCallTerminalStatus, Option<ToolCallFailureKind>) {
    match status {
        "failed" | "error" => (
            ToolCallTerminalStatus::Failed,
            Some(ToolCallFailureKind::ToolError),
        ),
        _ => (ToolCallTerminalStatus::Completed, None),
    }
}

fn collab_agent_tool_name(tool: &CollabAgentTool) -> &'static str {
    match tool {
        CollabAgentTool::SpawnAgent => "spawn_agent",
        CollabAgentTool::SendInput => "send_input",
        CollabAgentTool::ResumeAgent => "resume_agent",
        CollabAgentTool::Wait => "wait_agent",
        CollabAgentTool::CloseAgent => "close_agent",
    }
}

fn option_i64_to_u64(value: Option<i64>) -> Option<u64> {
    value.and_then(|value| u64::try_from(value).ok())
}

pub(crate) fn skill_id_for_local_skill(
    repo_url: Option<&str>,
    repo_root: Option<&Path>,
    skill_path: &Path,
    skill_name: &str,
) -> String {
    let path = normalize_path_for_skill_id(repo_url, repo_root, skill_path);
    let prefix = if let Some(url) = repo_url {
        format!("repo_{url}")
    } else {
        "personal".to_string()
    };
    let raw_id = format!("{prefix}_{path}_{skill_name}");
    let mut hasher = sha1::Sha1::new();
    sha1::Digest::update(&mut hasher, raw_id.as_bytes());
    format!("{:x}", sha1::Digest::finalize(hasher))
}

/// Returns a normalized path for skill ID construction.
///
/// - Repo-scoped skills use a path relative to the repo root.
/// - User/admin/system skills use an absolute path.
pub(crate) fn normalize_path_for_skill_id(
    repo_url: Option<&str>,
    repo_root: Option<&Path>,
    skill_path: &Path,
) -> String {
    let resolved_path =
        std::fs::canonicalize(skill_path).unwrap_or_else(|_| skill_path.to_path_buf());
    match (repo_url, repo_root) {
        (Some(_), Some(root)) => {
            let resolved_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
            resolved_path
                .strip_prefix(&resolved_root)
                .unwrap_or(resolved_path.as_path())
                .to_string_lossy()
                .replace('\\', "/")
        }
        _ => resolved_path.to_string_lossy().replace('\\', "/"),
    }
}
