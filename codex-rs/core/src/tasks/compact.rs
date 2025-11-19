use std::sync::Arc;

use super::SessionTask;
use super::SessionTaskContext;
use crate::codex::TurnContext;
use crate::features::Feature;
use crate::state::TaskKind;
use async_trait::async_trait;
use codex_app_server_protocol::AuthMode;
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
        if session
            .services
            .auth_manager
            .auth()
            .is_some_and(|auth| auth.mode == AuthMode::ChatGPT)
            && session.enabled(Feature::RemoteCompaction).await
        {
            crate::compact_remote::run_remote_compact_task(session, ctx).await
        } else {
            crate::compact::run_compact_task(session, ctx, input).await
        }
    }
}
