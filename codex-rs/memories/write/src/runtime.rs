use crate::metrics::MEMORY_STORAGE_BYTES;
use crate::workspace::memory_storage_bytes;
use codex_core::CodexThread;
use codex_core::ModelClient;
use codex_core::NewThread;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::StartIfIdleSubmission;
use codex_core::StartThreadOptions;
use codex_core::ThreadManager;
use codex_core::TurnInputRequest;
use codex_core::TurnStartOptions;
use codex_core::config::Config;
use codex_core::content_items_to_text;
use codex_core::detached_memory_responses_metadata;
use codex_core::resolve_installation_id;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::auth::AgentIdentityAuthPolicy;
use codex_login::auth_env_telemetry::collect_auth_env_telemetry;
use codex_login::default_client::originator;
use codex_model_provider::ModelProvider;
use codex_model_provider::SharedModelProvider;
use codex_model_provider::create_model_provider;
use codex_otel::SessionTelemetry;
use codex_otel::TelemetryAuthMode;
use codex_protocol::MemoryVersion;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;
use codex_rollout_trace::InferenceTraceContext;
use codex_state::MemoryStore;
use codex_terminal_detection::user_agent;
use futures::StreamExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct SpawnedConsolidationAgent {
    pub(crate) thread_id: ThreadId,
    pub(crate) thread: Arc<CodexThread>,
}

#[derive(Clone, Debug)]
pub(crate) struct StageOneRequestContext {
    version: MemoryVersion,
    pub(crate) model_info: ModelInfo,
    pub(crate) session_telemetry: SessionTelemetry,
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
    pub(crate) reasoning_summary: ReasoningSummary,
    pub(crate) service_tier: Option<String>,
}

impl StageOneRequestContext {
    pub(crate) fn start_timer(&self, name: &str) -> Option<codex_otel::Timer> {
        self.session_telemetry
            .start_timer(name, &memory_metric_tags(self.version, &[]))
            .ok()
    }

    pub(crate) fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) {
        self.session_telemetry
            .counter(name, inc, &memory_metric_tags(self.version, tags));
    }

    pub(crate) fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) {
        self.session_telemetry
            .histogram(name, value, &memory_metric_tags(self.version, tags));
    }
}

pub(crate) struct MemoryStartupContext {
    version: MemoryVersion,
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
    thread_manager: Arc<ThreadManager>,
    auth_manager: Arc<AuthManager>,
    provider: SharedModelProvider,
    session_telemetry: SessionTelemetry,
}

fn memory_metric_tags<'a>(
    version: MemoryVersion,
    tags: &[(&'a str, &'a str)],
) -> Vec<(&'a str, &'a str)> {
    let mut tags = tags.to_vec();
    tags.push((
        "memory_version",
        match version {
            MemoryVersion::V1 => "v1",
            MemoryVersion::V2 => "v2",
        },
    ));
    tags
}

fn build_session_telemetry(
    auth_manager: &AuthManager,
    thread_id: ThreadId,
    config: &Config,
    source: SessionSource,
    model: &str,
    originator: String,
) -> SessionTelemetry {
    let auth = auth_manager.auth_cached();
    let auth = auth.as_ref();
    let auth_mode = auth.map(CodexAuth::auth_mode).map(TelemetryAuthMode::from);
    let account_id = auth.and_then(CodexAuth::get_account_id);
    let account_email = auth.and_then(CodexAuth::get_account_email);
    let auth_env_telemetry = collect_auth_env_telemetry(
        &config.model_provider,
        auth_manager.codex_api_key_env_enabled(),
    );
    SessionTelemetry::new(
        thread_id,
        model,
        model,
        account_id,
        account_email,
        auth_mode,
        originator,
        config.otel.log_user_prompt,
        user_agent(),
        source,
    )
    .with_auth_env(auth_env_telemetry.to_otel_metadata())
}

