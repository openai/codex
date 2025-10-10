use std::sync::Arc;

use async_trait::async_trait;

use crate::codex::TurnContext;
use crate::codex::compact;
use crate::protocol::InputItem;
use crate::state::TaskKind;

use super::SessionTask;
use super::SessionTaskContext;

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
        sub_id: String,
        input: Vec<InputItem>,
    ) -> Option<String> {
        compact::run_compact_task(session.clone_session(), ctx, sub_id, input).await
    }
}
