use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::OtelExporter;
use codex_otel::OtelHttpProtocol;
use codex_otel::OtelProvider;
use codex_otel::OtelSettings;
use codex_otel::Result;
use codex_otel::current_span_w3c_trace_context;
use codex_otel::set_parent_from_w3c_trace_context;
use codex_protocol::protocol::W3cTraceContext;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::Read as _;
use std::io::Write as _;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tracing_subscriber::layer::SubscriberExt;

static TRACE_CONTEXT_CONFIG_LOCK: Mutex<()> = Mutex::new(());

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

    metrics.counter("codex.turns", /*inc*/ 1, &[("source", "test")])?;
    metrics.counter("codex.api_request", /*inc*/ 1, &[("status", "200")])?;
    metrics.record_duration(
        "codex.api_request.duration_ms",
        Duration::from_millis(100),
        &[("status", "200")],
    )?;
    metrics.counter("codex.conversation.turn.count", /*inc*/ 1, &[])?;
    metrics.record_duration(
        "codex.responses_api_engine_iapi_ttft.duration_ms",
        Duration::from_millis(100),
        &[],
    )?;
    metrics.record_duration(
        "codex.responses_api_engine_service_tbt.duration_ms",
        Duration::from_millis(100),
        &[],
    )?;
    metrics.record_duration(
        "codex.responses_api_engine_service_ttft.duration_ms",
        Duration::from_millis(100),
        &[],
    )?;
    metrics.counter("codex.tool.call", /*inc*/ 1, &[("tool", "test")])?;
    metrics.record_duration(
        "codex.tool.call.duration_ms",
        Duration::from_millis(42),
        &[("tool", "test")],
    )?;
    metrics.histogram(
        "codex.turn.token_usage",
        /*value*/ 100,
        &[("token_type", "total")],
    )?;
    metrics.gauge_with_description(
        "codex.active",
        "Number of active Codex operations.",
        /*value*/ 1,
        &[("component", "test")],
    )?;
    metrics.shutdown()?;

    server.join().expect("server join");
    let captured = rx.recv_timeout(Duration::from_secs(1)).expect("captured");

    let request = captured
        .iter()
        .find(|req| req.path == "/v1/metrics")
        .expect("/v1/metrics request should be captured");
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
    assert!(
        body.contains("codex.active"),
        "expected gauge not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("\"codex.api_request\""),
        "expected API-request counter not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("\"codex.api_request.duration_ms\""),
        "expected API-request duration not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("\"codex.conversation.turn.count\""),
        "expected conversation turn count not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("\"codex.responses_api_engine_iapi_ttft.duration_ms\""),
        "expected engine IAPI TTFT duration not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("\"codex.responses_api_engine_service_tbt.duration_ms\""),
        "expected engine service TBT duration not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("\"codex.responses_api_engine_service_ttft.duration_ms\""),
        "expected engine service TTFT duration not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("\"codex.tool.call\""),
        "expected tool-call counter not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("\"codex.turn.token_usage\""),
        "expected turn-token histogram not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("\"codex.tool.call.duration_ms\""),
        "expected tool-call duration not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("component") && body.contains("test"),
        "expected gauge tag not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );

    Ok(())
}

#[test]
fn otlp_http_exporter_sends_logs_to_collector()
-> std::result::Result<(), Box<dyn std::error::Error>> {
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

    let otel = OtelProvider::from(&OtelSettings {
        environment: "test".to_string(),
        service_name: "codex-cli".to_string(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        codex_home: PathBuf::from("."),
        exporter: OtelExporter::OtlpHttp {
            endpoint: format!("http://{addr}/v1/logs"),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        },
        trace_exporter: OtelExporter::None,
        metrics_exporter: OtelExporter::None,
        runtime_metrics: false,
        span_attributes: BTreeMap::new(),
        tracestate: BTreeMap::new(),
    })?
    .expect("otel provider");
    let logger_layer = otel.logger_layer().expect("logger layer");
    let subscriber = tracing_subscriber::registry().with(logger_layer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::callsite::rebuild_interest_cache();
        tracing::event!(
            target: "codex_otel.log_only",
            tracing::Level::INFO,
            event.name = "codex.test.log_exported",
            "test OTEL log export"
        );
    });
    otel.shutdown();

    server.join().expect("server join");
    let captured = rx.recv_timeout(Duration::from_secs(1)).expect("captured");

    let request = captured
        .iter()
        .find(|req| req.path == "/v1/logs")
        .expect("/v1/logs request should be captured");
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
        body.contains("codex.test.log_exported"),
        "expected exported log event not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    Ok(())
}

