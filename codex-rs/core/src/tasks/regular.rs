use std::sync::Arc;
use std::sync::Mutex;

use crate::client::ModelClient;
use crate::client::ModelClientSession;
use crate::codex::TurnContext;
use crate::codex::run_turn;
use crate::state::TaskKind;
use async_trait::async_trait;
use codex_otel::OtelManager;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::user_input::UserInput;
use futures::future::BoxFuture;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing::trace_span;
use tracing::warn;

use super::SessionTask;
use super::SessionTaskContext;

type PrewarmedSessionTask = JoinHandle<Option<ModelClientSession>>;

pub(crate) struct RegularTask {
    prewarmed_session_task: Mutex<Option<PrewarmedSessionTask>>,
    pre_turn_context_items: Vec<ResponseItem>,
}

impl Default for RegularTask {
    fn default() -> Self {
        Self {
            prewarmed_session_task: Mutex::new(None),
            pre_turn_context_items: Vec::new(),
        }
    }
}

impl RegularTask {
    pub(crate) fn with_startup_prewarm(
        model_client: ModelClient,
        otel_manager: OtelManager,
        model_info: ModelInfo,
        turn_metadata_header: BoxFuture<'static, Option<String>>,
    ) -> Self {
        let prewarmed_session_task = tokio::spawn(async move {
            let mut client_session = model_client.new_session();
            let turn_metadata_header = turn_metadata_header.await;
            match client_session
                .prewarm_websocket(&otel_manager, &model_info, turn_metadata_header.as_deref())
                .await
            {
                Ok(()) => Some(client_session),
                Err(err) => {
                    warn!("startup websocket prewarm task failed: {err}");
                    None
                }
            }
        });

        Self {
            prewarmed_session_task: Mutex::new(Some(prewarmed_session_task)),
            pre_turn_context_items: Vec::new(),
        }
    }

    pub(crate) fn with_pre_turn_context_items(mut self, items: Vec<ResponseItem>) -> Self {
        self.pre_turn_context_items = items;
        self
    }

    async fn take_prewarmed_session(&self) -> Option<ModelClientSession> {
        let prewarmed_session_task = self
            .prewarmed_session_task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        match prewarmed_session_task {
            Some(task) => match task.await {
                Ok(client_session) => client_session,
                Err(err) => {
                    warn!("startup websocket prewarm task join failed: {err}");
                    None
                }
            },
            None => None,
        }
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
            self.pre_turn_context_items.clone(),
            prewarmed_client_session,
            cancellation_token,
        )
        .instrument(run_turn_span)
        .await
    }
}
