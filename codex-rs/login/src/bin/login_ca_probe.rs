//! Helper binary for exercising custom CA environment handling in tests.
//!
//! The login flows honor `CODEX_CA_CERTIFICATE` and `SSL_CERT_FILE`, but those environment
//! variables are process-global and unsafe to mutate in parallel test execution. This probe keeps
//! the behavior under test while letting integration tests (`tests/ca_env.rs`) set env vars
//! per-process, proving:
//!
//! - env precedence is respected,
//! - multi-cert PEM bundles load,
//! - error messages guide users when CA files are invalid.
//!
//! The probe intentionally disables reqwest proxy autodetection while building the client. That
//! keeps the subprocess tests hermetic in macOS seatbelt runs, where
//! `reqwest::Client::builder().build()` can panic inside the `system-configuration` crate while
//! probing macOS proxy settings. Without that workaround, the subprocess exits before the custom
//! CA code reports either success or a structured `BuildLoginHttpClientError`, so tests that are
//! supposed to validate CA parsing instead fail on unrelated platform proxy discovery.

use std::process;

fn main() {
    match codex_login::probe_support::build_login_http_client() {
        Ok(_) => {
            println!("ok");
        }
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    }
}
