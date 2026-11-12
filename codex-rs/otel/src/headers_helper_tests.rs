#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;

fn read_command() -> &'static str {
    if cfg!(windows) {
        "type headers.json"
    } else {
        "cat headers.json"
    }
}

#[tokio::test]
async fn restricted_policy_skips_command_until_a_refresh_is_permitted() {
    use codex_http_client::DestinationPolicy;
    use codex_http_client::NetworkPolicyController;

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("headers.json"), "{}").unwrap();
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    let revision = policy.revision();
    controller.publish(
        revision,
        DestinationPolicy::Restricted {
            allowed_hosts: Default::default(),
        },
    );
    let helper = HeadersHelper::start(
        &OtelHeadersHelper {
            command: format!("echo invocation >> calls && {}", read_command()),
            refresh_interval: Duration::from_millis(20),
            apply_to_logs: true,
            apply_to_traces: true,
            apply_to_metrics: true,
        },
        dir.path(),
        policy,
    )
    .unwrap();
    assert!(helper.headers().await.is_err());
    assert!(!dir.path().join("calls").exists());
    let mut updates = helper.headers.clone();
    controller.publish(revision, DestinationPolicy::Unrestricted);
    tokio::time::timeout(
        Duration::from_secs(5),
        updates.wait_for(|v| matches!(v, Some(Ok(_)))),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(dir.path().join("calls").exists());
    helper.shutdown();
}

#[tokio::test]
async fn concurrent_exports_share_one_helper_execution() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("headers.json"),
        r#"{"Authorization":"first"}"#,
    )
    .unwrap();
    let helper = HeadersHelper::start(
        &OtelHeadersHelper {
            command: format!("echo invocation >> calls && {}", read_command()),
            refresh_interval: Duration::from_secs(3600),
            apply_to_logs: true,
            apply_to_traces: true,
            apply_to_metrics: true,
        },
        dir.path(),
        NetworkPolicy::unmanaged(),
    )
    .unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..20 {
        let helper = helper.clone();
        tasks.spawn(async move { helper.headers().await.unwrap() });
    }
    while let Some(result) = tasks.join_next().await {
        assert_eq!(result.unwrap()["authorization"], "first");
    }
    assert_eq!(
        std::fs::read_to_string(dir.path().join("calls"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    helper.shutdown();
}

#[tokio::test]
async fn periodic_refresh_replaces_credentials_and_recovers_after_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("headers.json");
    std::fs::write(&path, r#"{"Authorization":"first","X-Old":"old"}"#).unwrap();
    let helper = HeadersHelper::start(
        &OtelHeadersHelper {
            command: read_command().to_string(),
            refresh_interval: Duration::from_millis(20),
            apply_to_logs: true,
            apply_to_traces: true,
            apply_to_metrics: true,
        },
        dir.path(),
        NetworkPolicy::unmanaged(),
    )
    .unwrap();
    assert_eq!(helper.headers().await.unwrap()["authorization"], "first");
    let mut updates = helper.headers.clone();
    std::fs::write(&path, "invalid output containing a secret").unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        updates.wait_for(|value| matches!(value, Some(Err(_)))),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(helper.headers().await.is_err());
    std::fs::write(&path, r#"{"Authorization":"second"}"#).unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        updates.wait_for(
            |value| matches!(value, Some(Ok(headers)) if headers["authorization"] == "second"),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let headers = helper.headers().await.unwrap();
    assert_eq!(headers.len(), 1);
    assert_eq!(headers["authorization"], "second");
    helper.shutdown();
}

#[tokio::test]
async fn helper_failure_is_an_export_error_and_output_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run_helper("exit 1", dir.path()).await.is_err());
    std::fs::write(dir.path().join("headers.json"), vec![b'x'; 64 * 1024 + 1]).unwrap();
    assert_eq!(
        run_helper(read_command(), dir.path()).await,
        Err("headers helper output exceeds 64 KiB".to_string())
    );
}

#[tokio::test]
async fn failed_helper_prevents_http_export_even_with_static_credentials() {
    use opentelemetry_http::HttpClient as _;
    let dir = tempfile::tempdir().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let helper = HeadersHelper::start(
        &OtelHeadersHelper {
            command: "exit 1".to_string(),
            refresh_interval: Duration::from_secs(3600),
            apply_to_logs: true,
            apply_to_traces: true,
            apply_to_metrics: true,
        },
        dir.path(),
        NetworkPolicy::unmanaged(),
    )
    .unwrap();
    let factory = codex_http_client::HttpClientFactory::new(
        codex_http_client::OutboundProxyPolicy::ReqwestDefault,
    );
    let client = crate::otlp::build_async_http_client(
        &factory,
        /*tls*/ None,
        opentelemetry_otlp::OTEL_EXPORTER_OTLP_LOGS_TIMEOUT,
        Some(helper.clone()),
    )
    .unwrap();
    let request = http::Request::post(format!("http://{}/v1/logs", listener.local_addr().unwrap()))
        .header("authorization", "Bearer static-credential")
        .body(bytes::Bytes::new())
        .unwrap();
    assert!(client.send_bytes(request).await.is_err());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    helper.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_and_builtin_metrics_do_not_start_the_helper() {
    use crate::config::OtelExporter;
    let helper = OtelHeadersHelper {
        command: String::new(),
        refresh_interval: Duration::ZERO,
        apply_to_logs: true,
        apply_to_traces: true,
        apply_to_metrics: true,
    };
    for exporter in [
        OtelExporter::None,
        OtelExporter::Statsig,
        OtelExporter::OtlpGrpc {
            endpoint: "http://127.0.0.1:4317".to_string(),
            headers: Default::default(),
            tls: None,
        },
    ] {
        let settings = crate::config::OtelSettings {
            headers_helper: Some(helper.clone()),
            http_client_factory: codex_http_client::HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ),
            environment: "test".to_string(),
            service_name: "test".to_string(),
            service_version: "test".to_string(),
            codex_home: std::path::PathBuf::from("."),
            exporter: OtelExporter::None,
            trace_exporter: OtelExporter::None,
            metrics_exporter: exporter,
            runtime_metrics: false,
            span_attributes: Default::default(),
            tracestate: BTreeMap::new(),
        };
        // Invalid helper settings would fail provider construction if the helper started.
        let provider = crate::OtelProvider::try_new(&settings).unwrap();
        if let Some(provider) = provider {
            provider.shutdown();
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn executable_paths_with_spaces_and_timeouts_are_supported() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("helper with spaces");
    std::fs::write(
        &path,
        "#!/bin/sh\nprintf '%s' '{\"Authorization\":\"token\"}'",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        run_helper(path.to_str().unwrap(), dir.path())
            .await
            .unwrap()["authorization"],
        "token"
    );
    assert_eq!(
        run_helper("sleep 60", dir.path()).await,
        Err("headers helper timed out after 10 seconds".to_string())
    );
}
