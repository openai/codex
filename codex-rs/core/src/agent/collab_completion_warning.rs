use std::sync::Arc;

use codex_protocol::ThreadId;

use crate::agent::AgentStatus;
use crate::agent::status::is_final;
use crate::codex::Session;
use crate::codex::TurnContext;

/// Subscribe to a spawned sub-agent and warn the model once it reaches a final status.
pub(crate) fn spawn_collab_completion_warning_watcher(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    agent_id: ThreadId,
) {
    tokio::spawn(async move {
        if let Some(status) = wait_for_final_status(session.as_ref(), agent_id).await
            && !crate::agent::is_collab_wait_active(
                session.as_ref(),
                &turn_context.sub_id,
                agent_id,
            )
            .await
        {
            let message = completion_warning_message(agent_id, &status);
            session.record_model_warning(message, &turn_context).await;
        }
    });
}

async fn wait_for_final_status(session: &Session, agent_id: ThreadId) -> Option<AgentStatus> {
    let mut status_rx = match session
        .services
        .agent_control
        .subscribe_status(agent_id)
        .await
    {
        Ok(rx) => rx,
        Err(_) => {
            let status = session.services.agent_control.get_status(agent_id).await;
            return is_final(&status).then_some(status);
        }
    };

    let mut status = status_rx.borrow().clone();
    if is_final(&status) {
        return Some(status);
    }

    loop {
        if status_rx.changed().await.is_err() {
            let latest = session.services.agent_control.get_status(agent_id).await;
            return is_final(&latest).then_some(latest);
        }
        status = status_rx.borrow().clone();
        if is_final(&status) {
            return Some(status);
        }
    }
}

fn completion_warning_message(agent_id: ThreadId, status: &AgentStatus) -> String {
    format!("Sub-agent {agent_id} finished with status {status:?}. Use wait to collect the result.")
}
