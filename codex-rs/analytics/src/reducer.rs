use crate::events::AppServerRpcTransport;
use crate::events::CodexAppMentionedEventRequest;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexAppUsedEventRequest;
use crate::events::CodexCompactionEventRequest;
use crate::events::CodexHookRunEventRequest;
use crate::events::CodexPluginEventRequest;
use crate::events::CodexPluginUsedEventRequest;
use crate::events::CodexRuntimeMetadata;
use crate::events::CodexTurnEventParams;
use crate::events::CodexTurnEventRequest;
use crate::events::CodexTurnSteerEventParams;
use crate::events::CodexTurnSteerEventRequest;
use crate::events::GuardianReviewEventParams;
use crate::events::GuardianReviewEventPayload;
use crate::events::GuardianReviewEventRequest;
use crate::events::SkillInvocationEventParams;
use crate::events::SkillInvocationEventRequest;
use crate::events::ThreadInitializedEvent;
use crate::events::ThreadInitializedEventParams;
use crate::events::TrackEventRequest;
use crate::events::codex_app_metadata;
use crate::events::codex_compaction_event_params;
use crate::events::codex_hook_run_metadata;
use crate::events::codex_plugin_metadata;
use crate::events::codex_plugin_used_metadata;
use crate::events::plugin_state_event_type;
use crate::events::subagent_parent_thread_id;
use crate::events::subagent_source_name;
use crate::events::subagent_thread_started_event_request;
use crate::facts::AnalyticsFact;
use crate::facts::AnalyticsJsonRpcError;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::CodexCompactionEvent;
use crate::facts::CustomAnalyticsFact;
use crate::facts::HookRunInput;
use crate::facts::PluginState;
use crate::facts::PluginStateChangedInput;
use crate::facts::PluginUsedInput;
use crate::facts::SkillInvokedInput;
use crate::facts::SubAgentThreadStartedInput;
use crate::facts::ThreadInitializationMode;
use crate::facts::TurnResolvedConfigFact;
use crate::facts::TurnStatus;
use crate::facts::TurnSteerRejectionReason;
use crate::facts::TurnSteerResult;
use crate::facts::TurnTokenUsageFact;
use crate::now_unix_seconds;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::TurnSteerResponse;
use codex_app_server_protocol::UserInput;
use codex_git_utils::collect_git_info;
use codex_git_utils::get_git_repo_root;
use codex_login::default_client::originator;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::TokenUsage;
use sha1::Digest;
use std::collections::HashMap;
use std::path::Path;

#[derive(Default)]
pub(crate) struct AnalyticsReducer {
    requests: HashMap<(u64, RequestId), RequestState>,
    turns: HashMap<String, TurnState>,
    connections: HashMap<u64, ConnectionState>,
    thread_connections: HashMap<String, u64>,
    thread_metadata: HashMap<String, ThreadMetadataState>,
}

struct ConnectionState {
    app_server_client: CodexAppServerClientMetadata,
    runtime: CodexRuntimeMetadata,
}

#[derive(Clone)]
struct ThreadMetadataState {
    thread_source: Option<&'static str>,
    initialization_mode: ThreadInitializationMode,
    subagent_source: Option<String>,
    parent_thread_id: Option<String>,
}

impl ThreadMetadataState {
    fn from_thread_metadata(
        session_source: &SessionSource,
        initialization_mode: ThreadInitializationMode,
    ) -> Self {
        let (subagent_source, parent_thread_id) = match session_source {
            SessionSource::SubAgent(subagent_source) => (
                Some(subagent_source_name(subagent_source)),
                subagent_parent_thread_id(subagent_source),
            ),
            SessionSource::Cli
            | SessionSource::VSCode
            | SessionSource::Exec
            | SessionSource::Mcp
            | SessionSource::Custom(_)
            | SessionSource::Unknown => (None, None),
        };
        Self {
            thread_source: session_source.thread_source_name(),
            initialization_mode,
            subagent_source,
            parent_thread_id,
        }
    }
}

enum RequestState {
    TurnStart(PendingTurnStartState),
    TurnSteer(PendingTurnSteerState),
}

struct PendingTurnStartState {
    thread_id: String,
    num_input_images: usize,
}

struct PendingTurnSteerState {
    thread_id: String,
    expected_turn_id: String,
    num_input_images: usize,
    created_at: u64,
}

#[derive(Clone)]
struct CompletedTurnState {
    status: Option<TurnStatus>,
    turn_error: Option<CodexErrorInfo>,
    completed_at: u64,
    duration_ms: Option<u64>,
}

