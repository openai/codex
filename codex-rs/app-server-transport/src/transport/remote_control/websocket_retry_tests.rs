//! Checks sustained handshake failures and recovery through the reconnect loop.

use super::tests::TEST_INSTALLATION_ID;
use super::tests::TEST_REMOTE_CONTROL_SERVER_TOKEN;
use super::tests::enabled_desired_state_sender;
use super::tests::remote_control_auth_manager;
use super::tests::remote_control_enrollment;
use super::tests::remote_control_state_runtime;
use super::tests::remote_control_status_channel;
use super::tests::remote_control_url_for_listener;
use super::tests::test_current_enrollment;
use super::*;
use crate::transport::remote_control::protocol::normalize_remote_control_url;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::timeout;
use tokio_tungstenite::accept_hdr_async;
use tungstenite::handshake::server::Request;
use tungstenite::handshake::server::Response;

#[tokio::test]
async fn repeated_conflicts_without_cursor_stay_at_cap_and_recover() {
    assert_conflict_recovery(/*subscribe_cursor*/ None).await;
}

#[tokio::test]
async fn repeated_conflicts_stay_at_backoff_cap_and_recover() {
    assert_conflict_recovery(Some("stale-cursor")).await;
}

async fn assert_conflict_recovery(subscribe_cursor: Option<&str>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let remote_control_url = remote_control_url_for_listener(&listener);
    let remote_control_target = normalize_remote_control_url(&remote_control_url).unwrap();
    let codex_home = TempDir::new().unwrap();
    let state_db = remote_control_state_runtime(&codex_home).await;
    let mut enrollment = remote_control_enrollment(Some(TEST_REMOTE_CONTROL_SERVER_TOKEN));
    enrollment.remote_control_target = remote_control_target.clone();
    let current_enrollment = test_current_enrollment(Some(enrollment.clone()));
    let (status_publisher, _status_rx) = remote_control_status_channel();
    let (transport_event_tx, _transport_event_rx) = mpsc::channel(/*buffer*/ 1);
    let shutdown_token = CancellationToken::new();
    let mut websocket = RemoteControlWebsocket::new(
        RemoteControlWebsocketConfig {
            remote_control_url,
            remote_control_target: Some(remote_control_target),
            installation_id: TEST_INSTALLATION_ID.to_string(),
            server_name: "test-server".to_string(),
        },
        Some(state_db),
        RemoteControlAuth::capture(remote_control_auth_manager()).0,
        RemoteControlChannels {
            transport_event_tx,
            status_publisher,
            current_enrollment: current_enrollment.clone(),
            pairing_persistence_key: watch::channel(/*init*/ None).0,
            persistence: RemoteControlPersistence::default(),
        },
        shutdown_token.clone(),
        Arc::new(enabled_desired_state_sender()),
    );
    // Start at the cap to exercise two consecutive capped waits without spending
    // another minute reaching it. Use the real handshake and reconnect loop.
    websocket.reconnect_attempt = 9;
    websocket.state.lock().await.subscribe_cursor = subscribe_cursor.map(str::to_string);
    let connect_task = tokio::spawn(async move {
        let outcome = websocket
            .connect(&shutdown_token, /*app_server_client_name*/ None)
            .await;
        (websocket, outcome)
    });

    let mut rejected_at = None;
    for attempt in 0..3 {
        let (stream, _) = timeout(Duration::from_secs(40), listener.accept())
            .await
            .expect("connection should retry within the capped delay")
            .unwrap();
        if let Some(rejected_at) = rejected_at {
            assert!(
                Instant::now().duration_since(rejected_at) >= Duration::from_secs(30),
                "sustained HTTP 409 responses must not restart fast retries"
            );
        }
        rejected_at = Some(Instant::now());
        let result = accept_hdr_async(stream, |request: &Request, response: Response| {
            assert_eq!(
                request.uri().path(),
                "/backend-api/wham/remote/control/server"
            );
            assert_eq!(
                request
                    .headers()
                    .get(REMOTE_CONTROL_SUBSCRIBE_CURSOR_HEADER),
                subscribe_cursor
                    .map(HeaderValue::from_str)
                    .transpose()
                    .unwrap()
                    .as_ref(),
            );
            if attempt < 2 {
                Err(tungstenite::http::Response::builder()
                    .status(/*status*/ 409)
                    .body(Some("Remote app server already online".to_string()))
                    .unwrap())
            } else {
                Ok(response)
            }
        })
        .await;
        if attempt < 2 {
            assert!(result.is_err());
            if attempt == 1 {
                assert!(
                    timeout(Duration::from_secs(1), listener.accept())
                        .await
                        .is_err(),
                    "sustained HTTP 409 responses must not restart fast retries"
                );
                // The real loop is sleeping after the second rejection. Skip the
                // middle of this wait, retaining its boundary assertion below.
                tokio::time::pause();
                tokio::time::advance(Duration::from_secs(28)).await;
                tokio::time::resume();
            }
        } else {
            let _server_connection = result.expect("available ownership should allow recovery");
            let (websocket, outcome) = connect_task.await.unwrap();
            assert!(matches!(outcome, ConnectOutcome::Connected(_)));
            assert_eq!(websocket.reconnect_attempt, 0);
            assert_eq!(current_enrollment.snapshot(), Some(enrollment));
            assert_eq!(
                websocket.state.lock().await.subscribe_cursor.as_deref(),
                subscribe_cursor,
            );
            return;
        }
    }
}

#[test]
fn reconnect_backoff_stays_capped_during_sustained_failures() {
    let mut reconnect_attempt = 0;
    // Allow the initial exponential ramp, then exercise a long failure streak.
    for _ in 0..10 {
        assert!(next_reconnect_delay(&mut reconnect_attempt) <= Duration::from_secs(30));
    }
    for _ in 0..1000 {
        assert_eq!(
            next_reconnect_delay(&mut reconnect_attempt),
            Duration::from_secs(30)
        );
    }
}
