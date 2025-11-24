use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::string::String;
use std::thread;

use super::*;

macro_rules! p {
    ($e:expr) => {
        match $e {
            Ok(r) => r,
            Err(e) => panic!("{:?}", e),
        }
    };
}

#[test]
fn connect_google() {
    let builder = p!(TlsConnector::new());
    let s = p!(TcpStream::connect("google.com:443"));
    let mut socket = p!(builder.connect("google.com", s));

    p!(socket.write_all(b"GET / HTTP/1.0\r\n\r\n"));
    let mut result = vec![];
    p!(socket.read_to_end(&mut result));

    println!("{}", String::from_utf8_lossy(&result));
    assert!(result.starts_with(b"HTTP/1.0"));
    assert!(result.ends_with(b"</HTML>\r\n") || result.ends_with(b"</html>"));
}

#[test]
fn connect_bad_hostname() {
    let builder = p!(TlsConnector::new());
    let s = p!(TcpStream::connect("google.com:443"));
    builder.connect("goggle.com", s).unwrap_err();
}

#[test]
fn connect_bad_hostname_ignored() {
    let builder = p!(TlsConnector::builder()
        .danger_accept_invalid_hostnames(true)
        .build());
    let s = p!(TcpStream::connect("google.com:443"));
    builder.connect("goggle.com", s).unwrap();
}

#[test]
fn connect_no_root_certs() {
    let builder = p!(TlsConnector::builder().disable_built_in_roots(true).build());
    let s = p!(TcpStream::connect("google.com:443"));
    assert!(builder.connect("google.com", s).is_err());
}

#[test]
fn server_no_root_certs() {
    let keys = test_cert_gen::keys();

    let identity = p!(Identity::from_pkcs12(
        &keys.server.cert_and_key_pkcs12.pkcs12.0,
        &keys.server.cert_and_key_pkcs12.password
    ));
    let builder = p!(TlsAcceptor::new(identity));

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port = p!(listener.local_addr()).port();

    let j = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        let mut socket = p!(builder.accept(socket));

        let mut buf = [0; 5];
        p!(socket.read_exact(&mut buf));
        assert_eq!(&buf, b"hello");

        p!(socket.write_all(b"world"));
    });

    let root_ca = Certificate::from_der(keys.client.ca.get_der()).unwrap();

    let socket = p!(TcpStream::connect(("localhost", port)));
    let builder = p!(TlsConnector::builder()
        .disable_built_in_roots(true)
        .add_root_certificate(root_ca)
        .build());
    let mut socket = p!(builder.connect("localhost", socket));

    p!(socket.write_all(b"hello"));
    let mut buf = vec![];
    p!(socket.read_to_end(&mut buf));
    assert_eq!(buf, b"world");

    p!(j.join());
}

#[test]
fn server() {
    let keys = test_cert_gen::keys();

    let identity = p!(Identity::from_pkcs12(
        &keys.server.cert_and_key_pkcs12.pkcs12.0,
        &keys.server.cert_and_key_pkcs12.password
    ));
    let builder = p!(TlsAcceptor::new(identity));

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port = p!(listener.local_addr()).port();

    let j = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        let mut socket = p!(builder.accept(socket));

        let mut buf = [0; 5];
        p!(socket.read_exact(&mut buf));
        assert_eq!(&buf, b"hello");

        p!(socket.write_all(b"world"));
    });

    let root_ca = Certificate::from_der(keys.client.ca.get_der()).unwrap();

    let socket = p!(TcpStream::connect(("localhost", port)));
    let builder = p!(TlsConnector::builder()
        .add_root_certificate(root_ca)
        .build());
    let mut socket = p!(builder.connect("localhost", socket));

    p!(socket.write_all(b"hello"));
    let mut buf = vec![];
    p!(socket.read_to_end(&mut buf));
    assert_eq!(buf, b"world");

    p!(j.join());
}

#[test]
fn certificate_from_pem() {
    let dir = tempfile::tempdir().unwrap();
    let keys = test_cert_gen::keys();

    let der_path = dir.path().join("cert.der");
    fs::write(&der_path, keys.client.ca.get_der()).unwrap();
    let output = Command::new("openssl")
        .arg("x509")
        .arg("-in")
        .arg(der_path)
        .arg("-inform")
        .arg("der")
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert!(output.status.success());

    let cert = Certificate::from_pem(&output.stdout).unwrap();
    assert_eq!(cert.to_der().unwrap(), keys.client.ca.get_der());
}

