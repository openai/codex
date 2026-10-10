//! Ensures MCP cancellation releases pending elicitation routes and tool timeout pauses.

use super::ElicitationClientService;
use super::ElicitationPauseState;
use super::ElicitationResponse;
use super::RmcpClient;
use crate::InProcessTransportFactory;
use codex_protocol::mcp::OPENAI_ELICITATION_EXTENSION_ID;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use rmcp::RoleServer;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::model::CancelledNotification;
use rmcp::model::CancelledNotificationParam;
use rmcp::model::ClientConfig;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::CustomRequest;
use rmcp::model::ElicitationAction;
use rmcp::model::ProtocolVersion;
use rmcp::model::RequestId;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::model::ServerNotification;
use rmcp::model::ServerRequest;
use rmcp::service::PeerRequestOptions;
use rmcp::service::QuitReason;
use rmcp::service::RunningService;
use rmcp::service::ServerInitializeError;
use rmcp::service::serve_directly;
use rmcp::transport::IntoTransport;
use rmcp::transport::Transport;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing_test::traced_test;

struct ElicitationTestServer;

impl ServerHandler for ElicitationTestServer {}

struct ElicitationTestTransport {
    servers: mpsc::UnboundedSender<
        Result<RunningService<RoleServer, ElicitationTestServer>, ServerInitializeError>,
    >,
}

impl InProcessTransportFactory for ElicitationTestTransport {
    fn open(&self) -> BoxFuture<'static, std::io::Result<tokio::io::DuplexStream>> {
        let servers = self.servers.clone();
        Box::pin(async move {
            let (client, server) = tokio::io::duplex(/*max_buf_size*/ 4096);
            tokio::spawn(async move {
                let _ = servers.send(ElicitationTestServer.serve(server).await);
            });
            Ok(client)
        })
    }
}

#[traced_test]
#[tokio::test]
async fn recovered_connections_accept_elicitations_with_previously_cancelled_ids()
-> anyhow::Result<()> {
    for params in [
        json!({
            "mode": "form",
            "message": "Confirm",
            "requestedSchema": {"type": "object", "properties": {}},
        }),
        json!({
            "mode": "url",
            "message": "Authorize",
            "url": "https://example.com/authorize",
            "elicitationId": "authorization",
        }),
    ] {
        let (servers, mut server_rx) = mpsc::unbounded_channel();
        let client =
            RmcpClient::new_in_process_client(Arc::new(ElicitationTestTransport { servers }))
                .await?;
        client
            .initialize(
                ClientConfig::default().with_protocol_version(ProtocolVersion::V_2025_06_18),
                Some(Duration::from_secs(/*secs*/ 5)),
                Box::new(|_, _| {
                    Box::pin(async {
                        Ok(ElicitationResponse {
                            action: ElicitationAction::Accept,
                            content: None,
                            meta: None,
                        })
                    })
                }),
            )
            .await?;
        let server = timeout(Duration::from_secs(/*secs*/ 5), server_rx.recv())
            .await?
            .unwrap()?;
        let reason = format!("late cancellation for {}", params["mode"]);
        let request =
            ServerRequest::CustomRequest(CustomRequest::new("elicitation/create", Some(params)));
        let original = server
            .send_request_with_option(request.clone(), PeerRequestOptions::no_options())
            .await?;
        let id = original.id.clone();
        assert_eq!(
            serde_json::to_value(
                timeout(Duration::from_secs(/*secs*/ 5), original.await_response()).await??
            )?,
            json!({"action": "accept"}),
        );
        server
            .notify_cancelled(CancelledNotificationParam::new(
                Some(id.clone()),
                Some(reason.clone()),
            ))
            .await?;
        // Notifications run in independent tasks. The handler logs only after recording
        // the cancellation, so wait for that acknowledgement before starting recovery.
        let handled = format!(
            "MCP server cancelled request (request_id: Some({id:?}), reason: Some({reason:?}))"
        );
        timeout(Duration::from_secs(/*secs*/ 5), async {
            while !logs_contain(&handled) {
                tokio::task::yield_now().await;
            }
        })
        .await?;

        let previous = client.service().await?;
        client.reinitialize_after_session_expiry(&previous).await?;
        let recovered_server = timeout(Duration::from_secs(/*secs*/ 5), server_rx.recv())
            .await?
            .unwrap()?;
        let recovered = recovered_server
            .send_request_with_option(request, PeerRequestOptions::no_options())
            .await?;
        assert_eq!(recovered.id, id);
        assert_eq!(
            serde_json::to_value(
                timeout(Duration::from_secs(/*secs*/ 5), recovered.await_response()).await??
            )?,
            json!({"action": "accept"}),
        );
        client.shutdown().await;
        drop(previous);
        server.cancel().await?;
        recovered_server.cancel().await?;
    }
    Ok(())
}

