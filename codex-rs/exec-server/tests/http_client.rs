use std::future::Future;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_exec_server::ExecServerClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRequestBodyDeltaNotification;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpRequestTimeout;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server::RemoteExecServerConnectArgs;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::from_slice;
use serde_json::from_str;
use serde_json::from_value;
use serde_json::to_string;
use serde_json::to_value;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

const CLIENT_NAME: &str = "test-exec-server-client";
const HTTP_REQUEST_METHOD: &str = "http/request";
const HTTP_REQUEST_BODY_DELTA_METHOD: &str = "http/request/bodyDelta";
const INITIALIZE_METHOD: &str = "initialize";
const INITIALIZED_METHOD: &str = "initialized";
const TEST_TIMEOUT: Duration = Duration::from_secs(5);
const HTTP_BODY_DELTA_CHANNEL_CAPACITY: u64 = 256;
const OVERFLOWING_BODY_DELTA_FRAMES: u64 = 1_024;

/// What this tests: the buffered HTTP helper always sends a buffered
/// `http/request`, even when a caller accidentally provides streaming flags.
#[tokio::test]
async fn http_request_forces_buffered_request_params() -> Result<()> {
    // Phase 1: start a fake WebSocket exec-server so the test covers the
    // public client connection path without depending on the HTTP runner.
    let server = spawn_scripted_exec_server(|mut peer| async move {
        // Phase 2: verify the buffered helper strips streaming-only fields
        // before it sends the JSON-RPC call.
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/buffered".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: None,
                stream_response: false,
            }
        );

        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 200,
                headers: Vec::new(),
                body: b"buffered".to_vec().into(),
            },
        )
        .await
    })
    .await?;
    let client = server.connect_client().await?;

    // Phase 3: call the buffered helper with streaming-only fields populated
    // and assert callers still receive the buffered response body.
    let response = timeout(
        TEST_TIMEOUT,
        client.http_request(HttpRequestParams {
            method: "GET".to_string(),
            url: "https://example.test/buffered".to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: HttpRequestTimeout::Default,
            request_id: Some("ignored-stream-id".to_string()),
            stream_response: true,
        }),
    )
    .await
    .context("buffered http/request should complete")??;
    assert_eq!(
        response,
        HttpRequestResponse {
            status: 200,
            headers: Vec::new(),
            body: b"buffered".to_vec().into(),
        }
    );

    drop(client);
    server.finish().await?;
    Ok(())
}

