use std::io::{Cursor, ErrorKind};
use std::net::SocketAddr;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;
use std::{io, thread};

use futures_util::future::TryFutureExt;
use lazy_static::lazy_static;
use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use tokio::io::{copy, split, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::{runtime, time};
use tokio_rustls::{LazyConfigAcceptor, TlsAcceptor, TlsConnector};

lazy_static! {
    static ref TEST_SERVER: SocketAddr = {
        let (config, _) = utils::make_configs();
        let acceptor = TlsAcceptor::from(Arc::new(config));

        let (send, recv) = channel();

        thread::spawn(move || {
            let runtime = runtime::Builder::new_current_thread()
                .enable_io()
                .build()
                .unwrap();
            let runtime = Arc::new(runtime);
            let runtime2 = runtime.clone();

            let done = async move {
                let addr = SocketAddr::from(([127, 0, 0, 1], 0));
                let listener = TcpListener::bind(&addr).await?;

                send.send(listener.local_addr()?).unwrap();

                loop {
                    let (stream, _) = listener.accept().await?;

                    let acceptor = acceptor.clone();
                    let fut = async move {
                        let stream = acceptor.accept(stream).await?;

                        let (mut reader, mut writer) = split(stream);
                        copy(&mut reader, &mut writer).await?;

                        Ok(()) as io::Result<()>
                    }
                    .unwrap_or_else(|err| eprintln!("server: {:?}", err));

                    runtime2.spawn(fut);
                }
            }
            .unwrap_or_else(|err: io::Error| eprintln!("server: {:?}", err));

            runtime.block_on(done);
        });

        recv.recv().unwrap()
    };
}

async fn start_client(
    addr: SocketAddr,
    domain: &str,
    config: Arc<ClientConfig>,
    use_buf_read: bool,
) -> io::Result<()> {
    const FILE: &[u8] = include_bytes!("../README.md");

    let domain = ServerName::try_from(domain).unwrap().to_owned();
    let config = TlsConnector::from(config);
    let mut buf = vec![0; FILE.len()];

    let stream = TcpStream::connect(&addr).await?;
    let mut stream = config.connect(domain, stream).await?;
    stream.write_all(FILE).await?;
    stream.flush().await?;
    if use_buf_read {
        tokio::io::copy_buf(
            &mut (&mut stream).take(FILE.len() as u64),
            &mut Cursor::new(&mut buf),
        )
        .await?;
    } else {
        stream.read_exact(&mut buf).await?;
    }

    assert_eq!(buf, FILE);

    Ok(())
}

async fn pass_impl(use_buf_read: bool) -> io::Result<()> {
    // TODO: not sure how to resolve this right now but since
    // TcpStream::bind now returns a future it creates a race
    // condition until its ready sometimes.
    use std::time::*;
    tokio::time::sleep(Duration::from_secs(1)).await;

    let (_, config) = utils::make_configs();
    let config = Arc::new(config);

    start_client(
        *TEST_SERVER,
        utils::TEST_SERVER_DOMAIN,
        config,
        use_buf_read,
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn pass() -> io::Result<()> {
    pass_impl(false).await
}

#[tokio::test]
async fn pass_buf_read() -> io::Result<()> {
    pass_impl(true).await
}

#[tokio::test]
async fn fail() -> io::Result<()> {
    let (_, config) = utils::make_configs();
    let config = Arc::new(config);

    assert_ne!(utils::TEST_SERVER_DOMAIN, "google.com");
    let ret = start_client(*TEST_SERVER, "google.com", config, false).await;
    assert!(ret.is_err());

    Ok(())
}

#[tokio::test]
async fn test_lazy_config_acceptor() -> io::Result<()> {
    let (sconfig, cconfig) = utils::make_configs();

    let (cstream, sstream) = tokio::io::duplex(1200);
    let domain = ServerName::try_from("foobar.com").unwrap().to_owned();
    tokio::spawn(async move {
        let connector = crate::TlsConnector::from(Arc::new(cconfig));
        let mut client = connector.connect(domain, cstream).await.unwrap();
        client.write_all(b"hello, world!").await.unwrap();

        let mut buf = Vec::new();
        client.read_to_end(&mut buf).await.unwrap();
    });

    let acceptor = LazyConfigAcceptor::new(rustls::server::Acceptor::default(), sstream);
    let start = acceptor.await.unwrap();
    let ch = start.client_hello();

    assert_eq!(ch.server_name(), Some("foobar.com"));
    assert_eq!(
        ch.alpn()
            .map(|protos| protos.collect::<Vec<_>>())
            .unwrap_or_default(),
        Vec::<&[u8]>::new()
    );

    let mut stream = start.into_stream(Arc::new(sconfig)).await.unwrap();
    let mut buf = [0; 13];
    stream.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf[..], b"hello, world!");

    stream.write_all(b"bye").await.unwrap();
    Ok(())
}

// This test is a follow-up from https://github.com/tokio-rs/tls/issues/85
#[tokio::test]
async fn lazy_config_acceptor_eof() {
    let buf = Cursor::new(Vec::new());
    let acceptor = LazyConfigAcceptor::new(rustls::server::Acceptor::default(), buf);

    let accept_result = match time::timeout(Duration::from_secs(3), acceptor).await {
        Ok(res) => res,
        Err(_elapsed) => panic!("timeout"),
    };

    match accept_result {
        Ok(_) => panic!("accepted a connection from zero bytes of data"),
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => {}
        Err(e) => panic!("unexpected error: {:?}", e),
    }
}

#[tokio::test]
async fn lazy_config_acceptor_take_io() -> Result<(), rustls::Error> {
    let (mut cstream, sstream) = tokio::io::duplex(1200);

    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        cstream.write_all(b"hello, world!").await.unwrap();

        let mut buf = Vec::new();
        cstream.read_to_end(&mut buf).await.unwrap();
        tx.send(buf).unwrap();
    });

    let acceptor = LazyConfigAcceptor::new(rustls::server::Acceptor::default(), sstream);
    futures_util::pin_mut!(acceptor);
    if (acceptor.as_mut().await).is_ok() {
        panic!("Expected Err(err)");
    }

    let server_msg = b"message from server";
    let fatal_alert_decode_error = b"\x15\x03\x03\x00\x02\x02\x32";

    let some_io = acceptor.take_io();
    assert!(some_io.is_some(), "Expected Some(io)");
    some_io.unwrap().write_all(server_msg).await.unwrap();

    assert_eq!(
        rx.await.unwrap(),
        [&fatal_alert_decode_error[..], &server_msg[..]].concat()
    );

    assert!(
        acceptor.take_io().is_none(),
        "Should not be able to take twice"
    );
    Ok(())
}

