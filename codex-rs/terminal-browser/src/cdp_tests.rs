use std::time::Duration;

use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use super::CdpClient;
use super::CdpEvent;
use super::PipeFrameDecoder;

const TEST_PIPE_CAPACITY: usize = 64 * 1024;

async fn test_server(
    handler: impl FnOnce(tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>) -> JoinHandle<()>
    + Send
    + 'static,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test websocket");
    let address = listener.local_addr().expect("test websocket address");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept test websocket");
        let socket = accept_async(stream).await.expect("upgrade test websocket");
        handler(socket).await.expect("test handler task");
    });
    (format!("ws://{address}"), server)
}

async fn receive_request(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
) -> Value {
    let Message::Text(text) = socket
        .next()
        .await
        .expect("request message")
        .expect("valid request message")
    else {
        panic!("expected text request");
    };
    serde_json::from_str(text.as_str()).expect("decode request")
}

async fn respond(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    request: &Value,
    result: Value,
) {
    socket
        .send(Message::Text(
            json!({ "id": request["id"], "result": result })
                .to_string()
                .into(),
        ))
        .await
        .expect("send response");
}

async fn complete_initialization(
    socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
) {
    for expected_method in [
        "Page.enable",
        "Runtime.enable",
        "DOM.enable",
        "Accessibility.enable",
        "Page.setLifecycleEventsEnabled",
    ] {
        let request = receive_request(socket).await;
        assert_eq!(request["method"], expected_method);
        respond(socket, &request, json!({})).await;
    }
}

#[tokio::test]
async fn lifecycle_messages_are_broadcast_without_leaking_unknown_responses() {
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            complete_initialization(&mut socket).await;
            let request = receive_request(&mut socket).await;
            socket
                .send(Message::Text(
                    json!({ "method": "Page.loadEventFired", "params": { "timestamp": 1 } })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send notification");
            socket
                .send(Message::Text(
                    json!({ "id": 999, "result": { "late": true } })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send unknown response");
            respond(&mut socket, &request, json!({ "ok": true })).await;
        })
    })
    .await;
    let client = CdpClient::connect(&url).await.expect("connect client");
    let mut events = client.subscribe_events();

    let result = client
        .call("Test.command", json!({}))
        .await
        .expect("command response");

    assert_eq!(result, json!({ "ok": true }));
    assert_eq!(
        events.recv().await.expect("notification"),
        CdpEvent::Message(json!({ "method": "Page.loadEventFired", "params": { "timestamp": 1 } }))
    );
    assert!(matches!(
        events.recv().await.expect("disconnect event"),
        CdpEvent::Disconnected(_)
    ));
    server.await.expect("server task");
}

#[tokio::test]
async fn cdp_errors_keep_the_method_name() {
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            complete_initialization(&mut socket).await;
            let request = receive_request(&mut socket).await;
            socket
                .send(Message::Text(
                    json!({ "id": request["id"], "error": { "code": -1, "message": "nope" } })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send error response");
        })
    })
    .await;
    let client = CdpClient::connect(&url).await.expect("connect client");

    let error = client
        .call("Test.failure", json!({}))
        .await
        .expect_err("command should fail");

    assert!(error.to_string().contains("CDP Test.failure failed"));
    server.await.expect("server task");
}

#[tokio::test]
async fn disconnect_fails_pending_calls_and_emits_an_event() {
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            complete_initialization(&mut socket).await;
            let _request = receive_request(&mut socket).await;
            socket.close(None).await.expect("close websocket");
        })
    })
    .await;
    let client = CdpClient::connect(&url).await.expect("connect client");
    let mut events = client.subscribe_events();

    let error = client
        .call("Test.disconnect", json!({}))
        .await
        .expect_err("disconnect should fail the call");

    assert!(error.to_string().contains("closed the DevTools connection"));
    assert!(matches!(
        events.recv().await.expect("disconnect event"),
        CdpEvent::Disconnected(_)
    ));
    server.await.expect("server task");
}

