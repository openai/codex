use codex_otel::config::OtelExporter;
use codex_otel::config::OtelHttpProtocol;
use codex_otel::metrics::MetricsClient;
use codex_otel::metrics::MetricsConfig;
use codex_otel::metrics::Result;
use std::collections::HashMap;
use std::io::Read as _;
use std::io::Write as _;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

struct CapturedRequest {
    path: String,
    content_type: Option<String>,
    body: Vec<u8>,
}

fn read_http_request(
    stream: &mut TcpStream,
) -> std::io::Result<(String, HashMap<String, String>, Vec<u8>)> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let deadline = Instant::now() + Duration::from_secs(2);

    let mut read_next = |buf: &mut [u8]| -> std::io::Result<usize> {
        loop {
            match stream.read(buf) {
                Ok(n) => return Ok(n),
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::Interrupted =>
                {
                    if Instant::now() >= deadline {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "timed out waiting for request data",
                        ));
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Err(err) => return Err(err),
            }
        }
    };

    let mut buf = Vec::new();
    let mut scratch = [0u8; 8192];
    let header_end = loop {
        let n = read_next(&mut scratch)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "EOF before headers",
            ));
        }
        buf.extend_from_slice(&scratch[..n]);
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break end;
        }
        if buf.len() > 1024 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "headers too large",
            ));
        }
    };

    let headers_bytes = &buf[..header_end];
    let mut body_bytes = buf[header_end + 4..].to_vec();

    let headers_str = std::str::from_utf8(headers_bytes).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("headers not utf-8: {err}"),
        )
    })?;
    let mut lines = headers_str.split("\r\n");
    let start = lines.next().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing request line")
    })?;
    let mut parts = start.split_whitespace();
    let _method = parts.next().unwrap_or_default();
    let path = parts
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing path"))?
        .to_string();

    let mut headers = HashMap::new();
    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
    }

    if let Some(len) = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
    {
        while body_bytes.len() < len {
            let n = read_next(&mut scratch)?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "EOF before body complete",
                ));
            }
            body_bytes.extend_from_slice(&scratch[..n]);
            if body_bytes.len() > len + 1024 * 1024 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "body too large",
                ));
            }
        }
        body_bytes.truncate(len);
    }

    Ok((path, headers, body_bytes))
}

fn write_http_response(stream: &mut TcpStream, status: &str) -> std::io::Result<()> {
    let response = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

#[test]
fn otlp_http_exporter_sends_metrics_to_collector() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    listener.set_nonblocking(true).expect("set_nonblocking");

    let (tx, rx) = mpsc::channel::<Vec<CapturedRequest>>();
    let server = thread::spawn(move || {
        let mut captured = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(3);

        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let result = read_http_request(&mut stream);
                    let _ = write_http_response(&mut stream, "202 Accepted");
                    if let Ok((path, headers, body)) = result {
                        captured.push(CapturedRequest {
                            path,
                            content_type: headers.get("content-type").cloned(),
                            body,
                        });
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }

        let _ = tx.send(captured);
    });

    let metrics = MetricsClient::new(MetricsConfig::otlp(
        "test",
        "codex-cli",
        env!("CARGO_PKG_VERSION"),
        OtelExporter::OtlpHttp {
            endpoint: format!("http://{addr}/v1/metrics"),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        },
    ))?;

    metrics.counter("codex.turns", 1, &[("source", "test")])?;
    metrics.shutdown()?;

    server.join().expect("server join");
    let captured = rx.recv_timeout(Duration::from_secs(1)).expect("captured");

    let request = captured
        .iter()
        .find(|req| req.path == "/v1/metrics")
        .unwrap_or_else(|| {
            let paths = captured
                .iter()
                .map(|req| req.path.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            panic!(
                "missing /v1/metrics request; got {}: {paths}",
                captured.len()
            );
        });
    let content_type = request
        .content_type
        .as_deref()
        .unwrap_or("<missing content-type>");
    assert!(
        content_type.starts_with("application/json"),
        "unexpected content-type: {content_type}"
    );

    let body = String::from_utf8_lossy(&request.body);
    assert!(
        body.contains("codex.turns"),
        "expected metric name not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );

    Ok(())
}