#[test]
fn peer_certificate() {
    let keys = test_cert_gen::keys();

    let identity = p!(Identity::from_pkcs12(
        &keys.server.cert_and_key_pkcs12.pkcs12.0,
        &keys.server.cert_and_key_pkcs12.password
    ));
    let builder = p!(TlsAcceptor::new(identity));

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port = p!(listener.local_addr()).port();

    let j = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        let socket = p!(builder.accept(socket));
        assert!(socket.peer_certificate().unwrap().is_none());
    });

    let root_ca = Certificate::from_der(keys.client.ca.get_der()).unwrap();

    let socket = p!(TcpStream::connect(("localhost", port)));
    let builder = p!(TlsConnector::builder()
        .add_root_certificate(root_ca)
        .build());
    let socket = p!(builder.connect("localhost", socket));

    let cert = socket.peer_certificate().unwrap().unwrap();
    assert_eq!(
        cert.to_der().unwrap(),
        keys.server.cert_and_key.cert.get_der()
    );

    p!(j.join());
}

#[test]
fn server_tls11_only() {
    let keys = test_cert_gen::keys();

    let identity = p!(Identity::from_pkcs12(
        &keys.server.cert_and_key_pkcs12.pkcs12.0,
        &keys.server.cert_and_key_pkcs12.password
    ));
    let builder = p!(TlsAcceptor::builder(identity)
        .min_protocol_version(Some(Protocol::Tlsv12))
        .max_protocol_version(Some(Protocol::Tlsv12))
        .build());

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port = p!(listener.local_addr()).port();

    let j = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        let mut socket = p!(builder.accept(socket));

        let mut buf = [0; 5];
        p!(socket.read_exact(&mut buf));
        assert_eq!(&buf, b"hello");

        p!(socket.write_all(b"world"));
    });

    let root_ca = Certificate::from_der(keys.client.ca.get_der()).unwrap();

    let socket = p!(TcpStream::connect(("localhost", port)));
    let builder = p!(TlsConnector::builder()
        .add_root_certificate(root_ca)
        .min_protocol_version(Some(Protocol::Tlsv12))
        .max_protocol_version(Some(Protocol::Tlsv12))
        .build());
    let mut socket = p!(builder.connect("localhost", socket));

    p!(socket.write_all(b"hello"));
    let mut buf = vec![];
    p!(socket.read_to_end(&mut buf));
    assert_eq!(buf, b"world");

    p!(j.join());
}

#[test]
fn server_no_shared_protocol() {
    let keys = test_cert_gen::keys();

    let identity = p!(Identity::from_pkcs12(
        &keys.server.cert_and_key_pkcs12.pkcs12.0,
        &keys.server.cert_and_key_pkcs12.password
    ));
    let builder = p!(TlsAcceptor::builder(identity)
        .min_protocol_version(Some(Protocol::Tlsv12))
        .build());

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port = p!(listener.local_addr()).port();

    let j = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        assert!(builder.accept(socket).is_err());
    });

    let root_ca = Certificate::from_der(keys.client.ca.get_der()).unwrap();

    let socket = p!(TcpStream::connect(("localhost", port)));
    let builder = p!(TlsConnector::builder()
        .add_root_certificate(root_ca)
        .min_protocol_version(Some(Protocol::Tlsv11))
        .max_protocol_version(Some(Protocol::Tlsv11))
        .build());
    assert!(builder.connect("localhost", socket).is_err());

    p!(j.join());
}

#[test]
fn server_untrusted() {
    let keys = test_cert_gen::keys();

    let identity = p!(Identity::from_pkcs12(
        &keys.server.cert_and_key_pkcs12.pkcs12.0,
        &keys.server.cert_and_key_pkcs12.password
    ));
    let builder = p!(TlsAcceptor::new(identity));

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port = p!(listener.local_addr()).port();

    let j = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        // FIXME should assert error
        // https://github.com/steffengy/schannel-rs/issues/20
        let _ = builder.accept(socket);
    });

    let socket = p!(TcpStream::connect(("localhost", port)));
    let builder = p!(TlsConnector::new());
    builder.connect("localhost", socket).unwrap_err();

    p!(j.join());
}

