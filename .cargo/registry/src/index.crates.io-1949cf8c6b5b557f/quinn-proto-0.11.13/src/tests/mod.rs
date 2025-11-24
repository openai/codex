use std::{
    convert::TryInto,
    mem,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{Arc, Mutex},
};

use assert_matches::assert_matches;
#[cfg(all(feature = "aws-lc-rs", not(feature = "ring")))]
use aws_lc_rs::hmac;
use bytes::{Bytes, BytesMut};
use hex_literal::hex;
use rand::RngCore;
#[cfg(feature = "ring")]
use ring::hmac;
#[cfg(all(feature = "rustls-aws-lc-rs", not(feature = "rustls-ring")))]
use rustls::crypto::aws_lc_rs::default_provider;
#[cfg(feature = "rustls-ring")]
use rustls::crypto::ring::default_provider;
use rustls::{
    AlertDescription, RootCertStore,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    server::WebPkiClientVerifier,
};
use tracing::info;

use super::*;
use crate::{
    Duration, Instant,
    cid_generator::{ConnectionIdGenerator, RandomConnectionIdGenerator},
    crypto::rustls::QuicServerConfig,
    frame::FrameStruct,
    transport_parameters::TransportParameters,
};
mod util;
use util::*;

mod token;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

// Enable this if you want to run these tests in the browser.
// Unfortunately it's either-or: Enable this and you can run in the browser, disable to run in nodejs.
// #[cfg(all(target_family = "wasm", target_os = "unknown"))]
// wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[test]
fn version_negotiate_server() {
    let _guard = subscribe();
    let client_addr = "[::2]:7890".parse().unwrap();
    let mut server = Endpoint::new(
        Default::default(),
        Some(Arc::new(server_config())),
        true,
        None,
    );
    let now = Instant::now();
    let mut buf = Vec::with_capacity(server.config().get_max_udp_payload_size() as usize);
    let event = server.handle(
        now,
        client_addr,
        None,
        None,
        // Long-header packet with reserved version number
        hex!("80 0a1a2a3a 04 00000000 04 00000000 00")[..].into(),
        &mut buf,
    );
    let Some(DatagramEvent::Response(Transmit { .. })) = event else {
        panic!("expected a response");
    };

    assert_ne!(buf[0] & 0x80, 0);
    assert_eq!(&buf[1..15], hex!("00000000 04 00000000 04 00000000"));
    assert!(buf[15..].chunks(4).any(|x| {
        DEFAULT_SUPPORTED_VERSIONS.contains(&u32::from_be_bytes(x.try_into().unwrap()))
    }));
}

#[test]
fn version_negotiate_client() {
    let _guard = subscribe();
    let server_addr = "[::2]:7890".parse().unwrap();
    // Configure client to use empty CIDs so we can easily hardcode a server version negotiation
    // packet
    let cid_generator_factory: fn() -> Box<dyn ConnectionIdGenerator> =
        || Box::new(RandomConnectionIdGenerator::new(0));
    let mut client = Endpoint::new(
        Arc::new(EndpointConfig {
            connection_id_generator_factory: Arc::new(cid_generator_factory),
            ..Default::default()
        }),
        None,
        true,
        None,
    );
    let (_, mut client_ch) = client
        .connect(Instant::now(), client_config(), server_addr, "localhost")
        .unwrap();
    let now = Instant::now();
    let mut buf = Vec::with_capacity(client.config().get_max_udp_payload_size() as usize);
    let opt_event = client.handle(
        now,
        server_addr,
        None,
        None,
        // Version negotiation packet for reserved version, with empty DCID
        hex!(
            "80 00000000 00 04 00000000
             0a1a2a3a"
        )[..]
            .into(),
        &mut buf,
    );
    if let Some(DatagramEvent::ConnectionEvent(_, event)) = opt_event {
        client_ch.handle_event(event);
    }
    assert_matches!(
        client_ch.poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::VersionMismatch,
        })
    );
}

#[test]
fn lifecycle() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    assert!(pair.client_conn_mut(client_ch).using_ecn());
    assert!(pair.server_conn_mut(server_ch).using_ecn());

    const REASON: &[u8] = b"whee";
    info!("closing");
    pair.client.connections.get_mut(&client_ch).unwrap().close(
        pair.time,
        VarInt(42),
        REASON.into(),
    );
    pair.drive();
    assert_matches!(pair.server_conn_mut(server_ch).poll(),
                    Some(Event::ConnectionLost { reason: ConnectionError::ApplicationClosed(
                        ApplicationClose { error_code: VarInt(42), ref reason }
                    )}) if reason == REASON);
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);
}

#[test]
fn draft_version_compat() {
    let _guard = subscribe();

    let mut client_config = client_config();
    client_config.version(0xff00_0020);

    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect_with(client_config);

    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    assert!(pair.client_conn_mut(client_ch).using_ecn());
    assert!(pair.server_conn_mut(server_ch).using_ecn());

    const REASON: &[u8] = b"whee";
    info!("closing");
    pair.client.connections.get_mut(&client_ch).unwrap().close(
        pair.time,
        VarInt(42),
        REASON.into(),
    );
    pair.drive();
    assert_matches!(pair.server_conn_mut(server_ch).poll(),
                    Some(Event::ConnectionLost { reason: ConnectionError::ApplicationClosed(
                        ApplicationClose { error_code: VarInt(42), ref reason }
                    )}) if reason == REASON);
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);
}

#[test]
fn server_stateless_reset() {
    let _guard = subscribe();
    let mut key_material = vec![0; 64];
    let mut rng = rand::rng();
    rng.fill_bytes(&mut key_material);
    let reset_key = hmac::Key::new(hmac::HMAC_SHA256, &key_material);
    rng.fill_bytes(&mut key_material);

    let mut endpoint_config = EndpointConfig::new(Arc::new(reset_key));
    endpoint_config.cid_generator(move || Box::new(HashedConnectionIdGenerator::from_key(0)));
    let endpoint_config = Arc::new(endpoint_config);

    let mut pair = Pair::new(endpoint_config.clone(), server_config());
    let (client_ch, _) = pair.connect();
    pair.drive(); // Flush any post-handshake frames
    pair.server.endpoint =
        Endpoint::new(endpoint_config, Some(Arc::new(server_config())), true, None);
    // Force the server to generate the smallest possible stateless reset
    pair.client.connections.get_mut(&client_ch).unwrap().ping();
    info!("resetting");
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::Reset
        })
    );
}

#[test]
fn client_stateless_reset() {
    let _guard = subscribe();
    let mut key_material = vec![0; 64];
    let mut rng = rand::rng();
    rng.fill_bytes(&mut key_material);
    let reset_key = hmac::Key::new(hmac::HMAC_SHA256, &key_material);
    rng.fill_bytes(&mut key_material);

    let mut endpoint_config = EndpointConfig::new(Arc::new(reset_key));
    endpoint_config.cid_generator(move || Box::new(HashedConnectionIdGenerator::from_key(0)));
    let endpoint_config = Arc::new(endpoint_config);

    let mut pair = Pair::new(endpoint_config.clone(), server_config());
    let (_, server_ch) = pair.connect();
    pair.client.endpoint =
        Endpoint::new(endpoint_config, Some(Arc::new(server_config())), true, None);
    // Send something big enough to allow room for a smaller stateless reset.
    pair.server.connections.get_mut(&server_ch).unwrap().close(
        pair.time,
        VarInt(42),
        (&[0xab; 128][..]).into(),
    );
    info!("resetting");
    pair.drive();
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::Reset
        })
    );
}

/// Verify that stateless resets are rate-limited
#[test]
fn stateless_reset_limit() {
    let _guard = subscribe();
    let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 42);
    let mut endpoint_config = EndpointConfig::default();
    endpoint_config.cid_generator(move || Box::new(RandomConnectionIdGenerator::new(8)));
    let endpoint_config = Arc::new(endpoint_config);
    let mut endpoint = Endpoint::new(
        endpoint_config.clone(),
        Some(Arc::new(server_config())),
        true,
        None,
    );
    let time = Instant::now();
    let mut buf = Vec::new();
    let event = endpoint.handle(time, remote, None, None, [0u8; 1024][..].into(), &mut buf);
    assert!(matches!(event, Some(DatagramEvent::Response(_))));
    let event = endpoint.handle(time, remote, None, None, [0u8; 1024][..].into(), &mut buf);
    assert!(event.is_none());
    let event = endpoint.handle(
        time + endpoint_config.min_reset_interval - Duration::from_nanos(1),
        remote,
        None,
        None,
        [0u8; 1024][..].into(),
        &mut buf,
    );
    assert!(event.is_none());
    let event = endpoint.handle(
        time + endpoint_config.min_reset_interval,
        remote,
        None,
        None,
        [0u8; 1024][..].into(),
        &mut buf,
    );
    assert!(matches!(event, Some(DatagramEvent::Response(_))));
}

#[test]
fn export_keying_material() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    const LABEL: &[u8] = b"test_label";
    const CONTEXT: &[u8] = b"test_context";

    // client keying material
    let mut client_buf = [0u8; 64];
    pair.client_conn_mut(client_ch)
        .crypto_session()
        .export_keying_material(&mut client_buf, LABEL, CONTEXT)
        .unwrap();

    // server keying material
    let mut server_buf = [0u8; 64];
    pair.server_conn_mut(server_ch)
        .crypto_session()
        .export_keying_material(&mut server_buf, LABEL, CONTEXT)
        .unwrap();

    assert_eq!(&client_buf[..], &server_buf[..]);
}

#[test]
fn finish_stream_simple() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();

    const MSG: &[u8] = b"hello";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    assert_eq!(pair.client_streams(client_ch).send_streams(), 1);
    pair.client_send(client_ch, s).finish().unwrap();
    pair.drive();

    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Stream(StreamEvent::Finished { id })) if id == s
    );
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    assert_eq!(pair.client_streams(client_ch).send_streams(), 0);
    assert_eq!(pair.server_conn_mut(client_ch).streams().send_streams(), 0);
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );
    // Receive-only streams do not get `StreamFinished` events
    assert_eq!(pair.server_conn_mut(client_ch).streams().send_streams(), 0);
    assert_matches!(pair.server_streams(server_ch).accept(Dir::Uni), Some(stream) if stream == s);
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(
        chunks.next(usize::MAX),
        Ok(Some(chunk)) if chunk.offset == 0 && chunk.bytes == MSG
    );
    assert_matches!(chunks.next(usize::MAX), Ok(None));
    let _ = chunks.finalize();
}

#[test]
fn reset_stream() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();

    const MSG: &[u8] = b"hello";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    pair.drive();

    info!("resetting stream");
    const ERROR: VarInt = VarInt(42);
    pair.client_send(client_ch, s).reset(ERROR).unwrap();
    pair.drive();

    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );
    assert_matches!(pair.server_streams(server_ch).accept(Dir::Uni), Some(stream) if stream == s);
    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(chunks.next(usize::MAX), Err(ReadError::Reset(ERROR)));
    let _ = chunks.finalize();
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
}

#[test]
fn stop_stream() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    const MSG: &[u8] = b"hello";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    pair.drive();

    info!("stopping stream");
    const ERROR: VarInt = VarInt(42);
    pair.server_recv(server_ch, s).stop(ERROR).unwrap();
    pair.drive();

    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );
    assert_matches!(pair.server_streams(server_ch).accept(Dir::Uni), Some(stream) if stream == s);

    assert_matches!(
        pair.client_send(client_ch, s).write(b"foo"),
        Err(WriteError::Stopped(ERROR))
    );
    assert_matches!(
        pair.client_send(client_ch, s).finish(),
        Err(FinishError::Stopped(ERROR))
    );
}

