//! Captures one model response on an inherited ephemeral fork before Core can act on it.
//! Tool calls produce a null sample; an owned task retains startup and cleanup.

use crate::connection_rpc_gate::ConnectionRpcGate;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::request_processors::build_legacy_api_turns_from_rollout_items;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ThreadSampleParams;
use codex_app_server_protocol::ThreadSampleResponse;
use codex_app_server_protocol::TurnStatus;
use codex_core::CodexThread;
use codex_core::ForkSnapshot;
use codex_core::ThreadManager;
use codex_extension_api::ModelRequestContributor;
use codex_extension_api::ModelRequestInput;
use codex_extension_api::ModelRequestKind;
use codex_extension_api::ModelResponseInterceptor;
use codex_extension_api::ModelResponseStream;
use codex_extension_api::ResponseEvent;
use codex_features::Feature;
use codex_otel::auth_storage::AuthStorageOriginator;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::protocol::W3cTraceContext;
use codex_protocol::turn_input::StartIfIdleSubmission;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnStartOptions;
use codex_protocol::user_input::UserInput;
use codex_rollout::InitialHistory;
use codex_rollout::ResumedHistory;
use codex_thread_store::ListTurnsParams;
use codex_thread_store::LoadThreadHistoryParams;
use codex_thread_store::SortDirection;
use codex_thread_store::StoredTurnItemsView;
use codex_thread_store::StoredTurnStatus;
use codex_thread_store::ThreadStore;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

const SAMPLE_TIMEOUT: Duration = Duration::from_secs(30);
// Bytes conservatively bound tokens, including for byte-fallback tokenizers.
const MAX_PROMPT_BYTES: usize = 4_000;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
type SampleResult = Result<ThreadSampleResponse, JSONRPCErrorError>;
type SampleSender = Arc<Mutex<Option<oneshot::Sender<SampleResult>>>>;

/// Marks private sample children from startup through cleanup.
pub(crate) struct SampleThread;

pub(crate) struct SampleTask {
    pub(crate) manager: Arc<ThreadManager>,
    pub(crate) store: Arc<dyn ThreadStore>,
    pub(crate) parent: Arc<CodexThread>,
    pub(crate) samples: Arc<SampleResponseRegistry>,
    pub(crate) params: ThreadSampleParams,
    pub(crate) trace: Option<W3cTraceContext>,
    pub(crate) cancellation: CancellationToken,
}

pub(crate) async fn generate_sample(
    task: SampleTask,
    tasks: &TaskTracker,
    rpc_gate: Arc<ConnectionRpcGate>,
) -> SampleResult {
    let deadline = Instant::now() + SAMPLE_TIMEOUT;
    if task.params.expected_turn_id.trim().is_empty()
        || task.params.prompt.trim().is_empty()
        || task.params.prompt.len() > MAX_PROMPT_BYTES
    {
        return Err(invalid_request(format!(
            "thread/sample requires expectedTurnId and a prompt of at most {MAX_PROMPT_BYTES} bytes",
        )));
    }
    let (sender, receiver) = oneshot::channel();
    let originator = AuthStorageOriginator::current();
    tasks.spawn(async move {
        rpc_gate
            .run(originator.scope(async move {
                let result = Box::pin(task.run(deadline)).await;
                let _ = sender.send(result);
            }))
            .await;
    });
    tokio::time::timeout_at(deadline, receiver)
        .await
        .map_err(|_| internal_error("sample timed out"))?
        .map_err(|_| internal_error("sample task stopped"))?
}

