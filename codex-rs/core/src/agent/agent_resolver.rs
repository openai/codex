use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use codex_protocol::ThreadId;
use std::sync::Arc;

/// Resolves a single tool-facing agent target to a thread id.
pub(crate) async fn resolve_agent_target(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    target: &str,
) -> Result<ThreadId, FunctionCallError> {
    register_session_root(session, turn);
    if let Ok(thread_id) = ThreadId::from_string(target) {
        return Ok(thread_id);
    }

    session
        .services
        .agent_control
        .resolve_agent_reference(session.conversation_id, &turn.session_source, target)
        .await
        .map_err(|err| match err {
            crate::error::CodexErr::UnsupportedOperation(message) => {
                FunctionCallError::RespondToModel(message)
            }
            other => FunctionCallError::RespondToModel(other.to_string()),
        })
}

/// Resolves multiple tool-facing agent targets to thread ids.
pub(crate) async fn resolve_agent_targets(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    targets: Vec<String>,
) -> Result<Vec<ThreadId>, FunctionCallError> {
    if targets.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "agent targets must be non-empty".to_string(),
        ));
    }

    let mut resolved = Vec::with_capacity(targets.len());
    for target in &targets {
        resolved.push(resolve_agent_target(session, turn, target).await?);
    }
    Ok(resolved)
}

fn register_session_root(session: &Arc<Session>, turn: &Arc<TurnContext>) {
    session
        .services
        .agent_control
        .register_session_root(session.conversation_id, &turn.session_source);
}
