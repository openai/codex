use std::collections::HashMap;
use std::num::NonZeroU16;
use std::string::String;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use reqwest::ClientBuilder;
use reqwest::Url;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::AuthorizationMetadata;
use rmcp::transport::auth::OAuthState;
use tiny_http::Response;
use tiny_http::Server;
use tokio::sync::oneshot;
use tokio::time::timeout;
use urlencoding::decode;

use crate::OAuthCredentialsStoreMode;
use crate::StoredOAuthTokens;
use crate::WrappedOAuthTokenResponse;
use crate::oauth::compute_expires_at_millis;
use crate::save_oauth_tokens;
use crate::utils::apply_default_headers;
use crate::utils::build_default_headers;

/// Built-in Client ID Metadata Document used for SEP-991/CIMD fallback.
///
/// This intentionally tracks the `main` branch so clients can pick up metadata updates
/// without requiring a binary release.
const DEFAULT_CIMD_CLIENT_METADATA_URL: &str =
    "https://raw.githubusercontent.com/openai/codex/main/codex-rs/client-metadata.json";
const CLIENT_ID_METADATA_DOCUMENT_SUPPORTED_FIELD: &str = "client_id_metadata_document_supported";
/// Fixed loopback callback port required by Codex's built-in CIMD redirect URIs.
const DEFAULT_CIMD_CALLBACK_PORT: u16 = 33418;
const DEFAULT_CIMD_REDIRECT_URI_ROOT: &str = "http://127.0.0.1:33418/";
const DEFAULT_CIMD_REDIRECT_URI_CALLBACK: &str = "http://127.0.0.1:33418/callback";

struct OauthHeaders {
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
}

struct CallbackServerGuard {
    server: Arc<Server>,
}

impl Drop for CallbackServerGuard {
    fn drop(&mut self) {
        self.server.unblock();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthProviderError {
    error: Option<String>,
    error_description: Option<String>,
}

impl OAuthProviderError {
    pub fn new(error: Option<String>, error_description: Option<String>) -> Self {
        Self {
            error,
            error_description,
        }
    }
}

impl std::fmt::Display for OAuthProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.error.as_deref(), self.error_description.as_deref()) {
            (Some(error), Some(error_description)) => {
                write!(f, "OAuth provider returned `{error}`: {error_description}")
            }
            (Some(error), None) => write!(f, "OAuth provider returned `{error}`"),
            (None, Some(error_description)) => write!(f, "OAuth error: {error_description}"),
            (None, None) => write!(f, "OAuth provider returned an error"),
        }
    }
}

