//! Default Codex HTTP client: shared `User-Agent`, `originator`, optional residency header, and
//! reqwest/`CodexHttpClient` construction.
//!
//! Use [`crate::default_client`] or [`codex_login::default_client`] from other crates in this
//! workspace.

use codex_client::BuildCustomCaTransportError;
use codex_client::CodexHttpClient;
pub use codex_client::CodexRequestBuilder;
use codex_client::build_reqwest_client_with_custom_ca;
use codex_client::with_chatgpt_cloudflare_cookie_store;
use codex_terminal_detection::user_agent;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::USER_AGENT;
use std::sync::LazyLock;
use std::sync::RwLock;
pub const DEFAULT_ORIGINATOR: &str = "codex_cli_rs";
pub const CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR: &str = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";
pub const RESIDENCY_HEADER_NAME: &str = "x-openai-internal-codex-residency";

pub use codex_config::ResidencyRequirement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Originator {
    kind: OriginatorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OriginatorKind {
    Process { value: String },
    AppServerClient { client: AppServerClient },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServerClient {
    name: String,
    version: String,
}

impl Originator {
    pub fn process_default() -> Self {
        let value = std::env::var(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR)
            .unwrap_or_else(|_| DEFAULT_ORIGINATOR.to_string());

        match Self::for_process(value.clone()) {
            Ok(originator) => originator,
            Err(e) => {
                tracing::error!(
                    "Unable to turn originator override {value} into header value: {e}"
                );
                Self::for_process(DEFAULT_ORIGINATOR.to_string())
                    .expect("default originator should be a valid HTTP header value")
            }
        }
    }

    pub fn for_process(value: String) -> Result<Self, InvalidOriginator> {
        validate_originator_value(&value)?;
        Ok(Self {
            kind: OriginatorKind::Process { value },
        })
    }

    pub fn from_app_server_client(
        name: String,
        version: String,
    ) -> Result<Self, InvalidOriginator> {
        validate_originator_value(&name)?;
        Ok(Self {
            kind: OriginatorKind::AppServerClient {
                client: AppServerClient { name, version },
            },
        })
    }

    pub fn value(&self) -> &str {
        match &self.kind {
            OriginatorKind::Process { value } => value,
            OriginatorKind::AppServerClient { client } => client.name(),
        }
    }

    pub fn app_server_client(&self) -> Option<&AppServerClient> {
        match &self.kind {
            OriginatorKind::Process { .. } => None,
            OriginatorKind::AppServerClient { client } => Some(client),
        }
    }
}

impl AppServerClient {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

static REQUIREMENTS_RESIDENCY: LazyLock<RwLock<Option<ResidencyRequirement>>> =
    LazyLock::new(|| RwLock::new(None));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidOriginator {
    InvalidHeaderValue,
}

impl std::fmt::Display for InvalidOriginator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHeaderValue => f.write_str("invalid HTTP header value"),
        }
    }
}

impl std::error::Error for InvalidOriginator {}

fn validate_originator_value(value: &str) -> Result<(), InvalidOriginator> {
    HeaderValue::from_str(value).map_err(|_| InvalidOriginator::InvalidHeaderValue)?;
    Ok(())
}

pub fn set_default_client_residency_requirement(enforce_residency: Option<ResidencyRequirement>) {
    let Ok(mut guard) = REQUIREMENTS_RESIDENCY.write() else {
        tracing::warn!("Failed to acquire requirements residency lock");
        return;
    };
    *guard = enforce_residency;
}

pub fn is_first_party_originator(originator_value: &str) -> bool {
    originator_value == DEFAULT_ORIGINATOR
        || originator_value == "codex-tui"
        || originator_value == "codex_vscode"
        || originator_value.starts_with("Codex ")
}

pub fn is_first_party_chat_originator(originator_value: &str) -> bool {
    originator_value == "codex_atlas" || originator_value == "codex_chatgpt_desktop"
}

