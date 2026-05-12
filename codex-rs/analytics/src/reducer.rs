use crate::accepted_lines::AcceptedLineFingerprintEventInput;
use crate::accepted_lines::accepted_line_fingerprint_event_requests;
use crate::accepted_lines::accepted_line_fingerprints_from_unified_diff;
use crate::accepted_lines::accepted_line_repo_hash_for_cwd;
use crate::events::AppServerRpcTransport;
use crate::events::CodexAppMentionedEventRequest;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexAppUsedEventRequest;
use crate::events::CodexCollabAgentToolCallEventParams;
use crate::events::CodexCollabAgentToolCallEventRequest;
use crate::events::CodexCommandExecutionEventParams;
use crate::events::CodexCommandExecutionEventRequest;
use crate::events::CodexCompactionEventRequest;
use crate::events::CodexDynamicToolCallEventParams;
use crate::events::CodexDynamicToolCallEventRequest;
use crate::events::CodexFileChangeEventParams;
use crate::events::CodexFileChangeEventRequest;
use crate::events::CodexHookRunEventRequest;
use crate::events::CodexImageGenerationEventParams;
use crate::events::CodexImageGenerationEventRequest;
use crate::events::CodexMcpToolCallEventParams;
use crate::events::CodexMcpToolCallEventRequest;
use crate::events::CodexPluginEventRequest;
use crate::events::CodexPluginUsedEventRequest;
use crate::events::CodexReviewEventParams;
use crate::events::CodexReviewEventRequest;
use crate::events::CodexRuntimeMetadata;
use crate::events::CodexToolItemEventBase;
use crate::events::CodexTurnEventParams;
use crate::events::CodexTurnEventRequest;
use crate::events::CodexTurnSteerEventParams;
use crate::events::CodexTurnSteerEventRequest;
use crate::events::CodexWebSearchEventParams;
use crate::events::CodexWebSearchEventRequest;
use crate::events::FinalApprovalOutcome;
use crate::events::GuardianReviewEventParams;
use crate::events::GuardianReviewEventPayload;
use crate::events::GuardianReviewEventRequest;
use crate::events::ReviewResolution;
use crate::events::ReviewStatus;
use crate::events::ReviewSubjectKind;
use crate::events::ReviewTrigger;
use crate::events::Reviewer;
use crate::events::SkillInvocationEventParams;
use crate::events::SkillInvocationEventRequest;
use crate::events::ThreadInitializedEvent;
use crate::events::ThreadInitializedEventParams;
use crate::events::ToolItemFailureKind;
use crate::events::ToolItemTerminalStatus;
use crate::events::TrackEventRequest;
use crate::events::WebSearchActionKind;
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
use crate::option_i64_to_u64;
use crate::serialize_enum_as_string;
use crate::usize_to_u64;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::CollabAgentStatus;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::CommandAction;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::GuardianApprovalReviewAction;
use codex_app_server_protocol::GuardianApprovalReviewStatus;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::NetworkPolicyRuleAction;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::RequestPermissionProfile;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnSteerResponse;
use codex_app_server_protocol::UserInput;
use codex_app_server_protocol::WebSearchAction;
use codex_git_utils::collect_git_info;
use codex_git_utils::get_git_repo_root;
use codex_login::default_client::originator;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::request_permissions::PermissionGrantScope as CorePermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionsResponse as CoreRequestPermissionsResponse;
use sha1::Digest;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct AnalyticsReducer {
    requests: HashMap<(u64, RequestId), RequestState>,
    turns: HashMap<String, TurnState>,
    connections: HashMap<u64, ConnectionState>,
    threads: HashMap<String, ThreadAnalyticsState>,
    tool_items_started_at_ms: HashMap<ToolItemKey, u64>,
    pending_reviews: HashMap<RequestId, PendingReviewState>,
    item_review_summaries: HashMap<ToolItemKey, ItemReviewSummary>,
}

struct ConnectionState {
    app_server_client: CodexAppServerClientMetadata,
    runtime: CodexRuntimeMetadata,
}

#[derive(Default)]
struct ThreadAnalyticsState {
    connection_id: Option<u64>,
    metadata: Option<ThreadMetadataState>,
}

#[derive(Clone, Copy)]
struct AnalyticsDropSite<'a> {
    event_name: &'static str,
    thread_id: &'a str,
    turn_id: Option<&'a str>,
    review_id: Option<&'a str>,
    item_id: Option<&'a str>,
}

impl<'a> AnalyticsDropSite<'a> {
    fn guardian(input: &'a GuardianReviewEventParams) -> Self {
        Self {
            event_name: "guardian",
            thread_id: &input.thread_id,
            turn_id: Some(&input.turn_id),
            review_id: Some(&input.review_id),
            item_id: None,
        }
    }

    fn review(input: &'a PendingReviewState) -> Self {
        Self {
            event_name: "review",
            thread_id: &input.thread_id,
            turn_id: Some(&input.turn_id),
            review_id: Some(&input.review_id),
            item_id: input.item_id.as_deref(),
        }
    }

    fn compaction(input: &'a CodexCompactionEvent) -> Self {
        Self {
            event_name: "compaction",
            thread_id: &input.thread_id,
            turn_id: Some(&input.turn_id),
            review_id: None,
            item_id: None,
        }
    }

    fn tool_item(
        notification: &'a codex_app_server_protocol::ItemCompletedNotification,
        item_id: &'a str,
    ) -> Self {
        Self {
            event_name: "tool item",
            thread_id: &notification.thread_id,
            turn_id: Some(&notification.turn_id),
            review_id: None,
            item_id: Some(item_id),
        }
    }

    fn turn_steer(thread_id: &'a str) -> Self {
        Self {
            event_name: "turn steer",
            thread_id,
            turn_id: None,
            review_id: None,
            item_id: None,
        }
    }

    fn turn(thread_id: &'a str, turn_id: &'a str) -> Self {
        Self {
            event_name: "turn",
            thread_id,
            turn_id: Some(turn_id),
            review_id: None,
            item_id: None,
        }
    }
}

enum MissingAnalyticsContext {
    ThreadConnection,
    Connection { connection_id: u64 },
    ThreadMetadata,
}

#[derive(Clone)]
struct PendingReviewState {
    thread_id: String,
    turn_id: String,
    item_id: Option<String>,
    review_id: String,
    subject_kind: ReviewSubjectKind,
    subject_name: String,
    trigger: ReviewTrigger,
    started_at_ms: u64,
    requested_additional_permissions: bool,
    requested_network_access: bool,
}

