#![cfg(any(feature = "rustls-aws-lc-rs", feature = "rustls-ring"))]

#[cfg(all(feature = "rustls-aws-lc-rs", not(feature = "rustls-ring")))]
use rustls::crypto::aws_lc_rs::default_provider;
#[cfg(feature = "rustls-ring")]
use rustls::crypto::ring::default_provider;

use std::{
    convert::TryInto,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    str,
    sync::Arc,
};

use crate::runtime::TokioRuntime;
use crate::{Duration, Instant};
use bytes::Bytes;
use proto::{RandomConnectionIdGenerator, crypto::rustls::QuicClientConfig};
use rand::{RngCore, SeedableRng, rngs::StdRng};
use rustls::{
    RootCertStore,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use tokio::runtime::{Builder, Runtime};
use tracing::{error_span, info};
use tracing_futures::Instrument as _;
use tracing_subscriber::EnvFilter;

use super::{ClientConfig, Endpoint, EndpointConfig, RecvStream, SendStream, TransportConfig};

#[test]
fn handshake_timeout() {
    let _guard = subscribe();
    let runtime = rt_threaded();
    let client = {
        let _guard = runtime.enter();
        Endpoint::client(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).unwrap()
    };

    // Avoid NoRootAnchors error
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let mut roots = RootCertStore::empty();
    roots.add(cert.cert.into()).unwrap();

    let mut client_config = crate::ClientConfig::with_root_certificates(Arc::new(roots)).unwrap();
    const IDLE_TIMEOUT: Duration = Duration::from_millis(500);
    let mut transport_config = crate::TransportConfig::default();
    transport_config
        .max_idle_timeout(Some(IDLE_TIMEOUT.try_into().unwrap()))
        .initial_rtt(Duration::from_millis(10));
    client_config.transport_config(Arc::new(transport_config));

    let start = Instant::now();
    runtime.block_on(async move {
        match client
            .connect_with(
                client_config,
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1),
                "localhost",
            )
            .unwrap()
            .await
        {
            Err(crate::ConnectionError::TimedOut) => {}
            Err(e) => panic!("unexpected error: {e:?}"),
            Ok(_) => panic!("unexpected success"),
        }
    });
    let dt = start.elapsed();
    assert!(dt > IDLE_TIMEOUT && dt < 2 * IDLE_TIMEOUT);
}

#[tokio::test]
async fn close_endpoint() {
    let _guard = subscribe();

    // Avoid NoRootAnchors error
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let mut roots = RootCertStore::empty();
    roots.add(cert.cert.into()).unwrap();

    let mut endpoint =
        Endpoint::client(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).unwrap();
    endpoint
        .set_default_client_config(ClientConfig::with_root_certificates(Arc::new(roots)).unwrap());

    let conn = endpoint
        .connect(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1234),
            "localhost",
        )
        .unwrap();

    tokio::spawn(async move {
        let _ = conn.await;
    });

    let conn = endpoint
        .connect(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1234),
            "localhost",
        )
        .unwrap();
    endpoint.close(0u32.into(), &[]);
    match conn.await {
        Err(crate::ConnectionError::LocallyClosed) => (),
        Err(e) => panic!("unexpected error: {e}"),
        Ok(_) => {
            panic!("unexpected success");
        }
    }
}

#[test]
fn local_addr() {
    let socket = UdpSocket::bind((Ipv6Addr::LOCALHOST, 0)).unwrap();
    let addr = socket.local_addr().unwrap();
    let runtime = rt_basic();
    let ep = {
        let _guard = runtime.enter();
        Endpoint::new(Default::default(), None, socket, Arc::new(TokioRuntime)).unwrap()
    };
    assert_eq!(
        addr,
        ep.local_addr()
            .expect("Could not obtain our local endpoint")
    );
}

