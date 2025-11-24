//! Tests specifically for tokens

use super::*;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn stateless_retry() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    pair.server.handle_incoming = Box::new(validate_incoming);
    let (client_ch, _server_ch) = pair.connect();
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);
}

#[test]
fn retry_token_expired() {
    let _guard = subscribe();

    let fake_time = Arc::new(FakeTimeSource::new());
    let retry_token_lifetime = Duration::from_secs(1);

    let mut pair = Pair::default();
    pair.server.handle_incoming = Box::new(validate_incoming);

    let mut config = server_config();
    config
        .time_source(Arc::clone(&fake_time) as _)
        .retry_token_lifetime(retry_token_lifetime);
    pair.server.set_server_config(Some(Arc::new(config)));

    let client_ch = pair.begin_connect(client_config());
    pair.drive_client();
    pair.drive_server();
    pair.drive_client();

    // to expire retry token
    fake_time.advance(retry_token_lifetime + Duration::from_millis(1));

    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::ConnectionLost { reason: ConnectionError::ConnectionClosed(err) })
        if err.error_code == TransportErrorCode::INVALID_TOKEN
    );

    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);
}

#[test]
fn use_token() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let client_config = client_config();
    let (client_ch, _server_ch) = pair.connect_with(client_config.clone());
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);

    pair.server.handle_incoming = Box::new(|incoming| {
        assert!(incoming.remote_address_validated());
        assert!(incoming.may_retry());
        IncomingConnectionBehavior::Accept
    });
    let (client_ch_2, _server_ch_2) = pair.connect_with(client_config);
    pair.client
        .connections
        .get_mut(&client_ch_2)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);
}

#[test]
fn retry_then_use_token() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let client_config = client_config();
    pair.server.handle_incoming = Box::new(validate_incoming);
    let (client_ch, _server_ch) = pair.connect_with(client_config.clone());
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);

    pair.server.handle_incoming = Box::new(|incoming| {
        assert!(incoming.remote_address_validated());
        assert!(incoming.may_retry());
        IncomingConnectionBehavior::Accept
    });
    let (client_ch_2, _server_ch_2) = pair.connect_with(client_config);
    pair.client
        .connections
        .get_mut(&client_ch_2)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);
}

#[test]
fn use_token_then_retry() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let client_config = client_config();
    let (client_ch, _server_ch) = pair.connect_with(client_config.clone());
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);

    pair.server.handle_incoming = Box::new({
        let mut i = 0;
        move |incoming| {
            if i == 0 {
                assert!(incoming.remote_address_validated());
                assert!(incoming.may_retry());
                i += 1;
                IncomingConnectionBehavior::Retry
            } else if i == 1 {
                assert!(incoming.remote_address_validated());
                assert!(!incoming.may_retry());
                i += 1;
                IncomingConnectionBehavior::Accept
            } else {
                panic!("too many handle_incoming iterations")
            }
        }
    });
    let (client_ch_2, _server_ch_2) = pair.connect_with(client_config);
    pair.client
        .connections
        .get_mut(&client_ch_2)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);
}

#[test]
fn use_same_token_twice() {
    #[derive(Default)]
    struct EvilTokenStore(Mutex<Bytes>);

    impl TokenStore for EvilTokenStore {
        fn insert(&self, _server_name: &str, token: Bytes) {
            let mut lock = self.0.lock().unwrap();
            if lock.is_empty() {
                *lock = token;
            }
        }

        fn take(&self, _server_name: &str) -> Option<Bytes> {
            let lock = self.0.lock().unwrap();
            if lock.is_empty() {
                None
            } else {
                Some(lock.clone())
            }
        }
    }

    let _guard = subscribe();
    let mut pair = Pair::default();
    let mut client_config = client_config();
    client_config.token_store(Arc::new(EvilTokenStore::default()));
    let (client_ch, _server_ch) = pair.connect_with(client_config.clone());
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);

    pair.server.handle_incoming = Box::new(|incoming| {
        assert!(incoming.remote_address_validated());
        assert!(incoming.may_retry());
        IncomingConnectionBehavior::Accept
    });
    let (client_ch_2, _server_ch_2) = pair.connect_with(client_config.clone());
    pair.client
        .connections
        .get_mut(&client_ch_2)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);

    pair.server.handle_incoming = Box::new(|incoming| {
        assert!(!incoming.remote_address_validated());
        assert!(incoming.may_retry());
        IncomingConnectionBehavior::Accept
    });
    let (client_ch_3, _server_ch_3) = pair.connect_with(client_config);
    pair.client
        .connections
        .get_mut(&client_ch_3)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);
}

#[test]
fn use_token_expired() {
    let _guard = subscribe();
    let fake_time = Arc::new(FakeTimeSource::new());
    let lifetime = Duration::from_secs(10000);
    let mut server_config = server_config();
    server_config
        .time_source(Arc::clone(&fake_time) as _)
        .validation_token
        .lifetime(lifetime);
    let mut pair = Pair::new(Default::default(), server_config);
    let client_config = client_config();
    let (client_ch, _server_ch) = pair.connect_with(client_config.clone());
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);

    pair.server.handle_incoming = Box::new(|incoming| {
        assert!(incoming.remote_address_validated());
        assert!(incoming.may_retry());
        IncomingConnectionBehavior::Accept
    });
    let (client_ch_2, _server_ch_2) = pair.connect_with(client_config.clone());
    pair.client
        .connections
        .get_mut(&client_ch_2)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);

    fake_time.advance(lifetime + Duration::from_secs(1));

    pair.server.handle_incoming = Box::new(|incoming| {
        assert!(!incoming.remote_address_validated());
        assert!(incoming.may_retry());
        IncomingConnectionBehavior::Accept
    });
    let (client_ch_3, _server_ch_3) = pair.connect_with(client_config);
    pair.client
        .connections
        .get_mut(&client_ch_3)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);
}

pub(super) struct FakeTimeSource(Mutex<SystemTime>);

impl FakeTimeSource {
    pub(super) fn new() -> Self {
        Self(Mutex::new(SystemTime::now()))
    }

    pub(super) fn advance(&self, dur: Duration) {
        *self.0.lock().unwrap() += dur;
    }
}

impl TimeSource for FakeTimeSource {
    fn now(&self) -> SystemTime {
        *self.0.lock().unwrap()
    }
}
