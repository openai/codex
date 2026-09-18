use super::*;
use codex_app_server_client::RemoteAppServerClient;
use codex_app_server_client::RemoteAppServerConnectArgs;
use codex_app_server_client::RemoteAppServerEndpoint;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn experimental_features_rpc_paginates_thread_config_and_bounds_bad_servers() {
    for scenario in ["pages", "cycle", "limit", "error", "timeout"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let thread_id = ThreadId::new();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let mut page = 0;
            while let Some(Ok(Message::Text(text))) = socket.next().await {
                let JSONRPCMessage::Request(request) = serde_json::from_str(&text).unwrap() else {
                    continue;
                };
                let result = if request.method == "initialize" {
                    json!({"userAgent": "experimental-test"})
                } else {
                    assert_eq!(request.method, "experimentalFeature/list");
                    assert_eq!(
                        request.params,
                        Some(json!({
                            "cursor": if page == 0 { None } else { Some(page.to_string()) },
                            "limit": 100, "threadId": thread_id.to_string()
                        }))
                    );
                    page += 1;
                    if scenario == "timeout" {
                        // Leave the connection open without answering discovery.
                        continue;
                    }
                    if scenario == "error" {
                        socket.send(Message::Text(json!({"id": request.id, "error": {"code": -32601, "message": "method not found", "data": "private wire content"}}).to_string().into())).await.unwrap();
                        continue;
                    }
                    json!({"data": [{
                        "name": format!("feature-{page}"), "stage": "beta", "displayName": "Server experiment",
                        "description": "Server description", "announcement": null,
                        "enabled": true, "defaultEnabled": false
                    }], "nextCursor": match scenario {
                        "pages" if page == 2 => None,
                        "cycle" => Some("1".to_string()),
                        _ => Some(page.to_string()),
                    }})
                };
                socket
                    .send(Message::Text(
                        json!({"id": request.id, "result": result})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
            }
            page
        });
        let client = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
            endpoint: RemoteAppServerEndpoint::WebSocket {
                websocket_url: url,
                auth_token: None,
            },
            client_name: "experimental-test".to_string(),
            client_version: "0.0.0".to_string(),
            experimental_api: true,
            mcp_server_openai_form_elicitation: false,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: 8,
        })
        .await
        .unwrap();
        let (tx, rx) = oneshot::channel();
        fetch(
            AppServerRequestHandle::Remote(client.request_handle()),
            Some(thread_id),
            "tui-experimental-features",
            tx,
        );
        let result = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), rx)
            .await
            .unwrap()
            .unwrap();
        if scenario == "pages" {
            let features = result.unwrap();
            assert_eq!(
                features,
                ["feature-1", "feature-2"]
                    .into_iter()
                    .map(|name| ExperimentalFeature {
                        name: name.to_string(),
                        stage: codex_app_server_protocol::ExperimentalFeatureStage::Beta,
                        display_name: Some("Server experiment".to_string()),
                        description: Some("Server description".to_string()),
                        announcement: None,
                        enabled: true,
                        default_enabled: false,
                    })
                    .collect::<Vec<_>>()
            );
        } else {
            let error = result.unwrap_err();
            assert_eq!(
                error,
                match scenario {
                    "cycle" => "Experimental feature pagination repeated a cursor",
                    "limit" => "Experimental feature discovery exceeded 10 pages",
                    "error" => "Experimental feature request failed",
                    "timeout" => "Experimental feature discovery timed out",
                    _ => unreachable!(),
                }
            );
        }
        if scenario == "timeout" {
            // Reopening while the abandoned request is still unanswered cannot
            // accumulate more pending requests or forward more calls to the server.
            for _ in 0..3 {
                let (tx, rx) = oneshot::channel();
                fetch(
                    AppServerRequestHandle::Remote(client.request_handle()),
                    Some(thread_id),
                    "tui-experimental-features",
                    tx,
                );
                assert_eq!(
                    rx.await.unwrap(),
                    Err("Experimental feature request failed".to_string())
                );
            }
        }
        client.shutdown().await.unwrap();
        assert_eq!(
            server.await.unwrap(),
            match scenario {
                "pages" | "cycle" => 2,
                "limit" => 10,
                _ => 1,
            }
        );
    }
}