#[test]
fn read_after_close() {
    let _guard = subscribe();
    let runtime = rt_basic();
    let endpoint = {
        let _guard = runtime.enter();
        endpoint()
    };

    const MSG: &[u8] = b"goodbye!";
    let endpoint2 = endpoint.clone();
    runtime.spawn(async move {
        let new_conn = endpoint2
            .accept()
            .await
            .expect("endpoint")
            .await
            .expect("connection");
        let mut s = new_conn.open_uni().await.unwrap();
        s.write_all(MSG).await.unwrap();
        s.finish().unwrap();
        // Wait for the stream to be closed, one way or another.
        _ = s.stopped().await;
    });
    runtime.block_on(async move {
        let new_conn = endpoint
            .connect(endpoint.local_addr().unwrap(), "localhost")
            .unwrap()
            .await
            .expect("connect");
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut stream = new_conn.accept_uni().await.expect("incoming streams");
        let msg = stream.read_to_end(usize::MAX).await.expect("read_to_end");
        assert_eq!(msg, MSG);
    });
}

#[test]
fn export_keying_material() {
    let _guard = subscribe();
    let runtime = rt_basic();
    let endpoint = {
        let _guard = runtime.enter();
        endpoint()
    };

    runtime.block_on(async move {
        let outgoing_conn_fut = tokio::spawn({
            let endpoint = endpoint.clone();
            async move {
                endpoint
                    .connect(endpoint.local_addr().unwrap(), "localhost")
                    .unwrap()
                    .await
                    .expect("connect")
            }
        });
        let incoming_conn_fut = tokio::spawn({
            let endpoint = endpoint.clone();
            async move {
                endpoint
                    .accept()
                    .await
                    .expect("endpoint")
                    .await
                    .expect("connection")
            }
        });
        let outgoing_conn = outgoing_conn_fut.await.unwrap();
        let incoming_conn = incoming_conn_fut.await.unwrap();
        let mut i_buf = [0u8; 64];
        incoming_conn
            .export_keying_material(&mut i_buf, b"asdf", b"qwer")
            .unwrap();
        let mut o_buf = [0u8; 64];
        outgoing_conn
            .export_keying_material(&mut o_buf, b"asdf", b"qwer")
            .unwrap();
        assert_eq!(&i_buf[..], &o_buf[..]);
    });
}

#[tokio::test]
async fn ip_blocking() {
    let _guard = subscribe();
    let endpoint_factory = EndpointFactory::new();
    let client_1 = endpoint_factory.endpoint();
    let client_1_addr = client_1.local_addr().unwrap();
    let client_2 = endpoint_factory.endpoint();
    let server = endpoint_factory.endpoint();
    let server_addr = server.local_addr().unwrap();
    let server_task = tokio::spawn(async move {
        loop {
            let accepting = server.accept().await.unwrap();
            if accepting.remote_address() == client_1_addr {
                accepting.refuse();
            } else if accepting.remote_address_validated() {
                accepting.await.expect("connection");
            } else {
                accepting.retry().unwrap();
            }
        }
    });
    tokio::join!(
        async move {
            let e = client_1
                .connect(server_addr, "localhost")
                .unwrap()
                .await
                .expect_err("server should have blocked this");
            assert!(
                matches!(e, crate::ConnectionError::ConnectionClosed(_)),
                "wrong error"
            );
        },
        async move {
            client_2
                .connect(server_addr, "localhost")
                .unwrap()
                .await
                .expect("connect");
        }
    );
    server_task.abort();
}

/// Construct an endpoint suitable for connecting to itself
fn endpoint() -> Endpoint {
    EndpointFactory::new().endpoint()
}

fn endpoint_with_config(transport_config: TransportConfig) -> Endpoint {
    EndpointFactory::new().endpoint_with_config(transport_config)
}

/// Constructs endpoints suitable for connecting to themselves and each other
struct EndpointFactory {
    cert: rcgen::CertifiedKey<rcgen::KeyPair>,
    endpoint_config: EndpointConfig,
}

impl EndpointFactory {
    fn new() -> Self {
        Self {
            cert: rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap(),
            endpoint_config: EndpointConfig::default(),
        }
    }

    fn endpoint(&self) -> Endpoint {
        self.endpoint_with_config(TransportConfig::default())
    }