#[test]
fn reject_self_signed_server_cert() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    info!("connecting");

    // Create a self-signed certificate with a different distinguished name than the default one,
    // such that path building cannot confuse the default root the server is using and the one
    // the client is trusting (in which case we'd get a different error).
    let mut cert = rcgen::CertificateParams::new(["localhost".into()]).unwrap();
    let mut issuer = rcgen::DistinguishedName::new();
    issuer.push(
        rcgen::DnType::OrganizationName,
        "Crazy Quinn's House of Certificates",
    );
    cert.distinguished_name = issuer;
    let cert = cert
        .self_signed(&rcgen::KeyPair::generate().unwrap())
        .unwrap();
    let client_ch = pair.begin_connect(client_config_with_certs(vec![cert.into()]));

    pair.drive();

    assert_matches!(pair.client_conn_mut(client_ch).poll(),
                    Some(Event::ConnectionLost { reason: ConnectionError::TransportError(ref error)})
                    if error.code == TransportErrorCode::crypto(AlertDescription::UnknownCA.into()));
}

#[test]
fn reject_missing_client_cert() {
    let _guard = subscribe();

    let mut store = RootCertStore::empty();
    // `WebPkiClientVerifier` requires a non-empty store, so we stick our own certificate into it
    // because it's convenient.
    store.add(CERTIFIED_KEY.cert.der().clone()).unwrap();

    let key = PrivatePkcs8KeyDer::from(CERTIFIED_KEY.signing_key.serialize_der());
    let cert = CERTIFIED_KEY.cert.der().clone();

    let provider = Arc::new(default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_client_cert_verifier(
            WebPkiClientVerifier::builder_with_provider(Arc::new(store), provider)
                .build()
                .unwrap(),
        )
        .with_single_cert(vec![cert], PrivateKeyDer::from(key))
        .unwrap();
    let config = QuicServerConfig::try_from(config).unwrap();

    let mut pair = Pair::new(
        Default::default(),
        ServerConfig::with_crypto(Arc::new(config)),
    );

    info!("connecting");
    let client_ch = pair.begin_connect(client_config());
    pair.drive();

    // The client completes the connection, but finds it immediately closed
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Connected)
    );
    assert_matches!(pair.client_conn_mut(client_ch).poll(),
                    Some(Event::ConnectionLost { reason: ConnectionError::ConnectionClosed(ref close)})
                    if close.error_code == TransportErrorCode::crypto(AlertDescription::CertificateRequired.into()));

    // The server never completes the connection
    let server_ch = pair.server.assert_accept();
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(pair.server_conn_mut(server_ch).poll(),
                    Some(Event::ConnectionLost { reason: ConnectionError::TransportError(ref error)})
                    if error.code == TransportErrorCode::crypto(AlertDescription::CertificateRequired.into()));
}

#[test]
fn congestion() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, _) = pair.connect();

    const TARGET: u64 = 2048;
    assert!(pair.client_conn_mut(client_ch).congestion_window() > TARGET);
    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    // Send data without receiving ACKs until the congestion state falls below target
    while pair.client_conn_mut(client_ch).congestion_window() > TARGET {
        let n = pair.client_send(client_ch, s).write(&[42; 1024]).unwrap();
        assert_eq!(n, 1024);
        pair.drive_client();
    }
    // Ensure that the congestion state recovers after receiving the ACKs
    pair.drive();
    assert!(pair.client_conn_mut(client_ch).congestion_window() >= TARGET);
    pair.client_send(client_ch, s).write(&[42; 1024]).unwrap();
}

#[test]
fn high_latency_handshake() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    pair.latency = Duration::from_micros(200 * 1000);
    let (client_ch, server_ch) = pair.connect();
    assert_eq!(pair.client_conn_mut(client_ch).bytes_in_flight(), 0);
    assert_eq!(pair.server_conn_mut(server_ch).bytes_in_flight(), 0);
    assert!(pair.client_conn_mut(client_ch).using_ecn());
    assert!(pair.server_conn_mut(server_ch).using_ecn());
}

#[test]
fn zero_rtt_happypath() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    pair.server.handle_incoming = Box::new(validate_incoming);
    let config = client_config();

    // Establish normal connection
    let client_ch = pair.begin_connect(config.clone());
    pair.drive();
    pair.server.assert_accept();
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(0), [][..].into());
    pair.drive();

    pair.client.addr = SocketAddr::new(
        Ipv6Addr::LOCALHOST.into(),
        CLIENT_PORTS.lock().unwrap().next().unwrap(),
    );
    info!("resuming session");
    let client_ch = pair.begin_connect(config);
    assert!(pair.client_conn_mut(client_ch).has_0rtt());
    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    const MSG: &[u8] = b"Hello, 0-RTT!";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    pair.drive();

    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Connected)
    );

    assert!(pair.client_conn_mut(client_ch).accepted_0rtt());
    let server_ch = pair.server.assert_accept();

    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    // We don't currently preserve stream event order wrt. connection events
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Connected)
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(
        chunks.next(usize::MAX),
        Ok(Some(chunk)) if chunk.offset == 0 && chunk.bytes == MSG
    );
    let _ = chunks.finalize();
    assert_eq!(pair.client_conn_mut(client_ch).stats().path.lost_packets, 0);
}

#[test]
fn zero_rtt_rejection() {
    let _guard = subscribe();
    let server_config = ServerConfig::with_crypto(Arc::new(server_crypto_with_alpn(vec![
        "foo".into(),
        "bar".into(),
    ])));
    let mut pair = Pair::new(Arc::new(EndpointConfig::default()), server_config);
    let mut client_crypto = Arc::new(client_crypto_with_alpn(vec!["foo".into()]));
    let client_config = ClientConfig::new(client_crypto.clone());

    // Establish normal connection
    let client_ch = pair.begin_connect(client_config);
    pair.drive();
    let server_ch = pair.server.assert_accept();
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Connected)
    );
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(0), [][..].into());
    pair.drive();
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::ConnectionLost { .. })
    );
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);
    pair.client.connections.clear();
    pair.server.connections.clear();

    // We want to have a TLS client config with the existing session cache (so resumption could
    // happen), but with different ALPN protocols (so that the server must reject it). Reuse
    // the existing `ClientConfig` and change the ALPN protocols to make that happen.
    let this = Arc::get_mut(&mut client_crypto).expect("QuicClientConfig is shared");
    let inner = Arc::get_mut(&mut this.inner).expect("QuicClientConfig.inner is shared");
    inner.alpn_protocols = vec!["bar".into()];

    // Changing protocols invalidates 0-RTT
    let client_config = ClientConfig::new(client_crypto);
    info!("resuming session");
    let client_ch = pair.begin_connect(client_config);
    assert!(pair.client_conn_mut(client_ch).has_0rtt());
    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    const MSG: &[u8] = b"Hello, 0-RTT!";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    pair.drive();
    assert!(!pair.client_conn_mut(client_ch).accepted_0rtt());
    let server_ch = pair.server.assert_accept();
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Connected)
    );
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);
    let s2 = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    assert_eq!(s, s2);

    let mut recv = pair.server_recv(server_ch, s2);
    let mut chunks = recv.read(false).unwrap();
    assert_eq!(chunks.next(usize::MAX), Err(ReadError::Blocked));
    let _ = chunks.finalize();
    assert_eq!(pair.client_conn_mut(client_ch).stats().path.lost_packets, 0);
}

fn test_zero_rtt_incoming_limit<F: FnOnce(&mut ServerConfig)>(configure_server: F) {
    // caller sets the server limit to 4000 bytes
    // the client writes 8000 bytes
    const CLIENT_WRITES: usize = 8000;
    // this gets split across 8 packets
    // the first packet is stored in the Incoming
    // the next three are incoming-buffered, bringing the incoming buffer size to 3600 bytes
    // the last four are dropped due to the buffering limit and must be retransmitted
    const EXPECTED_DROPPED: u64 = 4;

    let _guard = subscribe();
    let mut server_config = server_config();
    configure_server(&mut server_config);
    let mut pair = Pair::new(Arc::new(EndpointConfig::default()), server_config);
    let config = client_config();

    // Establish normal connection
    let client_ch = pair.begin_connect(config.clone());
    pair.drive();
    pair.server.assert_accept();
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(0), [][..].into());
    pair.drive();

    pair.client.addr = SocketAddr::new(
        Ipv6Addr::LOCALHOST.into(),
        CLIENT_PORTS.lock().unwrap().next().unwrap(),
    );
    info!("resuming session");
    pair.server.handle_incoming = Box::new(|_| IncomingConnectionBehavior::Wait);
    let client_ch = pair.begin_connect(config);
    assert!(pair.client_conn_mut(client_ch).has_0rtt());
    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    pair.client_send(client_ch, s)
        .write(&vec![0; CLIENT_WRITES])
        .unwrap();
    pair.drive();
    let incoming = pair.server.waiting_incoming.pop().unwrap();
    assert!(pair.server.waiting_incoming.is_empty());
    let _ = pair.server.try_accept(incoming, pair.time);
    pair.drive();

    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Connected)
    );

    assert!(pair.client_conn_mut(client_ch).accepted_0rtt());
    let server_ch = pair.server.assert_accept();

    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    // We don't currently preserve stream event order wrt. connection events
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Connected)
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    let mut offset = 0;
    loop {
        match chunks.next(usize::MAX) {
            Ok(Some(chunk)) => {
                assert_eq!(chunk.offset as usize, offset);
                offset += chunk.bytes.len();
            }
            Err(ReadError::Blocked) => break,
            Ok(None) => panic!("unexpected stream end"),
            Err(e) => panic!("{}", e),
        }
    }
    assert_eq!(offset, CLIENT_WRITES);
    let _ = chunks.finalize();
    assert_eq!(
        pair.client_conn_mut(client_ch).stats().path.lost_packets,
        EXPECTED_DROPPED
    );
}

#[test]
fn zero_rtt_incoming_buffer_size() {
    test_zero_rtt_incoming_limit(|config| {
        config.incoming_buffer_size(4000);
    });
}

#[test]
fn zero_rtt_incoming_buffer_size_total() {
    test_zero_rtt_incoming_limit(|config| {
        config.incoming_buffer_size_total(4000);
    });
}

#[test]
fn alpn_success() {
    let _guard = subscribe();
    let server_config = ServerConfig::with_crypto(Arc::new(server_crypto_with_alpn(vec![
        "foo".into(),
        "bar".into(),
        "baz".into(),
    ])));

    let mut pair = Pair::new(Arc::new(EndpointConfig::default()), server_config);
    let client_config = ClientConfig::new(Arc::new(client_crypto_with_alpn(vec![
        "bar".into(),
        "quux".into(),
        "corge".into(),
    ])));

    // Establish normal connection
    let client_ch = pair.begin_connect(client_config);
    pair.drive();
    let server_ch = pair.server.assert_accept();
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Connected)
    );

    let hd = pair
        .client_conn_mut(client_ch)
        .crypto_session()
        .handshake_data()
        .unwrap()
        .downcast::<crate::crypto::rustls::HandshakeData>()
        .unwrap();
    assert_eq!(hd.protocol.unwrap(), &b"bar"[..]);
}

#[test]
fn server_alpn_unset() {
    let _guard = subscribe();
    let mut pair = Pair::new(Arc::new(EndpointConfig::default()), server_config());
    let client_config = ClientConfig::new(Arc::new(client_crypto_with_alpn(vec!["foo".into()])));

    let client_ch = pair.begin_connect(client_config);
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::ConnectionLost { reason: ConnectionError::ConnectionClosed(err) }) if err.error_code == TransportErrorCode::crypto(0x78)
    );
}

#[test]
fn client_alpn_unset() {
    let _guard = subscribe();
    let server_config = ServerConfig::with_crypto(Arc::new(server_crypto_with_alpn(vec![
        "foo".into(),
        "bar".into(),
        "baz".into(),
    ])));

    let mut pair = Pair::new(Arc::new(EndpointConfig::default()), server_config);
    let client_ch = pair.begin_connect(client_config());
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::ConnectionLost { reason: ConnectionError::ConnectionClosed(err) }) if err.error_code == TransportErrorCode::crypto(0x78)
    );
}

