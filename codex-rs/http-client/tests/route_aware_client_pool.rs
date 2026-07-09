use std::io;
use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::RouteAwareClientPool;
use http::StatusCode;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;

#[tokio::test]
async fn disabled_pool_logging_does_not_expose_request_or_response_data() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("HTTP listener should bind");
    let address = listener
        .local_addr()
        .expect("HTTP listener should have an address");
    let server_thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("HTTP listener should accept");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("HTTP stream should get a read timeout");
        let mut buffer = [0_u8; 4096];
        let _size = stream.read(&mut buffer).expect("HTTP request should read");
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nx-sensitive-response: response-secret-value\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
            )
            .expect("HTTP response should write");
    });
    let endpoint = format!(
        "http://auth-user:password-secret-value@{address}/token?client_secret=query-secret-value"
    );
    let pool = RouteAwareClientPool::with_chatgpt_cloudflare_cookies_without_request_logging(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
    );
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(TestLogWriter {
                buffer: Arc::clone(&buffer),
            })
            .with_filter(
                tracing_subscriber::filter::Targets::new()
                    .with_target("codex_http_client", tracing::Level::TRACE),
            ),
    );
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::debug!(target: "codex_http_client", "log capture sentinel");

    let response = pool
        .post(&endpoint)
        .header("x-sensitive-request", "request-header-secret-value")
        .body("request-body-secret-value")
        .send()
        .await
        .expect("route-aware request should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    server_thread.join().expect("server thread should finish");

    let unresponsive_listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("unresponsive listener should bind");
    let unresponsive_address = unresponsive_listener
        .local_addr()
        .expect("unresponsive listener should have an address");
    let unresponsive_endpoint = format!(
        "http://auth-user:failure-password-secret-value@{unresponsive_address}/token?client_secret=failure-query-secret-value"
    );
    let error = pool
        .post(&unresponsive_endpoint)
        .timeout(Duration::from_millis(100))
        .send()
        .await
        .expect_err("request to an unresponsive listener should time out");
    assert!(error.is_timeout());

    let logs = String::from_utf8(buffer.lock().expect("log buffer lock").clone())
        .expect("logs should be UTF-8");
    assert!(logs.contains("log capture sentinel"));
    for secret in [
        "password-secret-value",
        "query-secret-value",
        "request-header-secret-value",
        "request-body-secret-value",
        "response-secret-value",
        "failure-password-secret-value",
        "failure-query-secret-value",
    ] {
        assert!(!logs.contains(secret), "logs exposed {secret}:\n{logs}");
    }
}

#[derive(Clone)]
struct TestLogWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

struct TestLogSink {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for TestLogWriter {
    type Writer = TestLogSink;

    fn make_writer(&'a self) -> Self::Writer {
        TestLogSink {
            buffer: Arc::clone(&self.buffer),
        }
    }
}

impl Write for TestLogSink {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut log_buffer = self
            .buffer
            .lock()
            .map_err(|_| io::Error::other("log buffer lock was poisoned"))?;
        log_buffer.extend(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