/// What this tests: streamed executor HTTP response frames are routed by the
/// client's generated request id, delivered in sequence, and concatenated by
/// the caller.
#[tokio::test]
async fn http_response_body_stream_uses_generated_ids_and_receives_ordered_deltas() -> Result<()> {
    // Phase 1: script two requests. The caller supplies reusable ids, but the
    // client replaces them with connection-local ids on the wire.
    let server = spawn_scripted_exec_server(|mut peer| async move {
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp".to_string(),
                headers: vec![HttpHeader {
                    name: "accept".to_string(),
                    value: "text/event-stream".to_string(),
                }],
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("http-1".to_string()),
                stream_response: true,
            }
        );

        // Phase 2: return headers first, then body notifications in the order
        // the public body stream should expose them.
        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 200,
                headers: vec![HttpHeader {
                    name: "content-type".to_string(),
                    value: "text/event-stream".to_string(),
                }],
                body: Vec::new().into(),
            },
        )
        .await?;
        for delta in [
            HttpRequestBodyDeltaNotification {
                request_id: "http-1".to_string(),
                seq: 1,
                delta: b"hello ".to_vec().into(),
                done: false,
                error: None,
            },
            HttpRequestBodyDeltaNotification {
                request_id: "http-1".to_string(),
                seq: 2,
                delta: b"world".to_vec().into(),
                done: false,
                error: None,
            },
            HttpRequestBodyDeltaNotification {
                request_id: "http-1".to_string(),
                seq: 3,
                delta: b"!".to_vec().into(),
                done: true,
                error: None,
            },
        ] {
            peer.write_body_delta(delta).await?;
        }

        // Phase 3: accept the next generated request id after EOF.
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/reuse".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("http-2".to_string()),
                stream_response: true,
            }
        );
        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 204,
                headers: Vec::new(),
                body: Vec::new().into(),
            },
        )
        .await
    })
    .await?;
    let client = server.connect_client().await?;

    // Phase 4: start a streaming HTTP request through the public client API.
    let (response, mut body_stream) = timeout(
        TEST_TIMEOUT,
        client.http_request_stream(HttpRequestParams {
            method: "GET".to_string(),
            url: "https://example.test/mcp".to_string(),
            headers: vec![HttpHeader {
                name: "accept".to_string(),
                value: "text/event-stream".to_string(),
            }],
            body: None,
            timeout_ms: HttpRequestTimeout::Default,
            request_id: Some("caller-stream-id".to_string()),
            stream_response: false,
        }),
    )
    .await
    .context("streamed http/request should return headers")??;
    assert_eq!(
        response,
        HttpRequestResponse {
            status: 200,
            headers: vec![HttpHeader {
                name: "content-type".to_string(),
                value: "text/event-stream".to_string(),
            }],
            body: Vec::new().into(),
        }
    );

    // Phase 5: drain the body stream and verify the caller-visible byte order.
    let mut body = Vec::new();
    while let Some(chunk) = timeout(TEST_TIMEOUT, body_stream.recv())
        .await
        .context("http response body delta should arrive")??
    {
        body.extend_from_slice(&chunk);
    }
    assert_eq!(body, b"hello world!".to_vec());

    // Phase 6: start another stream through the public API to validate cleanup
    // after EOF without reaching into the client routing table.
    let (reuse_response, _reuse_body_stream) = timeout(
        TEST_TIMEOUT,
        client.http_request_stream(HttpRequestParams {
            method: "GET".to_string(),
            url: "https://example.test/mcp/reuse".to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: HttpRequestTimeout::Default,
            request_id: Some("caller-stream-id".to_string()),
            stream_response: false,
        }),
    )
    .await
    .context("second streamed http/request should return headers")??;
    assert_eq!(
        reuse_response,
        HttpRequestResponse {
            status: 204,
            headers: Vec::new(),
            body: Vec::new().into(),
        }
    );

    drop(client);
    server.finish().await?;
    Ok(())
}

/// What this tests: dropping a body stream with a queued terminal frame removes
/// the old route while the next stream gets a fresh generated id.
#[tokio::test]
async fn http_response_body_stream_drops_queued_terminal_before_next_generated_id() -> Result<()> {
    // Phase 1: send terminal EOF before the header response so the public body
    // stream starts with EOF already queued but unread.
    let server = spawn_scripted_exec_server(|mut peer| async move {
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/queued-terminal".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("http-1".to_string()),
                stream_response: true,
            }
        );
        peer.write_body_delta(HttpRequestBodyDeltaNotification {
            request_id: "http-1".to_string(),
            seq: 1,
            delta: Vec::new().into(),
            done: true,
            error: None,
        })
        .await?;
        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 200,
                headers: Vec::new(),
                body: Vec::new().into(),
            },
        )
        .await?;

        // Phase 2: accept another stream after the client drops the unread
        // body. The second request receives a distinct generated id.
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/retry-queued-terminal".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("http-2".to_string()),
                stream_response: true,
            }
        );
        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 204,
                headers: Vec::new(),
                body: Vec::new().into(),
            },
        )
        .await
    })
    .await?;
    let client = server.connect_client().await?;

    // Phase 3: drop the body stream without reading the queued EOF frame.
    let (response, body_stream) = timeout(
        TEST_TIMEOUT,
        client.http_request_stream(HttpRequestParams {
            method: "GET".to_string(),
            url: "https://example.test/mcp/queued-terminal".to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: HttpRequestTimeout::Default,
            request_id: Some("caller-stream-id".to_string()),
            stream_response: false,
        }),
    )
    .await
    .context("streamed http/request should return headers")??;
    assert_eq!(
        response,
        HttpRequestResponse {
            status: 200,
            headers: Vec::new(),
            body: Vec::new().into(),
        }
    );
    drop(body_stream);

    // Phase 4: start another stream through the public API. The caller-provided
    // id is ignored, so the request uses the next generated route id.
    let params = HttpRequestParams {
        method: "GET".to_string(),
        url: "https://example.test/mcp/retry-queued-terminal".to_string(),
        headers: Vec::new(),
        body: None,
        timeout_ms: HttpRequestTimeout::Default,
        request_id: Some("caller-stream-id".to_string()),
        stream_response: false,
    };
    let (reuse_response, _reuse_body_stream) =
        timeout(TEST_TIMEOUT, client.http_request_stream(params))
            .await
            .context("second streamed http/request should return headers")??;
    assert_eq!(
        reuse_response,
        HttpRequestResponse {
            status: 204,
            headers: Vec::new(),
            body: Vec::new().into(),
        }
    );

    drop(client);
    server.finish().await?;
    Ok(())
}

