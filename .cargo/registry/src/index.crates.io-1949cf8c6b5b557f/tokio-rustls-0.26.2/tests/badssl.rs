use std::io;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

async fn get(
    config: Arc<ClientConfig>,
    domain: &str,
    port: u16,
    vectored: bool,
) -> io::Result<(TlsStream<TcpStream>, String)> {
    let connector = TlsConnector::from(config);
    let input = format!("GET / HTTP/1.0\r\nHost: {}\r\n\r\n", domain);

    let addr = (domain, port).to_socket_addrs()?.next().unwrap();
    let domain = ServerName::try_from(domain).unwrap().to_owned();
    let mut buf = Vec::new();

    let stream = TcpStream::connect(&addr).await?;
    let mut stream = connector.connect(domain, stream).await?;
    utils::write(&mut stream, input.as_bytes(), vectored).await?;
    stream.flush().await?;
    stream.read_to_end(&mut buf).await?;

    Ok((stream, String::from_utf8(buf).unwrap()))
}

#[tokio::test]
async fn test_tls12() -> io::Result<()> {
    test_tls12_impl(false).await
}

#[tokio::test]
async fn test_tls12_vectored() -> io::Result<()> {
    test_tls12_impl(true).await
}

async fn test_tls12_impl(vectored: bool) -> io::Result<()> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS12])
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let config = Arc::new(config);
    let domain = "tls-v1-2.badssl.com";

    let (_, output) = get(config.clone(), domain, 1012, vectored).await?;
    assert!(
        output.contains("<title>tls-v1-2.badssl.com</title>"),
        "failed badssl test, output: {}",
        output
    );

    Ok(())
}

#[ignore]
#[should_panic]
#[test]
fn test_tls13() {
    unimplemented!("todo https://github.com/chromium/badssl.com/pull/373");
}

#[tokio::test]
async fn test_modern() -> io::Result<()> {
    test_modern_impl(false).await
}

#[tokio::test]
async fn test_modern_vectored() -> io::Result<()> {
    test_modern_impl(true).await
}

async fn test_modern_impl(vectored: bool) -> io::Result<()> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let config = Arc::new(config);
    let domain = "mozilla-modern.badssl.com";

    let (_, output) = get(config.clone(), domain, 443, vectored).await?;
    assert!(
        output.contains("<title>mozilla-modern.badssl.com</title>"),
        "failed badssl test, output: {}",
        output
    );

    Ok(())
}

// Include `utils` module
include!("utils.rs");
