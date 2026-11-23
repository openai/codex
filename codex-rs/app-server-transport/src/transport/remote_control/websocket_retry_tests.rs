//! Checks sustained handshake failures and recovery through the reconnect loop.

use super::tests::TEST_INSTALLATION_ID;
use super::tests::TEST_REMOTE_CONTROL_SERVER_TOKEN;
use super::tests::enabled_desired_state_sender;
use super::tests::initialize_client;
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
use tokio::io::AsyncReadExt;
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
        RemoteControlAuth::capture(remote_control_auth_manager(), /*runtime_auth*/ None).0,
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
    // Start at the cap and advance the retry waits below. Keep socket I/O on
    // real time so a pending handshake cannot auto-advance its own timeout.
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
        let (mut stream, _) = timeout(Duration::from_secs(40), listener.accept())
            .await
            .expect("connection should retry within the capped delay")
            .unwrap();
        if let Some(rejected_at) = rejected_at {
            assert!(
                Instant::now().duration_since(rejected_at) >= Duration::from_secs(15),
                "sustained HTTP 409 responses must not restart fast retries"
            );
        }
        rejected_at = Some(Instant::now());
        let result = accept_hdr_async(&mut stream, |request: &Request, response: Response| {
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
                    .body(/*body*/ None)
                    .unwrap())
            } else {
                Ok(response)
            }
        })
        .await;
        if attempt < 2 {
            assert!(result.is_err());
            drop(result);
            // The peer closes the rejected socket while processing the 409.
            // On this current-thread runtime it then enters the retry sleep
            // before we can observe EOF, so the clock advance cannot race it.
            assert_eq!(stream.read(&mut [0]).await.unwrap(), 0);
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(13)).await;
            tokio::time::resume();
            assert!(
                timeout(Duration::from_secs(1), listener.accept())
                    .await
                    .is_err(),
                "sustained HTTP 409 responses must not restart fast retries"
            );
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(17)).await;
            tokio::time::resume();
        } else {
            let _server_connection = result.expect("available ownership should allow recovery");
            let (mut websocket, outcome) = connect_task.await.unwrap();
            assert!(matches!(outcome, ConnectOutcome::Connected(_)));
            assert_eq!(websocket.reconnect_attempt, 11);
            assert!(
                websocket.next_reconnect_delay().await >= Duration::from_secs(15),
                "a successful handshake must not reset backoff before the connection stays healthy"
            );
            assert_eq!(current_enrollment.snapshot(), Some(enrollment));
            assert_eq!(
                websocket.state.lock().await.subscribe_cursor.as_deref(),
                subscribe_cursor,
            );
            return;
        }
    }
}

#[tokio::test]
async fn fast_retries_restart_when_the_first_controller_attaches() {
    let (server_tx, _server_rx) = mpsc::channel(/*buffer*/ 32);
    let (transport_tx, transport_rx) = mpsc::channel(/*buffer*/ 32);
    let mut clients = ClientTracker::new(server_tx, transport_tx, &CancellationToken::new());
    let mut attempt = 0;
    let fast_retries = [5, 5, 5, 5, 5, 10, 20];

    initialize_client(&mut clients, "backend_host").await;
    assert_reconnect_backoff(&mut attempt, &mut clients, &[5, 10, 20, 30, 30, 30, 30]);
    initialize_client(&mut clients, "first").await;
    assert_reconnect_backoff(&mut attempt, &mut clients, &fast_retries);

    // Replaying initialize must preserve the original connection and its backoff.
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(599)).await;
    let events_before_replay = transport_rx.len();
    initialize_client(&mut clients, "first").await;
    assert_eq!(transport_rx.len(), events_before_replay);
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(clients.has_controller());
    assert!(next_reconnect_delay(&mut attempt, &mut clients) >= Duration::from_secs(15));

    initialize_client(&mut clients, "second").await;
    let stream = StreamId("stream".to_string());
    clients
        .close_client(&(ClientId("first".to_string()), stream.clone()))
        .await
        .unwrap();
    assert!(next_reconnect_delay(&mut attempt, &mut clients) >= Duration::from_secs(15));

    // The last controller leaves and a new one joins without an intervening retry.
    clients
        .close_client(&(ClientId("second".to_string()), stream))
        .await
        .unwrap();
    initialize_client(&mut clients, "replacement").await;
    assert_reconnect_backoff(&mut attempt, &mut clients, &fast_retries);

    // A controller may initialize before the first idle sweep after a long outage.
    tokio::time::advance(Duration::from_secs(600)).await;
    assert!(!clients.has_controller());
    initialize_client(&mut clients, "after-expiry").await;
    assert_reconnect_backoff(&mut attempt, &mut clients, &fast_retries);

    tokio::time::advance(Duration::from_secs(600)).await;
    let ping = serde_json::from_value(serde_json::json!({
        "type": "ping", "client_id": "after-expiry", "stream_id": "stream"
    }))
    .unwrap();
    clients.handle_message(ping).await.unwrap();
    assert_reconnect_backoff(&mut attempt, &mut clients, &fast_retries);
}