#[test]
fn otel_provider_rejects_header_unsafe_configured_tracestate() {
    let result = OtelProvider::from(&OtelSettings {
        environment: "test".to_string(),
        service_name: "codex-cli".to_string(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        codex_home: PathBuf::from("."),
        exporter: OtelExporter::None,
        trace_exporter: OtelExporter::OtlpHttp {
            endpoint: "http://127.0.0.1:1/v1/traces".to_string(),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        },
        metrics_exporter: OtelExporter::None,
        runtime_metrics: false,
        span_attributes: BTreeMap::new(),
        tracestate: BTreeMap::from([(
            "example".to_string(),
            BTreeMap::from([("alpha".to_string(), "one\ntwo".to_string())]),
        )]),
    });

    let err = result
        .err()
        .expect("header-unsafe configured tracestate should be rejected");
    assert!(err.to_string().contains("configured tracestate value"));
}

#[test]
fn otlp_http_exporter_sends_traces_to_collector()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let _trace_context_config_guard = TRACE_CONTEXT_CONFIG_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    let otel = OtelProvider::from(&OtelSettings {
        environment: "test".to_string(),
        service_name: "codex-cli".to_string(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        codex_home: PathBuf::from("."),
        exporter: OtelExporter::None,
        trace_exporter: OtelExporter::OtlpHttp {
            endpoint: format!("http://{addr}/v1/traces"),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        },
        metrics_exporter: OtelExporter::None,
        runtime_metrics: false,
        span_attributes: BTreeMap::from([(
            "test.configured_attribute".to_string(),
            "configured-value".to_string(),
        )]),
        tracestate: BTreeMap::from([(
            "example".to_string(),
            BTreeMap::from([
                ("alpha".to_string(), "one".to_string()),
                ("beta".to_string(), "two".to_string()),
            ]),
        )]),
    })?
    .expect("otel provider");
    let tracing_layer = otel.tracing_layer().expect("tracing layer");
    let subscriber = tracing_subscriber::registry().with(tracing_layer);

    let propagated_trace = tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!(
            "trace-loopback",
            otel.name = "trace-loopback",
            otel.kind = "server",
            rpc.system = "jsonrpc",
            rpc.method = "trace-loopback",
        );
        assert!(set_parent_from_w3c_trace_context(
            &span,
            &W3cTraceContext {
                traceparent: Some(
                    "00-00000000000000000000000000000001-0000000000000002-01".to_string(),
                ),
                tracestate: Some("example=alpha:zero;keep:yes,other=value".to_string()),
            },
        ));
        let _guard = span.enter();
        let propagated_trace =
            current_span_w3c_trace_context().expect("current span should have trace context");
        tracing::event!(
            target: "codex_otel.trace_safe",
            tracing::Level::INFO,
            event.name = "codex.test.trace_event",
            "test OTEL trace event"
        );
        tracing::info!("trace loopback event");
        propagated_trace
    });
    otel.shutdown();

    assert_eq!(
        propagated_trace.tracestate.as_deref(),
        Some("example=alpha:one;keep:yes;beta:two,other=value")
    );

    server.join().expect("server join");
    let captured = rx.recv_timeout(Duration::from_secs(1)).expect("captured");

    let request = captured
        .iter()
        .find(|req| req.path == "/v1/traces")
        .expect("/v1/traces request should be captured");
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
        body.contains("trace-loopback"),
        "expected span name not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("codex-cli"),
        "expected service name not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("test.configured_attribute") && body.contains("configured-value"),
        "expected configured span attribute not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("codex.test.trace_event"),
        "expected trace event not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn otlp_http_exporter_sends_traces_to_collector_with_bounded_shutdown_in_tokio_runtime()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let _trace_context_config_guard = TRACE_CONTEXT_CONFIG_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    let otel = OtelProvider::from(&OtelSettings {
        environment: "test".to_string(),
        service_name: "codex-cli".to_string(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        codex_home: PathBuf::from("."),
        exporter: OtelExporter::None,
        trace_exporter: OtelExporter::OtlpHttp {
            endpoint: format!("http://{addr}/v1/traces"),
            headers: HashMap::new(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        },
        metrics_exporter: OtelExporter::None,
        runtime_metrics: false,
        span_attributes: BTreeMap::new(),
        tracestate: BTreeMap::new(),
    })?
    .expect("otel provider");
    let tracing_layer = otel.tracing_layer().expect("tracing layer");
    let subscriber = tracing_subscriber::registry().with(tracing_layer);

    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!(
            "trace-loopback-tokio",
            otel.name = "trace-loopback-tokio",
            otel.kind = "server",
            rpc.system = "jsonrpc",
            rpc.method = "trace-loopback-tokio",
        );
        let _guard = span.enter();
        tracing::info!("trace loopback event from tokio runtime");
    });
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(otel.shutdown_with_timeout(Duration::from_secs(/*secs*/ 2)))
    })?;

    server.join().expect("server join");
    let captured = rx.recv_timeout(Duration::from_secs(1)).expect("captured");

    let request = captured
        .iter()
        .find(|req| req.path == "/v1/traces")
        .expect("/v1/traces request should be captured");
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
        body.contains("trace-loopback-tokio"),
        "expected span name not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("codex-cli"),
        "expected service name not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );

    Ok(())
}

