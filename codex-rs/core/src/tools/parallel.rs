use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;
use tokio::sync::Semaphore;
use tokio_util::either::Either;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;
use tracing::Instrument;
use tracing::instrument;
use tracing::trace_span;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::function_tool::FunctionCallError;
use crate::protocol::EventMsg;
use crate::protocol::SubAgentToolCallEndEvent;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::SPAWN_SUBAGENT_TOOL_NAME;
use crate::tools::handlers::parse_spawn_subagent_invocation;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolRouter;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;

#[derive(Clone)]
pub(crate) struct ToolCallRuntime {
    router: Arc<ToolRouter>,
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
    parallel_execution: Arc<RwLock<()>>,
    subagent_parallel_limit: Arc<Semaphore>,
}

impl ToolCallRuntime {
    pub(crate) fn new(
        router: Arc<ToolRouter>,
        session: Arc<Session>,
        turn_context: Arc<TurnContext>,
        tracker: SharedTurnDiffTracker,
    ) -> Self {
        Self {
            router,
            session,
            turn_context,
            tracker,
            parallel_execution: Arc::new(RwLock::new(())),
            subagent_parallel_limit: Arc::new(Semaphore::new(3)),
        }
    }

    #[instrument(level = "trace", skip_all, fields(call = ?call))]
    pub(crate) fn handle_tool_call(
        self,
        call: ToolCall,
        cancellation_token: CancellationToken,
    ) -> impl std::future::Future<Output = Result<ResponseInputItem, CodexErr>> {
        let supports_parallel = self.router.tool_supports_parallel(&call.tool_name);

        let router = Arc::clone(&self.router);
        let session = Arc::clone(&self.session);
        let turn = Arc::clone(&self.turn_context);
        let tracker = Arc::clone(&self.tracker);
        let lock = Arc::clone(&self.parallel_execution);
        let subagent_parallel_limit = Arc::clone(&self.subagent_parallel_limit);
        let started = Instant::now();
        let session_for_cancel = Arc::clone(&session);
        let turn_for_cancel = Arc::clone(&turn);
        let session_for_dispatch = Arc::clone(&session);
        let turn_for_dispatch = Arc::clone(&turn);

        let dispatch_span = trace_span!(
            "dispatch_tool_call",
            otel.name = call.tool_name.as_str(),
            tool_name = call.tool_name.as_str(),
            call_id = call.call_id.as_str(),
            aborted = false,
        );

        let handle: AbortOnDropHandle<Result<ResponseInputItem, FunctionCallError>> =
            AbortOnDropHandle::new(tokio::spawn(async move {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        let elapsed = started.elapsed();
                        let secs = elapsed.as_secs_f32().max(0.1);
                        dispatch_span.record("aborted", true);
                        if call.tool_name == SPAWN_SUBAGENT_TOOL_NAME
                            && let ToolPayload::Function { arguments } = &call.payload
                            && let Ok(invocation) = parse_spawn_subagent_invocation(arguments)
                        {
                            let message = Self::abort_message(&call, secs);
                            session_for_cancel
                                .send_event(
                                    turn_for_cancel.as_ref(),
                                    EventMsg::SubAgentToolCallEnd(SubAgentToolCallEndEvent {
                                        call_id: call.call_id.clone(),
                                        invocation,
                                        duration: elapsed,
                                        tokens: None,
                                        result: Err(message),
                                    }),
                                )
                                .await;
                        }
                        Ok(Self::aborted_response(&call, secs))
                    },
                    res = async {
                        let _subagent_permit = if call.tool_name == SPAWN_SUBAGENT_TOOL_NAME {
                            Some(
                                Arc::clone(&subagent_parallel_limit)
                                    .acquire_owned()
                                    .await
                                    .map_err(|_| {
                                        FunctionCallError::Fatal(
                                            "subagent semaphore unexpectedly closed".to_string(),
                                        )
                                    })?,
                            )
                        } else {
                            None
                        };

                        let _guard = if supports_parallel {
                            Either::Left(lock.read().await)
                        } else {
                            Either::Right(lock.write().await)
                        };

                        router
                            .dispatch_tool_call(
                                session_for_dispatch,
                                turn_for_dispatch,
                                tracker,
                                call.clone(),
                            )
                            .instrument(dispatch_span.clone())
                            .await
                    } => res,
                }
            }));

        async move {
            match handle.await {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(FunctionCallError::Fatal(message))) => Err(CodexErr::Fatal(message)),
                Ok(Err(other)) => Err(CodexErr::Fatal(other.to_string())),
                Err(err) => Err(CodexErr::Fatal(format!(
                    "tool task failed to receive: {err:?}"
                ))),
            }
        }
        .in_current_span()
    }
}

impl ToolCallRuntime {
    fn aborted_response(call: &ToolCall, secs: f32) -> ResponseInputItem {
        match &call.payload {
            ToolPayload::Custom { .. } => ResponseInputItem::CustomToolCallOutput {
                call_id: call.call_id.clone(),
                output: Self::abort_message(call, secs),
            },
            ToolPayload::Mcp { .. } => ResponseInputItem::McpToolCallOutput {
                call_id: call.call_id.clone(),
                result: Err(Self::abort_message(call, secs)),
            },
            _ => ResponseInputItem::FunctionCallOutput {
                call_id: call.call_id.clone(),
                output: FunctionCallOutputPayload {
                    content: Self::abort_message(call, secs),
                    ..Default::default()
                },
            },
        }
    }

    fn abort_message(call: &ToolCall, secs: f32) -> String {
        match call.tool_name.as_str() {
            "shell" | "container.exec" | "local_shell" | "shell_command" | "unified_exec" => {
                format!("Wall time: {secs:.1} seconds\naborted by user")
            }
            _ => format!("aborted by user after {secs:.1}s"),
        }
    }
}
