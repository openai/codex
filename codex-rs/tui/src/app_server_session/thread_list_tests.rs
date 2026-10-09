//! Public-RPC compatibility keeps modern grouping and narrows only legacy array errors.

use super::*;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn thread_list_preserves_filters_and_retries_only_legacy_cwd_errors() -> Result<()> {
    let legacy_message = "invalid type: sequence, expected a string";
    for (code, message, retry) in [
        (0, "", false),
        (-32600, legacy_message, true),
        (-32602, legacy_message, true),
        (-32603, legacy_message, false),
        (-32602, "invalid cursor", false),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            let mut socket = tokio_tungstenite::accept_async(stream).await?;
            let mut requests = Vec::new();
            while let Some(Ok(Message::Text(text))) = socket.next().await {
                let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
                    continue;
                };
                let mut reply = match request.method.as_str() {
                    "initialize" => json!({"result": {"userAgent": "thread-list-test/1.0"}}),
                    "thread/list" => {
                        requests.push(request.params.unwrap());
                        if code != 0 && requests.len() == 1 {
                            json!({"error": {"code": code, "message": message}})
                        } else {
                            json!({"result": {"data": [], "nextCursor": "next"}})
                        }
                    }
                    method => panic!("unexpected request: {method}"),
                };
                reply["id"] = json!(request.id);
                socket.send(Message::Text(reply.to_string().into())).await?;
            }
            Ok::<_, color_eyre::Report>(requests)
        });
        let params: ThreadListParams = serde_json::from_value(json!({
            "cwd": ["/requested/nested", "/linked/nested"], "cursor": "cursor", "limit": 3,
            "sortKey": "created_at", "modelProviders": ["provider"], "archived": true,
            "searchTerm": "needle"
        }))?;
        let mut expected = vec![serde_json::to_value(&params)?];
        if retry {
            let mut fallback = expected[0].clone();
            fallback["cwd"] = json!("/requested/nested");
            expected.push(fallback);
        }
        let mut session = AppServerSession::new(
            crate::connect_remote_app_server(endpoint).await?,
            ThreadParamsMode::Embedded,
        );
        let result = session.thread_list(params).await;
        if code == 0 || retry {
            assert_eq!(
                serde_json::to_value(result?)?,
                json!({"data": [], "readStates": null, "nextCursor": "next", "backwardsCursor": null})
            );
        } else {
            assert!(format!("{:#}", result.unwrap_err()).contains(message));
        }
        session.shutdown().await?;
        assert_eq!(server.await??, expected);
    }
    Ok(())
}
