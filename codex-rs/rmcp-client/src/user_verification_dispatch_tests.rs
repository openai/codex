use super::*;
use pretty_assertions::assert_eq;
use rmcp::RoleServer;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::service::serve_directly;
use rmcp::transport::IntoTransport;
use rmcp::transport::Transport;
use serde_json::json;
use std::time::Duration;
use tokio::time::timeout;

fn service() -> ElicitationClientService {
    let mut info = ClientInfo::default();
    info.capabilities.extensions = Some(
        [(
            OPENAI_ELICITATION_EXTENSION_ID.into(),
            Map::from_iter([("userVerification".into(), json!({}))]),
        )]
        .into_iter()
        .collect(),
    );
    ElicitationClientService::new(
        info,
        Box::new(|_, _| panic!("cancelled or malformed verification must not reach the UI")),
        ElicitationPauseState::new(),
    )
}

#[tokio::test]
async fn user_verification_dispatch_preserves_context_metadata() -> anyhow::Result<()> {
    let mut service = service();
    service.send_elicitation = Arc::new(Box::new(|_, request| {
        assert_eq!(
            request,
            Elicitation::UserVerification {
                meta: Some(json!({"example/display": {"label": "Operation"}})),
                title: "Approve".into(),
                description: "Review operation".into(),
                challenge: "AQID".into(),
            }
        );
        Box::pin(async {
            Ok(ElicitationResponse {
                action: ElicitationAction::Cancel,
                content: None,
                meta: None,
            })
        })
    }));
    let (client_transport, server_transport) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let client = serve_directly(service, client_transport, /*peer_info*/ None);
    let mut server = IntoTransport::<RoleServer, _, _>::into_transport(server_transport);
    server
        .send(serde_json::from_value::<ServerJsonRpcMessage>(json!({
            "jsonrpc": "2.0", "id": 1, "method": OPENAI_ELICITATION_METHOD,
            "params": {
                "mode": crate::user_verification::MODE,
                "title": "Approve", "description": "Review operation", "challenge": "AQID",
                "_meta": {"progressToken": 7, "example/display": {"label": "Operation"}}
            }
        }))?)
        .await?;
    let response = timeout(Duration::from_secs(/*secs*/ 5), server.receive())
        .await?
        .unwrap();
    assert_eq!(
        serde_json::to_value(response)?,
        json!({"jsonrpc": "2.0", "id": 1, "result": {"action": "cancel"}})
    );
    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn user_verification_remembers_cancellation_before_request_handler_runs() -> anyhow::Result<()>
{
    let service = service();
    let (client_transport, server_transport) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let client = serve_directly(service.clone(), client_transport, /*peer_info*/ None);
    let mut server = IntoTransport::<RoleServer, _, _>::into_transport(server_transport);
    server
        .send(serde_json::from_value::<ServerJsonRpcMessage>(json!({
            "jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 1}
        }))?)
        .await?;
    // Force the scheduling order that RMCP's independently spawned handlers allow.
    timeout(Duration::from_secs(/*secs*/ 5), async {
        loop {
            if service
                .pending_verifications
                .lock()
                .unwrap()
                .early
                .contains(&RequestId::Number(1))
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    server.send(serde_json::from_value::<ServerJsonRpcMessage>(json!({
        "jsonrpc": "2.0", "id": 1, "method": OPENAI_ELICITATION_METHOD,
        "params": {"mode": crate::user_verification::MODE, "title": "Approve", "description": "", "challenge": "AQID"}
    }))?).await?;
    let response = timeout(Duration::from_secs(/*secs*/ 5), server.receive())
        .await?
        .unwrap();
    assert_eq!(
        serde_json::to_value(response)?,
        json!({
            "jsonrpc": "2.0", "id": 1, "result": {"action": "cancel"}
        })
    );
    assert!(
        service
            .pending_verifications
            .lock()
            .unwrap()
            .early
            .is_empty()
    );
    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn user_verification_early_cancellation_storage_fails_closed_at_capacity()
-> anyhow::Result<()> {
    let service = service();
    let (client_transport, server_transport) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let client = serve_directly(service.clone(), client_transport, /*peer_info*/ None);
    let mut server = IntoTransport::<RoleServer, _, _>::into_transport(server_transport);
    for id in 0..=MAX_EARLY_CANCELLATIONS {
        server
            .send(serde_json::from_value::<ServerJsonRpcMessage>(json!({
                "jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": id}
            }))?)
            .await?;
    }
    timeout(Duration::from_secs(/*secs*/ 5), async {
        while !service.pending_verifications.lock().unwrap().saturated {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert!(
        service
            .pending_verifications
            .lock()
            .unwrap()
            .early
            .is_empty()
    );
    let response = service.handle_request(
        ServerRequest::CustomRequest(CustomRequest::new(OPENAI_ELICITATION_METHOD, Some(json!({
            "mode": crate::user_verification::MODE, "title": "Approve", "description": "", "challenge": "AQID"
        })))),
        RequestContext::new(RequestId::Number(5000), client.peer().clone()),
    ).await?;
    assert_eq!(serde_json::to_value(response)?, json!({"action": "cancel"}));
    // Ordinary requests still work after verification fails closed.
    server
        .send(serde_json::from_value::<ServerJsonRpcMessage>(json!({
            "jsonrpc": "2.0", "id": 5001, "method": "ping"
        }))?)
        .await?;
    let response = timeout(Duration::from_secs(/*secs*/ 5), server.receive())
        .await?
        .unwrap();
    assert_eq!(
        serde_json::to_value(response)?,
        json!({"jsonrpc": "2.0", "id": 5001, "result": {}})
    );
    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn user_verification_dispatch_rejects_malformed_modes_as_invalid_params() -> anyhow::Result<()>
{
    let (client_transport, server_transport) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let client = serve_directly(service(), client_transport, /*peer_info*/ None);
    let mut server = IntoTransport::<RoleServer, _, _>::into_transport(server_transport);
    for params in [json!({}), json!({"mode": 7}), json!({"mode": "unknown"})] {
        server
            .send(ServerJsonRpcMessage::request(
                ServerRequest::CustomRequest(CustomRequest::new(
                    OPENAI_ELICITATION_METHOD,
                    Some(params),
                )),
                RequestId::Number(1),
            ))
            .await?;
        let response = timeout(Duration::from_secs(/*secs*/ 5), server.receive())
            .await?
            .unwrap();
        let ClientJsonRpcMessage::Error(response) = response else {
            anyhow::bail!("expected invalid params");
        };
        assert_eq!(
            response.error,
            rmcp::ErrorData::invalid_params("invalid elicitation mode", /*data*/ None)
        );
    }
    client.cancel().await?;
    Ok(())
}