/// What this tests: cancelling a streaming HTTP request while it is waiting for
/// headers drops its route, and a later stream gets a fresh generated id.
#[tokio::test]
async fn http_response_body_stream_ignores_late_deltas_after_cancelled_request() -> Result<()> {
    // Phase 1: coordinate cancellation after the fake server observes the
    // first request but before it returns headers. The server later sends a
    // stale delta for the cancelled id before serving the fresh stream.
    let (request_seen_tx, request_seen_rx) = oneshot::channel();
    let server = spawn_scripted_exec_server(|mut peer| async move {
        let (_request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/cancel".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("http-1".to_string()),
                stream_response: true,
            }
        );
        request_seen_tx
            .send(())
            .expect("test should wait for the first request");

        // Phase 2: the next stream uses a new generated id. A late body delta
        // for the cancelled id is ignored by the client-side router.
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/retry-cancelled".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("http-2".to_string()),
                stream_response: true,
            }
        );
        peer.write_body_delta(HttpRequestBodyDeltaNotification {
            request_id: "http-1".to_string(),
            seq: 1,
            delta: b"stale".to_vec().into(),
            done: false,
            error: None,
        })
        .await?;
        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 200,
                headers: Vec::new(),
                body: Vec::new().into(),
            },
        )
        .await?;
        peer.write_body_delta(HttpRequestBodyDeltaNotification {
            request_id: "http-2".to_string(),
            seq: 1,
            delta: b"fresh".to_vec().into(),
            done: true,
            error: None,
        })
        .await
    })
    .await?;
    let client = server.connect_client().await?;

    // Phase 3: start a streaming request and abort the caller future while it
    // is blocked waiting for response headers.
    let client_for_request = client.clone();
    let stream_task = tokio::spawn(async move {
        let _ = client_for_request
            .http_request_stream(HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/cancel".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("caller-stream-id".to_string()),
                stream_response: false,
            })
            .await;
    });
    request_seen_rx
        .await
        .expect("server should observe the first http/request");
    stream_task.abort();
    let _ = stream_task.await;

    // Phase 4: start a new stream immediately. It receives only the fresh body
    // bytes for its generated id.
    let (response, mut body_stream) = timeout(
        TEST_TIMEOUT,
        client.http_request_stream(HttpRequestParams {
            method: "GET".to_string(),
            url: "https://example.test/mcp/retry-cancelled".to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: HttpRequestTimeout::Default,
            request_id: Some("caller-stream-id".to_string()),
            stream_response: false,
        }),
    )
    .await
    .context("second streamed http/request should return headers")??;
    assert_eq!(
        response,
        HttpRequestResponse {
            status: 200,
            headers: Vec::new(),
            body: Vec::new().into(),
        }
    );
    let mut body = Vec::new();
    while let Some(chunk) = timeout(TEST_TIMEOUT, body_stream.recv())
        .await
        .context("fresh http response body delta should arrive")??
    {
        body.extend_from_slice(&chunk);
    }
    assert_eq!(body, b"fresh".to_vec());

    drop(client);
    server.finish().await?;
    Ok(())
}