#[test]
fn server_untrusted_unverified() {
    let keys = test_cert_gen::keys();

    let identity = p!(Identity::from_pkcs12(
        &keys.server.cert_and_key_pkcs12.pkcs12.0,
        &keys.server.cert_and_key_pkcs12.password
    ));
    let builder = p!(TlsAcceptor::new(identity));

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port = p!(listener.local_addr()).port();

    let j = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        let mut socket = p!(builder.accept(socket));

        let mut buf = [0; 5];
        p!(socket.read_exact(&mut buf));
        assert_eq!(&buf, b"hello");

        p!(socket.write_all(b"world"));
    });

    let socket = p!(TcpStream::connect(("localhost", port)));
    let builder = p!(TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build());
    let mut socket = p!(builder.connect("localhost", socket));

    p!(socket.write_all(b"hello"));
    let mut buf = vec![];
    p!(socket.read_to_end(&mut buf));
    assert_eq!(buf, b"world");

    p!(j.join());
}

#[test]
fn import_same_identity_multiple_times() {
    let keys = test_cert_gen::keys();

    let _ = p!(Identity::from_pkcs12(
        &keys.server.cert_and_key_pkcs12.pkcs12.0,
        &keys.server.cert_and_key_pkcs12.password
    ));
    let _ = p!(Identity::from_pkcs12(
        &keys.server.cert_and_key_pkcs12.pkcs12.0,
        &keys.server.cert_and_key_pkcs12.password
    ));

    let cert = keys.server.cert_and_key.cert.to_pem().into_bytes();
    let key = rsa_to_pkcs8(&keys.server.cert_and_key.key.to_pem_incorrect()).into_bytes();
    let _ = p!(Identity::from_pkcs8(&cert, &key));
    let _ = p!(Identity::from_pkcs8(&cert, &key));
}

#[test]
fn from_pkcs8_rejects_rsa_key() {
    let keys = test_cert_gen::keys();
    let cert = keys.server.cert_and_key.cert.to_pem().into_bytes();
    let rsa_key = keys.server.cert_and_key.key.to_pem_incorrect();
    assert!(Identity::from_pkcs8(&cert, rsa_key.as_bytes()).is_err());
    let pkcs8_key = rsa_to_pkcs8(&rsa_key);
    assert!(Identity::from_pkcs8(&cert, pkcs8_key.as_bytes()).is_ok());
}

#[test]
fn shutdown() {
    let keys = test_cert_gen::keys();

    let identity = p!(Identity::from_pkcs12(
        &keys.server.cert_and_key_pkcs12.pkcs12.0,
        &keys.server.cert_and_key_pkcs12.password
    ));
    let builder = p!(TlsAcceptor::new(identity));

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port = p!(listener.local_addr()).port();

    let j = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        let mut socket = p!(builder.accept(socket));

        let mut buf = [0; 5];
        p!(socket.read_exact(&mut buf));
        assert_eq!(&buf, b"hello");

        assert_eq!(p!(socket.read(&mut buf)), 0);
        p!(socket.shutdown());
    });

    let root_ca = Certificate::from_der(keys.client.ca.get_der()).unwrap();

    let socket = p!(TcpStream::connect(("localhost", port)));
    let builder = p!(TlsConnector::builder()
        .add_root_certificate(root_ca)
        .build());
    let mut socket = p!(builder.connect("localhost", socket));

    p!(socket.write_all(b"hello"));
    p!(socket.shutdown());

    p!(j.join());
}

#[test]
#[cfg(feature = "alpn")]
fn alpn_google_h2() {
    let builder = p!(TlsConnector::builder().request_alpns(&["h2"]).build());
    let s = p!(TcpStream::connect("google.com:443"));
    let socket = p!(builder.connect("google.com", s));
    let alpn = p!(socket.negotiated_alpn());
    assert_eq!(alpn, Some(b"h2".to_vec()));
}

#[test]
#[cfg(feature = "alpn")]
fn alpn_google_invalid() {
    let builder = p!(TlsConnector::builder().request_alpns(&["h2c"]).build());
    let s = p!(TcpStream::connect("google.com:443"));
    let socket = p!(builder.connect("google.com", s));
    let alpn = p!(socket.negotiated_alpn());
    assert_eq!(alpn, None);
}

#[test]
#[cfg(feature = "alpn")]
fn alpn_google_none() {
    let builder = p!(TlsConnector::new());
    let s = p!(TcpStream::connect("google.com:443"));
    let socket = p!(builder.connect("google.com", s));
    let alpn = p!(socket.negotiated_alpn());
    assert_eq!(alpn, None);
}

