use futures::join;
use lazy_static::lazy_static;
use native_tls::{Certificate, Identity};
use std::{fs, io::Error, path::PathBuf, process::Command};
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_native_tls::{TlsAcceptor, TlsConnector};

lazy_static! {
    static ref CERT_DIR: PathBuf = {
        if cfg!(unix) {
            let dir = tempfile::TempDir::new().unwrap();
            let path = dir.path().to_str().unwrap();

            Command::new("sh")
                .arg("-c")
                .arg(format!("./scripts/generate-certificate.sh {}", path))
                .output()
                .expect("failed to execute process");

            dir.into_path()
        } else {
            PathBuf::from("tests")
        }
    };
}

#[tokio::test]
async fn client_to_server() {
    let srv = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = srv.local_addr().unwrap();

    let (server_tls, client_tls) = context();

    // Create a future to accept one socket, connect the ssl stream, and then
    // read all the data from it.
    let server = async move {
        let (socket, _) = srv.accept().await.unwrap();
        let mut socket = server_tls.accept(socket).await.unwrap();

        // Verify access to all of the nested inner streams (e.g. so that peer
        // certificates can be accessed). This is just a compile check.
        let native_tls_stream: &native_tls::TlsStream<_> = socket.get_ref();
        let _peer_cert = native_tls_stream.peer_certificate().unwrap();
        let allow_std_stream: &tokio_native_tls::AllowStd<_> = native_tls_stream.get_ref();
        let _tokio_tcp_stream: &tokio::net::TcpStream = allow_std_stream.get_ref();

        let mut data = Vec::new();
        socket.read_to_end(&mut data).await.unwrap();
        data
    };

    // Create a future to connect to our server, connect the ssl stream, and
    // then write a bunch of data to it.
    let client = async move {
        let socket = TcpStream::connect(&addr).await.unwrap();
        let socket = client_tls.connect("foobar.com", socket).await.unwrap();
        copy_data(socket).await
    };

    // Finally, run everything!
    let (data, _) = join!(server, client);
    // assert_eq!(amt, AMT);
    assert!(data == vec![9; AMT]);
}

#[tokio::test]
async fn server_to_client() {
    // Create a server listening on a port, then figure out what that port is
    let srv = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = srv.local_addr().unwrap();

    let (server_tls, client_tls) = context();

    let server = async move {
        let (socket, _) = srv.accept().await.unwrap();
        let socket = server_tls.accept(socket).await.unwrap();
        copy_data(socket).await
    };

    let client = async move {
        let socket = TcpStream::connect(&addr).await.unwrap();
        let mut socket = client_tls.connect("foobar.com", socket).await.unwrap();
        let mut data = Vec::new();
        socket.read_to_end(&mut data).await.unwrap();
        data
    };

    // Finally, run everything!
    let (_, data) = join!(server, client);
    assert!(data == vec![9; AMT]);
}

#[tokio::test]
async fn one_byte_at_a_time() {
    const AMT: usize = 1024;

    let srv = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = srv.local_addr().unwrap();

    let (server_tls, client_tls) = context();

    let server = async move {
        let (socket, _) = srv.accept().await.unwrap();
        let mut socket = server_tls.accept(socket).await.unwrap();
        let mut amt = 0;
        for b in std::iter::repeat(9).take(AMT) {
            let data = [b as u8];
            socket.write_all(&data).await.unwrap();
            amt += 1;
        }
        amt
    };

    let client = async move {
        let socket = TcpStream::connect(&addr).await.unwrap();
        let mut socket = client_tls.connect("foobar.com", socket).await.unwrap();
        let mut data = Vec::new();
        loop {
            let mut buf = [0; 1];
            match socket.read_exact(&mut buf).await {
                Ok(_) => data.extend_from_slice(&buf),
                Err(ref err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(err) => panic!("{}", err),
            }
        }
        data
    };

    let (amt, data) = join!(server, client);
    assert_eq!(amt, AMT);
    assert!(data == vec![9; AMT as usize]);
}

fn context() -> (TlsAcceptor, TlsConnector) {
    let pkcs12 = fs::read(CERT_DIR.join("identity.p12")).unwrap();
    let der = fs::read(CERT_DIR.join("root-ca.der")).unwrap();

    let identity = Identity::from_pkcs12(&pkcs12, "mypass").unwrap();
    let acceptor = native_tls::TlsAcceptor::builder(identity).build().unwrap();

    let cert = Certificate::from_der(&der).unwrap();
    let connector = native_tls::TlsConnector::builder()
        .add_root_certificate(cert)
        .build()
        .unwrap();

    (acceptor.into(), connector.into())
}

const AMT: usize = 128 * 1024;

async fn copy_data<W: AsyncWrite + Unpin>(mut w: W) -> Result<usize, Error> {
    let mut data = vec![9; AMT as usize];
    let mut amt = 0;
    while !data.is_empty() {
        let written = w.write(&data).await?;
        if written <= data.len() {
            amt += written;
            data.resize(data.len() - written, 0);
        } else {
            w.write_all(&data).await?;
            amt += data.len();
            break;
        }

        println!("remaining: {}", data.len());
    }
    Ok(amt)
}