/// What this tests: dropping a returned body stream before EOF removes its
/// route and prevents stale body deltas from reaching the next stream.
#[tokio::test]
async fn http_response_body_stream_ignores_late_deltas_after_drop() -> Result<()> {
    // Phase 1: script two requests. The first returns only headers; after the
    // client drops its body receiver, the server sends a stale body delta.
    let (body_dropped_tx, body_dropped_rx) = oneshot::channel();
    let (stale_delta_sent_tx, stale_delta_sent_rx) = oneshot::channel();
    let server = spawn_scripted_exec_server(|mut peer| async move {
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/drop".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("http-1".to_string()),
                stream_response: true,
            }
        );
        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 200,
                headers: Vec::new(),
                body: Vec::new().into(),
            },
        )
        .await?;
        body_dropped_rx
            .await
            .expect("test should drop the first body stream");
        peer.write_body_delta(HttpRequestBodyDeltaNotification {
            request_id: "http-1".to_string(),
            seq: 1,
            delta: b"stale".to_vec().into(),
            done: false,
            error: None,
        })
        .await?;
        stale_delta_sent_tx
            .send(())
            .expect("test should wait for the stale delta");

        // Phase 2: accept the next request with a new generated id. The new
        // stream must receive only fresh body bytes.
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/retry-dropped".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("http-2".to_string()),
                stream_response: true,
            }
        );
        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 200,
                headers: Vec::new(),
                body: Vec::new().into(),
            },
        )
        .await?;
        peer.write_body_delta(HttpRequestBodyDeltaNotification {
            request_id: "http-2".to_string(),
            seq: 1,
            delta: b"fresh".to_vec().into(),
            done: true,
            error: None,
        })
        .await
    })
    .await?;
    let client = server.connect_client().await?;

    // Phase 3: receive headers for the first stream, then drop the body stream
    // without reading any body frames.
    let (response, body_stream) = timeout(
        TEST_TIMEOUT,
        client.http_request_stream(HttpRequestParams {
            method: "GET".to_string(),
            url: "https://example.test/mcp/drop".to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: HttpRequestTimeout::Default,
            request_id: Some("caller-stream-id".to_string()),
            stream_response: false,
        }),
    )
    .await
    .context("streamed http/request should return headers")??;
    assert_eq!(
        response,
        HttpRequestResponse {
            status: 200,
            headers: Vec::new(),
            body: Vec::new().into(),
        }
    );
    drop(body_stream);
    body_dropped_tx
        .send(())
        .expect("server should wait for the body stream drop");
    stale_delta_sent_rx
        .await
        .expect("server should send one stale nonterminal delta");

    // Phase 4: start the next stream immediately. The caller-provided id is
    // ignored, and the fresh generated id isolates it from stale bytes.
    let (reuse_response, mut reuse_body_stream) = timeout(
        TEST_TIMEOUT,
        client.http_request_stream(HttpRequestParams {
            method: "GET".to_string(),
            url: "https://example.test/mcp/retry-dropped".to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: HttpRequestTimeout::Default,
            request_id: Some("caller-stream-id".to_string()),
            stream_response: false,
        }),
    )
    .await
    .context("second streamed http/request should return headers")??;
    assert_eq!(
        reuse_response,
        HttpRequestResponse {
            status: 200,
            headers: Vec::new(),
            body: Vec::new().into(),
        }
    );
    let mut body = Vec::new();
    while let Some(chunk) = timeout(TEST_TIMEOUT, reuse_body_stream.recv())
        .await
        .context("fresh http response body delta should arrive")??
    {
        body.extend_from_slice(&chunk);
    }
    assert_eq!(body, b"fresh".to_vec());

    drop(client);
    server.finish().await?;
    Ok(())
}

