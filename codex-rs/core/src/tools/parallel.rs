use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;
use std::time::Instant;

use tokio::sync::RwLock;
use tokio::task::JoinError;
use tokio_util::either::Either;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;
use tracing::Instrument;
use tracing::info;
use tracing::instrument;
use tracing::trace_span;

use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::tools::call_trace;
use crate::tools::context::AbortedToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolCallState;
use crate::tools::context::ToolPayload;
use crate::tools::lifecycle::notify_tool_aborted;
use crate::tools::registry::AnyToolResult;
use crate::tools::registry::ToolArgumentDiffConsumer;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolCallSource;
use codex_history::ResponseItemEnvelope;
use codex_protocol::error::CodexErr;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ToolResultMetadata;

struct ToolCallTimingGuard {
    started_at: Option<Instant>,
    execution_started_at: Arc<OnceLock<Instant>>,
    conversation_id: String,
    turn_id: String,
    call_id: String,
    tool_name: codex_tools::ToolName,
    extensions: Arc<codex_extension_api::ExtensionRegistry<crate::config::Config>>,
}

#[derive(Clone)]
pub(crate) struct ToolCallRuntime {
    session: Arc<Session>,
    // Tool calls may run later, so retain the step whose tool list advertised them.
    step_context: Arc<StepContext>,
    tracker: SharedTurnDiffTracker,
    parallel_execution: Arc<RwLock<()>>,
}

impl ToolCallRuntime {
    pub(crate) fn new(
        session: Arc<Session>,
        step_context: Arc<StepContext>,
        tracker: SharedTurnDiffTracker,
    ) -> Self {
        Self {
            session,
            step_context,
            tracker,
            parallel_execution: Arc::new(RwLock::new(())),
        }
    }

