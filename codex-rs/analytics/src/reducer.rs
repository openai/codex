use crate::events::AppServerRpcTransport;
use crate::events::CodexAppMentionedEventRequest;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexAppUsedEventRequest;
use crate::events::CodexCompactionEventRequest;
use crate::events::CodexPluginEventRequest;
use crate::events::CodexPluginUsedEventRequest;
use crate::events::CodexRuntimeMetadata;
use crate::events::SkillInvocationEventParams;
use crate::events::SkillInvocationEventRequest;
use crate::events::ThreadInitializationMode;
use crate::events::ThreadInitializedEvent;
use crate::events::ThreadInitializedEventParams;
use crate::events::TrackEventRequest;
use crate::events::codex_app_metadata;
use crate::events::codex_compaction_event_params;
use crate::events::codex_plugin_metadata;
use crate::events::codex_plugin_used_metadata;
use crate::events::plugin_state_event_type;
use crate::events::subagent_thread_started_event_request;
use crate::events::thread_source_name;
use crate::facts::AnalyticsFact;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::CodexCompactionEvent;
use crate::facts::CustomAnalyticsFact;
use crate::facts::PluginState;
use crate::facts::PluginStateChangedInput;
use crate::facts::PluginUsedInput;
use crate::facts::SkillInvokedInput;
use crate::facts::SubAgentThreadStartedInput;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::InitializeParams;
use codex_git_utils::collect_git_info;
use codex_git_utils::get_git_repo_root;
use codex_login::default_client::originator;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use sha1::Digest;
use std::collections::HashMap;
use std::path::Path;

#[derive(Default)]
pub(crate) struct AnalyticsReducer {
    connections: HashMap<u64, ConnectionState>,
    thread_connections: HashMap<String, u64>,
    pending_compactions: HashMap<String, Vec<CodexCompactionEvent>>,
}

struct ConnectionState {
    app_server_client: CodexAppServerClientMetadata,
    runtime: CodexRuntimeMetadata,
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
                connection_id: _connection_id,
                request_id: _request_id,
                request: _request,
            } => {}
            AnalyticsFact::Response {
                connection_id,
                response,
            } => {
                self.ingest_response(connection_id, *response, out);
            }
            AnalyticsFact::Notification(_notification) => {}
            AnalyticsFact::Custom(input) => match input {
                CustomAnalyticsFact::SubAgentThreadStarted(input) => {
                    self.ingest_subagent_thread_started(input, out);
                }
                CustomAnalyticsFact::Compaction(input) => {
                    self.ingest_compaction(*input, out);
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
        let thread_id = thread.id;
        self.thread_connections
            .insert(thread_id.clone(), connection_id);
        let Some(connection_state) = self.connections.get(&connection_id) else {
            return;
        };
        out.push(TrackEventRequest::ThreadInitialized(
            ThreadInitializedEvent {
                event_type: "codex_thread_initialized",
                event_params: ThreadInitializedEventParams {
                    thread_id: thread_id.clone(),
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
        self.maybe_emit_pending_compactions(&thread_id, out);
    }

    fn ingest_compaction(&mut self, input: CodexCompactionEvent, out: &mut Vec<TrackEventRequest>) {
        let thread_id = input.thread_id.clone();
        if self.try_emit_compaction(input.clone(), out) {
            return;
        }
        self.pending_compactions
            .entry(thread_id)
            .or_default()
            .push(input);
    }

    fn maybe_emit_pending_compactions(
        &mut self,
        thread_id: &str,
        out: &mut Vec<TrackEventRequest>,
    ) {
        let Some(pending) = self.pending_compactions.remove(thread_id) else {
            return;
        };
        let mut still_pending = Vec::new();
        for input in pending {
            if !self.try_emit_compaction(input.clone(), out) {
                still_pending.push(input);
            }
        }
        if !still_pending.is_empty() {
            self.pending_compactions
                .insert(thread_id.to_string(), still_pending);
        }
    }

    fn try_emit_compaction(
        &mut self,
        input: CodexCompactionEvent,
        out: &mut Vec<TrackEventRequest>,
    ) -> bool {
        let connection_metadata = self
            .thread_connections
            .get(&input.thread_id)
            .and_then(|connection_id| self.connections.get(connection_id))
            .map(|connection_state| {
                (
                    connection_state.app_server_client.clone(),
                    connection_state.runtime.clone(),
                )
            });
        let Some((app_server_client, runtime)) = connection_metadata else {
            return false;
        };
        out.push(TrackEventRequest::Compaction(Box::new(
            CodexCompactionEventRequest {
                event_type: "codex_compaction_event",
                event_params: codex_compaction_event_params(app_server_client, runtime, input),
            },
        )));
        true
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
