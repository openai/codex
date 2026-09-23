//! Default Codex HTTP client: shared `User-Agent`, `originator`, optional residency header, and
//! HTTP client and transport construction.
//!
//! Use [`crate::default_client`] or [`codex_login::default_client`] from other crates in this
//! workspace.

use codex_http_client::BuildRouteAwareHttpClientError;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientBuilder;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
pub use codex_http_client::RequestBuilder as CodexRequestBuilder;
use codex_http_client::ReqwestTransport;
use codex_http_client::RouteAwareClientPool;
use codex_terminal_detection::user_agent;
use http::HeaderMap;
use http::HeaderValue;
use http::header::USER_AGENT;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::RwLock;

use crate::outbound_proxy::AuthRouteConfig;

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
pub use codex_model_provider_info::RESIDENCY_HEADER_NAME;
pub use codex_model_provider_info::ResidencyRequirement;
pub use codex_model_provider_info::read_managed_residency_requirement as read_default_client_residency_requirement;
pub use codex_model_provider_info::set_managed_residency_requirement as set_default_client_residency_requirement;

#[derive(Debug, Clone)]
pub struct Originator {
    pub value: String,
    pub header_value: HeaderValue,
}
static ORIGINATOR: LazyLock<RwLock<Option<Originator>>> = LazyLock::new(|| RwLock::new(None));
static ROUTE_AWARE_CLIENT_BUILD_PERMIT: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(1);

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
    if HeaderValue::from_str(&value).is_err() {
        return Err(SetOriginatorError::InvalidHeaderValue);
    }
    let originator = get_originator_value(Some(value));
    let Ok(mut guard) = ORIGINATOR.write() else {
        return Err(SetOriginatorError::AlreadyInitialized);
    };
    if guard.is_some() {
        return Err(SetOriginatorError::AlreadyInitialized);
    }
    *guard = Some(originator);
    Ok(())
}

pub fn originator() -> Originator {
    if let Ok(guard) = ORIGINATOR.read()
        && let Some(originator) = guard.as_ref()
    {
        return originator.clone();
    }

    if std::env::var(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR).is_ok() {
        let originator = get_originator_value(/*provided*/ None);
        if let Ok(mut guard) = ORIGINATOR.write() {
            match guard.as_ref() {
                Some(originator) => return originator.clone(),
                None => *guard = Some(originator.clone()),
            }
        }
        return originator;
    }

    get_originator_value(/*provided*/ None)
}

