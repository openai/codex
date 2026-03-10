//! Test-only support for spawned login probe binaries.
//!
//! This module exists because `login_ca_probe` is compiled as a separate binary target, so it
//! cannot call crate-private helpers directly. Keeping the probe entry point under a hidden module
//! avoids surfacing it as part of the normal `codex-login` public API while still letting the
//! subprocess tests share the real custom-CA client-construction code. It is intentionally not a
//! general-purpose login API: the functions here exist only so the subprocess tests can exercise
//! CA loading in a separate process without duplicating logic in the probe binary.

use crate::BuildLoginHttpClientError;

/// Builds the login HTTP client for the subprocess CA probe tests.
///
/// The probe disables reqwest proxy autodetection so it can exercise custom-CA success and
/// failure in macOS seatbelt runs without tripping the known `system-configuration` panic during
/// platform proxy discovery. This is intentionally not the main public login entry point: normal
/// login callers should continue to use [`crate::build_login_http_client`]. A non-test caller that
/// reached for this helper would mask real proxy behavior and risk debugging a code path that does
/// not match production login.
pub fn build_login_http_client() -> Result<reqwest::Client, BuildLoginHttpClientError> {
    crate::custom_ca::build_login_http_client_for_subprocess_tests()
}
