#![allow(unused_imports)]

use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, BufReader};
use std::iter;
use std::mem;
use std::net::UdpSocket;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use crate::dh::Dh;
use crate::error::ErrorStack;
use crate::hash::MessageDigest;
#[cfg(not(any(boringssl, awslc)))]
use crate::ocsp::{OcspResponse, OcspResponseStatus};
use crate::pkey::{Id, PKey};
use crate::srtp::SrtpProfileId;
use crate::ssl::test::server::Server;
#[cfg(any(ossl110, ossl111, libressl261))]
use crate::ssl::SslVersion;
use crate::ssl::{self, NameType, SslConnectorBuilder};
#[cfg(ossl111)]
use crate::ssl::{ClientHelloResponse, ExtensionContext};
use crate::ssl::{
    Error, HandshakeError, MidHandshakeSslStream, ShutdownResult, ShutdownState, Ssl, SslAcceptor,
    SslAcceptorBuilder, SslConnector, SslContext, SslContextBuilder, SslFiletype, SslMethod,
    SslOptions, SslSessionCacheMode, SslStream, SslVerifyMode, StatusType,
};
#[cfg(ossl102)]
use crate::x509::store::X509StoreBuilder;
#[cfg(ossl102)]
use crate::x509::verify::X509CheckFlags;
use crate::x509::{X509Name, X509StoreContext, X509VerifyResult, X509};

mod server;

static ROOT_CERT: &[u8] = include_bytes!("../../../test/root-ca.pem");
static CERT: &[u8] = include_bytes!("../../../test/cert.pem");
static KEY: &[u8] = include_bytes!("../../../test/key.pem");

#[test]
fn verify_untrusted() {
    let mut server = Server::builder();
    server.should_error();
    let server = server.build();

    let mut client = server.client();
    client.ctx().set_verify(SslVerifyMode::PEER);

    client.connect_err();
}

#[test]
fn verify_trusted() {
    let server = Server::builder().build();

    let mut client = server.client();
    client.ctx().set_ca_file("test/root-ca.pem").unwrap();

    client.connect();
}

#[test]
#[cfg(ossl102)]
fn verify_trusted_with_set_cert() {
    let server = Server::builder().build();

    let mut store = X509StoreBuilder::new().unwrap();
    let x509 = X509::from_pem(ROOT_CERT).unwrap();
    store.add_cert(x509).unwrap();

    let mut client = server.client();
    client.ctx().set_verify(SslVerifyMode::PEER);
    client.ctx().set_verify_cert_store(store.build()).unwrap();

    client.connect();
}

#[test]
fn verify_untrusted_callback_override_ok() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let server = Server::builder().build();

    let mut client = server.client();
    client
        .ctx()
        .set_verify_callback(SslVerifyMode::PEER, |_, x509| {
            CALLED_BACK.store(true, Ordering::SeqCst);
            assert!(x509.current_cert().is_some());
            true
        });

    client.connect();
    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
fn verify_untrusted_callback_override_bad() {
    let mut server = Server::builder();
    server.should_error();
    let server = server.build();

    let mut client = server.client();
    client
        .ctx()
        .set_verify_callback(SslVerifyMode::PEER, |_, _| false);

    client.connect_err();
}

#[test]
fn verify_trusted_callback_override_ok() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let server = Server::builder().build();

    let mut client = server.client();
    client.ctx().set_ca_file("test/root-ca.pem").unwrap();
    client
        .ctx()
        .set_verify_callback(SslVerifyMode::PEER, |_, x509| {
            CALLED_BACK.store(true, Ordering::SeqCst);
            assert!(x509.current_cert().is_some());
            true
        });

    client.connect();
    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
fn verify_trusted_callback_override_bad() {
    let mut server = Server::builder();
    server.should_error();
    let server = server.build();

    let mut client = server.client();
    client.ctx().set_ca_file("test/root-ca.pem").unwrap();
    client
        .ctx()
        .set_verify_callback(SslVerifyMode::PEER, |_, _| false);

    client.connect_err();
}

#[test]
fn verify_callback_load_certs() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let server = Server::builder().build();

    let mut client = server.client();
    client
        .ctx()
        .set_verify_callback(SslVerifyMode::PEER, |_, x509| {
            CALLED_BACK.store(true, Ordering::SeqCst);
            assert!(x509.current_cert().is_some());
            true
        });

    client.connect();
    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
fn verify_trusted_get_error_ok() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let server = Server::builder().build();

    let mut client = server.client();
    client.ctx().set_ca_file("test/root-ca.pem").unwrap();
    client
        .ctx()
        .set_verify_callback(SslVerifyMode::PEER, |_, x509| {
            CALLED_BACK.store(true, Ordering::SeqCst);
            assert_eq!(x509.error(), X509VerifyResult::OK);
            true
        });

    client.connect();
    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
fn verify_trusted_get_error_err() {
    let mut server = Server::builder();
    server.should_error();
    let server = server.build();

    let mut client = server.client();
    client
        .ctx()
        .set_verify_callback(SslVerifyMode::PEER, |_, x509| {
            assert_ne!(x509.error(), X509VerifyResult::OK);
            false
        });

    client.connect_err();
}

#[test]
fn verify_callback() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let server = Server::builder().build();

    let mut client = server.client();
    let expected = "59172d9313e84459bcff27f967e79e6e9217e584";
    client
        .ctx()
        .set_verify_callback(SslVerifyMode::PEER, move |_, x509| {
            CALLED_BACK.store(true, Ordering::SeqCst);
            let cert = x509.current_cert().unwrap();
            let digest = cert.digest(MessageDigest::sha1()).unwrap();
            assert_eq!(hex::encode(digest), expected);
            true
        });

    client.connect();
    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
fn ssl_verify_callback() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let server = Server::builder().build();

    let mut client = server.client().build().builder();
    let expected = "59172d9313e84459bcff27f967e79e6e9217e584";
    client
        .ssl()
        .set_verify_callback(SslVerifyMode::PEER, move |_, x509| {
            CALLED_BACK.store(true, Ordering::SeqCst);
            let cert = x509.current_cert().unwrap();
            let digest = cert.digest(MessageDigest::sha1()).unwrap();
            assert_eq!(hex::encode(digest), expected);
            true
        });

    client.connect();
    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
