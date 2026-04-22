use std::sync::Arc;

use super::SessionTask;
use super::SessionTaskContext;
use super::TurnTaskInput;
use crate::session::turn_context::TurnContext;
use crate::state::TaskKind;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Default)]
pub(crate) struct CompactTask;

impl SessionTask for CompactTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Compact
    }

    fn span_name(&self) -> &'static str {
        "session_task.compact"
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: TurnTaskInput,
        _cancellation_token: CancellationToken,
    ) -> Option<String> {
        let session = session.clone_session();
        let _ = if crate::compact::should_use_remote_compact_task(ctx.provider.info()) {
            session.services.session_telemetry.counter(
                "codex.task.compact",
                /*inc*/ 1,
                &[("type", "remote")],
            );
            crate::compact_remote::run_remote_compact_task(session.clone(), ctx).await
        } else {
            session.services.session_telemetry.counter(
                "codex.task.compact",
                /*inc*/ 1,
                &[("type", "local")],
            );
            crate::compact::run_compact_task(session.clone(), ctx, input.user_input).await
        };
        None
    }
}