/// Adds a valid, non-default thread originator override to request headers.
///
/// The default client already supplies the process originator. Thread-scoped callers should use
/// this helper to override that value only when the thread originator differs.
pub fn add_originator_header(headers: &mut HeaderMap, originator_value: &str) {
    let default_originator = originator();
    if originator_value == default_originator.value.as_str() {
        return;
    }

    match HeaderValue::from_str(originator_value) {
        Ok(header_value) => {
            headers.insert("originator", header_value);
        }
        Err(err) => {
            tracing::warn!("ignoring invalid thread originator header value: {err}");
        }
    }
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

pub fn get_codex_user_agent() -> String {
    // OS discovery can spawn subprocesses on Linux. Reuse it across requests,
    // while continuing to read the mutable originator and suffix below.
    static OS_INFO: LazyLock<os_info::Info> = LazyLock::new(os_info::get);
    let build_version = env!("CARGO_PKG_VERSION");
    let os_info = &*OS_INFO;
    let originator = originator();
    let prefix = format!(
        "{}/{build_version} ({} {}; {}) {}",
        originator.value.as_str(),
        os_info.os_type(),
        os_info.version(),
        os_info.architecture().unwrap_or("unknown"),
        user_agent()
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
        originator().value
    }
}

/// Create an HTTP client with default `originator` and `User-Agent` headers set.
///
/// This supported default path preserves the transport's existing proxy behavior and does not opt into
/// Codex's route-aware system/PAC resolution.
pub fn create_client() -> HttpClient {
    build_default_client(default_http_client_builder())
}

/// Creates the default client with configured ChatGPT cookies and no sensitive-response logging.
pub fn create_client_with_chatgpt_cookies(http_client_factory: &HttpClientFactory) -> HttpClient {
    build_default_client(
        default_http_client_builder()
            .with_chatgpt_cookies(http_client_factory)
            .without_request_logging(),
    )
}

/// Create the default HTTP client without request URL or response-header diagnostics.
///
/// This preserves the default client's legacy custom-CA fallback and transport proxy behavior while
/// avoiding diagnostics that could expose credentials embedded in request URLs or headers.
pub fn create_client_without_request_logging() -> HttpClient {
    build_default_client(default_http_client_builder().without_request_logging())
}

/// Builds the default Codex HTTP client for a concrete outbound route.
///
/// When route-aware proxy handling is disabled, or the client is running inside the Codex
/// sandbox, this preserves the default client's existing proxy behavior. Otherwise it resolves
/// the destination through the shared system/PAC-aware routing policy.
pub fn create_client_for_route(
    http_client_factory: &HttpClientFactory,
    request_url: &str,
    route_class: ClientRouteClass,
    redirect_policy: ClientRedirectPolicy,
) -> Result<HttpClient, BuildRouteAwareHttpClientError> {
    let builder = match redirect_policy {
        ClientRedirectPolicy::Default => default_http_client_builder(),
        ClientRedirectPolicy::Reject => default_http_client_builder().without_redirects(),
    };
    create_client_for_route_with_builder(http_client_factory, request_url, route_class, builder)
}

fn create_client_for_route_with_builder(
    http_client_factory: &HttpClientFactory,
    request_url: &str,
    route_class: ClientRouteClass,
    builder: HttpClientBuilder,
) -> Result<HttpClient, BuildRouteAwareHttpClientError> {
    if http_client_factory.network_policy().is_managed() {
        let mut pool = codex_http_client::RouteAwareClientPool::with_builder(
            http_client_factory.clone(),
            route_class,
            builder,
        );
        if is_sandboxed() {
            pool = pool.with_legacy_direct_proxy_and_custom_ca_fallback();
        } else if matches!(
            http_client_factory.outbound_proxy_policy(),
            OutboundProxyPolicy::ReqwestDefault
        ) {
            pool = pool.with_legacy_custom_ca_fallback();
        }
        return Ok(pool.into_client());
    }
    if matches!(
        http_client_factory.outbound_proxy_policy(),
        OutboundProxyPolicy::ReqwestDefault
    ) {
        return Ok(build_default_client(builder));
    }
    if is_sandboxed() {
        // Preserve the sandbox's existing no-proxy policy; sandboxed command egress is routed
        // separately through network-proxy.
        return Ok(build_default_client(builder));
    }

    builder.build_respecting_outbound_proxy_policy(http_client_factory, request_url, route_class)
}

/// Redirect handling for the shared HTTP client builders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientRedirectPolicy {
    Default,
    Reject,
}

/// Builds the default Codex HTTP client for a concrete outbound route without blocking the
/// async runtime worker that initiated the request.
pub async fn create_client_for_route_async(
    http_client_factory: HttpClientFactory,
    request_url: String,
    route_class: ClientRouteClass,
    redirect_policy: ClientRedirectPolicy,
) -> std::io::Result<HttpClient> {
    create_client_for_route_async_with_builder(
        http_client_factory,
        request_url,
        route_class,
        move || match redirect_policy {
            ClientRedirectPolicy::Default => default_http_client_builder(),
            ClientRedirectPolicy::Reject => default_http_client_builder().without_redirects(),
        },
    )
    .await
}

/// Builds a routed default client without request URL or response-header diagnostics.
pub async fn create_client_for_route_without_request_logging_async(
    http_client_factory: HttpClientFactory,
    request_url: String,
    route_class: ClientRouteClass,
) -> std::io::Result<HttpClient> {
    create_client_for_route_async_with_builder(
        http_client_factory,
        request_url,
        route_class,
        || default_http_client_builder().without_request_logging(),
    )
    .await
}

