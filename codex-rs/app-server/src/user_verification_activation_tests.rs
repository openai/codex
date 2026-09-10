//! Verifies that host initialization owns capability projection and response eligibility.

use super::test_support::Harness;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessage;
use crate::transport::ConnectionOrigin;
use anyhow::Result;
use codex_app_server_protocol as rpc;
use codex_protocol::mcp::OPENAI_ELICITATION_EXTENSION_ID;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn user_verification_initialize_owns_advertisement_and_eligibility() -> Result<()> {
    for (origin, name, opt_in, supported, expected) in [
        (ConnectionOrigin::InProcess, "codex-tui", true, true, true),
        (ConnectionOrigin::WebSocket, "codex-tui", true, true, false),
        (
            ConnectionOrigin::RemoteControl,
            "codex-tui",
            true,
            true,
            false,
        ),
        (ConnectionOrigin::Stdio, "codex-tui", true, true, false),
        (ConnectionOrigin::InProcess, "other-ui", true, true, false),
        (ConnectionOrigin::InProcess, "codex-tui", false, true, false),
        (ConnectionOrigin::InProcess, "codex-tui", true, false, false),
        (ConnectionOrigin::Stdio, "Codex Desktop", true, true, true),
        (
            ConnectionOrigin::InProcess,
            "Codex Desktop",
            true,
            true,
            false,
        ),
        (
            ConnectionOrigin::WebSocket,
            "Codex Desktop",
            true,
            true,
            false,
        ),
        (
            ConnectionOrigin::RemoteControl,
            "Codex Desktop",
            true,
            true,
            false,
        ),
        (ConnectionOrigin::Stdio, "other-ui", true, true, false),
        (ConnectionOrigin::Stdio, "codex_desktop", true, true, false),
        (ConnectionOrigin::Stdio, "Codex Desktop", false, true, false),
        (ConnectionOrigin::Stdio, "Codex Desktop", true, false, false),
    ] {
        let probe: fn() -> bool = if supported { || true } else { || false };
        let mut h = Harness::new(origin, probe).await?;
        h.initialize(name, opt_in).await;
        let projected = h
            .session
            .client_mcp_extensions()
            .iter()
            .any(|(id, settings)| {
                id == OPENAI_ELICITATION_EXTENSION_ID && settings.get("userVerification").is_some()
            });
        assert_eq!(
            projected, expected,
            "{origin:?}/{name}/{opt_in}/{supported}"
        );
        let thread_id = codex_protocol::ThreadId::new();
        let (id, response) = h
            .outgoing
            .send_request_to_connections(
                Some(&[ConnectionId(1)]),
                rpc::ServerRequestPayload::McpServerElicitationRequest(
                    rpc::McpServerElicitationRequestParams {
                        thread_id: thread_id.to_string(),
                        turn_id: None,
                        server_name: "codex_apps".into(),
                        request: rpc::McpServerElicitationRequest::UserVerification {
                            challenge: "AQ".into(),
                            title: "Approve".into(),
                            description: String::new(),
                        },
                    },
                ),
                Some(thread_id),
            )
            .await;
        if expected {
            assert!(matches!(h.response().await, OutgoingMessage::Request(_)));
            let proof =
                json!({"action":"accept", "content":{"credentialId":"AQ", "signature":"Ag"}});
            h.processor
                .process_response(
                    ConnectionId(2),
                    rpc::JSONRPCResponse {
                        id: id.clone(),
                        result: proof.clone(),
                    },
                )
                .await;
            let mut response = std::pin::pin!(response);
            assert!(matches!(
                futures::poll!(&mut response),
                std::task::Poll::Pending
            ));
            h.processor
                .process_response(
                    ConnectionId(1),
                    rpc::JSONRPCResponse {
                        id,
                        result: proof.clone(),
                    },
                )
                .await;
            assert_eq!(response.await?, Ok(proof));
        } else {
            assert!(response.await.is_err());
            assert!(h.messages.try_recv().is_err());
        }
        h.shutdown().await;
    }
    Ok(())
}