#[test]
fn server_pkcs8() {
    let keys = test_cert_gen::keys();
    let cert = keys.server.cert_and_key.cert.to_pem().into_bytes();
    let key = rsa_to_pkcs8(&keys.server.cert_and_key.key.to_pem_incorrect()).into_bytes();

    let ident = Identity::from_pkcs8(&cert, &key).unwrap();
    let ident2 = ident.clone();
    let builder = p!(TlsAcceptor::new(ident));

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port = p!(listener.local_addr()).port();

    let j = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        let mut socket = p!(builder.accept(socket));

        let mut buf = [0; 5];
        p!(socket.read_exact(&mut buf));
        assert_eq!(&buf, b"hello");

        p!(socket.write_all(b"world"));
    });

    let root_ca = Certificate::from_der(keys.client.ca.get_der()).unwrap();

    let socket = p!(TcpStream::connect(("localhost", port)));
    let mut builder = TlsConnector::builder();
    // FIXME
    // This checks that we can successfully add a certificate on the client side.
    // Unfortunately, we can not request client certificates through the API of this library,
    // otherwise we could check in the server thread that
    // socket.peer_certificate().unwrap().is_some()
    builder.identity(ident2);

    builder.add_root_certificate(root_ca);
    let builder = p!(builder.build());
    let mut socket = p!(builder.connect("localhost", socket));

    p!(socket.write_all(b"hello"));
    let mut buf = vec![];
    p!(socket.read_to_end(&mut buf));
    assert_eq!(buf, b"world");

    p!(j.join());
}

#[test]
fn two_servers() {
    let keys1 = test_cert_gen::gen_keys();
    let cert = keys1.server.cert_and_key.cert.to_pem().into_bytes();
    let key = rsa_to_pkcs8(&keys1.server.cert_and_key.key.to_pem_incorrect()).into_bytes();
    let identity = p!(Identity::from_pkcs8(&cert, &key));
    let builder = TlsAcceptor::builder(identity);
    let builder = p!(builder.build());

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port = p!(listener.local_addr()).port();

    let j = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        let mut socket = p!(builder.accept(socket));

        let mut buf = [0; 5];
        p!(socket.read_exact(&mut buf));
        assert_eq!(&buf, b"hello");

        p!(socket.write_all(b"world"));
    });

    let keys2 = test_cert_gen::gen_keys();
    let cert = keys2.server.cert_and_key.cert.to_pem().into_bytes();
    let key = rsa_to_pkcs8(&keys2.server.cert_and_key.key.to_pem_incorrect()).into_bytes();
    let identity = p!(Identity::from_pkcs8(&cert, &key));
    let builder = TlsAcceptor::builder(identity);
    let builder = p!(builder.build());

    let listener = p!(TcpListener::bind("0.0.0.0:0"));
    let port2 = p!(listener.local_addr()).port();

    let j2 = thread::spawn(move || {
        let socket = p!(listener.accept()).0;
        let mut socket = p!(builder.accept(socket));

        let mut buf = [0; 5];
        p!(socket.read_exact(&mut buf));
        assert_eq!(&buf, b"hello");

        p!(socket.write_all(b"world"));
    });

    let root_ca = Certificate::from_der(keys1.client.ca.get_der()).unwrap();

    let socket = p!(TcpStream::connect(("localhost", port)));
    let mut builder = TlsConnector::builder();
    builder.add_root_certificate(root_ca);
    let builder = p!(builder.build());
    let mut socket = p!(builder.connect("localhost", socket));

    p!(socket.write_all(b"hello"));
    let mut buf = vec![];
    p!(socket.read_to_end(&mut buf));
    assert_eq!(buf, b"world");

    let root_ca = Certificate::from_der(keys2.client.ca.get_der()).unwrap();

    let socket = p!(TcpStream::connect(("localhost", port2)));
    let mut builder = TlsConnector::builder();
    builder.add_root_certificate(root_ca);
    let builder = p!(builder.build());
    let mut socket = p!(builder.connect("localhost", socket));

    p!(socket.write_all(b"hello"));
    let mut buf = vec![];
    p!(socket.read_to_end(&mut buf));
    assert_eq!(buf, b"world");

    p!(j.join());
    p!(j2.join());
}

fn rsa_to_pkcs8(pem: &str) -> String {
    let mut child = Command::new("openssl")
        .arg("pkcs8")
        .arg("-topk8")
        .arg("-nocrypt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let child_stdin = child.stdin.as_mut().unwrap();
        child_stdin.write_all(pem.as_bytes()).unwrap();
    }
    String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap()
}