#[tokio::test]
async fn late_responses_after_timeout_are_discarded() {
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            complete_initialization(&mut socket).await;
            let request = receive_request(&mut socket).await;
            release_rx.await.expect("release late response");
            respond(&mut socket, &request, json!({ "late": true })).await;
        })
    })
    .await;
    let client = CdpClient::connect_with_call_timeout(&url, Duration::from_millis(20))
        .await
        .expect("connect client");
    let mut events = client.subscribe_events();

    let error = client
        .call("Test.timeout", json!({}))
        .await
        .expect_err("command should time out");
    assert!(error.to_string().contains("timed out waiting for CDP"));
    release_tx.send(()).expect("release server response");
    assert!(matches!(
        events.recv().await.expect("disconnect event"),
        CdpEvent::Disconnected(_)
    ));
    server.await.expect("server task");
}

#[tokio::test]
async fn cancelling_a_call_removes_its_pending_entry() {
    let (seen_tx, seen_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            complete_initialization(&mut socket).await;
            let _request = receive_request(&mut socket).await;
            seen_tx.send(()).expect("mark request seen");
            release_rx.await.expect("release server");
        })
    })
    .await;
    let client = CdpClient::connect(&url).await.expect("connect client");
    let caller = {
        let client = client.clone();
        tokio::spawn(async move { client.call("Test.cancel", json!({})).await })
    };
    seen_rx.await.expect("request seen");

    caller.abort();
    let _ = caller.await;

    assert!(
        client
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
    release_tx.send(()).expect("release server");
    server.await.expect("server task");
}

#[tokio::test]
async fn concurrent_calls_match_reverse_order_responses() {
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            complete_initialization(&mut socket).await;
            let first = receive_request(&mut socket).await;
            let second = receive_request(&mut socket).await;
            respond(&mut socket, &second, json!({ "order": 2 })).await;
            socket
                .send(Message::Text(
                    json!({ "method": "Page.loadEventFired", "params": { "timestamp": 1 } })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("send lifecycle event");
            respond(&mut socket, &first, json!({ "order": 1 })).await;
        })
    })
    .await;
    let client = CdpClient::connect(&url).await.expect("connect client");
    let mut events = client.subscribe_events();

    let (first, second) = tokio::join!(
        client.call("Test.first", json!({})),
        client.call("Test.second", json!({}))
    );

    assert_eq!(first.expect("first response"), json!({ "order": 1 }));
    assert_eq!(second.expect("second response"), json!({ "order": 2 }));
    assert!(matches!(
        events.recv().await.expect("lifecycle event"),
        CdpEvent::Message(_)
    ));
    server.await.expect("server task");
}

async fn receive_pipe_request(reader: &mut (impl AsyncRead + Unpin)) -> Value {
    let mut frame = Vec::new();
    loop {
        let byte = reader.read_u8().await.expect("read pipe request");
        if byte == 0 {
            return serde_json::from_slice(&frame).expect("decode pipe request");
        }
        frame.push(byte);
    }
}

fn framed_pipe_message(message: &Value) -> Vec<u8> {
    let mut frame = serde_json::to_vec(message).expect("encode pipe message");
    frame.push(/*value*/ 0);
    frame
}

async fn respond_pipe(writer: &mut (impl AsyncWrite + Unpin), request: &Value, result: Value) {
    let mut response = json!({ "id": request["id"], "result": result });
    if let Some(session_id) = request.get("sessionId") {
        response["sessionId"] = session_id.clone();
    }
    writer
        .write_all(&framed_pipe_message(&response))
        .await
        .expect("write pipe response");
}

