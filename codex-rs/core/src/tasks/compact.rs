use std::sync::Arc;

use super::SessionTask;
use super::SessionTaskContext;
use crate::codex::TurnContext;
use crate::state::TaskKind;
use async_trait::async_trait;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Default)]
pub(crate) struct CompactTask;

#[async_trait]
impl SessionTask for CompactTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Compact
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        _cancellation_token: CancellationToken,
    ) -> Option<String> {
        let session = session.clone_session();
        if crate::compact::should_use_remote_compact_task(session.as_ref(), &ctx.provider) {
            let _ = session.services.otel_manager.counter(
                "codex.task.compact",
                1,
                &[("type", "remote")],
            );
            crate::compact_remote::run_remote_compact_task(session, ctx).await
        } else {
            let _ = session.services.otel_manager.counter(
                "codex.task.compact",
                1,
                &[("type", "local")],
            );
            crate::compact::run_compact_task(session, ctx, input).await
        }

        None
    }
}
