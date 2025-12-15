use assert_cmd::cargo::CommandCargoExt;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const CODEX_CA_CERT_ENV: &str = "CODEX_CA_CERTIFICATE";
const SSL_CERT_FILE_ENV: &str = "SSL_CERT_FILE";

const TEST_CERT_1: &str = "-----BEGIN CERTIFICATE-----
MIIDBTCCAe2gAwIBAgIUZYhGvBUG7SucNzYh9VIeZ7b9zHowDQYJKoZIhvcNAQEL
BQAwEjEQMA4GA1UEAwwHdGVzdC1jYTAeFw0yNTEyMTEyMzEyNTFaFw0zNTEyMDky
MzEyNTFaMBIxEDAOBgNVBAMMB3Rlc3QtY2EwggEiMA0GCSqGSIb3DQEBAQUAA4IB
DwAwggEKAoIBAQC+NJRZAdn15FFBN8eR1HTAe+LMVpO19kKtiCsQjyqHONfhfHcF
7zQfwmH6MqeNpC/5k5m8V1uSIhyHBskQm83Jv8/vHlffNxE/hl0Na/Yd1bc+2kxH
twIAsF32GKnSKnFva/iGczV81+/ETgG6RXfTfy/Xs6fXL8On8SRRmTcMw0bEfwko
ziid87VOHg2JfdRKN5QpS9lvQ8q4q2M3jMftolpUTpwlR0u8j9OXnZfn+ja33X0l
kjkoCbXE2fVbAzO/jhUHQX1H5RbTGGUnrrCWAj84Rq/E80KK1nrRF91K+vgZmilM
gOZosLMMI1PeqTakwg1yIRngpTyk0eJP+haxAgMBAAGjUzBRMB0GA1UdDgQWBBT6
sqvfjMIl0DFZkeu8LU577YqMVDAfBgNVHSMEGDAWgBT6sqvfjMIl0DFZkeu8LU57
7YqMVDAPBgNVHRMBAf8EBTADAQH/MA0GCSqGSIb3DQEBCwUAA4IBAQBQ1sYs2RvB
TZ+xSBglLwH/S7zXVJIDwQ23Rlj11dgnVvcilSJCX24Rr+pfIVLpYNDdZzc/DIJd
S1dt2JuLnvXnle29rU7cxuzYUkUkRtaeY2Sj210vsE3lqUFyIy8XCc/lteb+FiJ7
zo/gPk7P+y4ihK9Mm6SBqkDVEYSFSn9bgoemK+0e93jGe2182PyuTwfTmZgENSBO
2f9dSuay4C7e5UO8bhVccQJg6f4d70zUNG0oPHrnVxJLjwCd++jx25Gh4U7+ek13
CW57pxJrpPMDWb2YK64rT2juHMKF73YuplW92SInd+QLpI2ekTLc+bRw8JvqzXg+
SprtRUBjlWzj
-----END CERTIFICATE-----
";

const TEST_CERT_2: &str = "-----BEGIN CERTIFICATE-----
MIIDGTCCAgGgAwIBAgIUWxlcvHzwITWAHWHbKMFUTgeDmjwwDQYJKoZIhvcNAQEL
BQAwHDEaMBgGA1UEAwwRdGVzdC1pbnRlcm1lZGlhdGUwHhcNMjUxMTE5MTU1MDIz
WhcNMjYxMTE5MTU1MDIzWjAcMRowGAYDVQQDDBF0ZXN0LWludGVybWVkaWF0ZTCC
ASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBANq7xbeYpC2GaXANqD1nLk0t
j9j2sOk6e7DqTapxnIUijS7z4DF0Vo1xHM07wK1m+wsB/t9CubNYRvtn6hrIzx7K
jjlmvxo4/YluwO1EDMQWZAXkaY2O28ESKVx7QLfBPYAc4bf/5B4Nmt6KX5sQyyyH
2qTfzVBUCAl3sI+Ydd3mx7NOye1yNNkCNqyK3Hj45F1JuH8NZxcb4OlKssZhMlD+
EQx4G46AzKE9Ho8AqlQvg/tiWrMHRluw7zolMJ/AXzedAXedNIrX4fCOmZwcTkA1
a8eLPP8oM9VFrr67a7on6p4zPqugUEQ4fawp7A5KqSjUAVCt1FXmn2V8N8V6W/sC
AwEAAaNTMFEwHQYDVR0OBBYEFBEwRwW0gm3IjhLw1U3eOAvR0r6SMB8GA1UdIwQY
MBaAFBEwRwW0gm3IjhLw1U3eOAvR0r6SMA8GA1UdEwEB/wQFMAMBAf8wDQYJKoZI
hvcNAQELBQADggEBAB2fjAlpevK42Odv8XUEgV6VWlEP9HAmkRvugW9hjhzx1Iz9
Vh/l9VcxL7PcqdpyGH+BIRvQIMokcYF5TXzf/KV1T2y56U8AWaSd2/xSjYNWwkgE
TLE5V+H/YDKzvTe58UrOaxa5N3URscQL9f+ZKworODmfMlkJ1mlREK130ZMlBexB
p9w5wo1M1fjx76Rqzq9MkpwBSbIO2zx/8+qy4BAH23MPGW+9OOnnq2DiIX3qUu1v
hnjYOxYpCB28MZEJmqsjFJQQ9RF+Te4U2/oknVcf8lZIMJ2ZBOwt2zg8RqCtM52/
IbATwYj77wg3CFLFKcDYs3tdUqpiniabKcf6zAs=
-----END CERTIFICATE-----
";

fn write_cert_file(temp_dir: &TempDir, name: &str, contents: &str) -> std::path::PathBuf {
    let path = temp_dir.path().join(name);
    fs::write(&path, contents).unwrap_or_else(|error| {
        panic!("write cert fixture failed for {}: {error}", path.display())
    });
    path
}

fn run_probe(envs: &[(&str, &Path)]) -> std::process::Output {
    let mut cmd = Command::cargo_bin("login_ca_probe")
        .unwrap_or_else(|error| panic!("failed to locate login_ca_probe: {error}"));
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
fn accepts_trusted_certificate_label() {
    let temp_dir = TempDir::new().expect("tempdir");
    let trusted = TEST_CERT_1
        .replace("BEGIN CERTIFICATE", "BEGIN TRUSTED CERTIFICATE")
        .replace("END CERTIFICATE", "END TRUSTED CERTIFICATE");
    let cert_path = write_cert_file(&temp_dir, "trusted.pem", &trusted);

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