#[derive(Clone, Default)]
struct ItemReviewSummary {
    review_count: u64,
    guardian_review_count: u64,
    user_review_count: u64,
    final_approval_outcome: Option<FinalApprovalOutcome>,
    requested_additional_permissions: bool,
    requested_network_access: bool,
}

#[derive(Clone)]
struct ThreadMetadataState {
    thread_source: Option<ThreadSource>,
    initialization_mode: ThreadInitializationMode,
    subagent_source: Option<String>,
    parent_thread_id: Option<String>,
}

impl ThreadMetadataState {
    fn from_thread_metadata(
        session_source: &SessionSource,
        thread_source: Option<ThreadSource>,
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
            | SessionSource::Internal(_)
            | SessionSource::Unknown => (None, None),
        };
        Self {
            thread_source,
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
    latest_diff: Option<String>,
    steer_count: usize,
    tool_counts: TurnToolCounts,
}

#[derive(Hash, Eq, PartialEq)]
struct ToolItemKey {
    thread_id: String,
    turn_id: String,
    item_id: String,
}

#[derive(Default)]
struct TurnToolCounts {
    total: usize,
    shell_command: usize,
    file_change: usize,
    mcp_tool_call: usize,
    dynamic_tool_call: usize,
    subagent_tool_call: usize,
    web_search: usize,
    image_generation: usize,
}

impl TurnToolCounts {
    fn record(&mut self, item: &ThreadItem) {
        match item {
            ThreadItem::CommandExecution { .. } => self.shell_command += 1,
            ThreadItem::FileChange { .. } => self.file_change += 1,
            ThreadItem::McpToolCall { .. } => self.mcp_tool_call += 1,
            ThreadItem::DynamicToolCall { .. } => self.dynamic_tool_call += 1,
            ThreadItem::CollabAgentToolCall { .. } => self.subagent_tool_call += 1,
            ThreadItem::WebSearch { .. } => self.web_search += 1,
            ThreadItem::ImageGeneration { .. } => self.image_generation += 1,
            ThreadItem::UserMessage { .. }
            | ThreadItem::HookPrompt { .. }
            | ThreadItem::AgentMessage { .. }
            | ThreadItem::Plan { .. }
            | ThreadItem::Reasoning { .. }
            | ThreadItem::ImageView { .. }
            | ThreadItem::EnteredReviewMode { .. }
            | ThreadItem::ExitedReviewMode { .. }
            | ThreadItem::ContextCompaction { .. } => return,
        }
        self.total += 1;
    }
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
                connection_id,
                request_id,
                request,
            } => {
                self.ingest_request(connection_id, request_id, *request);
            }
            AnalyticsFact::ClientResponse {
                connection_id,
                request_id,
                response,
            } => {
                if let Some(response) = response.into_client_response(request_id) {
                    self.ingest_response(connection_id, response, out).await;
                }
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
                self.ingest_notification(*notification, out).await;
            }
            AnalyticsFact::ServerRequest {
                connection_id,
                request,
            } => {
                self.ingest_server_request(connection_id, *request);
            }
            AnalyticsFact::ServerResponse {
                completed_at_ms,
                response,
            } => {
                self.ingest_server_response(completed_at_ms, *response, out);
            }
            AnalyticsFact::EffectivePermissionsApprovalResponse {
                completed_at_ms,
                request_id,
                response,
            } => {
                self.ingest_effective_permissions_approval_response(
                    completed_at_ms,
                    request_id,
                    *response,
                    out,
                );
            }
            AnalyticsFact::ServerRequestAborted {
                completed_at_ms,
                request_id,
            } => {
                self.ingest_server_request_aborted(completed_at_ms, request_id, out);
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
                    self.ingest_turn_resolved_config(*input, out).await;
                }
                CustomAnalyticsFact::TurnTokenUsage(input) => {
                    self.ingest_turn_token_usage(*input, out).await;
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
        let parent_thread_id = input
            .parent_thread_id
            .clone()
            .or_else(|| subagent_parent_thread_id(&input.subagent_source));
        let parent_connection_id = parent_thread_id
            .as_ref()
            .and_then(|parent_thread_id| self.threads.get(parent_thread_id))
            .and_then(|thread| thread.connection_id);
        let thread_state = self.threads.entry(input.thread_id.clone()).or_default();
        thread_state
            .metadata
            .get_or_insert_with(|| ThreadMetadataState {
                thread_source: Some(ThreadSource::Subagent),
                initialization_mode: ThreadInitializationMode::New,
                subagent_source: Some(subagent_source_name(&input.subagent_source)),
                parent_thread_id,
            });
        if thread_state.connection_id.is_none() {
            thread_state.connection_id = parent_connection_id;
        }
        out.push(TrackEventRequest::ThreadInitialized(
            subagent_thread_started_event_request(input),
        ));
    }

    fn ingest_guardian_review(
        &mut self,
        input: GuardianReviewEventParams,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let Some(connection_state) =
            self.thread_connection_or_warn(AnalyticsDropSite::guardian(&input))
        else {
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

    async fn ingest_turn_resolved_config(
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
            latest_diff: None,
            steer_count: 0,
            tool_counts: TurnToolCounts::default(),
        });
        turn_state.thread_id = Some(thread_id);
        turn_state.num_input_images = Some(num_input_images);
        turn_state.resolved_config = Some(input);
        self.maybe_emit_turn_event(&turn_id, out).await;
    }