    fn endpoint_with_config(&self, transport_config: TransportConfig) -> Endpoint {
        let key = PrivateKeyDer::Pkcs8(self.cert.signing_key.serialize_der().into());
        let transport_config = Arc::new(transport_config);
        let mut server_config =
            crate::ServerConfig::with_single_cert(vec![self.cert.cert.der().clone()], key).unwrap();
        server_config.transport_config(transport_config.clone());

        let mut roots = rustls::RootCertStore::empty();
        roots.add(self.cert.cert.der().clone()).unwrap();
        let mut endpoint = Endpoint::new(
            self.endpoint_config.clone(),
            Some(server_config),
            UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).unwrap(),
            Arc::new(TokioRuntime),
        )
        .unwrap();
        let mut client_config = ClientConfig::with_root_certificates(Arc::new(roots)).unwrap();
        client_config.transport_config(transport_config);
        endpoint.set_default_client_config(client_config);

        endpoint
    }
}

#[tokio::test]
async fn zero_rtt() {
    let _guard = subscribe();
    let endpoint = endpoint();

    const MSG0: &[u8] = b"zero";
    const MSG1: &[u8] = b"one";
    let endpoint2 = endpoint.clone();
    tokio::spawn(async move {
        for _ in 0..2 {
            let incoming = endpoint2.accept().await.unwrap().accept().unwrap();
            let (connection, established) = incoming.into_0rtt().unwrap_or_else(|_| unreachable!());
            let c = connection.clone();
            tokio::spawn(async move {
                while let Ok(mut x) = c.accept_uni().await {
                    let msg = x.read_to_end(usize::MAX).await.unwrap();
                    assert_eq!(msg, MSG0);
                }
            });
            info!("sending 0.5-RTT");
            let mut s = connection.open_uni().await.expect("open_uni");
            s.write_all(MSG0).await.expect("write");
            s.finish().unwrap();
            established.await;
            info!("sending 1-RTT");
            let mut s = connection.open_uni().await.expect("open_uni");
            s.write_all(MSG1).await.expect("write");
            // The peer might close the connection before ACKing
            let _ = s.finish();
        }
    });

    let connection = endpoint
        .connect(endpoint.local_addr().unwrap(), "localhost")
        .unwrap()
        .into_0rtt()
        .err()
        .expect("0-RTT succeeded without keys")
        .await
        .expect("connect");

    {
        let mut stream = connection.accept_uni().await.expect("incoming streams");
        let msg = stream.read_to_end(usize::MAX).await.expect("read_to_end");
        assert_eq!(msg, MSG0);
        // Read a 1-RTT message to ensure the handshake completes fully, allowing the server's
        // NewSessionTicket frame to be received.
        let mut stream = connection.accept_uni().await.expect("incoming streams");
        let msg = stream.read_to_end(usize::MAX).await.expect("read_to_end");
        assert_eq!(msg, MSG1);
        drop(connection);
    }

    info!("initial connection complete");

    let (connection, zero_rtt) = endpoint
        .connect(endpoint.local_addr().unwrap(), "localhost")
        .unwrap()
        .into_0rtt()
        .unwrap_or_else(|_| panic!("missing 0-RTT keys"));
    // Send something ASAP to use 0-RTT
    let c = connection.clone();
    tokio::spawn(async move {
        let mut s = c.open_uni().await.expect("0-RTT open uni");
        info!("sending 0-RTT");
        s.write_all(MSG0).await.expect("0-RTT write");
        s.finish().unwrap();
    });

    let mut stream = connection.accept_uni().await.expect("incoming streams");
    let msg = stream.read_to_end(usize::MAX).await.expect("read_to_end");
    assert_eq!(msg, MSG0);
    assert!(zero_rtt.await);

    drop((stream, connection));

    endpoint.wait_idle().await;
}

#[test]
#[cfg_attr(
    any(target_os = "solaris", target_os = "illumos"),
    ignore = "Fails on Solaris and Illumos"
)]
fn echo_v6() {
    run_echo(EchoArgs {
        client_addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        server_addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
        nr_streams: 1,
        stream_size: 10 * 1024,
        receive_window: None,
        stream_receive_window: None,
    });
}

