use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::session::TurnInput;
use crate::session::turn::run_turn;
use crate::session::turn_context::TurnContext;
use crate::session_startup_prewarm::SessionStartupPrewarmResolution;
use crate::state::TaskKind;
use tracing::Instrument;
use tracing::trace_span;

use super::SessionTask;
use super::SessionTaskContext;
use super::SessionTaskResult;

pub(crate) struct InitialUserInputEvents {
    abort_input: Mutex<Option<Vec<TurnInput>>>,
    started: AtomicBool,
    recorded: watch::Sender<bool>,
}

impl Default for InitialUserInputEvents {
    fn default() -> Self {
        let (recorded, _receiver) = watch::channel(false);
        Self {
            abort_input: Mutex::new(None),
            started: AtomicBool::new(false),
            recorded,
        }
    }
}

impl InitialUserInputEvents {
    /// Publish the hook-filtered input that an interrupt should preserve if it wins before normal
    /// persistence. This is synchronous so a completed hook decision is visible before the next
    /// cancellable await.
    pub(crate) fn set_abort_input(&self, input: Vec<TurnInput>) {
        *self
            .abort_input
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(input);
    }

    fn abort_input(&self, original_input: &[TurnInput]) -> Vec<TurnInput> {
        self.abort_input
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| original_input.to_vec())
    }

    /// Record one durable copy of the initial user items and report whether this caller won the
    /// right to choose which items were recorded.
    pub(crate) async fn record(
        &self,
        session: Arc<crate::session::session::Session>,
        ctx: Arc<TurnContext>,
        input: Vec<TurnInput>,
    ) -> bool {
        let mut recorded = self.recorded.subscribe();
        let claimed = self
            .started
            .compare_exchange(
                /*current*/ false,
                /*new*/ true,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok();
        if claimed {
            let recorded = self.recorded.clone();
            tokio::spawn(async move {
                session
                    .record_initial_user_input_events(ctx.as_ref(), &input)
                    .await;
                recorded.send_replace(true);
            });
        }

        while !*recorded.borrow_and_update() {
            // `self.recorded` keeps the channel open for the duration of this borrow.
            let _ = recorded.changed().await;
        }
        claimed
    }
}

#[derive(Default)]
pub(crate) struct RegularTask {
    initial_user_input_events: InitialUserInputEvents,
}

impl RegularTask {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl SessionTask for RegularTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    fn span_name(&self) -> &'static str {
        "session_task.turn"
    }

    fn requires_synchronous_turn_start(&self) -> bool {
        true
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<TurnInput>,
        cancellation_token: CancellationToken,
    ) -> SessionTaskResult {
        let sess = session.clone_session();
        let turn_extension_data = session.turn_extension_data();
        let run_turn_span = trace_span!("run_turn");
        // `Session::start_task` emits the regular-turn start before the cancellable startup
        // prewarm work begins.
        let prewarmed_client_session = async {
            sess.consume_startup_prewarm_for_regular_turn(&cancellation_token)
                .await
        }
        .instrument(trace_span!("regular_task.prepare_run_turn"))
        .await;
        let prewarmed_client_session = match prewarmed_client_session {
            SessionStartupPrewarmResolution::Cancelled => return Ok(None),
            SessionStartupPrewarmResolution::Unavailable { .. } => None,
            SessionStartupPrewarmResolution::Ready(prewarmed_client_session) => {
                Some(*prewarmed_client_session)
            }
        };
        let mut next_input = input;
        let mut prewarmed_client_session = prewarmed_client_session;
        let mut initial_user_input_events = Some(&self.initial_user_input_events);
        loop {
            let last_agent_message = run_turn(
                Arc::clone(&sess),
                Arc::clone(&ctx),
                Arc::clone(&turn_extension_data),
                next_input,
                prewarmed_client_session.take(),
                cancellation_token.child_token(),
                initial_user_input_events.take(),
            )
            .instrument(run_turn_span.clone())
            .await?;
            if !sess.input_queue.has_pending_input(&sess.active_turn).await {
                return Ok(last_agent_message);
            }
            next_input = Vec::new();
        }
    }

    async fn abort(
        &self,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: &[TurnInput],
    ) {
        let input = self.initial_user_input_events.abort_input(input);
        self.initial_user_input_events
            .record(session.clone_session(), ctx, input)
            .await;
    }
}