    async fn ingest_turn_token_usage(
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
            latest_diff: None,
            steer_count: 0,
            tool_counts: TurnToolCounts::default(),
        });
        turn_state.thread_id = Some(input.thread_id);
        turn_state.token_usage = Some(input.token_usage);
        self.maybe_emit_turn_event(&turn_id, out).await;
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
                        turn_id: Some(tracking.turn_id.clone()),
                        invoke_type: Some(invocation.invocation_type),
                        model_slug: Some(tracking.model_slug.clone()),
                        product_client_id: Some(originator().value),
                        repo_url,
                        skill_scope: Some(skill_scope.to_string()),
                        plugin_id: invocation.plugin_id,
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

    async fn ingest_response(
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
                    latest_diff: None,
                    steer_count: 0,
                    tool_counts: TurnToolCounts::default(),
                });
                turn_state.connection_id = Some(connection_id);
                turn_state.thread_id = Some(pending_request.thread_id);
                turn_state.num_input_images = Some(pending_request.num_input_images);
                self.maybe_emit_turn_event(&turn_id, out).await;
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

    fn ingest_server_request(&mut self, _connection_id: u64, request: ServerRequest) {
        match request {
            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                let is_network_access_review = params.network_approval_context.is_some();
                let requested_network_access = is_network_access_review
                    || params
                        .proposed_network_policy_amendments
                        .as_ref()
                        .is_some_and(|amendments| !amendments.is_empty())
                    || params
                        .additional_permissions
                        .as_ref()
                        .and_then(|permissions| permissions.network.as_ref())
                        .and_then(|network| network.enabled)
                        .unwrap_or(false);
                let requested_additional_permissions = params.additional_permissions.is_some();
                let trigger = if params.approval_id.is_some() {
                    ReviewTrigger::ExecveIntercept
                } else if requested_network_access {
                    ReviewTrigger::NetworkPolicyDenial
                } else if requested_additional_permissions {
                    ReviewTrigger::SandboxDenial
                } else {
                    ReviewTrigger::Initial
                };
                let Some(started_at_ms) = option_i64_to_u64(Some(params.started_at_ms)) else {
                    return;
                };
                self.pending_reviews.insert(
                    request_id.clone(),
                    PendingReviewState {
                        thread_id: params.thread_id,
                        turn_id: params.turn_id,
                        item_id: Some(params.item_id),
                        review_id: user_review_id(&request_id),
                        subject_kind: if is_network_access_review {
                            ReviewSubjectKind::NetworkAccess
                        } else {
                            ReviewSubjectKind::CommandExecution
                        },
                        subject_name: if is_network_access_review {
                            "network_access".to_string()
                        } else {
                            "command_execution".to_string()
                        },
                        trigger,
                        started_at_ms,
                        requested_additional_permissions,
                        requested_network_access,
                    },
                );
            }
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                let requested_additional_permissions = params.grant_root.is_some();
                let Some(started_at_ms) = option_i64_to_u64(Some(params.started_at_ms)) else {
                    return;
                };
                self.pending_reviews.insert(
                    request_id.clone(),
                    PendingReviewState {
                        thread_id: params.thread_id,
                        turn_id: params.turn_id,
                        item_id: Some(params.item_id),
                        review_id: user_review_id(&request_id),
                        subject_kind: ReviewSubjectKind::FileChange,
                        subject_name: "apply_patch".to_string(),
                        trigger: if requested_additional_permissions {
                            ReviewTrigger::SandboxDenial
                        } else {
                            ReviewTrigger::Initial
                        },
                        started_at_ms,
                        requested_additional_permissions,
                        requested_network_access: false,
                    },
                );
            }
            ServerRequest::PermissionsRequestApproval { request_id, params } => {
                let requested_network_access = params
                    .permissions
                    .network
                    .as_ref()
                    .and_then(|network| network.enabled)
                    .unwrap_or(false);
                let requested_additional_permissions =
                    requested_network_access || params.permissions.file_system.is_some();
                let trigger = if requested_network_access {
                    ReviewTrigger::NetworkPolicyDenial
                } else if requested_additional_permissions {
                    ReviewTrigger::SandboxDenial
                } else {
                    ReviewTrigger::Initial
                };
                let Some(started_at_ms) = option_i64_to_u64(Some(params.started_at_ms)) else {
                    return;
                };
                self.pending_reviews.insert(
                    request_id.clone(),
                    PendingReviewState {
                        thread_id: params.thread_id,
                        turn_id: params.turn_id,
                        item_id: Some(params.item_id),
                        review_id: user_review_id(&request_id),
                        subject_kind: ReviewSubjectKind::Permissions,
                        subject_name: "permissions".to_string(),
                        trigger,
                        started_at_ms,
                        requested_additional_permissions,
                        requested_network_access,
                    },
                );
            }
            _ => {}
        }
    }

    fn ingest_server_response(
        &mut self,
        completed_at_ms: u64,
        response: ServerResponse,
        out: &mut Vec<TrackEventRequest>,
    ) {
        match response {
            ServerResponse::CommandExecutionRequestApproval {
                request_id,
                response,
            } => {
                let Some(pending_review) = self.pending_reviews.remove(&request_id) else {
                    return;
                };
                let (status, resolution) = command_execution_review_result(response.decision);
                self.emit_review_event(
                    pending_review,
                    Reviewer::User,
                    status,
                    resolution,
                    completed_at_ms,
                    out,
                );
            }
            ServerResponse::FileChangeRequestApproval {
                request_id,
                response,
            } => {
                let Some(pending_review) = self.pending_reviews.remove(&request_id) else {
                    return;
                };
                let (status, resolution) = file_change_review_result(response.decision);
                self.emit_review_event(
                    pending_review,
                    Reviewer::User,
                    status,
                    resolution,
                    completed_at_ms,
                    out,
                );
            }
            _ => {}
        }
    }

    fn ingest_effective_permissions_approval_response(
        &mut self,
        completed_at_ms: u64,
        request_id: RequestId,
        response: CoreRequestPermissionsResponse,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let Some(pending_review) = self.pending_reviews.remove(&request_id) else {
            return;
        };
        let (status, resolution) = effective_permissions_review_result(&response);
        self.emit_review_event(
            pending_review,
            Reviewer::User,
            status,
            resolution,
            completed_at_ms,
            out,
        );
    }

    fn ingest_server_request_aborted(
        &mut self,
        completed_at_ms: u64,
        request_id: RequestId,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let Some(pending_review) = self.pending_reviews.remove(&request_id) else {
            return;
        };
        self.emit_review_event(
            pending_review,
            Reviewer::User,
            ReviewStatus::Aborted,
            ReviewResolution::None,
            completed_at_ms,
            out,
        );
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

    async fn ingest_notification(
        &mut self,
        notification: ServerNotification,
        out: &mut Vec<TrackEventRequest>,
    ) {
        match notification {
            ServerNotification::ItemStarted(notification) => {
                let Some(item_id) = tracked_tool_item_id(&notification.item) else {
                    return;
                };
                let Some(started_at_ms) = option_i64_to_u64(Some(notification.started_at_ms))
                else {
                    return;
                };
                self.tool_items_started_at_ms.insert(
                    ToolItemKey {
                        thread_id: notification.thread_id,
                        turn_id: notification.turn_id,
                        item_id: item_id.to_string(),
                    },
                    started_at_ms,
                );
            }
            ServerNotification::ItemCompleted(notification) => {
                let Some(item_id) = tracked_tool_item_id(&notification.item) else {
                    return;
                };
                let Some(turn_state) = self.turns.get_mut(&notification.turn_id) else {
                    tracing::warn!(
                        thread_id = %notification.thread_id,
                        turn_id = %notification.turn_id,
                        item_id,
                        "dropping turn tool count update: missing turn state"
                    );
                    return;
                };
                turn_state.tool_counts.record(&notification.item);
                let key = ToolItemKey {
                    thread_id: notification.thread_id.clone(),
                    turn_id: notification.turn_id.clone(),
                    item_id: item_id.to_string(),
                };
                let Some(started_at_ms) = self.tool_items_started_at_ms.remove(&key) else {
                    tracing::warn!(
                        thread_id = %notification.thread_id,
                        turn_id = %notification.turn_id,
                        item_id,
                        "dropping tool item analytics event: missing item started notification"
                    );
                    return;
                };
                let Some(completed_at_ms) = option_i64_to_u64(Some(notification.completed_at_ms))
                else {
                    return;
                };
                let Some((connection_state, thread_metadata)) = self
                    .thread_context_or_warn(AnalyticsDropSite::tool_item(&notification, item_id))
                else {
                    return;
                };
                if let Some(event) = tool_item_event(ToolItemEventInput {
                    thread_id: &notification.thread_id,
                    turn_id: &notification.turn_id,
                    item: &notification.item,
                    started_at_ms,
                    completed_at_ms,
                    connection_state,
                    thread_metadata,
                    review_summary: self.item_review_summaries.get(&key),
                }) {
                    out.push(event);
                }
                self.item_review_summaries.remove(&key);
            }
            ServerNotification::ItemGuardianApprovalReviewStarted(notification) => {
                let _ = notification;
            }
            ServerNotification::ItemGuardianApprovalReviewCompleted(notification) => {
                self.ingest_guardian_review_completed(notification, out);
            }
            ServerNotification::TurnStarted(notification) => {
                let turn_state = self.turns.entry(notification.turn.id).or_insert(TurnState {
                    connection_id: None,
                    thread_id: None,
                    num_input_images: None,
                    resolved_config: None,
                    started_at: None,
                    token_usage: None,
                    completed: None,
                    latest_diff: None,
                    steer_count: 0,
                    tool_counts: TurnToolCounts::default(),
                });
                turn_state.started_at = notification
                    .turn
                    .started_at
                    .and_then(|started_at| u64::try_from(started_at).ok());
            }
            ServerNotification::TurnDiffUpdated(notification) => {
                let turn_state =
                    self.turns
                        .entry(notification.turn_id.clone())
                        .or_insert(TurnState {
                            connection_id: None,
                            thread_id: None,
                            num_input_images: None,
                            resolved_config: None,
                            started_at: None,
                            token_usage: None,
                            completed: None,
                            latest_diff: None,
                            steer_count: 0,
                            tool_counts: TurnToolCounts::default(),
                        });
                turn_state.thread_id = Some(notification.thread_id);
                turn_state.latest_diff = Some(notification.diff);
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
                            latest_diff: None,
                            steer_count: 0,
                            tool_counts: TurnToolCounts::default(),
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
                self.maybe_emit_turn_event(&turn_id, out).await;
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
        let session_source: SessionSource = thread.source.into();
        let thread_id = thread.id;
        let Some(connection_state) = self.connections.get(&connection_id) else {
            return;
        };
        let thread_metadata = ThreadMetadataState::from_thread_metadata(
            &session_source,
            thread.thread_source.map(Into::into),
            initialization_mode,
        );
        self.threads.insert(
            thread_id.clone(),
            ThreadAnalyticsState {
                connection_id: Some(connection_id),
                metadata: Some(thread_metadata.clone()),
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
                    thread_source: thread_metadata.thread_source,
                    initialization_mode,
                    subagent_source: thread_metadata.subagent_source.clone(),
                    parent_thread_id: thread_metadata.parent_thread_id,
                    created_at: u64::try_from(thread.created_at).unwrap_or_default(),
                },
            },
        ));
    }

    fn ingest_compaction(&mut self, input: CodexCompactionEvent, out: &mut Vec<TrackEventRequest>) {
        let Some((connection_state, thread_metadata)) =
            self.thread_context_or_warn(AnalyticsDropSite::compaction(&input))
        else {
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

    fn ingest_guardian_review_completed(
        &mut self,
        notification: codex_app_server_protocol::ItemGuardianApprovalReviewCompletedNotification,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let Some((status, resolution)) = guardian_review_result(notification.review.status) else {
            return;
        };
        let (subject_kind, subject_name, trigger) =
            guardian_review_subject_metadata(&notification.action);
        let Some(started_at_ms) = option_i64_to_u64(Some(notification.started_at_ms)) else {
            return;
        };
        let pending_review = PendingReviewState {
            thread_id: notification.thread_id,
            turn_id: notification.turn_id,
            item_id: notification.target_item_id,
            review_id: notification.review_id,
            subject_kind,
            subject_name,
            trigger,
            started_at_ms,
            requested_additional_permissions: guardian_review_requested_additional_permissions(
                &notification.action,
            ),
            requested_network_access: guardian_review_requested_network_access(
                &notification.action,
            ),
        };
        let Some(completed_at_ms) = option_i64_to_u64(Some(notification.completed_at_ms)) else {
            return;
        };
        self.emit_review_event(
            pending_review,
            Reviewer::Guardian,
            status,
            resolution,
            completed_at_ms,
            out,
        );
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
        let drop_site = AnalyticsDropSite::turn_steer(&pending_request.thread_id);
        let Some(thread_metadata) = self
            .threads
            .get(drop_site.thread_id)
            .and_then(|thread| thread.metadata.as_ref())
        else {
            warn_missing_analytics_context(&drop_site, MissingAnalyticsContext::ThreadMetadata);
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
                thread_source: thread_metadata.thread_source,
                subagent_source: thread_metadata.subagent_source.clone(),
                parent_thread_id: thread_metadata.parent_thread_id.clone(),
                num_input_images: pending_request.num_input_images,
                result,
                rejection_reason,
                created_at: pending_request.created_at,
            },
        }));
    }

    fn emit_review_event(
        &mut self,
        pending_review: PendingReviewState,
        reviewer: Reviewer,
        status: ReviewStatus,
        resolution: ReviewResolution,
        completed_at_ms: u64,
        out: &mut Vec<TrackEventRequest>,
    ) {
        if let Some(item_key) = item_review_summary_key(&pending_review) {
            self.record_item_review_summary(
                item_key,
                reviewer,
                status,
                resolution,
                &pending_review,
            );
        }
        let Some((connection_state, thread_metadata)) =
            self.thread_context_or_warn(AnalyticsDropSite::review(&pending_review))
        else {
            return;
        };
        out.push(TrackEventRequest::ReviewEvent(CodexReviewEventRequest {
            event_type: "codex_review_event",
            event_params: CodexReviewEventParams {
                thread_id: pending_review.thread_id,
                turn_id: pending_review.turn_id,
                item_id: pending_review.item_id,
                review_id: pending_review.review_id,
                app_server_client: connection_state.app_server_client.clone(),
                runtime: connection_state.runtime.clone(),
                thread_source: thread_metadata.thread_source,
                subagent_source: thread_metadata.subagent_source.clone(),
                parent_thread_id: thread_metadata.parent_thread_id.clone(),
                subject_kind: pending_review.subject_kind,
                subject_name: pending_review.subject_name,
                reviewer,
                trigger: pending_review.trigger,
                status,
                resolution,
                started_at_ms: pending_review.started_at_ms,
                completed_at_ms,
                duration_ms: observed_duration_ms(pending_review.started_at_ms, completed_at_ms),
            },
        }));
    }

    fn record_item_review_summary(
        &mut self,
        item_key: ToolItemKey,
        reviewer: Reviewer,
        status: ReviewStatus,
        resolution: ReviewResolution,
        pending_review: &PendingReviewState,
    ) {
        let summary = self.item_review_summaries.entry(item_key).or_default();
        summary.review_count += 1;
        match reviewer {
            Reviewer::Guardian => summary.guardian_review_count += 1,
            Reviewer::User => summary.user_review_count += 1,
        }
        summary.final_approval_outcome = Some(final_approval_outcome(reviewer, status, resolution));
        summary.requested_additional_permissions |= pending_review.requested_additional_permissions;
        summary.requested_network_access |= pending_review.requested_network_access;
    }

    async fn maybe_emit_turn_event(&mut self, turn_id: &str, out: &mut Vec<TrackEventRequest>) {
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
        let Some(thread_id) = turn_state.thread_id.as_ref() else {
            return;
        };
        let Some(connection_id) = turn_state.connection_id else {
            return;
        };
        let Some(connection_state) = self.connections.get(&connection_id) else {
            warn_missing_analytics_context(
                &AnalyticsDropSite::turn(thread_id, turn_id),
                MissingAnalyticsContext::Connection { connection_id },
            );
            return;
        };
        let drop_site = AnalyticsDropSite::turn(thread_id, turn_id);
        let Some(thread_metadata) = self
            .threads
            .get(drop_site.thread_id)
            .and_then(|thread| thread.metadata.as_ref())
        else {
            warn_missing_analytics_context(&drop_site, MissingAnalyticsContext::ThreadMetadata);
            return;
        };
        let turn_event = TrackEventRequest::TurnEvent(Box::new(CodexTurnEventRequest {
            event_type: "codex_turn_event",
            event_params: codex_turn_event_params(
                connection_state.app_server_client.clone(),
                connection_state.runtime.clone(),
                turn_id.to_string(),
                turn_state,
                thread_metadata,
            ),
        }));
        let accepted_line_event = accepted_line_event_input(turn_id, turn_state);

        out.push(turn_event);
        if let Some((mut input, cwd)) = accepted_line_event {
            input.repo_hash = accepted_line_repo_hash_for_cwd(cwd.as_path()).await;
            out.extend(accepted_line_fingerprint_event_requests(input));
        }
        self.turns.remove(turn_id);
    }

    fn thread_connection_or_warn(
        &self,
        drop_site: AnalyticsDropSite<'_>,
    ) -> Option<&ConnectionState> {
        let Some(thread_state) = self.threads.get(drop_site.thread_id) else {
            warn_missing_analytics_context(&drop_site, MissingAnalyticsContext::ThreadConnection);
            return None;
        };
        let Some(connection_id) = thread_state.connection_id else {
            warn_missing_analytics_context(&drop_site, MissingAnalyticsContext::ThreadConnection);
            return None;
        };
        let Some(connection_state) = self.connections.get(&connection_id) else {
            warn_missing_analytics_context(
                &drop_site,
                MissingAnalyticsContext::Connection { connection_id },
            );
            return None;
        };
        Some(connection_state)
    }

    fn thread_context_or_warn(
        &self,
        drop_site: AnalyticsDropSite<'_>,
    ) -> Option<(&ConnectionState, &ThreadMetadataState)> {
        let connection_state = self.thread_connection_or_warn(drop_site)?;
        let Some(thread_metadata) = self
            .threads
            .get(drop_site.thread_id)
            .and_then(|thread| thread.metadata.as_ref())
        else {
            warn_missing_analytics_context(&drop_site, MissingAnalyticsContext::ThreadMetadata);
            return None;
        };
        Some((connection_state, thread_metadata))
    }
}