impl std::error::Error for OAuthProviderError {}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
) -> Result<()> {
    let headers = OauthHeaders {
        http_headers,
        env_http_headers,
    };
    OauthLoginFlow::new(
        server_name,
        server_url,
        store_mode,
        headers,
        scopes,
        oauth_resource,
        /*launch_browser*/ true,
        callback_port,
        callback_url,
        /*timeout_secs*/ None,
    )
    .await?
    .finish()
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login_return_url(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_resource: Option<&str>,
    timeout_secs: Option<i64>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
) -> Result<OauthLoginHandle> {
    let headers = OauthHeaders {
        http_headers,
        env_http_headers,
    };
    let flow = OauthLoginFlow::new(
        server_name,
        server_url,
        store_mode,
        headers,
        scopes,
        oauth_resource,
        /*launch_browser*/ false,
        callback_port,
        callback_url,
        timeout_secs,
    )
    .await?;

    let authorization_url = flow.authorization_url();
    let completion = flow.spawn();

    Ok(OauthLoginHandle::new(authorization_url, completion))
}

fn spawn_callback_server(
    server: Arc<Server>,
    tx: oneshot::Sender<CallbackResult>,
    expected_callback_path: String,
) {
    tokio::task::spawn_blocking(move || {
        while let Ok(request) = server.recv() {
            let path = request.url().to_string();
            match parse_oauth_callback(&path, &expected_callback_path) {
                CallbackOutcome::Success(OauthCallbackResult { code, state }) => {
                    let response = Response::from_string(
                        "Authentication complete. You may close this window.",
                    );
                    if let Err(err) = request.respond(response) {
                        eprintln!("Failed to respond to OAuth callback: {err}");
                    }
                    if let Err(err) =
                        tx.send(CallbackResult::Success(OauthCallbackResult { code, state }))
                    {
                        eprintln!("Failed to send OAuth callback: {err:?}");
                    }
                    break;
                }
                CallbackOutcome::Error(error) => {
                    let response = Response::from_string(error.to_string()).with_status_code(400);
                    if let Err(err) = request.respond(response) {
                        eprintln!("Failed to respond to OAuth callback: {err}");
                    }
                    if let Err(err) = tx.send(CallbackResult::Error(error)) {
                        eprintln!("Failed to send OAuth callback error: {err:?}");
                    }
                    break;
                }
                CallbackOutcome::Invalid => {
                    let response =
                        Response::from_string("Invalid OAuth callback").with_status_code(400);
                    if let Err(err) = request.respond(response) {
                        eprintln!("Failed to respond to OAuth callback: {err}");
                    }
                }
            }
        }
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OauthCallbackResult {
    code: String,
    state: String,
}

#[derive(Debug)]
enum CallbackResult {
    Success(OauthCallbackResult),
    Error(OAuthProviderError),
}

#[derive(Debug, PartialEq, Eq)]
enum CallbackOutcome {
    Success(OauthCallbackResult),
    Error(OAuthProviderError),
    Invalid,
}

fn parse_oauth_callback(path: &str, expected_callback_path: &str) -> CallbackOutcome {
    let Some((route, query)) = path.split_once('?') else {
        return CallbackOutcome::Invalid;
    };
    if route != expected_callback_path {
        return CallbackOutcome::Invalid;
    }

    let mut code = None;
    let mut state = None;
    let mut error = None;
    let mut error_description = None;

    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let Ok(decoded) = decode(value) else {
            continue;
        };
        let decoded = decoded.into_owned();
        match key {
            "code" => code = Some(decoded),
            "state" => state = Some(decoded),
            "error" => error = Some(decoded),
            "error_description" => error_description = Some(decoded),
            _ => {}
        }
    }

    if let (Some(code), Some(state)) = (code, state) {
        return CallbackOutcome::Success(OauthCallbackResult { code, state });
    }

    if error.is_some() || error_description.is_some() {
        return CallbackOutcome::Error(OAuthProviderError::new(error, error_description));
    }

    CallbackOutcome::Invalid
}

pub struct OauthLoginHandle {
    authorization_url: String,
    completion: oneshot::Receiver<Result<()>>,
}

impl OauthLoginHandle {
    fn new(authorization_url: String, completion: oneshot::Receiver<Result<()>>) -> Self {
        Self {
            authorization_url,
            completion,
        }
    }

    pub fn authorization_url(&self) -> &str {
        &self.authorization_url
    }

    pub fn into_parts(self) -> (String, oneshot::Receiver<Result<()>>) {
        (self.authorization_url, self.completion)
    }

    pub async fn wait(self) -> Result<()> {
        self.completion
            .await
            .map_err(|err| anyhow!("OAuth login task was cancelled: {err}"))?
    }
}

struct OauthLoginFlow {
    auth_url: String,
    oauth_state: OAuthState,
    rx: oneshot::Receiver<CallbackResult>,
    guard: CallbackServerGuard,
    server_name: String,
    server_url: String,
    store_mode: OAuthCredentialsStoreMode,
    launch_browser: bool,
    timeout: Duration,
}

struct CallbackListener {
    guard: CallbackServerGuard,
    redirect_uri: String,
    rx: oneshot::Receiver<CallbackResult>,
}

fn default_cimd_callback_port_nonzero() -> Result<NonZeroU16> {
    NonZeroU16::new(DEFAULT_CIMD_CALLBACK_PORT).ok_or_else(|| {
        anyhow!(
            "invalid built-in CIMD callback port `{DEFAULT_CIMD_CALLBACK_PORT}`: port must be between 1 and 65535"
        )
    })
}

fn resolve_callback_port(
    callback_port: Option<u16>,
    use_default_cimd_metadata: bool,
) -> Result<Option<NonZeroU16>> {
    if let Some(port) = callback_port {
        let Some(port) = NonZeroU16::new(port) else {
            bail!("invalid MCP OAuth callback port `{port}`: port must be between 1 and 65535");
        };
        return Ok(Some(port));
    }

    if use_default_cimd_metadata {
        Ok(Some(default_cimd_callback_port_nonzero()?))
    } else {
        Ok(None)
    }
}

fn local_redirect_uri(server: &Server) -> Result<String> {
    match server.server_addr() {
        tiny_http::ListenAddr::IP(std::net::SocketAddr::V4(addr)) => {
            let ip = addr.ip();
            let port = addr.port();
            Ok(format!("http://{ip}:{port}/callback"))
        }
        tiny_http::ListenAddr::IP(std::net::SocketAddr::V6(addr)) => {
            let ip = addr.ip();
            let port = addr.port();
            Ok(format!("http://[{ip}]:{port}/callback"))
        }
        #[cfg(not(target_os = "windows"))]
        _ => Err(anyhow!("unable to determine callback address")),
    }
}

fn resolve_redirect_uri(server: &Server, callback_url: Option<&str>) -> Result<String> {
    let Some(callback_url) = callback_url else {
        return local_redirect_uri(server);
    };
    Url::parse(callback_url)
        .with_context(|| format!("invalid MCP OAuth callback URL `{callback_url}`"))?;
    Ok(callback_url.to_string())
}

fn callback_path_from_redirect_uri(redirect_uri: &str) -> Result<String> {
    let parsed = Url::parse(redirect_uri)
        .with_context(|| format!("invalid redirect URI `{redirect_uri}`"))?;
    Ok(parsed.path().to_string())
}

fn uses_default_cimd_redirect_uri(redirect_uri: &str) -> bool {
    matches!(
        redirect_uri,
        DEFAULT_CIMD_REDIRECT_URI_ROOT | DEFAULT_CIMD_REDIRECT_URI_CALLBACK
    )
}

fn validate_callback_listener_settings(
    callback_port: Option<u16>,
    callback_url: Option<&str>,
) -> Result<()> {
    let Some(callback_url) = callback_url else {
        return Ok(());
    };
    if !uses_default_cimd_redirect_uri(callback_url) {
        return Ok(());
    };
    let Some(callback_port) = callback_port else {
        return Ok(());
    };

    if callback_port == DEFAULT_CIMD_CALLBACK_PORT {
        Ok(())
    } else {
        Err(anyhow!(
            "MCP OAuth callback URL `{callback_url}` is a built-in Codex client metadata redirect URI, so `mcp_oauth_callback_port` must be `{DEFAULT_CIMD_CALLBACK_PORT}` or unset"
        ))
    }
}

fn callback_bind_host(callback_url: Option<&str>) -> String {
    let Some(callback_url) = callback_url else {
        return "127.0.0.1".to_string();
    };

    let Ok(parsed) = Url::parse(callback_url) else {
        return "127.0.0.1".to_string();
    };

    let Some(host) = parsed.host_str() else {
        return "127.0.0.1".to_string();
    };

    if host.eq_ignore_ascii_case("localhost") {
        return "127.0.0.1".to_string();
    }
    let host = host.trim_matches(['[', ']']);

    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) if ip.is_loopback() => ip.to_string(),
        Ok(std::net::IpAddr::V6(ip)) if ip.is_loopback() => format!("[{ip}]"),
        Ok(_) | Err(_) => "0.0.0.0".to_string(),
    }
}

fn start_callback_listener(
    bind_host: &str,
    callback_port: Option<NonZeroU16>,
    callback_url: Option<&str>,
) -> Result<CallbackListener> {
    let bind_addr = match callback_port {
        Some(port) => format!("{bind_host}:{}", port.get()),
        None => format!("{bind_host}:0"),
    };

    let server = Arc::new(Server::http(&bind_addr).map_err(|err| anyhow!(err))?);
    let guard = CallbackServerGuard {
        server: Arc::clone(&server),
    };

    let redirect_uri = resolve_redirect_uri(&server, callback_url)?;
    let callback_path = callback_path_from_redirect_uri(&redirect_uri)?;

    let (tx, rx) = oneshot::channel();
    spawn_callback_server(server, tx, callback_path);

    Ok(CallbackListener {
        guard,
        redirect_uri,
        rx,
    })
}

impl OauthLoginFlow {
    #[allow(clippy::too_many_arguments)]
    async fn new(
        server_name: &str,
        server_url: &str,
        store_mode: OAuthCredentialsStoreMode,
        headers: OauthHeaders,
        scopes: &[String],
        oauth_resource: Option<&str>,
        launch_browser: bool,
        callback_port: Option<u16>,
        callback_url: Option<&str>,
        timeout_secs: Option<i64>,
    ) -> Result<Self> {
        const DEFAULT_OAUTH_TIMEOUT_SECS: i64 = 300;

        validate_callback_listener_settings(callback_port, callback_url)?;

        let OauthHeaders {
            http_headers,
            env_http_headers,
        } = headers;
        let default_headers = build_default_headers(http_headers, env_http_headers)?;
        let http_client = apply_default_headers(ClientBuilder::new(), &default_headers).build()?;
        let metadata = discover_oauth_authorization_metadata(server_url, http_client.clone())
            .await
            .context("failed to discover OAuth authorization metadata")?;
        let should_start_with_default_cimd_metadata = should_use_default_cimd_metadata(&metadata);
        let supports_default_cimd_metadata = client_id_metadata_document_supported(&metadata);
        let callback_port_is_explicitly_set = callback_port.is_some();
        let callback_url_is_explicitly_set = callback_url.is_some();
        let callback_url_uses_default_cimd_redirect = callback_url
            .map(uses_default_cimd_redirect_uri)
            .unwrap_or(false);
        let should_rebind_callback_for_cimd_fallback =
            !callback_port_is_explicitly_set && !callback_url_is_explicitly_set;
        let should_bind_to_default_cimd_port = should_start_with_default_cimd_metadata
            || (!callback_port_is_explicitly_set && callback_url_uses_default_cimd_redirect);

        let bind_host = callback_bind_host(callback_url);
        let callback_port = resolve_callback_port(callback_port, should_bind_to_default_cimd_port)?;
        let mut callback_listener =
            start_callback_listener(&bind_host, callback_port, callback_url)?;

        let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
        let oauth_state = match start_oauth_authorization(
            server_url,
            &http_client,
            &scope_refs,
            &callback_listener.redirect_uri,
            should_start_with_default_cimd_metadata,
        )
        .await
        {
            Ok(oauth_state) => oauth_state,
            Err(dynamic_registration_err)
                if !should_start_with_default_cimd_metadata
                    && supports_default_cimd_metadata
                    && matches!(dynamic_registration_err, AuthError::RegistrationFailed(_)) =>
            {
                if should_rebind_callback_listener_for_cimd_fallback(
                    should_rebind_callback_for_cimd_fallback,
                    &callback_listener.redirect_uri,
                ) {
                    callback_listener = start_callback_listener(
                        "127.0.0.1",
                        Some(default_cimd_callback_port_nonzero().context(
                            "invalid built-in CIMD callback port for fallback listener",
                        )?),
                        None,
                    )
                    .context("failed to rebind OAuth callback listener for CIMD fallback")?;
                } else if callback_port_is_explicitly_set || callback_url_is_explicitly_set {
                    validate_redirect_uri_for_default_cimd_metadata(
                        &callback_listener.redirect_uri,
                    )?;
                }

                start_oauth_authorization(
                    server_url,
                    &http_client,
                    &scope_refs,
                    &callback_listener.redirect_uri,
                    true,
                )
                .await
                .map_err(anyhow::Error::from)
                .context(
                    "failed to start OAuth authorization with default client metadata URL after dynamic registration failed",
                )?
            }
            Err(err) if should_start_with_default_cimd_metadata => {
                return Err(anyhow::Error::from(err).context(
                    "failed to start OAuth authorization with default client metadata URL",
                ));
            }
            Err(err) => {
                return Err(anyhow::Error::from(err).context("failed to start OAuth authorization"));
            }
        };
        let auth_url = append_query_param(
            &oauth_state.get_authorization_url().await?,
            "resource",
            oauth_resource,
        );
        let timeout_secs = timeout_secs.unwrap_or(DEFAULT_OAUTH_TIMEOUT_SECS).max(1);
        let timeout = Duration::from_secs(timeout_secs as u64);

        Ok(Self {
            auth_url,
            oauth_state,
            rx: callback_listener.rx,
            guard: callback_listener.guard,
            server_name: server_name.to_string(),
            server_url: server_url.to_string(),
            store_mode,
            launch_browser,
            timeout,
        })
    }

    fn authorization_url(&self) -> String {
        self.auth_url.clone()
    }

    async fn finish(mut self) -> Result<()> {
        if self.launch_browser {
            let server_name = &self.server_name;
            let auth_url = &self.auth_url;
            println!(
                "Authorize `{server_name}` by opening this URL in your browser:\n{auth_url}\n"
            );

            if webbrowser::open(auth_url).is_err() {
                println!("(Browser launch failed; please copy the URL above manually.)");
            }
        }

        let result = async {
            let callback = timeout(self.timeout, &mut self.rx)
                .await
                .context("timed out waiting for OAuth callback")?
                .context("OAuth callback was cancelled")?;
            let OauthCallbackResult {
                code,
                state: csrf_state,
            } = match callback {
                CallbackResult::Success(callback) => callback,
                CallbackResult::Error(error) => return Err(anyhow!(error)),
            };

            self.oauth_state
                .handle_callback(&code, &csrf_state)
                .await
                .context("failed to handle OAuth callback")?;

            let (client_id, credentials_opt) = self
                .oauth_state
                .get_credentials()
                .await
                .context("failed to retrieve OAuth credentials")?;
            let credentials = credentials_opt
                .ok_or_else(|| anyhow!("OAuth provider did not return credentials"))?;

            let expires_at = compute_expires_at_millis(&credentials);
            let stored = StoredOAuthTokens {
                server_name: self.server_name.clone(),
                url: self.server_url.clone(),
                client_id,
                token_response: WrappedOAuthTokenResponse(credentials),
                expires_at,
            };
            save_oauth_tokens(&self.server_name, &stored, self.store_mode)?;

            Ok(())
        }
        .await;

        drop(self.guard);
        result
    }

    fn spawn(self) -> oneshot::Receiver<Result<()>> {
        let server_name_for_logging = self.server_name.clone();
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = self.finish().await;

            if let Err(err) = &result {
                eprintln!(
                    "Failed to complete OAuth login for '{server_name_for_logging}': {err:#}"
                );
            }

            let _ = tx.send(result);
        });

        rx
    }
}

