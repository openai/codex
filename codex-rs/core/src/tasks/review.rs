use std::sync::Arc;

use async_trait::async_trait;

use crate::codex::TurnContext;
use crate::codex::exit_review_mode;
use crate::codex::run_task;
use crate::protocol::InputItem;
use crate::state::TaskKind;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Clone, Copy, Default)]
pub(crate) struct ReviewTask;

#[async_trait]
impl SessionTask for ReviewTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Review
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        sub_id: String,
        input: Vec<InputItem>,
    ) -> Option<String> {
        let sess = session.clone_session();
        run_task(sess, ctx, sub_id, input).await
    }

    async fn abort(&self, session: Arc<SessionTaskContext>, sub_id: &str) {
        exit_review_mode(session.clone_session(), sub_id.to_string(), None).await;
    }
}