fn warn_missing_analytics_context(
    drop_site: &AnalyticsDropSite<'_>,
    missing: MissingAnalyticsContext,
) {
    let (missing_context, connection_id) = match missing {
        MissingAnalyticsContext::ThreadConnection => ("thread_connection", None),
        MissingAnalyticsContext::Connection { connection_id } => {
            ("connection", Some(connection_id))
        }
        MissingAnalyticsContext::ThreadMetadata => ("thread_metadata", None),
    };
    tracing::warn!(
        thread_id = %drop_site.thread_id,
        turn_id = ?drop_site.turn_id,
        review_id = ?drop_site.review_id,
        item_id = ?drop_site.item_id,
        missing_context,
        connection_id,
        "dropping {} analytics event: missing analytics context",
        drop_site.event_name
    );
}

fn tracked_tool_item_id(item: &ThreadItem) -> Option<&str> {
    match item {
        ThreadItem::CommandExecution { id, .. }
        | ThreadItem::FileChange { id, .. }
        | ThreadItem::McpToolCall { id, .. }
        | ThreadItem::DynamicToolCall { id, .. }
        | ThreadItem::CollabAgentToolCall { id, .. }
        | ThreadItem::WebSearch { id, .. }
        | ThreadItem::ImageGeneration { id, .. } => Some(id),
        ThreadItem::UserMessage { .. }
        | ThreadItem::HookPrompt { .. }
        | ThreadItem::AgentMessage { .. }
        | ThreadItem::Plan { .. }
        | ThreadItem::Reasoning { .. }
        | ThreadItem::ImageView { .. }
        | ThreadItem::EnteredReviewMode { .. }
        | ThreadItem::ExitedReviewMode { .. }
        | ThreadItem::ContextCompaction { .. } => None,
    }
}

