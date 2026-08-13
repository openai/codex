use std::io;

use pretty_assertions::assert_eq;

use super::MAX_CACHED_RUSTLS_DESTINATIONS;
use super::RustlsClientCache;
use super::SCHANNEL_PROTOCOL_VERSION_ERROR;
use super::has_retryable_tls_error;
use crate::HttpClientBuilder;
use crate::OutboundProxyRoute;

#[test]
fn recognizes_platform_specific_tls_protocol_negotiation_failures() {
    let errors = [
        ("client error (Connect): bad protocol version", true),
        ("BAD PROTOCOL VERSION", true),
        (
            "error:0A00042E:SSL routines:ssl3_read_bytes:tlsv1 alert protocol version",
            true,
        ),
        ("TLSV1 ALERT PROTOCOL VERSION", true),
        (
            "The function requested is not supported. (os error -2146893054)",
            true,
        ),
        ("Schannel protocol error 0x80090302", true),
        ("SCHANNEL PROTOCOL ERROR 0X80090302", true),
        ("certificate validation failed: bad protocol version", false),
        ("bad protocol version: certificate has expired", false),
        (
            "certificate validation failed: tlsv1 alert protocol version",
            false,
        ),
        (
            "tlsv1 alert protocol version: certificate has expired",
            false,
        ),
        (
            "certificate validation failed (os error -2146893054)",
            false,
        ),
        ("certificate validation failed: 0x80090302", false),
        ("unknown issuer", false),
        ("self-signed certificate", false),
        ("hostname mismatch", false),
        ("connection refused", false),
        ("connection reset", false),
        ("dns lookup failed", false),
        ("tls handshake failed", false),
        ("unsupported protocol", false),
        ("wrong version number", false),
        ("The function requested is not supported.", false),
        (
            "The client and server cannot communicate. (os error -2146893007)",
            false,
        ),
        ("operation timed out", false),
        ("407 Proxy Authentication Required", false),
    ];

    for (message, expected) in errors {
        let error = io::Error::other(message);
        assert_eq!(has_retryable_tls_error(&error), expected, "{message}");
    }
}

#[test]
fn recognizes_schannel_protocol_version_error_codes() {
    let protocol_error = io::Error::from_raw_os_error(SCHANNEL_PROTOCOL_VERSION_ERROR);
    let another_schannel_error = io::Error::from_raw_os_error(/*code*/ -2_146_893_007);

    assert_eq!(
        (
            has_retryable_tls_error(&protocol_error),
            has_retryable_tls_error(&another_schannel_error),
        ),
        (true, false)
    );
}

#[test]
fn certificate_errors_in_an_error_source_never_enable_fallback() {
    for message in [
        "certificate verification failed: bad protocol version",
        "certificate verification failed: tlsv1 alert protocol version",
        "certificate verification failed (os error -2146893054)",
        "certificate verification failed: 0x80090302",
    ] {
        let error = io::Error::other(io::Error::other(message));

        assert!(!has_retryable_tls_error(&error), "{message}");
    }
}

#[test]
fn rustls_fallback_decisions_are_scoped_to_origin_and_outbound_route() {
    let cache = RustlsClientCache::default();
    let destination = reqwest::Url::parse("https://mcp.example.com/first").expect("valid URL");
    let same_origin = reqwest::Url::parse("https://mcp.example.com/second").expect("valid URL");
    let another_host = reqwest::Url::parse("https://another.example.com/first").expect("valid URL");
    let another_port =
        reqwest::Url::parse("https://mcp.example.com:8443/first").expect("valid URL");
    let insecure = reqwest::Url::parse("http://mcp.example.com/first").expect("valid URL");
    let direct = OutboundProxyRoute::Direct;
    let proxy = OutboundProxyRoute::Proxy {
        url: "http://proxy.example.com".to_string(),
        no_proxy: None,
    };

    let client = HttpClientBuilder::new()
        .with_rustls_tls()
        .build_direct()
        .expect("rustls client should build without proxy autodiscovery");
    cache.remember(&destination, &direct, client);

    assert_eq!(
        [
            cache.requires_rustls(&destination, &direct),
            cache.requires_rustls(&same_origin, &direct),
            cache.requires_rustls(&another_host, &direct),
            cache.requires_rustls(&another_port, &direct),
            cache.requires_rustls(&destination, &proxy),
            cache.requires_rustls(&insecure, &direct),
        ],
        [true, true, false, false, false, false]
    );
}

#[test]
fn cached_rustls_clients_are_reused_for_the_same_outbound_route() {
    let cache = RustlsClientCache::default();
    let first_destination =
        reqwest::Url::parse("https://first.example.com").expect("valid first URL");
    let second_destination =
        reqwest::Url::parse("https://second.example.com").expect("valid second URL");
    let direct = OutboundProxyRoute::Direct;
    let client = HttpClientBuilder::new()
        .with_rustls_tls()
        .build_direct()
        .expect("rustls client should build without proxy autodiscovery");

    cache.remember(&first_destination, &direct, client.clone());
    cache.remember(&second_destination, &direct, client);

    assert_eq!(
        (
            cache.requires_rustls(&first_destination, &direct),
            cache.requires_rustls(&second_destination, &direct),
            cache.client_for_route(&direct).is_some(),
            cache
                .state
                .lock()
                .expect("rustls client cache lock")
                .clients
                .len(),
        ),
        (true, true, true, 1)
    );
}

#[test]
fn cached_rustls_destinations_remain_bounded_while_sharing_a_route_client() {
    let cache = RustlsClientCache::default();
    let client = HttpClientBuilder::new()
        .with_rustls_tls()
        .build_direct()
        .expect("rustls client should build without proxy autodiscovery");

    for index in 0..=MAX_CACHED_RUSTLS_DESTINATIONS {
        let destination =
            reqwest::Url::parse(&format!("https://mcp-{index}.example.com")).expect("valid URL");
        cache.remember(&destination, &OutboundProxyRoute::Direct, client.clone());
    }

    let state = cache.state.lock().expect("rustls client cache lock");
    assert_eq!(
        (state.destinations.len(), state.clients.len()),
        (MAX_CACHED_RUSTLS_DESTINATIONS, 1)
    );
}

#[test]
fn evicting_a_destination_removes_its_unshared_route_client() {
    let cache = RustlsClientCache::default();
    let client = HttpClientBuilder::new()
        .with_rustls_tls()
        .build_direct()
        .expect("rustls client should build without proxy autodiscovery");

    for index in 0..=MAX_CACHED_RUSTLS_DESTINATIONS {
        let destination =
            reqwest::Url::parse(&format!("https://mcp-{index}.example.com")).expect("valid URL");
        let route = OutboundProxyRoute::Proxy {
            url: format!("http://proxy-{index}.example.com"),
            no_proxy: None,
        };
        cache.remember(&destination, &route, client.clone());
    }

    let state = cache.state.lock().expect("rustls client cache lock");
    assert_eq!(
        (
            state.destinations.len(),
            state.clients.len(),
            state
                .destinations
                .iter()
                .all(|destination| state.clients.contains_key(&destination.route)),
        ),
        (
            MAX_CACHED_RUSTLS_DESTINATIONS,
            MAX_CACHED_RUSTLS_DESTINATIONS,
            true,
        )
    );
}
