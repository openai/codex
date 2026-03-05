use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use codex_app_server_client::ClientSurface;
use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessClientStartArgs;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::ApplyPatchApprovalResponse;
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
use codex_core::config::Config;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::LoaderOverrides;
use codex_feedback::CodexFeedback;
use codex_protocol::ThreadId;
use codex_protocol::approvals::ApplyPatchApprovalRequestEvent;
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
    request_user_input: HashMap<String, VecDeque<RequestId>>,
    dynamic_tool_calls: HashMap<String, RequestId>,
    pending_file_changes: HashMap<String, HashMap<PathBuf, FileChange>>,
}

impl PendingServerRequests {
    fn clear_turn_scoped(&mut self) {
        self.exec_approvals.clear();
        self.patch_approvals.clear();
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

fn resolve_elicitation_deferred_message() -> String {
    "ResolveElicitation is temporarily unavailable in in-process local-only mode".to_string()
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

fn legacy_notification_to_event(notification: JSONRPCNotification) -> Result<Event, String> {
    let mut value = notification
        .params
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    let serde_json::Value::Object(ref mut object) = value else {
        return Err(format!(
            "legacy notification `{}` params were not an object",
            notification.method
        ));
    };
    object.insert(
        "type".to_string(),
        serde_json::Value::String(notification.method),
    );

    let msg: EventMsg =
        serde_json::from_value(value).map_err(|err| format!("failed to decode event: {err}"))?;
    Ok(Event {
        id: String::new(),
        msg,
    })
}

async fn process_in_process_command(
    op: Op,
    thread_id: &str,
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
        Op::ResolveElicitation { .. } => {
            // TODO(fcoury): support this once app-server protocol has a server-request
            // variant for MCP elicitation and a corresponding typed response payload.
            // This branch intentionally avoids protocol expansion.
            send_warning_event(app_event_tx, resolve_elicitation_deferred_message());
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

async fn run_in_process_agent_loop(
    mut codex_op_rx: tokio::sync::mpsc::UnboundedReceiver<Op>,
    mut client: InProcessAppServerClient,
    thread_id: String,
    _session_id: ThreadId,
    app_event_tx: AppEventSender,
    mut request_ids: RequestIdSequencer,
    mut current_turn_id: Option<String>,
) {
    let mut pending_shutdown_complete = false;
    let mut pending_server_requests = PendingServerRequests::default();
    loop {
        tokio::select! {
            maybe_op = codex_op_rx.recv() => {
                match maybe_op {
                    Some(op) => {
                        let should_shutdown = process_in_process_command(
                            op,
                            &thread_id,
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
                            ServerRequest::ChatgptAuthTokensRefresh { request_id, .. } => {
                                // TODO(fcoury): wire a local token-refresh adapter for in-process TUI.
                                // For now we reject explicitly to avoid request timeouts.
                                reject_server_request(
                                    &client,
                                    request_id,
                                    &method,
                                    "chatgpt auth token refresh is not wired for in-process TUI yet"
                                        .to_string(),
                                    &app_event_tx,
                                )
                                .await;
                            }
                        }
                    }
                    InProcessServerEvent::ServerNotification(notification) => {
                        if let ServerNotification::ItemStarted(notification) = notification
                            && notification.thread_id == thread_id
                            && let ThreadItem::FileChange { id, changes, .. } = notification.item
                        {
                            pending_server_requests
                                .pending_file_changes
                                .insert(id, file_update_changes_to_core(changes));
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
                        if matches!(event.msg, EventMsg::SessionConfigured(_)) {
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
                        send_warning_event(
                            &app_event_tx,
                            format!("in-process app-server event stream lagged; dropped {skipped} events"),
                        );
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
        let session_id = session_configured.session_id;
        send_codex_event(
            &app_event_tx_clone,
            EventMsg::SessionConfigured(session_configured),
        );

        run_in_process_agent_loop(
            codex_op_rx,
            client,
            thread_id,
            session_id,
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

        let session_id = session_configured.session_id;
        let thread_id = session_id.to_string();
        let current_turn_id = active_turn_id_from_turns(&thread_resume.thread.turns);
        send_codex_event(
            &app_event_tx_clone,
            EventMsg::SessionConfigured(session_configured),
        );

        run_in_process_agent_loop(
            codex_op_rx,
            client,
            thread_id,
            session_id,
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
    use codex_core::config::ConfigBuilder;
    use codex_protocol::protocol::ConversationAudioParams;
    use codex_protocol::protocol::ConversationStartParams;
    use codex_protocol::protocol::ConversationTextParams;
    use codex_protocol::protocol::RealtimeAudioFrame;
    use pretty_assertions::assert_eq;
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
}