#[test]
#[cfg_attr(target_os = "solaris", ignore = "Sometimes hangs in poll() on Solaris")]
fn echo_v4() {
    run_echo(EchoArgs {
        client_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        server_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        nr_streams: 1,
        stream_size: 10 * 1024,
        receive_window: None,
        stream_receive_window: None,
    });
}

#[test]
#[cfg_attr(target_os = "solaris", ignore = "Hangs in poll() on Solaris")]
fn echo_dualstack() {
    run_echo(EchoArgs {
        client_addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        server_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        nr_streams: 1,
        stream_size: 10 * 1024,
        receive_window: None,
        stream_receive_window: None,
    });
}

#[test]
#[ignore]
#[cfg_attr(target_os = "solaris", ignore = "Hangs in poll() on Solaris")]
fn stress_receive_window() {
    run_echo(EchoArgs {
        client_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        server_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        nr_streams: 50,
        stream_size: 25 * 1024 + 11,
        receive_window: Some(37),
        stream_receive_window: Some(100 * 1024 * 1024),
    });
}

#[test]
#[ignore]
#[cfg_attr(target_os = "solaris", ignore = "Hangs in poll() on Solaris")]
fn stress_stream_receive_window() {
    // Note that there is no point in running this with too many streams,
    // since the window is only active within a stream.
    run_echo(EchoArgs {
        client_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        server_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        nr_streams: 2,
        stream_size: 250 * 1024 + 11,
        receive_window: Some(100 * 1024 * 1024),
        stream_receive_window: Some(37),
    });
}

#[test]
#[ignore]
#[cfg_attr(target_os = "solaris", ignore = "Hangs in poll() on Solaris")]
fn stress_both_windows() {
    run_echo(EchoArgs {
        client_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        server_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        nr_streams: 50,
        stream_size: 25 * 1024 + 11,
        receive_window: Some(37),
        stream_receive_window: Some(37),
    });
}

