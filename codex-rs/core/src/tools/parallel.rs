use std::sync::Arc;
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
use crate::function_tool::OrCancelToolExt;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::tools::call_trace;
use crate::tools::code_mode::PUBLIC_TOOL_NAME;
use crate::tools::code_mode::WAIT_TOOL_NAME;
use crate::tools::context::AbortedToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolCallState;
use crate::tools::context::ToolPayload;
use crate::tools::lifecycle::notify_tool_aborted;
use crate::tools::registry::AnyToolResult;
use crate::tools::registry::RegisteredTool;
use crate::tools::registry::ToolArgumentDiffConsumer;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolCallSource;
use codex_analytics::IncompleteReason;
use codex_analytics::IncompleteReason::CodeModeUnsupported;
use codex_analytics::IncompleteReason::Unclassified;
use codex_analytics::IncompleteReason::UnsupportedOperation;
use codex_direct_tool_timing::ToolAdmission;
use codex_direct_tool_timing::ToolBatchTiming;
use codex_direct_tool_timing::ToolGateTiming;
use codex_extension_api::ToolCallOutcome;
use codex_history::ResponseItemEnvelope;
use codex_protocol::error::CodexErr;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ToolResultMetadata;

struct ToolCallTimingGuard {
    started_at: Option<Instant>,
    call_state: Arc<ToolCallState>,
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
    pub(crate) step_context: Arc<StepContext>,
    tracker: SharedTurnDiffTracker,
    parallel_execution: Arc<RwLock<()>>,
    timing: Option<DirectToolTiming>,
}

#[derive(Clone, Default)]
struct DirectToolTiming {
    gate: ToolGateTiming,
    batch: ToolBatchTiming,
}

enum ToolDispatchError {
    Call(FunctionCallError),
    Aborted(ToolAbort),
}

#[derive(Clone, Copy)]
enum ToolAbort {
    BeforeDispatch,
    User(f32),
}

impl From<FunctionCallError> for ToolDispatchError {
    fn from(error: FunctionCallError) -> Self {
        Self::Call(error)
    }
}

// Direct spawn and wait calls use this turn's tool time; child work stays on its own turn.
// CodeMode and follow-up agent operations still lack runtime capture.
fn direct_path_incomplete_reasons(
    tool_name: &codex_tools::ToolName,
    registered_tool: Option<&RegisteredTool>,
    source: &ToolCallSource,
) -> &'static [IncompleteReason] {
    // The registry resolves the call by this name, normalizing only the default namespace.
    // Keep legacy name-based invalidation even when the name resolves to an external tool.
    let reasons: &[_] = match tool_name.name.as_str() {
        PUBLIC_TOOL_NAME | WAIT_TOOL_NAME if tool_name.is_default_namespace() => {
            &[CodeModeUnsupported]
        }
        "send_input" | "close_agent" | "resume_agent" | "list_agents" | "followup_task"
        | "interrupt_agent" | "send_message" => &[UnsupportedOperation],
        _ => &[],
    };
    let trusted = registered_tool.is_some_and(|tool| tool.is_trusted);
    match (source, reasons, trusted) {
        (ToolCallSource::CodeMode { .. }, [UnsupportedOperation], true) => {
            &[CodeModeUnsupported, UnsupportedOperation]
        }
        (ToolCallSource::CodeMode { .. }, [_], false) => &[CodeModeUnsupported, Unclassified],
        (ToolCallSource::CodeMode { .. }, _, _) => &[CodeModeUnsupported],
        (_, [], _) => &[],
        (_, reasons, true) => reasons,
        (_, _, false) => &[Unclassified],
    }
}