#[test]
fn alpn_mismatch() {
    let _guard = subscribe();
    let server_config = ServerConfig::with_crypto(Arc::new(server_crypto_with_alpn(vec![
        "foo".into(),
        "bar".into(),
        "baz".into(),
    ])));

    let mut pair = Pair::new(Arc::new(EndpointConfig::default()), server_config);
    let client_ch = pair.begin_connect(ClientConfig::new(Arc::new(client_crypto_with_alpn(vec![
        "quux".into(),
        "corge".into(),
    ]))));

    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::ConnectionLost { reason: ConnectionError::ConnectionClosed(err) }) if err.error_code == TransportErrorCode::crypto(0x78)
    );
}

#[test]
fn stream_id_limit() {
    let _guard = subscribe();
    let server = ServerConfig {
        transport: Arc::new(TransportConfig {
            max_concurrent_uni_streams: 1u32.into(),
            ..TransportConfig::default()
        }),
        ..server_config()
    };
    let mut pair = Pair::new(Default::default(), server);
    let (client_ch, server_ch) = pair.connect();

    let s = pair
        .client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .streams()
        .open(Dir::Uni)
        .expect("couldn't open first stream");
    assert_eq!(
        pair.client_streams(client_ch).open(Dir::Uni),
        None,
        "only one stream is permitted at a time"
    );
    // Generate some activity to allow the server to see the stream
    const MSG: &[u8] = b"hello";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    pair.client_send(client_ch, s).finish().unwrap();
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Stream(StreamEvent::Finished { id })) if id == s
    );
    assert_eq!(
        pair.client_streams(client_ch).open(Dir::Uni),
        None,
        "server does not immediately grant additional credit"
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );
    assert_matches!(pair.server_streams(server_ch).accept(Dir::Uni), Some(stream) if stream == s);

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(
        chunks.next(usize::MAX),
        Ok(Some(chunk)) if chunk.offset == 0 && chunk.bytes == MSG
    );
    assert_eq!(chunks.next(usize::MAX), Ok(None));
    let _ = chunks.finalize();

    // Server will only send MAX_STREAM_ID now that the application's been notified
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Stream(StreamEvent::Available { dir: Dir::Uni }))
    );
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);

    // Try opening the second stream again, now that we've made room
    let s = pair
        .client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .streams()
        .open(Dir::Uni)
        .expect("didn't get stream id budget");
    pair.client_send(client_ch, s).finish().unwrap();
    pair.drive();
    // Make sure the server actually processes data on the newly-available stream
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );
    assert_matches!(pair.server_streams(server_ch).accept(Dir::Uni), Some(stream) if stream == s);
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(chunks.next(usize::MAX), Ok(None));
    let _ = chunks.finalize();
}

#[test]
fn key_update_simple() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();
    let s = pair
        .client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .streams()
        .open(Dir::Bi)
        .expect("couldn't open first stream");

    const MSG1: &[u8] = b"hello1";
    pair.client_send(client_ch, s).write(MSG1).unwrap();
    pair.drive();

    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Bi }))
    );
    assert_matches!(pair.server_streams(server_ch).accept(Dir::Bi), Some(stream) if stream == s);
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);
    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(
        chunks.next(usize::MAX),
        Ok(Some(chunk)) if chunk.offset == 0 && chunk.bytes == MSG1
    );
    let _ = chunks.finalize();

    info!("initiating key update");
    pair.client_conn_mut(client_ch).force_key_update();

    const MSG2: &[u8] = b"hello2";
    pair.client_send(client_ch, s).write(MSG2).unwrap();
    pair.drive();

    assert_matches!(pair.server_conn_mut(server_ch).poll(), Some(Event::Stream(StreamEvent::Readable { id })) if id == s);
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);
    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(
        chunks.next(usize::MAX),
        Ok(Some(chunk)) if chunk.offset == 6 && chunk.bytes == MSG2
    );
    let _ = chunks.finalize();

    assert_eq!(pair.client_conn_mut(client_ch).stats().path.lost_packets, 0);
    assert_eq!(pair.server_conn_mut(server_ch).stats().path.lost_packets, 0);
}

#[test]
fn key_update_reordered() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();
    let s = pair
        .client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .streams()
        .open(Dir::Bi)
        .expect("couldn't open first stream");

    const MSG1: &[u8] = b"1";
    pair.client_send(client_ch, s).write(MSG1).unwrap();
    pair.client.drive(pair.time, pair.server.addr);
    assert!(!pair.client.outbound.is_empty());
    pair.client.delay_outbound();

    pair.client_conn_mut(client_ch).force_key_update();
    info!("updated keys");

    const MSG2: &[u8] = b"two";
    pair.client_send(client_ch, s).write(MSG2).unwrap();
    pair.client.drive(pair.time, pair.server.addr);
    pair.client.finish_delay();
    pair.drive();

    assert_eq!(pair.client_conn_mut(client_ch).stats().path.lost_packets, 0);
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Bi }))
    );
    assert_matches!(pair.server_streams(server_ch).accept(Dir::Bi), Some(stream) if stream == s);

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(true).unwrap();
    let buf1 = chunks.next(usize::MAX).unwrap().unwrap();
    assert_matches!(&*buf1.bytes, MSG1);
    let buf2 = chunks.next(usize::MAX).unwrap().unwrap();
    assert_eq!(buf2.bytes, MSG2);
    let _ = chunks.finalize();

    assert_eq!(pair.client_conn_mut(client_ch).stats().path.lost_packets, 0);
    assert_eq!(pair.server_conn_mut(server_ch).stats().path.lost_packets, 0);
}

#[test]
fn initial_retransmit() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let client_ch = pair.begin_connect(client_config());
    pair.client.drive(pair.time, pair.server.addr);
    pair.client.outbound.clear(); // Drop initial
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Connected)
    );
}

#[test]
fn instant_close_1() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    info!("connecting");
    let client_ch = pair.begin_connect(client_config());
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(0), Bytes::new());
    pair.drive();
    let server_ch = pair.server.assert_accept();
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::ConnectionClosed(ConnectionClose {
                error_code: TransportErrorCode::APPLICATION_ERROR,
                ..
            }),
        })
    );
}

#[test]
fn instant_close_2() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    info!("connecting");
    let client_ch = pair.begin_connect(client_config());
    // Unlike `instant_close`, the server sees a valid Initial packet first.
    pair.drive_client();
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    let server_ch = pair.server.assert_accept();
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::ConnectionClosed(ConnectionClose {
                error_code: TransportErrorCode::APPLICATION_ERROR,
                ..
            }),
        })
    );
}

#[test]
fn instant_server_close() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    info!("connecting");
    pair.begin_connect(client_config());
    pair.drive_client();
    pair.server.drive_incoming(pair.time, pair.client.addr);
    let server_ch = pair.server.assert_accept();
    info!("closing");
    pair.server
        .connections
        .get_mut(&server_ch)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(server_ch).poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::ConnectionClosed(ConnectionClose {
                error_code: TransportErrorCode::APPLICATION_ERROR,
                ..
            }),
        })
    );
}

#[test]
fn idle_timeout() {
    let _guard = subscribe();
    const IDLE_TIMEOUT: u64 = 100;
    let server = ServerConfig {
        transport: Arc::new(TransportConfig {
            max_idle_timeout: Some(VarInt(IDLE_TIMEOUT)),
            ..TransportConfig::default()
        }),
        ..server_config()
    };
    let mut pair = Pair::new(Default::default(), server);
    let (client_ch, server_ch) = pair.connect();
    pair.client_conn_mut(client_ch).ping();
    let start = pair.time;

    while !pair.client_conn_mut(client_ch).is_closed()
        || !pair.server_conn_mut(server_ch).is_closed()
    {
        if !pair.step() {
            if let Some(t) = min_opt(pair.client.next_wakeup(), pair.server.next_wakeup()) {
                pair.time = t;
            }
        }
        pair.client.inbound.clear(); // Simulate total S->C packet loss
    }

    assert!(pair.time - start < Duration::from_millis(2 * IDLE_TIMEOUT));
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::TimedOut,
        })
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::TimedOut,
        })
    );
}

#[test]
fn connection_close_sends_acks() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, _server_ch) = pair.connect();

    let client_acks = pair.client_conn_mut(client_ch).stats().frame_rx.acks;

    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    let time = pair.time;
    pair.server_conn_mut(client_ch)
        .close(time, VarInt(42), Bytes::new());

    pair.drive();

    let client_acks_2 = pair.client_conn_mut(client_ch).stats().frame_rx.acks;
    assert!(
        client_acks_2 > client_acks,
        "Connection close should send pending ACKs"
    );
}

#[test]
fn server_hs_retransmit() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let client_ch = pair.begin_connect(client_config());
    pair.step();
    assert!(!pair.client.inbound.is_empty()); // Initial + Handshakes
    pair.client.inbound.clear();
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Connected)
    );
}

#[test]
fn migration() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();
    pair.drive();

    let client_stats_after_connect = pair.client_conn_mut(client_ch).stats();

    pair.client.addr = SocketAddr::new(
        Ipv4Addr::new(127, 0, 0, 1).into(),
        CLIENT_PORTS.lock().unwrap().next().unwrap(),
    );
    pair.client_conn_mut(client_ch).ping();

    // Assert that just receiving the ping message is accounted into the servers
    // anti-amplification budget
    pair.drive_client();
    pair.drive_server();
    assert_ne!(pair.server_conn_mut(server_ch).total_recvd(), 0);

    pair.drive();
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    assert_eq!(
        pair.server_conn_mut(server_ch).remote_address(),
        pair.client.addr
    );

    // Assert that the client's response to the PATH_CHALLENGE was an IMMEDIATE_ACK, instead of a
    // second ping
    let client_stats_after_migrate = pair.client_conn_mut(client_ch).stats();
    assert_eq!(
        client_stats_after_migrate.frame_tx.ping - client_stats_after_connect.frame_tx.ping,
        1
    );
    assert_eq!(
        client_stats_after_migrate.frame_tx.immediate_ack
            - client_stats_after_connect.frame_tx.immediate_ack,
        1
    );
}

fn test_flow_control(config: TransportConfig, window_size: usize) {
    let _guard = subscribe();
    let mut pair = Pair::new(
        Default::default(),
        ServerConfig {
            transport: Arc::new(config),
            ..server_config()
        },
    );
    let (client_ch, server_ch) = pair.connect();
    let msg = vec![0xAB; window_size + 10];

    // Stream reset before read
    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    info!("writing");
    assert_eq!(pair.client_send(client_ch, s).write(&msg), Ok(window_size));
    assert_eq!(
        pair.client_send(client_ch, s).write(&msg[window_size..]),
        Err(WriteError::Blocked)
    );
    pair.drive();
    info!("resetting");
    pair.client_send(client_ch, s).reset(VarInt(42)).unwrap();
    pair.drive();

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(true).unwrap();
    assert_eq!(
        chunks.next(usize::MAX).err(),
        Some(ReadError::Reset(VarInt(42)))
    );
    let _ = chunks.finalize();

    // Happy path
    info!("writing");
    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    assert_eq!(pair.client_send(client_ch, s).write(&msg), Ok(window_size));
    assert_eq!(
        pair.client_send(client_ch, s).write(&msg[window_size..]),
        Err(WriteError::Blocked)
    );

    pair.drive();
    let mut cursor = 0;
    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(true).unwrap();
    loop {
        match chunks.next(usize::MAX) {
            Ok(Some(chunk)) => {
                cursor += chunk.bytes.len();
            }
            Ok(None) => {
                panic!("end of stream");
            }
            Err(ReadError::Blocked) => {
                break;
            }
            Err(e) => {
                panic!("{}", e);
            }
        }
    }
    let _ = chunks.finalize();

    info!("finished reading");
    assert_eq!(cursor, window_size);
    pair.drive();
    info!("writing");
    assert_eq!(pair.client_send(client_ch, s).write(&msg), Ok(window_size));
    assert_eq!(
        pair.client_send(client_ch, s).write(&msg[window_size..]),
        Err(WriteError::Blocked)
    );

    pair.drive();
    let mut cursor = 0;
    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(true).unwrap();
    loop {
        match chunks.next(usize::MAX) {
            Ok(Some(chunk)) => {
                cursor += chunk.bytes.len();
            }
            Ok(None) => {
                panic!("end of stream");
            }
            Err(ReadError::Blocked) => {
                break;
            }
            Err(e) => {
                panic!("{}", e);
            }
        }
    }
    assert_eq!(cursor, window_size);
    let _ = chunks.finalize();
    info!("finished reading");
}