fn get_ctx_options() {
    let ctx = SslContext::builder(SslMethod::tls()).unwrap();
    ctx.options();
}

#[test]
fn set_ctx_options() {
    let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
    let opts = ctx.set_options(SslOptions::NO_TICKET);
    assert!(opts.contains(SslOptions::NO_TICKET));
}

#[test]
#[cfg(not(any(boringssl, awslc)))]
fn clear_ctx_options() {
    let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
    ctx.set_options(SslOptions::ALL);
    let opts = ctx.clear_options(SslOptions::ALL);
    assert!(!opts.contains(SslOptions::ALL));
}

#[test]
fn zero_length_buffers() {
    let server = Server::builder().build();

    let mut s = server.client().connect();
    assert_eq!(s.write(&[]).unwrap(), 0);
    assert_eq!(s.read(&mut []).unwrap(), 0);
}

#[test]
fn peer_certificate() {
    let server = Server::builder().build();

    let s = server.client().connect();
    let cert = s.ssl().peer_certificate().unwrap();
    let fingerprint = cert.digest(MessageDigest::sha1()).unwrap();
    assert_eq!(
        hex::encode(fingerprint),
        "59172d9313e84459bcff27f967e79e6e9217e584"
    );
}

#[test]
fn pending() {
    let mut server = Server::builder();
    server.io_cb(|mut s| s.write_all(&[0; 10]).unwrap());
    let server = server.build();

    let mut s = server.client().connect();
    s.read_exact(&mut [0]).unwrap();

    assert_eq!(s.ssl().pending(), 9);
    assert_eq!(s.read(&mut [0; 10]).unwrap(), 9);
}

#[test]
fn state() {
    const EXPECTED_STATE_STRING_LONG: &str = "SSL negotiation finished successfully";

    let server = Server::builder().build();

    let s = server.client().connect();
    #[cfg(not(any(boringssl, awslc)))]
    assert_eq!(s.ssl().state_string().trim(), "SSLOK");
    #[cfg(boringssl)]
    assert_eq!(s.ssl().state_string(), "!!!!!!");
    #[cfg(awslc)]
    assert_eq!(s.ssl().state_string(), EXPECTED_STATE_STRING_LONG);

    assert_eq!(s.ssl().state_string_long(), EXPECTED_STATE_STRING_LONG);
}

// when a connection uses ECDHE P-384 key exchange, then the temp key APIs
// return P-384 keys, and the peer and local keys are different.
#[test]
#[cfg(ossl300)]
fn peer_tmp_key_p384() {
    let mut server = Server::builder();
    server.ctx().set_groups_list("P-384").unwrap();
    let server = server.build();
    let s = server.client().connect();
    let peer_temp = s.ssl().peer_tmp_key().unwrap();
    assert_eq!(peer_temp.id(), Id::EC);
    assert_eq!(peer_temp.bits(), 384);

    let local_temp = s.ssl().tmp_key().unwrap();
    assert_eq!(local_temp.id(), Id::EC);
    assert_eq!(local_temp.bits(), 384);

    assert_ne!(
        peer_temp.ec_key().unwrap().public_key_to_der().unwrap(),
        local_temp.ec_key().unwrap().public_key_to_der().unwrap(),
    );
}

// when a connection uses RSA key exchange, then the peer (server) temp key is
// an Error because there is no temp key, and the local (client) temp key is the
// temp key sent in the initial key share.
#[test]
#[cfg(ossl300)]
fn peer_tmp_key_rsa() {
    let mut server = Server::builder();
    server.ctx().set_cipher_list("RSA").unwrap();
    // RSA key exchange is not allowed in TLS 1.3, so force the connection
    // to negotiate TLS 1.2
    server
        .ctx()
        .set_max_proto_version(Some(SslVersion::TLS1_2))
        .unwrap();
    let server = server.build();
    let mut client = server.client();
    client.ctx().set_groups_list("P-521").unwrap();
    let s = client.connect();
    let peer_temp = s.ssl().peer_tmp_key();
    assert!(peer_temp.is_err());

    // this is the temp key that the client sent in the initial key share
    let local_temp = s.ssl().tmp_key().unwrap();
    assert_eq!(local_temp.id(), Id::EC);
    assert_eq!(local_temp.bits(), 521);
}

/// Tests that when both the client as well as the server use SRTP and their
/// lists of supported protocols have an overlap -- with only ONE protocol
/// being valid for both.
#[test]
fn test_connect_with_srtp_ctx() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let guard = thread::spawn(move || {
        let stream = listener.accept().unwrap().0;
        let mut ctx = SslContext::builder(SslMethod::dtls()).unwrap();
        ctx.set_tlsext_use_srtp("SRTP_AES128_CM_SHA1_80:SRTP_AES128_CM_SHA1_32")
            .unwrap();
        ctx.set_certificate_file(Path::new("test/cert.pem"), SslFiletype::PEM)
            .unwrap();
        ctx.set_private_key_file(Path::new("test/key.pem"), SslFiletype::PEM)
            .unwrap();
        let mut ssl = Ssl::new(&ctx.build()).unwrap();
        ssl.set_mtu(1500).unwrap();
        let mut stream = ssl.accept(stream).unwrap();

        let mut buf = [0; 60];
        stream
            .ssl()
            .export_keying_material(&mut buf, "EXTRACTOR-dtls_srtp", None)
            .unwrap();

        stream.write_all(&[0]).unwrap();

        buf
    });

    let stream = TcpStream::connect(addr).unwrap();
    let mut ctx = SslContext::builder(SslMethod::dtls()).unwrap();
    ctx.set_tlsext_use_srtp("SRTP_AES128_CM_SHA1_80:SRTP_AES128_CM_SHA1_32")
        .unwrap();
    let mut ssl = Ssl::new(&ctx.build()).unwrap();
    ssl.set_mtu(1500).unwrap();
    let mut stream = ssl.connect(stream).unwrap();

    let mut buf = [1; 60];
    {
        let srtp_profile = stream.ssl().selected_srtp_profile().unwrap();
        assert_eq!("SRTP_AES128_CM_SHA1_80", srtp_profile.name());
        assert_eq!(SrtpProfileId::SRTP_AES128_CM_SHA1_80, srtp_profile.id());
    }
    stream
        .ssl()
        .export_keying_material(&mut buf, "EXTRACTOR-dtls_srtp", None)
        .expect("extract");

    stream.read_exact(&mut [0]).unwrap();

    let buf2 = guard.join().unwrap();

    assert_eq!(buf[..], buf2[..]);
}

