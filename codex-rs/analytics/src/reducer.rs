use crate::events::AppServerRpcTransport;
use crate::events::CodexAppMentionedEventRequest;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexAppUsedEventRequest;
use crate::events::CodexCollabAgentToolCallEventParams;
use crate::events::CodexCollabAgentToolCallEventRequest;
use crate::events::CodexCommandExecutionEventParams;
use crate::events::CodexCommandExecutionEventRequest;
use crate::events::CodexDynamicToolCallEventParams;
use crate::events::CodexDynamicToolCallEventRequest;
use crate::events::CodexFileChangeEventParams;
use crate::events::CodexFileChangeEventRequest;
use crate::events::CodexImageGenerationEventParams;
use crate::events::CodexImageGenerationEventRequest;
use crate::events::CodexMcpToolCallEventParams;
use crate::events::CodexMcpToolCallEventRequest;
use crate::events::CodexPluginEventRequest;
use crate::events::CodexPluginUsedEventRequest;
use crate::events::CodexRuntimeMetadata;
use crate::events::CodexToolItemEventBase;
use crate::events::CodexWebSearchEventParams;
use crate::events::CodexWebSearchEventRequest;
use crate::events::CollabAgentToolKind;
use crate::events::CommandExecutionFamily;
use crate::events::CommandExecutionSourceKind;
use crate::events::SkillInvocationEventParams;
use crate::events::SkillInvocationEventRequest;
use crate::events::ThreadInitializationMode;
use crate::events::ThreadInitializedEvent;
use crate::events::ThreadInitializedEventParams;
use crate::events::ToolItemFailureKind;
use crate::events::ToolItemFinalApprovalOutcome;
use crate::events::ToolItemTerminalStatus;
use crate::events::TrackEventRequest;
use crate::events::WebSearchActionKind;
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
use codex_app_server_protocol::CollabAgentStatus;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::WebSearchAction;
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
    threads: HashMap<String, ThreadMetadataState>,
    tool_items: HashMap<String, ToolItemState>,
}

struct ConnectionState {
    app_server_client: CodexAppServerClientMetadata,
    runtime: CodexRuntimeMetadata,
}

#[derive(Clone, Default)]
struct ThreadMetadataState {
    thread_source: Option<&'static str>,
    subagent_source: Option<String>,
    parent_thread_id: Option<String>,
}

