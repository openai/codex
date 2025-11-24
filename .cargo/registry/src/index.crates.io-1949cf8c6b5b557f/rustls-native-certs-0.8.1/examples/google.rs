use std::io::{stdout, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

fn main() {
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().expect("could not load platform certs") {
        roots.add(cert).unwrap();
    }

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    let mut conn =
        rustls::ClientConnection::new(Arc::new(config), "google.com".try_into().unwrap()).unwrap();
    let mut sock = TcpStream::connect("google.com:443").expect("cannot connect");
    let mut tls = rustls::Stream::new(&mut conn, &mut sock);
    tls.write_all(
        concat!(
            "GET / HTTP/1.1\r\n",
            "Host: google.com\r\n",
            "Connection: close\r\n",
            "Accept-Encoding: identity\r\n",
            "\r\n"
        )
        .as_bytes(),
    )
    .expect("write failed");
    let ciphersuite = tls
        .conn
        .negotiated_cipher_suite()
        .expect("tls handshake failed");
    writeln!(
        &mut std::io::stderr(),
        "Current ciphersuite: {:?}",
        ciphersuite.suite()
    )
    .unwrap();
    let mut plaintext = Vec::new();
    tls.read_to_end(&mut plaintext).unwrap();
    stdout().write_all(&plaintext).unwrap();
}
