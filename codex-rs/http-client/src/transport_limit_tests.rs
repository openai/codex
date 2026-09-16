//! HTTP fixtures exercise opt-in body limits for both buffered and streaming callers.

use super::*;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::time::Duration;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
enum Delivery {
    Buffered,
    Streamed,
}

#[tokio::test]
async fn accepts_complete_bodies_at_the_limit() {
    for delivery in [Delivery::Buffered, Delivery::Streamed] {
        for (framing, wire_body, max_bytes, expected_body) in [
            ("Content-Length: 4\r\n", "test", 4, "test"),
            ("", "test", 4, "test"),
            (
                "Transfer-Encoding: chunked\r\n",
                "2\r\nte\r\n2\r\nst\r\n0\r\n\r\n",
                4,
                "test",
            ),
            ("Content-Length: 0\r\n", "", 0, ""),
        ] {
            let response = receive_response(
                format!("HTTP/1.1 200 OK\r\n{framing}Connection: close\r\n\r\n{wire_body}"),
                &transport(),
                Some(max_bytes),
                delivery,
            )
            .await
            .expect("a complete body within the limit should succeed");
            let mut expected_headers = HeaderMap::new();
            expected_headers.insert("connection", "close".parse().unwrap());
            for line in framing.lines() {
                let (name, value) = line.split_once(':').unwrap();
                expected_headers.insert(
                    http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                    value.trim().parse().unwrap(),
                );
            }
            assert_eq!(
                (response.status, response.headers, response.body),
                (
                    StatusCode::OK,
                    expected_headers,
                    Bytes::copy_from_slice(expected_body.as_bytes()),
                ),
                "{delivery:?} with framing {framing:?}",
            );
        }
    }
}

#[tokio::test]
async fn rejects_oversized_success_and_error_bodies_without_sensitive_diagnostics() {
    for delivery in [Delivery::Buffered, Delivery::Streamed] {
        for status in ["200 OK", "500 Internal Server Error"] {
            for (framing, wire_body) in [
                ("Content-Length: 11\r\n", "body-secret"),
                ("", "body-secret"),
                (
                    "Transfer-Encoding: chunked\r\n",
                    "4\r\nbody\r\n7\r\n-secret\r\n0\r\n\r\n",
                ),
                (
                    "Transfer-Encoding: chunked\r\nContent-Length: 1\r\n",
                    "4\r\nbody\r\n7\r\n-secret\r\n0\r\n\r\n",
                ),
            ] {
                let error = receive_response(
                    format!(
                        "HTTP/1.1 {status}\r\n{framing}X-Secret: header-secret\r\nConnection: close\r\n\r\n{wire_body}"
                    ),
                    &transport(),
                    Some(4),
                    delivery,
                )
                .await
                .expect_err("the observed body must not exceed the limit");
                assert_eq!(
                    (error.to_string(), format!("{error:?}")),
                    (
                        "response body exceeds the 4 byte limit".to_string(),
                        "ResponseTooLarge { max_bytes: 4 }".to_string(),
                    ),
                    "{delivery:?}, {status}, framing {framing:?}",
                );
            }
        }
    }
}

#[tokio::test]
async fn rejects_oversized_declared_lengths_before_reading_the_body() {
    for delivery in [Delivery::Buffered, Delivery::Streamed] {
        for status in [
            StatusCode::OK,
            StatusCode::UNAUTHORIZED,
            StatusCode::TOO_MANY_REQUESTS,
        ] {
            let error = receive_response(
                format!(
                    "HTTP/1.1 {status}\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\n"
                ),
                &transport(),
                Some(4),
                delivery,
            )
            .await
            .expect_err("an oversized declaration should fail before the incomplete body");
            assert!(matches!(
                error,
                TransportError::ResponseTooLarge { max_bytes: 4 }
            ));
        }
    }
}

