#![cfg(test)]

use super::*;
use crate::app_server_session::ThreadParamsMode;
use crate::backend_id::BackendId;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn remote_preview_requests_the_source_thread_and_validates_text() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
    let source = ThreadId::new();
    let text = "\tfirst\r\nsecond\n".repeat(80_000);
    let responses = [
        json!({"result": {"dataBase64": STANDARD.encode(&text)}}),
        json!({"error": {
            "code": -32603,
            "message": "No such file or directory (os error 2)",
            "data": {"detail": "private diagnostic marker"}
        }}),
        json!({"result": {"dataBase64": STANDARD.encode([0xff])}}),
        json!({"result": {"dataBase64": STANDARD.encode(b"bad\0")}}),
        json!({"result": {"dataBase64": STANDARD.encode("stale")}}),
    ];
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut socket = tokio_tungstenite::accept_async(stream).await?;
        let mut workspace = "/workspace";
        let mut responses = responses.into_iter();
        while let Some(Ok(Message::Text(wire))) = socket.next().await {
            let JSONRPCMessage::Request(request) = serde_json::from_str(&wire)? else {
                continue;
            };
            let mut reply = match request.method.as_str() {
                "initialize" => json!({"result": {"userAgent": "reader-test/1"}}),
                "thread/read" => {
                    let params: codex_app_server_protocol::ThreadReadParams =
                        serde_json::from_value(request.params.expect("thread params"))?;
                    assert_eq!(
                        params,
                        codex_app_server_protocol::ThreadReadParams {
                            thread_id: source.to_string(),
                            include_turns: false,
                        }
                    );
                    json!({"result": {"thread": {"environments": [
                        {"environmentId": "local", "cwd": workspace, "runtimeWorkspaceRoots": []}
                    ]}}})
                }
                "fs/readFile" => {
                    assert_eq!(request.params, Some(json!({"path": "/workspace/notes.md"})));
                    let reply = responses.next().expect("one preview request per case");
                    if responses.len() == 0 {
                        workspace = "/other";
                    }
                    reply
                }
                method => panic!("unexpected RPC: {method}"),
            };
            reply["id"] = json!(request.id);
            socket.send(Message::Text(reply.to_string().into())).await?;
        }
        assert!(responses.next().is_none());
        Ok::<_, color_eyre::Report>(())
    });
    let session = AppServerSession::new(
        BackendId::new(),
        crate::connect_remote_app_server(endpoint).await?,
        ThreadParamsMode::Remote,
    );
    let path = AppServerPath::from_app_server("notes.md");
    let resolved = session.resolve_thread_file(source, &path).await?;
    assert_eq!(session.read_document(source, &path, &resolved).await?, text);
    let error = session
        .read_document(source, &path, &resolved)
        .await
        .unwrap_err();
    let Some(TypedRequestError::Server {
        method,
        source: rpc_error,
    }) = error.downcast_ref::<TypedRequestError>()
    else {
        panic!("expected a typed filesystem error: {error:#}");
    };
    assert_eq!(method, "fs/readFile");
    assert_eq!(
        rpc_error,
        &JSONRPCErrorError {
            code: -32603,
            message: "No such file or directory (os error 2)".into(),
            data: None,
        }
    );
    assert!(!error.to_string().contains("private diagnostic marker"));
    for expected in [
        "Not a UTF-8 text file",
        "Not a text file",
        "Workspace changed",
    ] {
        let error = session
            .read_document(source, &path, &resolved)
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }
    session.shutdown().await?;
    server.await??;
    Ok(())
}