struct ToolItemState {
    connection_id: u64,
    started_at: u64,
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
        let event = subagent_thread_started_event_request(input);
        self.threads.insert(
            event.event_params.thread_id.clone(),
            ThreadMetadataState {
                thread_source: event.event_params.thread_source,
                subagent_source: event.event_params.subagent_source.clone(),
                parent_thread_id: event.event_params.parent_thread_id.clone(),
            },
        );
        out.push(TrackEventRequest::ThreadInitialized(event));
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
        let thread_id = thread.id.clone();
        let thread_source = thread_source_name(&thread_source);
        self.threads.insert(
            thread_id.clone(),
            ThreadMetadataState {
                thread_source,
                subagent_source: None,
                parent_thread_id: None,
            },
        );
        out.push(TrackEventRequest::ThreadInitialized(
            ThreadInitializedEvent {
                event_type: "codex_thread_initialized",
                event_params: ThreadInitializedEventParams {
                    thread_id,
                    app_server_client: connection_state.app_server_client.clone(),
                    runtime: connection_state.runtime.clone(),
                    model,
                    ephemeral: thread.ephemeral,
                    thread_source,
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
                if let Some(item_id) = tool_item_id(&notification.item) {
                    self.tool_items
                        .entry(item_id.to_string())
                        .or_insert_with(|| ToolItemState {
                            connection_id,
                            started_at: now_unix_secs(),
                        });
                }
            }
            ServerNotification::ItemCompleted(notification) => {
                let Some(item_id) = tool_item_id(&notification.item) else {
                    return;
                };
                let Some(started) = self.tool_items.remove(item_id) else {
                    return;
                };
                let Some(connection_state) = self.connections.get(&started.connection_id) else {
                    return;
                };
                let completed_at = now_unix_secs();
                if let Some(event) = tool_item_event(
                    &notification.thread_id,
                    &notification.turn_id,
                    &notification.item,
                    started.started_at,
                    completed_at,
                    connection_state,
                    self.threads.get(&notification.thread_id),
                ) {
                    out.push(event);
                }
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

fn tool_item_id(item: &ThreadItem) -> Option<&str> {
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

fn tool_item_event(
    thread_id: &str,
    turn_id: &str,
    item: &ThreadItem,
    started_at: u64,
    completed_at: u64,
    connection_state: &ConnectionState,
    thread_metadata: Option<&ThreadMetadataState>,
) -> Option<TrackEventRequest> {
    match item {
        ThreadItem::CommandExecution {
            id,
            process_id,
            source,
            status,
            command_actions,
            exit_code,
            duration_ms,
            ..
        } => {
            let (terminal_status, failure_kind) = command_execution_outcome(status)?;
            let base = tool_item_base(
                thread_id,
                turn_id,
                id.clone(),
                command_execution_tool_name(*source).to_string(),
                ToolItemOutcome {
                    terminal_status,
                    failure_kind,
                    duration_ms: completed_duration_ms(
                        option_i64_to_u64(*duration_ms),
                        started_at,
                        completed_at,
                    ),
                },
                ToolItemContext {
                    started_at,
                    completed_at,
                    connection_state,
                    thread_metadata,
                },
            );
            Some(TrackEventRequest::CommandExecution(
                CodexCommandExecutionEventRequest {
                    event_type: "codex_command_execution_event",
                    event_params: CodexCommandExecutionEventParams {
                        base,
                        command_execution_source: command_execution_source_kind(*source),
                        command_execution_family: command_execution_family(*source),
                        process_id: process_id.clone(),
                        exit_code: *exit_code,
                        command_action_count: usize_to_u64(command_actions.len()),
                    },
                },
            ))
        }
        ThreadItem::FileChange {
            id,
            changes,
            status,
        } => {
            let (terminal_status, failure_kind) = patch_apply_outcome(status)?;
            let counts = file_change_counts(changes);
            let base = tool_item_base(
                thread_id,
                turn_id,
                id.clone(),
                "apply_patch".to_string(),
                ToolItemOutcome {
                    terminal_status,
                    failure_kind,
                    duration_ms: completed_duration_ms(None, started_at, completed_at),
                },
                ToolItemContext {
                    started_at,
                    completed_at,
                    connection_state,
                    thread_metadata,
                },
            );
            Some(TrackEventRequest::FileChange(CodexFileChangeEventRequest {
                event_type: "codex_file_change_event",
                event_params: CodexFileChangeEventParams {
                    base,
                    file_change_count: usize_to_u64(changes.len()),
                    file_add_count: counts.add,
                    file_update_count: counts.update,
                    file_delete_count: counts.delete,
                    file_move_count: counts.move_,
                },
            }))
        }
        ThreadItem::McpToolCall {
            id,
            server,
            tool,
            status,
            error,
            duration_ms,
            ..
        } => {
            let (terminal_status, failure_kind) = mcp_tool_call_outcome(status)?;
            let base = tool_item_base(
                thread_id,
                turn_id,
                id.clone(),
                tool.clone(),
                ToolItemOutcome {
                    terminal_status,
                    failure_kind,
                    duration_ms: completed_duration_ms(
                        option_i64_to_u64(*duration_ms),
                        started_at,
                        completed_at,
                    ),
                },
                ToolItemContext {
                    started_at,
                    completed_at,
                    connection_state,
                    thread_metadata,
                },
            );
            Some(TrackEventRequest::McpToolCall(
                CodexMcpToolCallEventRequest {
                    event_type: "codex_mcp_tool_call_event",
                    event_params: CodexMcpToolCallEventParams {
                        base,
                        mcp_server_name: server.clone(),
                        mcp_tool_name: tool.clone(),
                        mcp_error_present: error.is_some(),
                    },
                },
            ))
        }
        ThreadItem::DynamicToolCall {
            id,
            tool,
            status,
            content_items,
            success,
            duration_ms,
            ..
        } => {
            let (terminal_status, failure_kind) = dynamic_tool_call_outcome(status)?;
            let counts = content_items
                .as_ref()
                .map(|items| dynamic_content_counts(items));
            let base = tool_item_base(
                thread_id,
                turn_id,
                id.clone(),
                tool.clone(),
                ToolItemOutcome {
                    terminal_status,
                    failure_kind,
                    duration_ms: completed_duration_ms(
                        option_i64_to_u64(*duration_ms),
                        started_at,
                        completed_at,
                    ),
                },
                ToolItemContext {
                    started_at,
                    completed_at,
                    connection_state,
                    thread_metadata,
                },
            );
            Some(TrackEventRequest::DynamicToolCall(
                CodexDynamicToolCallEventRequest {
                    event_type: "codex_dynamic_tool_call_event",
                    event_params: CodexDynamicToolCallEventParams {
                        base,
                        dynamic_tool_name: tool.clone(),
                        success: *success,
                        output_content_item_count: counts.map(|counts| counts.total),
                        output_text_item_count: counts.map(|counts| counts.text),
                        output_image_item_count: counts.map(|counts| counts.image),
                    },
                },
            ))
        }
        ThreadItem::CollabAgentToolCall {
            id,
            tool,
            status,
            sender_thread_id,
            receiver_thread_ids,
            model,
            reasoning_effort,
            agents_states,
            ..
        } => {
            let (terminal_status, failure_kind) = collab_tool_call_outcome(status)?;
            let base = tool_item_base(
                thread_id,
                turn_id,
                id.clone(),
                collab_agent_tool_name(tool).to_string(),
                ToolItemOutcome {
                    terminal_status,
                    failure_kind,
                    duration_ms: completed_duration_ms(None, started_at, completed_at),
                },
                ToolItemContext {
                    started_at,
                    completed_at,
                    connection_state,
                    thread_metadata,
                },
            );
            Some(TrackEventRequest::CollabAgentToolCall(
                CodexCollabAgentToolCallEventRequest {
                    event_type: "codex_collab_agent_tool_call_event",
                    event_params: CodexCollabAgentToolCallEventParams {
                        base,
                        collab_agent_tool: collab_agent_tool_kind(tool),
                        sender_thread_id: sender_thread_id.clone(),
                        receiver_thread_count: usize_to_u64(receiver_thread_ids.len()),
                        receiver_thread_ids: receiver_thread_ids.clone(),
                        requested_model: model.clone(),
                        requested_reasoning_effort: reasoning_effort
                            .as_ref()
                            .and_then(serialize_enum_as_string),
                        agent_state_count: usize_to_u64(agents_states.len()),
                        completed_agent_count: usize_to_u64(
                            agents_states
                                .values()
                                .filter(|state| state.status == CollabAgentStatus::Completed)
                                .count(),
                        ),
                        failed_agent_count: usize_to_u64(
                            agents_states
                                .values()
                                .filter(|state| {
                                    matches!(
                                        state.status,
                                        CollabAgentStatus::Errored
                                            | CollabAgentStatus::Shutdown
                                            | CollabAgentStatus::NotFound
                                    )
                                })
                                .count(),
                        ),
                    },
                },
            ))
        }
        ThreadItem::WebSearch { id, query, action } => {
            let base = tool_item_base(
                thread_id,
                turn_id,
                id.clone(),
                "web_search".to_string(),
                ToolItemOutcome {
                    terminal_status: ToolItemTerminalStatus::Completed,
                    failure_kind: None,
                    duration_ms: completed_duration_ms(None, started_at, completed_at),
                },
                ToolItemContext {
                    started_at,
                    completed_at,
                    connection_state,
                    thread_metadata,
                },
            );
            Some(TrackEventRequest::WebSearch(CodexWebSearchEventRequest {
                event_type: "codex_web_search_event",
                event_params: CodexWebSearchEventParams {
                    base,
                    web_search_action: action.as_ref().map(web_search_action_kind),
                    query_present: !query.trim().is_empty(),
                    query_count: web_search_query_count(query, action.as_ref()),
                },
            }))
        }
        ThreadItem::ImageGeneration {
            id,
            status,
            revised_prompt,
            saved_path,
            ..
        } => {
            let (terminal_status, failure_kind) = image_generation_outcome(status.as_str());
            let base = tool_item_base(
                thread_id,
                turn_id,
                id.clone(),
                "image_generation".to_string(),
                ToolItemOutcome {
                    terminal_status,
                    failure_kind,
                    duration_ms: completed_duration_ms(None, started_at, completed_at),
                },
                ToolItemContext {
                    started_at,
                    completed_at,
                    connection_state,
                    thread_metadata,
                },
            );
            Some(TrackEventRequest::ImageGeneration(
                CodexImageGenerationEventRequest {
                    event_type: "codex_image_generation_event",
                    event_params: CodexImageGenerationEventParams {
                        base,
                        image_generation_status: status.clone(),
                        revised_prompt_present: revised_prompt.is_some(),
                        saved_path_present: saved_path.is_some(),
                    },
                },
            ))
        }
        _ => None,
    }
}

struct ToolItemOutcome {
    terminal_status: ToolItemTerminalStatus,
    failure_kind: Option<ToolItemFailureKind>,
    duration_ms: Option<u64>,
}

struct ToolItemContext<'a> {
    started_at: u64,
    completed_at: u64,
    connection_state: &'a ConnectionState,
    thread_metadata: Option<&'a ThreadMetadataState>,
}

fn tool_item_base(
    thread_id: &str,
    turn_id: &str,
    item_id: String,
    tool_name: String,
    outcome: ToolItemOutcome,
    context: ToolItemContext<'_>,
) -> CodexToolItemEventBase {
    let thread_metadata = context.thread_metadata.cloned().unwrap_or_default();
    CodexToolItemEventBase {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        item_id,
        app_server_client: context.connection_state.app_server_client.clone(),
        runtime: context.connection_state.runtime.clone(),
        thread_source: thread_metadata.thread_source,
        subagent_source: thread_metadata.subagent_source,
        parent_thread_id: thread_metadata.parent_thread_id,
        tool_name,
        started_at: context.started_at,
        completed_at: Some(context.completed_at),
        duration_ms: outcome.duration_ms,
        execution_started: true,
        review_count: 0,
        guardian_review_count: 0,
        user_review_count: 0,
        final_approval_outcome: ToolItemFinalApprovalOutcome::NotNeeded,
        terminal_status: outcome.terminal_status,
        failure_kind: outcome.failure_kind,
        requested_additional_permissions: false,
        requested_network_access: false,
        retry_count: 0,
    }
}

fn completed_duration_ms(
    item_duration_ms: Option<u64>,
    started_at: u64,
    completed_at: u64,
) -> Option<u64> {
    item_duration_ms.or_else(|| {
        completed_at
            .checked_sub(started_at)
            .map(|duration_secs| duration_secs.saturating_mul(1000))
    })
}

fn command_execution_source_kind(source: CommandExecutionSource) -> CommandExecutionSourceKind {
    match source {
        CommandExecutionSource::Agent => CommandExecutionSourceKind::Agent,
        CommandExecutionSource::UserShell => CommandExecutionSourceKind::UserShell,
        CommandExecutionSource::UnifiedExecStartup => {
            CommandExecutionSourceKind::UnifiedExecStartup
        }
        CommandExecutionSource::UnifiedExecInteraction => {
            CommandExecutionSourceKind::UnifiedExecInteraction
        }
    }
}

fn command_execution_family(source: CommandExecutionSource) -> CommandExecutionFamily {
    match source {
        CommandExecutionSource::Agent => CommandExecutionFamily::Shell,
        CommandExecutionSource::UserShell => CommandExecutionFamily::UserShell,
        CommandExecutionSource::UnifiedExecStartup
        | CommandExecutionSource::UnifiedExecInteraction => CommandExecutionFamily::UnifiedExec,
    }
}

fn command_execution_tool_name(source: CommandExecutionSource) -> &'static str {
    match source {
        CommandExecutionSource::UnifiedExecStartup
        | CommandExecutionSource::UnifiedExecInteraction => "unified_exec",
        CommandExecutionSource::UserShell => "user_shell",
        CommandExecutionSource::Agent => "shell",
    }
}

fn command_execution_outcome(
    status: &CommandExecutionStatus,
) -> Option<(ToolItemTerminalStatus, Option<ToolItemFailureKind>)> {
    match status {
        CommandExecutionStatus::InProgress => None,
        CommandExecutionStatus::Completed => Some((ToolItemTerminalStatus::Completed, None)),
        CommandExecutionStatus::Failed => Some((
            ToolItemTerminalStatus::Failed,
            Some(ToolItemFailureKind::ToolError),
        )),
        CommandExecutionStatus::Declined => Some((
            ToolItemTerminalStatus::Rejected,
            Some(ToolItemFailureKind::ApprovalDenied),
        )),
    }
}

fn patch_apply_outcome(
    status: &PatchApplyStatus,
) -> Option<(ToolItemTerminalStatus, Option<ToolItemFailureKind>)> {
    match status {
        PatchApplyStatus::InProgress => None,
        PatchApplyStatus::Completed => Some((ToolItemTerminalStatus::Completed, None)),
        PatchApplyStatus::Failed => Some((
            ToolItemTerminalStatus::Failed,
            Some(ToolItemFailureKind::ToolError),
        )),
        PatchApplyStatus::Declined => Some((
            ToolItemTerminalStatus::Rejected,
            Some(ToolItemFailureKind::ApprovalDenied),
        )),
    }
}

fn mcp_tool_call_outcome(
    status: &McpToolCallStatus,
) -> Option<(ToolItemTerminalStatus, Option<ToolItemFailureKind>)> {
    match status {
        McpToolCallStatus::InProgress => None,
        McpToolCallStatus::Completed => Some((ToolItemTerminalStatus::Completed, None)),
        McpToolCallStatus::Failed => Some((
            ToolItemTerminalStatus::Failed,
            Some(ToolItemFailureKind::ToolError),
        )),
    }
}

fn dynamic_tool_call_outcome(
    status: &DynamicToolCallStatus,
) -> Option<(ToolItemTerminalStatus, Option<ToolItemFailureKind>)> {
    match status {
        DynamicToolCallStatus::InProgress => None,
        DynamicToolCallStatus::Completed => Some((ToolItemTerminalStatus::Completed, None)),
        DynamicToolCallStatus::Failed => Some((
            ToolItemTerminalStatus::Failed,
            Some(ToolItemFailureKind::ToolError),
        )),
    }
}

fn collab_tool_call_outcome(
    status: &CollabAgentToolCallStatus,
) -> Option<(ToolItemTerminalStatus, Option<ToolItemFailureKind>)> {
    match status {
        CollabAgentToolCallStatus::InProgress => None,
        CollabAgentToolCallStatus::Completed => Some((ToolItemTerminalStatus::Completed, None)),
        CollabAgentToolCallStatus::Failed => Some((
            ToolItemTerminalStatus::Failed,
            Some(ToolItemFailureKind::ToolError),
        )),
    }
}

fn image_generation_outcome(status: &str) -> (ToolItemTerminalStatus, Option<ToolItemFailureKind>) {
    match status {
        "failed" | "error" => (
            ToolItemTerminalStatus::Failed,
            Some(ToolItemFailureKind::ToolError),
        ),
        _ => (ToolItemTerminalStatus::Completed, None),
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

fn collab_agent_tool_kind(tool: &CollabAgentTool) -> CollabAgentToolKind {
    match tool {
        CollabAgentTool::SpawnAgent => CollabAgentToolKind::SpawnAgent,
        CollabAgentTool::SendInput => CollabAgentToolKind::SendInput,
        CollabAgentTool::ResumeAgent => CollabAgentToolKind::ResumeAgent,
        CollabAgentTool::Wait => CollabAgentToolKind::Wait,
        CollabAgentTool::CloseAgent => CollabAgentToolKind::CloseAgent,
    }
}

#[derive(Default)]
struct FileChangeCounts {
    add: u64,
    update: u64,
    delete: u64,
    move_: u64,
}

fn file_change_counts(changes: &[codex_app_server_protocol::FileUpdateChange]) -> FileChangeCounts {
    let mut counts = FileChangeCounts::default();
    for change in changes {
        match &change.kind {
            PatchChangeKind::Add => counts.add += 1,
            PatchChangeKind::Delete => counts.delete += 1,
            PatchChangeKind::Update { move_path: Some(_) } => counts.move_ += 1,
            PatchChangeKind::Update { move_path: None } => counts.update += 1,
        }
    }
    counts
}

#[derive(Clone, Copy)]
struct DynamicContentCounts {
    total: u64,
    text: u64,
    image: u64,
}

fn dynamic_content_counts(items: &[DynamicToolCallOutputContentItem]) -> DynamicContentCounts {
    let mut text = 0;
    let mut image = 0;
    for item in items {
        match item {
            DynamicToolCallOutputContentItem::InputText { .. } => text += 1,
            DynamicToolCallOutputContentItem::InputImage { .. } => image += 1,
        }
    }
    DynamicContentCounts {
        total: usize_to_u64(items.len()),
        text,
        image,
    }
}

fn web_search_action_kind(action: &WebSearchAction) -> WebSearchActionKind {
    match action {
        WebSearchAction::Search { .. } => WebSearchActionKind::Search,
        WebSearchAction::OpenPage { .. } => WebSearchActionKind::OpenPage,
        WebSearchAction::FindInPage { .. } => WebSearchActionKind::FindInPage,
        WebSearchAction::Other => WebSearchActionKind::Other,
    }
}

fn web_search_query_count(query: &str, action: Option<&WebSearchAction>) -> Option<u64> {
    match action {
        Some(WebSearchAction::Search { query, queries }) => queries
            .as_ref()
            .map(|queries| usize_to_u64(queries.len()))
            .or_else(|| query.as_ref().map(|_| 1)),
        Some(WebSearchAction::OpenPage { .. })
        | Some(WebSearchAction::FindInPage { .. })
        | Some(WebSearchAction::Other) => None,
        None => (!query.trim().is_empty()).then_some(1),
    }
}

fn serialize_enum_as_string<T: serde::Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
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
