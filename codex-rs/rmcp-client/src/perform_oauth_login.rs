use std::collections::HashMap;
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

const DEFAULT_CIMD_CLIENT_METADATA_URL: &str =
    "https://raw.githubusercontent.com/openai/codex/main/codex-rs/client-metadata.json";
const CLIENT_ID_METADATA_DOCUMENT_SUPPORTED_FIELD: &str = "client_id_metadata_document_supported";
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
        true,
        callback_port,
        callback_url,
        None,
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
        false,
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
    tx: oneshot::Sender<(String, String)>,
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
                    if let Err(err) = tx.send((code, state)) {
                        eprintln!("Failed to send OAuth callback: {err:?}");
                    }
                    break;
                }
                CallbackOutcome::Error(description) => {
                    let response = Response::from_string(format!("OAuth error: {description}"))
                        .with_status_code(400);
                    if let Err(err) = request.respond(response) {
                        eprintln!("Failed to respond to OAuth callback: {err}");
                    }
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

struct OauthCallbackResult {
    code: String,
    state: String,
}

enum CallbackOutcome {
    Success(OauthCallbackResult),
    Error(String),
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
            "error_description" => error_description = Some(decoded),
            _ => {}
        }
    }

    if let (Some(code), Some(state)) = (code, state) {
        return CallbackOutcome::Success(OauthCallbackResult { code, state });
    }

    if let Some(description) = error_description {
        return CallbackOutcome::Error(description);
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
    rx: oneshot::Receiver<(String, String)>,
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
    rx: oneshot::Receiver<(String, String)>,
}