async fn start_oauth_authorization(
    server_url: &str,
    http_client: &reqwest::Client,
    scope_refs: &[&str],
    redirect_uri: &str,
    use_default_cimd_metadata: bool,
) -> std::result::Result<OAuthState, AuthError> {
    let mut oauth_state = OAuthState::new(server_url, Some(http_client.clone())).await?;

    if use_default_cimd_metadata {
        validate_redirect_uri_for_default_cimd_metadata(redirect_uri)
            .map_err(|err| AuthError::InternalError(err.to_string()))?;
        oauth_state
            .start_authorization_with_metadata_url(
                scope_refs,
                redirect_uri,
                Some("Codex"),
                Some(DEFAULT_CIMD_CLIENT_METADATA_URL),
            )
            .await?;
    } else {
        oauth_state
            .start_authorization(scope_refs, redirect_uri, Some("Codex"))
            .await?;
    }

    Ok(oauth_state)
}

async fn discover_oauth_authorization_metadata(
    server_url: &str,
    http_client: reqwest::Client,
) -> Result<AuthorizationMetadata> {
    let mut manager = AuthorizationManager::new(server_url)
        .await
        .context("failed to create OAuth authorization manager")?;
    manager
        .with_client(http_client)
        .context("failed to configure OAuth HTTP client")?;
    manager
        .discover_metadata()
        .await
        .context("failed to discover OAuth server metadata")
}