/// What this tests: an in-flight streamed HTTP body is failed when the shared
/// JSON-RPC transport disconnects before a terminal body frame.
#[tokio::test]
async fn http_response_body_stream_fails_when_transport_disconnects() -> Result<()> {
    // Phase 1: return response headers for a streaming request, then drop the
    // fake server transport without sending EOF.
    let server = spawn_scripted_exec_server(|mut peer| async move {
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/disconnect".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("http-1".to_string()),
                stream_response: true,
            }
        );
        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 200,
                headers: Vec::new(),
                body: Vec::new().into(),
            },
        )
        .await
    })
    .await?;
    let client = server.connect_client().await?;

    // Phase 2: start a streaming HTTP request and receive headers.
    let (_response, mut body_stream) = timeout(
        TEST_TIMEOUT,
        client.http_request_stream(HttpRequestParams {
            method: "GET".to_string(),
            url: "https://example.test/mcp/disconnect".to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: HttpRequestTimeout::Default,
            request_id: Some("caller-stream-id".to_string()),
            stream_response: false,
        }),
    )
    .await
    .context("streamed http/request should return headers")??;

    // Phase 3: assert transport disconnect wakes the body stream with a
    // terminal error instead of hanging.
    let error = timeout(TEST_TIMEOUT, body_stream.recv())
        .await
        .context("disconnect should wake http body stream")?
        .expect_err("disconnect should fail the http body stream");
    let error_message = error.to_string();
    assert_eq!(
        error_message.starts_with(
            "exec-server protocol error: http response stream `http-1` failed: exec-server transport disconnected"
        ),
        true
    );

    drop(client);
    server.finish().await?;
    Ok(())
}

/// What this tests: transport disconnect still records a terminal stream
/// failure even when the client-side body-delta queue is already full.
#[tokio::test]
async fn http_response_body_stream_reports_disconnect_when_queue_is_full() -> Result<()> {
    // Phase 1: fill the queued body-delta route exactly to capacity before the
    // response headers arrive, then drop the transport without sending EOF.
    let server = spawn_scripted_exec_server(|mut peer| async move {
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/disconnect-full-queue".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: None,
                request_id: Some("http-1".to_string()),
                stream_response: true,
            }
        );
        for seq in 1..=HTTP_BODY_DELTA_CHANNEL_CAPACITY {
            peer.write_body_delta(HttpRequestBodyDeltaNotification {
                request_id: "http-1".to_string(),
                seq,
                delta: b"x".to_vec().into(),
                done: false,
                error: None,
            })
            .await?;
        }
        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 200,
                headers: Vec::new(),
                body: Vec::new().into(),
            },
        )
        .await
    })
    .await?;
    let client = server.connect_client().await?;

    // Phase 2: start the streaming request and receive headers while the
    // queue is already full.
    let (_response, mut body_stream) = timeout(
        TEST_TIMEOUT,
        client.http_request_stream(HttpRequestParams {
            method: "GET".to_string(),
            url: "https://example.test/mcp/disconnect-full-queue".to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: None,
            request_id: Some("caller-stream-id".to_string()),
            stream_response: false,
        }),
    )
    .await
    .context("streamed http/request should return headers")??;

    // Phase 3: drain the queued chunks and assert the transport disconnect is
    // still reported as an error rather than a clean EOF.
    let mut chunks = 0;
    let error = loop {
        match timeout(TEST_TIMEOUT, body_stream.recv())
            .await
            .context("disconnect should wake the full queued body stream")?
        {
            Ok(Some(_chunk)) => {
                chunks += 1;
            }
            Ok(None) => bail!("disconnect with a full queue should not look like clean EOF"),
            Err(error) => break error,
        }
    };
    assert_eq!(
        (
            chunks,
            error
                .to_string()
                .starts_with(
                    "exec-server protocol error: http response stream `http-1` failed: exec-server transport disconnected",
                ),
        ),
        (HTTP_BODY_DELTA_CHANNEL_CAPACITY as usize, true)
    );

    drop(client);
    server.finish().await?;
    Ok(())
}