    pub(crate) fn create_diff_consumer(
        &self,
        tool_name: &codex_tools::ToolName,
    ) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        self.step_context
            .tool_router
            .create_diff_consumer(tool_name)
    }

    #[instrument(level = "trace", skip_all)]
    pub(crate) fn handle_tool_call(
        self,
        call: ToolCall,
        cancellation_token: CancellationToken,
    ) -> impl std::future::Future<Output = Result<ResponseItemEnvelope, CodexErr>> {
        let error_call = call.clone();
        let source = call.direct_source();
        let recorder = self.session.services.executed_tool_calls.clone();
        let recorded_call = recorder.prepare_direct_call(&call, &source, &self.step_context);
        let step_context = Arc::clone(&self.step_context);
        let call_state = Arc::new(ToolCallState::default());
        let future = self.handle_tool_call_with_source(
            step_context,
            call,
            source,
            cancellation_token,
            Arc::clone(&call_state),
        );
        async move {
            let result = future.await;
            let mut recorded_call =
                recorded_call.filter(|(_, recording)| recording.strong_count() > 0);
            let mut response = match result {
                Ok(result) => {
                    if let Some((call, _)) = recorded_call.as_mut()
                        && let Some(metadata) = result.result.tool_result_metadata()
                    {
                        call.set_tool_result_metadata(ToolResultMetadata::new(metadata));
                    }
                    result.into_response()
                }
                Err(FunctionCallError::Fatal(message)) => return Err(CodexErr::Fatal(message)),
                Err(other) => {
                    ResponseItemEnvelope::new(Self::failure_response(error_call, other).into())
                }
            };
            if let Some(text) = call_state.delivered_assistant_message.get() {
                response
                    .metadata
                    .get_or_insert_default()
                    .delivered_assistant_message = Some(text.clone());
            }
            recorder.attach_direct_call_to_output(&mut response.item, recorded_call);
            Ok(response)
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub(crate) fn handle_tool_call_with_source(
        self,
        step_context: Arc<StepContext>,
        call: ToolCall,
        source: ToolCallSource,
        cancellation_token: CancellationToken,
        call_state: Arc<ToolCallState>,
    ) -> impl std::future::Future<Output = Result<AnyToolResult, FunctionCallError>> {
        self.session
            .services
            .executed_tool_calls
            .record_tool_call(&call, &source, &step_context);
        let router = &step_context.tool_router;
        let supports_parallel = router.tool_supports_parallel(&call);
        let tool_runtime = router.tool_runtime(&call.tool_name);
        let router = Arc::clone(router);
        let session = Arc::clone(&self.session);
        let turn = Arc::clone(&step_context.turn);
        let tracker = Arc::clone(&self.tracker);
        let lock = Arc::clone(&self.parallel_execution);
        let invocation_cancellation_token = cancellation_token.clone();
        let started = Instant::now();
        let mut tool_call_timing_guard = ToolCallTimingGuard::capture(
            started,
            &session.thread_id,
            &turn.sub_id,
            &call,
            &source,
            Arc::clone(&session.services.extensions),
        );
        if let Some(guard) = tool_call_timing_guard.as_mut() {
            for observer in session.services.extensions.tool_lifecycle_contributors() {
                observer.on_tool_dispatch(codex_extension_api::ToolDispatchInput {
                    thread_id: &guard.conversation_id,
                    turn_id: &guard.turn_id,
                    call_id: &guard.call_id,
                    tool_name: &guard.tool_name,
                });
            }
        }
        let execution_started_at = tool_call_timing_guard
            .as_ref()
            .map(|timing| Arc::clone(&timing.execution_started_at));
        let abort_session = Arc::clone(&session);
        let abort_source = source.clone();
        let abort_turn = Arc::clone(&turn);
        let dispatch_call_state = Arc::clone(&call_state);
        let dispatch_call = call.clone();
        let thread_id = session.thread_id;
        let trace_source = match &source {
            ToolCallSource::Direct | ToolCallSource::DirectPlaintextMessage => {
                call_trace::Source::Direct
            }
            ToolCallSource::CodeMode { .. } => call_trace::Source::CodeMode,
        };
        let dispatch_tool_name = call.tool_name.clone();
        let dispatch_call_id = call.call_id.clone();

        // Code-mode callbacks can resume outside the turn's local span ancestry.
        let dispatch_span = trace_span!(
            "dispatch_tool_call_with_code_mode_result",
            otel.name = %call.tool_name,
            tool_name = %call.tool_name,
            thread.id = %session.thread_id,
            call_id = call.call_id.as_str(),
            aborted = false,
        );
        let abort_dispatch_span = dispatch_span.clone();

        let mut dispatch_handle = AbortOnDropHandle::new(tokio::spawn(
            async move {
                if let Some(tool_runtime) = tool_runtime
                    && let Some(readiness) = tool_runtime.wait_until_ready(&session)
                {
                    readiness.await;
                }

                let guard = if supports_parallel {
                    Either::Left(lock.read().await)
                } else {
                    Either::Right(lock.write().await)
                };
                // Admission through the parallel-execution gate marks the end
                // of dispatch waiting and the start of handler execution.
                if let Some(execution_started_at) = execution_started_at {
                    let _ = execution_started_at.set(Instant::now());
                }

                let result = router
                    .dispatch_tool_call_with_state(
                        session,
                        step_context,
                        invocation_cancellation_token,
                        tracker,
                        dispatch_call,
                        source,
                        dispatch_call_state,
                    )
                    .instrument(dispatch_span.clone())
                    .await;
                drop(guard);
                // The sampling loop collects results in order only after its stream ends.
                // Record readiness here, before either caller encodes or collects the result.
                // A fatal error still propagates to the caller instead of producing a tool
                // result; unlike a normal tool failure, it has no readiness event.
                if !matches!(&result, Err(FunctionCallError::Fatal(_))) {
                    call_trace::result_ready(
                        thread_id,
                        &turn.sub_id,
                        &dispatch_tool_name,
                        &dispatch_call_id,
                        trace_source,
                    );
                }
                result
            }
            .in_current_span(),
        ));

        async move {
            let mut tool_call_timing_guard = tool_call_timing_guard;
            let mut result = tokio::select! {
                res = &mut dispatch_handle => res.map_err(Self::tool_task_join_error)?,
                _ = cancellation_token.cancelled() => {
                    if call_state.terminal_outcome_reached.load(Ordering::Acquire) || dispatch_handle.is_finished() {
                        dispatch_handle.await.map_err(Self::tool_task_join_error)?
                    } else {
                        let secs = started.elapsed().as_secs_f32().max(0.1);
                        abort_dispatch_span.record("aborted", true);
                        dispatch_handle.abort();
                        match dispatch_handle.await {
                            Ok(result) => result,
                            Err(err) if err.is_cancelled() => {
                                let response = Self::aborted_response(&call, secs);
                                call_trace::result_ready(
                                    thread_id,
                                    &abort_turn.sub_id,
                                    &call.tool_name,
                                    &call.call_id,
                                    trace_source,
                                );
                                notify_tool_aborted(
                                    abort_session.as_ref(),
                                    abort_turn.as_ref(),
                                    call.call_id.as_str(),
                                    &call.tool_name,
                                    abort_source,
                                )
                                .await;
                                Ok(response)
                            }
                            Err(err) => Err(Self::tool_task_join_error(err)),
                        }
                    }
                },
            };
            // Use one completion measurement for logging and response formatting.
            // Measuring inside a handler would omit routing and output processing.
            if let Some(timing) = tool_call_timing_guard.as_mut()
                && let Some(handler_duration_ms) = timing.finish()
                && let Ok(result) = &mut result
            {
                result.result.set_handler_duration_ms(handler_duration_ms);
            }
            result
        }
        .in_current_span()
    }
}

impl ToolCallRuntime {
    fn tool_task_join_error(err: JoinError) -> FunctionCallError {
        FunctionCallError::Fatal(format!("tool task failed to receive: {err:?}"))
    }

    fn failure_response(call: ToolCall, err: FunctionCallError) -> ResponseInputItem {
        let message = err.to_string();
        match call.payload {
            ToolPayload::ToolSearch { .. } => ResponseInputItem::ToolSearchOutput {
                call_id: call.call_id,
                status: "completed".to_string(),
                execution: "client".to_string(),
                tools: Vec::new(),
            },
            ToolPayload::Custom { .. } => ResponseInputItem::CustomToolCallOutput {
                call_id: call.call_id,
                name: None,
                output: codex_protocol::models::FunctionCallOutputPayload {
                    body: codex_protocol::models::FunctionCallOutputBody::Text(message),
                    success: Some(false),
                },
            },
            _ => ResponseInputItem::FunctionCallOutput {
                call_id: call.call_id,
                output: codex_protocol::models::FunctionCallOutputPayload {
                    body: codex_protocol::models::FunctionCallOutputBody::Text(message),
                    success: Some(false),
                },
            },
        }
    }

    fn aborted_response(call: &ToolCall, secs: f32) -> AnyToolResult {
        AnyToolResult {
            call_id: call.call_id.clone(),
            payload: call.payload.clone(),
            result: Box::new(AbortedToolOutput {
                message: Self::abort_message(call, secs),
            }),
            post_tool_use_payload: None,
        }
    }

    fn abort_message(call: &ToolCall, secs: f32) -> String {
        if call.tool_name.is_default_namespace() && call.tool_name.name == "exec_command" {
            format!("Wall time: {secs:.1} seconds\naborted by user")
        } else {
            format!("aborted by user after {secs:.1}s")
        }
    }
}

impl ToolCallTimingGuard {
    fn capture(
        started_at: Instant,
        conversation_id: &impl std::fmt::Display,
        turn_id: &str,
        call: &ToolCall,
        source: &ToolCallSource,
        extensions: Arc<codex_extension_api::ExtensionRegistry<crate::config::Config>>,
    ) -> Option<Self> {
        // Code-mode calls are nested within a direct code-mode tool call whose
        // timing already includes them. Suppress nested guards so consumers do
        // not mistake overlapping events for independent tool-call latency.
        if !matches!(
            source,
            ToolCallSource::Direct | ToolCallSource::DirectPlaintextMessage
        ) {
            return None;
        }

        Some(Self {
            started_at: Some(started_at),
            execution_started_at: Arc::new(OnceLock::new()),
            conversation_id: conversation_id.to_string(),
            turn_id: turn_id.to_string(),
            call_id: call.call_id.clone(),
            tool_name: call.tool_name.clone(),
            extensions,
        })
    }

    /// Log call timing once and return the handler duration in milliseconds.
    fn finish(&mut self) -> Option<u64> {
        // Taking the start instant keeps normal completion and Drop from logging twice.
        let started_at = self.started_at.take()?;
        let completed_at = Instant::now();
        // Snapshot once so a concurrently-starting dispatch cannot make one
        // event internally inconsistent.
        let execution_started_at = self
            .execution_started_at
            .get()
            .copied()
            .filter(|execution_started_at| *execution_started_at <= completed_at);
        for observer in self.extensions.tool_lifecycle_contributors() {
            observer.on_tool_timing(codex_extension_api::ToolTimingInput {
                thread_id: &self.conversation_id,
                turn_id: &self.turn_id,
                call_id: &self.call_id,
                boundary: codex_extension_api::ToolTimingBoundary::Handler,
                duration: execution_started_at
                    .map(|start| completed_at.duration_since(start))
                    .unwrap_or_default(),
            });
        }
        let duration_ms = |duration: std::time::Duration| u64::try_from(duration.as_millis()).ok();
        let total_duration_ms = duration_ms(completed_at.duration_since(started_at));
        let dispatch_duration_ms = execution_started_at.map_or_else(
            || total_duration_ms,
            |execution_started_at| duration_ms(execution_started_at.duration_since(started_at)),
        );
        let handler_duration_ms = execution_started_at.map_or(Some(0), |execution_started_at| {
            duration_ms(completed_at.duration_since(execution_started_at))
        });

        macro_rules! log_tool_call {
            ($dispatch_duration_ms:expr, $handler_duration_ms:expr, $total_duration_ms:expr) => {
                info!(
                    event.name = "codex.tool_call",
                    trace_id = %codex_otel::current_span_trace_id().unwrap_or_default(),
                    conversation.id = %self.conversation_id,
                    turn_id = %self.turn_id,
                    tool_name = %self.tool_name,
                    call_id = %self.call_id,
                    tool_source = "direct",
                    execution_started = execution_started_at.is_some(),
                    dispatch_duration_ms = $dispatch_duration_ms,
                    handler_duration_ms = $handler_duration_ms,
                    total_duration_ms = $total_duration_ms,
                    "tool call completed"
                );
            };
        }

        match (dispatch_duration_ms, handler_duration_ms, total_duration_ms) {
            (Some(dispatch_duration_ms), Some(handler_duration_ms), Some(total_duration_ms)) => {
                log_tool_call!(dispatch_duration_ms, handler_duration_ms, total_duration_ms);
            }
            _ => {
                log_tool_call!(
                    tracing::field::Empty,
                    tracing::field::Empty,
                    tracing::field::Empty
                );
            }
        }
        handler_duration_ms
    }
}

impl Drop for ToolCallTimingGuard {
    fn drop(&mut self) {
        // Cancellation can drop the outer future before its normal completion path runs.
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::time::Duration;

    use crate::session::step_context::StepContext;
    use crate::tools::context::FunctionToolOutput;
    use crate::tools::context::ToolInvocation;
    use crate::tools::registry::CoreToolRuntime;
    use crate::tools::registry::ToolExecutor;
    use crate::tools::registry::ToolRegistry;
    use crate::tools::router::ToolRouter;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_extension_api::ToolCallOutcome;
    use codex_protocol::models::FunctionCallOutputBody;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::openai_models::ToolMode;
    use pretty_assertions::assert_eq;
    use tokio::sync::Notify;
    use tokio::sync::oneshot;
    use tracing_test::internal::MockWriter;

    #[test]
    fn tool_call_timing_guard_ignores_code_mode_source() {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            let call = ToolCall {
                tool_name: codex_tools::ToolName::plain("test_tool"),
                call_id: "call-1".to_string(),
                payload: ToolPayload::Function {
                    arguments: "{}".to_string(),
                },
                encrypted_function_args: None,
            };
            let direct_guard = ToolCallTimingGuard::capture(
                Instant::now(),
                &"conversation-id",
                "turn-id",
                &call,
                &ToolCallSource::Direct,
                codex_extension_api::empty_extension_registry(),
            );
            assert!(
                direct_guard.is_some(),
                "direct tool calls should create a timing guard"
            );
            drop(direct_guard);

            let code_mode_guard = ToolCallTimingGuard::capture(
                Instant::now(),
                &"conversation-id",
                "turn-id",
                &call,
                &ToolCallSource::CodeMode {
                    cell_id: "cell-1".to_string(),
                    runtime_tool_call_id: "runtime-call-1".to_string(),
                },
                codex_extension_api::empty_extension_registry(),
            );
            assert!(
                code_mode_guard.is_none(),
                "nested code-mode calls should not create overlapping timing events"
            );
        });
    }

    #[test]
    fn output_timing_works_without_logs_and_excludes_dispatch_waiting() {
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            let call = ToolCall {
                tool_name: codex_tools::ToolName::plain("exec"),
                call_id: "call-1".to_string(),
                payload: ToolPayload::Custom {
                    input: "text('ready')".to_string(),
                },
                encrypted_function_args: None,
            };
            let started_at = Instant::now() - Duration::from_secs(/*secs*/ 5);
            let mut timing = ToolCallTimingGuard::capture(
                started_at,
                &"conversation-id",
                "turn-id",
                &call,
                &ToolCallSource::Direct,
                codex_extension_api::empty_extension_registry(),
            )
            .expect("model-visible timing must not depend on INFO logging");
            timing
                .execution_started_at
                .set(started_at + Duration::from_secs(/*secs*/ 4))
                .expect("execution start is unset");
            let handler_ms = timing.finish().expect("completed handler duration");
            assert!(u128::from(handler_ms) <= started_at.elapsed().as_millis() - 4_000);
            assert!(handler_ms >= 1_000);
            assert_eq!(timing.finish(), None, "completion is consumed exactly once");
        });
    }

    #[tokio::test]
    async fn cancellation_before_dispatch_admission_logs_dispatch_only_timing() -> anyhow::Result<()>
    {
        let (session, turn_context) = crate::session::tests::make_session_and_context().await;
        let session = Arc::new(session);
        let turn_context = Arc::new(turn_context);
        let tool_name = codex_tools::ToolName::plain("test_tool");
        let handler = Arc::new(ImmediateHandler {
            tool_name: tool_name.clone(),
        }) as Arc<dyn CoreToolRuntime>;
        let step_context = StepContext::for_test(Arc::clone(&turn_context));
        let router = Arc::new(ToolRouter::from_parts(
            ToolRegistry::from_tools([handler]),
            Vec::new(),
            ToolMode::Direct,
            BTreeMap::new(),
            /*tool_namespaces_info*/ None,
            &[],
        ));
        let step_context = step_context.with_tool_router_for_test(router);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
        let runtime = ToolCallRuntime::new(session, step_context, tracker);
        let execution_gate = Arc::clone(&runtime.parallel_execution);
        let execution_gate_guard = execution_gate
            .try_write_owned()
            .expect("execution gate should be available before dispatch starts");
        let (release_execution_gate_tx, release_execution_gate_rx) = std::sync::mpsc::channel();
        let execution_gate_task = tokio::task::spawn_blocking(move || {
            let _execution_gate_guard = execution_gate_guard;
            release_execution_gate_rx
                .recv()
                .expect("test should release the execution gate");
        });

        let buffer: &'static std::sync::Mutex<Vec<u8>> =
            Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(MockWriter::new(buffer))
            .finish();
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);

        let cancellation_token = CancellationToken::new();
        let call = ToolCall {
            tool_name,
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: "{}".to_string(),
            },
            encrypted_function_args: None,
        };
        let response_task =
            tokio::spawn(runtime.handle_tool_call(call, cancellation_token.clone()));
        cancellation_token.cancel();
        tokio::time::timeout(Duration::from_secs(1), response_task)
            .await
            .expect("timed out waiting for cancelled tool response")
            .expect("cancelled tool response task should join")
            .expect("cancelled tool call should produce a response");

        let logs = String::from_utf8(
            buffer
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        )?;
        let timing_events = logs
            .lines()
            .filter(|line| line.contains("event.name=\"codex.tool_call\""))
            .collect::<Vec<_>>();
        assert_eq!(
            timing_events.len(),
            1,
            "cancelled tool call should emit exactly one timing event; logs:\n{logs}"
        );
        let timing_event = timing_events[0];
        assert!(
            timing_event.contains("execution_started=false"),
            "tool cancelled before admission should not report execution started: {timing_event}"
        );
        assert!(
            timing_event.contains("handler_duration_ms=0"),
            "tool cancelled before admission should report zero handler duration: {timing_event}"
        );
        let duration_field = |name: &str| {
            timing_event.split_whitespace().find_map(|field| {
                field
                    .strip_prefix(&format!("{name}="))
                    .and_then(|value| value.parse::<u64>().ok())
            })
        };
        let dispatch_duration_ms = duration_field("dispatch_duration_ms")
            .expect("timing event should include dispatch_duration_ms");
        let total_duration_ms = duration_field("total_duration_ms")
            .expect("timing event should include total_duration_ms");
        assert_eq!(
            dispatch_duration_ms, total_duration_ms,
            "tool cancelled before admission should attribute all elapsed time to dispatch: {timing_event}"
        );
        release_execution_gate_tx
            .send(())
            .expect("execution gate task should remain available");
        execution_gate_task
            .await
            .expect("execution gate task should join");

        Ok(())
    }

    struct ImmediateHandler {
        tool_name: codex_tools::ToolName,
    }

    impl ToolExecutor<ToolInvocation> for ImmediateHandler {
        fn tool_name(&self) -> codex_tools::ToolName {
            self.tool_name.clone()
        }

        fn spec(&self) -> codex_tools::ToolSpec {
            codex_tools::ToolSpec::Function(codex_tools::ResponsesApiTool {
                name: self.tool_name.name.clone(),
                description: "Immediate test tool.".to_string(),
                strict: false,
                defer_loading: None,
                parameters: codex_tools::JsonSchema::default(),
                output_schema: None,
            })
        }

        fn handle<'a>(&'a self, _invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
        where
            ToolInvocation: 'a,
        {
            Box::pin(async {
                Ok(
                    Box::new(FunctionToolOutput::from_text("ok".to_string(), Some(true)))
                        as Box<dyn crate::tools::context::ToolOutput>,
                )
            })
        }
    }

    impl CoreToolRuntime for ImmediateHandler {}

    struct BlockingFinishContributor {
        records: Arc<std::sync::Mutex<Vec<ToolCallOutcome>>>,
        finish_started: std::sync::Mutex<Option<oneshot::Sender<()>>>,
        allow_finish: Arc<Notify>,
    }

    impl codex_extension_api::ToolLifecycleContributor for BlockingFinishContributor {
        fn on_tool_finish<'a>(
            &'a self,
            input: codex_extension_api::ToolFinishInput<'a>,
        ) -> codex_extension_api::ToolLifecycleFuture<'a> {
            let records = Arc::clone(&self.records);
            let allow_finish = Arc::clone(&self.allow_finish);
            let finish_started = self
                .finish_started
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            let outcome = input.outcome;
            Box::pin(async move {
                if let Some(finish_started) = finish_started {
                    let _ = finish_started.send(());
                }
                allow_finish.notified().await;
                records
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(outcome);
            })
        }
    }

    #[tokio::test]
    async fn cancellation_after_handler_finishes_preserves_completed_lifecycle()
    -> anyhow::Result<()> {
        let (mut session, turn_context) = crate::session::tests::make_session_and_context().await;
        let records = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (finish_started_tx, finish_started_rx) = oneshot::channel();
        let allow_finish = Arc::new(Notify::new());
        let mut builder =
            codex_extension_api::ExtensionRegistryBuilder::<crate::config::Config>::new();
        builder.tool_lifecycle_contributor(Arc::new(BlockingFinishContributor {
            records: Arc::clone(&records),
            finish_started: std::sync::Mutex::new(Some(finish_started_tx)),
            allow_finish: Arc::clone(&allow_finish),
        }));
        session.services.extensions = Arc::new(builder.build());

        let session = Arc::new(session);
        let turn_context = Arc::new(turn_context);
        let tool_name = codex_tools::ToolName::plain("test_tool");
        let handler = Arc::new(ImmediateHandler {
            tool_name: tool_name.clone(),
        }) as Arc<dyn CoreToolRuntime>;
        let step_context = StepContext::for_test(Arc::clone(&turn_context));
        let router = Arc::new(ToolRouter::from_parts(
            ToolRegistry::from_tools([handler]),
            Vec::new(),
            ToolMode::Direct,
            BTreeMap::new(),
            /*tool_namespaces_info*/ None,
            &[],
        ));
        let step_context = step_context.with_tool_router_for_test(router);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
        let runtime = ToolCallRuntime::new(session, step_context, tracker);
        let cancellation_token = CancellationToken::new();
        let call = ToolCall {
            tool_name,
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: "{}".to_string(),
            },
            encrypted_function_args: None,
        };

        let response_task =
            tokio::spawn(runtime.handle_tool_call(call, cancellation_token.clone()));
        tokio::time::timeout(Duration::from_secs(1), finish_started_rx)
            .await
            .expect("timed out waiting for lifecycle notification to start")
            .expect("lifecycle notification should start");
        cancellation_token.cancel();
        tokio::time::sleep(Duration::from_millis(10)).await;
        allow_finish.notify_waiters();

        let response = tokio::time::timeout(Duration::from_secs(1), response_task)
            .await
            .expect("timed out waiting for tool response")
            .expect("tool response task should join")?;
        let expected_response = ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text("ok".to_string()),
                success: Some(true),
            },
        };
        assert_eq!(
            ResponseItemEnvelope::new(expected_response.into()),
            response
        );

        let actual = records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain(..)
            .collect::<Vec<_>>();
        assert_eq!(vec![ToolCallOutcome::Completed { success: true }], actual);

        Ok(())
    }
}

#[cfg(test)]
#[path = "parallel_observation_tests.rs"]
mod observation_tests;
