use std::io;
use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use http::header::AUTHORIZATION;
use http::header::COOKIE;
use http::header::PROXY_AUTHORIZATION;
use pretty_assertions::assert_eq;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;

use super::*;
use crate::RouteAwareClientPool;

#[tokio::test]
async fn route_aware_pool_re_resolves_redirects_and_logs_only_final_outcome() {
    let (proxy_addr, proxy_thread) =
        spawn_response("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    let (redirect_addr, redirect_thread) = spawn_response(
        "HTTP/1.1 302 Found\r\nLocation: /final?token=redirect-target-secret\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let initial_url = format!("http://{redirect_addr}/start");
    let redirected_url = format!("http://{redirect_addr}/final?token=redirect-target-secret");
    cache_system_proxy_decision(&initial_url, SystemProxyDecision::Direct);
    cache_system_proxy_decision(
        &redirected_url,
        SystemProxyDecision::Proxy {
            url: format!("http://{proxy_addr}"),
        },
    );
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ClientRouteClass::Api,
    );
    let log_buffer = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(TestLogWriter {
                buffer: Arc::clone(&log_buffer),
            })
            .with_filter(
                tracing_subscriber::filter::Targets::new()
                    .with_target("codex_http_client", tracing::Level::TRACE),
            ),
    );
    let _guard = tracing::subscriber::set_default(subscriber);

    tokio::time::timeout(
        Duration::from_secs(2),
        pool.get(&initial_url)
            .header(AUTHORIZATION, "Bearer origin-secret")
            .header(COOKIE, "session=origin-secret")
            .header(PROXY_AUTHORIZATION, "Basic proxy-secret")
            .send(),
    )
    .await
    .expect("redirected request should finish")
    .expect("redirected request should use both selected routes");
    redirect_thread
        .join()
        .expect("redirect listener should finish");
    let redirected_request = proxy_thread.join().expect("proxy listener should finish");
    assert_eq!(
        redirected_request.lines().next(),
        Some(format!("GET {redirected_url} HTTP/1.1").as_str())
    );
    assert!(has_header(&redirected_request, "authorization"));
    assert!(has_header(&redirected_request, "cookie"));
    assert!(!has_header(&redirected_request, "proxy-authorization"));

    let logs = String::from_utf8(log_buffer.lock().expect("log buffer lock").clone())
        .expect("logs should be UTF-8");
    assert!(logs.contains(&initial_url));
    assert_eq!(logs.matches("Request completed").count(), 1);
    assert!(!logs.contains("redirect-target-secret"));
}

fn spawn_response(
    response: &'static str,
) -> (std::net::SocketAddr, std::thread::JoinHandle<String>) {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
    let address = listener.local_addr().expect("HTTP listener address");
    listener.set_nonblocking(true).expect("set nonblocking");
    let thread = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "request timed out");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("HTTP listener should accept: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        let request = read_http_headers(&mut stream);
        stream
            .write_all(response.as_bytes())
            .expect("write response");
        request
    });
    (address, thread)
}

fn read_http_headers(stream: &mut impl Read) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let bytes_read = stream.read(&mut chunk).expect("HTTP headers should read");
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

fn has_header(request: &str, expected_name: &str) -> bool {
    request.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(name, _)| name.eq_ignore_ascii_case(expected_name))
    })
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
