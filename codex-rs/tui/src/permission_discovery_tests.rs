use super::*;
use crate::legacy_core::config::ConfigBuilder;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn discovery_preserves_config_scope_and_bounds_server_requests() {
    let home = tempfile::tempdir().unwrap();
    for case in [
        "local",
        "session-only",
        "remote",
        "remote-default",
        "thread",
        "legacy",
        "empty",
        "unsupported",
        "config-error",
        "cycle",
        "limit",
        "timeout",
    ] {
        let mode = if matches!(case, "local" | "session-only" | "timeout") {
            ThreadParamsMode::Embedded
        } else {
            ThreadParamsMode::Remote
        };
        let overrides = if case == "session-only" {
            vec![
                (
                    "permissions.session-only.extends".into(),
                    ":workspace".into(),
                ),
                ("default_permissions".into(), "session-only".into()),
            ]
        } else {
            Vec::new()
        };
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .cli_overrides(overrides)
            .build()
            .await
            .unwrap();
        let thread_cwd = (case == "thread").then_some(std::path::Path::new("/thread"));
        let remote_cwd = (case != "remote-default").then_some("/server");
        let cwd = thread_cwd.map(|path| path.to_str().unwrap()).or_else(|| {
            if mode == ThreadParamsMode::Embedded {
                config.cwd.to_str()
            } else {
                remote_cwd
            }
        });
        let data = json!([
            {"id": ":workspace", "description": null, "allowed": true},
            {"id": "remote-profile", "description": "Server description", "allowed": true}
        ]);
        let requirements = json!({"allowedPermissionProfiles": {"remote-profile": true}});
        let mut replies = vec![json!({"result": {"requirements": requirements}})];
        replies.extend(match case {
            "empty" => vec![json!({"result": {"data": [], "nextCursor": null}})],
            "unsupported" => vec![json!({"error": {"code": -32601, "message": "Unknown method"}})],
            "config-error" => {
                vec![json!({"error": {"code": -32603, "message": "Invalid project config"}})]
            }
            "cycle" => vec![json!({"result": {"data": [], "nextCursor": "repeat"}}); 2],
            "limit" => (0..10)
                .map(|i| json!({"result": {"data": [], "nextCursor": i.to_string()}}))
                .collect(),
            _ => vec![
                json!({"result": {"data": [data[0]], "nextCursor": "second"}}),
                json!({"result": {"data": [data[1]], "nextCursor": null}}),
            ],
        });
        if case == "timeout" {
            replies = vec![Value::Null];
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let websocket_url = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let mut replies = replies.into_iter();
            let mut requests = Vec::new();
            while let Some(Ok(Message::Text(text))) = socket.next().await {
                let request: Value = serde_json::from_str(&text).unwrap();
                if request["id"].is_null() {
                    continue;
                }
                let mut response = if request["method"] == "initialize" {
                    json!({"result": {"userAgent": "permission-discovery-test", "platformFamily": "unix", "platformOs": "linux"}})
                } else {
                    requests.push(request.clone());
                    replies.next().expect("unexpected discovery request")
                };
                if response.is_null() {
                    continue;
                }
                response["id"] = request["id"].clone();
                socket
                    .send(Message::Text(response.to_string().into()))
                    .await
                    .unwrap();
            }
            requests
        });
        let client = crate::connect_remote_app_server(crate::RemoteAppServerEndpoint::WebSocket {
            websocket_url,
            auth_token: None,
        })
        .await
        .unwrap();
        let session = AppServerSession::new(client, mode)
            .with_remote_cwd_override(remote_cwd.map(std::path::PathBuf::from));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        if case == "timeout" {
            tokio::time::pause();
        } // Keep real time for the socket handshake.
        for attempt in 0..if case == "timeout" { 4 } else { 1 } {
            fetch(
                &session,
                Uuid::new_v4(),
                &config,
                thread_cwd,
                AppEventSender::new(tx.clone()),
            );
            let AppEvent::PermissionProfilesLoaded { result, .. } = rx.recv().await.unwrap() else {
                panic!("expected discovery result")
            };
            let error = match case {
                "unsupported" => Some("Upgrade"),
                "config-error" => Some("Invalid project config"),
                "cycle" | "limit" => Some("pagination limit"),
                "timeout" if attempt == 0 => Some("timed out"),
                "timeout" => Some("duplicate"),
                _ => None,
            };
            if let Some(error) = error {
                assert!(result.unwrap_err().contains(error));
                continue;
            }
            let discovery = result.unwrap();
            if case == "empty" {
                assert!(discovery.profiles.is_empty());
            } else {
                assert_eq!(serde_json::to_value(&discovery.profiles).unwrap(), data);
                assert_eq!(
                    discovery.requirements,
                    Some(serde_json::from_value(requirements.clone()).unwrap())
                );
            }
        }
        session.shutdown().await.unwrap();
        let requests = server.await.unwrap();
        match case {
            "timeout" => assert_eq!(requests.len(), 1),
            "local" | "session-only" | "remote" | "remote-default" | "thread" | "legacy" => {
                let pages: Vec<_> = requests
                    .iter()
                    .filter(|request| request["method"] == "permissionProfile/list")
                    .map(|request| request["params"].clone())
                    .collect();
                assert_eq!(
                    pages,
                    vec![
                        json!({"cwd": cwd, "limit": 100, "cursor": null}),
                        json!({"cwd": cwd, "limit": 100, "cursor": "second"})
                    ]
                );
            }
            _ => {}
        }
        if case == "timeout" {
            tokio::time::resume();
        }
    }
}