fn should_use_default_cimd_metadata(metadata: &AuthorizationMetadata) -> bool {
    metadata.registration_endpoint.is_none() && client_id_metadata_document_supported(metadata)
}

fn client_id_metadata_document_supported(metadata: &AuthorizationMetadata) -> bool {
    metadata
        .additional_fields
        .get(CLIENT_ID_METADATA_DOCUMENT_SUPPORTED_FIELD)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn validate_redirect_uri_for_default_cimd_metadata(redirect_uri: &str) -> Result<()> {
    if uses_default_cimd_redirect_uri(redirect_uri) {
        Ok(())
    } else {
        Err(anyhow!(
            "MCP OAuth callback URL `{redirect_uri}` is incompatible with built-in Codex client metadata; use `{DEFAULT_CIMD_REDIRECT_URI_ROOT}` or `{DEFAULT_CIMD_REDIRECT_URI_CALLBACK}`"
        ))
    }
}

fn should_rebind_callback_listener_for_cimd_fallback(
    should_rebind_callback_for_cimd_fallback: bool,
    redirect_uri: &str,
) -> bool {
    should_rebind_callback_for_cimd_fallback
        && validate_redirect_uri_for_default_cimd_metadata(redirect_uri).is_err()
}

fn append_query_param(url: &str, key: &str, value: Option<&str>) -> String {
    let Some(value) = value else {
        return url.to_string();
    };
    let value = value.trim();
    if value.is_empty() {
        return url.to_string();
    }
    if let Ok(mut parsed) = Url::parse(url) {
        parsed.query_pairs_mut().append_pair(key, value);
        return parsed.to_string();
    }
    let encoded = urlencoding::encode(value);
    let separator = if url.contains('?') { "&" } else { "?" };
    format!("{url}{separator}{key}={encoded}")
}

#[cfg(test)]
#[path = "perform_oauth_login.rs_tests.rs"]
mod tests;