fn run_echo(args: EchoArgs) {
    let _guard = subscribe();
    let runtime = rt_basic();
    let handle = {
        // Use small receive windows
        let mut transport_config = TransportConfig::default();
        if let Some(receive_window) = args.receive_window {
            transport_config.receive_window(receive_window.try_into().unwrap());
        }
        if let Some(stream_receive_window) = args.stream_receive_window {
            transport_config.stream_receive_window(stream_receive_window.try_into().unwrap());
        }
        transport_config.max_concurrent_bidi_streams(1_u8.into());
        transport_config.max_concurrent_uni_streams(1_u8.into());
        let transport_config = Arc::new(transport_config);

        // We don't use the `endpoint` helper here because we want two different endpoints with
        // different addresses.
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let key = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
        let cert = CertificateDer::from(cert.cert);
        let mut server_config =
            crate::ServerConfig::with_single_cert(vec![cert.clone()], key.into()).unwrap();

        server_config.transport = transport_config.clone();
        let server_sock = UdpSocket::bind(args.server_addr).unwrap();
        let server_addr = server_sock.local_addr().unwrap();
        let server = {
            let _guard = runtime.enter();
            let _guard = error_span!("server").entered();
            Endpoint::new(
                Default::default(),
                Some(server_config),
                server_sock,
                Arc::new(TokioRuntime),
            )
            .unwrap()
        };

        let mut roots = rustls::RootCertStore::empty();
        roots.add(cert).unwrap();
        let mut client_crypto =
            rustls::ClientConfig::builder_with_provider(default_provider().into())
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(roots)
                .with_no_client_auth();
        client_crypto.key_log = Arc::new(rustls::KeyLogFile::new());

        let mut client = {
            let _guard = runtime.enter();
            let _guard = error_span!("client").entered();
            Endpoint::client(args.client_addr).unwrap()
        };
        let mut client_config =
            ClientConfig::new(Arc::new(QuicClientConfig::try_from(client_crypto).unwrap()));
        client_config.transport_config(transport_config);
        client.set_default_client_config(client_config);

        let handle = runtime.spawn(async move {
            let incoming = server.accept().await.unwrap();

            // Note for anyone modifying the platform support in this test:
            // If `local_ip` gets available on additional platforms - which
            // requires modifying this test - please update the list of supported
            // platforms in the doc comment of `quinn_udp::RecvMeta::dst_ip`.
            if cfg!(target_os = "linux")
                || cfg!(target_os = "android")
                || cfg!(target_os = "freebsd")
                || cfg!(target_os = "openbsd")
                || cfg!(target_os = "netbsd")
                || cfg!(target_os = "macos")
                || cfg!(target_os = "windows")
            {
                let local_ip = incoming.local_ip().expect("Local IP must be available");
                assert!(local_ip.is_loopback());
            } else {
                assert_eq!(None, incoming.local_ip());
            }

            let new_conn = incoming.await.unwrap();
            tokio::spawn(async move {
                while let Ok(stream) = new_conn.accept_bi().await {
                    tokio::spawn(echo(stream));
                }
            });
            server.wait_idle().await;
        });

        info!("connecting from {} to {}", args.client_addr, server_addr);
        runtime.block_on(
            async move {
                let new_conn = client
                    .connect(server_addr, "localhost")
                    .unwrap()
                    .await
                    .expect("connect");

                /// This is just an arbitrary number to generate deterministic test data
                const SEED: u64 = 0x12345678;

                for i in 0..args.nr_streams {
                    println!("Opening stream {i}");
                    let (mut send, mut recv) = new_conn.open_bi().await.expect("stream open");
                    let msg = gen_data(args.stream_size, SEED);

                    let send_task = async {
                        send.write_all(&msg).await.expect("write");
                        send.finish().unwrap();
                    };
                    let recv_task = async { recv.read_to_end(usize::MAX).await.expect("read") };

                    let (_, data) = tokio::join!(send_task, recv_task);

                    assert_eq!(data[..], msg[..], "Data mismatch");
                }
                new_conn.close(0u32.into(), b"done");
                client.wait_idle().await;
            }
            .instrument(error_span!("client")),
        );
        handle
    };
    runtime.block_on(handle).unwrap();
}

struct EchoArgs {
    client_addr: SocketAddr,
    server_addr: SocketAddr,
    nr_streams: usize,
    stream_size: usize,
    receive_window: Option<u64>,
    stream_receive_window: Option<u64>,
}

async fn echo((mut send, mut recv): (SendStream, RecvStream)) {
    loop {
        // These are 32 buffers, for reading approximately 32kB at once
        #[rustfmt::skip]
        let mut bufs = [
            Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
            Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
            Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
            Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
            Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
            Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
            Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
            Bytes::new(), Bytes::new(), Bytes::new(), Bytes::new(),
        ];

        match recv.read_chunks(&mut bufs).await.expect("read chunks") {
            Some(n) => {
                send.write_all_chunks(&mut bufs[..n])
                    .await
                    .expect("write chunks");
            }
            None => break,
        }
    }

    let _ = send.finish();
}

fn gen_data(size: usize, seed: u64) -> Vec<u8> {
    let mut rng: StdRng = SeedableRng::seed_from_u64(seed);
    let mut buf = vec![0; size];
    rng.fill_bytes(&mut buf);
    buf
}

fn subscribe() -> tracing::subscriber::DefaultGuard {
    let sub = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(|| TestWriter)
        .finish();
    tracing::subscriber::set_default(sub)
}

struct TestWriter;

impl std::io::Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        print!(
            "{}",
            str::from_utf8(buf).expect("tried to log invalid UTF-8")
        );
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}

fn rt_basic() -> Runtime {
    Builder::new_current_thread().enable_all().build().unwrap()
}

fn rt_threaded() -> Runtime {
    Builder::new_multi_thread().enable_all().build().unwrap()
}