/// Tests that when both the client as well as the server use SRTP and their
/// lists of supported protocols have an overlap -- with only ONE protocol
/// being valid for both.
#[test]
fn test_connect_with_srtp_ssl() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let guard = thread::spawn(move || {
        let stream = listener.accept().unwrap().0;
        let mut ctx = SslContext::builder(SslMethod::dtls()).unwrap();
        ctx.set_certificate_file(Path::new("test/cert.pem"), SslFiletype::PEM)
            .unwrap();
        ctx.set_private_key_file(Path::new("test/key.pem"), SslFiletype::PEM)
            .unwrap();
        let mut ssl = Ssl::new(&ctx.build()).unwrap();
        ssl.set_tlsext_use_srtp("SRTP_AES128_CM_SHA1_80:SRTP_AES128_CM_SHA1_32")
            .unwrap();
        let mut profilenames = String::new();
        for profile in ssl.srtp_profiles().unwrap() {
            if !profilenames.is_empty() {
                profilenames.push(':');
            }
            profilenames += profile.name();
        }
        assert_eq!(
            "SRTP_AES128_CM_SHA1_80:SRTP_AES128_CM_SHA1_32",
            profilenames
        );
        ssl.set_mtu(1500).unwrap();
        let mut stream = ssl.accept(stream).unwrap();

        let mut buf = [0; 60];
        stream
            .ssl()
            .export_keying_material(&mut buf, "EXTRACTOR-dtls_srtp", None)
            .unwrap();

        stream.write_all(&[0]).unwrap();

        buf
    });

    let stream = TcpStream::connect(addr).unwrap();
    let ctx = SslContext::builder(SslMethod::dtls()).unwrap();
    let mut ssl = Ssl::new(&ctx.build()).unwrap();
    ssl.set_tlsext_use_srtp("SRTP_AES128_CM_SHA1_80:SRTP_AES128_CM_SHA1_32")
        .unwrap();
    ssl.set_mtu(1500).unwrap();
    let mut stream = ssl.connect(stream).unwrap();

    let mut buf = [1; 60];
    {
        let srtp_profile = stream.ssl().selected_srtp_profile().unwrap();
        assert_eq!("SRTP_AES128_CM_SHA1_80", srtp_profile.name());
        assert_eq!(SrtpProfileId::SRTP_AES128_CM_SHA1_80, srtp_profile.id());
    }
    stream
        .ssl()
        .export_keying_material(&mut buf, "EXTRACTOR-dtls_srtp", None)
        .expect("extract");

    stream.read_exact(&mut [0]).unwrap();

    let buf2 = guard.join().unwrap();

    assert_eq!(buf[..], buf2[..]);
}

/// Tests that when the `SslStream` is created as a server stream, the protocols
/// are correctly advertised to the client.
#[test]
#[cfg(any(ossl102, libressl261, boringssl, awslc))]
fn test_alpn_server_advertise_multiple() {
    let mut server = Server::builder();
    server.ctx().set_alpn_select_callback(|_, client| {
        ssl::select_next_proto(b"\x08http/1.1\x08spdy/3.1", client).ok_or(ssl::AlpnError::NOACK)
    });
    let server = server.build();

    let mut client = server.client();
    client.ctx().set_alpn_protos(b"\x08spdy/3.1").unwrap();
    let s = client.connect();
    assert_eq!(s.ssl().selected_alpn_protocol(), Some(&b"spdy/3.1"[..]));
}

#[test]
#[cfg(any(ossl110, boringssl, awslc))]
fn test_alpn_server_select_none_fatal() {
    let mut server = Server::builder();
    server.ctx().set_alpn_select_callback(|_, client| {
        ssl::select_next_proto(b"\x08http/1.1\x08spdy/3.1", client)
            .ok_or(ssl::AlpnError::ALERT_FATAL)
    });
    server.should_error();
    let server = server.build();

    let mut client = server.client();
    client.ctx().set_alpn_protos(b"\x06http/2").unwrap();
    client.connect_err();
}

#[test]
#[cfg(any(ossl102, libressl261, boringssl, awslc))]
fn test_alpn_server_select_none() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server.ctx().set_alpn_select_callback(|_, client| {
        CALLED_BACK.store(true, Ordering::SeqCst);
        ssl::select_next_proto(b"\x08http/1.1\x08spdy/3.1", client).ok_or(ssl::AlpnError::NOACK)
    });
    let server = server.build();

    let mut client = server.client();
    client.ctx().set_alpn_protos(b"\x06http/2").unwrap();
    let s = client.connect();
    assert_eq!(None, s.ssl().selected_alpn_protocol());
    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
#[cfg(any(boringssl, ossl102, libressl261, awslc))]
fn test_alpn_server_unilateral() {
    let server = Server::builder().build();

    let mut client = server.client();
    client.ctx().set_alpn_protos(b"\x06http/2").unwrap();
    let s = client.connect();
    assert_eq!(None, s.ssl().selected_alpn_protocol());
}

#[test]
#[should_panic(expected = "blammo")]
fn write_panic() {
    struct ExplodingStream(TcpStream);

    impl Read for ExplodingStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
    }

    impl Write for ExplodingStream {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            panic!("blammo");
        }

        fn flush(&mut self) -> io::Result<()> {
            self.0.flush()
        }
    }

    let mut server = Server::builder();
    server.should_error();
    let server = server.build();

    let stream = ExplodingStream(server.connect_tcp());

    let ctx = SslContext::builder(SslMethod::tls()).unwrap();
    let _ = Ssl::new(&ctx.build()).unwrap().connect(stream);
}

#[test]
#[should_panic(expected = "blammo")]
fn read_panic() {
    struct ExplodingStream(TcpStream);

    impl Read for ExplodingStream {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("blammo");
        }
    }

    impl Write for ExplodingStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.0.flush()
        }
    }

    let mut server = Server::builder();
    server.should_error();
    let server = server.build();

    let stream = ExplodingStream(server.connect_tcp());

    let ctx = SslContext::builder(SslMethod::tls()).unwrap();
    let _ = Ssl::new(&ctx.build()).unwrap().connect(stream);
}

