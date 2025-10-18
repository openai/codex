use std::sync::Arc;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::codex::TurnContext;
use crate::codex::run_task;
use crate::protocol::InputItem;
use crate::state::TaskKind;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Clone, Copy, Default)]
pub(crate) struct RegularTask;

#[async_trait]
impl SessionTask for RegularTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        sub_id: String,
        input: Vec<InputItem>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let sess = session.clone_session();
        run_task(
            sess,
            ctx,
            sub_id,
            input,
            TaskKind::Regular,
            cancellation_token,
        )
        .await
    }
}