#[tokio::test]
async fn streaming_limit_counts_across_chunks_and_terminates_on_error() {
    let (continue_tx, continue_rx) = std::sync::mpsc::channel();
    let (url, server) = start_server(move |connection| {
        connection
            .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\npart\r\n")
            .expect("write the first chunk");
        continue_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("client should consume the first chunk");
        connection
            .write_all(b"4\r\nnext\r\n0\r\n\r\n")
            .expect("write the second chunk");
    });
    let mut request = Request::new(Method::GET, url);
    request.response_body_limit_bytes = Some(6);
    let mut response = transport()
        .stream(request)
        .await
        .expect("stream should open before the body limit is reached");
    assert_eq!(
        response.bytes.next().await.unwrap().unwrap(),
        Bytes::from_static(b"part"),
    );
    continue_tx.send(()).expect("release the second chunk");
    let error = response
        .bytes
        .next()
        .await
        .expect("second chunk should produce the limit error")
        .expect_err("two individually small chunks should exceed the total limit");
    assert!(matches!(
        error,
        TransportError::ResponseTooLarge { max_bytes: 6 }
    ));
    assert!(response.bytes.next().await.is_none());
    server.join().expect("HTTP fixture should finish");
}

#[tokio::test]
async fn preserves_http_failures_within_the_limit() {
    for delivery in [Delivery::Buffered, Delivery::Streamed] {
        let error = receive_response(
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 6\r\nConnection: close\r\n\r\ndenied"
                .to_string(),
            &transport(),
            Some(6),
            delivery,
        )
        .await
        .expect_err("a small error body should retain HTTP error semantics");
        let TransportError::Http {
            status,
            headers,
            body,
            ..
        } = error
        else {
            panic!("expected HTTP error, got {error:?}");
        };
        assert_eq!(
            (status, headers, body),
            (
                StatusCode::FORBIDDEN,
                Some(HeaderMap::from_iter([
                    (http::header::CONTENT_LENGTH, "6".parse().unwrap()),
                    (http::header::CONNECTION, "close".parse().unwrap()),
                ])),
                Some("denied".to_string()),
            ),
        );
    }
}

#[tokio::test]
async fn bounded_errors_preserve_status_when_the_body_is_interrupted() {
    for delivery in [Delivery::Buffered, Delivery::Streamed] {
        for status in [StatusCode::UNAUTHORIZED, StatusCode::TOO_MANY_REQUESTS] {
            for max_bytes in [None, Some(5), Some(4), Some(3)] {
                // The first chunk is complete, but the peer closes before the next chunk.
                let error = receive_response(
                    format!(
                        "HTTP/1.1 {status}\r\nTransfer-Encoding: chunked\r\nRetry-After: 7\r\nWWW-Authenticate: Bearer\r\nConnection: close\r\n\r\n4\r\npart\r\n"
                    ),
                    &transport(),
                    max_bytes,
                    delivery,
                )
                .await
                .expect_err("an interrupted error body must still fail");
                if matches!(delivery, Delivery::Buffered) && max_bytes.is_none() {
                    assert!(matches!(error, TransportError::Network(_)));
                    continue;
                }
                if max_bytes == Some(3) {
                    assert!(matches!(
                        error,
                        TransportError::ResponseTooLarge { max_bytes: 3 }
                    ));
                    continue;
                }
                let TransportError::Http {
                    status: actual_status,
                    headers,
                    body,
                    ..
                } = error
                else {
                    panic!("expected preserved HTTP error, got {error:?}");
                };
                assert_eq!(
                    (actual_status, headers, body),
                    (
                        status,
                        Some(HeaderMap::from_iter([
                            (http::header::TRANSFER_ENCODING, "chunked".parse().unwrap()),
                            (http::header::RETRY_AFTER, "7".parse().unwrap()),
                            (http::header::WWW_AUTHENTICATE, "Bearer".parse().unwrap()),
                            (http::header::CONNECTION, "close".parse().unwrap()),
                        ])),
                        None,
                    ),
                    "{delivery:?}, max_bytes={max_bytes:?}",
                );
            }
        }
    }
}

#[tokio::test]
async fn bounded_successes_report_interrupted_bodies_as_network_errors() {
    for delivery in [Delivery::Buffered, Delivery::Streamed] {
        let error = receive_response(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\npart\r\n"
                .to_string(),
            &transport(),
            Some(4),
            delivery,
        )
        .await
        .expect_err("a successful status cannot hide an incomplete response body");
        assert!(matches!(error, TransportError::Network(_)));
    }
}