async fn create_client_for_route_async_with_builder(
    http_client_factory: HttpClientFactory,
    request_url: String,
    route_class: ClientRouteClass,
    builder: impl FnOnce() -> HttpClientBuilder + Send + 'static,
) -> std::io::Result<HttpClient> {
    let permit = ROUTE_AWARE_CLIENT_BUILD_PERMIT
        .acquire()
        .await
        .map_err(std::io::Error::other)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        create_client_for_route_with_builder(
            &http_client_factory,
            &request_url,
            route_class,
            builder(),
        )
        .map_err(std::io::Error::from)
    })
    .await
    .map_err(std::io::Error::other)?
}

/// Builds the default Codex transport without blocking the async runtime worker.
///
/// Route-aware proxy handling resolves each request and redirect destination. When it is disabled,
/// or the client is running inside the Codex sandbox, this preserves the default client's proxy
/// behavior while retaining managed application network policy enforcement.
pub async fn create_transport_for_routes_async(
    http_client_factory: HttpClientFactory,
    route_class: ClientRouteClass,
) -> std::io::Result<ReqwestTransport> {
    let permit = ROUTE_AWARE_CLIENT_BUILD_PERMIT
        .acquire()
        .await
        .map_err(std::io::Error::other)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let sandboxed = is_sandboxed();
        let transport_default_proxy = matches!(
            http_client_factory.outbound_proxy_policy(),
            OutboundProxyPolicy::ReqwestDefault
        );
        if !http_client_factory.network_policy().is_managed()
            && (transport_default_proxy || sandboxed)
        {
            return ReqwestTransport::from_http_client(create_client());
        }

        let mut pool = RouteAwareClientPool::with_builder(
            http_client_factory,
            route_class,
            default_http_client_builder(),
        );
        if sandboxed {
            pool = pool.with_legacy_direct_proxy_and_custom_ca_fallback();
        } else if transport_default_proxy {
            pool = pool.with_legacy_custom_ca_fallback();
        }
        ReqwestTransport::from_route_aware_client_pool(pool)
    })
    .await
    .map_err(std::io::Error::other)
}

fn default_http_client_builder() -> HttpClientBuilder {
    HttpClientBuilder::new()
        .default_headers(default_headers())
        .with_chatgpt_cloudflare_cookie_store()
}

// These legacy constructors intentionally preserve the infallible behavior of `create_client`.
// New endpoint-aware call sites use `create_client_for_route` and propagate construction errors.
#[allow(deprecated)]
fn build_default_client(builder: HttpClientBuilder) -> HttpClient {
    if is_sandboxed() {
        builder.build_direct_with_custom_ca_fallback()
    } else {
        builder.build_with_transport_default_proxy_and_custom_ca_fallback()
    }
}

/// Builds an HTTP client for an auth endpoint without Codex default headers.
pub(crate) fn create_raw_auth_client(
    endpoint: &str,
    auth_route_config: &AuthRouteConfig,
) -> Result<HttpClient, BuildRouteAwareHttpClientError> {
    auth_route_config
        .authentication_factory(endpoint)
        .build_client_without_request_logging(endpoint, ClientRouteClass::Auth)
}

/// Builds the default Codex HTTP client wrapper for an auth endpoint.
pub(crate) fn create_default_auth_client(
    endpoint: &str,
    auth_route_config: &AuthRouteConfig,
) -> Result<HttpClient, BuildRouteAwareHttpClientError> {
    create_client_for_route(
        &auth_route_config.authentication_factory(endpoint),
        endpoint,
        ClientRouteClass::Auth,
        ClientRedirectPolicy::Default,
    )
}

pub fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("originator", originator().header_value);
    if let Ok(user_agent) = HeaderValue::from_str(&get_codex_user_agent()) {
        headers.insert(USER_AGENT, user_agent);
    }
    if let Some(requirement) = read_default_client_residency_requirement() {
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
