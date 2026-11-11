//! Test which WebSocket closes switch the session to HTTP.

use anyhow::Result;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Rejection {
    Oversized,
    AdvertisedLimit,
    OversizedWarmup,
    AfterModelMetadata,
    AfterRateLimits,
    AfterResponseCreated,
    GoingAway,
}

#[test_case::test_case(Rejection::AdvertisedLimit, 0, 1; "advertised_limit_skips_websocket_upload")]
#[test_case::test_case(Rejection::Oversized, 1, 1; "size_rejection_before_any_text")]
#[test_case::test_case(Rejection::OversizedWarmup, 0, 1; "size_rejection_during_prewarm")]
#[test_case::test_case(Rejection::AfterModelMetadata, 2, 0; "size_close_after_model_metadata")]
#[test_case::test_case(Rejection::AfterRateLimits, 2, 0; "size_close_after_rate_limits")]
#[test_case::test_case(Rejection::AfterResponseCreated, 2, 0; "size_close_after_response_created")]
#[test_case::test_case(Rejection::GoingAway, 2, 0; "ordinary_close")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retry_transport(
    rejection: Rejection,
    expected_ws_generations: usize,
    expected_posts: usize,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let http = responses::start_mock_server().await;
    responses::mount_sse_once(
        &http,
        sse(vec![ev_response_created("http"), ev_completed("http")]),
    )
    .await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}/v1", listener.local_addr()?);
    let http_address = *http.address();
    let generations = Arc::new(AtomicUsize::new(0));
    let captured = Arc::clone(&generations);
    let mut server = JoinSet::new();
    server.spawn(async move {
        let mut connections = JoinSet::new();
        while let Ok((stream, _)) = listener.accept().await {
            connections.spawn(serve_connection(
                stream,
                http_address,
                Arc::clone(&captured),
                rejection,
            ));
        }
    });

    let mut builder = test_codex().with_config(move |config| {
        config.model_provider.base_url = Some(base_url);
        config.model_provider.supports_websockets = true;
        config.model_provider.stream_max_retries = Some(1);
        config.model_provider.request_max_retries = Some(0);
    });
    let test = builder.build_with_auto_env(&http).await?;
    test.submit_turn("first").await?;
    assert_eq!(generations.load(Ordering::Relaxed), expected_ws_generations);
    assert_eq!(
        responses::received_responses_requests(&http).await.len(),
        expected_posts
    );
    Ok(())
}

async fn serve_connection(
    mut stream: TcpStream,
    http_address: SocketAddr,
    generations: Arc<AtomicUsize>,
    rejection: Rejection,
) -> Result<()> {
    let mut method = [0];
    stream.peek(&mut method).await?;
    if method == *b"P" {
        let mut upstream = TcpStream::connect(http_address).await?;
        tokio::io::copy_bidirectional(&mut stream, &mut upstream).await?;
        return Ok(());
    }
    let mut socket =
        tokio_tungstenite::accept_hdr_async(stream, |_: &Request, mut res: Response| {
            if rejection == Rejection::AdvertisedLimit {
                res.headers_mut().insert(
                    "x-codex-websocket-max-message-bytes",
                    http::HeaderValue::from_static("1"),
                );
            }
            Ok(res)
        })
        .await?;
    while let Some(message) = socket.next().await.transpose()? {
        let Message::Text(text) = message else {
            continue;
        };
        let request: Value = serde_json::from_str(&text)?;
        let warmup = request["generate"] == false;
        let reject = if warmup {
            rejection == Rejection::OversizedWarmup
        } else {
            generations.fetch_add(1, Ordering::Relaxed) == 0
        };
        if reject {
            let event = match rejection {
                Rejection::AfterModelMetadata => Some(serde_json::json!({
                    "type": "codex.response.metadata",
                    "headers": {"openai-model": "test-model"}
                })),
                Rejection::AfterRateLimits => Some(serde_json::json!({
                    "type": "codex.rate_limits",
                    "rate_limits": {"primary": {"used_percent": 1.0}}
                })),
                Rejection::AfterResponseCreated => Some(ev_response_created("started")),
                _ => None,
            };
            if let Some(event) = event {
                socket.send(Message::text(event.to_string())).await?;
            }
            let code = if rejection == Rejection::GoingAway {
                CloseCode::Away
            } else {
                CloseCode::Size
            };
            socket
                .close(Some(CloseFrame {
                    code,
                    reason: "".into(),
                }))
                .await?;
            break;
        }
        let id = if warmup { "warmup" } else { "generation" };
        for event in [ev_response_created(id), ev_completed(id)] {
            socket.send(Message::text(event.to_string())).await?;
        }
    }
    Ok(())
}
