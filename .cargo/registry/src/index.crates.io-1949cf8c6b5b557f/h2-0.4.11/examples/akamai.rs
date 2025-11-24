use h2::client;
use http::{Method, Request};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use tokio_rustls::rustls::{pki_types::ServerName, RootCertStore};

use std::error::Error;
use std::net::ToSocketAddrs;

const ALPN_H2: &str = "h2";

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let _ = env_logger::try_init();

    let tls_client_config = std::sync::Arc::new({
        let root_store = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let mut c = tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        c.alpn_protocols.push(ALPN_H2.as_bytes().to_owned());
        c
    });

    // Sync DNS resolution.
    let addr = "http2.akamai.com:443"
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();

    println!("ADDR: {:?}", addr);

    let tcp = TcpStream::connect(&addr).await?;
    let dns_name = ServerName::try_from("http2.akamai.com").unwrap();
    let connector = TlsConnector::from(tls_client_config);
    let res = connector.connect(dns_name, tcp).await;
    let tls = res.unwrap();
    {
        let (_, session) = tls.get_ref();
        let negotiated_protocol = session.alpn_protocol();
        assert_eq!(Some(ALPN_H2.as_bytes()), negotiated_protocol);
    }

    println!("Starting client handshake");
    let (mut client, h2) = client::handshake(tls).await?;

    println!("building request");
    let request = Request::builder()
        .method(Method::GET)
        .uri("https://http2.akamai.com/")
        .body(())
        .unwrap();

    println!("sending request");
    let (response, other) = client.send_request(request, true).unwrap();

    tokio::spawn(async move {
        if let Err(e) = h2.await {
            println!("GOT ERR={:?}", e);
        }
    });

    println!("waiting on response : {:?}", other);
    let (_, mut body) = response.await?.into_parts();
    println!("processing body");
    while let Some(chunk) = body.data().await {
        println!("RX: {:?}", chunk?);
    }
    Ok(())
}