impl ToolCallRuntime {
    pub(crate) fn new(
        session: Arc<Session>,
        step_context: Arc<StepContext>,
        tracker: SharedTurnDiffTracker,
    ) -> Self {
        let timing = step_context
            .turn
            .config
            .features
            .enabled(codex_features::Feature::CriticalPathTiming)
            .then(DirectToolTiming::default);
        Self {
            session,
            step_context,
            tracker,
            parallel_execution: Arc::new(RwLock::new(())),
            timing,
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
        let file_backed_tool_audio = self
            .step_context
            .turn
            .config
            .features
            .enabled(codex_features::Feature::FileBackedToolAudio);
        let recorder = self.session.services.executed_tool_calls.clone();
        let item_id_generator = Arc::clone(&self.session.item_id_generator);
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
                    result.into_response(file_backed_tool_audio)
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
            recorder.attach_direct_call_to_output(
                &mut response.item,
                recorded_call,
                &item_id_generator,
            );
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
        if let Err(err) =
            call_state
                .invocation
                .reserve(&self.session, &step_context.turn, &call.call_id)
        {
            return Either::Left(std::future::ready(Err(err)));
        }
        let message_admission =
            super::user_messaging::admit_code_mode_send(&self.session, &source, &call.tool_name);
        self.session
            .services
            .executed_tool_calls
            .record_tool_call(&call, &source, &step_context);
        let router = &step_context.tool_router;
        let supports_parallel = router.tool_supports_parallel(&call);
        let registered_tool = router.registered_tool(&call.tool_name);
        let tool_runtime = registered_tool.map(|tool| Arc::clone(&tool.runtime));
        let finishes_on_cancellation = tool_runtime
            .as_ref()
            .is_some_and(|runtime| runtime.finishes_on_cancellation());
        let router = Arc::clone(router);
        let session = Arc::clone(&self.session);
        let turn = Arc::clone(&step_context.turn);
        let tracker = Arc::clone(&self.tracker);
        let lock = Arc::clone(&self.parallel_execution);
        let invocation_cancellation_token = cancellation_token.clone();
        let started = Instant::now();
        if turn
            .config
            .features
            .enabled(codex_features::Feature::CriticalPathTiming)
        {
            let reasons = direct_path_incomplete_reasons(&call.tool_name, registered_tool, &source);
            if !reasons.is_empty() {
                turn.turn_timing_state.invalidate_critical_path(reasons);
            }
        }
        let work_timing = self.timing.and_then(|timing| match &source {
            ToolCallSource::Direct | ToolCallSource::DirectPlaintextMessage => {
                let work = turn
                    .turn_timing_state
                    .direct_tool_timings
                    .start_turn_direct_call(
                        &call.call_id,
                        &turn.sub_id,
                        timing.batch,
                        started,
                        turn.turn_timing_state.snapshot_path(started),
                    );
                Some((timing.gate, work))
            }
            ToolCallSource::CodeMode { .. } => None,
        });
        let mut tool_call_timing_guard = ToolCallTimingGuard::capture(
            started,
            &session.thread_id,
            &turn.sub_id,
            &call,
            &source,
            Arc::clone(&call_state),
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
        let record_result_ready =
            move |turn_id: &str, tool_name: &codex_tools::ToolName, call_id: &str| {
                call_trace::result_ready(
                    thread_id,
                    turn_id,
                    tool_name,
                    call_id,
                    trace_source,
                    started.elapsed(),
                );
            };

        // Code-mode callbacks can resume outside the turn's local span ancestry.
        let dispatch_span = trace_span!(
            "dispatch_tool_call_with_code_mode_result",
            otel.name = %call.tool_name,
            tool_name = %call.tool_name,
            thread.id = %session.thread_id,
            conversation.id = %session.thread_id,
            turn.id = %turn.sub_id,
            turn_id = %turn.sub_id,
            tool_source = trace_source.as_str(),
            call_id = call.call_id.as_str(),
            codex.tool.phase = "dispatch",
            aborted = false,
        );
        let abort_dispatch_span = dispatch_span.clone();

        let mut dispatch_handle = AbortOnDropHandle::new(tokio::spawn(
            async move {
                let _message_admission = message_admission?;
                // A response replayed after settlement can already have exhausted the budget.
                // Record its denial without waiting for a handler that will never start.
                if session.work_budget().is_exhausted() {
                    tracing::Span::current().record("aborted", true);
                    record_result_ready(&turn.sub_id, &dispatch_tool_name, &dispatch_call_id);
                    return Err(ToolDispatchError::Aborted(ToolAbort::BeforeDispatch));
                }
                // Readiness and admission remain cancellable even when the handler must finish.
                let (guard, admission_started) = async {
                    if let Some(tool_runtime) = tool_runtime
                        && let Some(readiness) = tool_runtime.wait_until_ready(&session)
                    {
                        readiness
                            .instrument(trace_span!("tool_dispatch.wait_until_ready"))
                            .await;
                    }

                    let admission_started = Instant::now();
                    let guard = if supports_parallel {
                        Either::Left(
                            lock.read()
                                .instrument(trace_span!("tool_dispatch.parallel_lock"))
                                .await,
                        )
                    } else {
                        Either::Right(
                            lock.write()
                                .instrument(trace_span!("tool_dispatch.exclusive_lock"))
                                .await,
                        )
                    };
                    (guard, admission_started)
                }
                .or_cancel_tool(&invocation_cancellation_token)
                .await?;
                // A peer may have exhausted the account while this call waited for its slot.
                if session
                    .tool_start_denial()
                    .map_err(|error| FunctionCallError::Fatal(error.to_string()))?
                    .is_some()
                {
                    tracing::Span::current().record("aborted", true);
                    record_result_ready(&turn.sub_id, &dispatch_tool_name, &dispatch_call_id);
                    return Err(ToolDispatchError::Aborted(ToolAbort::BeforeDispatch));
                }
                // Publish the completed path before releasing the admission lock,
                // including when cancellation drops the handler future.
                let _admitted_timing = work_timing.map(|(gate, work)| {
                    gate.admit(
                        work,
                        if supports_parallel {
                            ToolAdmission::Parallel
                        } else {
                            ToolAdmission::Exclusive
                        },
                        admission_started,
                    )
                });
                let result = router
                    .dispatch_tool_call_with_state(
                        session,
                        step_context,
                        invocation_cancellation_token,
                        tracker,
                        dispatch_call,
                        source,
                        Arc::clone(&dispatch_call_state),
                    )
                    .await;
                drop(_admitted_timing);
                drop(guard);
                // The sampling loop collects results in order only after its stream ends.
                // Record readiness here, before either caller encodes or collects the result.
                // A fatal error still propagates to the caller instead of producing a tool
                // result; unlike a normal tool failure, it has no readiness event.
                if !matches!(&result, Err(FunctionCallError::Fatal(_))) {
                    record_result_ready(&turn.sub_id, &dispatch_tool_name, &dispatch_call_id);
                }
                // Finished calls release their IDs even if their results have not been collected.
                // Otherwise the outer task may still need the ID for its abort callback.
                if let Some(outcome) = dispatch_call_state.terminal_outcome.get() {
                    if *outcome == ToolCallOutcome::Aborted {
                        tracing::Span::current().record("aborted", true);
                    }
                    drop(dispatch_call_state.invocation.take());
                }
                result.map_err(ToolDispatchError::Call)
            }
            .instrument(dispatch_span.clone().or_current()),
        ));

        let completion = async move {
            let mut tool_call_timing_guard = tool_call_timing_guard;
            let join_error = |err| {
                drop(call_state.invocation.take());
                Self::tool_task_join_error(err)
            };
            let mut result = tokio::select! {
                res = &mut dispatch_handle => res.map_err(join_error)?,
                _ = cancellation_token.cancelled() => {
                    // Dynamic handlers observe the token and publish their terminal item themselves.
                    if finishes_on_cancellation
                        || call_state.terminal_outcome.get().is_some()
                        || dispatch_handle.is_finished()
                    {
                        dispatch_handle.await.map_err(join_error)?
                    } else {
                        let secs = started.elapsed().as_secs_f32().max(0.1);
                        abort_dispatch_span.record("aborted", true);
                        dispatch_handle.abort();
                        match dispatch_handle.await {
                            Ok(result) => result,
                            Err(err) if err.is_cancelled() => {
                                record_result_ready(&abort_turn.sub_id, &call.tool_name, &call.call_id);
                                Err(ToolDispatchError::Aborted(ToolAbort::User(secs)))
                            }
                            Err(err) => Err(Self::tool_task_join_error(err).into()),
                        }
                    }
                },
            };
            let _reservation = call_state.invocation.take();
            // Cancellation outside the handler can return before the registry reports an outcome.
            if cancellation_token.is_cancelled()
                && !matches!(result, Err(ToolDispatchError::Aborted(_)))
                && call_state.terminal_outcome.get().is_none()
            {
                result = Err(ToolDispatchError::Aborted(ToolAbort::User(
                    started.elapsed().as_secs_f32().max(0.1),
                )));
            }
            let mut result = match result {
                Ok(output) => Ok(output),
                Err(ToolDispatchError::Call(error)) => Err(error),
                Err(ToolDispatchError::Aborted(stop)) => {
                    if call_state.terminal_outcome.set(ToolCallOutcome::Aborted).is_ok() {
                        notify_tool_aborted(
                            &abort_session,
                            &abort_turn,
                            &call.call_id,
                            &call.tool_name,
                            abort_source,
                        )
                        .await;
                    }
                    Ok(Self::stopped_response(&call, stop))
                }
            };
            // Use one completion measurement for logging and response formatting.
            // Completion includes output processing after the handler returns.
            if let Some(timing) = tool_call_timing_guard.as_mut()
                && let Some(handler_duration_ms) = timing.finish()
                && let Ok(result) = &mut result
            {
                result.result.set_handler_duration_ms(handler_duration_ms);
            }
            result
        }
        .instrument(dispatch_span.or_current());
        Either::Right(completion)
    }
}

impl ToolCallRuntime {
    pub(crate) fn batch_timing(&self) -> Option<ToolBatchTiming> {
        self.timing.as_ref().map(|timing| timing.batch.clone())
    }

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

    pub(crate) fn aborted_before_dispatch(call: &ToolCall) -> AnyToolResult {
        Self::stopped_response(call, ToolAbort::BeforeDispatch)
    }

    fn stopped_response(call: &ToolCall, stop: ToolAbort) -> AnyToolResult {
        let message = match stop {
            ToolAbort::BeforeDispatch => "aborted before dispatch".to_string(),
            ToolAbort::User(secs)
                if call.tool_name.is_default_namespace()
                    && call.tool_name.name == "exec_command" =>
            {
                format!("Wall time: {secs:.1} seconds\naborted by user")
            }
            ToolAbort::User(secs) => format!("aborted by user after {secs:.1}s"),
        };
        AnyToolResult {
            call_id: call.call_id.clone(),
            payload: call.payload.clone(),
            result: Box::new(AbortedToolOutput { message }),
            post_tool_use_payload: None,
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
        call_state: Arc<ToolCallState>,
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
            call_state,
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
            .call_state
            .handler_started_at
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
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use crate::session::step_context::StepContext;
    use crate::tools::context::FunctionToolOutput;
    use crate::tools::context::ToolInvocation;
    use crate::tools::registry::CoreToolRuntime;
    use crate::tools::registry::ToolExecutor;
    use crate::tools::registry::ToolRegistry;
    use crate::tools::router::ToolRouter;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_protocol::models::FunctionCallOutputBody;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::openai_models::ToolMode;
    use codex_tools::ToolName;
    use pretty_assertions::assert_eq;
    use tokio::sync::Notify;
    use tracing_test::internal::MockWriter;

    #[test]
    fn direct_path_reasons_follow_namespace_resolution() {
        let nested = ToolCallSource::CodeMode {
            cell_id: "cell".into(),
            runtime_tool_call_id: "call".into(),
        };
        let code: &[IncompleteReason] = &[CodeModeUnsupported];
        let mixed: &[IncompleteReason] = &[CodeModeUnsupported, UnsupportedOperation];
        let mixed_unknown: &[IncompleteReason] = &[CodeModeUnsupported, Unclassified];
        let namespaced_agent = ToolName::namespaced("custom_collaboration", "send_message");
        let wait_agent = ToolName::namespaced("custom_collaboration", "wait_agent");
        let external_wait = ToolName::plain(WAIT_TOOL_NAME).with_default_namespace();
        let mcp_message = ToolName::namespaced("mcp__chat", "send_message");
        let unknown = &[Unclassified][..];
        for (name, builtin, direct, indirect) in [
            (ToolName::plain(PUBLIC_TOOL_NAME), true, code, code),
            (ToolName::plain(WAIT_TOOL_NAME), true, code, code),
            (external_wait, false, unknown, mixed_unknown),
            (wait_agent, true, &[], code),
            (namespaced_agent, true, &[UnsupportedOperation], mixed),
            (mcp_message, false, unknown, mixed_unknown),
            (ToolName::namespaced("mcp", "wait"), false, &[], code),
        ] {
            let mut registry = ToolRegistry::default();
            let handler = ImmediateHandler {
                tool_name: name.clone(),
                supports_parallel: false,
                calls: AtomicUsize::default(),
            };
            if builtin {
                registry.add(handler);
            } else {
                assert!(registry.register_external(Arc::new(handler)));
            }
            let registered = registry.registered_tool(&name).unwrap();
            let reasons = |source| direct_path_incomplete_reasons(&name, Some(registered), source);
            assert_eq!(
                (reasons(&ToolCallSource::Direct), reasons(&nested)),
                (direct, indirect),
                "{name}"
            );
        }
    }

    #[test]
    fn tool_call_timing_guard_ignores_code_mode_source() {
        let buffer: &'static std::sync::Mutex<Vec<u8>> =
            Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(MockWriter::new(buffer))
            .finish();
        // Keep callsite interest independent of untraced parallel test threads.
        let _untraced = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
        tracing::subscriber::with_default(subscriber, || {
            let call = ToolCall {
                tool_name: codex_tools::ToolName::plain("test_tool"),
                call_id: "call-1".to_string(),
                payload: ToolPayload::Function {
                    arguments: "{}".to_string(),
                },
                encrypted_function_args: None,
                native_call: None,
            };
            let direct_guard = ToolCallTimingGuard::capture(
                Instant::now(),
                &"conversation-id",
                "turn-id",
                &call,
                &ToolCallSource::Direct,
                Arc::new(ToolCallState::default()),
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
                Arc::new(ToolCallState::default()),
                codex_extension_api::empty_extension_registry(),
            );
            assert!(
                code_mode_guard.is_none(),
                "nested code-mode calls should not create overlapping timing events"
            );
        });

        let logs = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert_eq!(
            logs.lines()
                .filter(|line| line.contains("event.name=\"codex.tool_call\""))
                .count(),
            1,
            "only direct tool calls should emit a timing event; logs:\n{logs}"
        );
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
                native_call: None,
            };
            let started_at = Instant::now() - Duration::from_secs(/*secs*/ 5);
            let mut timing = ToolCallTimingGuard::capture(
                started_at,
                &"conversation-id",
                "turn-id",
                &call,
                &ToolCallSource::Direct,
                Arc::new(ToolCallState::default()),
                codex_extension_api::empty_extension_registry(),
            )
            .expect("model-visible timing must not depend on INFO logging");
            timing
                .call_state
                .handler_started_at
                .set(started_at + Duration::from_secs(/*secs*/ 4))
                .expect("execution start is unset");
            let handler_ms = timing.finish().expect("completed handler duration");
            assert!(u128::from(handler_ms) <= started_at.elapsed().as_millis() - 4_000);
            assert!(handler_ms >= 1_000);
            assert_eq!(timing.finish(), None, "completion is consumed exactly once");
        });
    }

    #[test_case::test_case(false, false; "ordinary_capture_disabled")]
    #[test_case::test_case(false, true; "ordinary_capture_enabled")]
    #[test_case::test_case(true, false; "dynamic_capture_disabled")]
    #[test_case::test_case(true, true; "dynamic_capture_enabled")]
    #[tokio::test]
    async fn cancellation_before_dispatch_admission_logs_dispatch_only_timing(
        dynamic: bool,
        critical_path_timing: bool,
    ) -> anyhow::Result<()> {
        // Create and drop instrumented session resources under the same subscriber.
        // A prior traced test may have installed a different global subscriber.
        let buffer: &'static std::sync::Mutex<Vec<u8>> =
            Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(MockWriter::new(buffer))
            .finish();
        // An untraced parallel worker can register this callsite before the test does.
        let _untraced = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);

        let (session, mut turn_context) = crate::session::tests::make_session_and_context().await;
        Arc::make_mut(&mut turn_context.config)
            .features
            .enable(codex_features::Feature::RuntimeMetrics)
            .unwrap();
        Arc::make_mut(&mut turn_context.config)
            .features
            .set_enabled(
                codex_features::Feature::CriticalPathTiming,
                critical_path_timing,
            )
            .unwrap();
        if critical_path_timing {
            turn_context.turn_timing_state.enable_critical_path();
        }
        turn_context
            .turn_timing_state
            .mark_turn_started(Instant::now())
            .await;
        let session = Arc::new(session);
        let turn_context = Arc::new(turn_context);
        let tool_name = codex_tools::ToolName::plain("test_tool");
        let handler: Arc<dyn CoreToolRuntime> = if dynamic {
            Arc::new(
                crate::tools::handlers::DynamicToolHandler::new(
                    &codex_protocol::dynamic_tools::DynamicToolFunctionSpec {
                        name: tool_name.name.clone(),
                        description: "test".to_string(),
                        input_schema: serde_json::json!({"type": "object"}),
                        defer_loading: false,
                    },
                )
                .expect("dynamic handler"),
            )
        } else {
            Arc::new(ImmediateHandler {
                tool_name: tool_name.clone(),
                supports_parallel: false,
                calls: AtomicUsize::default(),
            })
        };
        let step_context = StepContext::for_test(Arc::clone(&turn_context));
        let router = Arc::new(ToolRouter::from_parts(
            ToolRegistry::from_tools([handler]),
            Vec::new(),
            ToolMode::Direct,
            BTreeMap::new(),
            /*exec_definitions*/ None,
            /*tool_namespaces_info*/ None,
            &[],
        ));
        let step_context = step_context.with_tool_router_for_test(router);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
        let runtime = ToolCallRuntime::new(session, step_context, tracker);
        let batch = runtime.batch_timing();
        assert_eq!(batch.is_some(), critical_path_timing);
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

        let cancellation_token = CancellationToken::new();
        let call = ToolCall {
            tool_name,
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: "{}".to_string(),
            },
            encrypted_function_args: None,
            native_call: None,
        };
        let response_task =
            tokio::spawn(runtime.handle_tool_call(call, cancellation_token.clone()));
        cancellation_token.cancel();
        tokio::time::timeout(Duration::from_secs(1), response_task)
            .await
            .expect("timed out waiting for cancelled tool response")
            .expect("cancelled tool response task should join")
            .expect("cancelled tool call should produce a response");