#[tokio::test]
async fn bounded_streamed_errors_preserve_text_decoding() {
    for (content_type, bytes, expected) in [
        (
            "text/plain; charset=windows-1252",
            [0x63, 0x61, 0x66, 0xe9].as_slice(),
            "café",
        ),
        (
            "text/plain; charset=utf-8",
            b"error: \xff".as_slice(),
            "error: �",
        ),
        (
            "text/plain; charset=unknown",
            b"error: \xff".as_slice(),
            "error: �",
        ),
        ("text/plain", b"\xef\xbb\xbfdenied".as_slice(), "denied"),
    ] {
        for bounded in [false, true] {
            let mut wire = format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            )
            .into_bytes();
            wire.extend_from_slice(bytes);
            let error = receive_response(
                wire,
                &transport(),
                bounded.then_some(bytes.len()),
                Delivery::Streamed,
            )
            .await
            .expect_err("retain the HTTP error and decoded diagnostics");
            let TransportError::Http {
                status,
                headers,
                body,
                ..
            } = error
            else {
                panic!("expected HTTP error, got {error:?}");
            };
            assert_eq!(
                (status, headers, body),
                (
                    StatusCode::FORBIDDEN,
                    Some(HeaderMap::from_iter([
                        (http::header::CONTENT_TYPE, content_type.parse().unwrap()),
                        (
                            http::header::CONTENT_LENGTH,
                            bytes.len().to_string().parse().unwrap()
                        ),
                        (http::header::CONNECTION, "close".parse().unwrap()),
                    ])),
                    Some(expected.to_string()),
                ),
                "{content_type}, bounded={bounded}",
            );
        }
    }
}

#[tokio::test]
async fn shared_transport_keeps_each_requests_response_limit_independent() {
    let transport = transport();
    for delivery in [Delivery::Buffered, Delivery::Streamed] {
        for max_bytes in [Some(4), None, Some(11), Some(0), None] {
            let response = receive_response(
                "HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nbody-secret"
                    .to_string(),
                &transport,
                max_bytes,
                delivery,
            )
            .await;
            match max_bytes {
                Some(max_bytes) if max_bytes < 11 => assert!(matches!(
                    response,
                    Err(TransportError::ResponseTooLarge { max_bytes: actual }) if actual == max_bytes
                )),
                _ => assert_eq!(
                    response.expect("only this request's limit applies").body,
                    Bytes::from_static(b"body-secret"),
                ),
            }
        }
    }
}

fn transport() -> ReqwestTransport {
    ReqwestTransport::new(
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("test client should build"),
    )
}

async fn receive_response(
    wire_response: impl Into<Vec<u8>>,
    transport: &ReqwestTransport,
    response_body_limit_bytes: Option<usize>,
    delivery: Delivery,
) -> Result<Response, TransportError> {
    let wire_response = wire_response.into();
    let (url, server) = start_server(move |connection| {
        connection
            .write_all(&wire_response)
            .expect("write HTTP response");
    });
    let mut request = Request::new(Method::GET, url);
    request.headers.insert(
        http::header::AUTHORIZATION,
        "Bearer auth-secret".parse().unwrap(),
    );
    request.timeout = Some(Duration::from_secs(5));
    request.response_body_limit_bytes = response_body_limit_bytes;
    let result = match delivery {
        Delivery::Buffered => transport.execute(request).await,
        Delivery::Streamed => {
            async {
                let mut response = transport.stream(request).await?;
                let mut body = BytesMut::new();
                while let Some(chunk) = response.bytes.next().await {
                    body.extend_from_slice(&chunk?);
                }
                Ok(Response {
                    status: response.status,
                    headers: response.headers,
                    body: body.freeze(),
                })
            }
            .await
        }
    };
    server.join().expect("HTTP fixture should finish");
    result
}

fn start_server(
    respond: impl FnOnce(&mut TcpStream) + Send + 'static,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("HTTP fixture should bind");
    let address = listener.local_addr().expect("HTTP fixture address");
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let (mut connection, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "HTTP fixture accept timed out");
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("HTTP fixture should accept: {error}"),
            }
        };
        connection
            .set_nonblocking(false)
            .expect("HTTP fixture connection should use blocking reads");
        connection
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("HTTP fixture read timeout");
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let count = connection.read(&mut buffer).expect("read HTTP request");
            assert_ne!(count, 0, "HTTP request should contain complete headers");
            request.extend_from_slice(&buffer[..count]);
        }
        respond(&mut connection);
    });
    (
        format!("http://{address}/models?credential=url-secret"),
        server,
    )
}