#[test]
#[cfg_attr(all(libressl321, not(libressl340)), ignore)]
#[should_panic(expected = "blammo")]
fn flush_panic() {
    struct ExplodingStream(TcpStream);

    impl Read for ExplodingStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
    }

    impl Write for ExplodingStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            panic!("blammo");
        }
    }

    let mut server = Server::builder();
    server.should_error();
    let server = server.build();

    let stream = ExplodingStream(server.connect_tcp());

    let ctx = SslContext::builder(SslMethod::tls()).unwrap();
    let _ = Ssl::new(&ctx.build()).unwrap().connect(stream);
}

#[test]
fn refcount_ssl_context() {
    let mut ssl = {
        let ctx = SslContext::builder(SslMethod::tls()).unwrap();
        ssl::Ssl::new(&ctx.build()).unwrap()
    };

    {
        let new_ctx_a = SslContext::builder(SslMethod::tls()).unwrap().build();
        ssl.set_ssl_context(&new_ctx_a).unwrap();
    }
}

#[test]
#[cfg_attr(libressl250, ignore)]
#[cfg_attr(target_os = "windows", ignore)]
#[cfg_attr(all(target_os = "macos", feature = "vendored"), ignore)]
fn default_verify_paths() {
    let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
    ctx.set_default_verify_paths().unwrap();
    ctx.set_verify(SslVerifyMode::PEER);
    let ctx = ctx.build();
    let s = match TcpStream::connect("google.com:443") {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut ssl = Ssl::new(&ctx).unwrap();
    ssl.set_hostname("google.com").unwrap();
    let mut socket = ssl.connect(s).unwrap();

    socket.write_all(b"GET / HTTP/1.0\r\n\r\n").unwrap();
    let mut result = vec![];
    socket.read_to_end(&mut result).unwrap();

    println!("{}", String::from_utf8_lossy(&result));
    assert!(result.starts_with(b"HTTP/1.0"));
    assert!(result.ends_with(b"</HTML>\r\n") || result.ends_with(b"</html>"));
}

#[test]
fn add_extra_chain_cert() {
    let cert = X509::from_pem(CERT).unwrap();
    let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
    ctx.add_extra_chain_cert(cert).unwrap();
}

#[test]
#[cfg(ossl102)]
fn verify_valid_hostname() {
    let server = Server::builder().build();

    let mut client = server.client();
    client.ctx().set_ca_file("test/root-ca.pem").unwrap();
    client.ctx().set_verify(SslVerifyMode::PEER);

    let mut client = client.build().builder();
    client
        .ssl()
        .param_mut()
        .set_hostflags(X509CheckFlags::NO_PARTIAL_WILDCARDS);
    client.ssl().param_mut().set_host("foobar.com").unwrap();
    client.connect();
}

#[test]
#[cfg(ossl102)]
fn verify_invalid_hostname() {
    let mut server = Server::builder();
    server.should_error();
    let server = server.build();

    let mut client = server.client();
    client.ctx().set_ca_file("test/root-ca.pem").unwrap();
    client.ctx().set_verify(SslVerifyMode::PEER);

    let mut client = client.build().builder();
    client
        .ssl()
        .param_mut()
        .set_hostflags(X509CheckFlags::NO_PARTIAL_WILDCARDS);
    client.ssl().param_mut().set_host("bogus.com").unwrap();
    client.connect_err();
}

#[test]
fn connector_valid_hostname() {
    let server = Server::builder().build();

    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    connector.set_ca_file("test/root-ca.pem").unwrap();

    let s = server.connect_tcp();
    let mut s = connector.build().connect("foobar.com", s).unwrap();
    s.read_exact(&mut [0]).unwrap();
}

#[test]
fn connector_invalid_hostname() {
    let mut server = Server::builder();
    server.should_error();
    let server = server.build();

    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    connector.set_ca_file("test/root-ca.pem").unwrap();

    let s = server.connect_tcp();
    connector.build().connect("bogus.com", s).unwrap_err();
}

#[test]
fn connector_invalid_no_hostname_verification() {
    let server = Server::builder().build();

    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    connector.set_ca_file("test/root-ca.pem").unwrap();

    let s = server.connect_tcp();
    let mut s = connector
        .build()
        .configure()
        .unwrap()
        .verify_hostname(false)
        .connect("bogus.com", s)
        .unwrap();
    s.read_exact(&mut [0]).unwrap();
}

#[test]
fn connector_no_hostname_still_verifies() {
    let mut server = Server::builder();
    server.should_error();
    let server = server.build();

    let connector = SslConnector::builder(SslMethod::tls()).unwrap().build();

    let s = server.connect_tcp();
    assert!(connector
        .configure()
        .unwrap()
        .verify_hostname(false)
        .connect("fizzbuzz.com", s)
        .is_err());
}

#[test]
fn connector_can_disable_verify() {
    let server = Server::builder().build();

    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    connector.set_verify(SslVerifyMode::NONE);
    let connector = connector.build();

    let s = server.connect_tcp();
    let mut s = connector
        .configure()
        .unwrap()
        .connect("fizzbuzz.com", s)
        .unwrap();
    s.read_exact(&mut [0]).unwrap();
}

#[test]
fn connector_does_use_sni_with_dnsnames() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut builder = Server::builder();
    builder.ctx().set_servername_callback(|ssl, _| {
        assert_eq!(ssl.servername(NameType::HOST_NAME), Some("foobar.com"));
        CALLED_BACK.store(true, Ordering::SeqCst);
        Ok(())
    });
    let server = builder.build();

    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    connector.set_ca_file("test/root-ca.pem").unwrap();

    let s = server.connect_tcp();
    let mut s = connector
        .build()
        .configure()
        .unwrap()
        .connect("foobar.com", s)
        .unwrap();
    s.read_exact(&mut [0]).unwrap();

    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
fn connector_doesnt_use_sni_with_ips() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut builder = Server::builder();
    builder.ctx().set_servername_callback(|ssl, _| {
        assert_eq!(ssl.servername(NameType::HOST_NAME), None);
        CALLED_BACK.store(true, Ordering::SeqCst);
        Ok(())
    });
    let server = builder.build();

    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    // The server's cert isn't issued for 127.0.0.1 but we don't care for this test.
    connector.set_verify(SslVerifyMode::NONE);

    let s = server.connect_tcp();
    let mut s = connector
        .build()
        .configure()
        .unwrap()
        .connect("127.0.0.1", s)
        .unwrap();
    s.read_exact(&mut [0]).unwrap();

    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

fn test_mozilla_server(new: fn(SslMethod) -> Result<SslAcceptorBuilder, ErrorStack>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let t = thread::spawn(move || {
        let key = PKey::private_key_from_pem(KEY).unwrap();
        let cert = X509::from_pem(CERT).unwrap();
        let mut acceptor = new(SslMethod::tls()).unwrap();
        acceptor.set_private_key(&key).unwrap();
        acceptor.set_certificate(&cert).unwrap();
        let acceptor = acceptor.build();
        let stream = listener.accept().unwrap().0;
        let mut stream = acceptor.accept(stream).unwrap();

        stream.write_all(b"hello").unwrap();
    });

    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    connector.set_ca_file("test/root-ca.pem").unwrap();
    let connector = connector.build();

    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut stream = connector.connect("foobar.com", stream).unwrap();

    let mut buf = [0; 5];
    stream.read_exact(&mut buf).unwrap();
    assert_eq!(b"hello", &buf);

    t.join().unwrap();
}

#[test]
fn connector_client_server_mozilla_intermediate() {
    test_mozilla_server(SslAcceptor::mozilla_intermediate);
}

#[test]
fn connector_client_server_mozilla_modern() {
    test_mozilla_server(SslAcceptor::mozilla_modern);
}

#[test]
fn connector_client_server_mozilla_intermediate_v5() {
    test_mozilla_server(SslAcceptor::mozilla_intermediate_v5);
}

#[test]
#[cfg(any(ossl111, libressl340))]
fn connector_client_server_mozilla_modern_v5() {
    test_mozilla_server(SslAcceptor::mozilla_modern_v5);
}

#[test]
fn shutdown() {
    let mut server = Server::builder();
    server.io_cb(|mut s| {
        assert_eq!(s.read(&mut [0]).unwrap(), 0);
        assert_eq!(s.shutdown().unwrap(), ShutdownResult::Received);
    });
    let server = server.build();

    let mut s = server.client().connect();

    assert_eq!(s.get_shutdown(), ShutdownState::empty());
    assert_eq!(s.shutdown().unwrap(), ShutdownResult::Sent);
    assert_eq!(s.get_shutdown(), ShutdownState::SENT);
    assert_eq!(s.shutdown().unwrap(), ShutdownResult::Received);
    assert_eq!(
        s.get_shutdown(),
        ShutdownState::SENT | ShutdownState::RECEIVED
    );
}

#[test]
fn client_ca_list() {
    let names = X509Name::load_client_ca_file("test/root-ca.pem").unwrap();
    assert_eq!(names.len(), 1);

    let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
    ctx.set_client_ca_list(names);
}

#[test]
fn cert_store() {
    let server = Server::builder().build();

    let mut client = server.client();
    let cert = X509::from_pem(ROOT_CERT).unwrap();
    client.ctx().cert_store_mut().add_cert(cert).unwrap();
    client.ctx().set_verify(SslVerifyMode::PEER);

    client.connect();
}

#[test]
#[cfg_attr(any(all(libressl321, not(libressl340)), boringssl, awslc), ignore)]
fn tmp_dh_callback() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server.ctx().set_tmp_dh_callback(|_, _, _| {
        CALLED_BACK.store(true, Ordering::SeqCst);
        let dh = include_bytes!("../../../test/dhparams.pem");
        Dh::params_from_pem(dh)
    });

    let server = server.build();

    let mut client = server.client();
    // TLS 1.3 has no DH suites, so make sure we don't pick that version
    #[cfg(any(ossl111, libressl340))]
    client.ctx().set_options(super::SslOptions::NO_TLSV1_3);
    client.ctx().set_cipher_list("EDH").unwrap();
    client.connect();

    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
#[cfg(all(ossl101, not(ossl110)))]
#[allow(deprecated)]
fn tmp_ecdh_callback() {
    use crate::ec::EcKey;
    use crate::nid::Nid;

    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server.ctx().set_tmp_ecdh_callback(|_, _, _| {
        CALLED_BACK.store(true, Ordering::SeqCst);
        EcKey::from_curve_name(Nid::X9_62_PRIME256V1)
    });

    let server = server.build();

    let mut client = server.client();
    client.ctx().set_cipher_list("ECDH").unwrap();
    client.connect();

    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
#[cfg_attr(any(all(libressl321, not(libressl340)), boringssl, awslc), ignore)]
fn tmp_dh_callback_ssl() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server.ssl_cb(|ssl| {
        ssl.set_tmp_dh_callback(|_, _, _| {
            CALLED_BACK.store(true, Ordering::SeqCst);
            let dh = include_bytes!("../../../test/dhparams.pem");
            Dh::params_from_pem(dh)
        });
    });

    let server = server.build();

    let mut client = server.client();
    // TLS 1.3 has no DH suites, so make sure we don't pick that version
    #[cfg(any(ossl111, libressl340))]
    client.ctx().set_options(super::SslOptions::NO_TLSV1_3);
    client.ctx().set_cipher_list("EDH").unwrap();
    client.connect();

    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
#[cfg(all(ossl101, not(ossl110)))]
#[allow(deprecated)]
fn tmp_ecdh_callback_ssl() {
    use crate::ec::EcKey;
    use crate::nid::Nid;

    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server.ssl_cb(|ssl| {
        ssl.set_tmp_ecdh_callback(|_, _, _| {
            CALLED_BACK.store(true, Ordering::SeqCst);
            EcKey::from_curve_name(Nid::X9_62_PRIME256V1)
        });
    });

    let server = server.build();

    let mut client = server.client();
    client.ctx().set_cipher_list("ECDH").unwrap();
    client.connect();

    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
fn idle_session() {
    let ctx = SslContext::builder(SslMethod::tls()).unwrap().build();
    let ssl = Ssl::new(&ctx).unwrap();
    assert!(ssl.session().is_none());
}

/// LibreSSL 3.2.1 enabled TLSv1.3 by default for clients and sessions do
/// not work due to lack of PSK support. The test passes with NO_TLSV1_3,
/// but let's ignore it until LibreSSL supports it out of the box.
#[test]
#[cfg_attr(libressl321, ignore)]
fn active_session() {
    let server = Server::builder().build();

    let s = server.client().connect();

    let session = s.ssl().session().unwrap();
    let len = session.master_key_len();
    let mut buf = vec![0; len - 1];
    let copied = session.master_key(&mut buf);
    assert_eq!(copied, buf.len());
    let mut buf = vec![0; len + 1];
    let copied = session.master_key(&mut buf);
    assert_eq!(copied, len);
}

#[test]
#[cfg(not(any(boringssl, awslc)))]
fn status_callbacks() {
    static CALLED_BACK_SERVER: AtomicBool = AtomicBool::new(false);
    static CALLED_BACK_CLIENT: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server
        .ctx()
        .set_status_callback(|ssl| {
            CALLED_BACK_SERVER.store(true, Ordering::SeqCst);
            let response = OcspResponse::create(OcspResponseStatus::UNAUTHORIZED, None).unwrap();
            let response = response.to_der().unwrap();
            ssl.set_ocsp_status(&response).unwrap();
            Ok(true)
        })
        .unwrap();

    let server = server.build();

    let mut client = server.client();
    client
        .ctx()
        .set_status_callback(|ssl| {
            CALLED_BACK_CLIENT.store(true, Ordering::SeqCst);
            let response = OcspResponse::from_der(ssl.ocsp_status().unwrap()).unwrap();
            assert_eq!(response.status(), OcspResponseStatus::UNAUTHORIZED);
            Ok(true)
        })
        .unwrap();

    let mut client = client.build().builder();
    client.ssl().set_status_type(StatusType::OCSP).unwrap();

    client.connect();

    assert!(CALLED_BACK_SERVER.load(Ordering::SeqCst));
    assert!(CALLED_BACK_CLIENT.load(Ordering::SeqCst));
}

/// LibreSSL 3.2.1 enabled TLSv1.3 by default for clients and sessions do
/// not work due to lack of PSK support. The test passes with NO_TLSV1_3,
/// but let's ignore it until LibreSSL supports it out of the box.
#[test]
#[cfg_attr(libressl321, ignore)]
fn new_session_callback() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server.ctx().set_session_id_context(b"foo").unwrap();

    let server = server.build();

    let mut client = server.client();

    client
        .ctx()
        .set_session_cache_mode(SslSessionCacheMode::CLIENT | SslSessionCacheMode::NO_INTERNAL);
    client
        .ctx()
        .set_new_session_callback(|_, _| CALLED_BACK.store(true, Ordering::SeqCst));

    client.connect();

    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

/// LibreSSL 3.2.1 enabled TLSv1.3 by default for clients and sessions do
/// not work due to lack of PSK support. The test passes with NO_TLSV1_3,
/// but let's ignore it until LibreSSL supports it out of the box.
#[test]
#[cfg_attr(libressl321, ignore)]
fn new_session_callback_swapped_ctx() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server.ctx().set_session_id_context(b"foo").unwrap();

    let server = server.build();

    let mut client = server.client();

    client
        .ctx()
        .set_session_cache_mode(SslSessionCacheMode::CLIENT | SslSessionCacheMode::NO_INTERNAL);
    client
        .ctx()
        .set_new_session_callback(|_, _| CALLED_BACK.store(true, Ordering::SeqCst));

    let mut client = client.build().builder();

    let ctx = SslContextBuilder::new(SslMethod::tls()).unwrap().build();
    client.ssl().set_ssl_context(&ctx).unwrap();

    client.connect();

    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
fn keying_export() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let label = "EXPERIMENTAL test";
    let context = b"my context";

    let guard = thread::spawn(move || {
        let stream = listener.accept().unwrap().0;
        let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
        ctx.set_certificate_file(Path::new("test/cert.pem"), SslFiletype::PEM)
            .unwrap();
        ctx.set_private_key_file(Path::new("test/key.pem"), SslFiletype::PEM)
            .unwrap();
        let ssl = Ssl::new(&ctx.build()).unwrap();
        let mut stream = ssl.accept(stream).unwrap();

        let mut buf = [0; 32];
        stream
            .ssl()
            .export_keying_material(&mut buf, label, Some(context))
            .unwrap();

        stream.write_all(&[0]).unwrap();

        buf
    });

    let stream = TcpStream::connect(addr).unwrap();
    let ctx = SslContext::builder(SslMethod::tls()).unwrap();
    let ssl = Ssl::new(&ctx.build()).unwrap();
    let mut stream = ssl.connect(stream).unwrap();

    let mut buf = [1; 32];
    stream
        .ssl()
        .export_keying_material(&mut buf, label, Some(context))
        .unwrap();

    stream.read_exact(&mut [0]).unwrap();

    let buf2 = guard.join().unwrap();

    assert_eq!(buf, buf2);
}

#[test]
#[cfg(any(ossl110, libressl261))]
fn no_version_overlap() {
    let mut server = Server::builder();
    server.ctx().set_min_proto_version(None).unwrap();
    server
        .ctx()
        .set_max_proto_version(Some(SslVersion::TLS1_1))
        .unwrap();
    #[cfg(any(ossl110g, libressl270))]
    assert_eq!(server.ctx().max_proto_version(), Some(SslVersion::TLS1_1));
    server.should_error();
    let server = server.build();

    let mut client = server.client();
    client
        .ctx()
        .set_min_proto_version(Some(SslVersion::TLS1_2))
        .unwrap();
    #[cfg(ossl110g)]
    assert_eq!(client.ctx().min_proto_version(), Some(SslVersion::TLS1_2));
    client.ctx().set_max_proto_version(None).unwrap();

    client.connect_err();
}

#[test]
#[cfg(ossl111)]
fn custom_extensions() {
    static FOUND_EXTENSION: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server
        .ctx()
        .add_custom_ext(
            12345,
            ExtensionContext::CLIENT_HELLO,
            |_, _, _| -> Result<Option<&'static [u8]>, _> { unreachable!() },
            |_, _, data, _| {
                FOUND_EXTENSION.store(data == b"hello", Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap();

    let server = server.build();

    let mut client = server.client();
    client
        .ctx()
        .add_custom_ext(
            12345,
            ssl::ExtensionContext::CLIENT_HELLO,
            |_, _, _| Ok(Some(b"hello")),
            |_, _, _, _| unreachable!(),
        )
        .unwrap();

    client.connect();

    assert!(FOUND_EXTENSION.load(Ordering::SeqCst));
}

fn _check_kinds() {
    fn is_send<T: Send>() {}
    fn is_sync<T: Sync>() {}

    is_send::<SslStream<TcpStream>>();
    is_sync::<SslStream<TcpStream>>();
}

#[test]
#[cfg(ossl111)]
fn stateless() {
    use super::SslOptions;

    #[derive(Debug)]
    struct MemoryStream {
        incoming: io::Cursor<Vec<u8>>,
        outgoing: Vec<u8>,
    }

    impl MemoryStream {
        pub fn new() -> Self {
            Self {
                incoming: io::Cursor::new(Vec::new()),
                outgoing: Vec::new(),
            }
        }

        pub fn extend_incoming(&mut self, data: &[u8]) {
            self.incoming.get_mut().extend_from_slice(data);
        }

        pub fn take_outgoing(&mut self) -> Outgoing<'_> {
            Outgoing(&mut self.outgoing)
        }
    }

    impl Read for MemoryStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.incoming.read(buf)?;
            if self.incoming.position() == self.incoming.get_ref().len() as u64 {
                self.incoming.set_position(0);
                self.incoming.get_mut().clear();
            }
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "no data available",
                ));
            }
            Ok(n)
        }
    }

    impl Write for MemoryStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.outgoing.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    pub struct Outgoing<'a>(&'a mut Vec<u8>);

    impl Drop for Outgoing<'_> {
        fn drop(&mut self) {
            self.0.clear();
        }
    }

    impl ::std::ops::Deref for Outgoing<'_> {
        type Target = [u8];
        fn deref(&self) -> &[u8] {
            self.0
        }
    }

    impl AsRef<[u8]> for Outgoing<'_> {
        fn as_ref(&self) -> &[u8] {
            self.0
        }
    }

    fn send(from: &mut MemoryStream, to: &mut MemoryStream) {
        to.extend_incoming(&from.take_outgoing());
    }

    //
    // Setup
    //

    let mut client_ctx = SslContext::builder(SslMethod::tls()).unwrap();
    client_ctx.clear_options(SslOptions::ENABLE_MIDDLEBOX_COMPAT);
    let mut client_stream =
        SslStream::new(Ssl::new(&client_ctx.build()).unwrap(), MemoryStream::new()).unwrap();

    let mut server_ctx = SslContext::builder(SslMethod::tls()).unwrap();
    server_ctx
        .set_certificate_file(Path::new("test/cert.pem"), SslFiletype::PEM)
        .unwrap();
    server_ctx
        .set_private_key_file(Path::new("test/key.pem"), SslFiletype::PEM)
        .unwrap();
    const COOKIE: &[u8] = b"chocolate chip";
    server_ctx.set_stateless_cookie_generate_cb(|_tls, buf| {
        buf[0..COOKIE.len()].copy_from_slice(COOKIE);
        Ok(COOKIE.len())
    });
    server_ctx.set_stateless_cookie_verify_cb(|_tls, buf| buf == COOKIE);
    let mut server_stream =
        SslStream::new(Ssl::new(&server_ctx.build()).unwrap(), MemoryStream::new()).unwrap();

    //
    // Handshake
    //

    // Initial ClientHello
    client_stream.connect().unwrap_err();
    send(client_stream.get_mut(), server_stream.get_mut());
    // HelloRetryRequest
    assert!(!server_stream.stateless().unwrap());
    send(server_stream.get_mut(), client_stream.get_mut());
    // Second ClientHello
    client_stream.do_handshake().unwrap_err();
    send(client_stream.get_mut(), server_stream.get_mut());
    // OldServerHello
    assert!(server_stream.stateless().unwrap());
    server_stream.accept().unwrap_err();
    send(server_stream.get_mut(), client_stream.get_mut());
    // Finished
    client_stream.do_handshake().unwrap();
    send(client_stream.get_mut(), server_stream.get_mut());
    server_stream.do_handshake().unwrap();
}