#[test]
fn stream_flow_control() {
    test_flow_control(
        TransportConfig {
            stream_receive_window: 2000u32.into(),
            ..TransportConfig::default()
        },
        2000,
    );
}

#[test]
fn conn_flow_control() {
    test_flow_control(
        TransportConfig {
            receive_window: 2000u32.into(),
            ..TransportConfig::default()
        },
        2000,
    );
}

#[test]
fn stop_opens_bidi() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();
    assert_eq!(pair.client_streams(client_ch).send_streams(), 0);
    let s = pair.client_streams(client_ch).open(Dir::Bi).unwrap();
    assert_eq!(pair.client_streams(client_ch).send_streams(), 1);
    const ERROR: VarInt = VarInt(42);
    pair.client
        .connections
        .get_mut(&server_ch)
        .unwrap()
        .recv_stream(s)
        .stop(ERROR)
        .unwrap();
    pair.drive();

    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Bi }))
    );
    assert_eq!(pair.server_conn_mut(client_ch).streams().send_streams(), 0);
    assert_matches!(pair.server_streams(server_ch).accept(Dir::Bi), Some(stream) if stream == s);
    assert_eq!(pair.server_conn_mut(client_ch).streams().send_streams(), 1);

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(chunks.next(usize::MAX), Err(ReadError::Blocked));
    let _ = chunks.finalize();

    assert_matches!(
        pair.server_send(server_ch, s).write(b"foo"),
        Err(WriteError::Stopped(ERROR))
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Stopped {
            id: _,
            error_code: ERROR
        }))
    );
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);
}

#[test]
fn implicit_open() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();
    let s1 = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    let s2 = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    pair.client_send(client_ch, s2).write(b"hello").unwrap();
    pair.drive();
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );
    assert_eq!(pair.server_streams(server_ch).accept(Dir::Uni), Some(s1));
    assert_eq!(pair.server_streams(server_ch).accept(Dir::Uni), Some(s2));
    assert_eq!(pair.server_streams(server_ch).accept(Dir::Uni), None);
}

#[test]
fn zero_length_cid() {
    let _guard = subscribe();
    let cid_generator_factory: fn() -> Box<dyn ConnectionIdGenerator> =
        || Box::new(RandomConnectionIdGenerator::new(0));
    let mut pair = Pair::new(
        Arc::new(EndpointConfig {
            connection_id_generator_factory: Arc::new(cid_generator_factory),
            ..EndpointConfig::default()
        }),
        server_config(),
    );
    let (client_ch, server_ch) = pair.connect();
    // Ensure we can reconnect after a previous connection is cleaned up
    info!("closing");
    pair.client
        .connections
        .get_mut(&client_ch)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.drive();
    pair.server
        .connections
        .get_mut(&server_ch)
        .unwrap()
        .close(pair.time, VarInt(42), Bytes::new());
    pair.connect();
}

#[test]
fn keep_alive() {
    let _guard = subscribe();
    const IDLE_TIMEOUT: u64 = 10;
    let server = ServerConfig {
        transport: Arc::new(TransportConfig {
            keep_alive_interval: Some(Duration::from_millis(IDLE_TIMEOUT / 2)),
            max_idle_timeout: Some(VarInt(IDLE_TIMEOUT)),
            ..TransportConfig::default()
        }),
        ..server_config()
    };
    let mut pair = Pair::new(Default::default(), server);
    let (client_ch, server_ch) = pair.connect();
    // Run a good while longer than the idle timeout
    let end = pair.time + Duration::from_millis(20 * IDLE_TIMEOUT);
    while pair.time < end {
        if !pair.step() {
            if let Some(time) = min_opt(pair.client.next_wakeup(), pair.server.next_wakeup()) {
                pair.time = time;
            }
        }
        assert!(!pair.client_conn_mut(client_ch).is_closed());
        assert!(!pair.server_conn_mut(server_ch).is_closed());
    }
}

#[test]
fn cid_rotation() {
    let _guard = subscribe();
    const CID_TIMEOUT: Duration = Duration::from_secs(2);

    let cid_generator_factory: fn() -> Box<dyn ConnectionIdGenerator> =
        || Box::new(*RandomConnectionIdGenerator::new(8).set_lifetime(CID_TIMEOUT));

    // Only test cid rotation on server side to have a clear output trace
    let server = Endpoint::new(
        Arc::new(EndpointConfig {
            connection_id_generator_factory: Arc::new(cid_generator_factory),
            ..EndpointConfig::default()
        }),
        Some(Arc::new(server_config())),
        true,
        None,
    );
    let client = Endpoint::new(Arc::new(EndpointConfig::default()), None, true, None);

    let mut pair = Pair::new_from_endpoint(client, server);
    let (_, server_ch) = pair.connect();

    let mut round: u64 = 1;
    let mut stop = pair.time;
    let end = pair.time + 5 * CID_TIMEOUT;

    use crate::LOC_CID_COUNT;
    use crate::cid_queue::CidQueue;
    let mut active_cid_num = CidQueue::LEN as u64 + 1;
    active_cid_num = active_cid_num.min(LOC_CID_COUNT);
    let mut left_bound = 0;
    let mut right_bound = active_cid_num - 1;

    while pair.time < end {
        stop += CID_TIMEOUT;
        // Run a while until PushNewCID timer fires
        while pair.time < stop {
            if !pair.step() {
                if let Some(time) = min_opt(pair.client.next_wakeup(), pair.server.next_wakeup()) {
                    pair.time = time;
                }
            }
        }
        info!(
            "Checking active cid sequence range before {:?} seconds",
            round * CID_TIMEOUT.as_secs()
        );
        let _bound = (left_bound, right_bound);
        assert_matches!(
            pair.server_conn_mut(server_ch).active_local_cid_seq(),
            _bound
        );
        round += 1;
        left_bound += active_cid_num;
        right_bound += active_cid_num;
        pair.drive_server();
    }
}

#[test]
fn cid_retirement() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    // Server retires current active remote CIDs
    pair.server_conn_mut(server_ch)
        .rotate_local_cid(1, Instant::now());
    pair.drive();
    // Any unexpected behavior may trigger TransportError::CONNECTION_ID_LIMIT_ERROR
    assert!(!pair.client_conn_mut(client_ch).is_closed());
    assert!(!pair.server_conn_mut(server_ch).is_closed());
    assert_matches!(pair.client_conn_mut(client_ch).active_rem_cid_seq(), 1);

    use crate::LOC_CID_COUNT;
    use crate::cid_queue::CidQueue;
    let mut active_cid_num = CidQueue::LEN as u64;
    active_cid_num = active_cid_num.min(LOC_CID_COUNT);

    let next_retire_prior_to = active_cid_num + 1;
    pair.client_conn_mut(client_ch).ping();
    // Server retires all valid remote CIDs
    pair.server_conn_mut(server_ch)
        .rotate_local_cid(next_retire_prior_to, Instant::now());
    pair.drive();
    assert!(!pair.client_conn_mut(client_ch).is_closed());
    assert!(!pair.server_conn_mut(server_ch).is_closed());

    assert_eq!(
        pair.client_conn_mut(client_ch).active_rem_cid_seq(),
        next_retire_prior_to,
    );
}

#[test]
fn finish_stream_flow_control_reordered() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();

    const MSG: &[u8] = b"hello";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    pair.drive_client(); // Send stream data
    pair.server.drive(pair.time, pair.client.addr); // Receive

    // Issue flow control credit
    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(
        chunks.next(usize::MAX),
        Ok(Some(chunk)) if chunk.offset == 0 && chunk.bytes == MSG
    );
    let _ = chunks.finalize();

    pair.server.drive(pair.time, pair.client.addr);
    pair.server.delay_outbound(); // Delay it

    pair.client_send(client_ch, s).finish().unwrap();
    pair.drive_client(); // Send FIN
    pair.server.drive(pair.time, pair.client.addr); // Acknowledge
    pair.server.finish_delay(); // Add flow control packets after
    pair.drive();

    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Stream(StreamEvent::Finished { id })) if id == s
    );
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );
    assert_matches!(pair.server_streams(server_ch).accept(Dir::Uni), Some(stream) if stream == s);

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(chunks.next(usize::MAX), Ok(None));
    let _ = chunks.finalize();
}

#[test]
fn handshake_1rtt_handling() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let client_ch = pair.begin_connect(client_config());
    pair.drive_client();
    pair.drive_server();
    let server_ch = pair.server.assert_accept();
    // Server now has 1-RTT keys, but remains in Handshake state until the TLS CFIN has
    // authenticated the client. Delay the final client handshake flight so that doesn't happen yet.
    pair.client.drive(pair.time, pair.server.addr);
    pair.client.delay_outbound();

    // Send some 1-RTT data which will be received first.
    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    const MSG: &[u8] = b"hello";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    pair.client_send(client_ch, s).finish().unwrap();
    pair.client.drive(pair.time, pair.server.addr);

    // Add the handshake flight back on.
    pair.client.finish_delay();

    pair.drive();

    assert!(pair.client_conn_mut(client_ch).stats().path.lost_packets != 0);
    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(
        chunks.next(usize::MAX),
        Ok(Some(chunk)) if chunk.offset == 0 && chunk.bytes == MSG
    );
    let _ = chunks.finalize();
}

#[test]
fn stop_before_finish() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    const MSG: &[u8] = b"hello";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    pair.drive();

    info!("stopping stream");
    const ERROR: VarInt = VarInt(42);
    pair.server_recv(server_ch, s).stop(ERROR).unwrap();
    pair.drive();

    assert_matches!(
        pair.client_send(client_ch, s).finish(),
        Err(FinishError::Stopped(ERROR))
    );
}

#[test]
fn stop_during_finish() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    const MSG: &[u8] = b"hello";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    pair.drive();

    assert_matches!(pair.server_streams(server_ch).accept(Dir::Uni), Some(stream) if stream == s);
    info!("stopping and finishing stream");
    const ERROR: VarInt = VarInt(42);
    pair.server_recv(server_ch, s).stop(ERROR).unwrap();
    pair.drive_server();
    pair.client_send(client_ch, s).finish().unwrap();
    pair.drive_client();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Stream(StreamEvent::Stopped { id, error_code: ERROR })) if id == s
    );
}

// Ensure we can recover from loss of tail packets when the congestion window is full
#[test]
fn congested_tail_loss() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, _) = pair.connect();

    const TARGET: u64 = 2048;
    assert!(pair.client_conn_mut(client_ch).congestion_window() > TARGET);
    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    // Send data without receiving ACKs until the congestion state falls below target
    while pair.client_conn_mut(client_ch).congestion_window() > TARGET {
        let n = pair.client_send(client_ch, s).write(&[42; 1024]).unwrap();
        assert_eq!(n, 1024);
        pair.drive_client();
    }
    assert!(!pair.server.inbound.is_empty());
    pair.server.inbound.clear();
    // Ensure that the congestion state recovers after retransmits occur and are ACKed
    info!("recovering");
    pair.drive();
    assert!(pair.client_conn_mut(client_ch).congestion_window() > TARGET);
    pair.client_send(client_ch, s).write(&[42; 1024]).unwrap();
}

