use std::sync::Arc;

use crate::codex::TurnContext;
use crate::codex::run_turn;
use crate::hooks;
use crate::protocol::HookInput;
use crate::protocol::HookKind;
use crate::state::TaskKind;
use async_trait::async_trait;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing::trace_span;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Clone, Default)]
pub(crate) struct RegularTask {
    hook_input: Option<HookInput>,
}

impl RegularTask {
    pub(crate) fn new(hook_input: Option<HookInput>) -> Self {
        Self { hook_input }
    }
}

#[async_trait]
impl SessionTask for RegularTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let sess = session.clone_session();
        let run_turn_span = trace_span!("run_turn");
        sess.set_server_reasoning_included(false).await;
        sess.services
            .otel_manager
            .apply_traceparent_parent(&run_turn_span);
        let mut hook_inputs = Vec::new();
        if let Some(hook_input) = self.hook_input.clone() {
            hook_inputs.push(hook_input);
        }

        let should_run_turn_start = if self.hook_input.is_some() {
            hooks::should_run_turn_start_on_hook_input(&ctx.cwd).await
        } else {
            true
        };
        if !cancellation_token.is_cancelled()
            && should_run_turn_start
            && let Some(hook_input) =
                hooks::run_hook(&sess, &ctx, HookKind::TurnStart, &cancellation_token).await
        {
            hook_inputs.push(hook_input);
        }

        let run_session = Arc::clone(&sess);
        let last_agent_message = run_turn(
            run_session,
            Arc::clone(&ctx),
            input,
            hook_inputs,
            cancellation_token.clone(),
        )
        .instrument(run_turn_span)
        .await;

        if cancellation_token.is_cancelled() {
            return None;
        }

        if let Some(hook_input) =
            hooks::run_hook(&sess, &ctx, HookKind::TurnEnd, &cancellation_token).await
        {
            sess.enqueue_hook_input(hook_input).await;
            sess.maybe_start_next_hook_turn();
        }

        last_agent_message
    }
}