#[tokio::test]
async fn acceptor_alert() {
    let (sconfig, _) = utils::make_configs();
    // this is the client hello from https://tls12.xargs.org/#client-hello/annotated with the minor
    // version byte changed
    let bad_hello = [
        0x16, 0x03, 0x01, 0x00, 0xa5, 0x01, 0x00, 0x00, 0xa1, 0x03, 0x01, 0x00, 0x01, 0x02, 0x03,
        0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12,
        0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x00, 0x00,
        0x20, 0xcc, 0xa8, 0xcc, 0xa9, 0xc0, 0x2f, 0xc0, 0x30, 0xc0, 0x2b, 0xc0, 0x2c, 0xc0, 0x13,
        0xc0, 0x09, 0xc0, 0x14, 0xc0, 0x0a, 0x00, 0x9c, 0x00, 0x9d, 0x00, 0x2f, 0x00, 0x35, 0xc0,
        0x12, 0x00, 0x0a, 0x01, 0x00, 0x00, 0x58, 0x00, 0x00, 0x00, 0x18, 0x00, 0x16, 0x00, 0x00,
        0x13, 0x65, 0x78, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x2e, 0x75, 0x6c, 0x66, 0x68, 0x65, 0x69,
        0x6d, 0x2e, 0x6e, 0x65, 0x74, 0x00, 0x05, 0x00, 0x05, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x0a, 0x00, 0x0a, 0x00, 0x08, 0x00, 0x1d, 0x00, 0x17, 0x00, 0x18, 0x00, 0x19, 0x00, 0x0b,
        0x00, 0x02, 0x01, 0x00, 0x00, 0x0d, 0x00, 0x12, 0x00, 0x10, 0x04, 0x01, 0x04, 0x03, 0x05,
        0x01, 0x05, 0x03, 0x06, 0x01, 0x06, 0x03, 0x02, 0x01, 0x02, 0x03, 0xff, 0x01, 0x00, 0x01,
        0x00, 0x00, 0x12, 0x00, 0x00,
    ];

    // Intentionally small so that we have to call alert.write several times
    let (mut cstream, sstream) = tokio::io::duplex(2);

    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        cstream.write_all(&bad_hello).await.unwrap();
        let mut buf = Vec::new();
        cstream.read_to_end(&mut buf).await.unwrap();
        tx.send(buf).unwrap();
    });

    let accept = LazyConfigAcceptor::new(rustls::server::Acceptor::default(), sstream);

    let Ok(Ok(start_handshake)) = time::timeout(Duration::from_secs(3), accept).await else {
        panic!("timeout");
    };

    let err = start_handshake
        .into_stream(Arc::new(sconfig))
        .await
        .unwrap_err();

    assert_eq!(err.to_string(), "peer is incompatible: Tls12NotOffered");

    let Ok(Ok(received)) = time::timeout(Duration::from_secs(3), rx).await else {
        panic!("failed to receive");
    };

    assert_eq!(received, [0x15, 0x03, 0x03, 0x00, 0x02, 0x02, 0x46]);
}

#[tokio::test]
async fn lazy_config_acceptor_alert() {
    // Intentionally small so that we have to call alert.write several times
    let (mut cstream, sstream) = tokio::io::duplex(2);

    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        // This is write instead of write_all because of the short duplex size, which is necessarily
        // symmetrical. We never finish writing because the LazyConfigAcceptor returns an error
        let _ = cstream.write(b"not tls").await;
        let mut buf = Vec::new();
        cstream.read_to_end(&mut buf).await.unwrap();
        tx.send(buf).unwrap();
    });

    let acceptor = LazyConfigAcceptor::new(rustls::server::Acceptor::default(), sstream);

    let Ok(accept_result) = time::timeout(Duration::from_secs(3), acceptor).await else {
        panic!("timeout");
    };

    assert!(accept_result.is_err());

    let Ok(Ok(received)) = time::timeout(Duration::from_secs(3), rx).await else {
        panic!("failed to receive");
    };

    let fatal_alert_decode_error = b"\x15\x03\x03\x00\x02\x02\x32";
    assert_eq!(received, fatal_alert_decode_error)
}

// Include `utils` module
include!("utils.rs");