#[test]
fn otlp_http_exporter_times_out_when_collector_stalls_during_bounded_shutdown() {
    let _trace_context_config_guard = TRACE_CONTEXT_CONFIG_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let (request_started_tx, request_started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept trace request");
        let (path, _, _) = read_http_request(&mut stream).expect("read trace request");
        request_started_tx.send(path).expect("request started");
        release_rx
            .recv_timeout(Duration::from_secs(/*secs*/ 2))
            .expect("collector released");
        write_http_response(&mut stream, "202 Accepted").expect("write trace response");
    });

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    let (result, elapsed) = runtime.block_on(async move {
        let otel = OtelProvider::from(&OtelSettings {
            environment: "test".to_string(),
            service_name: "codex-cli".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            codex_home: PathBuf::from("."),
            exporter: OtelExporter::None,
            trace_exporter: OtelExporter::OtlpHttp {
                endpoint: format!("http://{addr}/v1/traces"),
                headers: HashMap::new(),
                protocol: OtelHttpProtocol::Json,
                tls: None,
            },
            metrics_exporter: OtelExporter::None,
            runtime_metrics: false,
            span_attributes: BTreeMap::new(),
            tracestate: BTreeMap::new(),
        })
        .expect("build otel provider")
        .expect("otel provider");
        let tracing_layer = otel.tracing_layer().expect("tracing layer");
        let subscriber = tracing_subscriber::registry().with(tracing_layer);

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("trace-loopback-stalled");
            let _guard = span.enter();
            tracing::info!("trace loopback event for stalled collector");
        });

        let started = Instant::now();
        let result = otel
            .shutdown_with_timeout(Duration::from_millis(/*millis*/ 50))
            .await;
        (result, started.elapsed())
    });

    let path = request_started_rx
        .recv_timeout(Duration::from_secs(/*secs*/ 1))
        .expect("trace request reached collector");
    release_tx.send(()).expect("release collector");
    server.join().expect("server join");
    runtime.shutdown_timeout(Duration::from_secs(/*secs*/ 1));

    assert_eq!(path, "/v1/traces");
    assert_eq!(
        result.as_ref().map_err(std::io::Error::kind),
        Err(std::io::ErrorKind::TimedOut)
    );
    assert!(
        elapsed < Duration::from_secs(/*secs*/ 1),
        "bounded shutdown blocked for {elapsed:?}"
    );
}

#[test]
fn otlp_http_exporter_sends_traces_to_collector_in_current_thread_tokio_runtime()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let _trace_context_config_guard = TRACE_CONTEXT_CONFIG_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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

    let (runtime_result_tx, runtime_result_rx) = mpsc::channel::<std::result::Result<(), String>>();
    let runtime_thread = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime");

        let result = runtime.block_on(async move {
            let otel = OtelProvider::from(&OtelSettings {
                environment: "test".to_string(),
                service_name: "codex-cli".to_string(),
                service_version: env!("CARGO_PKG_VERSION").to_string(),
                codex_home: PathBuf::from("."),
                exporter: OtelExporter::None,
                trace_exporter: OtelExporter::OtlpHttp {
                    endpoint: format!("http://{addr}/v1/traces"),
                    headers: HashMap::new(),
                    protocol: OtelHttpProtocol::Json,
                    tls: None,
                },
                metrics_exporter: OtelExporter::None,
                runtime_metrics: false,
                span_attributes: BTreeMap::new(),
                tracestate: BTreeMap::new(),
            })
            .map_err(|err| err.to_string())?
            .expect("otel provider");
            let tracing_layer = otel.tracing_layer().expect("tracing layer");
            let subscriber = tracing_subscriber::registry().with(tracing_layer);

            tracing::subscriber::with_default(subscriber, || {
                let span = tracing::info_span!(
                    "trace-loopback-current-thread",
                    otel.name = "trace-loopback-current-thread",
                    otel.kind = "server",
                    rpc.system = "jsonrpc",
                    rpc.method = "trace-loopback-current-thread",
                );
                let _guard = span.enter();
                tracing::info!("trace loopback event from current-thread tokio runtime");
            });
            otel.shutdown();
            Ok::<(), String>(())
        });
        let _ = runtime_result_tx.send(result);
    });

    runtime_result_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("current-thread runtime should complete")
        .map_err(std::io::Error::other)?;
    runtime_thread.join().expect("runtime thread");

    server.join().expect("server join");
    let captured = rx.recv_timeout(Duration::from_secs(1)).expect("captured");

    let request = captured
        .iter()
        .find(|req| req.path == "/v1/traces")
        .expect("/v1/traces request should be captured");
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
        body.contains("trace-loopback-current-thread"),
        "expected span name not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );
    assert!(
        body.contains("codex-cli"),
        "expected service name not found; body prefix: {}",
        &body.chars().take(2000).collect::<String>()
    );

    Ok(())
}