impl SampleTask {
    async fn run(self, deadline: Instant) -> SampleResult {
        let Self {
            manager,
            store,
            parent,
            samples,
            params,
            trace,
            cancellation,
        } = self;
        let id = parent.startup_metadata().thread_id;
        if parent.agent_status().await == AgentStatus::Running {
            return Err(invalid_request("sample source must be idle"));
        }
        let snapshot = parent.config_snapshot().await;
        if snapshot.ephemeral || snapshot.session_source.is_non_root_agent() {
            return Err(invalid_request(
                "sample source must be a persistent root thread",
            ));
        }
        let parent_history = parent.conversation_history_snapshot().await;
        let revision = parent_history.history_version();
        parent
            .flush_rollout()
            .await
            .map_err(|error| internal_error(error.to_string()))?;
        let history = store
            .load_latest_model_context(LoadThreadHistoryParams {
                thread_id: id,
                include_archived: false,
            })
            .await
            .map_err(|error| invalid_request(error.to_string()))?;
        let matches_source = match snapshot.history_mode {
            ThreadHistoryMode::Legacy => build_legacy_api_turns_from_rollout_items(&history.items)
                .last()
                .is_some_and(|turn| {
                    turn.id == params.expected_turn_id
                        && matches!(
                            turn.status,
                            TurnStatus::Completed | TurnStatus::Failed | TurnStatus::Interrupted
                        )
                }),
            ThreadHistoryMode::Paginated => store
                .list_turns(ListTurnsParams {
                    thread_id: id,
                    include_archived: false,
                    cursor: None,
                    page_size: 1,
                    sort_direction: SortDirection::Desc,
                    items_view: StoredTurnItemsView::NotLoaded,
                })
                .await
                .map_err(|error| invalid_request(error.to_string()))?
                .turns
                .first()
                .is_some_and(|turn| {
                    turn.turn_id == params.expected_turn_id
                        && matches!(
                            turn.status,
                            StoredTurnStatus::Completed
                                | StoredTurnStatus::Failed
                                | StoredTurnStatus::Interrupted
                        )
                }),
        };
        if !matches_source {
            return Err(invalid_request(
                "sample source turn must be the latest finished turn",
            ));
        }
        let (mut options, settings) = Box::pin(async {
            manager
                .fork_options_from_parent(id)
                .await
                .map(|(options, settings)| (Box::new(options), Box::new(settings)))
        })
        .await
        .map_err(|error| internal_error(error.to_string()))?;
        let model = manager
            .get_models_manager()
            .get_model_info(
                options.config.model.as_deref().unwrap_or(&snapshot.model),
                &options.config.to_models_manager_config(),
            )
            .await;
        options.config.ephemeral = true;
        options
            .config
            .features
            .enable(Feature::ReasoningEffortOverride)
            .map_err(|error| invalid_request(error.to_string()))?;
        options
            .thread_extension_init
            .insert(codex_core::compact::AutoCompactionDisabled);
        if params.disable_mcp
            && model.use_responses_lite
            && parent_history
                .items()
                .any(|item| matches!(item, ResponseItem::AdditionalTools { .. }))
        {
            options
                .thread_extension_init
                .insert(codex_core::McpServersDisabled);
        }
        options.thread_extension_init.insert(SampleThread);
        let effort = params.effort.filter(|_| {
            model.supports_reasoning_effort_updates && options.config.model_provider.is_openai()
        });
        options.session_source = Some(SessionSource::Custom("thread_sample".to_string()));
        options.thread_source = Some(
            params
                .thread_source
                .map(Into::into)
                .unwrap_or_else(|| ThreadSource::Feature("thread_sample".to_string())),
        );
        options.parent_trace = trace.clone();
        if cancellation.is_cancelled() {
            return Err(internal_error("sample canceled"));
        }
        if Instant::now() >= deadline {
            return Err(internal_error("sample timed out"));
        }
        // Do not cancel startup by dropping it: the owner must reclaim a partially initialized fork.
        let fork = Box::pin(manager.fork_thread_from_history(
            ForkSnapshot::Interrupted,
            *options,
            InitialHistory::Resumed(ResumedHistory {
                history_revision: history.revision,
                conversation_id: id,
                history: Arc::new(history.items),
                rollout_path: None,
            }),
        ))
        .await
        .map_err(|error| internal_error(format!("sample fork failed: {error}")))?;
        let (sender, mut receiver) = oneshot::channel();
        samples
            .captures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                fork.thread_id.to_string(),
                Arc::new(Mutex::new(Some(sender))),
            );
        let sample = async {
            Box::pin(fork.thread.restore_thread_settings(*settings))
                .await
                .map_err(|error| internal_error(error.to_string()))?;
            let input = TurnInputRequest::user_input(vec![UserInput::Text {
                text: params.prompt,
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                effort: effort.map(Some),
                ..Default::default()
            })
            .on_start(TurnStartOptions {
                turn_trigger: params.turn_trigger,
                ..Default::default()
            })
            .with_responses_metadata(params.responsesapi_client_metadata)
            .with_trace(trace);
            if !matches!(
                fork.thread
                    .start_turn_if_idle(input)
                    .await
                    .map_err(|error| internal_error(error.to_string()))?,
                StartIfIdleSubmission::Started { .. }
            ) {
                return Err(internal_error("sample turn was not admitted"));
            }
            loop {
                let event = tokio::select! {
                    biased;
                    result = &mut receiver => {
                        return result.map_err(|_| internal_error("sample request stopped"))?;
                    }
                    event = fork.thread.next_event() => {
                        event.map_err(|error| internal_error(error.to_string()))?
                    }
                };
                match event.msg {
                    EventMsg::TurnComplete(turn) => {
                        return Err(internal_error(turn.error.map_or_else(
                            || "sample completed without a response".to_string(),
                            |error| error.message,
                        )));
                    }
                    EventMsg::TurnAborted(_) | EventMsg::ShutdownComplete => {
                        return Err(internal_error("sample turn stopped"));
                    }
                    EventMsg::ExecApprovalRequest(_)
                    | EventMsg::ApplyPatchApprovalRequest(_)
                    | EventMsg::RequestPermissions(_)
                    | EventMsg::RequestUserInput(_)
                    | EventMsg::DynamicToolCallRequest(_)
                    | EventMsg::ElicitationRequest(_) => {
                        return Err(internal_error("sample turn requires client interaction"));
                    }
                    _ => {}
                }
            }
        };
        let result = tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(internal_error("sample canceled")),
            _ = tokio::time::sleep_until(deadline) => Err(internal_error("sample timed out")),
            result = sample => result,
        };
        let cleanup = fork.thread.shutdown_and_wait().await;
        manager
            .remove_thread_if_matches(&fork.thread_id, &fork.thread)
            .await;
        samples
            .captures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&fork.thread_id.to_string());
        let mut response = result?;
        cleanup.map_err(|error| internal_error(error.to_string()))?;
        if parent
            .conversation_history_snapshot()
            .await
            .history_version()
            != revision
        {
            return Err(invalid_request("sample source changed"));
        }
        response.source_turn_id = params.expected_turn_id;
        Ok(response)
    }
}

