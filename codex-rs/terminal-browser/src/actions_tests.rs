use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use super::BrowserToolOutput;
use super::dispatch_human_key;
use super::press;
use crate::accessibility::click;
use crate::accessibility::snapshot;
use crate::cdp::CdpClient;
use crate::handles::BrowserHandles;
use crate::input::BrowserInputModifiers;
use crate::input::BrowserKeyInput;
use crate::navigation::LoadState;
use crate::navigation::NavigateRequest;
use crate::navigation::NavigationAction;
use crate::navigation::navigate;
use crate::navigation::navigate_request;

type TestSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

async fn test_server(
    handler: impl FnOnce(TestSocket) -> JoinHandle<()> + Send + 'static,
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

async fn request(socket: &mut TestSocket) -> Value {
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

async fn respond(socket: &mut TestSocket, request: &Value, result: Value) {
    socket
        .send(Message::Text(
            json!({ "id": request["id"], "result": result })
                .to_string()
                .into(),
        ))
        .await
        .expect("send response");
}

async fn initialize(socket: &mut TestSocket) {
    for method in [
        "Page.enable",
        "Runtime.enable",
        "DOM.enable",
        "Accessibility.enable",
        "Page.setLifecycleEventsEnabled",
    ] {
        let request = request(socket).await;
        assert_eq!(request["method"], method);
        respond(socket, &request, json!({})).await;
    }
}

#[tokio::test]
async fn accessibility_snapshot_creates_host_owned_handles_and_redacts_editable_values() {
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            initialize(&mut socket).await;
            let tree = request(&mut socket).await;
            assert_eq!(tree["method"], "Accessibility.getFullAXTree");
            respond(
                &mut socket,
                &tree,
                json!({
                    "nodes": [
                        {
                            "ignored": false,
                            "role": { "value": "textbox" },
                            "name": { "value": "Password" },
                            "value": { "value": "super-secret" },
                            "backendDOMNodeId": 7
                        },
                        {
                            "ignored": false,
                            "role": { "value": "button" },
                            "name": { "value": "Submit" },
                            "backendDOMNodeId": 8
                        },
                        {
                            "ignored": false,
                            "role": { "value": "StaticText" },
                            "name": { "value": "Hello" }
                        }
                    ]
                }),
            )
            .await;
            let metadata = request(&mut socket).await;
            assert_eq!(metadata["method"], "Runtime.evaluate");
            respond(
                &mut socket,
                &metadata,
                json!({ "result": { "value": { "url": "https://example.test", "title": "Example" } } }),
            )
            .await;
        })
    })
    .await;
    let client = CdpClient::connect(&url).await.expect("connect client");
    let mut handles = BrowserHandles::default();

    let BrowserToolOutput::Text(output) = snapshot(&client, &mut handles).await.expect("snapshot")
    else {
        panic!("expected text snapshot");
    };
    let output: Value = serde_json::from_str(&output).expect("snapshot JSON");

    assert_eq!(output["nodes"][0]["value"], "<redacted>");
    assert_eq!(output["nodes"][1]["name"], "Submit");
    assert_eq!(output["text"], "Hello");
    let node_id = output["nodes"][1]["nodeId"]
        .as_str()
        .expect("button node id");
    assert_eq!(handles.resolve(node_id).expect("resolve button"), 8);
    server.await.expect("server task");
}

