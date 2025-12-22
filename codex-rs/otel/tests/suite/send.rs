use crate::harness::parse_envelope;
use crate::harness::parse_statsd_line;
use crate::harness::spawn_server;
use codex_metrics::HistogramBuckets;
use codex_metrics::MetricsClient;
use codex_metrics::MetricsConfig;
use codex_metrics::MetricsError;
use codex_metrics::Result;
use pretty_assertions::assert_eq;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

// Ensures counters/histograms render with default + per-call tags.
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
    metrics.shutdown()?;

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
    assert_eq!(lines.len(), 5);

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

    for (line, expected_le) in lines.iter().skip(1).zip(["25", "50", "100", "inf"]) {
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

// Ensures defaults merge per line and overrides take precedence.
#[test]
fn send_merges_default_tags_per_line() -> Result<()> {
    let (dsn, handle) = spawn_server(200);
    let metrics = MetricsClient::new(
        MetricsConfig::new(dsn.clone())
            .with_tag("service", "codex-cli")?
            .with_tag("env", "prod")?
            .with_tag("region", "us")?,
    )?;

    let mut batch = metrics.batch();
    batch.counter("codex.alpha", 1, &[("env", "dev"), ("component", "alpha")])?;
    batch.counter(
        "codex.beta",
        2,
        &[("service", "worker"), ("component", "beta")],
    )?;
    metrics.send(batch)?;
    metrics.shutdown()?;

    let captured = handle.join().expect("server thread");
    let envelope = parse_envelope(&captured.body);
    let lines: Vec<&str> = envelope.payload.split('\n').collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0],
        "codex.alpha:1|c|#component:alpha,env:dev,region:us,service:codex-cli"
    );
    assert_eq!(
        lines[1],
        "codex.beta:2|c|#component:beta,env:prod,region:us,service:worker"
    );

    Ok(())
}

// Verifies values above the max bucket use the inf tag.
#[test]
fn send_uses_inf_bucket_for_values_over_max() -> Result<()> {
    let (dsn, handle) = spawn_server(200);
    let metrics = MetricsClient::new(MetricsConfig::new(dsn))?;
    let buckets = HistogramBuckets::from_values(&[10, 20])?;

    let mut batch = metrics.batch();
    batch.histogram("codex.tool_latency", 99, &buckets, &[("tool", "shell")])?;
    metrics.send(batch)?;
    metrics.shutdown()?;

    let captured = handle.join().expect("server thread");
    let envelope = parse_envelope(&captured.body);
    let lines: Vec<&str> = envelope.payload.split('\n').collect();
    assert_eq!(lines.len(), 1);
    let line = parse_statsd_line(lines[0]);
    assert_eq!(line.tags.get("le").map(String::as_str), Some("inf"));

    Ok(())
}

// Verifies enqueued batches are delivered by the background worker.
#[test]
fn client_sends_enqueued_batch() -> Result<()> {
    let (dsn, handle) = spawn_server(200);
    let metrics = MetricsClient::new(MetricsConfig::new(dsn))?;

    let mut batch = metrics.batch();
    batch.counter("codex.turns", 1, &[("model", "gpt-5.1")])?;
    metrics.send(batch)?;
    metrics.shutdown()?;

    let captured = handle.join().expect("server thread");
    let envelope = parse_envelope(&captured.body);
    let lines: Vec<&str> = envelope.payload.split('\n').collect();
    assert_eq!(lines.len(), 1);

    let line = parse_statsd_line(lines[0]);
    assert_eq!(line.name, "codex.turns");
    assert_eq!(line.value, 1);
    assert_eq!(line.kind, "c");
    assert_eq!(line.tags.get("model").map(String::as_str), Some("gpt-5.1"));

    Ok(())
}

// Ensures a non-success response panics in debug builds via error_or_panic.
#[test]
fn send_panics_on_non_success_status_in_debug() -> Result<()> {
    let (dsn, handle) = spawn_server(500);
    let metrics = MetricsClient::new(MetricsConfig::new(dsn))?;

    let mut batch = metrics.batch();
    batch.counter("codex.turns", 1, &[])?;
    metrics.send(batch)?;
    let err = metrics.shutdown().unwrap_err();
    assert!(matches!(err, MetricsError::WorkerPanicked));

    let captured = handle.join().expect("server thread");
    assert_eq!(captured.method, "POST");
    Ok(())
}

// Ensures empty batches do not trigger any HTTP request.
#[test]
fn client_core_skips_empty_batch() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    listener.set_nonblocking(true).expect("set nonblocking");
    let addr = listener.local_addr().expect("local addr");
    let dsn = format!("http://public:@{addr}/123");
    let metrics = MetricsClient::new(MetricsConfig::new(dsn))?;

    metrics.send(metrics.batch())?;
    metrics.shutdown()?;

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
