use codex_metrics::HistogramBuckets;
use codex_metrics::MetricsBatch;
use codex_metrics::MetricsClient;
use codex_metrics::MetricsConfig;
use codex_metrics::MetricsError;
use codex_metrics::Result;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
struct CapturedRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug)]
struct ParsedEnvelope {
    header: Value,
    item_header: Value,
    payload: String,
}

#[derive(Debug)]
struct ParsedStatsdLine {
    name: String,
    value: i64,
    kind: String,
    tags: BTreeMap<String, String>,
}

fn spawn_server(status: u16) -> (String, thread::JoinHandle<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    let dsn = format!("http://public:@{addr}/123");

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept connection");
        let request = read_http_request(&mut stream);
        let reason = match status {
            200 => "OK",
            500 => "Internal Server Error",
            _ => "OK",
        };
        let response =
            format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        stream
            .write_all(response.as_bytes())
            .expect("write response");
        request
    });

    (dsn, handle)
}

fn read_http_request(stream: &mut TcpStream) -> CapturedRequest {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let mut header_end = None;
    while header_end.is_none() {
        let read = stream.read(&mut chunk).expect("read request");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        header_end = find_header_end(&buffer);
    }
    let header_end = header_end.expect("request headers");
    let headers_bytes = &buffer[..header_end];
    let headers_str = std::str::from_utf8(headers_bytes).expect("headers utf-8");
    let mut lines = headers_str.split("\r\n");
    let request_line = lines.next().expect("request line");
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().expect("method").to_string();
    let path = request_parts.next().expect("path").to_string();

    let mut headers = BTreeMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = buffer[header_end..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk).expect("read body");
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }

    CapturedRequest {
        method,
        path,
        headers,
        body,
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

fn parse_envelope(body: &[u8]) -> ParsedEnvelope {
    let mut parts = body.splitn(3, |byte| *byte == b'\n');
    let header_line = parts.next().expect("envelope header");
    let item_header_line = parts.next().expect("item header");
    let payload = parts.next().unwrap_or(&[]);

    let header = serde_json::from_slice(header_line).expect("parse envelope header");
    let item_header = serde_json::from_slice(item_header_line).expect("parse item header");
    let payload = std::str::from_utf8(payload)
        .expect("payload utf-8")
        .trim_end_matches('\n')
        .to_string();

    ParsedEnvelope {
        header,
        item_header,
        payload,
    }
}

fn parse_statsd_line(line: &str) -> ParsedStatsdLine {
    let (metric, tags_part) = line
        .split_once("|#")
        .map(|(metric, tags)| (metric, Some(tags)))
        .unwrap_or((line, None));
    let (name_value, kind) = metric.split_once('|').expect("metric kind");
    let (name, value) = name_value.split_once(':').expect("metric value");
    let value = value.parse::<i64>().expect("metric value parse");

    let mut tags = BTreeMap::new();
    if let Some(tags_part) = tags_part
        && !tags_part.is_empty()
    {
        for tag in tags_part.split(',') {
            let (key, value) = tag.split_once(':').expect("tag");
            tags.insert(key.to_string(), value.to_string());
        }
    }

    ParsedStatsdLine {
        name: name.to_string(),
        value,
        kind: kind.to_string(),
        tags,
    }
}

#[test]
fn send_builds_payload_with_tags_and_histograms() -> Result<()> {
    let (dsn, handle) = spawn_server(200);
    let metrics = MetricsClient::new(
        MetricsConfig::new(dsn.clone())
            .with_tag("service", "codex-cli")?
            .with_tag("env", "prod")?,
    )?;
    let buckets = HistogramBuckets::from_values(&[25, 50, 100])?;

    let mut batch = metrics.batch();
    batch.counter("codex.turns", 1, &[("model", "gpt-5.1"), ("env", "dev")])?;
    batch.histogram("codex.tool_latency", 25, &buckets, &[("tool", "shell")])?;
    metrics.send(batch)?;

    let captured = handle.join().expect("server thread");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/api/123/envelope/");
    assert_eq!(
        captured.headers.get("content-type").map(String::as_str),
        Some("application/x-sentry-envelope")
    );

    let envelope = parse_envelope(&captured.body);
    assert_eq!(envelope.header["dsn"].as_str(), Some(dsn.as_str()));
    assert_eq!(envelope.item_header["type"], "statsd");
    assert_eq!(envelope.item_header["content_type"], "text/plain");
    assert_eq!(
        envelope.item_header["length"].as_u64(),
        Some(envelope.payload.len() as u64)
    );

    let lines: Vec<&str> = envelope.payload.split('\n').collect();
    assert_eq!(lines.len(), 4);

    let line = parse_statsd_line(lines[0]);
    assert_eq!(line.name, "codex.turns");
    assert_eq!(line.value, 1);
    assert_eq!(line.kind, "c");
    assert_eq!(
        line.tags.get("service").map(String::as_str),
        Some("codex-cli")
    );
    assert_eq!(line.tags.get("env").map(String::as_str), Some("dev"));
    assert_eq!(line.tags.get("model").map(String::as_str), Some("gpt-5.1"));

    for (line, expected_le) in lines.iter().skip(1).zip(["25", "50", "100"]) {
        let line = parse_statsd_line(line);
        assert_eq!(line.name, "codex.tool_latency");
        assert_eq!(line.value, 1);
        assert_eq!(line.kind, "c");
        assert_eq!(
            line.tags.get("service").map(String::as_str),
            Some("codex-cli")
        );
        assert_eq!(line.tags.get("env").map(String::as_str), Some("prod"));
        assert_eq!(line.tags.get("tool").map(String::as_str), Some("shell"));
        assert_eq!(line.tags.get("le").map(String::as_str), Some(expected_le));
    }

    Ok(())
}

#[test]
fn send_uses_inf_bucket_for_values_over_max() -> Result<()> {
    let (dsn, handle) = spawn_server(200);
    let metrics = MetricsClient::new(MetricsConfig::new(dsn))?;
    let buckets = HistogramBuckets::from_values(&[10, 20])?;

    let mut batch = metrics.batch();
    batch.histogram("codex.tool_latency", 99, &buckets, &[("tool", "shell")])?;
    metrics.send(batch)?;

    let captured = handle.join().expect("server thread");
    let envelope = parse_envelope(&captured.body);
    let lines: Vec<&str> = envelope.payload.split('\n').collect();
    assert_eq!(lines.len(), 1);
    let line = parse_statsd_line(lines[0]);
    assert_eq!(line.tags.get("le").map(String::as_str), Some("inf"));

    Ok(())
}

#[test]
fn send_reports_non_success_status() -> Result<()> {
    let (dsn, handle) = spawn_server(500);
    let metrics = MetricsClient::new(MetricsConfig::new(dsn))?;

    let mut batch = metrics.batch();
    batch.counter("codex.turns", 1, &[])?;
    let err = metrics.send(batch).unwrap_err();
    assert!(matches!(
        err,
        MetricsError::SentryUploadFailed { status, .. } if status.as_u16() == 500
    ));

    let _ = handle.join().expect("server thread");
    Ok(())
}

#[test]
fn invalid_dsn_reports_error() -> Result<()> {
    let err = MetricsClient::new(MetricsConfig::new("not a dsn")).unwrap_err();
    assert!(matches!(err, MetricsError::InvalidDsn { .. }));
    Ok(())
}

#[test]
fn send_is_noop_when_batch_empty() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    listener.set_nonblocking(true).expect("set nonblocking");
    let addr = listener.local_addr().expect("local addr");
    let dsn = format!("http://public:@{addr}/123");
    let metrics = MetricsClient::new(MetricsConfig::new(dsn))?;

    metrics.send(metrics.batch())?;

    let mut saw_connection = false;
    for _ in 0..10 {
        match listener.accept() {
            Ok(_) => {
                saw_connection = true;
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("unexpected accept error: {err}"),
        }
    }
    assert!(!saw_connection, "expected no request for empty batch");
    Ok(())
}

#[test]
fn invalid_tag_component_is_rejected() -> Result<()> {
    let err = MetricsConfig::default()
        .with_tag("bad key", "value")
        .unwrap_err();
    assert!(matches!(
        err,
        MetricsError::InvalidTagComponent { label, value }
            if label == "tag key" && value == "bad key"
    ));
    Ok(())
}

#[test]
fn counter_rejects_invalid_metric_name() -> Result<()> {
    let mut batch = MetricsBatch::new();
    let err = batch.counter("bad name", 1, &[]).unwrap_err();
    assert!(matches!(
        err,
        MetricsError::InvalidMetricName { name } if name == "bad name"
    ));
    Ok(())
}

#[test]
fn empty_buckets_are_rejected() {
    let err = HistogramBuckets::from_values(&[]).unwrap_err();
    assert!(matches!(err, MetricsError::EmptyBuckets));
}

#[test]
fn range_overflow_is_reported() {
    let err = HistogramBuckets::from_range(i64::MAX - 1, i64::MAX, 2).unwrap_err();
    assert!(matches!(err, MetricsError::BucketRangeOverflow { .. }));
}
