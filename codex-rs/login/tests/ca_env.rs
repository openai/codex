//! Subprocess coverage for custom CA behavior that must build a real reqwest client.
//!
//! These tests intentionally run through `login_ca_probe` instead of calling the helper in-process:
//! reqwest client construction is not hermetic on macOS sandboxed runs, and these cases also need
//! exact control over inherited CA environment variables. The probe disables reqwest proxy
//! autodetection because `reqwest::Client::builder().build()` can panic inside
//! `system-configuration` while probing macOS proxy settings under seatbelt. The probe-level
//! workaround keeps these tests focused on custom-CA success and failure instead of failing first
//! on unrelated platform proxy discovery. These tests still stop at client construction: they
//! verify CA file selection, PEM parsing, and user-facing errors, not a full TLS handshake.

use codex_utils_cargo_bin::cargo_bin;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const CODEX_CA_CERT_ENV: &str = "CODEX_CA_CERTIFICATE";
const SSL_CERT_FILE_ENV: &str = "SSL_CERT_FILE";

const TEST_CERT_1: &str = include_str!("fixtures/test-ca.pem");
const TEST_CERT_2: &str = include_str!("fixtures/test-intermediate.pem");
const TRUSTED_TEST_CERT: &str = include_str!("fixtures/test-ca-trusted.pem");

fn write_cert_file(temp_dir: &TempDir, name: &str, contents: &str) -> std::path::PathBuf {
    let path = temp_dir.path().join(name);
    fs::write(&path, contents).unwrap_or_else(|error| {
        panic!("write cert fixture failed for {}: {error}", path.display())
    });
    path
}

fn run_probe(envs: &[(&str, &Path)]) -> std::process::Output {
    let mut cmd = Command::new(
        cargo_bin("login_ca_probe")
            .unwrap_or_else(|error| panic!("failed to locate login_ca_probe: {error}")),
    );
    // `Command` inherits the parent environment by default, so scrub CA-related variables first or
    // these tests can accidentally pass/fail based on the developer shell or CI runner.
    cmd.env_remove(CODEX_CA_CERT_ENV);
    cmd.env_remove(SSL_CERT_FILE_ENV);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output()
        .unwrap_or_else(|error| panic!("failed to run login_ca_probe: {error}"))
}

#[test]
fn uses_codex_ca_cert_env() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "ca.pem", TEST_CERT_1);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn falls_back_to_ssl_cert_file() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "ssl.pem", TEST_CERT_1);

    let output = run_probe(&[(SSL_CERT_FILE_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn prefers_codex_ca_cert_over_ssl_cert_file() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "ca.pem", TEST_CERT_1);
    let bad_path = write_cert_file(&temp_dir, "bad.pem", "");

    let output = run_probe(&[
        (CODEX_CA_CERT_ENV, cert_path.as_path()),
        (SSL_CERT_FILE_ENV, bad_path.as_path()),
    ]);

    assert!(output.status.success());
}

#[test]
fn handles_multi_certificate_bundle() {
    let temp_dir = TempDir::new().expect("tempdir");
    let bundle = format!("{TEST_CERT_1}\n{TEST_CERT_2}");
    let cert_path = write_cert_file(&temp_dir, "bundle.pem", &bundle);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn rejects_empty_pem_file_with_hint() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "empty.pem", "");

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no certificates found in PEM file"));
    assert!(stderr.contains("CODEX_CA_CERTIFICATE"));
    assert!(stderr.contains("SSL_CERT_FILE"));
}

#[test]
fn rejects_malformed_pem_with_hint() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(
        &temp_dir,
        "malformed.pem",
        "-----BEGIN CERTIFICATE-----\nMIIBroken",
    );

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to parse PEM file"));
    assert!(stderr.contains("CODEX_CA_CERTIFICATE"));
    assert!(stderr.contains("SSL_CERT_FILE"));
}

#[test]
fn accepts_openssl_trusted_certificate() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "trusted.pem", TRUSTED_TEST_CERT);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn accepts_bundle_with_crl() {
    let temp_dir = TempDir::new().expect("tempdir");
    let crl = "-----BEGIN X509 CRL-----\nMIIC\n-----END X509 CRL-----";
    let bundle = format!("{TEST_CERT_1}\n{crl}");
    let cert_path = write_cert_file(&temp_dir, "bundle_crl.pem", &bundle);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}
