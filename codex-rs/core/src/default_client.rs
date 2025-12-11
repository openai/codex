use crate::default_client_config::TlsConfig;
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use reqwest::header::HeaderValue;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::OnceLock;

use codex_client::CodexHttpClient;
pub use codex_client::CodexRequestBuilder;

/// Set this to add a suffix to the User-Agent string.
///
/// It is not ideal that we're using a global singleton for this.
/// This is primarily designed to differentiate MCP clients from each other.
/// Because there can only be one MCP server per process, it should be safe for this to be a global static.
/// However, future users of this should use this with caution as a result.
/// In addition, we want to be confident that this value is used for ALL clients and doing that requires a
/// lot of wiring and it's easy to miss code paths by doing so.
/// See https://github.com/openai/codex/pull/3388/files for an example of what that would look like.
/// Finally, we want to make sure this is set for ALL mcp clients without needing to know a special env var
/// or having to set data that they already specified in the mcp initialize request somewhere else.
///
/// A space is automatically added between the suffix and the rest of the User-Agent string.
/// The full user agent string is returned from the mcp initialize response.
/// Parenthesis will be added by Codex. This should only specify what goes inside of the parenthesis.
pub static USER_AGENT_SUFFIX: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));
pub const DEFAULT_ORIGINATOR: &str = "codex_cli_rs";
pub const CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR: &str = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";

#[derive(Debug, Clone)]
pub struct Originator {
    pub value: String,
    pub header_value: HeaderValue,
}
static ORIGINATOR: OnceLock<Originator> = OnceLock::new();

#[derive(Debug)]
pub enum SetOriginatorError {
    InvalidHeaderValue,
    AlreadyInitialized,
}

fn get_originator_value(provided: Option<String>) -> Originator {
    let value = std::env::var(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR)
        .ok()
        .or(provided)
        .unwrap_or(DEFAULT_ORIGINATOR.to_string());

    match HeaderValue::from_str(&value) {
        Ok(header_value) => Originator {
            value,
            header_value,
        },
        Err(e) => {
            tracing::error!("Unable to turn originator override {value} into header value: {e}");
            Originator {
                value: DEFAULT_ORIGINATOR.to_string(),
                header_value: HeaderValue::from_static(DEFAULT_ORIGINATOR),
            }
        }
    }
}

pub fn set_default_originator(value: String) -> Result<(), SetOriginatorError> {
    let originator = get_originator_value(Some(value));
    ORIGINATOR
        .set(originator)
        .map_err(|_| SetOriginatorError::AlreadyInitialized)
}

pub fn originator() -> &'static Originator {
    ORIGINATOR.get_or_init(|| get_originator_value(None))
}

pub fn get_codex_user_agent() -> String {
    let build_version = env!("CARGO_PKG_VERSION");
    let os_info = os_info::get();
    let prefix = format!(
        "{}/{build_version} ({} {}; {}) {}",
        originator().value.as_str(),
        os_info.os_type(),
        os_info.version(),
        os_info.architecture().unwrap_or("unknown"),
        crate::terminal::user_agent()
    );
    let suffix = USER_AGENT_SUFFIX
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let suffix = suffix
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or_else(String::new, |value| format!(" ({value})"));

    let candidate = format!("{prefix}{suffix}");
    sanitize_user_agent(candidate, &prefix)
}

/// Sanitize the user agent string.
///
/// Invalid characters are replaced with an underscore.
///
/// If the user agent fails to parse, it falls back to fallback and then to ORIGINATOR.
fn sanitize_user_agent(candidate: String, fallback: &str) -> String {
    if HeaderValue::from_str(candidate.as_str()).is_ok() {
        return candidate;
    }

    let sanitized: String = candidate
        .chars()
        .map(|ch| if matches!(ch, ' '..='~') { ch } else { '_' })
        .collect();
    if !sanitized.is_empty() && HeaderValue::from_str(sanitized.as_str()).is_ok() {
        tracing::warn!(
            "Sanitized Codex user agent because provided suffix contained invalid header characters"
        );
        sanitized
    } else if HeaderValue::from_str(fallback).is_ok() {
        tracing::warn!(
            "Falling back to base Codex user agent because provided suffix could not be sanitized"
        );
        fallback.to_string()
    } else {
        tracing::warn!(
            "Falling back to default Codex originator because base user agent string is invalid"
        );
        originator().value.clone()
    }
}

/// Create an HTTP client with default `originator` and `User-Agent` headers set.
pub fn create_client() -> CodexHttpClient {
    let inner = build_reqwest_client();
    CodexHttpClient::new(inner)
}

/// Create an HTTP client with optional TLS configuration.
/// Optionally configure TLS/mTLS settings via the `tls_config` parameter.
pub fn create_configured_client(tls_config: Option<&TlsConfig>) -> CodexHttpClient {
    let inner = build_configured_reqwest_client(tls_config);
    CodexHttpClient::new(inner)
}

pub fn build_reqwest_client() -> reqwest::Client {
    build_configured_reqwest_client(None)
}