#[tokio::test]
async fn rebind_recv() {
    let _guard = subscribe();

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    let cert = CertificateDer::from(cert.cert);

    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert.clone()).unwrap();

    let mut client = Endpoint::client(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).unwrap();
    let mut client_config = ClientConfig::with_root_certificates(Arc::new(roots)).unwrap();
    client_config.transport_config(Arc::new({
        let mut cfg = TransportConfig::default();
        cfg.max_concurrent_uni_streams(1u32.into());
        cfg
    }));
    client.set_default_client_config(client_config);

    let server_config =
        crate::ServerConfig::with_single_cert(vec![cert.clone()], key.into()).unwrap();
    let server = {
        let _guard = tracing::error_span!("server").entered();
        Endpoint::server(
            server_config,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        )
        .unwrap()
    };
    let server_addr = server.local_addr().unwrap();

    const MSG: &[u8; 5] = b"hello";

    let write_send = Arc::new(tokio::sync::Notify::new());
    let write_recv = write_send.clone();
    let connected_send = Arc::new(tokio::sync::Notify::new());
    let connected_recv = connected_send.clone();
    let server = tokio::spawn(async move {
        let connection = server.accept().await.unwrap().await.unwrap();
        info!("got conn");
        connected_send.notify_one();
        write_recv.notified().await;
        let mut stream = connection.open_uni().await.unwrap();
        stream.write_all(MSG).await.unwrap();
        stream.finish().unwrap();
        // Wait for the stream to be closed, one way or another.
        _ = stream.stopped().await;
    });

    let connection = {
        let _guard = tracing::error_span!("client").entered();
        client
            .connect(server_addr, "localhost")
            .unwrap()
            .await
            .unwrap()
    };
    info!("connected");
    connected_recv.notified().await;
    client
        .rebind(UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).unwrap())
        .unwrap();
    info!("rebound");
    write_send.notify_one();
    let mut stream = connection.accept_uni().await.unwrap();
    assert_eq!(stream.read_to_end(MSG.len()).await.unwrap(), MSG);
    server.await.unwrap();
}

#[tokio::test]
async fn stream_id_flow_control() {
    let _guard = subscribe();
    let mut cfg = TransportConfig::default();
    cfg.max_concurrent_uni_streams(1u32.into());
    let endpoint = endpoint_with_config(cfg);

    let (client, server) = tokio::join!(
        endpoint
            .connect(endpoint.local_addr().unwrap(), "localhost")
            .unwrap(),
        async { endpoint.accept().await.unwrap().await }
    );
    let client = client.unwrap();
    let server = server.unwrap();

    // If `open_uni` doesn't get unblocked when the previous stream is dropped, this will time out.
    tokio::join!(
        async {
            client.open_uni().await.unwrap();
        },
        async {
            client.open_uni().await.unwrap();
        },
        async {
            client.open_uni().await.unwrap();
        },
        async {
            server.accept_uni().await.unwrap();
            server.accept_uni().await.unwrap();
        }
    );
}

#[tokio::test]
async fn two_datagram_readers() {
    let _guard = subscribe();
    let endpoint = endpoint();

    let (client, server) = tokio::join!(
        endpoint
            .connect(endpoint.local_addr().unwrap(), "localhost")
            .unwrap(),
        async { endpoint.accept().await.unwrap().await }
    );
    let client = client.unwrap();
    let server = server.unwrap();

    let done = tokio::sync::Notify::new();
    let (a, b, ()) = tokio::join!(
        async {
            let x = client.read_datagram().await.unwrap();
            done.notify_waiters();
            x
        },
        async {
            let x = client.read_datagram().await.unwrap();
            done.notify_waiters();
            x
        },
        async {
            server.send_datagram(b"one"[..].into()).unwrap();
            done.notified().await;
            server.send_datagram_wait(b"two"[..].into()).await.unwrap();
        }
    );
    assert!(*a == *b"one" || *b == *b"one");
    assert!(*a == *b"two" || *b == *b"two");
}