#[derive(Debug, Default)]
pub(crate) struct SampleResponseRegistry {
    captures: Mutex<HashMap<String, SampleSender>>,
}

impl ModelRequestContributor for SampleResponseRegistry {
    fn request(&self, input: ModelRequestInput<'_>) -> Option<Box<dyn ModelResponseInterceptor>> {
        if input.kind != ModelRequestKind::Generation {
            return None;
        }
        self.captures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(input.thread_id)
            .cloned()
            .map(|sender| {
                Box::new(SampleInterceptor { sender }) as Box<dyn ModelResponseInterceptor>
            })
    }
}

struct SampleInterceptor {
    sender: SampleSender,
}

impl ModelResponseInterceptor for SampleInterceptor {
    fn intercept(self: Box<Self>, upstream: ModelResponseStream) -> ModelResponseStream {
        Box::pin(futures::stream::once(async move {
            let result = collect_response(upstream).await;
            if let Some(sender) = self
                .sender
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                let _ = sender.send(result);
            }
            // Completion permits Stop hooks; EOF triggers stream recovery. Wait for owner cleanup.
            std::future::pending().await
        }))
    }
}

async fn collect_response(mut upstream: ModelResponseStream) -> SampleResult {
    let mut text = String::new();
    let mut text_only = true;
    while let Some(event) = upstream.next().await {
        match event.map_err(|error| internal_error(format!("sample failed: {error}")))? {
            ResponseEvent::OutputItemDone(ResponseItem::Message {
                phase: Some(MessagePhase::Commentary),
                ..
            }) => {}
            ResponseEvent::OutputItemDone(ResponseItem::Message { role, content, .. })
                if role == "assistant" =>
            {
                for item in content {
                    match item {
                        ContentItem::OutputText { text: part, .. } => {
                            if text.len().saturating_add(part.len()) > MAX_OUTPUT_BYTES {
                                return Err(internal_error("sample output exceeds limit"));
                            }
                            text.push_str(&part);
                        }
                        _ => text_only = false,
                    }
                }
            }
            ResponseEvent::OutputItemDone(ResponseItem::Reasoning { .. }) => {}
            ResponseEvent::OutputItemDone(_)
            | ResponseEvent::OutputItemAdded(
                ResponseItem::FunctionCall { .. }
                | ResponseItem::CustomToolCall { .. }
                | ResponseItem::LocalShellCall { .. }
                | ResponseItem::ToolSearchCall { .. },
            ) => text_only = false,
            ResponseEvent::Completed {
                response_id,
                token_usage,
                ..
            } => {
                return Ok(ThreadSampleResponse {
                    source_turn_id: String::new(),
                    response_id,
                    text: (text_only && !text.trim().is_empty()).then_some(text),
                    input_tokens: token_usage.as_ref().map(|usage| usage.input_tokens),
                    cached_input_tokens: token_usage
                        .as_ref()
                        .map(|usage| usage.cached_input_tokens),
                    output_tokens: token_usage.as_ref().map(|usage| usage.output_tokens),
                });
            }
            _ => {}
        }
    }
    Err(internal_error("sample stream ended before completion"))
}