pub fn build_configured_reqwest_client(tls_config: Option<&TlsConfig>) -> reqwest::Client {
    let builder = create_base_client_builder();

    // Apply TLS configuration if provided
    let builder = if let Some(tls) = tls_config {
        match apply_tls_config(builder, tls) {
            Ok(configured_builder) => configured_builder,
            Err(e) => {
                tracing::error!("Failed to apply TLS configuration: {}", e);
                // Fall back to base builder without TLS
                create_base_client_builder()
            }
        }
    } else {
        builder
    };

    builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

/// Create the base HTTP client builder with standard configuration.
fn create_base_client_builder() -> reqwest::ClientBuilder {
    use reqwest::header::HeaderMap;

    let mut headers = HeaderMap::new();
    headers.insert("originator", originator().header_value.clone());
    let ua = get_codex_user_agent();

    let mut builder = reqwest::Client::builder()
        .user_agent(ua)
        .default_headers(headers);

    if is_sandboxed() {
        builder = builder.no_proxy();
    }

    builder
}

/// Apply TLS configuration to a reqwest ClientBuilder.
fn apply_tls_config(
    mut builder: reqwest::ClientBuilder,
    tls: &TlsConfig,
) -> Result<reqwest::ClientBuilder, String> {
    use reqwest::Certificate;
    use reqwest::Identity;

    // Add custom CA certificate if provided
    if let Some(ca_path) = &tls.ca_certificate {
        let cert_pem = std::fs::read(ca_path).map_err(|e| {
            format!(
                "Failed to read CA certificate from {}: {}",
                ca_path.display(),
                e
            )
        })?;

        let certificate = Certificate::from_pem(&cert_pem).map_err(|e| {
            format!(
                "Failed to parse CA certificate from {}: {}",
                ca_path.display(),
                e
            )
        })?;

        // Disable built-in root certificates and use only our custom CA
        builder = builder
            .tls_built_in_root_certs(false)
            .add_root_certificate(certificate);
    }

    // Configure client certificate and private key for mTLS
    match (&tls.client_certificate, &tls.client_private_key) {
        (Some(cert_path), Some(key_path)) => {
            // Read cert and key files
            let cert_pem = std::fs::read(cert_path).map_err(|e| {
                format!(
                    "Failed to read client certificate from {}: {}",
                    cert_path.display(),
                    e
                )
            })?;
            let key_pem = std::fs::read(key_path).map_err(|e| {
                format!(
                    "Failed to read client private key from {}: {}",
                    key_path.display(),
                    e
                )
            })?;

            // For rustls, Identity::from_pem() accepts combined cert+key PEM data
            let mut combined_pem = cert_pem;
            combined_pem.extend_from_slice(&key_pem);

            let identity = Identity::from_pem(&combined_pem).map_err(|e| {
                format!(
                    "Failed to create client identity from {} and {}: {}",
                    cert_path.display(),
                    key_path.display(),
                    e
                )
            })?;

            builder = builder.identity(identity).https_only(true);
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(
                "client_certificate and client_private_key must both be provided for mTLS"
                    .to_string(),
            );
        }
        (None, None) => {
            // No client certificate configured
        }
    }

    Ok(builder)
}

fn is_sandboxed() -> bool {
    std::env::var(CODEX_SANDBOX_ENV_VAR).as_deref() == Ok("seatbelt")
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_test_support::skip_if_no_network;

    #[test]
    fn test_get_codex_user_agent() {
        let user_agent = get_codex_user_agent();
        assert!(user_agent.starts_with("codex_cli_rs/"));
    }

    #[tokio::test]
    async fn test_create_client_sets_default_headers() {
        skip_if_no_network!();

        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let client = create_client();

        // Spin up a local mock server and capture a request.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let resp = client
            .get(server.uri())
            .send()
            .await
            .expect("failed to send request");
        assert!(resp.status().is_success());

        let requests = server
            .received_requests()
            .await
            .expect("failed to fetch received requests");
        assert!(!requests.is_empty());
        let headers = &requests[0].headers;

        // originator header is set to the provided value
        let originator_header = headers
            .get("originator")
            .expect("originator header missing");
        assert_eq!(originator_header.to_str().unwrap(), "codex_cli_rs");

        // User-Agent matches the computed Codex UA for that originator
        let expected_ua = get_codex_user_agent();
        let ua_header = headers
            .get("user-agent")
            .expect("user-agent header missing");
        assert_eq!(ua_header.to_str().unwrap(), expected_ua);
    }

    #[test]
    fn test_invalid_suffix_is_sanitized() {
        let prefix = "codex_cli_rs/0.0.0";
        let suffix = "bad\rsuffix";

        assert_eq!(
            sanitize_user_agent(format!("{prefix} ({suffix})"), prefix),
            "codex_cli_rs/0.0.0 (bad_suffix)"
        );
    }

    #[test]
    fn test_invalid_suffix_is_sanitized2() {
        let prefix = "codex_cli_rs/0.0.0";
        let suffix = "bad\0suffix";

        assert_eq!(
            sanitize_user_agent(format!("{prefix} ({suffix})"), prefix),
            "codex_cli_rs/0.0.0 (bad_suffix)"
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos() {
        use regex_lite::Regex;
        let user_agent = get_codex_user_agent();
        let re = Regex::new(
            r"^codex_cli_rs/\d+\.\d+\.\d+ \(Mac OS \d+\.\d+\.\d+; (x86_64|arm64)\) (\S+)$",
        )
        .unwrap();
        assert!(re.is_match(&user_agent));
    }
}