#[cfg(not(osslconf = "OPENSSL_NO_PSK"))]
#[test]
fn psk_ciphers() {
    const CIPHER: &str = "PSK-AES256-CBC-SHA";
    const PSK: &[u8] = b"thisisaverysecurekey";
    const CLIENT_IDENT: &[u8] = b"thisisaclient";
    static CLIENT_CALLED: AtomicBool = AtomicBool::new(false);
    static SERVER_CALLED: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server.ctx().set_cipher_list(CIPHER).unwrap();
    server.ctx().set_psk_server_callback(|_, identity, psk| {
        assert!(identity.unwrap_or(&[]) == CLIENT_IDENT);
        psk[..PSK.len()].copy_from_slice(PSK);
        SERVER_CALLED.store(true, Ordering::SeqCst);
        Ok(PSK.len())
    });

    let server = server.build();

    let mut client = server.client();
    // This test relies on TLS 1.2 suites
    #[cfg(any(boringssl, ossl111, awslc))]
    client.ctx().set_options(super::SslOptions::NO_TLSV1_3);
    client.ctx().set_cipher_list(CIPHER).unwrap();
    client
        .ctx()
        .set_psk_client_callback(move |_, _, identity, psk| {
            identity[..CLIENT_IDENT.len()].copy_from_slice(CLIENT_IDENT);
            identity[CLIENT_IDENT.len()] = 0;
            psk[..PSK.len()].copy_from_slice(PSK);
            CLIENT_CALLED.store(true, Ordering::SeqCst);
            Ok(PSK.len())
        });

    client.connect();

    assert!(SERVER_CALLED.load(Ordering::SeqCst));
    assert!(CLIENT_CALLED.load(Ordering::SeqCst));
}

