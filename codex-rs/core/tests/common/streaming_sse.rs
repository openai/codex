use std::collections::VecDeque;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::oneshot;

/// Streaming SSE chunk payload gated by a per-chunk signal.
#[derive(Debug)]
pub struct StreamingSseChunk {
    pub gate: oneshot::Receiver<()>,
    pub body: String,
}

/// Minimal streaming SSE server for tests that need gated per-chunk delivery.
pub struct StreamingSseServer {
    uri: String,
    shutdown: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl StreamingSseServer {
    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown.send(());
        let _ = self.task.await;
    }
}

/// Starts a lightweight HTTP server that supports:
/// - GET /v1/models -> empty models response
/// - POST /v1/responses -> SSE stream gated per-chunk, served in order
///
/// Returns the server handle and a list of receivers that fire when each
/// response stream finishes sending its final chunk.
pub async fn start_streaming_sse_server(
    responses: Vec<Vec<StreamingSseChunk>>,
) -> (StreamingSseServer, Vec<oneshot::Receiver<i64>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind streaming SSE server");
    let addr = listener.local_addr().expect("streaming SSE server address");
    let uri = format!("http://{addr}");

    let mut completion_senders = Vec::with_capacity(responses.len());
    let mut completion_receivers = Vec::with_capacity(responses.len());
    for _ in 0..responses.len() {
        let (tx, rx) = oneshot::channel();
        completion_senders.push(tx);
        completion_receivers.push(rx);
    }

    let state = Arc::new(TokioMutex::new(StreamingSseState {
        responses: VecDeque::from(responses),
        completions: VecDeque::from(completion_senders),
    }));
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accept_res = listener.accept() => {
                    let (mut stream, _) = accept_res.expect("accept streaming SSE connection");
                    let state = Arc::clone(&state);
                    tokio::spawn(async move {
                        let request = read_http_request(&mut stream).await;
                        let Some((method, path)) = parse_request_line(&request) else {
                            let _ = write_http_response(&mut stream, 400, "bad request", "text/plain").await;
                            return;
                        };

                        if method == "GET" && path == "/v1/models" {
                            let body = serde_json::json!({
                                "data": [],
                                "object": "list"
                            })
                            .to_string();
                            let _ = write_http_response(&mut stream, 200, &body, "application/json").await;
                            return;
                        }

                        if method == "POST" && path == "/v1/responses" {
                            let Some((chunks, completion)) = take_next_stream(&state).await else {
                                let _ = write_http_response(&mut stream, 500, "no responses queued", "text/plain").await;
                                return;
                            };

                            if write_sse_headers(&mut stream).await.is_err() {
                                return;
                            }

                            for chunk in chunks {
                                if chunk.gate.await.is_err() {
                                    return;
                                }
                                if stream.write_all(chunk.body.as_bytes()).await.is_err() {
                                    return;
                                }
                                let _ = stream.flush().await;
                            }

                            let _ = completion.send(unix_ms_now());
                            let _ = stream.shutdown().await;
                            return;
                        }

                        let _ = write_http_response(&mut stream, 404, "not found", "text/plain").await;
                    });
                }
            }
        }
    });

    (
        StreamingSseServer {
            uri,
            shutdown: shutdown_tx,
            task,
        },
        completion_receivers,
    )
}

struct StreamingSseState {
    responses: VecDeque<Vec<StreamingSseChunk>>,
    completions: VecDeque<oneshot::Sender<i64>>,
}

async fn take_next_stream(
    state: &TokioMutex<StreamingSseState>,
) -> Option<(Vec<StreamingSseChunk>, oneshot::Sender<i64>)> {
    let mut guard = state.lock().await;
    let chunks = guard.responses.pop_front()?;
    let completion = guard.completions.pop_front()?;
    Some((chunks, completion))
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut scratch = [0u8; 1024];
    loop {
        let read = stream.read(&mut scratch).await.unwrap_or(0);
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&scratch[..read]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn parse_request_line(request: &str) -> Option<(&str, &str)> {
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    Some((method, path))
}

async fn write_sse_headers(stream: &mut tokio::net::TcpStream) -> std::io::Result<()> {
    let headers = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n";
    stream.write_all(headers.as_bytes()).await
}

async fn write_http_response(
    stream: &mut tokio::net::TcpStream,
    status: i64,
    body: &str,
    content_type: &str,
) -> std::io::Result<()> {
    let body_len = body.len();
    let headers = format!(
        "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {body_len}\r\nconnection: close\r\n\r\n"
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.shutdown().await
}

fn unix_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