// Send a tail-loss probe when GSO segment_size is less than INITIAL_MTU
#[test]
fn tail_loss_small_segment_size() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    // No datagrams frames received in the handshake.
    let server_stats = pair.server_conn_mut(server_ch).stats();
    assert_eq!(server_stats.frame_rx.datagram, 0);

    const DGRAM_LEN: usize = 1000; // Below INITIAL_MTU after packet overhead.
    const DGRAM_NUM: u64 = 5; // Enough to build a GSO batch.

    info!("Sending an ack-eliciting datagram");
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    // Drop these packets on the server side.
    assert!(!pair.server.inbound.is_empty());
    pair.server.inbound.clear();

    // Doing one step makes the client advance time to the PTO fire time.
    info!("stepping forward to PTO");
    pair.step();

    // Still no datagrams frames received by the server.
    let server_stats = pair.server_conn_mut(server_ch).stats();
    assert_eq!(server_stats.frame_rx.datagram, 0);

    // Now we can send another batch of datagrams, so the PTO can send them instead of
    // sending a ping.  These are small enough that the segment_size is less than the
    // INITIAL_MTU.
    info!("Sending datagram batch");
    for _ in 0..DGRAM_NUM {
        pair.client_datagrams(client_ch)
            .send(vec![0; DGRAM_LEN].into(), false)
            .unwrap();
    }

    // If this succeeds the datagrams are received by the server and the client did not
    // crash.
    pair.drive();

    // Finally the server should have received some datagrams.
    let server_stats = pair.server_conn_mut(server_ch).stats();
    assert_eq!(server_stats.frame_rx.datagram, DGRAM_NUM);
}

// Respect max_datagrams when TLP happens
#[test]
fn tail_loss_respect_max_datagrams() {
    let _guard = subscribe();
    let client_config = {
        let mut c_config = client_config();
        let mut t_config = TransportConfig::default();
        //Disabling GSO, so only a single segment should be sent per iops
        t_config.enable_segmentation_offload(false);
        c_config.transport_config(t_config.into());
        c_config
    };
    let mut pair = Pair::default();
    let (client_ch, _) = pair.connect_with(client_config);

    const DGRAM_LEN: usize = 1000; // High enough so GSO batch could be built
    const DGRAM_NUM: u64 = 5; // Enough to build a GSO batch.

    info!("Sending an ack-eliciting datagram");
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    // Drop these packets on the server side.
    assert!(!pair.server.inbound.is_empty());
    pair.server.inbound.clear();

    // Doing one step makes the client advance time to the PTO fire time.
    info!("stepping forward to PTO");
    pair.step();

    // start sending datagram batches but the first should be a TLP
    info!("Sending datagram batch");
    for _ in 0..DGRAM_NUM {
        pair.client_datagrams(client_ch)
            .send(vec![0; DGRAM_LEN].into(), false)
            .unwrap();
    }

    pair.drive();

    // Finally checking the number of sent udp datagrams match the number of iops
    let client_stats = pair.client_conn_mut(client_ch).stats();
    assert_eq!(client_stats.udp_tx.ios, client_stats.udp_tx.datagrams);
}

#[test]
fn datagram_send_recv() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);
    assert_matches!(pair.client_datagrams(client_ch).max_size(), Some(x) if x > 0);

    const DATA: &[u8] = b"whee";
    pair.client_datagrams(client_ch)
        .send(DATA.into(), true)
        .unwrap();
    pair.drive();
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::DatagramReceived)
    );
    assert_eq!(pair.server_datagrams(server_ch).recv().unwrap(), DATA);
    assert_matches!(pair.server_datagrams(server_ch).recv(), None);
}

#[test]
fn datagram_recv_buffer_overflow() {
    let _guard = subscribe();
    const WINDOW: usize = 100;
    let server = ServerConfig {
        transport: Arc::new(TransportConfig {
            datagram_receive_buffer_size: Some(WINDOW),
            ..TransportConfig::default()
        }),
        ..server_config()
    };
    let mut pair = Pair::new(Default::default(), server);
    let (client_ch, server_ch) = pair.connect();
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);
    assert_eq!(
        pair.client_conn_mut(client_ch).datagrams().max_size(),
        Some(WINDOW - Datagram::SIZE_BOUND)
    );

    const DATA1: &[u8] = &[0xAB; (WINDOW / 3) + 1];
    const DATA2: &[u8] = &[0xBC; (WINDOW / 3) + 1];
    const DATA3: &[u8] = &[0xCD; (WINDOW / 3) + 1];
    pair.client_datagrams(client_ch)
        .send(DATA1.into(), true)
        .unwrap();
    pair.client_datagrams(client_ch)
        .send(DATA2.into(), true)
        .unwrap();
    pair.client_datagrams(client_ch)
        .send(DATA3.into(), true)
        .unwrap();
    pair.drive();
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::DatagramReceived)
    );
    assert_eq!(pair.server_datagrams(server_ch).recv().unwrap(), DATA2);
    assert_eq!(pair.server_datagrams(server_ch).recv().unwrap(), DATA3);
    assert_matches!(pair.server_datagrams(server_ch).recv(), None);

    pair.client_datagrams(client_ch)
        .send(DATA1.into(), true)
        .unwrap();
    pair.drive();
    assert_eq!(pair.server_datagrams(server_ch).recv().unwrap(), DATA1);
    assert_matches!(pair.server_datagrams(server_ch).recv(), None);
}

#[test]
fn datagram_unsupported() {
    let _guard = subscribe();
    let server = ServerConfig {
        transport: Arc::new(TransportConfig {
            datagram_receive_buffer_size: None,
            ..TransportConfig::default()
        }),
        ..server_config()
    };
    let mut pair = Pair::new(Default::default(), server);
    let (client_ch, server_ch) = pair.connect();
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);
    assert_matches!(pair.client_datagrams(client_ch).max_size(), None);

    match pair.client_datagrams(client_ch).send(Bytes::new(), true) {
        Err(SendDatagramError::UnsupportedByPeer) => {}
        Err(e) => panic!("unexpected error: {e}"),
        Ok(_) => panic!("unexpected success"),
    }
}

#[test]
fn large_initial() {
    let _guard = subscribe();
    let server_config =
        ServerConfig::with_crypto(Arc::new(server_crypto_with_alpn(vec![vec![0, 0, 0, 42]])));

    let mut pair = Pair::new(Arc::new(EndpointConfig::default()), server_config);
    let client_crypto =
        client_crypto_with_alpn((0..1000u32).map(|x| x.to_be_bytes().to_vec()).collect());
    let cfg = ClientConfig::new(Arc::new(client_crypto));
    let client_ch = pair.begin_connect(cfg);
    pair.drive();
    let server_ch = pair.server.assert_accept();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Connected)
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Connected)
    );
}

#[test]
/// Ensure that we don't yield a finish event before the actual FIN is acked so the peer isn't left
/// hanging
fn finish_acked() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();

    const MSG: &[u8] = b"hello";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    info!("client sends data to server");
    pair.drive_client(); // send data to server
    info!("server acknowledges data");
    pair.drive_server(); // process data and send data ack

    // Receive data
    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );
    assert_matches!(pair.server_conn_mut(server_ch).poll(), None);

    assert_matches!(pair.server_streams(server_ch).accept(Dir::Uni), Some(stream) if stream == s);

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(
        chunks.next(usize::MAX),
        Ok(Some(chunk)) if chunk.offset == 0 && chunk.bytes == MSG
    );
    assert_matches!(chunks.next(usize::MAX), Err(ReadError::Blocked));
    let _ = chunks.finalize();

    // Finish before receiving data ack
    pair.client_send(client_ch, s).finish().unwrap();
    // Send FIN, receive data ack
    info!("client receives ACK, sends FIN");
    pair.drive_client();
    // Check for premature finish from data ack
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    // Process FIN ack
    info!("server ACKs FIN");
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Stream(StreamEvent::Finished { id })) if id == s
    );

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(chunks.next(usize::MAX), Ok(None));
    let _ = chunks.finalize();
}

#[test]
/// Ensure that we don't yield a finish event while there's still unacknowledged data
fn finish_retransmit() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();

    const MSG: &[u8] = b"hello";
    pair.client_send(client_ch, s).write(MSG).unwrap();
    pair.drive_client(); // send data to server
    pair.server.inbound.clear(); // Lose it

    // Send FIN
    pair.client_send(client_ch, s).finish().unwrap();
    pair.drive_client();
    // Process FIN
    pair.drive_server();
    // Receive FIN ack, but no data ack
    pair.drive_client();
    // Check for premature finish from FIN ack
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    // Recover
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Stream(StreamEvent::Finished { id })) if id == s
    );

    assert_matches!(
        pair.server_conn_mut(server_ch).poll(),
        Some(Event::Stream(StreamEvent::Opened { dir: Dir::Uni }))
    );

    assert_matches!(pair.server_streams(server_ch).accept(Dir::Uni), Some(stream) if stream == s);

    let mut recv = pair.server_recv(server_ch, s);
    let mut chunks = recv.read(false).unwrap();
    assert_matches!(
        chunks.next(usize::MAX),
        Ok(Some(chunk)) if chunk.offset == 0 && chunk.bytes == MSG
    );
    assert_matches!(chunks.next(usize::MAX), Ok(None));
    let _ = chunks.finalize();
}

/// Ensures that exchanging data on a client-initiated bidirectional stream works past the initial
/// stream window.
#[test]
fn repeated_request_response() {
    let _guard = subscribe();
    let server = ServerConfig {
        transport: Arc::new(TransportConfig {
            max_concurrent_bidi_streams: 1u32.into(),
            ..TransportConfig::default()
        }),
        ..server_config()
    };
    let mut pair = Pair::new(Default::default(), server);
    let (client_ch, server_ch) = pair.connect();
    const REQUEST: &[u8] = b"hello";
    const RESPONSE: &[u8] = b"world";
    for _ in 0..3 {
        let s = pair.client_streams(client_ch).open(Dir::Bi).unwrap();

        pair.client_send(client_ch, s).write(REQUEST).unwrap();
        pair.client_send(client_ch, s).finish().unwrap();

        pair.drive();

        assert_eq!(pair.server_streams(server_ch).accept(Dir::Bi), Some(s));
        let mut recv = pair.server_recv(server_ch, s);
        let mut chunks = recv.read(false).unwrap();
        assert_matches!(
            chunks.next(usize::MAX),
            Ok(Some(chunk)) if chunk.offset == 0 && chunk.bytes == REQUEST
        );

        assert_matches!(chunks.next(usize::MAX), Ok(None));
        let _ = chunks.finalize();
        pair.server_send(server_ch, s).write(RESPONSE).unwrap();
        pair.server_send(server_ch, s).finish().unwrap();

        pair.drive();

        let mut recv = pair.client_recv(client_ch, s);
        let mut chunks = recv.read(false).unwrap();
        assert_matches!(
            chunks.next(usize::MAX),
            Ok(Some(chunk)) if chunk.offset == 0 && chunk.bytes == RESPONSE
        );
        assert_matches!(chunks.next(usize::MAX), Ok(None));
        let _ = chunks.finalize();
    }
}

/// Ensures that the client sends an anti-deadlock probe after an incomplete server's first flight
#[test]
fn handshake_anti_deadlock_probe() {
    let _guard = subscribe();

    let (cert, key) = big_cert_and_key();
    let server = server_config_with_cert(cert.clone(), key);
    let client = client_config_with_certs(vec![cert]);
    let mut pair = Pair::new(Default::default(), server);

    let client_ch = pair.begin_connect(client);
    // Client sends initial
    pair.drive_client();
    // Server sends first flight, gets blocked on anti-amplification
    pair.drive_server();
    // Client acks...
    pair.drive_client();
    // ...but it's lost, so the server doesn't get anti-amplification credit from it
    pair.server.inbound.clear();
    // Client sends an anti-deadlock probe, and the handshake completes as usual.
    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Connected)
    );
}

/// Ensures that the server can respond with 3 initial packets during the handshake
/// before the anti-amplification limit kicks in when MTUs are similar.
#[test]
fn server_can_send_3_inital_packets() {
    let _guard = subscribe();

    let (cert, key) = big_cert_and_key();
    let server = server_config_with_cert(cert.clone(), key);
    let client = client_config_with_certs(vec![cert]);
    let mut pair = Pair::new(Default::default(), server);

    let client_ch = pair.begin_connect(client);
    // Client sends initial
    pair.drive_client();
    // Server sends first flight, gets blocked on anti-amplification
    pair.drive_server();
    // Server should have queued 3 packets at this time
    assert_eq!(pair.client.inbound.len(), 3);

    pair.drive();
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::HandshakeDataReady)
    );
    assert_matches!(
        pair.client_conn_mut(client_ch).poll(),
        Some(Event::Connected)
    );
}

