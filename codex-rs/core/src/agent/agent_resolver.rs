use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
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
    if matches!(target, "parent" | "root")
        && let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id, ..
        }) = &turn.session_source
    {
        return Ok(*parent_thread_id);
    }

    session
        .services
        .agent_control
        .resolve_agent_reference(session.conversation_id, &turn.session_source, target)
        .await
        .map_err(|err| match err {
            codex_protocol::error::CodexErr::UnsupportedOperation(message) => {
                FunctionCallError::RespondToModel(message)
            }
            other => FunctionCallError::RespondToModel(other.to_string()),
        })
}

fn register_session_root(session: &Arc<Session>, turn: &Arc<TurnContext>) {
    session
        .services
        .agent_control
        .register_session_root(session.conversation_id, &turn.session_source);
}