impl MemoryStartupContext {
    pub(crate) fn new(
        thread_manager: Arc<ThreadManager>,
        auth_manager: Arc<AuthManager>,
        thread_id: ThreadId,
        thread: Arc<CodexThread>,
        config: &Config,
        source: SessionSource,
    ) -> Self {
        let provider = create_model_provider(
            config.model_provider.clone(),
            Some(Arc::clone(&auth_manager)),
        );
        Self::new_with_provider(
            thread_manager,
            auth_manager,
            thread_id,
            thread,
            config,
            source,
            provider,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_for_testing(
        thread_manager: Arc<ThreadManager>,
        auth_manager: Arc<AuthManager>,
        thread_id: ThreadId,
        thread: Arc<CodexThread>,
        config: &Config,
        source: SessionSource,
        provider: SharedModelProvider,
    ) -> Self {
        Self::new_with_provider(
            thread_manager,
            auth_manager,
            thread_id,
            thread,
            config,
            source,
            provider,
        )
    }

    fn new_with_provider(
        thread_manager: Arc<ThreadManager>,
        auth_manager: Arc<AuthManager>,
        thread_id: ThreadId,
        thread: Arc<CodexThread>,
        config: &Config,
        source: SessionSource,
        provider: SharedModelProvider,
    ) -> Self {
        let model = config.model.as_deref().unwrap_or("unknown");
        let session_telemetry = build_session_telemetry(
            &auth_manager,
            thread_id,
            config,
            source,
            model,
            originator().value,
        );

        Self {
            version: config.memories.version,
            thread_id,
            thread,
            thread_manager,
            auth_manager,
            provider,
            session_telemetry,
        }
    }

    pub(crate) fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    pub(crate) async fn record_storage_size(&self, root: &Path) {
        let bytes = match memory_storage_bytes(root).await {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::warn!("failed measuring memory storage size: {err}");
                return;
            }
        };
        self.session_telemetry.histogram_with_boundaries(
            MEMORY_STORAGE_BYTES,
            i64::try_from(bytes).unwrap_or(i64::MAX),
            // Log-spaced byte buckets cover small summaries through large memory collections.
            &[
                0.0,
                1_024.0,
                4_096.0,
                16_384.0,
                65_536.0,
                262_144.0,
                1_048_576.0,
                4_194_304.0,
                16_777_216.0,
                67_108_864.0,
                268_435_456.0,
                1_073_741_824.0,
            ],
            &memory_metric_tags(self.version, &[]),
        );
    }

    pub(crate) async fn memory_store(&self) -> Option<MemoryStore> {
        match self
            .thread
            .state_db()?
            .memories_for_version(self.version)
            .await
        {
            Ok(store) => Some(store),
            Err(err) => {
                tracing::warn!("failed opening memory store: {err}");
                None
            }
        }
    }

    pub(crate) fn provider(&self) -> &dyn ModelProvider {
        self.provider.as_ref()
    }

    pub(crate) fn counter(&self, name: &str, inc: i64, tags: &[(&str, &str)]) {
        self.session_telemetry
            .counter(name, inc, &memory_metric_tags(self.version, tags));
    }

    pub(crate) fn histogram(&self, name: &str, value: i64, tags: &[(&str, &str)]) {
        self.session_telemetry
            .histogram(name, value, &memory_metric_tags(self.version, tags));
    }

    pub(crate) fn start_timer(&self, name: &str) -> Option<codex_otel::Timer> {
        self.session_telemetry
            .start_timer(name, &memory_metric_tags(self.version, &[]))
            .ok()
    }

    pub(crate) async fn stage_one_request_context(
        &self,
        config: &Config,
        model_name: &str,
        reasoning_effort: ReasoningEffort,
    ) -> StageOneRequestContext {
        let config_snapshot = self.thread.config_snapshot().await;
        let model_info = self
            .thread_manager
            .get_models_manager()
            .get_model_info(model_name, &config.to_models_manager_config())
            .await;
        let reasoning_summary = config
            .model_reasoning_summary
            .unwrap_or(model_info.default_reasoning_summary);

        StageOneRequestContext {
            version: self.version,
            model_info,
            session_telemetry: build_session_telemetry(
                &self.auth_manager,
                self.thread_id,
                config,
                config_snapshot.session_source,
                model_name,
                config_snapshot.originator,
            ),
            reasoning_effort: Some(reasoning_effort),
            reasoning_summary,
            service_tier: config_snapshot.service_tier,
        }
    }

    pub(crate) async fn stream_stage_one_prompt(
        &self,
        config: &Config,
        prompt: &Prompt,
        context: &StageOneRequestContext,
    ) -> anyhow::Result<(String, Option<TokenUsage>)> {
        let installation_id = resolve_installation_id(&config.codex_home).await?;
        let config_snapshot = self.thread.config_snapshot().await;
        let session_source = config_snapshot.session_source;
        let session_id = SessionId::from(self.thread_id);
        let session_id_string = session_id.to_string();
        let model_client = ModelClient::new(
            Some(Arc::clone(&self.auth_manager)),
            AgentIdentityAuthPolicy::JwtOnly,
            self.thread_id,
            config.model_provider.clone(),
            session_source.clone(),
            config_snapshot.originator,
            config.model_verbosity,
            config.features.enabled(Feature::ContentItemKinds),
            config.features.enabled(Feature::ReasoningEffortOverride),
            config.features.enabled(Feature::EnableRequestCompression),
            config.features.enabled(Feature::RuntimeMetrics),
            /*beta_features_header*/ None,
            /*concurrent_reasoning_summaries_enabled*/ false,
            /*attestation_provider*/ None,
            config.http_client_factory(),
            config.workspace_routing_context(),
            Vec::new(),
        );

        let mut client_session = model_client.new_session();
        let window_id = format!("{}:0", self.thread_id);
        let responses_metadata = detached_memory_responses_metadata(
            &self.thread_manager,
            installation_id,
            session_id_string,
            self.thread_id.to_string(),
            window_id,
            &session_source,
            &config.cwd,
            &config_snapshot.permission_profile,
            /*sandbox*/ None,
        )
        .await;
        let mut stream = client_session
            .stream(
                prompt,
                &context.model_info,
                &context.session_telemetry,
                context.reasoning_effort.clone(),
                context.reasoning_summary,
                context.service_tier.clone(),
                &responses_metadata,
                &InferenceTraceContext::disabled(),
            )
            .await?;

        let mut result = String::new();
        let mut token_usage = None;
        while let Some(message) = stream.next().await.transpose()? {
            match message {
                ResponseEvent::OutputTextDelta(delta) => result.push_str(&delta),
                ResponseEvent::OutputItemDone(item) => {
                    if result.is_empty()
                        && let codex_protocol::models::ResponseItem::Message { content, .. } = item
                        && let Some(text) = content_items_to_text(&content)
                    {
                        result.push_str(&text);
                    }
                }
                ResponseEvent::Completed {
                    token_usage: usage, ..
                } => {
                    token_usage = usage;
                    break;
                }
                _ => {}
            }
        }

        Ok((result, token_usage))
    }

    pub(crate) async fn spawn_consolidation_agent(
        &self,
        config: Config,
        prompt: Vec<UserInput>,
    ) -> anyhow::Result<SpawnedConsolidationAgent> {
        let NewThread {
            thread_id, thread, ..
        } = self
            .thread_manager
            .start_thread(StartThreadOptions {
                session_source: Some(SessionSource::Internal(
                    InternalSessionSource::MemoryConsolidation,
                )),
                thread_source: Some(ThreadSource::MemoryConsolidation),
                ..StartThreadOptions::new(config)
            })
            .await?;

        let agent = SpawnedConsolidationAgent { thread_id, thread };
        let submit_result = match agent
            .thread
            .start_turn_if_idle(
                TurnInputRequest::user_input(prompt).on_start(TurnStartOptions {
                    turn_trigger: Some("memory_consolidation".to_owned()),
                    ..Default::default()
                }),
            )
            .await
        {
            Ok(StartIfIdleSubmission::Started { .. }) => Ok(()),
            Ok(submission) => Err(anyhow::anyhow!(
                "memory consolidation input was not started: {submission:?}"
            )),
            Err(err) => Err(err.into()),
        };
        if let Err(err) = submit_result {
            if let Err(shutdown_err) = self.shutdown_consolidation_agent(agent).await {
                tracing::warn!(
                    "failed to shut down consolidation agent after submit error: {shutdown_err}"
                );
            }
            return Err(err);
        }

        Ok(agent)
    }

    pub(crate) async fn shutdown_consolidation_agent(
        &self,
        agent: SpawnedConsolidationAgent,
    ) -> anyhow::Result<()> {
        let SpawnedConsolidationAgent { thread_id, thread } = agent;
        tokio::time::timeout(Duration::from_secs(10), thread.shutdown_and_wait())
            .await
            .map_err(|_| {
                anyhow::anyhow!("memory consolidation agent {thread_id} shutdown timed out")
            })??;

        self.thread_manager.remove_thread(&thread_id).await;

        Ok(())
    }
}