/// Generate a big fat certificate that can't fit inside the initial anti-amplification limit
fn big_cert_and_key() -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
    let cert = rcgen::generate_simple_self_signed(
        Some("localhost".into())
            .into_iter()
            .chain((0..1000).map(|x| format!("foo_{x}")))
            .collect::<Vec<_>>(),
    )
    .unwrap();

    (
        cert.cert.into(),
        PrivateKeyDer::Pkcs8(cert.signing_key.serialize_der().into()),
    )
}

#[test]
fn malformed_token_len() {
    let _guard = subscribe();
    let client_addr = "[::2]:7890".parse().unwrap();
    let mut server = Endpoint::new(
        Default::default(),
        Some(Arc::new(server_config())),
        true,
        None,
    );
    let mut buf = Vec::with_capacity(server.config().get_max_udp_payload_size() as usize);
    server.handle(
        Instant::now(),
        client_addr,
        None,
        None,
        hex!("8900 0000 0101 0000 1b1b 841b 0000 0000 3f00")[..].into(),
        &mut buf,
    );
}

#[test]
fn loss_probe_requests_immediate_ack() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, _) = pair.connect();
    pair.drive();

    let stats_after_connect = pair.client_conn_mut(client_ch).stats();

    // Lose a ping
    let default_mtu = mem::replace(&mut pair.mtu, 0);
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();
    pair.mtu = default_mtu;

    // Drive the connection further so a loss probe is sent
    pair.drive();

    // Assert that two IMMEDIATE_ACKs were sent (two loss probes)
    let stats_after_recovery = pair.client_conn_mut(client_ch).stats();
    assert_eq!(
        stats_after_recovery.frame_tx.immediate_ack - stats_after_connect.frame_tx.immediate_ack,
        2
    );
}

#[test]
/// This is mostly a sanity check to ensure our testing code is correctly dropping packets above the
/// pmtu
fn connect_too_low_mtu() {
    let _guard = subscribe();
    let mut pair = Pair::default();

    // The maximum payload size is lower than 1200, so no packages will get through!
    pair.mtu = 1000;

    pair.begin_connect(client_config());
    pair.drive();
    pair.server.assert_no_accept();
}

#[test]
fn connect_lost_mtu_probes_do_not_trigger_congestion_control() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    pair.mtu = 1200;

    let (client_ch, server_ch) = pair.connect();
    pair.drive();

    let client_stats = pair.client_conn_mut(client_ch).stats();
    let server_stats = pair.server_conn_mut(server_ch).stats();

    // Sanity check (all MTU probes should have been lost)
    assert_eq!(client_stats.path.sent_plpmtud_probes, 9);
    assert_eq!(client_stats.path.lost_plpmtud_probes, 9);
    assert_eq!(server_stats.path.sent_plpmtud_probes, 9);
    assert_eq!(server_stats.path.lost_plpmtud_probes, 9);

    // No congestion events
    assert_eq!(client_stats.path.congestion_events, 0);
    assert_eq!(server_stats.path.congestion_events, 0);
}

#[test]
fn connect_detects_mtu() {
    let _guard = subscribe();
    let max_udp_payload_and_expected_mtu = &[(1200, 1200), (1400, 1389), (1500, 1452)];

    for &(pair_max_udp, expected_mtu) in max_udp_payload_and_expected_mtu {
        let mut pair = Pair::default();
        pair.mtu = pair_max_udp;
        let (client_ch, server_ch) = pair.connect();
        pair.drive();

        assert_eq!(pair.client_conn_mut(client_ch).path_mtu(), expected_mtu);
        assert_eq!(pair.server_conn_mut(server_ch).path_mtu(), expected_mtu);
    }
}

#[test]
fn migrate_detects_new_mtu_and_respects_original_peer_max_udp_payload_size() {
    let _guard = subscribe();

    let client_max_udp_payload_size: u16 = 1400;

    // Set up a client with a max payload size of 1400 (and use the defaults for the server)
    let server_endpoint_config = EndpointConfig::default();
    let server = Endpoint::new(
        Arc::new(server_endpoint_config),
        Some(Arc::new(server_config())),
        true,
        None,
    );
    let client_endpoint_config = EndpointConfig {
        max_udp_payload_size: VarInt::from(client_max_udp_payload_size),
        ..EndpointConfig::default()
    };
    let client = Endpoint::new(Arc::new(client_endpoint_config), None, true, None);
    let mut pair = Pair::new_from_endpoint(client, server);
    pair.mtu = 1300;

    // Connect
    let (client_ch, server_ch) = pair.connect();
    pair.drive();

    // Sanity check: MTUD ran to completion (the numbers differ because binary search stops when
    // changes are smaller than 20, otherwise both endpoints would converge at the same MTU of 1300)
    assert_eq!(pair.client_conn_mut(client_ch).path_mtu(), 1293);
    assert_eq!(pair.server_conn_mut(server_ch).path_mtu(), 1300);

    // Migrate client to a different port (and simulate a higher path MTU)
    pair.mtu = 1500;
    pair.client.addr = SocketAddr::new(
        Ipv4Addr::new(127, 0, 0, 1).into(),
        CLIENT_PORTS.lock().unwrap().next().unwrap(),
    );
    pair.client_conn_mut(client_ch).ping();
    pair.drive();

    // Sanity check: the server saw that the client address was updated
    assert_eq!(
        pair.server_conn_mut(server_ch).remote_address(),
        pair.client.addr
    );

    // MTU detection has successfully run after migrating
    assert_eq!(
        pair.server_conn_mut(server_ch).path_mtu(),
        client_max_udp_payload_size
    );

    // Sanity check: the client keeps the old MTU, because migration is triggered by incoming
    // packets from a different address
    assert_eq!(pair.client_conn_mut(client_ch).path_mtu(), 1293);
}

#[test]
fn connect_runs_mtud_again_after_600_seconds() {
    let _guard = subscribe();
    let mut server_config = server_config();
    let mut client_config = client_config();

    // Note: we use an infinite idle timeout to ensure we can wait 600 seconds without the
    // connection closing
    Arc::get_mut(&mut server_config.transport)
        .unwrap()
        .max_idle_timeout(None);
    Arc::get_mut(&mut client_config.transport)
        .unwrap()
        .max_idle_timeout(None);

    let mut pair = Pair::new(Default::default(), server_config);
    pair.mtu = 1400;
    let (client_ch, server_ch) = pair.connect_with(client_config);
    pair.drive();

    // Sanity check: the mtu has been discovered
    let client_conn = pair.client_conn_mut(client_ch);
    assert_eq!(client_conn.path_mtu(), 1389);
    assert_eq!(client_conn.stats().path.sent_plpmtud_probes, 5);
    assert_eq!(client_conn.stats().path.lost_plpmtud_probes, 3);
    let server_conn = pair.server_conn_mut(server_ch);
    assert_eq!(server_conn.path_mtu(), 1389);
    assert_eq!(server_conn.stats().path.sent_plpmtud_probes, 5);
    assert_eq!(server_conn.stats().path.lost_plpmtud_probes, 3);

    // Sanity check: the mtu does not change after the fact, even though the link now supports a
    // higher udp payload size
    pair.mtu = 1500;
    pair.drive();
    assert_eq!(pair.client_conn_mut(client_ch).path_mtu(), 1389);
    assert_eq!(pair.server_conn_mut(server_ch).path_mtu(), 1389);

    // The MTU changes after 600 seconds, because now MTUD runs for the second time
    pair.time += Duration::from_secs(600);
    pair.drive();
    assert!(!pair.client_conn_mut(client_ch).is_closed());
    assert!(!pair.server_conn_mut(client_ch).is_closed());
    assert_eq!(pair.client_conn_mut(client_ch).path_mtu(), 1452);
    assert_eq!(pair.server_conn_mut(server_ch).path_mtu(), 1452);
}

#[test]
fn blackhole_after_mtu_change_repairs_itself() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    pair.mtu = 1500;
    let (client_ch, server_ch) = pair.connect();
    pair.drive();

    // Sanity check
    assert_eq!(pair.client_conn_mut(client_ch).path_mtu(), 1452);
    assert_eq!(pair.server_conn_mut(server_ch).path_mtu(), 1452);

    // Back to the base MTU
    pair.mtu = 1200;

    // The payload will be sent in a single packet, because the detected MTU was 1444, but it will
    // be dropped because the link no longer supports that packet size!
    let payload = vec![42; 1300];
    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();
    pair.client_send(client_ch, s).write(&payload).unwrap();
    let out_of_bounds = pair.drive_bounded();

    if out_of_bounds {
        panic!("Connections never reached an idle state");
    }

    let recv = pair.server_recv(server_ch, s);
    let buf = stream_chunks(recv);

    // The whole packet arrived in the end
    assert_eq!(buf.len(), 1300);

    // Sanity checks (black hole detected after 3 lost packets)
    let client_stats = pair.client_conn_mut(client_ch).stats();
    assert!(client_stats.path.lost_packets >= 3);
    assert!(client_stats.path.congestion_events >= 3);
    assert_eq!(client_stats.path.black_holes_detected, 1);
}

#[test]
fn mtud_probes_include_immediate_ack() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, _) = pair.connect();
    pair.drive();

    let stats = pair.client_conn_mut(client_ch).stats();
    assert_eq!(stats.path.sent_plpmtud_probes, 4);

    // Each probe contains a ping and an immediate ack
    assert_eq!(stats.frame_tx.ping, 4);
    assert_eq!(stats.frame_tx.immediate_ack, 4);
}

#[test]
fn packet_splitting_with_default_mtu() {
    let _guard = subscribe();

    // The payload needs to be split in 2 in order to be sent, because it is higher than the max MTU
    let payload = vec![42; 1300];

    let mut pair = Pair::default();
    pair.mtu = 1200;
    let (client_ch, _) = pair.connect();
    pair.drive();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();

    pair.client_send(client_ch, s).write(&payload).unwrap();
    pair.client.drive(pair.time, pair.server.addr);
    assert_eq!(pair.client.outbound.len(), 2);

    pair.drive_client();
    assert_eq!(pair.server.inbound.len(), 2);
}

#[test]
fn packet_splitting_not_necessary_after_higher_mtu_discovered() {
    let _guard = subscribe();
    let payload = vec![42; 1300];

    let mut pair = Pair::default();
    pair.mtu = 1500;

    let (client_ch, _) = pair.connect();
    pair.drive();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();

    pair.client_send(client_ch, s).write(&payload).unwrap();
    pair.client.drive(pair.time, pair.server.addr);
    assert_eq!(pair.client.outbound.len(), 1);

    pair.drive_client();
    assert_eq!(pair.server.inbound.len(), 1);
}