fn item_review_summary_key(pending_review: &PendingReviewState) -> Option<ToolItemKey> {
    match pending_review.subject_kind {
        ReviewSubjectKind::CommandExecution
        | ReviewSubjectKind::FileChange
        | ReviewSubjectKind::McpToolCall => Some(ToolItemKey {
            thread_id: pending_review.thread_id.clone(),
            turn_id: pending_review.turn_id.clone(),
            item_id: pending_review.item_id.clone()?,
        }),
        ReviewSubjectKind::Permissions | ReviewSubjectKind::NetworkAccess => None,
    }
}

struct ToolItemEventInput<'a> {
    thread_id: &'a str,
    turn_id: &'a str,
    item: &'a ThreadItem,
    started_at_ms: u64,
    completed_at_ms: u64,
    connection_state: &'a ConnectionState,
    thread_metadata: &'a ThreadMetadataState,
    review_summary: Option<&'a ItemReviewSummary>,
}

fn tool_item_event(input: ToolItemEventInput<'_>) -> Option<TrackEventRequest> {
    let ToolItemEventInput {
        thread_id,
        turn_id,
        item,
        started_at_ms,
        completed_at_ms,
        connection_state,
        thread_metadata,
        review_summary,
    } = input;
    match item {
        ThreadItem::CommandExecution {
            id,
            source,
            status,
            command_actions,
            exit_code,
            duration_ms,
            ..
        } => {
            let (terminal_status, failure_kind) = command_execution_outcome(status)?;
            let action_counts = command_action_counts(command_actions);
            let base = tool_item_base(
                thread_id,
                turn_id,
                id.clone(),
                command_execution_tool_name(*source).to_string(),
                ToolItemOutcome {
                    terminal_status,
                    failure_kind,
                    execution_duration_ms: option_i64_to_u64(*duration_ms),
                },
                ToolItemContext {
                    started_at_ms,
                    completed_at_ms,
                    connection_state,
                    thread_metadata,
                    review_summary,
                },
            );
            Some(TrackEventRequest::CommandExecution(
                CodexCommandExecutionEventRequest {
                    event_type: "codex_command_execution_event",
                    event_params: CodexCommandExecutionEventParams {
                        base,
                        command_execution_source: *source,
                        exit_code: *exit_code,
                        command_total_action_count: action_counts.total,
                        command_read_action_count: action_counts.read,
                        command_list_files_action_count: action_counts.list_files,
                        command_search_action_count: action_counts.search,
                        command_unknown_action_count: action_counts.unknown,
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
                    execution_duration_ms: None,
                },
                ToolItemContext {
                    started_at_ms,
                    completed_at_ms,
                    connection_state,
                    thread_metadata,
                    review_summary,
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
                    execution_duration_ms: option_i64_to_u64(*duration_ms),
                },
                ToolItemContext {
                    started_at_ms,
                    completed_at_ms,
                    connection_state,
                    thread_metadata,
                    review_summary,
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
                    execution_duration_ms: option_i64_to_u64(*duration_ms),
                },
                ToolItemContext {
                    started_at_ms,
                    completed_at_ms,
                    connection_state,
                    thread_metadata,
                    review_summary,
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
                    execution_duration_ms: None,
                },
                ToolItemContext {
                    started_at_ms,
                    completed_at_ms,
                    connection_state,
                    thread_metadata,
                    review_summary,
                },
            );
            Some(TrackEventRequest::CollabAgentToolCall(
                CodexCollabAgentToolCallEventRequest {
                    event_type: "codex_collab_agent_tool_call_event",
                    event_params: CodexCollabAgentToolCallEventParams {
                        base,
                        sender_thread_id: sender_thread_id.clone(),
                        receiver_thread_count: usize_to_u64(receiver_thread_ids.len()),
                        receiver_thread_ids: Some(receiver_thread_ids.clone()),
                        requested_model: model.clone(),
                        requested_reasoning_effort: reasoning_effort
                            .as_ref()
                            .and_then(serialize_enum_as_string),
                        agent_state_count: Some(usize_to_u64(agents_states.len())),
                        completed_agent_count: Some(usize_to_u64(
                            agents_states
                                .values()
                                .filter(|state| state.status == CollabAgentStatus::Completed)
                                .count(),
                        )),
                        failed_agent_count: Some(usize_to_u64(
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
                        )),
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
                    execution_duration_ms: None,
                },
                ToolItemContext {
                    started_at_ms,
                    completed_at_ms,
                    connection_state,
                    thread_metadata,
                    review_summary,
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
                    execution_duration_ms: None,
                },
                ToolItemContext {
                    started_at_ms,
                    completed_at_ms,
                    connection_state,
                    thread_metadata,
                    review_summary,
                },
            );
            Some(TrackEventRequest::ImageGeneration(
                CodexImageGenerationEventRequest {
                    event_type: "codex_image_generation_event",
                    event_params: CodexImageGenerationEventParams {
                        base,
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
    execution_duration_ms: Option<u64>,
}

#[derive(Default)]
struct CommandActionCounts {
    total: u64,
    read: u64,
    list_files: u64,
    search: u64,
    unknown: u64,
}

fn command_action_counts(command_actions: &[CommandAction]) -> CommandActionCounts {
    let mut counts = CommandActionCounts {
        total: usize_to_u64(command_actions.len()),
        ..Default::default()
    };
    for action in command_actions {
        match action {
            CommandAction::Read { .. } => counts.read += 1,
            CommandAction::ListFiles { .. } => counts.list_files += 1,
            CommandAction::Search { .. } => counts.search += 1,
            CommandAction::Unknown { .. } => counts.unknown += 1,
        }
    }
    counts
}

#[derive(Clone, Copy)]
struct ToolItemContext<'a> {
    started_at_ms: u64,
    completed_at_ms: u64,
    connection_state: &'a ConnectionState,
    thread_metadata: &'a ThreadMetadataState,
    review_summary: Option<&'a ItemReviewSummary>,
}

fn tool_item_base(
    thread_id: &str,
    turn_id: &str,
    item_id: String,
    tool_name: String,
    outcome: ToolItemOutcome,
    context: ToolItemContext<'_>,
) -> CodexToolItemEventBase {
    let thread_metadata = context.thread_metadata;
    let review_summary = context.review_summary.cloned().unwrap_or_default();
    CodexToolItemEventBase {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        item_id,
        app_server_client: context.connection_state.app_server_client.clone(),
        runtime: context.connection_state.runtime.clone(),
        thread_source: thread_metadata.thread_source,
        subagent_source: thread_metadata.subagent_source.clone(),
        parent_thread_id: thread_metadata.parent_thread_id.clone(),
        tool_name,
        started_at_ms: context.started_at_ms,
        completed_at_ms: context.completed_at_ms,
        // duration_ms reflects item lifecycle observed by app-server. For web
        // search and image generation in particular, that can be narrower than
        // full upstream execution time.
        duration_ms: observed_duration_ms(context.started_at_ms, context.completed_at_ms),
        execution_duration_ms: outcome.execution_duration_ms,
        review_count: review_summary.review_count,
        guardian_review_count: review_summary.guardian_review_count,
        user_review_count: review_summary.user_review_count,
        final_approval_outcome: review_summary
            .final_approval_outcome
            .unwrap_or(FinalApprovalOutcome::Unknown),
        terminal_status: outcome.terminal_status,
        failure_kind: outcome.failure_kind,
        requested_additional_permissions: review_summary.requested_additional_permissions,
        requested_network_access: review_summary.requested_network_access,
    }
}

fn observed_duration_ms(started_at_ms: u64, completed_at_ms: u64) -> Option<u64> {
    completed_at_ms.checked_sub(started_at_ms)
}

fn user_review_id(request_id: &RequestId) -> String {
    format!("user:{request_id}")
}

fn command_execution_review_result(
    decision: CommandExecutionApprovalDecision,
) -> (ReviewStatus, ReviewResolution) {
    match decision {
        CommandExecutionApprovalDecision::Accept => {
            (ReviewStatus::Approved, ReviewResolution::None)
        }
        CommandExecutionApprovalDecision::AcceptForSession => {
            (ReviewStatus::Approved, ReviewResolution::SessionApproval)
        }
        CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment { .. } => (
            ReviewStatus::Approved,
            ReviewResolution::ExecPolicyAmendment,
        ),
        CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
            network_policy_amendment,
        } => match network_policy_amendment.action {
            NetworkPolicyRuleAction::Allow => (
                ReviewStatus::Approved,
                ReviewResolution::NetworkPolicyAmendment,
            ),
            NetworkPolicyRuleAction::Deny => (
                ReviewStatus::Denied,
                ReviewResolution::NetworkPolicyAmendment,
            ),
        },
        CommandExecutionApprovalDecision::Decline => (ReviewStatus::Denied, ReviewResolution::None),
        CommandExecutionApprovalDecision::Cancel => (ReviewStatus::Aborted, ReviewResolution::None),
    }
}

fn file_change_review_result(
    decision: FileChangeApprovalDecision,
) -> (ReviewStatus, ReviewResolution) {
    match decision {
        FileChangeApprovalDecision::Accept => (ReviewStatus::Approved, ReviewResolution::None),
        FileChangeApprovalDecision::AcceptForSession => {
            (ReviewStatus::Approved, ReviewResolution::SessionApproval)
        }
        FileChangeApprovalDecision::Decline => (ReviewStatus::Denied, ReviewResolution::None),
        FileChangeApprovalDecision::Cancel => (ReviewStatus::Aborted, ReviewResolution::None),
    }
}

fn effective_permissions_review_result(
    response: &CoreRequestPermissionsResponse,
) -> (ReviewStatus, ReviewResolution) {
    if response.permissions.is_empty() {
        return (ReviewStatus::Denied, ReviewResolution::None);
    }

    match response.scope {
        CorePermissionGrantScope::Turn => (ReviewStatus::Approved, ReviewResolution::None),
        CorePermissionGrantScope::Session => {
            (ReviewStatus::Approved, ReviewResolution::SessionApproval)
        }
    }
}

fn guardian_review_result(
    status: GuardianApprovalReviewStatus,
) -> Option<(ReviewStatus, ReviewResolution)> {
    match status {
        GuardianApprovalReviewStatus::InProgress => None,
        GuardianApprovalReviewStatus::Approved => {
            Some((ReviewStatus::Approved, ReviewResolution::None))
        }
        GuardianApprovalReviewStatus::Denied => {
            Some((ReviewStatus::Denied, ReviewResolution::None))
        }
        GuardianApprovalReviewStatus::TimedOut => {
            Some((ReviewStatus::TimedOut, ReviewResolution::None))
        }
        GuardianApprovalReviewStatus::Aborted => {
            Some((ReviewStatus::Aborted, ReviewResolution::None))
        }
    }
}

fn guardian_review_subject_metadata(
    action: &GuardianApprovalReviewAction,
) -> (ReviewSubjectKind, String, ReviewTrigger) {
    match action {
        GuardianApprovalReviewAction::Command { .. } => (
            ReviewSubjectKind::CommandExecution,
            "command_execution".to_string(),
            ReviewTrigger::Initial,
        ),
        GuardianApprovalReviewAction::Execve { .. } => (
            ReviewSubjectKind::CommandExecution,
            "command_execution".to_string(),
            ReviewTrigger::ExecveIntercept,
        ),
        GuardianApprovalReviewAction::ApplyPatch { .. } => (
            ReviewSubjectKind::FileChange,
            "apply_patch".to_string(),
            ReviewTrigger::SandboxDenial,
        ),
        GuardianApprovalReviewAction::NetworkAccess { .. } => (
            ReviewSubjectKind::NetworkAccess,
            "network_access".to_string(),
            ReviewTrigger::NetworkPolicyDenial,
        ),
        GuardianApprovalReviewAction::RequestPermissions { permissions, .. } => {
            let requested_network_access = permissions
                .network
                .as_ref()
                .and_then(|network| network.enabled)
                .unwrap_or(false);
            let trigger = if requested_network_access {
                ReviewTrigger::NetworkPolicyDenial
            } else if permissions.file_system.is_some() {
                ReviewTrigger::SandboxDenial
            } else {
                ReviewTrigger::Initial
            };
            (
                ReviewSubjectKind::Permissions,
                "permissions".to_string(),
                trigger,
            )
        }
        GuardianApprovalReviewAction::McpToolCall { tool_name, .. } => (
            ReviewSubjectKind::McpToolCall,
            tool_name.clone(),
            ReviewTrigger::Initial,
        ),
    }
}

fn guardian_review_requested_additional_permissions(action: &GuardianApprovalReviewAction) -> bool {
    match action {
        GuardianApprovalReviewAction::ApplyPatch { .. }
        | GuardianApprovalReviewAction::NetworkAccess { .. } => true,
        GuardianApprovalReviewAction::RequestPermissions { permissions, .. } => {
            guardian_review_request_permissions_network_enabled(permissions)
                || permissions.file_system.is_some()
        }
        GuardianApprovalReviewAction::Command { .. }
        | GuardianApprovalReviewAction::Execve { .. }
        | GuardianApprovalReviewAction::McpToolCall { .. } => false,
    }
}

fn guardian_review_requested_network_access(action: &GuardianApprovalReviewAction) -> bool {
    match action {
        GuardianApprovalReviewAction::NetworkAccess { .. } => true,
        GuardianApprovalReviewAction::RequestPermissions { permissions, .. } => {
            guardian_review_request_permissions_network_enabled(permissions)
        }
        GuardianApprovalReviewAction::ApplyPatch { .. }
        | GuardianApprovalReviewAction::Command { .. }
        | GuardianApprovalReviewAction::Execve { .. }
        | GuardianApprovalReviewAction::McpToolCall { .. } => false,
    }
}

fn guardian_review_request_permissions_network_enabled(
    permissions: &RequestPermissionProfile,
) -> bool {
    permissions
        .network
        .as_ref()
        .and_then(|network| network.enabled)
        .unwrap_or(false)
}

fn final_approval_outcome(
    reviewer: Reviewer,
    status: ReviewStatus,
    resolution: ReviewResolution,
) -> FinalApprovalOutcome {
    match (reviewer, status, resolution) {
        (Reviewer::Guardian, ReviewStatus::Approved, _) => FinalApprovalOutcome::GuardianApproved,
        (Reviewer::Guardian, ReviewStatus::Denied, _) => FinalApprovalOutcome::GuardianDenied,
        (Reviewer::Guardian, _, _) => FinalApprovalOutcome::GuardianAborted,
        (Reviewer::User, ReviewStatus::Approved, ReviewResolution::SessionApproval) => {
            FinalApprovalOutcome::UserApprovedForSession
        }
        (Reviewer::User, ReviewStatus::Approved, _) => FinalApprovalOutcome::UserApproved,
        (Reviewer::User, ReviewStatus::Denied, _) => FinalApprovalOutcome::UserDenied,
        (Reviewer::User, _, _) => FinalApprovalOutcome::UserAborted,
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

fn accepted_line_event_input(
    turn_id: &str,
    turn_state: &TurnState,
) -> Option<(AcceptedLineFingerprintEventInput, PathBuf)> {
    let latest_diff = turn_state.latest_diff.as_deref()?;
    let summary = accepted_line_fingerprints_from_unified_diff(latest_diff);
    if summary.accepted_added_lines == 0 && summary.accepted_deleted_lines == 0 {
        return None;
    }

    let thread_id = turn_state.thread_id.clone()?;
    let resolved_config = turn_state.resolved_config.clone()?;

    Some((
        AcceptedLineFingerprintEventInput {
            event_type: "codex.accepted_line_fingerprints",
            turn_id: turn_id.to_string(),
            thread_id,
            product_surface: Some("codex".to_string()),
            model_slug: Some(resolved_config.model.clone()),
            completed_at: now_unix_seconds(),
            repo_hash: None,
            accepted_added_lines: summary.accepted_added_lines,
            accepted_deleted_lines: summary.accepted_deleted_lines,
            line_fingerprints: summary.line_fingerprints,
        },
        resolved_config.permission_profile_cwd,
    ))
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
        thread_source: thread_metadata.thread_source,
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
        total_tool_call_count: Some(turn_state.tool_counts.total),
        shell_command_count: Some(turn_state.tool_counts.shell_command),
        file_change_count: Some(turn_state.tool_counts.file_change),
        mcp_tool_call_count: Some(turn_state.tool_counts.mcp_tool_call),
        dynamic_tool_call_count: Some(turn_state.tool_counts.dynamic_tool_call),
        subagent_tool_call_count: Some(turn_state.tool_counts.subagent_tool_call),
        web_search_count: Some(turn_state.tool_counts.web_search),
        image_generation_count: Some(turn_state.tool_counts.image_generation),
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
            let file_system_policy = permission_profile.file_system_sandbox_policy();
            if file_system_policy.has_full_disk_write_access() {
                if permission_profile.network_sandbox_policy().is_enabled() {
                    "full_access"
                } else {
                    "external_sandbox"
                }
            } else if file_system_policy
                .get_writable_roots_with_cwd(cwd)
                .is_empty()
            {
                "read_only"
            } else {
                "workspace_write"
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

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::models::SandboxEnforcement;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::NetworkSandboxPolicy;

    #[test]
    fn managed_full_disk_with_restricted_network_reports_external_sandbox() {
        let permission_profile = PermissionProfile::from_runtime_permissions_with_enforcement(
            SandboxEnforcement::Managed,
            &FileSystemSandboxPolicy::unrestricted(),
            NetworkSandboxPolicy::Restricted,
        );

        assert_eq!(
            sandbox_policy_mode(&permission_profile, Path::new("/")),
            "external_sandbox"
        );
    }

    #[test]
    fn guardian_review_result_maps_terminal_statuses() {
        assert!(guardian_review_result(GuardianApprovalReviewStatus::InProgress).is_none());
        assert!(matches!(
            guardian_review_result(GuardianApprovalReviewStatus::TimedOut),
            Some((ReviewStatus::TimedOut, ReviewResolution::None))
        ));
    }
}