/// What this tests: body-delta backpressure closes the public body stream as
/// an error rather than letting callers accept a truncated body as clean EOF.
#[tokio::test]
async fn http_response_body_stream_reports_backpressure_truncation() -> Result<()> {
    // Phase 1: send enough body frames before headers to overflow the bounded
    // client-side route while the public request future is still pending.
    let (finish_tx, finish_rx) = oneshot::channel();
    let server = spawn_scripted_exec_server(|mut peer| async move {
        let (request_id, params) = peer.read_http_request().await?;
        assert_eq!(
            params,
            HttpRequestParams {
                method: "GET".to_string(),
                url: "https://example.test/mcp/backpressure".to_string(),
                headers: Vec::new(),
                body: None,
                timeout_ms: HttpRequestTimeout::Default,
                request_id: Some("http-1".to_string()),
                stream_response: true,
            }
        );
        for seq in 1..=OVERFLOWING_BODY_DELTA_FRAMES {
            peer.write_body_delta(HttpRequestBodyDeltaNotification {
                request_id: "http-1".to_string(),
                seq,
                delta: b"x".to_vec().into(),
                done: false,
                error: None,
            })
            .await?;
        }
        peer.write_response(
            request_id,
            HttpRequestResponse {
                status: 200,
                headers: Vec::new(),
                body: Vec::new().into(),
            },
        )
        .await?;

        // Phase 2: keep the transport connected so the body stream reports the
        // backpressure failure rather than a disconnect.
        finish_rx.await.expect("test should finish server task");
        Ok(())
    })
    .await?;
    let client = server.connect_client().await?;

    // Phase 3: start the streaming request; the server overfills the route
    // before returning the body stream to this consumer.
    let (_response, mut body_stream) = timeout(
        TEST_TIMEOUT,
        client.http_request_stream(HttpRequestParams {
            method: "GET".to_string(),
            url: "https://example.test/mcp/backpressure".to_string(),
            headers: Vec::new(),
            body: None,
            timeout_ms: HttpRequestTimeout::Default,
            request_id: Some("caller-stream-id".to_string()),
            stream_response: false,
        }),
    )
    .await
    .context("streamed http/request should return headers")??;

    // Phase 4: drain queued chunks and assert the truncated stream ends in an
    // explicit error, not a clean EOF.
    let mut chunks = 0;
    let error = loop {
        match timeout(TEST_TIMEOUT, body_stream.recv())
            .await
            .context("backpressure should close http body stream")?
        {
            Ok(Some(_chunk)) => {
                chunks += 1;
            }
            Ok(None) => bail!("backpressure truncation should not look like clean EOF"),
            Err(error) => break error,
        }
    };
    assert_eq!(
        (
            chunks < OVERFLOWING_BODY_DELTA_FRAMES as usize,
            error.to_string(),
        ),
        (
            true,
            "exec-server protocol error: http response stream `http-1` failed: body delta channel filled before delivery".to_string(),
        )
    );

    finish_tx
        .send(())
        .expect("server task should wait for test completion");
    drop(client);
    server.finish().await?;
    Ok(())
}

/// Fake WebSocket exec-server used by the integration tests.
///
/// The helper exercises `ExecServerClient::connect_websocket`, including the
/// initialize handshake, while each test controls the exact JSON-RPC traffic
/// that follows.
struct ScriptedExecServer {
    websocket_url: String,
    task: JoinHandle<Result<()>>,
}

impl ScriptedExecServer {
    /// Connects the public exec-server client to this fake WebSocket endpoint.
    async fn connect_client(&self) -> Result<ExecServerClient> {
        ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
            self.websocket_url.clone(),
            CLIENT_NAME.to_string(),
        ))
        .await
        .context("client should connect to fake exec-server")
    }

    /// Waits for the scripted fake server to finish.
    async fn finish(self) -> Result<()> {
        self.task
            .await
            .context("fake exec-server task should join")??;
        Ok(())
    }
}

/// Starts a fake exec-server that accepts one WebSocket client.
async fn spawn_scripted_exec_server<F, Fut>(script: F) -> Result<ScriptedExecServer>
where
    F: FnOnce(JsonRpcPeer) -> Fut + Send + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("fake exec-server should bind")?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let task = tokio::spawn(async move {
        let (stream, _) = timeout(TEST_TIMEOUT, listener.accept())
            .await
            .context("fake exec-server should accept a client")??;
        let websocket = accept_async(stream)
            .await
            .context("fake exec-server websocket handshake should complete")?;
        let mut peer = JsonRpcPeer { websocket };
        peer.complete_initialize().await?;
        script(peer).await
    });
    Ok(ScriptedExecServer {
        websocket_url,
        task,
    })
}

/// JSON-RPC peer for the fake exec-server WebSocket.
struct JsonRpcPeer {
    websocket: WebSocketStream<TcpStream>,
}

