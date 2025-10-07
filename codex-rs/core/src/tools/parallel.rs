use std::sync::Arc;

use tokio::sync::RwLock;
use tokio_util::either::Either;
use tokio_util::task::AbortOnDropHandle;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::function_tool::FunctionCallError;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolRouter;
use codex_protocol::models::ResponseInputItem;

pub(crate) struct ToolCallRuntime {
    router: Arc<ToolRouter>,
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
    sub_id: String,
    parallel_execution: Arc<RwLock<()>>,
}

impl ToolCallRuntime {
    pub(crate) fn new(
        router: Arc<ToolRouter>,
        session: Arc<Session>,
        turn_context: Arc<TurnContext>,
        tracker: SharedTurnDiffTracker,
        sub_id: String,
    ) -> Self {
        Self {
            router,
            session,
            turn_context,
            tracker,
            sub_id,
            parallel_execution: Arc::new(RwLock::new(())),
        }
    }

    pub(crate) fn handle_tool_call(
        &self,
        call: ToolCall,
    ) -> impl std::future::Future<Output = Result<ResponseInputItem, CodexErr>> {
        let supports_parallel = self.router.tool_supports_parallel(&call.tool_name);

        let router = Arc::clone(&self.router);
        let session = Arc::clone(&self.session);
        let turn = Arc::clone(&self.turn_context);
        let tracker = Arc::clone(&self.tracker);
        let sub_id = self.sub_id.clone();
        let lock = Arc::clone(&self.parallel_execution);

        let handle: AbortOnDropHandle<Result<ResponseInputItem, FunctionCallError>> =
            AbortOnDropHandle::new(tokio::spawn(async move {
                let _guard = if supports_parallel {
                    Either::Left(lock.read().await)
                } else {
                    Either::Right(lock.write().await)
                };

                router
                    .dispatch_tool_call(session, turn, tracker, sub_id, call)
                    .await
            }));

        async move {
            match handle.await {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(FunctionCallError::Fatal(message))) => Err(CodexErr::Fatal(message)),
                Ok(Err(other)) => Err(CodexErr::Fatal(other.to_string())),
                Err(err) => Err(CodexErr::Fatal(format!(
                    "tool task failed to receive: {err:?}"
                ))),
            }
        }
    }
}
