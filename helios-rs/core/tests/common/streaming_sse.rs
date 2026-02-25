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
    pub gate: Option<oneshot::Receiver<()>>,
    pub body: String,
}

/// Minimal streaming SSE server for tests that need gated per-chunk delivery.
pub struct StreamingSseServer {
    uri: String,
    requests: Arc<TokioMutex<Vec<Vec<u8>>>>,
    shutdown: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl StreamingSseServer {
    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub async fn requests(&self) -> Vec<Vec<u8>> {
        self.requests.lock().await.clone()
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
    let requests = Arc::new(TokioMutex::new(Vec::new()));
    let requests_for_task = Arc::clone(&requests);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accept_res = listener.accept() => {
                    let (mut stream, _) = accept_res.expect("accept streaming SSE connection");
                    let state = Arc::clone(&state);
                    let requests = Arc::clone(&requests_for_task);
                    tokio::spawn(async move {
                        let (request, body_prefix) = read_http_request(&mut stream).await;
                        let Some((method, path)) = parse_request_line(&request) else {
                            let _ = write_http_response(&mut stream, 400, "bad request", "text/plain").await;
                            return;
                        };

                        if method == "GET" && path == "/v1/models" {
                            if read_request_body(&mut stream, &request, body_prefix)
                                .await
                                .is_err()
                            {
                                let _ = write_http_response(&mut stream, 400, "bad request", "text/plain").await;
                                return;
                            }
                            let body = serde_json::json!({
                                "data": [],
                                "object": "list"
                            })
                            .to_string();
                            let _ = write_http_response(&mut stream, 200, &body, "application/json").await;
                            return;
                        }

                        if method == "POST" && path == "/v1/responses" {
                            let body = match read_request_body(&mut stream, &request, body_prefix)
                                .await
                            {
                                Ok(body) => body,
                                Err(_) => {
                                    let _ = write_http_response(&mut stream, 400, "bad request", "text/plain").await;
                                    return;
                                }
                            };
                            requests.lock().await.push(body);
                            let Some((chunks, completion)) = take_next_stream(&state).await else {
                                let _ = write_http_response(&mut stream, 500, "no responses queued", "text/plain").await;
                                return;
                            };

                            if write_sse_headers(&mut stream).await.is_err() {
                                return;
                            }

                            for chunk in chunks {
                                if let Some(gate) = chunk.gate
                                    && gate.await.is_err() {
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
            requests,
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

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> (String, Vec<u8>) {
    let mut buf = Vec::new();
    let mut scratch = [0u8; 1024];
    loop {
        let read = stream.read(&mut scratch).await.unwrap_or(0);
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&scratch[..read]);
        if let Some(end) = header_terminator_index(&buf) {
            let header_end = end + 4;
            let header = String::from_utf8_lossy(&buf[..header_end]).into_owned();
            let rest = buf[header_end..].to_vec();
            return (header, rest);
        }
    }
    (String::from_utf8_lossy(&buf).into_owned(), Vec::new())
}

fn parse_request_line(request: &str) -> Option<(&str, &str)> {
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    Some((method, path))
}

fn header_terminator_index(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn content_length(headers: &str) -> Option<usize> {
    headers.lines().skip(1).find_map(|line| {
        let mut parts = line.splitn(2, ':');
        let name = parts.next()?.trim();
        let value = parts.next()?.trim();
        if name.eq_ignore_ascii_case("content-length") {
            value.parse::<usize>().ok()
        } else {
            None
        }
    })
}

async fn read_request_body(
    stream: &mut tokio::net::TcpStream,
    headers: &str,
    mut body_prefix: Vec<u8>,
) -> std::io::Result<Vec<u8>> {
    let Some(content_len) = content_length(headers) else {
        return Ok(body_prefix);
    };

    if body_prefix.len() > content_len {
        body_prefix.truncate(content_len);
    }

    let remaining = content_len.saturating_sub(body_prefix.len());
    if remaining == 0 {
        return Ok(body_prefix);
    }

    let mut rest = vec![0u8; remaining];
    stream.read_exact(&mut rest).await?;
    body_prefix.extend_from_slice(&rest);
    Ok(body_prefix)
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use reqwest::StatusCode;
    use tokio::net::TcpStream;
    use tokio::time::Duration;
    use tokio::time::timeout;

    fn split_response(response: &str) -> (&str, &str) {
        response
            .split_once("\r\n\r\n")
            .expect("response missing header separator")
    }

    fn status_code(headers: &str) -> u16 {
        let line = headers.lines().next().expect("status line");
        let mut parts = line.split_whitespace();
        let _ = parts.next();
        let status = parts.next().expect("status code");
        status.parse().expect("parse status code")
    }

    fn header_value<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
        headers.lines().skip(1).find_map(|line| {
            let mut parts = line.splitn(2, ':');
            let key = parts.next()?.trim();
            let value = parts.next()?.trim();
            if key.eq_ignore_ascii_case(name) {
                Some(value)
            } else {
                None
            }
        })
    }

    async fn connect(uri: &str) -> TcpStream {
        let addr = uri.strip_prefix("http://").expect("uri should be http");
        TcpStream::connect(addr)
            .await
            .expect("connect to streaming SSE server")
    }

    async fn read_to_end(stream: &mut TcpStream) -> String {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.expect("read response");
        String::from_utf8_lossy(&buf).into_owned()
    }

    async fn read_until(stream: &mut TcpStream, needle: &str) -> (String, String) {
        let mut buf = Vec::new();
        let mut scratch = [0u8; 256];
        let needle_bytes = needle.as_bytes();
        loop {
            let read = stream.read(&mut scratch).await.expect("read response");
            if read == 0 {
                break;
            }
            buf.extend_from_slice(&scratch[..read]);
            if let Some(pos) = buf
                .windows(needle_bytes.len())
                .position(|window| window == needle_bytes)
            {
                let end = pos + needle_bytes.len();
                let headers = String::from_utf8_lossy(&buf[..end]).into_owned();
                let remainder = String::from_utf8_lossy(&buf[end..]).into_owned();
                return (headers, remainder);
            }
        }
        (String::from_utf8_lossy(&buf).into_owned(), String::new())
    }

    async fn send_request(stream: &mut TcpStream, request: &str) {
        stream
            .write_all(request.as_bytes())
            .await
            .expect("write request");
    }

    #[tokio::test]
    async fn get_models_returns_empty_list() {
        let (server, _) = start_streaming_sse_server(Vec::new()).await;
        let mut stream = connect(server.uri()).await;
        send_request(
            &mut stream,
            "GET /v1/models HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        )
        .await;
        let response = read_to_end(&mut stream).await;
        let (headers, body) = split_response(&response);
        assert_eq!(status_code(headers), 200);
        assert_eq!(
            header_value(headers, "content-type"),
            Some("application/json")
        );
        let parsed: serde_json::Value = serde_json::from_str(body).expect("parse json body");
        assert_eq!(
            parsed,
            serde_json::json!({
                "data": [],
                "object": "list"
            })
        );
        server.shutdown().await;
    }

    #[tokio::test]
    async fn post_responses_streams_in_order_and_closes() {
        let chunks = vec![
            StreamingSseChunk {
                gate: None,
                body: "event: one\n\n".to_string(),
            },
            StreamingSseChunk {
                gate: None,
                body: "event: two\n\n".to_string(),
            },
        ];
        let (server, mut completions) = start_streaming_sse_server(vec![chunks]).await;
        let mut stream = connect(server.uri()).await;
        send_request(
            &mut stream,
            "POST /v1/responses HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n",
        )
        .await;
        let response = read_to_end(&mut stream).await;
        let (headers, body) = split_response(&response);
        assert_eq!(status_code(headers), 200);
        assert_eq!(
            header_value(headers, "content-type"),
            Some("text/event-stream")
        );
        assert_eq!(body, "event: one\n\nevent: two\n\n");
        let mut extra = [0u8; 1];
        let read = stream.read(&mut extra).await.expect("read after eof");
        assert_eq!(read, 0);
        let completion = completions.pop().expect("completion receiver");
        let timestamp = completion.await.expect("completion timestamp");
        assert!(timestamp > 0);
        server.shutdown().await;
    }

    #[tokio::test]
    async fn none_gate_streams_immediately() {
        let chunks = vec![StreamingSseChunk {
            gate: None,
            body: "event: immediate\n\n".to_string(),
        }];
        let (server, _) = start_streaming_sse_server(vec![chunks]).await;
        let mut stream = connect(server.uri()).await;
        send_request(
            &mut stream,
            "POST /v1/responses HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n",
        )
        .await;
        let (headers, remainder) = read_until(&mut stream, "\r\n\r\n").await;
        let (headers, _) = split_response(&headers);
        assert_eq!(status_code(headers), 200);
        let immediate = format!("{remainder}{}", read_to_end(&mut stream).await);
        assert_eq!(immediate, "event: immediate\n\n");
        server.shutdown().await;
    }

    #[tokio::test]
    async fn post_responses_with_no_queue_returns_500() {
        let (server, _) = start_streaming_sse_server(Vec::new()).await;
        let mut stream = connect(server.uri()).await;
        send_request(
            &mut stream,
            "POST /v1/responses HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n",
        )
        .await;
        let response = read_to_end(&mut stream).await;
        let (headers, body) = split_response(&response);
        assert_eq!(status_code(headers), 500);
        assert_eq!(header_value(headers, "content-type"), Some("text/plain"));
        assert_eq!(body, "no responses queued");
        server.shutdown().await;
    }

    #[tokio::test]
    async fn gated_chunks_wait_for_signal_and_preserve_order() {
        let (gate_one_tx, gate_one_rx) = oneshot::channel();
        let (gate_two_tx, gate_two_rx) = oneshot::channel();
        let chunks = vec![
            StreamingSseChunk {
                gate: Some(gate_one_rx),
                body: "event: one\n\n".to_string(),
            },
            StreamingSseChunk {
                gate: Some(gate_two_rx),
                body: "event: two\n\n".to_string(),
            },
        ];
        let (server, _) = start_streaming_sse_server(vec![chunks]).await;
        let mut stream = connect(server.uri()).await;
        send_request(
            &mut stream,
            "POST /v1/responses HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n",
        )
        .await;
        let (headers, remainder) = read_until(&mut stream, "\r\n\r\n").await;
        let (headers, _) = split_response(&headers);
        assert_eq!(status_code(headers), 200);
        assert_eq!(
            header_value(headers, "content-type"),
            Some("text/event-stream")
        );
        assert!(
            remainder.is_empty(),
            "unexpected body before gate: {remainder:?}"
        );
        let mut scratch = [0u8; 32];
        let pending = timeout(Duration::from_millis(200), stream.read(&mut scratch)).await;
        assert!(pending.is_err());

        let _ = gate_one_tx.send(());
        let mut first_chunk = vec![0u8; "event: one\n\n".len()];
        stream
            .read_exact(&mut first_chunk)
            .await
            .expect("read first chunk");
        assert_eq!(String::from_utf8_lossy(&first_chunk), "event: one\n\n");
        let pending = timeout(Duration::from_millis(200), stream.read(&mut scratch)).await;
        assert!(pending.is_err());

        let _ = gate_two_tx.send(());
        let remaining = read_to_end(&mut stream).await;
        assert_eq!(remaining, "event: two\n\n");
        server.shutdown().await;
    }

    #[tokio::test]
    async fn multiple_responses_are_fifo_and_completion_timestamps_monotonic() {
        let first_chunks = vec![StreamingSseChunk {
            gate: None,
            body: "event: first\n\n".to_string(),
        }];
        let second_chunks = vec![StreamingSseChunk {
            gate: None,
            body: "event: second\n\n".to_string(),
        }];
        let (server, mut completions) =
            start_streaming_sse_server(vec![first_chunks, second_chunks]).await;

        let mut first_stream = connect(server.uri()).await;
        send_request(
            &mut first_stream,
            "POST /v1/responses HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n",
        )
        .await;
        let first_response = read_to_end(&mut first_stream).await;
        let (_, first_body) = split_response(&first_response);
        assert_eq!(first_body, "event: first\n\n");

        let mut second_stream = connect(server.uri()).await;
        send_request(
            &mut second_stream,
            "POST /v1/responses HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n",
        )
        .await;
        let second_response = read_to_end(&mut second_stream).await;
        let (_, second_body) = split_response(&second_response);
        assert_eq!(second_body, "event: second\n\n");

        let first_completion = completions.remove(0);
        let second_completion = completions.remove(0);
        let first_timestamp = first_completion.await.expect("first completion");
        let second_timestamp = second_completion.await.expect("second completion");
        assert!(first_timestamp > 0);
        assert!(second_timestamp > 0);
        assert!(first_timestamp <= second_timestamp);
        assert!(completions.is_empty());
        server.shutdown().await;
    }

    #[tokio::test]
    async fn unknown_route_returns_404() {
        let (server, _) = start_streaming_sse_server(Vec::new()).await;
        let mut stream = connect(server.uri()).await;
        send_request(
            &mut stream,
            "GET /v1/unknown HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        )
        .await;
        let response = read_to_end(&mut stream).await;
        let (headers, body) = split_response(&response);
        assert_eq!(status_code(headers), 404);
        assert_eq!(header_value(headers, "content-type"), Some("text/plain"));
        assert_eq!(body, "not found");
        server.shutdown().await;
    }

    #[tokio::test]
    async fn malformed_request_returns_400() {
        let (server, _) = start_streaming_sse_server(Vec::new()).await;
        let mut stream = connect(server.uri()).await;
        send_request(&mut stream, "BAD\r\n\r\n").await;
        let response = read_to_end(&mut stream).await;
        let (headers, body) = split_response(&response);
        assert_eq!(status_code(headers), 400);
        assert_eq!(header_value(headers, "content-type"), Some("text/plain"));
        assert_eq!(body, "bad request");
        server.shutdown().await;
    }

    #[tokio::test]
    async fn responses_post_drains_request_body() {
        let response_body = r#"event: response.completed
data: {"type":"response.completed","response":{"id":"resp-1"}}

"#;
        let (server, mut completions) = start_streaming_sse_server(vec![vec![StreamingSseChunk {
            gate: None,
            body: response_body.to_string(),
        }]])
        .await;

        let url = format!("{}/v1/responses", server.uri());
        let payload = serde_json::json!({
            "model": "gpt-5.1",
            "instructions": "test",
            "input": [{"type": "message", "role": "user", "content": [{"type": "input_text", "text": "hello"}]}],
            "stream": true
        });

        let resp = reqwest::Client::new()
            .post(url)
            .json(&payload)
            .send()
            .await
            .expect("send request");
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = resp.bytes().await.expect("read response body");
        assert_eq!(bytes, response_body.as_bytes());

        let completion = completions.remove(0);
        let completed_at = completion.await.expect("completion timestamp");
        assert!(completed_at > 0);

        server.shutdown().await;
    }

    #[tokio::test]
    async fn read_http_request_returns_after_header_terminator() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("listener address");
        let (tx, rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept client");
            let (request, body) = read_http_request(&mut stream).await;
            let _ = tx.send((request, body));
        });

        let mut client = TcpStream::connect(addr)
            .await
            .expect("connect to test listener");
        let request = "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        client
            .write_all(request.as_bytes())
            .await
            .expect("write request");
        let (received, body) = timeout(Duration::from_millis(200), rx)
            .await
            .expect("read_http_request timed out")
            .expect("receive request");
        assert_eq!(received, request);
        assert!(body.is_empty());
        drop(client);
        let _ = server_task.await;
    }

    #[test]
    fn parse_request_line_handles_valid_and_invalid() {
        assert_eq!(parse_request_line(""), None);
        assert_eq!(parse_request_line("BAD"), None);
        assert_eq!(
            parse_request_line("GET /v1/models HTTP/1.1"),
            Some(("GET", "/v1/models"))
        );
    }

    #[tokio::test]
    async fn take_next_stream_consumes_in_lockstep() {
        let (first_tx, first_rx) = oneshot::channel();
        let (second_tx, second_rx) = oneshot::channel();
        let state = TokioMutex::new(StreamingSseState {
            responses: VecDeque::from(vec![
                vec![StreamingSseChunk {
                    gate: None,
                    body: "first".to_string(),
                }],
                vec![StreamingSseChunk {
                    gate: None,
                    body: "second".to_string(),
                }],
            ]),
            completions: VecDeque::from(vec![first_tx, second_tx]),
        });

        let (first_chunks, first_completion) =
            take_next_stream(&state).await.expect("first stream");
        assert_eq!(first_chunks[0].body, "first");
        let _ = first_completion.send(11);
        assert_eq!(first_rx.await.expect("first completion"), 11);

        let (second_chunks, second_completion) =
            take_next_stream(&state).await.expect("second stream");
        assert_eq!(second_chunks[0].body, "second");
        let _ = second_completion.send(22);
        assert_eq!(second_rx.await.expect("second completion"), 22);

        let third = take_next_stream(&state).await;
        assert!(third.is_none());
    }

    #[tokio::test]
    async fn shutdown_terminates_accept_loop() {
        let (server, _) = start_streaming_sse_server(Vec::new()).await;
        let shutdown = timeout(Duration::from_millis(200), server.shutdown()).await;
        assert!(shutdown.is_ok());
    }
}
