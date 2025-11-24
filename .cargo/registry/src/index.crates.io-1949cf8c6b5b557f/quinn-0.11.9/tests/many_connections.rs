#![cfg(any(feature = "rustls-aws-lc-rs", feature = "rustls-ring"))]
use std::{
    convert::TryInto,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use crc::Crc;
use quinn::{ConnectionError, ReadError, StoppedError, TransportConfig, WriteError};
use rand::{self, RngCore};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use tokio::runtime::Builder;

struct Shared {
    errors: Vec<ConnectionError>,
}

#[test]
#[ignore]
fn connect_n_nodes_to_1_and_send_1mb_data() {
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    )
    .unwrap();

    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let _guard = runtime.enter();
    let shared = Arc::new(Mutex::new(Shared { errors: vec![] }));

    let (cfg, listener_cert) = configure_listener();
    let endpoint =
        quinn::Endpoint::server(cfg, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).unwrap();
    let listener_addr = endpoint.local_addr().unwrap();

    let expected_messages = 50;

    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
    let shared2 = shared.clone();
    let endpoint2 = endpoint.clone();
    let read_incoming_data = async move {
        for _ in 0..expected_messages {
            let conn = endpoint2.accept().await.unwrap().await.unwrap();

            let shared = shared2.clone();
            let task = async move {
                while let Ok(stream) = conn.accept_uni().await {
                    read_from_peer(stream).await?;
                    conn.close(0u32.into(), &[]);
                }
                Ok(())
            };
            tokio::spawn(async move {
                if let Err(e) = task.await {
                    shared.lock().unwrap().errors.push(e);
                }
            });
        }
    };
    runtime.spawn(read_incoming_data);

    let client_cfg = configure_connector(listener_cert);

    for _ in 0..expected_messages {
        let data = random_data_with_hash(1024 * 1024, &crc);
        let shared = shared.clone();
        let connecting = endpoint
            .connect_with(client_cfg.clone(), listener_addr, "localhost")
            .unwrap();
        let task = async move {
            let conn = connecting.await.map_err(WriteError::ConnectionLost)?;
            write_to_peer(conn, data).await?;
            Ok(())
        };
        runtime.spawn(async move {
            if let Err(e) = task.await {
                use quinn::ConnectionError::*;
                match e {
                    WriteError::ConnectionLost(ApplicationClosed { .. })
                    | WriteError::ConnectionLost(Reset) => {}
                    WriteError::ConnectionLost(e) => shared.lock().unwrap().errors.push(e),
                    _ => panic!("unexpected write error"),
                }
            }
        });
    }

    runtime.block_on(endpoint.wait_idle());
    let shared = shared.lock().unwrap();
    if !shared.errors.is_empty() {
        panic!("some connections failed: {:?}", shared.errors);
    }
}

async fn read_from_peer(mut stream: quinn::RecvStream) -> Result<(), quinn::ConnectionError> {
    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
    match stream.read_to_end(1024 * 1024 * 5).await {
        Ok(data) => {
            assert!(hash_correct(&data, &crc));
            Ok(())
        }
        Err(e) => {
            use ReadError::*;
            use quinn::ReadToEndError::*;
            match e {
                TooLong | Read(ClosedStream) | Read(ZeroRttRejected) | Read(IllegalOrderedRead) => {
                    unreachable!()
                }
                Read(Reset(error_code)) => panic!("unexpected stream reset: {error_code}"),
                Read(ConnectionLost(e)) => Err(e),
            }
        }
    }
}

async fn write_to_peer(conn: quinn::Connection, data: Vec<u8>) -> Result<(), WriteError> {
    let mut s = conn.open_uni().await.map_err(WriteError::ConnectionLost)?;
    s.write_all(&data).await?;
    s.finish().unwrap();
    // Wait for the stream to be fully received
    match s.stopped().await {
        Ok(_) => Ok(()),
        Err(StoppedError::ConnectionLost(ConnectionError::ApplicationClosed { .. })) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Builds client configuration. Trusts given node certificate.
fn configure_connector(node_cert: CertificateDer<'static>) -> quinn::ClientConfig {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(node_cert).unwrap();

    let mut transport_config = TransportConfig::default();
    transport_config.max_idle_timeout(Some(Duration::from_secs(20).try_into().unwrap()));

    let mut peer_cfg = quinn::ClientConfig::with_root_certificates(Arc::new(roots)).unwrap();
    peer_cfg.transport_config(Arc::new(transport_config));
    peer_cfg
}

/// Builds listener configuration along with its certificate.
fn configure_listener() -> (quinn::ServerConfig, CertificateDer<'static>) {
    let (our_cert, our_priv_key) = gen_cert();
    let mut our_cfg =
        quinn::ServerConfig::with_single_cert(vec![our_cert.clone()], our_priv_key.into()).unwrap();

    let transport_config = Arc::get_mut(&mut our_cfg.transport).unwrap();
    transport_config.max_idle_timeout(Some(Duration::from_secs(20).try_into().unwrap()));

    (our_cfg, our_cert)
}

fn gen_cert() -> (CertificateDer<'static>, PrivatePkcs8KeyDer<'static>) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    (
        cert.cert.into(),
        PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()),
    )
}

/// Constructs a buffer with random bytes of given size prefixed with a hash of this data.
fn random_data_with_hash(size: usize, crc: &Crc<u32>) -> Vec<u8> {
    let mut data = random_vec(size + 4);
    let hash = crc.checksum(&data[4..]);
    // write hash in big endian
    data[0] = (hash >> 24) as u8;
    data[1] = ((hash >> 16) & 0xff) as u8;
    data[2] = ((hash >> 8) & 0xff) as u8;
    data[3] = (hash & 0xff) as u8;
    data
}

/// Checks if given data buffer hash is correct. Hash itself is a 4 byte prefix in the data.
fn hash_correct(data: &[u8], crc: &Crc<u32>) -> bool {
    let encoded_hash = ((data[0] as u32) << 24)
        | ((data[1] as u32) << 16)
        | ((data[2] as u32) << 8)
        | data[3] as u32;
    let actual_hash = crc.checksum(&data[4..]);
    encoded_hash == actual_hash
}

fn random_vec(size: usize) -> Vec<u8> {
    let mut ret = vec![0; size];
    rand::rng().fill_bytes(&mut ret[..]);
    ret
}