#[test]
fn sni_callback_swapped_ctx() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();

    let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
    ctx.set_servername_callback(|_, _| {
        CALLED_BACK.store(true, Ordering::SeqCst);
        Ok(())
    });

    let keyed_ctx = mem::replace(server.ctx(), ctx).build();
    server.ssl_cb(move |ssl| ssl.set_ssl_context(&keyed_ctx).unwrap());

    let server = server.build();

    server.client().connect();

    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
#[cfg(ossl111)]
fn client_hello() {
    static CALLED_BACK: AtomicBool = AtomicBool::new(false);

    let mut server = Server::builder();
    server.ctx().set_client_hello_callback(|ssl, _| {
        assert!(!ssl.client_hello_isv2());
        assert_eq!(ssl.client_hello_legacy_version(), Some(SslVersion::TLS1_2));
        assert!(ssl.client_hello_random().is_some());
        assert!(ssl.client_hello_session_id().is_some());
        assert!(ssl.client_hello_ciphers().is_some());
        assert!(ssl.client_hello_compression_methods().is_some());
        assert!(ssl
            .bytes_to_cipher_list(ssl.client_hello_ciphers().unwrap(), ssl.client_hello_isv2())
            .is_ok());

        CALLED_BACK.store(true, Ordering::SeqCst);
        Ok(ClientHelloResponse::SUCCESS)
    });

    let server = server.build();
    server.client().connect();

    assert!(CALLED_BACK.load(Ordering::SeqCst));
}

