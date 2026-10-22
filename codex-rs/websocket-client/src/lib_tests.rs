use tokio_tungstenite::tungstenite::http::Uri;

use super::is_loopback_destination;

#[test]
fn recognizes_loopback_websocket_destinations() {
    for destination in [
        "ws://localhost:8080",
        "ws://LOCALHOST:8080",
        "ws://127.0.0.1:8080",
        "ws://[::1]:8080",
    ] {
        let uri = destination.parse::<Uri>().unwrap();
        assert!(
            is_loopback_destination(&uri),
            "expected loopback destination: {destination}; parsed host: {:?}",
            uri.host()
        );
    }
}

#[test]
fn rejects_non_loopback_websocket_destinations() {
    for destination in [
        "ws://relay.example:8080",
        "ws://192.0.2.1:8080",
        "ws://[2001:db8::1]:8080",
        "/missing-host",
    ] {
        assert!(!is_loopback_destination(
            &destination.parse::<Uri>().unwrap()
        ));
    }
}

#[path = "cookie_tests.rs"]
mod cookie_tests;
#[path = "network_policy_tests.rs"]
mod network_policy_tests;
