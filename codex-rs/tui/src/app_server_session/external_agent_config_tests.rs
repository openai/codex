//! Exercises migration over shared transports with interleaved import completions.

use super::*;
use crate::external_agent_config_migration::flow::ExternalAgentConfigMigrationFlowOutcome;
use crate::external_agent_config_migration::flow::handle_external_agent_config_migration_prompt;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::ServerNotification;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn external_agent_config_migration_over_shared_transport() -> Result<()> {
    for mode in [ThreadParamsMode::Embedded, ThreadParamsMode::Remote] {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut socket = tokio_tungstenite::accept_async(stream).await?;
            let mut requests = Vec::new();
            let mut import_attempts = 0;
            while let Some(Ok(Message::Text(text))) = socket.next().await {
                let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
                    continue;
                };
                let mut reply = match request.method.as_str() {
                    "initialize" => json!({"result": {"userAgent": "import-test/1.0"}}),
                    "externalAgentConfig/detect" => {
                        requests.push((request.method, request.params.unwrap()));
                        json!({"result": {"items": [], "connectors": []}})
                    }
                    "externalAgentConfig/import" => {
                        requests.push((request.method, request.params.unwrap()));
                        import_attempts += 1;
                        if import_attempts == 1 {
                            json!({"error": {"code": -32603, "message": "retry import"}})
                        } else {
                            // A fast completion can already be queued when the RPC returns.
                            for import_id in ["other-client", "this-client"] {
                                let notification = json!({
                                    "method": "externalAgentConfig/import/completed",
                                    "params": {"importId": import_id, "itemTypeResults": []}
                                });
                                socket
                                    .send(Message::Text(notification.to_string().into()))
                                    .await?;
                            }
                            json!({"result": {"importId": "this-client"}})
                        }
                    }
                    method => panic!("unexpected request: {method}"),
                };
                reply["id"] = json!(request.id);
                socket.send(Message::Text(reply.to_string().into())).await?;
            }
            Ok::<_, color_eyre::Report>(requests)
        });
        let mut session =
            AppServerSession::new(crate::connect_remote_app_server(endpoint).await?, mode);
        let workspace = tempfile::tempdir()?;
        let cwd = workspace.path();
        let mut tui = crate::tui::test_support::make_test_tui()?;
        // Exercise the actual /import entry point, including remote home-only detection.
        for cwd in [Some(cwd), None] {
            assert!(matches!(
                handle_external_agent_config_migration_prompt(&mut tui, &mut session, cwd)
                    .await
                    .map_err(|err| color_eyre::eyre::eyre!(err))?,
                ExternalAgentConfigMigrationFlowOutcome::NoItems
            ));
        }
        let items = json!([
            {"itemType": "SKILLS", "description": "Skills", "cwd": null, "details": null},
            {"itemType": "PLUGINS", "description": "Plugins", "cwd": null, "details": null},
            {"itemType": "MCP_SERVER_CONFIG", "description": "MCP", "cwd": null, "details": null}
        ]);
        let source = "claude-code";
        assert!(
            session
                .external_agent_config_import(serde_json::from_value(items.clone())?, source.into())
                .await
                .is_err()
        );
        assert!(!session.external_agent_config_import_in_progress());
        session
            .external_agent_config_import(serde_json::from_value(items.clone())?, source.into())
            .await?;
        assert!(session.external_agent_config_import_in_progress());
        let err = session
            .external_agent_config_import(serde_json::from_value(items.clone())?, source.into())
            .await
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            EXTERNAL_AGENT_CONFIG_IMPORT_IN_PROGRESS_MESSAGE
        );
        for (import_id, consumed) in [("other-client", false), ("this-client", true)] {
            let event =
                tokio::time::timeout(Duration::from_secs(/*secs*/ 5), session.next_event()).await?;
            let Some(AppServerEvent::ServerNotification(notification)) = event else {
                panic!("expected import completion");
            };
            let ServerNotification::ExternalAgentConfigImportCompleted(notification) =
                *notification
            else {
                panic!("expected import completion");
            };
            assert_eq!(notification.import_id, import_id);
            assert_eq!(
                session.consume_external_agent_config_import_completion(import_id),
                consumed
            );
            assert_eq!(
                session.external_agent_config_import_in_progress(),
                !consumed
            );
        }
        assert!(!session.consume_external_agent_config_import_completion("this-client"));
        session.shutdown().await?;
        let mut expected = Vec::new();
        for cwds in [json!([cwd]), json!(null)] {
            for source in ["claude-code", "cursor"] {
                expected.push((
                    "externalAgentConfig/detect".to_string(),
                    json!({
                        "includeHome": true, "cwds": cwds, "maxSessionAgeDays": null,
                        "maxSessions": null, "source": null, "migrationSource": source
                    }),
                ));
            }
        }
        for _ in 0..2 {
            expected.push((
                "externalAgentConfig/import".to_string(),
                json!({
                    "migrationItems": items, "source": "cli", "providerId": source,
                    "migrationSource": source
                }),
            ));
        }
        assert_eq!(server.await??, expected);
    }
    Ok(())
}
