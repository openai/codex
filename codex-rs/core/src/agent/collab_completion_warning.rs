use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use serde_json::json;
use uuid::Uuid;

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
            && !crate::agent::is_collab_wait_suppressed(
                session.as_ref(),
                &turn_context.sub_id,
                agent_id,
            )
            .await
        {
            let items = synthetic_wait_items(agent_id, status);
            session
                .record_conversation_items(&turn_context, &items)
                .await;
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

fn synthetic_wait_items(agent_id: ThreadId, status: AgentStatus) -> Vec<ResponseItem> {
    tracing::info!("synthetic_wait_items: agent_id: {}, status: {:?}", agent_id, status);
    let call_id = format!("synthetic-wait-{}", Uuid::new_v4());
    let agent_id_str = agent_id.to_string();
    let arguments = json!({
        "ids": [agent_id_str.clone()],
        "timeout_ms": 300_000,
    })
    .to_string();
    let output = json!({
        "status": { agent_id_str: status },
        "timed_out": false,
    })
    .to_string();

    let call = ResponseItem::FunctionCall {
        id: None,
        name: "wait".to_string(),
        arguments,
        call_id: call_id.clone(),
    };
    let output = ResponseItem::FunctionCallOutput {
        call_id,
        output: FunctionCallOutputPayload {
            content: output,
            ..Default::default()
        },
    };

    vec![call, output]
}

#[cfg(test)]
mod tests {
    use super::synthetic_wait_items;
    use crate::agent::AgentStatus;
    use codex_protocol::ThreadId;
    use codex_protocol::models::ResponseItem;
    use pretty_assertions::assert_eq;
    use serde_json::Value;

    #[test]
    fn synthetic_wait_items_look_like_a_real_wait_result() {
        let agent_id =
            ThreadId::from_string("00000000-0000-7000-0000-000000000001").expect("valid id");
        let status = AgentStatus::Completed(Some("done".to_string()));

        let items = synthetic_wait_items(agent_id, status.clone());
        assert_eq!(items.len(), 2);

        let (call_id, arguments_json) = match &items[0] {
            ResponseItem::FunctionCall {
                name,
                call_id,
                arguments,
                ..
            } => {
                assert_eq!(name, "wait");
                (call_id.clone(), arguments.clone())
            }
            other => panic!("expected function call, got {other:?}"),
        };

        let args: Value = serde_json::from_str(&arguments_json).expect("arguments should be json");
        let agent_id_string = agent_id.to_string();
        assert_eq!(args["ids"][0].as_str(), Some(agent_id_string.as_str()));

        match &items[1] {
            ResponseItem::FunctionCallOutput {
                call_id: out_id,
                output,
            } => {
                assert_eq!(out_id, &call_id);
                let out: Value =
                    serde_json::from_str(&output.content).expect("output should be json");
                assert_eq!(out["timed_out"].as_bool(), Some(false));
                let expected_status =
                    serde_json::to_value(status).expect("status should serialize");
                assert_eq!(out["status"][agent_id_string.as_str()], expected_status);
            }
            other => panic!("expected function call output, got {other:?}"),
        }
    }
}
