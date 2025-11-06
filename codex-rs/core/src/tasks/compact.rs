use std::sync::Arc;

use async_trait::async_trait;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

use super::{SessionTask, SessionTaskContext};
use crate::codex::{TurnContext, compact};
use crate::state::TaskKind;

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
        compact::run_compact_task(session.clone_session(), ctx, input).await
    }
}
