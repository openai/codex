use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::OnceCell;
use tokio::sync::RwLock;
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
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolPayload;
use crate::tools::mention_rewrite::TurnMentionRewriteData;
use crate::tools::mention_rewrite::load_turn_mention_rewrite_data;
use crate::tools::mention_rewrite::mention_rewrite_context_for_read_paths;
use crate::tools::mention_rewrite::read_paths_for_tool_call;
use crate::tools::mention_rewrite::rewrite_tool_response_mentions;
use crate::tools::mention_rewrite::tool_output_contains_mention_prefix;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolRouter;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;

#[derive(Clone)]
pub(crate) struct ToolCallRuntime {
    router: Arc<ToolRouter>,
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
    parallel_execution: Arc<RwLock<()>>,
    // Lazily populated once per turn and reused across tool calls.
    mention_rewrite_data: Arc<OnceCell<Option<TurnMentionRewriteData>>>,
}

fn should_attempt_mention_rewrite(read_paths: &[PathBuf], response: &ResponseInputItem) -> bool {
    !read_paths.is_empty() && tool_output_contains_mention_prefix(response)
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
            mention_rewrite_data: Arc::new(OnceCell::new()),
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
        let mention_rewrite_data = Arc::clone(&self.mention_rewrite_data);
        let started = Instant::now();

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
                        let secs = started.elapsed().as_secs_f32().max(0.1);
                        dispatch_span.record("aborted", true);
                        Ok(Self::aborted_response(&call, secs))
                    },
                    res = async {
                        let read_paths = read_paths_for_tool_call(
                            session.as_ref(),
                            turn.as_ref(),
                            &call.tool_name,
                            &call.payload,
                        );
                        let _guard = if supports_parallel {
                            Either::Left(lock.read().await)
                        } else {
                            Either::Right(lock.write().await)
                        };

                        let mut response = router
                            .dispatch_tool_call(
                                Arc::clone(&session),
                                Arc::clone(&turn),
                                Arc::clone(&tracker),
                                call.clone(),
                            )
                            .instrument(dispatch_span.clone())
                            .await?;

                        if should_attempt_mention_rewrite(&read_paths, &response) {
                            // Build turn-scoped mention rewrite data once, then reuse.
                            let rewrite_context = mention_rewrite_data
                                .get_or_init(|| async {
                                    load_turn_mention_rewrite_data(session.as_ref(), turn.as_ref()).await
                                })
                                .await
                                .as_ref()
                                .and_then(|data| {
                                    mention_rewrite_context_for_read_paths(read_paths, data)
                                });
                            if let Some(context) = rewrite_context.as_ref() {
                                rewrite_tool_response_mentions(
                                    &mut response,
                                    &call.tool_name,
                                    context.as_ref(),
                                );
                            }
                        }

                        Ok(response)
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

#[cfg(test)]
mod tests {
    use codex_protocol::models::FunctionCallOutputPayload;
    use pretty_assertions::assert_eq;

    use super::should_attempt_mention_rewrite;
    use super::*;

    #[test]
    fn should_attempt_mention_rewrite_requires_non_empty_read_paths() {
        let response = ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text("use $alpha-skill".to_string()),
        };
        assert_eq!(false, should_attempt_mention_rewrite(&[], &response));
    }

    #[test]
    fn should_attempt_mention_rewrite_requires_dollar_mentions_in_output() {
        let response = ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text("plain output".to_string()),
        };
        assert_eq!(
            false,
            should_attempt_mention_rewrite(
                &[PathBuf::from("/tmp/skills/alpha/SKILL.md")],
                &response
            )
        );
    }

    #[test]
    fn should_attempt_mention_rewrite_allows_when_all_gates_pass() {
        let response = ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text("use $alpha-skill".to_string()),
        };
        assert_eq!(
            true,
            should_attempt_mention_rewrite(
                &[PathBuf::from("/tmp/skills/alpha/SKILL.md")],
                &response
            )
        );
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
                    body: FunctionCallOutputBody::Text(Self::abort_message(call, secs)),
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
