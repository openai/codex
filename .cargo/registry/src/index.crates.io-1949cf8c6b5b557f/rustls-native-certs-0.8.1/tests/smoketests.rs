mod common;

use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, panic};

// #[serial] is used on all these tests to run them sequentially. If they're run in parallel,
// the global env var configuration in the env var test interferes with the others.
use serial_test::serial;

/// Check if connection to site works using native roots.
///
/// Yields an Err if and only if there is an issue connecting that
/// appears to be due to a certificate problem.
///
/// # Panics
///
/// Panics on errors unrelated to the TLS connection like errors during
/// certificate loading, or connecting via TCP.
fn check_site(domain: &str) -> Result<(), ()> {
    check_site_with_roots(domain, rustls_native_certs::load_native_certs().unwrap())
}

/// Check if connection to site works using the given roots.
///
/// Yields an Err if and only if there is an issue connecting that
/// appears to be due to a certificate problem.
///
/// # Panics
///
/// Panics on errors unrelated to the TLS connection like connecting via TCP.
fn check_site_with_roots(
    domain: &str,
    root_certs: Vec<pki_types::CertificateDer<'static>>,
) -> Result<(), ()> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(root_certs);

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    let mut conn = rustls::ClientConnection::new(
        Arc::new(config),
        pki_types::ServerName::try_from(domain)
            .unwrap()
            .to_owned(),
    )
    .unwrap();
    let mut sock = TcpStream::connect(format!("{}:443", domain)).unwrap();
    let mut tls = rustls::Stream::new(&mut conn, &mut sock);
    let result = tls.write_all(
        format!(
            "GET / HTTP/1.1\r\n\
                       Host: {}\r\n\
                       Connection: close\r\n\
                       Accept-Encoding: identity\r\n\
                       \r\n",
            domain
        )
        .as_bytes(),
    );
    match result {
        Ok(()) => (),
        Err(e) if e.kind() == ErrorKind::InvalidData => return Err(()), // TLS error
        Err(e) => panic!("{}", e),
    }
    let mut plaintext = [0u8; 1024];
    let len = tls.read(&mut plaintext).unwrap();
    assert!(plaintext[..len].starts_with(b"HTTP/1.1 ")); // or whatever
    Ok(())
}

#[test]
#[serial]
#[ignore]
fn google() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }
    check_site("google.com").unwrap();
}

#[test]
#[serial]
#[ignore]
fn amazon() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }
    check_site("amazon.com").unwrap();
}

#[test]
#[serial]
#[ignore]
fn facebook() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }
    check_site("facebook.com").unwrap();
}

#[test]
#[serial]
#[ignore]
fn netflix() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }
    check_site("netflix.com").unwrap();
}

#[test]
#[serial]
#[ignore]
fn ebay() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }
    check_site("ebay.com").unwrap();
}

#[test]
#[serial]
#[ignore]
fn apple() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }
    check_site("apple.com").unwrap();
}

#[test]
#[serial]
#[ignore]
fn badssl_with_env() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }

    // Self-signed certs should never be trusted by default:
    assert!(check_site("self-signed.badssl.com").is_err());

    // But they should be trusted if SSL_CERT_FILE is set:
    env::set_var(
        "SSL_CERT_FILE",
        // The CA cert, downloaded directly from the site itself:
        PathBuf::from("./tests/badssl-com-chain.pem"),
    );
    check_site("self-signed.badssl.com").unwrap();
}

#[test]
#[serial]
#[ignore]
fn badssl_with_dir_from_env() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }
    let temp_dir = tempfile::TempDir::new().unwrap();
    let original = Path::new("tests/badssl-com-chain.pem")
        .canonicalize()
        .unwrap();
    let link1 = temp_dir.path().join("5d30f3c5.3");
    #[cfg(unix)]
    let link2 = temp_dir.path().join("fd3003c5.0");

    env::set_var(
        "SSL_CERT_DIR",
        // The CA cert, downloaded directly from the site itself:
        temp_dir.path(),
    );
    assert!(check_site("self-signed.badssl.com").is_err());

    // OpenSSL uses symlinks too. So, use one for testing too, if possible.
    #[cfg(unix)]
    symlink(original, link1).unwrap();
    #[cfg(not(unix))]
    std::fs::copy(original, link1).unwrap();

    // Dangling symlink
    #[cfg(unix)]
    symlink("/a/path/which/does/not/exist/hopefully", link2).unwrap();

    check_site("self-signed.badssl.com").unwrap();
}

#[test]
#[serial]
#[ignore]
#[cfg(target_os = "linux")]
fn google_with_dir_but_broken_file() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }

    env::set_var("SSL_CERT_DIR", "/etc/ssl/certs");
    env::set_var("SSL_CERT_FILE", "not-exist");
    let res = rustls_native_certs::load_native_certs();

    let first_err = res.errors.first().unwrap().to_string();
    dbg!(&first_err);
    assert!(first_err.contains("from file"));
    assert!(first_err.contains("not-exist"));

    check_site_with_roots("google.com", res.certs).unwrap();
}

#[test]
#[serial]
#[ignore]
#[cfg(target_os = "linux")]
fn google_with_file_but_broken_dir() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }

    env::set_var("SSL_CERT_DIR", "/not-exist");
    env::set_var("SSL_CERT_FILE", "/etc/ssl/certs/ca-certificates.crt");
    let res = rustls_native_certs::load_native_certs();

    let first_err = res.errors.first().unwrap().to_string();
    dbg!(&first_err);
    assert!(first_err.contains("opening directory"));
    assert!(first_err.contains("/not-exist"));

    check_site_with_roots("google.com", res.certs).unwrap();
}

#[test]
#[serial]
#[ignore]
#[cfg(target_os = "linux")]
fn nothing_works_with_broken_file_and_dir() {
    unsafe {
        // SAFETY: safe because of #[serial]
        common::clear_env();
    }

    env::set_var("SSL_CERT_DIR", "/not-exist");
    env::set_var("SSL_CERT_FILE", "not-exist");
    let res = rustls_native_certs::load_native_certs();
    assert_eq!(res.errors.len(), 2);

    let first_err = res.errors.first().unwrap().to_string();
    dbg!(&first_err);
    assert!(first_err.contains("from file"));
    assert!(first_err.contains("not-exist"));

    let second_err = res.errors.get(1).unwrap().to_string();
    dbg!(&second_err);
    assert!(second_err.contains("opening directory"));
    assert!(second_err.contains("/not-exist"));
}