#[tokio::test]
async fn experimental_feature_writes_use_server_defaults_and_refresh_configured_values() {
    for scenario in [
        "overridden",
        "write_error",
        "write_timeout",
        "readback_error",
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let thread_id = ThreadId::new();
        let selected_file = std::env::temp_dir().join("selected.config.toml");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let mut wrote = false;
            while let Some(Ok(Message::Text(text))) = socket.next().await {
                let JSONRPCMessage::Request(request) = serde_json::from_str(&text).unwrap() else {
                    continue;
                };
                let fail = (request.method == "config/batchWrite" && scenario == "write_error")
                    || (request.method == "experimentalFeature/list"
                        && wrote
                        && scenario == "readback_error");
                if fail {
                    socket.send(Message::Text(json!({"id": request.id, "error": {"code": -32600, "message": "rejected", "data": "private diagnostic"}}).to_string().into())).await.unwrap();
                    continue;
                }
                let result = match request.method.as_str() {
                    "initialize" => json!({"userAgent": "experimental-test"}),
                    "experimentalFeature/list" => {
                        json!({"data": [
                            {"name": "daemon_auto_start", "stage": "beta", "displayName": null, "description": null, "announcement": null, "enabled": !wrote || scenario == "overridden", "defaultEnabled": false},
                            {"name": "default_off", "stage": "beta", "displayName": null, "description": null, "announcement": null, "enabled": !wrote || scenario == "overridden", "defaultEnabled": false},
                            {"name": "default_on", "stage": "beta", "displayName": null, "description": null, "announcement": null, "enabled": !wrote, "defaultEnabled": true},
                            {"name": "new_server_feature", "stage": "beta", "displayName": null, "description": null, "announcement": null, "enabled": wrote, "defaultEnabled": false}
                        ], "nextCursor": null})
                    }
                    "config/batchWrite" => {
                        assert_eq!(
                            request.params,
                            Some(json!({
                                "edits": [
                                    {"keyPath": "features.\"daemon_auto_start\"", "value": false, "mergeStrategy": "replace"},
                                    {"keyPath": "features.\"default_off\"", "value": null, "mergeStrategy": "replace"},
                                    {"keyPath": "features.\"default_on\"", "value": false, "mergeStrategy": "replace"},
                                    {"keyPath": "features.\"new_server_feature\"", "value": true, "mergeStrategy": "replace"}
                                ], "filePath": null, "expectedVersion": null, "reloadUserConfig": true
                            }))
                        );
                        wrote = true;
                        if scenario == "write_timeout" {
                            tokio::time::pause();
                            continue;
                        }
                        json!({"status": "ok", "version": "v2", "filePath": selected_file, "overriddenMetadata": null})
                    }
                    other => panic!("unexpected RPC: {other}"),
                };
                socket
                    .send(Message::Text(
                        json!({"id": request.id, "result": result})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
            }
        });
        let client = crate::connect_remote_app_server(RemoteAppServerEndpoint::WebSocket {
            websocket_url: url,
            auth_token: None,
        })
        .await
        .unwrap();
        let handle = client.request_handle();
        let result = write(
            handle,
            thread_id,
            vec![
                ("daemon_auto_start".to_string(), false),
                ("default_off".to_string(), false),
                ("default_on".to_string(), false),
                ("new_server_feature".to_string(), true),
            ],
        )
        .await;
        if scenario == "write_timeout" {
            tokio::time::resume();
        }
        match scenario {
            "overridden" => assert!(result.unwrap().warning.is_some()),
            scenario => {
                let error = result.unwrap_err();
                assert!(error.contains(match scenario {
                    "readback_error" => "saved, but configured values could not be refreshed",
                    "write_timeout" => "write may still finish",
                    _ => "Failed to save experimental features.",
                }));
            }
        }
        let _ = client.shutdown().await;
        server.await.unwrap();
    }
}