#[test]
fn single_ack_eliciting_packet_triggers_ack_after_delay() {
    let _guard = subscribe();
    let mut pair = Pair::default_with_deterministic_pns();
    let (client_ch, _) = pair.connect_with(client_config_with_deterministic_pns());
    pair.drive();

    let stats_after_connect = pair.client_conn_mut(client_ch).stats();

    let start = pair.time;
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client(); // Send ping
    pair.drive_server(); // Process ping
    pair.drive_client(); // Give the client a chance to process an ack, so our assertion can fail

    // Sanity check: the time hasn't advanced in the meantime)
    assert_eq!(pair.time, start);

    let stats_after_ping = pair.client_conn_mut(client_ch).stats();
    assert_eq!(
        stats_after_ping.frame_tx.ping - stats_after_connect.frame_tx.ping,
        1
    );
    assert_eq!(
        stats_after_ping.frame_rx.acks - stats_after_connect.frame_rx.acks,
        0
    );

    pair.client.capture_inbound_packets = true;
    pair.drive();
    let stats_after_drive = pair.client_conn_mut(client_ch).stats();
    assert_eq!(
        stats_after_drive.frame_rx.acks - stats_after_ping.frame_rx.acks,
        1
    );

    // The time is start + max_ack_delay
    let default_max_ack_delay_ms = TransportParameters::default().max_ack_delay.into_inner();
    assert_eq!(
        pair.time,
        start + Duration::from_millis(default_max_ack_delay_ms)
    );

    // The ACK delay is properly calculated
    assert_eq!(pair.client.captured_packets.len(), 1);
    let mut frames = frame::Iter::new(pair.client.captured_packets.remove(0).into())
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(frames.len(), 1);
    if let Frame::Ack(ack) = frames.remove(0) {
        let ack_delay_exp = TransportParameters::default().ack_delay_exponent;
        let delay = ack.delay << ack_delay_exp.into_inner();
        assert_eq!(delay, default_max_ack_delay_ms * 1_000);
    } else {
        panic!("Expected ACK frame");
    }

    // Sanity check: no loss probe was sent, because the delayed ACK was received on time
    assert_eq!(
        stats_after_drive.frame_tx.ping - stats_after_connect.frame_tx.ping,
        1
    );
}

#[test]
fn immediate_ack_triggers_ack() {
    let _guard = subscribe();
    let mut pair = Pair::default_with_deterministic_pns();
    let (client_ch, _) = pair.connect_with(client_config_with_deterministic_pns());
    pair.drive();

    let acks_after_connect = pair.client_conn_mut(client_ch).stats().frame_rx.acks;

    pair.client_conn_mut(client_ch).immediate_ack();
    pair.drive_client(); // Send immediate ack
    pair.drive_server(); // Process immediate ack
    pair.drive_client(); // Give the client a chance to process the ack

    let acks_after_ping = pair.client_conn_mut(client_ch).stats().frame_rx.acks;

    assert_eq!(acks_after_ping - acks_after_connect, 1);
}

#[test]
fn out_of_order_ack_eliciting_packet_triggers_ack() {
    let _guard = subscribe();
    let mut pair = Pair::default_with_deterministic_pns();
    let (client_ch, server_ch) = pair.connect_with(client_config_with_deterministic_pns());
    pair.drive();

    let default_mtu = pair.mtu;

    let client_stats_after_connect = pair.client_conn_mut(client_ch).stats();
    let server_stats_after_connect = pair.server_conn_mut(server_ch).stats();

    // Send a packet that won't arrive right away (it will be dropped and be re-sent later)
    pair.mtu = 0;
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    // Sanity check (ping sent, no ACK received)
    let client_stats_after_first_ping = pair.client_conn_mut(client_ch).stats();
    assert_eq!(
        client_stats_after_first_ping.frame_tx.ping - client_stats_after_connect.frame_tx.ping,
        1
    );
    assert_eq!(
        client_stats_after_first_ping.frame_rx.acks - client_stats_after_connect.frame_rx.acks,
        0
    );

    // Restore the default MTU and send another ping, which will arrive earlier than the dropped one
    pair.mtu = default_mtu;
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();
    pair.drive_server();
    pair.drive_client();

    // Client sanity check (ping sent, one ACK received)
    let client_stats_after_second_ping = pair.client_conn_mut(client_ch).stats();
    assert_eq!(
        client_stats_after_second_ping.frame_tx.ping - client_stats_after_connect.frame_tx.ping,
        2
    );
    assert_eq!(
        client_stats_after_second_ping.frame_rx.acks - client_stats_after_connect.frame_rx.acks,
        1
    );

    // Server checks (single ping received, ACK sent)
    let server_stats_after_second_ping = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after_second_ping.frame_rx.ping - server_stats_after_connect.frame_rx.ping,
        1
    );
    assert_eq!(
        server_stats_after_second_ping.frame_tx.acks - server_stats_after_connect.frame_tx.acks,
        1
    );
}

#[test]
fn single_ack_eliciting_packet_with_ce_bit_triggers_immediate_ack() {
    let _guard = subscribe();
    let mut pair = Pair::default_with_deterministic_pns();
    let (client_ch, _) = pair.connect_with(client_config_with_deterministic_pns());
    pair.drive();

    let stats_after_connect = pair.client_conn_mut(client_ch).stats();

    let start = pair.time;

    pair.client_conn_mut(client_ch).ping();

    pair.congestion_experienced = true;
    pair.drive_client(); // Send ping
    pair.congestion_experienced = false;

    pair.drive_server(); // Process ping, send ACK in response to congestion
    pair.drive_client(); // Process ACK

    // Sanity check: the time hasn't advanced in the meantime)
    assert_eq!(pair.time, start);

    let stats_after_ping = pair.client_conn_mut(client_ch).stats();
    assert_eq!(
        stats_after_ping.frame_tx.ping - stats_after_connect.frame_tx.ping,
        1
    );
    assert_eq!(
        stats_after_ping.frame_rx.acks - stats_after_connect.frame_rx.acks,
        1
    );
    assert_eq!(
        stats_after_ping.path.congestion_events - stats_after_connect.path.congestion_events,
        1
    );
}

fn setup_ack_frequency_test(max_ack_delay: Duration) -> (Pair, ConnectionHandle, ConnectionHandle) {
    let mut client_config = client_config_with_deterministic_pns();
    let mut ack_freq_config = AckFrequencyConfig::default();
    ack_freq_config
        .ack_eliciting_threshold(10u32.into())
        .max_ack_delay(Some(max_ack_delay));
    Arc::get_mut(&mut client_config.transport)
        .unwrap()
        .ack_frequency_config(Some(ack_freq_config))
        .mtu_discovery_config(None); // To keep traffic cleaner

    let mut pair = Pair::default_with_deterministic_pns();
    pair.latency = Duration::from_millis(10); // Need latency to avoid an RTT = 0
    let (client_ch, server_ch) = pair.connect_with(client_config);
    pair.drive();

    assert_eq!(
        pair.client_conn_mut(client_ch)
            .stats()
            .frame_tx
            .ack_frequency,
        1
    );
    assert_eq!(pair.client_conn_mut(client_ch).stats().frame_tx.ping, 0);
    (pair, client_ch, server_ch)
}

/// Verify that max ACK delay is counted from the first ACK-eliciting packet
#[test]
fn ack_frequency_ack_delayed_from_first_of_flight() {
    let _guard = subscribe();
    let (mut pair, client_ch, server_ch) = setup_ack_frequency_test(Duration::from_millis(30));

    // The client sends the following frames:
    //
    // * 0 ms: ping
    // * 5 ms: ping x2
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    pair.time += Duration::from_millis(5);
    for _ in 0..2 {
        pair.client_conn_mut(client_ch).ping();
        pair.drive_client();
    }

    pair.time += Duration::from_millis(5);
    // Server: receive the first ping and send no ACK
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_rx.ping - server_stats_before.frame_rx.ping,
        1
    );
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        0
    );

    // Server: receive the second and third pings and send no ACK
    pair.time += Duration::from_millis(10);
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_rx.ping - server_stats_before.frame_rx.ping,
        2
    );
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        0
    );

    // Server: Send an ACK after ACK delay expires
    pair.time += Duration::from_millis(20);
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        1
    );
}

#[test]
fn ack_frequency_ack_sent_after_max_ack_delay() {
    let _guard = subscribe();
    let max_ack_delay = Duration::from_millis(30);
    let (mut pair, client_ch, server_ch) = setup_ack_frequency_test(max_ack_delay);

    // Client sends a ping
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    // Server: receive the ping, send no ACK
    pair.time += pair.latency;
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_rx.ping - server_stats_before.frame_rx.ping,
        1
    );
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        0
    );

    // Server: send an ack after max_ack_delay has elapsed
    pair.time += max_ack_delay;
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_rx.ping - server_stats_before.frame_rx.ping,
        0
    );
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        1
    );
}

#[test]
fn ack_frequency_ack_sent_after_packets_above_threshold() {
    let _guard = subscribe();
    let max_ack_delay = Duration::from_millis(30);
    let (mut pair, client_ch, server_ch) = setup_ack_frequency_test(max_ack_delay);

    // The client sends the following frames:
    //
    // * 0 ms: ping
    // * 5 ms: ping (11x)
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    pair.time += Duration::from_millis(5);
    for _ in 0..11 {
        pair.client_conn_mut(client_ch).ping();
        pair.drive_client();
    }

    // Server: receive the first ping, send no ACK
    pair.time += Duration::from_millis(5);
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_rx.ping - server_stats_before.frame_rx.ping,
        1
    );
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        0
    );

    // Server: receive the remaining pings, send ACK
    pair.time += Duration::from_millis(5);
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_rx.ping - server_stats_before.frame_rx.ping,
        11
    );
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        1
    );
}

#[test]
fn ack_frequency_ack_sent_after_reordered_packets_below_threshold() {
    let _guard = subscribe();
    let max_ack_delay = Duration::from_millis(30);
    let (mut pair, client_ch, server_ch) = setup_ack_frequency_test(max_ack_delay);

    // The client sends the following frames:
    //
    // * 0 ms: ping
    // * 5 ms: ping (lost)
    // * 5 ms: ping
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    pair.time += Duration::from_millis(5);

    // Send and lose an ack-eliciting packet
    pair.mtu = 0;
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    // Restore the default MTU and send another ping, which will arrive earlier than the dropped one
    pair.mtu = DEFAULT_MTU;
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    // Server: receive first ping, send no ACK
    pair.time += Duration::from_millis(5);
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_rx.ping - server_stats_before.frame_rx.ping,
        1
    );
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        0
    );

    // Server: receive second ping, send no ACK
    pair.time += Duration::from_millis(5);
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_rx.ping - server_stats_before.frame_rx.ping,
        1
    );
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        0
    );
}

#[test]
fn ack_frequency_ack_sent_after_reordered_packets_above_threshold() {
    let _guard = subscribe();
    let max_ack_delay = Duration::from_millis(30);
    let (mut pair, client_ch, server_ch) = setup_ack_frequency_test(max_ack_delay);

    // Send a ping
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    // Send and lose two ack-eliciting packets
    pair.time += Duration::from_millis(5);
    pair.mtu = 0;
    for _ in 0..2 {
        pair.client_conn_mut(client_ch).ping();
        pair.drive_client();
    }

    // Restore the default MTU and send another ping, which will arrive earlier than the dropped ones
    pair.mtu = DEFAULT_MTU;
    pair.client_conn_mut(client_ch).ping();
    pair.drive_client();

    // Server: receive first ping, send no ACK
    pair.time += Duration::from_millis(5);
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_rx.ping - server_stats_before.frame_rx.ping,
        1
    );
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        0
    );

    // Server: receive remaining ping, send ACK
    pair.time += Duration::from_millis(5);
    let server_stats_before = pair.server_conn_mut(server_ch).stats();
    pair.drive_server();
    let server_stats_after = pair.server_conn_mut(server_ch).stats();
    assert_eq!(
        server_stats_after.frame_rx.ping - server_stats_before.frame_rx.ping,
        1
    );
    assert_eq!(
        server_stats_after.frame_tx.acks - server_stats_before.frame_tx.acks,
        1
    );
}

#[test]
fn ack_frequency_update_max_delay() {
    let _guard = subscribe();
    let (mut pair, client_ch, server_ch) = setup_ack_frequency_test(Duration::from_millis(200));

    // Ack frequency was sent initially
    assert_eq!(
        pair.server_conn_mut(server_ch)
            .stats()
            .frame_rx
            .ack_frequency,
        1
    );

    // Client sends a PING
    info!("first ping");
    pair.client_conn_mut(client_ch).ping();
    pair.drive();

    // No change in ACK frequency
    assert_eq!(
        pair.server_conn_mut(server_ch)
            .stats()
            .frame_rx
            .ack_frequency,
        1
    );

    // RTT jumps, client sends another ping
    info!("delayed ping");
    pair.latency *= 10;
    pair.client_conn_mut(client_ch).ping();
    pair.drive();

    // ACK frequency updated
    assert!(
        pair.server_conn_mut(server_ch)
            .stats()
            .frame_rx
            .ack_frequency
            >= 2
    );
}