#[tokio::test]
async fn native_click_uses_the_accessibility_handle_box_center() {
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            initialize(&mut socket).await;
            let scroll = request(&mut socket).await;
            assert_eq!(scroll["method"], "DOM.scrollIntoViewIfNeeded");
            respond(&mut socket, &scroll, json!({})).await;
            let box_model = request(&mut socket).await;
            assert_eq!(box_model["method"], "DOM.getBoxModel");
            respond(
                &mut socket,
                &box_model,
                json!({ "model": { "content": [10, 20, 30, 20, 30, 40, 10, 40] } }),
            )
            .await;
            for expected_type in ["mouseMoved", "mousePressed", "mouseReleased"] {
                let input = request(&mut socket).await;
                assert_eq!(input["method"], "Input.dispatchMouseEvent");
                assert_eq!(input["params"]["type"], expected_type);
                assert_eq!(input["params"]["x"], 20.0);
                assert_eq!(input["params"]["y"], 30.0);
                respond(&mut socket, &input, json!({})).await;
            }
        })
    })
    .await;
    let client = CdpClient::connect(&url).await.expect("connect client");
    let mut handles = BrowserHandles::default();
    let node_id = handles.insert(/*backend_node_id*/ 9);

    let output = click(&client, &handles, &node_id)
        .await
        .expect("native click");

    assert_eq!(
        output,
        BrowserToolOutput::Text(format!("clicked {node_id}"))
    );
    server.await.expect("server task");
}

#[tokio::test]
async fn keyboard_input_uses_key_down_when_chromium_needs_text() {
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            initialize(&mut socket).await;
            for (expected_key, expected_text) in [("Enter", "\r"), ("?", "?")] {
                let key_down = request(&mut socket).await;
                assert_eq!(key_down["method"], "Input.dispatchKeyEvent");
                assert_eq!(key_down["params"]["type"], "keyDown");
                assert_eq!(key_down["params"]["key"], expected_key);
                assert_eq!(key_down["params"]["text"], expected_text);
                respond(&mut socket, &key_down, json!({})).await;

                let key_up = request(&mut socket).await;
                assert_eq!(key_up["method"], "Input.dispatchKeyEvent");
                assert_eq!(key_up["params"]["type"], "keyUp");
                assert_eq!(key_up["params"]["key"], expected_key);
                respond(&mut socket, &key_up, json!({})).await;
            }
        })
    })
    .await;
    let client = CdpClient::connect(&url).await.expect("connect client");

    press(&client, "Enter").await.expect("press Enter");
    dispatch_human_key(
        &client,
        &BrowserKeyInput {
            key: "?".to_string(),
            code: "Slash".to_string(),
            text: Some("?".to_string()),
            modifiers: BrowserInputModifiers {
                shift: true,
                ..Default::default()
            },
        },
    )
    .await
    .expect("dispatch human key");

    server.await.expect("server task");
}

#[tokio::test]
async fn navigation_waits_for_the_cdp_lifecycle_event() {
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            initialize(&mut socket).await;
            let navigation = request(&mut socket).await;
            assert_eq!(navigation["method"], "Page.navigate");
            respond(
                &mut socket,
                &navigation,
                json!({ "frameId": "frame", "loaderId": "loader" }),
            )
            .await;
            socket
                .send(Message::Text(
                    json!({
                        "method": "Page.lifecycleEvent",
                        "params": {
                            "frameId": "frame",
                            "loaderId": "loader",
                            "name": "DOMContentLoaded",
                            "timestamp": 1,
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send lifecycle event");
        })
    })
    .await;
    let client = CdpClient::connect(&url).await.expect("connect client");

    navigate(&client, "https://example.test")
        .await
        .expect("navigation");

    server.await.expect("server task");
}

#[tokio::test]
async fn navigation_timeout_bounds_the_cdp_command_and_wait() {
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let (url, server) = test_server(|mut socket| {
        tokio::spawn(async move {
            initialize(&mut socket).await;
            let navigation = request(&mut socket).await;
            assert_eq!(navigation["method"], "Page.navigate");
            release_rx.await.expect("release server");
        })
    })
    .await;
    let client = CdpClient::connect(&url).await.expect("connect client");
    let started = std::time::Instant::now();

    let error = navigate_request(
        &client,
        &NavigateRequest {
            action: NavigationAction::Goto,
            url: Some("https://example.test".to_string()),
            wait_until: LoadState::Load,
            timeout_ms: Some(/*value*/ 10),
        },
    )
    .await
    .expect_err("navigation must time out");

    assert!(error.to_string().contains("navigation_timeout"));
    assert!(started.elapsed() < std::time::Duration::from_millis(/*millis*/ 500));
    release_tx.send(()).expect("release server");
    server.await.expect("server task");
}