impl JsonRpcPeer {
    /// Completes and validates the client initialize handshake.
    async fn complete_initialize(&mut self) -> Result<()> {
        let request = self.read_request(INITIALIZE_METHOD).await?;
        let params: InitializeParams = decode_request_params(&request)?;
        assert_eq!(
            params,
            InitializeParams {
                client_name: CLIENT_NAME.to_string(),
                resume_session_id: None,
            }
        );
        self.write_response(
            request.id,
            InitializeResponse {
                session_id: "session-1".to_string(),
            },
        )
        .await?;
        self.read_notification(INITIALIZED_METHOD).await?;
        Ok(())
    }

    /// Reads one typed `http/request` call from the client.
    async fn read_http_request(&mut self) -> Result<(RequestId, HttpRequestParams)> {
        let request = self.read_request(HTTP_REQUEST_METHOD).await?;
        let params = decode_request_params(&request)?;
        Ok((request.id, params))
    }

    /// Reads a JSON-RPC request and validates its method.
    async fn read_request(&mut self, expected_method: &str) -> Result<JSONRPCRequest> {
        let message = self.read_message().await?;
        let JSONRPCMessage::Request(request) = message else {
            bail!("expected JSON-RPC request `{expected_method}`, got {message:?}");
        };
        if request.method != expected_method {
            bail!(
                "expected JSON-RPC request `{expected_method}`, got `{}`",
                request.method
            );
        }
        Ok(request)
    }

    /// Reads a JSON-RPC notification and validates its method.
    async fn read_notification(&mut self, expected_method: &str) -> Result<JSONRPCNotification> {
        let message = self.read_message().await?;
        let JSONRPCMessage::Notification(notification) = message else {
            bail!("expected JSON-RPC notification `{expected_method}`, got {message:?}");
        };
        if notification.method != expected_method {
            bail!(
                "expected JSON-RPC notification `{expected_method}`, got `{}`",
                notification.method
            );
        }
        Ok(notification)
    }

    /// Sends a successful JSON-RPC response.
    async fn write_response<T>(&mut self, id: RequestId, result: T) -> Result<()>
    where
        T: Serialize,
    {
        self.write_message(JSONRPCMessage::Response(JSONRPCResponse {
            id,
            result: to_value(result)?,
        }))
        .await
    }

    /// Sends one streamed HTTP body notification.
    async fn write_body_delta(&mut self, delta: HttpRequestBodyDeltaNotification) -> Result<()> {
        self.write_message(JSONRPCMessage::Notification(JSONRPCNotification {
            method: HTTP_REQUEST_BODY_DELTA_METHOD.to_string(),
            params: Some(to_value(delta)?),
        }))
        .await
    }

    /// Reads one WebSocket JSON-RPC message.
    async fn read_message(&mut self) -> Result<JSONRPCMessage> {
        let message = timeout(TEST_TIMEOUT, self.websocket.next())
            .await
            .context("timed out waiting for JSON-RPC message")?
            .context("client websocket closed before JSON-RPC message arrived")?
            .context("failed to read websocket message")?;
        match message {
            Message::Text(text) => from_str(text.as_ref()).context("text JSON-RPC"),
            Message::Binary(bytes) => from_slice(bytes.as_ref()).context("binary JSON-RPC"),
            Message::Close(frame) => bail!("client websocket closed: {frame:?}"),
            other => bail!("expected text or binary JSON-RPC message, got {other:?}"),
        }
    }

    /// Writes one WebSocket JSON-RPC message.
    async fn write_message(&mut self, message: JSONRPCMessage) -> Result<()> {
        let encoded = to_string(&message)?;
        timeout(
            TEST_TIMEOUT,
            self.websocket.send(Message::Text(encoded.into())),
        )
        .await
        .context("timed out writing JSON-RPC message")?
        .context("failed to write JSON-RPC message")
    }
}

/// Decodes a request params object into its typed protocol payload.
fn decode_request_params<T>(request: &JSONRPCRequest) -> Result<T>
where
    T: DeserializeOwned,
{
    let params = request
        .params
        .clone()
        .context("JSON-RPC request should include params")?;
    from_value(params).context("JSON-RPC request params should decode")
}