#[test]
#[cfg(ossl111)]
fn openssl_cipher_name() {
    assert_eq!(
        super::cipher_name("TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA384"),
        "ECDHE-RSA-AES256-SHA384",
    );

    assert_eq!(super::cipher_name("asdf"), "(NONE)");
}

#[test]
fn session_cache_size() {
    let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
    ctx.set_session_cache_size(1234);
    let ctx = ctx.build();
    assert_eq!(ctx.session_cache_size(), 1234);
}

#[test]
#[cfg(ossl102)]
fn add_chain_cert() {
    let ctx = SslContext::builder(SslMethod::tls()).unwrap().build();
    let cert = X509::from_pem(CERT).unwrap();
    let mut ssl = Ssl::new(&ctx).unwrap();
    assert!(ssl.add_chain_cert(cert).is_ok());
}
#[test]
#[cfg(ossl111)]
fn set_ssl_certificate_key_related_api() {
    let cert_str: &str = include_str!("../../../test/cert.pem");
    let key_str: &str = include_str!("../../../test/key.pem");
    let ctx = SslContext::builder(SslMethod::tls()).unwrap().build();
    let cert_x509 = X509::from_pem(CERT).unwrap();
    let mut ssl = Ssl::new(&ctx).unwrap();
    assert!(ssl.set_method(SslMethod::tls()).is_ok());
    ssl.set_private_key_file("test/key.pem", SslFiletype::PEM)
        .unwrap();
    {
        let pkey = String::from_utf8(
            ssl.private_key()
                .unwrap()
                .private_key_to_pem_pkcs8()
                .unwrap(),
        )
        .unwrap();
        assert!(pkey.lines().eq(key_str.lines()));
    }
    let pkey = PKey::private_key_from_pem(KEY).unwrap();
    ssl.set_private_key(pkey.as_ref()).unwrap();
    {
        let pkey = String::from_utf8(
            ssl.private_key()
                .unwrap()
                .private_key_to_pem_pkcs8()
                .unwrap(),
        )
        .unwrap();
        assert!(pkey.lines().eq(key_str.lines()));
    }
    ssl.set_certificate(cert_x509.as_ref()).unwrap();
    let cert = String::from_utf8(ssl.certificate().unwrap().to_pem().unwrap()).unwrap();
    assert!(cert.lines().eq(cert_str.lines()));
    ssl.add_client_ca(cert_x509.as_ref()).unwrap();
    ssl.set_min_proto_version(Some(SslVersion::TLS1_2)).unwrap();
    ssl.set_max_proto_version(Some(SslVersion::TLS1_3)).unwrap();
    ssl.set_cipher_list("HIGH:!aNULL:!MD5").unwrap();
    ssl.set_ciphersuites("TLS_AES_128_GCM_SHA256").unwrap();
    let x509 = X509::from_pem(ROOT_CERT).unwrap();
    let mut builder = X509StoreBuilder::new().unwrap();
    builder.add_cert(x509).unwrap();
    let store = builder.build();
    ssl.set_verify_cert_store(store).unwrap();
}

