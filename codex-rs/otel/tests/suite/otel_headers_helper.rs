//! Collector-level coverage for dynamic headers on every HTTP signal and encoding.

use super::*;
use codex_otel::OtelHeadersHelper;
use pretty_assertions::assert_eq;

#[test]
fn http_headers_helper_authenticates_all_signals() {
    for protocol in [OtelHttpProtocol::Json, OtelHttpProtocol::Binary] {
        check_all_signals(protocol.clone(), /*managed_logs*/ false);
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("create test runtime")
            .block_on(async {
                check_all_signals(protocol, /*managed_logs*/ true)
            });
    }
}

fn check_all_signals(protocol: OtelHttpProtocol, managed_logs: bool) {
    let dir = tempfile::tempdir().expect("create helper directory");
    std::fs::write(
        dir.path().join("headers.json"),
        r#"{"Authorization":"Bearer refreshed","X-Helper":"present"}"#,
    )
    .expect("write helper headers");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind collector");
    let address = listener.local_addr().expect("collector address");
    listener
        .set_nonblocking(true)
        .expect("nonblocking collector");
    let collector = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut received = BTreeMap::new();
        while Instant::now() < deadline && received.len() < 3 {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let (path, headers, _) = read_http_request(&mut stream).expect("read export");
                    write_http_response(&mut stream, "200 OK").expect("acknowledge export");
                    received.insert(path, headers);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("collector failed: {error}"),
            }
        }
        received
    });
    let exporter = |signal| OtelExporter::OtlpHttp {
        endpoint: format!("http://{address}/v1/{signal}"),
        headers: HashMap::from([
            ("authorization".to_string(), "Bearer stale".to_string()),
            ("X-Static".to_string(), "retained".to_string()),
        ]),
        protocol: protocol.clone(),
        tls: None,
    };
    let command = if cfg!(windows) {
        "type headers.json"
    } else {
        "cat headers.json"
    };
    let helper = OtelHeadersHelper {
        command: format!("echo invocation >> calls && {command}"),
        refresh_interval: Duration::from_secs(3600),
        apply_to_logs: !managed_logs,
        apply_to_traces: true,
        apply_to_metrics: true,
    };
    let provider = OtelProvider::try_new(&OtelSettings {
        headers_helper: Some(helper),
        http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        environment: "test".to_string(),
        service_name: "otel-helper-test".to_string(),
        service_version: "test".to_string(),
        codex_home: dir.path().to_path_buf(),
        exporter: exporter("logs"),
        trace_exporter: exporter("traces"),
        metrics_exporter: exporter("metrics"),
        runtime_metrics: false,
        span_attributes: HashMap::new(),
        tracestate: BTreeMap::new(),
    })
    .expect("initialize exporters")
    .expect("exporters are enabled");
    let subscriber = tracing_subscriber::registry()
        .with(provider.logger_layer().expect("logs enabled"))
        .with(provider.tracing_layer().expect("traces enabled"));
    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::info_span!("helper-test");
        let _entered = span.enter();
        tracing::event!(target: "codex_otel.log_only", tracing::Level::INFO, event.name = "codex.test.helper", "test");
    });
    provider
        .metrics()
        .expect("metrics enabled")
        .counter("codex.test.helper", /*inc*/ 1, &[])
        .expect("record counter");
    provider.shutdown();
    let received = collector.join().expect("collector completed");
    assert_eq!(
        received.keys().map(String::as_str).collect::<Vec<_>>(),
        ["/v1/logs", "/v1/metrics", "/v1/traces"]
    );
    for (path, headers) in &received {
        if managed_logs && path == "/v1/logs" {
            assert_eq!(headers["authorization"], "Bearer stale");
            assert!(!headers.contains_key("x-helper"));
        } else {
            assert_eq!(headers["authorization"], "Bearer refreshed");
            assert_eq!(headers["x-helper"], "present");
        }
        assert_eq!(headers["x-static"], "retained");
    }
    assert_eq!(
        std::fs::read_to_string(dir.path().join("calls"))
            .expect("read helper invocation count")
            .lines()
            .count(),
        1
    );
}
