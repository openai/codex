mod streamable_http_test_support;

use std::convert::Infallible;
use std::time::Duration;

use anyhow::Context as _;
use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::Event;
use axum::response::sse::Sse;
use axum::routing::post;
use futures::StreamExt as _;
use futures::stream;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::time::timeout;

use streamable_http_test_support::create_client;

struct StreamClosed {
    event_name: String,
    closed: mpsc::UnboundedSender<String>,
}

impl Drop for StreamClosed {
    fn drop(&mut self) {
        let _ = self.closed.send(self.event_name.clone());
    }
}

fn initialize_response(message: &Value) -> axum::response::Response {
    Json(json!({
        "jsonrpc": "2.0",
        "id": message["id"],
        "result": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "serverInfo": {"name": "plugin-runtime", "version": "1.0.0"},
        },
    }))
    .into_response()
}

fn plugin_runtime_notifications(event_name: &str, request_id: Value) -> [Value; 2] {
    let metadata = json!({"io.modelcontextprotocol/subscriptionId": request_id});
    [
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/events/active",
            "params": {"cursor": null, "truncated": false, "_meta": metadata},
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/events/event",
            "params": {
                "eventId": format!("event-{event_name}"),
                "name": event_name,
                "timestamp": "2026-08-07T12:00:00Z",
                "data": {"issue": 42},
                "cursor": null,
                "_meta": metadata,
            },
        }),
    ]
}

#[tokio::test]
async fn plugin_runtime_event_streams_are_isolated_and_cancel_locally() -> anyhow::Result<()> {
    let (stream_closed_tx, mut stream_closed_rx) = mpsc::unbounded_channel::<String>();
    let router = Router::new().route(
        "/mcp",
        post(move |Json(message): Json<Value>| {
            let stream_closed_tx = stream_closed_tx.clone();
            async move {
                match message["method"].as_str() {
                    Some("initialize") => initialize_response(&message),
                    Some("notifications/initialized") => StatusCode::ACCEPTED.into_response(),
                    Some("events/stream") => {
                        let event_name = message["params"]["name"]
                            .as_str()
                            .expect("event stream request must contain a name")
                            .to_string();
                        assert_eq!(
                            message.pointer("/params/arguments"),
                            Some(&json!({"project": "codex"}))
                        );
                        assert!(message["params"].get("cursor").is_none());

                        let closed = StreamClosed {
                            event_name: event_name.clone(),
                            closed: stream_closed_tx,
                        };
                        let events = stream::iter(
                            plugin_runtime_notifications(&event_name, message["id"].clone())
                                .into_iter()
                                .map(|notification| {
                                    Ok::<_, Infallible>(
                                        Event::default()
                                            .event("message")
                                            .data(notification.to_string()),
                                    )
                                }),
                        )
                        .chain(stream::pending())
                        .map(move |event| {
                            let _ = &closed;
                            event
                        });

                        Sse::new(events).into_response()
                    }
                    Some("notifications/cancelled") => {
                        panic!("event cancellation should close the local stream without a POST")
                    }
                    method => panic!("unexpected Plugin Runtime request: {method:?}"),
                }
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}", listener.local_addr()?);
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let client = create_client(&base_url).await?;

    let mut first = client
        .send_event_stream_request(Some(json!({
            "name": "github.pull_request.opened",
            "arguments": {"project": "codex"},
        })))
        .await?;
    let mut second = client
        .send_event_stream_request(Some(json!({
            "name": "gmail.message.received",
            "arguments": {"project": "codex"},
        })))
        .await?;

    for (event_name, request) in [
        ("github.pull_request.opened", &mut first),
        ("gmail.message.received", &mut second),
    ] {
        let active = timeout(Duration::from_secs(5), request.notifications.recv())
            .await?
            .context("event stream closed before activation")?;
        assert_eq!(active.method, "notifications/events/active");
        assert_eq!(
            active.params,
            Some(json!({"cursor": null, "truncated": false}))
        );

        let event = timeout(Duration::from_secs(5), request.notifications.recv())
            .await?
            .context("event stream closed before delivery")?;
        assert_eq!(event.method, "notifications/events/event");
        assert_eq!(
            event.params.as_ref().and_then(|params| params.get("name")),
            Some(&json!(event_name))
        );
    }

    first
        .handle
        .cancel(Some("event subscription closed".to_string()))
        .await?;
    let closed = timeout(Duration::from_secs(5), stream_closed_rx.recv())
        .await?
        .context("cancelled stream did not close")?;
    assert_eq!(closed, "github.pull_request.opened");

    second
        .handle
        .cancel(Some("event subscription closed".to_string()))
        .await?;
    let closed = timeout(Duration::from_secs(5), stream_closed_rx.recv())
        .await?
        .context("cancelled stream did not close")?;
    assert_eq!(closed, "gmail.message.received");

    client.shutdown().await;
    server.abort();
    let _ = server.await;
    Ok(())
}

#[tokio::test]
async fn plugin_runtime_event_stream_times_out_before_stalled_headers() -> anyhow::Result<()> {
    let request_started = std::sync::Arc::new(Notify::new());
    let server_request_started = std::sync::Arc::clone(&request_started);
    let router = Router::new().route(
        "/mcp",
        post(move |Json(message): Json<Value>| {
            let request_started = std::sync::Arc::clone(&server_request_started);
            async move {
                match message["method"].as_str() {
                    Some("initialize") => initialize_response(&message),
                    Some("notifications/initialized") => StatusCode::ACCEPTED.into_response(),
                    Some("events/stream") => {
                        request_started.notify_one();
                        std::future::pending().await
                    }
                    method => panic!("unexpected Plugin Runtime request: {method:?}"),
                }
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}", listener.local_addr()?);
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let client = create_client(&base_url).await?;
    let request = client
        .send_event_stream_request(Some(json!({
            "name": "github.pull_request.opened",
            "arguments": {},
        })))
        .await?;

    timeout(Duration::from_secs(5), request_started.notified())
        .await
        .context("event stream request did not reach the server")?;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(31)).await;
    let error = request
        .handle
        .rx
        .await?
        .expect_err("stalled response headers must time out");
    tokio::time::resume();
    assert!(error.to_string().contains("timed out"));

    client.shutdown().await;
    server.abort();
    let _ = server.await;
    Ok(())
}