#[tokio::test]
async fn multiple_conns_with_zero_length_cids() {
    let _guard = subscribe();
    let mut factory = EndpointFactory::new();
    factory
        .endpoint_config
        .cid_generator(|| Box::new(RandomConnectionIdGenerator::new(0)));
    let server = {
        let _guard = error_span!("server").entered();
        factory.endpoint()
    };
    let server_addr = server.local_addr().unwrap();

    let client1 = {
        let _guard = error_span!("client1").entered();
        factory.endpoint()
    };
    let client2 = {
        let _guard = error_span!("client2").entered();
        factory.endpoint()
    };

    let client1 = async move {
        let conn = client1
            .connect(server_addr, "localhost")
            .unwrap()
            .await
            .unwrap();
        conn.closed().await;
    }
    .instrument(error_span!("client1"));
    let client2 = async move {
        let conn = client2
            .connect(server_addr, "localhost")
            .unwrap()
            .await
            .unwrap();
        conn.closed().await;
    }
    .instrument(error_span!("client2"));
    let server = async move {
        let client1 = server.accept().await.unwrap().await.unwrap();
        let client2 = server.accept().await.unwrap().await.unwrap();
        // Both connections are now concurrently live.
        client1.close(42u32.into(), &[]);
        client2.close(42u32.into(), &[]);
    }
    .instrument(error_span!("server"));
    tokio::join!(client1, client2, server);
}

#[tokio::test]
async fn stream_stopped() {
    let _guard = subscribe();
    let factory = EndpointFactory::new();
    let server = {
        let _guard = error_span!("server").entered();
        factory.endpoint()
    };
    let server_addr = server.local_addr().unwrap();

    let client = {
        let _guard = error_span!("client1").entered();
        factory.endpoint()
    };

    let client = async move {
        let conn = client
            .connect(server_addr, "localhost")
            .unwrap()
            .await
            .unwrap();
        let mut stream = conn.open_uni().await.unwrap();
        let stopped1 = stream.stopped();
        let stopped2 = stream.stopped();
        let stopped3 = stream.stopped();

        stream.write_all(b"hi").await.unwrap();
        // spawn one of the futures into a task
        let stopped1 = tokio::task::spawn(stopped1);
        // verify that both futures resolved
        let (stopped1, stopped2) = tokio::join!(stopped1, stopped2);
        assert!(matches!(stopped1, Ok(Ok(Some(val))) if val == 42u32.into()));
        assert!(matches!(stopped2, Ok(Some(val)) if val == 42u32.into()));
        // drop the stream
        drop(stream);
        // verify that a future also resolves after dropping the stream
        let stopped3 = stopped3.await;
        assert_eq!(stopped3, Ok(Some(42u32.into())));
    };
    let client =
        tokio::time::timeout(Duration::from_millis(100), client).instrument(error_span!("client"));
    let server = async move {
        let conn = server.accept().await.unwrap().await.unwrap();
        let mut stream = conn.accept_uni().await.unwrap();
        let mut buf = [0u8; 2];
        stream.read_exact(&mut buf).await.unwrap();
        stream.stop(42u32.into()).unwrap();
        conn
    }
    .instrument(error_span!("server"));
    let (client, conn) = tokio::join!(client, server);
    client.expect("timeout");
    drop(conn);
}

#[tokio::test]
async fn stream_stopped_2() {
    let _guard = subscribe();
    let endpoint = endpoint();

    let (conn, _server_conn) = tokio::try_join!(
        endpoint
            .connect(endpoint.local_addr().unwrap(), "localhost")
            .unwrap(),
        async { endpoint.accept().await.unwrap().await }
    )
    .unwrap();
    let send_stream = conn.open_uni().await.unwrap();
    let stopped = tokio::time::timeout(Duration::from_millis(100), send_stream.stopped())
        .instrument(error_span!("stopped"));
    tokio::pin!(stopped);
    // poll the future once so that the waker is registered.
    tokio::select! {
        biased;
        _x = &mut stopped => {},
        _x = std::future::ready(()) => {}
    }
    // drop the send stream
    drop(send_stream);
    // make sure the stopped future still resolves
    let res = stopped.await;
    assert_eq!(res, Ok(Ok(None)));
}