        assert_eq!(
            batch.and_then(|batch| batch.completed()).is_some(),
            critical_path_timing,
            "capture requires its own gate and must finish before cancellation returns"
        );
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
        supports_parallel: bool,
        calls: AtomicUsize,
    }

    impl ToolExecutor<ToolInvocation> for ImmediateHandler {
        fn supports_parallel_tool_calls(&self) -> bool {
            self.supports_parallel
        }

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
            self.calls.fetch_add(1, Ordering::Relaxed);
            Box::pin(async {
                Ok(
                    Box::new(FunctionToolOutput::from_text("ok".to_string(), Some(true)))
                        as Box<dyn crate::tools::context::ToolOutput>,
                )
            })
        }
    }

    impl CoreToolRuntime for ImmediateHandler {}

    #[derive(Default)]
    struct PausedToolLifecycle {
        finishes: std::sync::Mutex<Vec<(String, ToolCallOutcome)>>,
        at_start: Notify,
        release_start: Notify,
        at_finish: Notify,
        release_finish: Notify,
    }

    impl codex_extension_api::ToolLifecycleContributor for PausedToolLifecycle {
        fn on_tool_start<'a>(
            &'a self,
            input: codex_extension_api::ToolStartInput<'a>,
        ) -> codex_extension_api::ToolLifecycleFuture<'a> {
            Box::pin(async move {
                if input.call_id == "held-at-start" {
                    self.at_start.notify_one();
                    self.release_start.notified().await;
                }
            })
        }

        fn on_tool_finish<'a>(
            &'a self,
            input: codex_extension_api::ToolFinishInput<'a>,
        ) -> codex_extension_api::ToolLifecycleFuture<'a> {
            Box::pin(async move {
                self.finishes
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push((input.call_id.to_owned(), input.outcome));
                if input.call_id == "call-1" {
                    self.at_finish.notify_one();
                    self.release_finish.notified().await;
                }
            })
        }
    }

    #[test_case::test_case(false; "posted cost checked after the slot opens")]
    #[tokio::test]
    async fn completed_and_budget_stopped_tools_report_their_own_lifecycle(
        weighted: bool,
    ) -> anyhow::Result<()> {
        let (mut session, turn_context) = crate::session::tests::make_session_and_context().await;
        session
            .work_budget()
            .configure(crate::config::RolloutBudgetConfig {
                enforce_limit: true,
                limit_budget: 1.0,
                reminder_at_context_start: weighted,
                reminder_at_remaining_tokens: Vec::new(),
                sampling_token_weight: if weighted { f64::MAX } else { 1.0 },
                prefill_token_weight: 1.0,
            });
        let lifecycle = Arc::new(PausedToolLifecycle::default());
        let mut builder =
            codex_extension_api::ExtensionRegistryBuilder::<crate::config::Config>::new();
        builder.tool_lifecycle_contributor(lifecycle.clone());
        session.services.extensions = Arc::new(builder.build());

        let session = Arc::new(session);
        let turn_context = Arc::new(turn_context);
        let tool_name = codex_tools::ToolName::plain("parallel_tool");
        let queued_name = codex_tools::ToolName::plain("exclusive_tool");
        let parallel_handler = Arc::new(ImmediateHandler {
            tool_name: tool_name.clone(),
            supports_parallel: true,
            calls: AtomicUsize::default(),
        });
        let queued_handler = Arc::new(ImmediateHandler {
            tool_name: queued_name.clone(),
            supports_parallel: false,
            calls: AtomicUsize::default(),
        });
        let handlers: [Arc<dyn CoreToolRuntime>; 2] =
            [parallel_handler.clone(), queued_handler.clone()];
        let step_context = StepContext::for_test(Arc::clone(&turn_context));
        let router = Arc::new(ToolRouter::from_parts(
            ToolRegistry::from_tools(handlers),
            Vec::new(),
            ToolMode::Direct,
            BTreeMap::new(),
            /*exec_definitions*/ None,
            /*tool_namespaces_info*/ None,
            &[],
        ));
        let step_context = step_context.with_tool_router_for_test(router);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
        let runtime = ToolCallRuntime::new(Arc::clone(&session), step_context, tracker);
        let queued_runtime = runtime.clone();
        let cancellation_token = CancellationToken::new();
        let call = ToolCall {
            tool_name,
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: "{}".to_string(),
            },
            encrypted_function_args: None,
            native_call: None,
        };

        let queued_call = ToolCall {
            tool_name: queued_name,
            call_id: "call-2".to_string(),
            ..call.clone()
        };
        let held_call = ToolCall {
            call_id: "held-at-start".to_string(),
            ..call.clone()
        };
        let response = runtime
            .clone()
            .handle_tool_call(call, cancellation_token.clone());
        tokio::pin!(response);
        tokio::time::timeout(Duration::from_secs(1), lifecycle.at_finish.notified()).await?;

        // This call has a shared slot, but its start observer hasn't released the handler.
        let held = runtime.handle_tool_call(held_call, CancellationToken::new());
        tokio::time::timeout(Duration::from_secs(1), lifecycle.at_start.notified()).await?;

        // The first call keeps its shared slot until its finish callback returns. New
        // readers stop getting slots only once the exclusive call has actually queued.
        let gate = Arc::clone(&queued_runtime.parallel_execution);
        assert!(gate.try_read().is_ok());
        let expected_abort = ResponseInputItem::FunctionCallOutput {
            call_id: queued_call.call_id.clone(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text("aborted before dispatch".to_string()),
                success: None,
            },
        };
        let queued = queued_runtime.handle_tool_call(queued_call, CancellationToken::new());
        tokio::time::timeout(Duration::from_secs(1), async {
            while gate.try_read().is_ok() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the tool should be queued behind the held slot");
        let usage = codex_protocol::protocol::TokenUsage {
            output_tokens: 2,
            ..Default::default()
        };
        let bill = session
            .record_rollout_budget_usage("peer-turn", &usage.into())
            .await;
        bill?;
        assert!(session.work_budget().is_exhausted());
        cancellation_token.cancel();
        // Let the caller observe cancellation while the already-finished handler is held.
        assert!(futures::poll!(response.as_mut()).is_pending());
        // Both waiting handlers must see the posted stop themselves.
        lifecycle.release_start.notify_one();
        let held_reply = tokio::time::timeout(Duration::from_secs(1), held).await??;
        let expected_denial = "rollout budget exhausted";
        assert_eq!(
            held_reply.item,
            ResponseInputItem::FunctionCallOutput {
                call_id: "held-at-start".into(),
                output: FunctionCallOutputPayload::from_text(format!(
                    "aborted before handler start: {expected_denial}"
                ),),
            }
            .into()
        );
        lifecycle.release_finish.notify_one();
        assert_eq!(
            ResponseItemEnvelope::new(expected_abort.into()),
            tokio::time::timeout(Duration::from_secs(1), queued).await??
        );

        let response = tokio::time::timeout(Duration::from_secs(1), response).await??;
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
        assert_eq!(parallel_handler.calls.load(Ordering::Relaxed), 1);
        assert_eq!(queued_handler.calls.load(Ordering::Relaxed), 0);

        let actual = lifecycle
            .finishes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain(..)
            .collect::<Vec<_>>();
        assert_eq!(
            vec![
                (
                    "call-1".to_string(),
                    ToolCallOutcome::Completed { success: true }
                ),
                ("held-at-start".to_string(), ToolCallOutcome::Aborted),
                ("call-2".to_string(), ToolCallOutcome::Aborted),
            ],
            actual
        );

        Ok(())
    }
}

#[cfg(test)]
#[path = "parallel_observation_tests.rs"]
mod observation_tests;

#[cfg(test)]
#[path = "parallel_trace_tests.rs"]
mod trace_tests;