struct TurnState {
    connection_id: Option<u64>,
    thread_id: Option<String>,
    num_input_images: Option<usize>,
    resolved_config: Option<TurnResolvedConfigFact>,
    started_at: Option<u64>,
    token_usage: Option<TokenUsage>,
    completed: Option<CompletedTurnState>,
    steer_count: usize,
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
            AnalyticsFact::Request {
                connection_id,
                request_id,
                request,
            } => {
                self.ingest_request(connection_id, request_id, *request);
            }
            AnalyticsFact::Response {
                connection_id,
                response,
            } => {
                self.ingest_response(connection_id, *response, out);
            }
            AnalyticsFact::ErrorResponse {
                connection_id,
                request_id,
                error: _,
                error_type,
            } => {
                self.ingest_error_response(connection_id, request_id, error_type, out);
            }
            AnalyticsFact::Notification(notification) => {
                self.ingest_notification(*notification, out);
            }
            AnalyticsFact::Custom(input) => match input {
                CustomAnalyticsFact::SubAgentThreadStarted(input) => {
                    self.ingest_subagent_thread_started(input, out);
                }
                CustomAnalyticsFact::Compaction(input) => {
                    self.ingest_compaction(*input, out);
                }
                CustomAnalyticsFact::GuardianReview(input) => {
                    self.ingest_guardian_review(*input, out);
                }
                CustomAnalyticsFact::TurnResolvedConfig(input) => {
                    self.ingest_turn_resolved_config(*input, out);
                }
                CustomAnalyticsFact::TurnTokenUsage(input) => {
                    self.ingest_turn_token_usage(*input, out);
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
                CustomAnalyticsFact::HookRun(input) => {
                    self.ingest_hook_run(input, out);
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

    fn ingest_guardian_review(
        &mut self,
        input: GuardianReviewEventParams,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let Some(connection_id) = self.thread_connections.get(&input.thread_id) else {
            tracing::warn!(
                thread_id = %input.thread_id,
                turn_id = %input.turn_id,
                review_id = %input.review_id,
                "dropping guardian analytics event: missing thread connection metadata"
            );
            return;
        };
        let Some(connection_state) = self.connections.get(connection_id) else {
            tracing::warn!(
                thread_id = %input.thread_id,
                turn_id = %input.turn_id,
                review_id = %input.review_id,
                connection_id,
                "dropping guardian analytics event: missing connection metadata"
            );
            return;
        };
        out.push(TrackEventRequest::GuardianReview(Box::new(
            GuardianReviewEventRequest {
                event_type: "codex_guardian_review",
                event_params: GuardianReviewEventPayload {
                    app_server_client: connection_state.app_server_client.clone(),
                    runtime: connection_state.runtime.clone(),
                    guardian_review: input,
                },
            },
        )));
    }

    fn ingest_request(
        &mut self,
        connection_id: u64,
        request_id: RequestId,
        request: ClientRequest,
    ) {
        match request {
            ClientRequest::TurnStart { params, .. } => {
                self.requests.insert(
                    (connection_id, request_id),
                    RequestState::TurnStart(PendingTurnStartState {
                        thread_id: params.thread_id,
                        num_input_images: num_input_images(&params.input),
                    }),
                );
            }
            ClientRequest::TurnSteer { params, .. } => {
                self.requests.insert(
                    (connection_id, request_id),
                    RequestState::TurnSteer(PendingTurnSteerState {
                        thread_id: params.thread_id,
                        expected_turn_id: params.expected_turn_id,
                        num_input_images: num_input_images(&params.input),
                        created_at: now_unix_seconds(),
                    }),
                );
            }
            _ => {}
        }
    }

    fn ingest_turn_resolved_config(
        &mut self,
        input: TurnResolvedConfigFact,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let turn_id = input.turn_id.clone();
        let thread_id = input.thread_id.clone();
        let num_input_images = input.num_input_images;
        let turn_state = self.turns.entry(turn_id.clone()).or_insert(TurnState {
            connection_id: None,
            thread_id: None,
            num_input_images: None,
            resolved_config: None,
            started_at: None,
            token_usage: None,
            completed: None,
            steer_count: 0,
        });
        turn_state.thread_id = Some(thread_id);
        turn_state.num_input_images = Some(num_input_images);
        turn_state.resolved_config = Some(input);
        self.maybe_emit_turn_event(&turn_id, out);
    }

    fn ingest_turn_token_usage(
        &mut self,
        input: TurnTokenUsageFact,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let turn_id = input.turn_id.clone();
        let turn_state = self.turns.entry(turn_id.clone()).or_insert(TurnState {
            connection_id: None,
            thread_id: None,
            num_input_images: None,
            resolved_config: None,
            started_at: None,
            token_usage: None,
            completed: None,
            steer_count: 0,
        });
        turn_state.thread_id = Some(input.thread_id);
        turn_state.token_usage = Some(input.token_usage);
        self.maybe_emit_turn_event(&turn_id, out);
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

    fn ingest_hook_run(&mut self, input: HookRunInput, out: &mut Vec<TrackEventRequest>) {
        let HookRunInput { tracking, hook } = input;
        out.push(TrackEventRequest::HookRun(CodexHookRunEventRequest {
            event_type: "codex_hook_run",
            event_params: codex_hook_run_metadata(&tracking, hook),
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
        match response {
            ClientResponse::ThreadStart { response, .. } => {
                self.emit_thread_initialized(
                    connection_id,
                    response.thread,
                    response.model,
                    ThreadInitializationMode::New,
                    out,
                );
            }
            ClientResponse::ThreadResume { response, .. } => {
                self.emit_thread_initialized(
                    connection_id,
                    response.thread,
                    response.model,
                    ThreadInitializationMode::Resumed,
                    out,
                );
            }
            ClientResponse::ThreadFork { response, .. } => {
                self.emit_thread_initialized(
                    connection_id,
                    response.thread,
                    response.model,
                    ThreadInitializationMode::Forked,
                    out,
                );
            }
            ClientResponse::TurnStart {
                request_id,
                response,
            } => {
                let turn_id = response.turn.id;
                let Some(RequestState::TurnStart(pending_request)) =
                    self.requests.remove(&(connection_id, request_id))
                else {
                    return;
                };
                let turn_state = self.turns.entry(turn_id.clone()).or_insert(TurnState {
                    connection_id: None,
                    thread_id: None,
                    num_input_images: None,
                    resolved_config: None,
                    started_at: None,
                    token_usage: None,
                    completed: None,
                    steer_count: 0,
                });
                turn_state.connection_id = Some(connection_id);
                turn_state.thread_id = Some(pending_request.thread_id);
                turn_state.num_input_images = Some(pending_request.num_input_images);
                self.maybe_emit_turn_event(&turn_id, out);
            }
            ClientResponse::TurnSteer {
                request_id,
                response,
            } => {
                self.ingest_turn_steer_response(connection_id, request_id, response, out);
            }
            _ => {}
        }
    }

    fn ingest_error_response(
        &mut self,
        connection_id: u64,
        request_id: RequestId,
        error_type: Option<AnalyticsJsonRpcError>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let Some(request) = self.requests.remove(&(connection_id, request_id)) else {
            return;
        };
        self.ingest_request_error_response(connection_id, request, error_type, out);
    }

    fn ingest_request_error_response(
        &mut self,
        connection_id: u64,
        request: RequestState,
        error_type: Option<AnalyticsJsonRpcError>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        match request {
            RequestState::TurnStart(_) => {}
            RequestState::TurnSteer(pending_request) => {
                self.ingest_turn_steer_error_response(
                    connection_id,
                    pending_request,
                    error_type,
                    out,
                );
            }
        }
    }

    fn ingest_turn_steer_error_response(
        &mut self,
        connection_id: u64,
        pending_request: PendingTurnSteerState,
        error_type: Option<AnalyticsJsonRpcError>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        self.emit_turn_steer_event(
            connection_id,
            pending_request,
            /*accepted_turn_id*/ None,
            TurnSteerResult::Rejected,
            rejection_reason_from_error_type(error_type),
            out,
        );
    }

    fn ingest_notification(
        &mut self,
        notification: ServerNotification,
        out: &mut Vec<TrackEventRequest>,
    ) {
        match notification {
            ServerNotification::TurnStarted(notification) => {
                let turn_state = self.turns.entry(notification.turn.id).or_insert(TurnState {
                    connection_id: None,
                    thread_id: None,
                    num_input_images: None,
                    resolved_config: None,
                    started_at: None,
                    token_usage: None,
                    completed: None,
                    steer_count: 0,
                });
                turn_state.started_at = notification
                    .turn
                    .started_at
                    .and_then(|started_at| u64::try_from(started_at).ok());
            }
            ServerNotification::TurnCompleted(notification) => {
                let turn_state =
                    self.turns
                        .entry(notification.turn.id.clone())
                        .or_insert(TurnState {
                            connection_id: None,
                            thread_id: None,
                            num_input_images: None,
                            resolved_config: None,
                            started_at: None,
                            token_usage: None,
                            completed: None,
                            steer_count: 0,
                        });
                turn_state.completed = Some(CompletedTurnState {
                    status: analytics_turn_status(notification.turn.status),
                    turn_error: notification
                        .turn
                        .error
                        .and_then(|error| error.codex_error_info),
                    completed_at: notification
                        .turn
                        .completed_at
                        .and_then(|completed_at| u64::try_from(completed_at).ok())
                        .unwrap_or_default(),
                    duration_ms: notification
                        .turn
                        .duration_ms
                        .and_then(|duration_ms| u64::try_from(duration_ms).ok()),
                });
                let turn_id = notification.turn.id;
                self.maybe_emit_turn_event(&turn_id, out);
            }
            _ => {}
        }
    }

    fn emit_thread_initialized(
        &mut self,
        connection_id: u64,
        thread: codex_app_server_protocol::Thread,
        model: String,
        initialization_mode: ThreadInitializationMode,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let thread_source: SessionSource = thread.source.into();
        let thread_id = thread.id;
        let Some(connection_state) = self.connections.get(&connection_id) else {
            return;
        };
        let thread_metadata =
            ThreadMetadataState::from_thread_metadata(&thread_source, initialization_mode);
        self.thread_connections
            .insert(thread_id.clone(), connection_id);
        self.thread_metadata
            .insert(thread_id.clone(), thread_metadata.clone());
        out.push(TrackEventRequest::ThreadInitialized(
            ThreadInitializedEvent {
                event_type: "codex_thread_initialized",
                event_params: ThreadInitializedEventParams {
                    thread_id,
                    app_server_client: connection_state.app_server_client.clone(),
                    runtime: connection_state.runtime.clone(),
                    model,
                    ephemeral: thread.ephemeral,
                    thread_source: thread_metadata.thread_source,
                    initialization_mode,
                    subagent_source: thread_metadata.subagent_source,
                    parent_thread_id: thread_metadata.parent_thread_id,
                    created_at: u64::try_from(thread.created_at).unwrap_or_default(),
                },
            },
        ));
    }

    fn ingest_compaction(&mut self, input: CodexCompactionEvent, out: &mut Vec<TrackEventRequest>) {
        let Some(connection_id) = self.thread_connections.get(&input.thread_id) else {
            tracing::warn!(
                thread_id = %input.thread_id,
                turn_id = %input.turn_id,
                "dropping compaction analytics event: missing thread connection metadata"
            );
            return;
        };
        let Some(connection_state) = self.connections.get(connection_id) else {
            tracing::warn!(
                thread_id = %input.thread_id,
                turn_id = %input.turn_id,
                connection_id,
                "dropping compaction analytics event: missing connection metadata"
            );
            return;
        };
        let Some(thread_metadata) = self.thread_metadata.get(&input.thread_id) else {
            tracing::warn!(
                thread_id = %input.thread_id,
                turn_id = %input.turn_id,
                "dropping compaction analytics event: missing thread lifecycle metadata"
            );
            return;
        };
        out.push(TrackEventRequest::Compaction(Box::new(
            CodexCompactionEventRequest {
                event_type: "codex_compaction_event",
                event_params: codex_compaction_event_params(
                    input,
                    connection_state.app_server_client.clone(),
                    connection_state.runtime.clone(),
                    thread_metadata.thread_source,
                    thread_metadata.subagent_source.clone(),
                    thread_metadata.parent_thread_id.clone(),
                ),
            },
        )));
    }

    fn ingest_turn_steer_response(
        &mut self,
        connection_id: u64,
        request_id: RequestId,
        response: TurnSteerResponse,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let Some(RequestState::TurnSteer(pending_request)) =
            self.requests.remove(&(connection_id, request_id))
        else {
            return;
        };
        if let Some(turn_state) = self.turns.get_mut(&response.turn_id) {
            turn_state.steer_count += 1;
        }
        self.emit_turn_steer_event(
            connection_id,
            pending_request,
            Some(response.turn_id),
            TurnSteerResult::Accepted,
            /*rejection_reason*/ None,
            out,
        );
    }

    fn emit_turn_steer_event(
        &mut self,
        connection_id: u64,
        pending_request: PendingTurnSteerState,
        accepted_turn_id: Option<String>,
        result: TurnSteerResult,
        rejection_reason: Option<TurnSteerRejectionReason>,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let Some(connection_state) = self.connections.get(&connection_id) else {
            return;
        };
        let Some(thread_metadata) = self.thread_metadata.get(&pending_request.thread_id) else {
            tracing::warn!(
                thread_id = %pending_request.thread_id,
                "dropping turn steer analytics event: missing thread lifecycle metadata"
            );
            return;
        };
        out.push(TrackEventRequest::TurnSteer(CodexTurnSteerEventRequest {
            event_type: "codex_turn_steer_event",
            event_params: CodexTurnSteerEventParams {
                thread_id: pending_request.thread_id,
                expected_turn_id: Some(pending_request.expected_turn_id),
                accepted_turn_id,
                app_server_client: connection_state.app_server_client.clone(),
                runtime: connection_state.runtime.clone(),
                thread_source: thread_metadata.thread_source.map(str::to_string),
                subagent_source: thread_metadata.subagent_source.clone(),
                parent_thread_id: thread_metadata.parent_thread_id.clone(),
                num_input_images: pending_request.num_input_images,
                result,
                rejection_reason,
                created_at: pending_request.created_at,
            },
        }));
    }

    fn maybe_emit_turn_event(&mut self, turn_id: &str, out: &mut Vec<TrackEventRequest>) {
        let Some(turn_state) = self.turns.get(turn_id) else {
            return;
        };
        if turn_state.thread_id.is_none()
            || turn_state.num_input_images.is_none()
            || turn_state.resolved_config.is_none()
            || turn_state.completed.is_none()
        {
            return;
        }
        let connection_metadata = turn_state
            .connection_id
            .and_then(|connection_id| self.connections.get(&connection_id))
            .map(|connection_state| {
                (
                    connection_state.app_server_client.clone(),
                    connection_state.runtime.clone(),
                )
            });
        let Some((app_server_client, runtime)) = connection_metadata else {
            if let Some(connection_id) = turn_state.connection_id {
                tracing::warn!(
                    turn_id,
                    connection_id,
                    "dropping turn analytics event: missing connection metadata"
                );
            }
            return;
        };
        let Some(thread_id) = turn_state.thread_id.as_ref() else {
            return;
        };
        let Some(thread_metadata) = self.thread_metadata.get(thread_id) else {
            tracing::warn!(
                thread_id,
                turn_id,
                "dropping turn analytics event: missing thread lifecycle metadata"
            );
            return;
        };
        out.push(TrackEventRequest::TurnEvent(Box::new(
            CodexTurnEventRequest {
                event_type: "codex_turn_event",
                event_params: codex_turn_event_params(
                    app_server_client,
                    runtime,
                    turn_id.to_string(),
                    turn_state,
                    thread_metadata,
                ),
            },
        )));
        self.turns.remove(turn_id);
    }
}

fn codex_turn_event_params(
    app_server_client: CodexAppServerClientMetadata,
    runtime: CodexRuntimeMetadata,
    turn_id: String,
    turn_state: &TurnState,
    thread_metadata: &ThreadMetadataState,
) -> CodexTurnEventParams {
    let (Some(thread_id), Some(num_input_images), Some(resolved_config), Some(completed)) = (
        turn_state.thread_id.clone(),
        turn_state.num_input_images,
        turn_state.resolved_config.clone(),
        turn_state.completed.clone(),
    ) else {
        unreachable!("turn event params require a fully populated turn state");
    };
    let started_at = turn_state.started_at;
    let TurnResolvedConfigFact {
        turn_id: _resolved_turn_id,
        thread_id: _resolved_thread_id,
        num_input_images: _resolved_num_input_images,
        submission_type,
        ephemeral,
        session_source: _session_source,
        model,
        model_provider,
        permission_profile,
        permission_profile_cwd,
        reasoning_effort,
        reasoning_summary,
        service_tier,
        approval_policy,
        approvals_reviewer,
        sandbox_network_access,
        collaboration_mode,
        personality,
        is_first_turn,
    } = resolved_config;
    let token_usage = turn_state.token_usage.clone();
    CodexTurnEventParams {
        thread_id,
        turn_id,
        app_server_client,
        runtime,
        submission_type,
        ephemeral,
        thread_source: thread_metadata.thread_source.map(str::to_string),
        initialization_mode: thread_metadata.initialization_mode,
        subagent_source: thread_metadata.subagent_source.clone(),
        parent_thread_id: thread_metadata.parent_thread_id.clone(),
        model: Some(model),
        model_provider,
        sandbox_policy: Some(sandbox_policy_mode(
            &permission_profile,
            permission_profile_cwd.as_path(),
        )),
        reasoning_effort: reasoning_effort.map(|value| value.to_string()),
        reasoning_summary: reasoning_summary_mode(reasoning_summary),
        service_tier: service_tier
            .map(|value| value.to_string())
            .unwrap_or_else(|| "default".to_string()),
        approval_policy: approval_policy.to_string(),
        approvals_reviewer: approvals_reviewer.to_string(),
        sandbox_network_access,
        collaboration_mode: Some(collaboration_mode_mode(collaboration_mode)),
        personality: personality_mode(personality),
        num_input_images,
        is_first_turn,
        status: completed.status,
        turn_error: completed.turn_error,
        steer_count: Some(turn_state.steer_count),
        total_tool_call_count: None,
        shell_command_count: None,
        file_change_count: None,
        mcp_tool_call_count: None,
        dynamic_tool_call_count: None,
        subagent_tool_call_count: None,
        web_search_count: None,
        image_generation_count: None,
        input_tokens: token_usage
            .as_ref()
            .map(|token_usage| token_usage.input_tokens),
        cached_input_tokens: token_usage
            .as_ref()
            .map(|token_usage| token_usage.cached_input_tokens),
        output_tokens: token_usage
            .as_ref()
            .map(|token_usage| token_usage.output_tokens),
        reasoning_output_tokens: token_usage
            .as_ref()
            .map(|token_usage| token_usage.reasoning_output_tokens),
        total_tokens: token_usage
            .as_ref()
            .map(|token_usage| token_usage.total_tokens),
        duration_ms: completed.duration_ms,
        started_at,
        completed_at: Some(completed.completed_at),
    }
}

fn sandbox_policy_mode(permission_profile: &PermissionProfile, cwd: &Path) -> &'static str {
    match permission_profile {
        PermissionProfile::Disabled => "full_access",
        PermissionProfile::External { .. } => "external_sandbox",
        PermissionProfile::Managed { .. } => {
            match permission_profile.to_legacy_sandbox_policy(cwd) {
                Ok(SandboxPolicy::DangerFullAccess) => "full_access",
                Ok(SandboxPolicy::ReadOnly { .. }) => "read_only",
                Ok(SandboxPolicy::WorkspaceWrite { .. }) => "workspace_write",
                Ok(SandboxPolicy::ExternalSandbox { .. }) => "external_sandbox",
                Err(_) => "workspace_write",
            }
        }
    }
}

fn collaboration_mode_mode(mode: ModeKind) -> &'static str {
    match mode {
        ModeKind::Plan => "plan",
        ModeKind::Default | ModeKind::PairProgramming | ModeKind::Execute => "default",
    }
}

fn reasoning_summary_mode(summary: Option<ReasoningSummary>) -> Option<String> {
    match summary {
        Some(ReasoningSummary::None) | None => None,
        Some(summary) => Some(summary.to_string()),
    }
}

fn personality_mode(personality: Option<Personality>) -> Option<String> {
    match personality {
        Some(Personality::None) | None => None,
        Some(personality) => Some(personality.to_string()),
    }
}

fn analytics_turn_status(status: codex_app_server_protocol::TurnStatus) -> Option<TurnStatus> {
    match status {
        codex_app_server_protocol::TurnStatus::Completed => Some(TurnStatus::Completed),
        codex_app_server_protocol::TurnStatus::Failed => Some(TurnStatus::Failed),
        codex_app_server_protocol::TurnStatus::Interrupted => Some(TurnStatus::Interrupted),
        codex_app_server_protocol::TurnStatus::InProgress => None,
    }
}

fn num_input_images(input: &[UserInput]) -> usize {
    input
        .iter()
        .filter(|item| matches!(item, UserInput::Image { .. } | UserInput::LocalImage { .. }))
        .count()
}

fn rejection_reason_from_error_type(
    error_type: Option<AnalyticsJsonRpcError>,
) -> Option<TurnSteerRejectionReason> {
    match error_type? {
        AnalyticsJsonRpcError::TurnSteer(error) => Some(error.into()),
        AnalyticsJsonRpcError::Input(error) => Some(error.into()),
    }
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