fn resolve_callback_port(
    callback_port: Option<u16>,
    use_default_cimd_metadata: bool,
) -> Result<Option<u16>> {
    if let Some(port) = callback_port {
        if port == 0 {
            bail!("invalid MCP OAuth callback port `{port}`: port must be between 1 and 65535");
        }
        return Ok(Some(port));
    }

    if use_default_cimd_metadata {
        Ok(Some(DEFAULT_CIMD_CALLBACK_PORT))
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
    callback_port: Option<u16>,
    callback_url: Option<&str>,
) -> Result<CallbackListener> {
    let bind_addr = match callback_port {
        Some(port) => format!("{bind_host}:{port}"),
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
        let should_rebind_callback_for_cimd_fallback =
            callback_port.is_none() && callback_url.is_none();

        let bind_host = callback_bind_host(callback_url);
        let callback_port =
            resolve_callback_port(callback_port, should_start_with_default_cimd_metadata)?;
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
                        Some(DEFAULT_CIMD_CALLBACK_PORT),
                        None,
                    )
                    .context("failed to rebind OAuth callback listener for CIMD fallback")?;
                } else if !should_rebind_callback_for_cimd_fallback {
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
            let (code, csrf_state) = timeout(self.timeout, &mut self.rx)
                .await
                .context("timed out waiting for OAuth callback")?
                .context("OAuth callback was cancelled")?;

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
    if matches!(
        redirect_uri,
        DEFAULT_CIMD_REDIRECT_URI_ROOT | DEFAULT_CIMD_REDIRECT_URI_CALLBACK
    ) {
        return Ok(());
    }

    bail!(
        "MCP OAuth callback URL `{redirect_uri}` is incompatible with built-in Codex client metadata; use `{DEFAULT_CIMD_REDIRECT_URI_ROOT}` or `{DEFAULT_CIMD_REDIRECT_URI_CALLBACK}`"
    )
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
mod tests {
    use axum::Json;
    use axum::Router;
    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::routing::post;
    use pretty_assertions::assert_eq;
    use reqwest::Url;
    use serde_json::json;
    use std::io::ErrorKind;
    use std::net::TcpListener;
    use std::sync::OnceLock;
    use tokio::task::JoinHandle;

    use super::CLIENT_ID_METADATA_DOCUMENT_SUPPORTED_FIELD;
    use super::CallbackOutcome;
    use super::DEFAULT_CIMD_CLIENT_METADATA_URL;
    use super::DEFAULT_CIMD_REDIRECT_URI_CALLBACK;
    use super::DEFAULT_CIMD_REDIRECT_URI_ROOT;
    use super::OAuthCredentialsStoreMode;
    use super::append_query_param;
    use super::callback_bind_host;
    use super::callback_path_from_redirect_uri;
    use super::client_id_metadata_document_supported;
    use super::parse_oauth_callback;
    use super::perform_oauth_login_return_url;
    use super::should_rebind_callback_listener_for_cimd_fallback;
    use super::should_use_default_cimd_metadata;
    use super::validate_redirect_uri_for_default_cimd_metadata;
    use rmcp::transport::auth::AuthorizationMetadata;

    fn callback_port_test_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    fn available_loopback_port() -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral listener should bind");
        listener
            .local_addr()
            .expect("listener should have local addr")
            .port()
    }

    #[test]
    fn parse_oauth_callback_accepts_default_path() {
        let parsed = parse_oauth_callback("/callback?code=abc&state=xyz", "/callback");
        assert!(matches!(parsed, CallbackOutcome::Success(_)));
    }

    #[test]
    fn parse_oauth_callback_accepts_custom_path() {
        let parsed = parse_oauth_callback("/oauth/callback?code=abc&state=xyz", "/oauth/callback");
        assert!(matches!(parsed, CallbackOutcome::Success(_)));
    }

    #[test]
    fn parse_oauth_callback_rejects_wrong_path() {
        let parsed = parse_oauth_callback("/callback?code=abc&state=xyz", "/oauth/callback");
        assert!(matches!(parsed, CallbackOutcome::Invalid));
    }

    #[test]
    fn callback_path_comes_from_redirect_uri() {
        let path = callback_path_from_redirect_uri("https://example.com/oauth/callback")
            .expect("redirect URI should parse");
        assert_eq!(path, "/oauth/callback");
    }

    #[test]
    fn callback_bind_host_preserves_ipv6_loopback() {
        let bind_host = callback_bind_host(Some("http://[::1]:33418/callback"));
        assert_eq!(bind_host, "[::1]");
    }

    #[test]
    fn callback_bind_host_preserves_ipv4_loopback_alias() {
        let bind_host = callback_bind_host(Some("http://127.0.0.2:33418/callback"));
        assert_eq!(bind_host, "127.0.0.2");
    }

    #[test]
    fn callback_bind_host_maps_localhost_to_ipv4_loopback() {
        let bind_host = callback_bind_host(Some("http://localhost:33418/callback"));
        assert_eq!(bind_host, "127.0.0.1");
    }

    #[test]
    fn append_query_param_adds_resource_to_absolute_url() {
        let url = append_query_param(
            "https://example.com/authorize?scope=read",
            "resource",
            Some("https://api.example.com"),
        );

        assert_eq!(
            url,
            "https://example.com/authorize?scope=read&resource=https%3A%2F%2Fapi.example.com"
        );
    }

    #[test]
    fn append_query_param_ignores_empty_values() {
        let url = append_query_param(
            "https://example.com/authorize?scope=read",
            "resource",
            Some("   "),
        );

        assert_eq!(url, "https://example.com/authorize?scope=read");
    }

    #[test]
    fn append_query_param_handles_unparseable_url() {
        let url = append_query_param("not a url", "resource", Some("api/resource"));

        assert_eq!(url, "not a url?resource=api%2Fresource");
    }

    #[test]
    fn callback_port_defaults_to_ephemeral_for_non_cimd() {
        let port = super::resolve_callback_port(None, false)
            .expect("default callback port should resolve");
        assert_eq!(port, None);
    }

    #[test]
    fn callback_port_defaults_to_cimd_port_for_cimd_metadata() {
        let port =
            super::resolve_callback_port(None, true).expect("default callback port should resolve");
        assert_eq!(port, Some(super::DEFAULT_CIMD_CALLBACK_PORT));
    }

    #[test]
    fn default_cimd_redirect_uri_validation_accepts_supported_uris() {
        let result_root =
            validate_redirect_uri_for_default_cimd_metadata(DEFAULT_CIMD_REDIRECT_URI_ROOT);
        let result_callback =
            validate_redirect_uri_for_default_cimd_metadata(DEFAULT_CIMD_REDIRECT_URI_CALLBACK);

        assert_eq!(result_root.is_ok(), true);
        assert_eq!(result_callback.is_ok(), true);
    }

    #[test]
    fn default_cimd_redirect_uri_validation_rejects_other_uris() {
        let err = validate_redirect_uri_for_default_cimd_metadata("http://127.0.0.1:43210/")
            .expect_err("unexpected success for unsupported redirect URI");

        assert!(
            err.to_string()
                .contains("incompatible with built-in Codex client metadata"),
            "unexpected redirect validation error: {err:#}"
        );
    }

    #[test]
    fn cimd_fallback_skips_rebind_when_existing_listener_is_compatible() {
        let should_rebind = should_rebind_callback_listener_for_cimd_fallback(
            true,
            DEFAULT_CIMD_REDIRECT_URI_CALLBACK,
        );

        assert_eq!(should_rebind, false);
    }

    #[test]
    fn cimd_fallback_rebinds_when_existing_listener_is_incompatible() {
        let should_rebind =
            should_rebind_callback_listener_for_cimd_fallback(true, "http://127.0.0.1:43210/");

        assert_eq!(should_rebind, true);
    }

    #[test]
    fn cimd_fallback_never_rebinds_when_rebind_not_requested() {
        let should_rebind =
            should_rebind_callback_listener_for_cimd_fallback(false, "http://127.0.0.1:43210/");

        assert_eq!(should_rebind, false);
    }

    #[test]
    fn cimd_support_requires_metadata_flag_and_missing_registration_endpoint() {
        let supported = authorization_metadata(None, true);
        let missing_flag = authorization_metadata(None, false);
        let with_registration = authorization_metadata(Some("https://example.com/register"), true);

        assert_eq!(client_id_metadata_document_supported(&supported), true);
        assert_eq!(should_use_default_cimd_metadata(&supported), true);
        assert_eq!(should_use_default_cimd_metadata(&missing_flag), false);
        assert_eq!(should_use_default_cimd_metadata(&with_registration), false);
    }

    #[tokio::test]
    async fn oauth_login_uses_default_cimd_metadata_when_dynamic_registration_unsupported() {
        let _lock = callback_port_test_lock().lock().await;
        let (server_url, server_handle) = start_oauth_metadata_server(true, false, false).await;

        let login_handle = perform_oauth_login_return_url(
            "rmcp-http",
            &server_url,
            OAuthCredentialsStoreMode::File,
            None,
            None,
            &[],
            None,
            Some(1),
            None,
            None,
        )
        .await
        .expect("oauth login should start with default CIMD metadata URL");
        let (authorization_url, completion) = login_handle.into_parts();

        let parsed = Url::parse(&authorization_url).expect("authorization URL should parse");
        let params = parsed
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            params.get("client_id").map(std::convert::AsRef::as_ref),
            Some(DEFAULT_CIMD_CLIENT_METADATA_URL)
        );

        let err = completion
            .await
            .expect("oauth completion receiver should resolve")
            .expect_err("oauth should time out in test without callback");
        assert!(
            err.to_string()
                .contains("timed out waiting for OAuth callback"),
            "unexpected oauth completion error: {err}"
        );

        server_handle.abort();
    }

    #[tokio::test]
    async fn oauth_login_rejects_incompatible_callback_for_default_cimd_metadata() {
        let _lock = callback_port_test_lock().lock().await;
        let (server_url, server_handle) = start_oauth_metadata_server(true, false, false).await;
        let incompatible_port = available_loopback_port();

        let err = perform_oauth_login_return_url(
            "rmcp-http",
            &server_url,
            OAuthCredentialsStoreMode::File,
            None,
            None,
            &[],
            None,
            Some(1),
            Some(incompatible_port),
            None,
        )
        .await
        .err()
        .expect("oauth login should fail when callback URI is incompatible with fallback metadata");
        let err_text = format!("{err:#}");

        assert!(
            err_text.contains("incompatible with built-in Codex client metadata"),
            "unexpected oauth setup error: {err:#}"
        );

        server_handle.abort();
    }

    #[tokio::test]
    async fn oauth_login_non_cimd_starts_when_cimd_default_port_is_occupied() {
        let _lock = callback_port_test_lock().lock().await;
        let _occupied_port_listener =
            match TcpListener::bind(("127.0.0.1", super::DEFAULT_CIMD_CALLBACK_PORT)) {
                Ok(listener) => Some(listener),
                Err(err) if err.kind() == ErrorKind::AddrInUse => None,
                Err(err) => panic!("failed to bind default CIMD callback port: {err}"),
            };
        let (server_url, server_handle) = start_oauth_metadata_server(false, true, false).await;

        let login_handle = perform_oauth_login_return_url(
            "rmcp-http",
            &server_url,
            OAuthCredentialsStoreMode::File,
            None,
            None,
            &[],
            None,
            Some(1),
            None,
            None,
        )
        .await
        .expect("oauth login should start on an ephemeral callback port");
        let (authorization_url, completion) = login_handle.into_parts();

        let parsed = Url::parse(&authorization_url).expect("authorization URL should parse");
        let params = parsed
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        let redirect_uri = params
            .get("redirect_uri")
            .map(std::convert::AsRef::as_ref)
            .expect("authorization URL should include redirect_uri");
        assert!(
            redirect_uri.starts_with("http://127.0.0.1:"),
            "unexpected redirect URI: {redirect_uri}"
        );
        assert!(
            !redirect_uri.contains(":33418/"),
            "expected non-CIMD redirect URI to avoid default CIMD callback port: {redirect_uri}"
        );

        let err = completion
            .await
            .expect("oauth completion receiver should resolve")
            .expect_err("oauth should time out in test without callback");
        assert!(
            err.to_string()
                .contains("timed out waiting for OAuth callback"),
            "unexpected oauth completion error: {err}"
        );

        server_handle.abort();
    }

    #[tokio::test]
    async fn oauth_login_falls_back_to_default_cimd_metadata_when_registration_fails() {
        let _lock = callback_port_test_lock().lock().await;
        let (server_url, server_handle) = start_oauth_metadata_server(true, true, true).await;

        let login_handle = perform_oauth_login_return_url(
            "rmcp-http",
            &server_url,
            OAuthCredentialsStoreMode::File,
            None,
            None,
            &[],
            None,
            Some(1),
            None,
            None,
        )
        .await
        .expect("oauth login should fall back to default CIMD metadata URL");
        let (authorization_url, completion) = login_handle.into_parts();

        let parsed = Url::parse(&authorization_url).expect("authorization URL should parse");
        let params = parsed
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            params.get("client_id").map(std::convert::AsRef::as_ref),
            Some(DEFAULT_CIMD_CLIENT_METADATA_URL)
        );
        assert_eq!(
            params.get("redirect_uri").map(std::convert::AsRef::as_ref),
            Some(DEFAULT_CIMD_REDIRECT_URI_CALLBACK)
        );

        let err = completion
            .await
            .expect("oauth completion receiver should resolve")
            .expect_err("oauth should time out in test without callback");
        assert!(
            err.to_string()
                .contains("timed out waiting for OAuth callback"),
            "unexpected oauth completion error: {err}"
        );

        server_handle.abort();
    }

    #[tokio::test]
    async fn oauth_login_fallback_rejects_incompatible_explicit_callback_after_registration_failure()
     {
        let _lock = callback_port_test_lock().lock().await;
        let (server_url, server_handle) = start_oauth_metadata_server(true, true, true).await;
        let incompatible_port = available_loopback_port();

        let err = perform_oauth_login_return_url(
            "rmcp-http",
            &server_url,
            OAuthCredentialsStoreMode::File,
            None,
            None,
            &[],
            None,
            Some(1),
            Some(incompatible_port),
            None,
        )
        .await
        .err()
        .expect(
            "oauth login should fail when explicit callback is incompatible with CIMD fallback",
        );

        assert!(
            err.to_string()
                .contains("incompatible with built-in Codex client metadata"),
            "unexpected oauth setup error: {err:#}"
        );

        server_handle.abort();
    }

    fn authorization_metadata(
        registration_endpoint: Option<&str>,
        client_metadata_document_supported: bool,
    ) -> AuthorizationMetadata {
        let mut additional_fields = serde_json::Map::new();
        additional_fields.insert(
            CLIENT_ID_METADATA_DOCUMENT_SUPPORTED_FIELD.to_string(),
            serde_json::Value::Bool(client_metadata_document_supported),
        );

        AuthorizationMetadata {
            authorization_endpoint: "https://example.com/authorize".to_string(),
            token_endpoint: "https://example.com/token".to_string(),
            registration_endpoint: registration_endpoint.map(str::to_string),
            issuer: None,
            jwks_uri: None,
            scopes_supported: None,
            response_types_supported: Some(vec!["code".to_string()]),
            additional_fields: additional_fields.into_iter().collect(),
        }
    }

    async fn start_oauth_metadata_server(
        client_id_metadata_document_supported: bool,
        include_registration_endpoint: bool,
        registration_fails: bool,
    ) -> (String, JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener
            .local_addr()
            .expect("listener should have local addr");
        let base_url = format!("http://{addr}");
        let mut metadata = json!({
            "authorization_endpoint": format!("{base_url}/oauth/authorize"),
            "token_endpoint": format!("{base_url}/oauth/token"),
            "response_types_supported": ["code"],
            "code_challenge_methods_supported": ["S256"],
            "client_id_metadata_document_supported": client_id_metadata_document_supported,
        });
        if include_registration_endpoint && let Some(metadata_obj) = metadata.as_object_mut() {
            metadata_obj.insert(
                "registration_endpoint".to_string(),
                json!(format!("{base_url}/oauth/register")),
            );
        }

        let app = Router::new()
            .route(
                "/.well-known/oauth-authorization-server/mcp",
                get({
                    let metadata = metadata.clone();
                    move || async move { Json(metadata.clone()) }
                }),
            )
            .route(
                "/mcp/.well-known/oauth-authorization-server",
                get({
                    let metadata = metadata.clone();
                    move || async move { Json(metadata.clone()) }
                }),
            )
            .route(
                "/.well-known/oauth-authorization-server",
                get(move || async move { Json(metadata.clone()) }),
            )
            .route(
                "/oauth/register",
                post(move || async move {
                    if registration_fails {
                        return (
                            StatusCode::FORBIDDEN,
                            Json(json!({
                                "error": "access_denied",
                                "error_description": "dynamic registration denied by policy",
                            })),
                        );
                    }
                    (
                        StatusCode::CREATED,
                        Json(json!({
                            "client_id": "codex-test-client-id",
                            "client_secret": null,
                            "client_name": "Codex Test Client",
                            "redirect_uris": [],
                        })),
                    )
                }),
            );

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("oauth metadata server should run");
        });

        (format!("{base_url}/mcp"), handle)
    }
}
