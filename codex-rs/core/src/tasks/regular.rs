use std::future::Future;
use std::sync::Arc;

use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;

use crate::hook_runtime::run_app_bundled_internal_turn_stop_hooks;
use crate::session::TurnInput;
use crate::session::turn::run_turn;
use crate::session::turn_context::TurnContext;
use crate::session_startup_prewarm::SessionStartupPrewarmResolution;
use crate::state::TaskKind;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnStartedEvent;
use tracing::Instrument;
use tracing::trace_span;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Default)]
pub(crate) struct RegularTask {
    app_bundled_internal_stop_finalizer: AppBundledInternalStopFinalizer,
}

#[derive(Default)]
struct AppBundledInternalStopFinalizer {
    completed: OnceCell<()>,
}

impl AppBundledInternalStopFinalizer {
    async fn run_once<F, Fut>(&self, finalize: F)
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()>,
    {
        self.completed.get_or_init(finalize).await;
    }
}

impl RegularTask {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    async fn run_app_bundled_internal_stop_once(
        &self,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        last_assistant_message: Option<String>,
    ) {
        self.app_bundled_internal_stop_finalizer
            .run_once(|| async move {
                let sess = session.clone_session();
                run_app_bundled_internal_turn_stop_hooks(&sess, &ctx, last_assistant_message).await;
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use tokio::sync::Notify;

    use super::AppBundledInternalStopFinalizer;

    #[tokio::test]
    async fn internal_stop_finalizer_runs_exactly_once() {
        let finalizer = AppBundledInternalStopFinalizer::default();
        let calls = AtomicUsize::new(0);

        finalizer
            .run_once(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
            })
            .await;
        finalizer
            .run_once(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn canceled_internal_stop_finalizer_is_retried_by_abort_path() {
        let finalizer = Arc::new(AppBundledInternalStopFinalizer::default());
        let started = Arc::new(Notify::new());
        let never_finish = Arc::new(Notify::new());
        let first_finalizer = Arc::clone(&finalizer);
        let first_started = Arc::clone(&started);
        let first_never_finish = Arc::clone(&never_finish);
        let first = tokio::spawn(async move {
            first_finalizer
                .run_once(|| async move {
                    first_started.notify_one();
                    first_never_finish.notified().await;
                })
                .await;
        });
        started.notified().await;
        first.abort();
        let _ = first.await;

        let retries = AtomicUsize::new(0);
        finalizer
            .run_once(|| async {
                retries.fetch_add(1, Ordering::SeqCst);
            })
            .await;
        finalizer
            .run_once(|| async {
                retries.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        assert_eq!(retries.load(Ordering::SeqCst), 1);
    }
}

impl SessionTask for RegularTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    fn span_name(&self) -> &'static str {
        "session_task.turn"
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<TurnInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let sess = session.clone_session();
        let turn_extension_data = session.turn_extension_data();
        let run_turn_span = trace_span!("run_turn");
        // Regular turns emit `TurnStarted` inline so first-turn lifecycle does
        // not wait on startup prewarm resolution.
        let prewarmed_client_session = async {
            let event = EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: ctx.sub_id.clone(),
                trace_id: ctx.trace_id.clone(),
                started_at: ctx.turn_timing_state.started_at_unix_secs().await,
                model_context_window: ctx.model_context_window(),
                collaboration_mode_kind: ctx.collaboration_mode.mode,
            });
            sess.send_event(ctx.as_ref(), event).await;
            sess.set_server_reasoning_included(/*included*/ false).await;
            sess.consume_startup_prewarm_for_regular_turn(&cancellation_token)
                .await
        }
        .instrument(trace_span!("regular_task.prepare_run_turn"))
        .await;
        let prewarmed_client_session = match prewarmed_client_session {
            SessionStartupPrewarmResolution::Cancelled => return None,
            SessionStartupPrewarmResolution::Unavailable { .. } => None,
            SessionStartupPrewarmResolution::Ready(prewarmed_client_session) => {
                Some(*prewarmed_client_session)
            }
        };
        let mut next_input = input;
        let mut prewarmed_client_session = prewarmed_client_session;
        loop {
            let last_agent_message = run_turn(
                Arc::clone(&sess),
                Arc::clone(&ctx),
                Arc::clone(&turn_extension_data),
                next_input,
                prewarmed_client_session.take(),
                cancellation_token.child_token(),
            )
            .instrument(run_turn_span.clone())
            .await;
            if !sess.input_queue.has_pending_input(&sess.active_turn).await {
                self.run_app_bundled_internal_stop_once(
                    Arc::clone(&session),
                    Arc::clone(&ctx),
                    last_agent_message.clone(),
                )
                .await;
                return last_agent_message;
            }
            next_input = Vec::new();
        }
    }

    async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
        // The regular task is force-aborted after a short grace period. OnceCell is cancellation
        // safe: if the normal finalizer was interrupted while awaiting the hook, this abort path
        // retries it; if it finished, the idempotent cleanup is not run twice.
        self.run_app_bundled_internal_stop_once(session, ctx, None)
            .await;
    }
}