async fn complete_pipe_initialization(
    reader: &mut (impl AsyncRead + Unpin),
    writer: &mut (impl AsyncWrite + Unpin),
) {
    let targets = receive_pipe_request(reader).await;
    assert_eq!(targets["method"], "Target.getTargets");
    assert_eq!(targets.get("sessionId"), None);
    let response = json!({
        "id": targets["id"],
        "result": {
            "targetInfos": [{
                "targetId": "page-target",
                "type": "page",
                "title": "Pipe page"
            }]
        }
    });
    let response = framed_pipe_message(&response);
    let split = response.len() / 2;
    writer
        .write_all(&response[..split])
        .await
        .expect("write first response fragment");
    tokio::task::yield_now().await;
    writer
        .write_all(&response[split..])
        .await
        .expect("write second response fragment");

    let attach = receive_pipe_request(reader).await;
    assert_eq!(attach["method"], "Target.attachToTarget");
    assert_eq!(attach["params"]["targetId"], "page-target");
    assert_eq!(attach["params"]["flatten"], true);
    assert_eq!(attach.get("sessionId"), None);
    respond_pipe(writer, &attach, json!({ "sessionId": "page-session" })).await;

    for expected_method in [
        "Page.enable",
        "Runtime.enable",
        "DOM.enable",
        "Accessibility.enable",
        "Page.setLifecycleEventsEnabled",
    ] {
        let request = receive_pipe_request(reader).await;
        assert_eq!(request["method"], expected_method);
        assert_eq!(request["sessionId"], "page-session");
        respond_pipe(writer, &request, json!({})).await;
    }
}

#[tokio::test]
async fn pipe_transport_attaches_and_routes_page_messages_by_session() {
    let (client_pipe, server_pipe) = tokio::io::duplex(TEST_PIPE_CAPACITY);
    let (client_read, client_write) = tokio::io::split(client_pipe);
    let (mut server_read, mut server_write) = tokio::io::split(server_pipe);
    let server = tokio::spawn(async move {
        complete_pipe_initialization(&mut server_read, &mut server_write).await;
        let browser_request = receive_pipe_request(&mut server_read).await;
        assert_eq!(browser_request["method"], "Test.browserCommand");
        assert_eq!(browser_request.get("sessionId"), None);
        respond_pipe(&mut server_write, &browser_request, json!({})).await;

        let request = receive_pipe_request(&mut server_read).await;
        assert_eq!(request["method"], "Test.command");
        assert_eq!(request["sessionId"], "page-session");

        let messages = [
            json!({
                "method": "Page.loadEventFired",
                "params": { "timestamp": 1 },
                "sessionId": "another-session"
            }),
            json!({
                "method": "Page.loadEventFired",
                "params": { "timestamp": 2 },
                "sessionId": "page-session"
            }),
            json!({
                "id": request["id"],
                "result": { "ok": true },
                "sessionId": "page-session"
            }),
        ];
        let mut frames = Vec::new();
        for message in messages {
            frames.extend(framed_pipe_message(&message));
        }
        server_write
            .write_all(&frames)
            .await
            .expect("write combined pipe frames");
    });
    let connection =
        CdpClient::connect_pipe_io(client_read, client_write, Duration::from_secs(/*secs*/ 5))
            .await
            .expect("connect pipe client");
    assert_eq!(connection.title, "Pipe page");
    let mut events = connection.client.subscribe_events();
    connection
        .client
        .call_browser("Test.browserCommand", json!({}))
        .await
        .expect("browser-target pipe command");

    let result = connection
        .client
        .call("Test.command", json!({}))
        .await
        .expect("pipe command response");

    assert_eq!(result, json!({ "ok": true }));
    assert_eq!(
        events.recv().await.expect("page-session lifecycle event"),
        CdpEvent::Message(json!({
            "method": "Page.loadEventFired",
            "params": { "timestamp": 2 },
            "sessionId": "page-session"
        }))
    );
    server.await.expect("server task");
}

#[test]
fn pipe_frame_decoder_handles_fragments_and_enforces_its_limit() {
    let mut decoder = PipeFrameDecoder::new(/*max_frame_bytes*/ 8);

    assert_eq!(
        decoder.push(b"{\"id\"").expect("first fragment"),
        Vec::<Value>::new()
    );
    assert_eq!(
        decoder.push(b":1}\0{\"id\":2}\0").expect("complete frames"),
        vec![json!({ "id": 1 }), json!({ "id": 2 })]
    );

    let mut decoder = PipeFrameDecoder::new(/*max_frame_bytes*/ 4);
    let error = decoder
        .push(b"12345")
        .expect_err("oversized frame must fail");
    assert!(error.to_string().contains("4-byte frame limit"));
}