fn stream_chunks(mut recv: RecvStream) -> Vec<u8> {
    let mut buf = Vec::new();

    let mut chunks = recv.read(true).unwrap();
    while let Ok(Some(chunk)) = chunks.next(usize::MAX) {
        buf.extend(chunk.bytes);
    }

    let _ = chunks.finalize();

    buf
}

/// Verify that an endpoint which receives but does not send ACK-eliciting data still receives ACKs
/// occasionally. This is not required for conformance, but makes loss detection more responsive and
/// reduces receiver memory use.
#[test]
fn pure_sender_voluntarily_acks() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    let receiver_acks_initial = pair.server_conn_mut(server_ch).stats().frame_rx.acks;

    for _ in 0..100 {
        const MSG: &[u8] = b"hello";
        pair.client_datagrams(client_ch)
            .send(Bytes::from_static(MSG), true)
            .unwrap();
        pair.drive();
        assert_eq!(pair.server_datagrams(server_ch).recv().unwrap(), MSG);
    }

    let receiver_acks_final = pair.server_conn_mut(server_ch).stats().frame_rx.acks;
    assert!(receiver_acks_final > receiver_acks_initial);
}

#[test]
fn reject_manually() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    pair.server.handle_incoming = Box::new(|_| IncomingConnectionBehavior::Reject);

    // The server should now reject incoming connections.
    let client_ch = pair.begin_connect(client_config());
    pair.drive();
    pair.server.assert_no_accept();
    let client = pair.client.connections.get_mut(&client_ch).unwrap();
    assert!(client.is_closed());
    assert!(matches!(
        client.poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::ConnectionClosed(close)
        }) if close.error_code == TransportErrorCode::CONNECTION_REFUSED
    ));
}

#[test]
fn validate_then_reject_manually() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    pair.server.handle_incoming = Box::new({
        let mut i = 0;
        move |incoming| {
            if incoming.remote_address_validated() {
                assert_eq!(i, 1);
                i += 1;
                IncomingConnectionBehavior::Reject
            } else {
                assert_eq!(i, 0);
                i += 1;
                IncomingConnectionBehavior::Retry
            }
        }
    });

    // The server should now retry and reject incoming connections.
    let client_ch = pair.begin_connect(client_config());
    pair.drive();
    pair.server.assert_no_accept();
    let client = pair.client.connections.get_mut(&client_ch).unwrap();
    assert!(client.is_closed());
    assert!(matches!(
        client.poll(),
        Some(Event::ConnectionLost {
            reason: ConnectionError::ConnectionClosed(close)
        }) if close.error_code == TransportErrorCode::CONNECTION_REFUSED
    ));
    pair.drive();
    assert_matches!(pair.client_conn_mut(client_ch).poll(), None);
    assert_eq!(pair.client.known_connections(), 0);
    assert_eq!(pair.client.known_cids(), 0);
    assert_eq!(pair.server.known_connections(), 0);
    assert_eq!(pair.server.known_cids(), 0);
}

#[test]
fn endpoint_and_connection_impl_send_sync() {
    const fn is_send_sync<T: Send + Sync>() {}
    is_send_sync::<Endpoint>();
    is_send_sync::<Connection>();
}

#[test]
fn stream_gso() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, _) = pair.connect();

    let s = pair.client_streams(client_ch).open(Dir::Uni).unwrap();

    let initial_ios = pair.client_conn_mut(client_ch).stats().udp_tx.ios;

    // Send 20KiB of stream data, which comfortably fits inside two `tests::util::MAX_DATAGRAMS`
    // datagram batches
    info!("sending");
    for _ in 0..20 {
        pair.client_send(client_ch, s).write(&[0; 1024]).unwrap();
    }
    pair.client_send(client_ch, s).finish().unwrap();
    pair.drive();
    let final_ios = pair.client_conn_mut(client_ch).stats().udp_tx.ios;
    assert_eq!(final_ios - initial_ios, 2);
}

#[test]
fn datagram_gso() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, _) = pair.connect();

    let initial_ios = pair.client_conn_mut(client_ch).stats().udp_tx.ios;
    let initial_bytes = pair.client_conn_mut(client_ch).stats().udp_tx.bytes;

    // Send 10 datagrams above half the MTU, which fits inside a `tests::util::MAX_DATAGRAMS`
    // datagram batch
    info!("sending");
    const DATAGRAM_LEN: usize = 1024;
    const DATAGRAMS: usize = 10;
    for _ in 0..DATAGRAMS {
        pair.client_datagrams(client_ch)
            .send(Bytes::from_static(&[0; DATAGRAM_LEN]), false)
            .unwrap();
    }
    pair.drive();
    let final_ios = pair.client_conn_mut(client_ch).stats().udp_tx.ios;
    let final_bytes = pair.client_conn_mut(client_ch).stats().udp_tx.bytes;
    assert_eq!(final_ios - initial_ios, 1);
    // Expected overhead: flags + CID + PN + tag + frame type + frame length = 1 + 8 + 1 + 16 + 1 + 2 = 29
    assert_eq!(
        final_bytes - initial_bytes,
        ((29 + DATAGRAM_LEN) * DATAGRAMS) as u64
    );
}

#[test]
fn gso_truncation() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    let initial_ios = pair.client_conn_mut(client_ch).stats().udp_tx.ios;

    // Send three application datagrams such that each is large to be combined with another in a
    // single MTU, and the second datagram would require an unreasonably large amount of padding to
    // produce a QUIC packet of the same length as the first.
    info!("sending");
    const SIZES: [usize; 3] = [1024, 768, 768];
    for len in SIZES {
        pair.client_datagrams(client_ch)
            .send(vec![0; len].into(), false)
            .unwrap();
    }
    pair.drive();
    let final_ios = pair.client_conn_mut(client_ch).stats().udp_tx.ios;
    assert_eq!(final_ios - initial_ios, 2);
    for len in SIZES {
        assert_eq!(
            pair.server_datagrams(server_ch)
                .recv()
                .expect("datagram lost")
                .len(),
            len
        );
    }
}

/// Verify that UDP datagrams are padded to MTU if specified in the transport config.
#[test]
fn pad_to_mtu() {
    let _guard = subscribe();
    const MTU: u16 = 1333;
    let client_config = {
        let mut c_config = client_config();
        let t_config = TransportConfig {
            initial_mtu: MTU,
            mtu_discovery_config: None,
            pad_to_mtu: true,
            ..TransportConfig::default()
        };
        c_config.transport_config(t_config.into());
        c_config
    };
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect_with(client_config);

    let initial_ios = pair.client_conn_mut(client_ch).stats().udp_tx.ios;
    pair.server.capture_inbound_packets = true;

    info!("sending");
    // Send two datagrams significantly smaller than MTU, but large enough to require two UDP datagrams.
    const LEN_1: usize = 800;
    const LEN_2: usize = 600;
    pair.client_datagrams(client_ch)
        .send(vec![0; LEN_1].into(), false)
        .unwrap();
    pair.client_datagrams(client_ch)
        .send(vec![0; LEN_2].into(), false)
        .unwrap();
    pair.client.drive(pair.time, pair.server.addr);

    // Check padding
    assert_eq!(pair.client.outbound.len(), 2);
    assert_eq!(pair.client.outbound[0].0.size, usize::from(MTU));
    assert_eq!(pair.client.outbound[0].1.len(), usize::from(MTU));
    assert_eq!(pair.client.outbound[1].0.size, usize::from(MTU));
    assert_eq!(pair.client.outbound[1].1.len(), usize::from(MTU));
    pair.drive_client();
    assert_eq!(pair.server.inbound.len(), 2);
    assert_eq!(pair.server.inbound[0].2.len(), usize::from(MTU));
    assert_eq!(pair.server.inbound[1].2.len(), usize::from(MTU));
    pair.drive();

    // Check that both datagrams ended up in the same GSO batch
    let final_ios = pair.client_conn_mut(client_ch).stats().udp_tx.ios;
    assert_eq!(final_ios - initial_ios, 1);

    assert_eq!(
        pair.server_datagrams(server_ch)
            .recv()
            .expect("datagram lost")
            .len(),
        LEN_1
    );
    assert_eq!(
        pair.server_datagrams(server_ch)
            .recv()
            .expect("datagram lost")
            .len(),
        LEN_2
    );
}

/// Verify that a large application datagram is sent successfully when an ACK frame too large to fit
/// alongside it is also queued, in exactly 2 UDP datagrams.
#[test]
fn large_datagram_with_acks() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, server_ch) = pair.connect();

    // Force the client to generate a large ACK frame by dropping several packets
    for _ in 0..10 {
        pair.server_conn_mut(server_ch).ping();
        pair.drive_server();
        pair.client.inbound.pop_back();
        pair.server_conn_mut(server_ch).ping();
        pair.drive_server();
    }

    let max_size = pair.client_datagrams(client_ch).max_size().unwrap();
    let msg = Bytes::from(vec![0; max_size]);
    pair.client_datagrams(client_ch)
        .send(msg.clone(), true)
        .unwrap();
    let initial_datagrams = pair.client_conn_mut(client_ch).stats().udp_tx.datagrams;
    pair.drive();
    let final_datagrams = pair.client_conn_mut(client_ch).stats().udp_tx.datagrams;
    assert_eq!(pair.server_datagrams(server_ch).recv().unwrap(), msg);
    assert_eq!(final_datagrams - initial_datagrams, 2);
}

/// Verify that an ACK prompted by receipt of many non-ACK-eliciting packets is sent alongside
/// outgoing application datagrams too large to coexist in the same packet with it.
#[test]
fn voluntary_ack_with_large_datagrams() {
    let _guard = subscribe();
    let mut pair = Pair::default();
    let (client_ch, _) = pair.connect();

    // Prompt many large ACKs from the server
    let initial_datagrams = pair.client_conn_mut(client_ch).stats().udp_tx.datagrams;
    // Send enough packets that we're confident some packet numbers will be skipped, ensuring that
    // larger ACKs occur
    const COUNT: usize = 256;
    for _ in 0..COUNT {
        let max_size = pair.client_datagrams(client_ch).max_size().unwrap();
        pair.client_datagrams(client_ch)
            .send(vec![0; max_size].into(), true)
            .unwrap();
        pair.drive();
    }
    let final_datagrams = pair.client_conn_mut(client_ch).stats().udp_tx.datagrams;
    // Failure may indicate `max_size` is too small and ACKs are reliably being packed into the same
    // datagram, which is reasonable behavior but makes this test ineffective.
    assert_ne!(
        final_datagrams - initial_datagrams,
        COUNT as u64,
        "client should have sent some ACK-only packets"
    );
}

#[test]
fn reject_short_idcid() {
    let _guard = subscribe();
    let client_addr = "[::2]:7890".parse().unwrap();
    let mut server = Endpoint::new(
        Default::default(),
        Some(Arc::new(server_config())),
        true,
        None,
    );
    let now = Instant::now();
    let mut buf = Vec::with_capacity(server.config().get_max_udp_payload_size() as usize);
    // Initial header that has an empty DCID but is otherwise well-formed
    let mut initial = BytesMut::from(hex!("c4 00000001 00 00 00 3f").as_ref());
    initial.resize(MIN_INITIAL_SIZE.into(), 0);
    let event = server.handle(now, client_addr, None, None, initial, &mut buf);
    let Some(DatagramEvent::Response(Transmit { .. })) = event else {
        panic!("expected an initial close");
    };
}

/// Ensure that a connection can be made when a preferred address is advertised by the server,
/// regardless of whether the address is actually used.
#[test]
fn preferred_address() {
    let _guard = subscribe();
    let mut server_config = server_config();
    server_config.preferred_address_v6(Some("[::1]:65535".parse().unwrap()));

    let mut pair = Pair::new(Arc::new(EndpointConfig::default()), server_config);
    pair.connect();
}