#[tokio::test]
async fn ordinary_elicitations_release_pending_responses_on_cancellation() -> anyhow::Result<()> {
    for params in [
        json!({
            "mode": "form",
            "message": "Confirm",
            "requestedSchema": {"type": "object", "properties": {}},
        }),
        json!({
            "mode": "url",
            "message": "Authorize",
            "url": "https://example.com/authorize",
            "elicitationId": "authorization",
        }),
    ] {
        let pause_state = ElicitationPauseState::new();
        let mut paused = pause_state.subscribe();
        let (route_tx, mut route_rx) = mpsc::unbounded_channel();
        let service = ElicitationClientService::new(
            ClientConfig::default(),
            Box::new(move |_, _| {
                let (response_tx, response_rx) = oneshot::channel();
                route_tx.send(response_tx).expect("observe elicitation");
                Box::pin(async move { Ok(response_rx.await?) })
            }),
            pause_state,
        );
        let (client_transport, server_transport) = tokio::io::duplex(/*max_buf_size*/ 4096);
        let client = serve_directly(service, client_transport, /*peer_info*/ None);
        let mut server = IntoTransport::<RoleServer, _, _>::into_transport(server_transport);
        let request =
            ServerRequest::CustomRequest(CustomRequest::new("elicitation/create", Some(params)));
        server
            .send(ServerJsonRpcMessage::request(
                request.clone(),
                RequestId::Number(1),
            ))
            .await?;
        let mut response_tx = timeout(Duration::from_secs(/*secs*/ 5), route_rx.recv())
            .await?
            .expect("elicitation reached UI without user-verification capability");
        assert!(*paused.borrow());

        server
            .send(ServerJsonRpcMessage::notification(
                ServerNotification::CancelledNotification(CancelledNotification::new(
                    CancelledNotificationParam::new(
                        Some(RequestId::Number(1)),
                        /*reason*/ None,
                    ),
                )),
            ))
            .await?;
        timeout(Duration::from_secs(/*secs*/ 5), response_tx.closed()).await?;
        timeout(
            Duration::from_secs(/*secs*/ 5),
            paused.wait_for(|paused| !*paused),
        )
        .await??;
        server
            .send(ServerJsonRpcMessage::request(request, RequestId::Number(2)))
            .await?;
        let mut response_tx = timeout(Duration::from_secs(/*secs*/ 5), route_rx.recv())
            .await?
            .expect("connection still accepts elicitations after cancellation");
        assert!(*paused.borrow());

        client.cancel().await?;

        timeout(Duration::from_secs(/*secs*/ 5), response_tx.closed()).await?;
        timeout(
            Duration::from_secs(/*secs*/ 5),
            paused.wait_for(|paused| !*paused),
        )
        .await??;
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn transport_eof_releases_pending_elicitations_without_dropping_client() -> anyhow::Result<()>
{
    for (method, params) in [
        (
            "elicitation/create",
            json!({
                "mode": "form",
                "message": "Confirm",
                "requestedSchema": {"type": "object", "properties": {}},
            }),
        ),
        (
            "openai/elicitation/create",
            json!({
                "mode": "openai/userVerification",
                "title": "Approve",
                "description": "",
                "challenge": "AQID",
            }),
        ),
    ] {
        let pause_state = ElicitationPauseState::new();
        let mut paused = pause_state.subscribe();
        let (route_tx, mut route_rx) = mpsc::unbounded_channel();
        let mut client_info = ClientConfig::default();
        client_info.capabilities.extensions = Some(
            [(
                OPENAI_ELICITATION_EXTENSION_ID.to_string(),
                serde_json::Map::from_iter([("userVerification".to_string(), json!({}))]),
            )]
            .into_iter()
            .collect(),
        );
        let service = ElicitationClientService::new(
            client_info,
            Box::new(move |_, _| {
                let (response_tx, response_rx) = oneshot::channel();
                route_tx
                    .send(response_tx)
                    .expect("observe pending elicitation");
                Box::pin(async move { Ok(response_rx.await?) })
            }),
            pause_state,
        );
        let (client_transport, server_transport) = tokio::io::duplex(/*max_buf_size*/ 4096);
        let client = serve_directly(service, client_transport, /*peer_info*/ None);
        let mut server = IntoTransport::<RoleServer, _, _>::into_transport(server_transport);
        // A server can request approval without an outgoing tools/call to detect EOF.
        server
            .send(serde_json::from_value::<ServerJsonRpcMessage>(json!({
                "jsonrpc": "2.0", "id": 1, "method": method, "params": params,
            }))?)
            .await?;
        let mut response_tx = timeout(Duration::from_secs(/*secs*/ 5), route_rx.recv())
            .await?
            .expect("elicitation reached the UI");
        assert!(*paused.borrow());

        drop(server);

        // RMCP allows five seconds to drain responses after EOF. Retain the client
        // throughout cleanup: dropping it would cancel the handler and hide the bug.
        timeout(Duration::from_secs(/*secs*/ 7), response_tx.closed()).await?;
        timeout(
            Duration::from_secs(/*secs*/ 1),
            paused.wait_for(|paused| !*paused),
        )
        .await??;
        // Only consume the retained client after observing handler cleanup.
        let reason = timeout(Duration::from_secs(/*secs*/ 1), client.waiting()).await??;
        assert!(matches!(reason, QuitReason::Closed));
    }
    Ok(())
}

#[tokio::test]
async fn user_verification_service_cancellation_drops_pending_response() -> anyhow::Result<()> {
    let pause_state = ElicitationPauseState::new();
    let mut paused = pause_state.subscribe();
    let (route_tx, mut route_rx) = mpsc::unbounded_channel();
    let mut client_info = ClientConfig::default();
    client_info.capabilities.extensions = Some(
        [(
            OPENAI_ELICITATION_EXTENSION_ID.to_string(),
            serde_json::Map::from_iter([("userVerification".to_string(), json!({}))]),
        )]
        .into_iter()
        .collect(),
    );
    let service = ElicitationClientService::new(
        client_info,
        Box::new(move |_, _| {
            let (response_tx, response_rx) = oneshot::channel();
            route_tx
                .send(response_tx)
                .expect("observe pending verification");
            Box::pin(async move { Ok(response_rx.await?) })
        }),
        pause_state,
    );
    let (client_transport, server_transport) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let client = serve_directly(service, client_transport, /*peer_info*/ None);
    let mut server = IntoTransport::<RoleServer, _, _>::into_transport(server_transport);
    server
        .send(ServerJsonRpcMessage::request(
            ServerRequest::CustomRequest(CustomRequest::new(
                "openai/elicitation/create",
                Some(json!({
                    "mode": "openai/userVerification",
                    "title": "Approve",
                    "description": "",
                    "challenge": "AQID",
                })),
            )),
            RequestId::Number(1),
        ))
        .await?;
    let mut response_tx = timeout(Duration::from_secs(/*secs*/ 5), route_rx.recv())
        .await?
        .expect("verification was routed to the UI");
    assert!(*paused.borrow());

    client.cancel().await?;

    timeout(Duration::from_secs(/*secs*/ 5), response_tx.closed()).await?;
    timeout(
        Duration::from_secs(/*secs*/ 5),
        paused.wait_for(|paused| !*paused),
    )
    .await??;
    Ok(())
}

#[tokio::test]
async fn cancelling_one_verification_leaves_the_mcp_connection_and_other_requests_alive()
-> anyhow::Result<()> {
    let pause_state = ElicitationPauseState::new();
    let mut paused = pause_state.subscribe();
    let (route_tx, mut route_rx) = mpsc::unbounded_channel();
    let mut client_info = ClientConfig::default();
    client_info.capabilities.extensions = Some(
        [(
            OPENAI_ELICITATION_EXTENSION_ID.to_string(),
            serde_json::Map::from_iter([("userVerification".to_string(), json!({}))]),
        )]
        .into_iter()
        .collect(),
    );
    let service = ElicitationClientService::new(
        client_info,
        Box::new(move |id, _| {
            let (response_tx, response_rx) = oneshot::channel();
            route_tx
                .send((id, response_tx))
                .expect("observe verification");
            Box::pin(async move { Ok(response_rx.await?) })
        }),
        pause_state,
    );
    let (client_transport, server_transport) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let client = serve_directly(service, client_transport, /*peer_info*/ None);
    let mut server = IntoTransport::<RoleServer, _, _>::into_transport(server_transport);

    for id in [1, 2] {
        server
            .send(ServerJsonRpcMessage::request(
                ServerRequest::CustomRequest(CustomRequest::new(
                    "openai/elicitation/create",
                    Some(json!({
                        "mode": "openai/userVerification",
                        "title": "Approve",
                        "description": "",
                        "challenge": "AQID",
                    })),
                )),
                RequestId::Number(id),
            ))
            .await?;
    }
    let (first_id, mut first) = timeout(Duration::from_secs(/*secs*/ 5), route_rx.recv())
        .await?
        .expect("first verification reached UI");
    let (second_id, second) = timeout(Duration::from_secs(/*secs*/ 5), route_rx.recv())
        .await?
        .expect("second verification reached UI");
    assert_ne!(first_id, second_id);
    assert!(*paused.borrow());

    server
        .send(ServerJsonRpcMessage::notification(
            ServerNotification::CancelledNotification(CancelledNotification::new(
                CancelledNotificationParam::new(Some(first_id.clone()), None),
            )),
        ))
        .await?;
    timeout(Duration::from_secs(/*secs*/ 5), first.closed()).await?;
    assert!(!second.is_closed());
    assert!(*paused.borrow());

    for request_id in [first_id, RequestId::Number(999)] {
        server
            .send(ServerJsonRpcMessage::notification(
                ServerNotification::CancelledNotification(CancelledNotification::new(
                    CancelledNotificationParam::new(Some(request_id), None),
                )),
            ))
            .await?;
    }
    assert!(!second.is_closed());

    second
        .send(ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(json!({"credentialId": "AQID", "signature": "BAUG"})),
            meta: None,
        })
        .expect("second verification remains pending");
    let accepted = timeout(Duration::from_secs(/*secs*/ 5), server.receive())
        .await?
        .expect("second request survived cancellation");
    let ClientJsonRpcMessage::Response(accepted) = accepted else {
        anyhow::bail!("expected the second verification response");
    };
    assert_eq!(accepted.id, second_id);
    assert_eq!(serde_json::to_value(accepted.result)?["action"], "accept");
    timeout(
        Duration::from_secs(/*secs*/ 5),
        paused.wait_for(|paused| !*paused),
    )
    .await??;
    client.cancel().await?;
    Ok(())
}