#[test]
#[cfg(ossl110)]
fn test_ssl_set_cert_chain_file() {
    let ctx = SslContext::builder(SslMethod::tls()).unwrap().build();
    let mut ssl = Ssl::new(&ctx).unwrap();
    ssl.set_certificate_chain_file("test/cert.pem").unwrap();
}

#[test]
#[cfg(ossl111)]
fn set_num_tickets() {
    let mut ctx = SslContext::builder(SslMethod::tls_server()).unwrap();
    ctx.set_num_tickets(3).unwrap();
    let ctx = ctx.build();
    assert_eq!(3, ctx.num_tickets());

    let mut ssl = Ssl::new(&ctx).unwrap();
    ssl.set_num_tickets(5).unwrap();
    let ssl = ssl;
    assert_eq!(5, ssl.num_tickets());
}

#[test]
#[cfg(ossl110)]
fn set_security_level() {
    let mut ctx = SslContext::builder(SslMethod::tls_server()).unwrap();
    ctx.set_security_level(3);
    let ctx = ctx.build();
    assert_eq!(3, ctx.security_level());

    let mut ssl = Ssl::new(&ctx).unwrap();
    ssl.set_security_level(4);
    let ssl = ssl;
    assert_eq!(4, ssl.security_level());
}

#[test]
fn ssl_ctx_ex_data_leak() {
    static DROPS: AtomicUsize = AtomicUsize::new(0);

    struct DropTest;

    impl Drop for DropTest {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::Relaxed);
        }
    }

    let idx = SslContext::new_ex_index().unwrap();

    let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
    ctx.set_ex_data(idx, DropTest);
    ctx.set_ex_data(idx, DropTest);
    assert_eq!(DROPS.load(Ordering::Relaxed), 1);

    drop(ctx);
    assert_eq!(DROPS.load(Ordering::Relaxed), 2);
}

#[test]
fn ssl_ex_data_leak() {
    static DROPS: AtomicUsize = AtomicUsize::new(0);

    struct DropTest;

    impl Drop for DropTest {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::Relaxed);
        }
    }

    let idx = Ssl::new_ex_index().unwrap();

    let ctx = SslContext::builder(SslMethod::tls()).unwrap().build();
    let mut ssl = Ssl::new(&ctx).unwrap();
    ssl.set_ex_data(idx, DropTest);
    ssl.set_ex_data(idx, DropTest);
    assert_eq!(DROPS.load(Ordering::Relaxed), 1);

    drop(ssl);
    assert_eq!(DROPS.load(Ordering::Relaxed), 2);
}
