use super::BuildTelemetryHttpClientError;
use super::TelemetryClientTlsConfig;
use crate::HttpClientFactory;
use crate::OutboundProxyPolicy;
use crate::cache_system_proxy_route_for_test;
use bytes::Bytes;
use http::Request;
use opentelemetry_http::HttpClient as _;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

#[tokio::test]
async fn async_telemetry_client_uses_resolved_system_proxy() {
    let (proxy_address, proxy) = spawn_proxy();
    let endpoint = "http://async-telemetry-proxy.test/v1/traces";
    cache_system_proxy_route_for_test(endpoint, format!("http://{proxy_address}"));
    let client = HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy)
        .build_async_telemetry_client(
            endpoint,
            Duration::from_secs(2),
            &TelemetryClientTlsConfig::default(),
        )
        .expect("async telemetry client should build");

    let response = client
        .send_bytes(telemetry_request(endpoint))
        .await
        .expect("telemetry request should use proxy");
    let request = proxy.join().expect("proxy should complete");

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        request.lines().next(),
        Some("POST http://async-telemetry-proxy.test/v1/traces HTTP/1.1")
    );
}

#[test]
fn blocking_telemetry_client_uses_resolved_system_proxy() {
    let (proxy_address, proxy) = spawn_proxy();
    let endpoint = "http://blocking-telemetry-proxy.test/v1/metrics";
    cache_system_proxy_route_for_test(endpoint, format!("http://{proxy_address}"));
    let client = HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy)
        .build_blocking_telemetry_client(
            endpoint,
            Duration::from_secs(2),
            &TelemetryClientTlsConfig::default(),
        )
        .expect("blocking telemetry client should build");

    let response = futures::executor::block_on(client.send_bytes(telemetry_request(endpoint)))
        .expect("telemetry request should use proxy");
    let request = proxy.join().expect("proxy should complete");

    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        request.lines().next(),
        Some("POST http://blocking-telemetry-proxy.test/v1/metrics HTTP/1.1")
    );
}

#[tokio::test]
async fn async_system_proxy_telemetry_client_does_not_follow_redirects() {
    let (proxy_address, proxy) = spawn_proxy_with_response(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://different-route.test/v1/traces\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let endpoint = "http://async-telemetry-redirect.test/v1/traces";
    cache_system_proxy_route_for_test(endpoint, format!("http://{proxy_address}"));
    let client = HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy)
        .build_async_telemetry_client(
            endpoint,
            Duration::from_secs(2),
            &TelemetryClientTlsConfig::default(),
        )
        .expect("async telemetry client should build");

    let response = client
        .send_bytes(telemetry_request(endpoint))
        .await
        .expect("redirect should be returned to the exporter");
    proxy.join().expect("proxy should complete");

    assert_eq!(response.status(), http::StatusCode::TEMPORARY_REDIRECT);
}

#[test]
fn blocking_system_proxy_telemetry_client_does_not_follow_redirects() {
    let (proxy_address, proxy) = spawn_proxy_with_response(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://different-route.test/v1/metrics\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let endpoint = "http://blocking-telemetry-redirect.test/v1/metrics";
    cache_system_proxy_route_for_test(endpoint, format!("http://{proxy_address}"));
    let client = HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy)
        .build_blocking_telemetry_client(
            endpoint,
            Duration::from_secs(2),
            &TelemetryClientTlsConfig::default(),
        )
        .expect("blocking telemetry client should build");

    let response = futures::executor::block_on(client.send_bytes(telemetry_request(endpoint)))
        .expect("redirect should be returned to the exporter");
    proxy.join().expect("proxy should complete");

    assert_eq!(response.status(), http::StatusCode::TEMPORARY_REDIRECT);
}

#[test]
fn telemetry_client_rejects_incomplete_client_identity() {
    let result = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
        .build_async_telemetry_client(
            "https://telemetry.example/v1/traces",
            Duration::from_secs(2),
            &TelemetryClientTlsConfig {
                client_certificate: Some(PathBuf::from("client.pem")),
                ..Default::default()
            },
        );

    assert!(matches!(
        result,
        Err(BuildTelemetryHttpClientError::IncompleteClientIdentity)
    ));
}

#[test]
fn telemetry_client_reports_invalid_collector_certificate() {
    let temp = tempfile::tempdir().expect("temporary directory should exist");
    let certificate = temp.path().join("collector.pem");
    std::fs::write(&certificate, "not a certificate").expect("certificate fixture should write");

    let result = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
        .build_async_telemetry_client(
            "https://telemetry.example/v1/traces",
            Duration::from_secs(2),
            &TelemetryClientTlsConfig {
                ca_certificate: Some(certificate.clone()),
                ..Default::default()
            },
        );

    assert!(matches!(
        result,
        Err(BuildTelemetryHttpClientError::InvalidCertificate { path, .. }) if path == certificate
    ));
}

fn telemetry_request(endpoint: &str) -> Request<Bytes> {
    Request::builder()
        .method(http::Method::POST)
        .uri(endpoint)
        .body(Bytes::from_static(b"telemetry"))
        .expect("telemetry request should build")
}

fn spawn_proxy() -> (SocketAddr, thread::JoinHandle<String>) {
    spawn_proxy_with_response("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

fn spawn_proxy_with_response(response: &str) -> (SocketAddr, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("proxy should bind");
    let address = listener.local_addr().expect("proxy should expose address");
    let response = response.to_owned();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("proxy should receive request");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("proxy should set timeout");
        let mut request = [0; 4096];
        let count = stream
            .read(&mut request)
            .expect("proxy should read request");
        stream
            .write_all(response.as_bytes())
            .expect("proxy should write response");
        String::from_utf8_lossy(&request[..count]).into_owned()
    });

    (address, handle)
}
