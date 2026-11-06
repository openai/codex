//! Optional catalog discovery uses the public RPC and never prevents ordinary bootstrap.

use super::*;
use crate::legacy_core::config::ConfigBuilder;
use codex_app_server_protocol::JSONRPCMessage;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::openai_models::ReasoningEffort;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn collaboration_catalog_is_optional_and_refetched_on_bootstrap() -> Result<()> {
    for initial_reply in [
        Some(json!({"error": {"code": -32601, "message": "Method not found"}})),
        Some(json!({"error": {"code": -32600, "message": "Experimental API unavailable"}})),
        Some(json!({"error": {"code": -32603, "message": "Discovery failed"}})),
        Some(json!({"result": {"data": []}})),
        Some(json!({"result": {"data": "invalid"}})),
        None,
    ] {
        let codex_home = tempfile::tempdir()?;
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await?;
        config.model = Some("task-model".into());
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut socket = tokio_tungstenite::accept_async(stream).await?;
            let mut catalog_requests = 0;
            while let Some(Ok(Message::Text(text))) = socket.next().await {
                let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
                    continue;
                };
                let mut reply = match request.method.as_str() {
                    "initialize" => json!({"result": {"userAgent": "catalog-test/1.0.0"}}),
                    "account/read" => {
                        json!({"result": {"account": null, "requiresOpenaiAuth": false}})
                    }
                    "model/list" => json!({"result": {"data": [], "nextCursor": null}}),
                    "configRequirements/read" => json!({"result": {"requirements": null}}),
                    "collaborationMode/list" => {
                        assert_eq!(request.params, Some(json!({})));
                        catalog_requests += 1;
                        if catalog_requests == 1 {
                            let Some(reply) = initial_reply.clone() else {
                                continue;
                            };
                            reply
                        } else {
                            json!({"result": {"data": [{"name": "Server plan", "mode": "plan", "model": "server-model", "reasoning_effort": "high"}]}})
                        }
                    }
                    method => panic!("unexpected request: {method}"),
                };
                reply["id"] = json!(request.id);
                socket.send(Message::Text(reply.to_string().into())).await?;
            }
            Ok::<_, color_eyre::Report>(catalog_requests)
        });
        let mut session = AppServerSession::new(
            crate::backend_id::BackendId::new(),
            crate::connect_remote_app_server(endpoint).await?,
            ThreadParamsMode::Remote,
        );
        let initial = session.bootstrap(&config).await?;
        assert_eq!(
            (initial.default_model, initial.collaboration_modes),
            (Some("task-model".into()), vec![])
        );
        let refreshed = session.bootstrap(&config).await?;
        assert_eq!(
            refreshed.collaboration_modes,
            vec![CollaborationModeMask {
                name: "Server plan".into(),
                mode: Some(ModeKind::Plan),
                model: Some("server-model".into()),
                reasoning_effort: Some(Some(ReasoningEffort::High)),
                developer_instructions: Some(None),
            }]
        );
        session.shutdown().await?;
        assert_eq!(server.await??, 2);
    }
    Ok(())
}