#[tokio::test]
async fn legacy_reinitialization_does_not_renew_fast_retries() {
    let (server_tx, _server_rx) = mpsc::channel(/*buffer*/ 32);
    let (transport_tx, _transport_rx) = mpsc::channel(/*buffer*/ 32);
    let mut clients = ClientTracker::new(server_tx, transport_tx, &CancellationToken::new());
    let initialize: ClientEnvelope = serde_json::from_value(serde_json::json!({
        "type": "client_message",
        "client_id": "legacy",
        "message": {"id": 1, "method": "initialize", "params": {}}
    }))
    .unwrap();
    let mut attempt = 0;
    for _ in 0..7 {
        clients.handle_message(initialize.clone()).await.unwrap();
        next_reconnect_delay(&mut attempt, &mut clients);
    }
    assert!(next_reconnect_delay(&mut attempt, &mut clients) >= Duration::from_secs(15));
}

#[tokio::test(start_paused = true)]
async fn last_controller_can_expire_while_the_next_open_is_blocked() {
    let (server_tx, _server_rx) = mpsc::channel(/*buffer*/ 2);
    let (transport_tx, mut transport_rx) = mpsc::channel(/*buffer*/ 2);
    let mut clients = ClientTracker::new(server_tx, transport_tx, &CancellationToken::new());
    initialize_client(&mut clients, "first").await;
    let mut attempt = 0;
    for _ in 0..7 {
        next_reconnect_delay(&mut attempt, &mut clients);
    }
    let wait = Duration::from_millis(5);
    tokio::time::advance(Duration::from_secs(600) - wait).await;
    assert!(clients.has_controller());
    assert_eq!(transport_rx.len(), 2);

    let ((), _first_open) = tokio::join!(
        biased;
        initialize_client(&mut clients, "next"),
        async {
            tokio::time::advance(wait).await;
            let opened = transport_rx.recv().await.unwrap();
            transport_rx.recv().await.unwrap();
            opened
        },
    );
    assert!(next_reconnect_delay(&mut attempt, &mut clients) <= Duration::from_secs(5));
}

#[tokio::test(start_paused = true)]
async fn cancelled_controller_uses_idle_backoff_before_writer_is_reaped() {
    let (server_tx, _server_rx) = mpsc::channel(/*buffer*/ 2);
    let (transport_tx, mut transport_rx) = mpsc::channel(/*buffer*/ 4);
    let mut clients = ClientTracker::new(server_tx, transport_tx, &CancellationToken::new());
    initialize_client(&mut clients, "controller").await;
    let TransportEvent::ConnectionOpened {
        disconnect_sender: Some(token),
        writer: _writer,
        ..
    } = transport_rx.recv().await.unwrap()
    else {
        panic!("missing connection open");
    };
    let mut attempt = 0;
    for _ in 0..3 {
        next_reconnect_delay(&mut attempt, &mut clients);
    }
    token.cancel();
    assert!(!clients.has_controller());
    assert!(next_reconnect_delay(&mut attempt, &mut clients) >= Duration::from_secs(15));

    // A replay on the cancelled stream may open a replacement before the old
    // writer finishes. Its cleanup must not close the replacement.
    initialize_client(&mut clients, "controller").await;
    assert!(
        timeout(Duration::from_millis(10), clients.bookkeep_join_set())
            .await
            .is_err()
    );
    assert!(next_reconnect_delay(&mut attempt, &mut clients) <= Duration::from_secs(5));
}

fn assert_reconnect_backoff(attempt: &mut u64, clients: &mut ClientTracker, max_delays: &[u64]) {
    for &max_delay in max_delays {
        let delay = next_reconnect_delay(attempt, clients);
        assert!(delay >= Duration::from_secs(max_delay) / 2);
        assert!(delay <= Duration::from_secs(max_delay));
    }
    for mut reconnect_attempt in [*attempt, 9, u64::MAX] {
        for _ in 0..1000 {
            let delay = next_reconnect_delay(&mut reconnect_attempt, clients);
            assert!(delay >= Duration::from_secs(15));
            assert!(delay <= Duration::from_secs(30));
        }
    }
}
