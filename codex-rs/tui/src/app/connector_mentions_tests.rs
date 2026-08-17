use super::*;
use codex_app_server_protocol::AppsListParams;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MockResponses {
    FirstBatchFails,
    Unsupported,
}

async fn load_mock_mentions(responses: MockResponses) -> Result<(ConnectorsSnapshot, Vec<usize>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut websocket = accept_async(stream).await?;
        let mut read_batches = Vec::new();

        while let Some(frame) = websocket.next().await {
            let Message::Text(text) = frame? else {
                continue;
            };
            let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
                continue;
            };
            let request_id = request.id.clone();
            let response = match request.method.as_str() {
                "initialize" => {
                    json!({ "id": request_id, "result": { "userAgent": "codex-tui-test" } })
                }
                "app/installed" => {
                    assert_eq!(
                        request.params,
                        Some(json!({ "threadId": "thread-1", "forceRefresh": true }))
                    );
                    if responses == MockResponses::Unsupported {
                        let message = "Invalid request: unknown variant `app/installed`";
                        json!({ "id": request_id, "error": { "code": -32600, "message": message } })
                    } else {
                        let mut apps = (0..103)
                            .map(|index| json!({
                                "id": format!("app-{index:03}"),
                                "runtimeName": (index != 102).then(|| format!("Runtime {index:03}")),
                                "enabled": true,
                                "callable": true,
                            }))
                            .collect::<Vec<_>>();
                        apps.extend([
                            json!({ "id": "disabled", "enabled": false, "callable": false }),
                            json!({ "id": "no-visible-tools", "enabled": true, "callable": false }),
                        ]);
                        json!({ "id": request_id, "result": { "apps": apps } })
                    }
                }
                "app/list" => {
                    let params = serde_json::from_value::<AppsListParams>(
                        request.params.expect("app/list params"),
                    )?;
                    assert_eq!(
                        (params.thread_id.as_deref(), params.force_refetch),
                        (Some("thread-1"), true)
                    );
                    json!({
                        "id": request_id,
                        "result": {
                            "data": [
                                { "id": "installed", "name": "Installed", "isAccessible": true },
                                { "id": "disabled", "name": "Disabled", "isAccessible": true, "isEnabled": false },
                                { "id": "directory", "name": "Directory", "isAccessible": false }
                            ],
                            "nextCursor": null
                        }
                    })
                }
                "app/read" => {
                    let params = serde_json::from_value::<AppsReadParams>(
                        request.params.expect("app/read params"),
                    )?;
                    assert_eq!(params.thread_id.as_deref(), Some("thread-1"));
                    assert!(!params.include_tools);
                    assert!(params.app_ids.iter().all(|id| id.starts_with("app-")));
                    read_batches.push(params.app_ids.len());

                    if responses == MockResponses::FirstBatchFails && read_batches.len() == 1 {
                        json!({
                            "id": request_id,
                            "error": { "code": -32603, "message": "metadata service unavailable" }
                        })
                    } else {
                        let apps = params
                            .app_ids
                            .into_iter()
                            .filter(|id| id != "app-102")
                            .map(|id| json!({ "id": id, "name": format!("Connector {id}") }))
                            .collect::<Vec<_>>();
                        json!({
                            "id": request_id,
                            "result": { "apps": apps, "missingAppIds": ["app-102"] }
                        })
                    }
                }
                method => panic!("unexpected app-server request: {method}"),
            };
            websocket
                .send(Message::Text(response.to_string().into()))
                .await?;
        }

        Ok::<_, anyhow::Error>(read_batches)
    });
    let client = crate::connect_remote_app_server(
        codex_app_server_client::RemoteAppServerEndpoint::WebSocket {
            websocket_url,
            auth_token: None,
        },
    )
    .await
    .expect("connect to mock app server");

    let snapshot = fetch_installed_connector_mentions(
        client.request_handle(),
        Some("thread-1".to_string()),
        /*force_refresh*/ true,
    )
    .await?;
    client.shutdown().await?;
    Ok((snapshot, server.await??))
}

#[tokio::test]
async fn installed_connector_mentions_read_callable_apps_in_bounded_thread_scoped_batches()
-> Result<()> {
    let (snapshot, read_batches) = load_mock_mentions(MockResponses::FirstBatchFails).await?;
    assert_eq!(read_batches, vec![100, 3]);
    assert_eq!(snapshot.connectors.len(), 105);
    assert!(snapshot.connectors[103..].iter().all(|app| !app.is_enabled));
    let selected_apps = snapshot
        .connectors
        .iter()
        .filter(|app| ["app-000", "app-100", "app-102"].contains(&app.id.as_str()))
        .map(|app| app.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        selected_apps,
        ["Connector app-100", "Runtime 000", "app-102"]
    );
    Ok(())
}

#[tokio::test]
async fn installed_connector_mentions_fall_back_for_legacy_app_servers() -> Result<()> {
    let (snapshot, read_batches) = load_mock_mentions(MockResponses::Unsupported).await?;

    assert!(read_batches.is_empty());
    assert_eq!(
        snapshot.connectors,
        serde_json::from_value::<Vec<AppInfo>>(json!([
            { "id": "installed", "name": "Installed", "isAccessible": true },
            { "id": "disabled", "name": "Disabled", "isAccessible": true, "isEnabled": false }
        ]))?
    );
    Ok(())
}
