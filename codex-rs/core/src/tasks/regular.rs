use std::sync::Arc;
use std::sync::Mutex;

use crate::client::ModelClient;
use crate::client::ModelClientSession;
use crate::client_common::Prompt;
use crate::codex::TurnContext;
use crate::codex::run_turn;
use crate::error::Result as CodexResult;
use crate::state::TaskKind;
use async_trait::async_trait;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing::trace_span;

use super::SessionTask;
use super::SessionTaskContext;

pub(crate) struct RegularTask {
    prewarmed_session: Mutex<Option<ModelClientSession>>,
}

impl Default for RegularTask {
    fn default() -> Self {
        Self {
            prewarmed_session: Mutex::new(None),
        }
    }
}

impl RegularTask {
    pub(crate) async fn with_startup_prewarm(
        model_client: ModelClient,
        prompt: Prompt,
        turn_context: Arc<TurnContext>,
        turn_metadata_header: Option<String>,
    ) -> CodexResult<Self> {
        let mut client_session = model_client.new_session();
        client_session
            .prewarm_websocket(
                &prompt,
                &turn_context.model_info,
                &turn_context.otel_manager,
                turn_context.reasoning_effort,
                turn_context.reasoning_summary,
                turn_metadata_header.as_deref(),
            )
            .await?;

        Ok(Self {
            prewarmed_session: Mutex::new(Some(client_session)),
        })
    }

    async fn take_prewarmed_session(&self) -> Option<ModelClientSession> {
        self.prewarmed_session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
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
        let prewarmed_client_session = self.take_prewarmed_session().await;
        run_turn(
            sess,
            ctx,
            input,
            prewarmed_client_session,
            cancellation_token,
        )
        .instrument(run_turn_span)
        .await
    }
}