pub fn get_codex_user_agent(originator: &Originator) -> String {
    let build_version = env!("CARGO_PKG_VERSION");
    let os_info = os_info::get();
    let prefix = format!(
        "{}/{build_version} ({} {}; {}) {}",
        originator.value(),
        os_info.os_type(),
        os_info.version(),
        os_info.architecture().unwrap_or("unknown"),
        user_agent()
    );
    let suffix = user_agent_suffix(originator)
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
        DEFAULT_ORIGINATOR.to_string()
    }
}

fn user_agent_suffix(originator: &Originator) -> Option<String> {
    originator
        .app_server_client()
        .map(|client| format!("{}; {}", client.name(), client.version()))
}

/// Create an HTTP client with default `originator` and `User-Agent` headers set.
pub fn create_client(originator: &Originator) -> CodexHttpClient {
    let inner = build_reqwest_client(originator);
    CodexHttpClient::new(inner)
}

/// Builds the default reqwest client used for ordinary Codex HTTP traffic.
///
/// This starts from the standard Codex user agent, default headers, and sandbox-specific proxy
/// policy, then layers in shared custom CA handling from `CODEX_CA_CERTIFICATE` /
/// `SSL_CERT_FILE`. The function remains infallible for compatibility with existing call sites, so
/// a custom-CA or builder failure is logged and falls back to `reqwest::Client::new()`.
pub fn build_reqwest_client(originator: &Originator) -> reqwest::Client {
    build_reqwest_client_with_headers(default_headers(originator))
}

fn build_reqwest_client_with_headers(headers: HeaderMap) -> reqwest::Client {
    try_build_reqwest_client_with_headers(headers).unwrap_or_else(|error| {
        tracing::warn!(error = %error, "failed to build default reqwest client");
        with_chatgpt_cloudflare_cookie_store(reqwest::Client::builder())
            .build()
            .unwrap_or_else(|fallback_error| {
                tracing::warn!(
                    error = %fallback_error,
                    "failed to build fallback reqwest client with ChatGPT Cloudflare cookie store"
                );
                reqwest::Client::new()
            })
    })
}

/// Tries to build the default reqwest client used for ordinary Codex HTTP traffic.
///
/// Callers that need a structured CA-loading failure instead of the legacy logged fallback can use
/// this method directly.
pub fn try_build_reqwest_client(
    originator: &Originator,
) -> Result<reqwest::Client, BuildCustomCaTransportError> {
    try_build_reqwest_client_with_headers(default_headers(originator))
}

fn try_build_reqwest_client_with_headers(
    headers: HeaderMap,
) -> Result<reqwest::Client, BuildCustomCaTransportError> {
    let mut builder = reqwest::Client::builder().default_headers(headers);
    if is_sandboxed() {
        builder = builder.no_proxy();
    }
    builder = with_chatgpt_cloudflare_cookie_store(builder);

    build_reqwest_client_with_custom_ca(builder)
}

pub fn default_headers(originator: &Originator) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let originator_header = HeaderValue::from_str(originator.value())
        .expect("originator should have been validated as a header value");
    headers.insert("originator", originator_header);
    let user_agent = get_codex_user_agent(originator);
    if let Ok(user_agent) = HeaderValue::from_str(&user_agent) {
        headers.insert(USER_AGENT, user_agent);
    }
    if let Ok(guard) = REQUIREMENTS_RESIDENCY.read()
        && let Some(requirement) = guard.as_ref()
        && !headers.contains_key(RESIDENCY_HEADER_NAME)
    {
        let value = match requirement {
            ResidencyRequirement::Us => HeaderValue::from_static("us"),
        };
        headers.insert(RESIDENCY_HEADER_NAME, value);
    }
    headers
}

fn is_sandboxed() -> bool {
    std::env::var("CODEX_SANDBOX").as_deref() == Ok("seatbelt")
}

#[cfg(test)]
#[path = "default_client_tests.rs"]
mod tests;
