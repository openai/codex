use std::collections::HashMap;
use std::net::SocketAddr;
use std::string::String;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_exec_server::HttpClient;
use rmcp::transport::AuthorizationManager;
use rmcp::transport::AuthorizationSession;
use rmcp::transport::auth::AuthorizationMetadata;
use rmcp::transport::auth::OAuthClientConfig;
use rmcp::transport::auth::OAuthHttpClient;
use rmcp::transport::auth::OAuthState;
use tiny_http::Response;
use tiny_http::Server;
use tokio::sync::oneshot;
use tokio::time::timeout;
use url::Url;
use urlencoding::decode;

use crate::StoredOAuthTokens;
use crate::WrappedOAuthTokenResponse;
use crate::enterprise_oauth_login::enterprise_authorization_url;
use crate::enterprise_oauth_login::enterprise_callback_settings;
use crate::enterprise_oauth_login::resolve_enterprise_authorization_manager;
use crate::http_client_adapter::StreamableHttpRedirectMode;
use crate::oauth::compute_expires_at_millis;
use crate::oauth::validate_authorization_server_endpoints;
use crate::oauth_callback::McpOAuthCallbackMode;
use crate::oauth_callback::append_callback_id_to_redirect_uri;
use crate::oauth_callback::callback_id_from_server_url;
use crate::oauth_callback::callback_mode;
use crate::oauth_callback::resolve_mcp_oauth_callback_url;
use crate::oauth_callback::validate_callback_redirect;
use crate::oauth_client_registration::McpOAuthClientRegistration;
use crate::oauth_client_registration::PreparedOAuthLogin;
use crate::oauth_client_registration::start_authorization as start_client_registration;
use crate::oauth_http_client::OAuthHttpClientAdapter;
use crate::save_oauth_tokens;
use crate::utils::build_default_headers;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;

#[path = "oauth_callback_input.rs"]
mod callback_input;
pub use callback_input::perform_oauth_login_with_callback_input;

#[derive(Clone, Copy)]
pub(crate) enum OAuthLoginPurpose {
    Mcp,
    EnterpriseIdp,
}

pub(crate) struct OAuthHttpContext {
    pub(crate) http_headers: Option<HashMap<String, String>>,
    pub(crate) env_http_headers: Option<HashMap<String, String>>,
    pub(crate) http_client: Arc<dyn HttpClient>,
    pub(crate) redirect_mode: StreamableHttpRedirectMode,
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
    keyring_backend_kind: AuthKeyringBackendKind,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    client_registration: McpOAuthClientRegistration,
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
    global_callback_url: Option<&str>,
    http_client: Arc<dyn HttpClient>,
) -> Result<()> {
    perform_oauth_login_with_browser_output(
        server_name,
        server_url,
        store_mode,
        keyring_backend_kind,
        http_headers,
        env_http_headers,
        scopes,
        oauth_client_id,
        client_registration,
        oauth_resource,
        callback_port,
        callback_url,
        global_callback_url,
        http_client,
        /*emit_browser_url*/ true,
        StreamableHttpRedirectMode::Legacy,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login_silent(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    client_registration: McpOAuthClientRegistration,
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
    global_callback_url: Option<&str>,
    http_client: Arc<dyn HttpClient>,
    redirect_mode: StreamableHttpRedirectMode,
) -> Result<()> {
    perform_oauth_login_with_browser_output(
        server_name,
        server_url,
        store_mode,
        keyring_backend_kind,
        http_headers,
        env_http_headers,
        scopes,
        oauth_client_id,
        client_registration,
        oauth_resource,
        callback_port,
        callback_url,
        global_callback_url,
        http_client,
        /*emit_browser_url*/ false,
        redirect_mode,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn perform_oauth_login_with_browser_output(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    client_registration: McpOAuthClientRegistration,
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
    global_callback_url: Option<&str>,
    http_client: Arc<dyn HttpClient>,
    emit_browser_url: bool,
    redirect_mode: StreamableHttpRedirectMode,
) -> Result<()> {
    let http_context = OAuthHttpContext {
        http_headers,
        env_http_headers,
        http_client,
        redirect_mode,
    };
    OauthLoginFlow::new(
        server_name,
        server_url,
        store_mode,
        keyring_backend_kind,
        http_context,
        scopes,
        oauth_client_id,
        OAuthLoginPurpose::Mcp,
        client_registration,
        oauth_resource,
        /*launch_browser*/ true,
        callback_port,
        callback_url,
        global_callback_url,
        /*timeout_secs*/ None,
    )
    .await?
    .finish(emit_browser_url)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login_return_url(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    client_registration: McpOAuthClientRegistration,
    oauth_resource: Option<&str>,
    timeout_secs: Option<i64>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
    global_callback_url: Option<&str>,
    http_client: Arc<dyn HttpClient>,
    redirect_mode: StreamableHttpRedirectMode,
) -> Result<OauthLoginHandle> {
    let http_context = OAuthHttpContext {
        http_headers,
        env_http_headers,
        http_client,
        redirect_mode,
    };
    let flow = OauthLoginFlow::new(
        server_name,
        server_url,
        store_mode,
        keyring_backend_kind,
        http_context,
        scopes,
        oauth_client_id,
        OAuthLoginPurpose::Mcp,
        client_registration,
        oauth_resource,
        /*launch_browser*/ false,
        callback_port,
        callback_url,
        global_callback_url,
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
                CallbackOutcome::Success(OauthCallbackResult {
                    code,
                    state,
                    issuer,
                }) => {
                    let response = Response::from_string(
                        "Authentication complete. You may close this window.",
                    );
                    if let Err(err) = request.respond(response) {
                        eprintln!("Failed to respond to OAuth callback: {err}");
                    }
                    if let Err(message) = send_oauth_callback(
                        tx,
                        CallbackResult::Success(OauthCallbackResult {
                            code,
                            state,
                            issuer,
                        }),
                    ) {
                        eprintln!("{message}");
                    }
                    break;
                }
                CallbackOutcome::Error(error) => {
                    let response = Response::from_string(error.to_string()).with_status_code(400);
                    if let Err(err) = request.respond(response) {
                        eprintln!("Failed to respond to OAuth callback: {err}");
                    }
                    if let Err(message) = send_oauth_callback(tx, CallbackResult::Error(error)) {
                        eprintln!("{message}");
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
    issuer: Option<String>,
}

#[derive(Debug)]
enum CallbackResult {
    Success(OauthCallbackResult),
    Error(OAuthProviderError),
}

fn send_oauth_callback(
    tx: oneshot::Sender<CallbackResult>,
    result: CallbackResult,
) -> std::result::Result<(), &'static str> {
    tx.send(result)
        .map_err(|_| "OAuth callback receiver closed")
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
    let mut issuer = None;

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
            "iss" => issuer = Some(decoded),
            _ => {}
        }
    }

    if let (Some(code), Some(state)) = (code, state) {
        return CallbackOutcome::Success(OauthCallbackResult {
            code,
            state,
            issuer,
        });
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

pub(crate) struct OauthLoginFlow {
    auth_url: String,
    redirect_uri: String,
    oauth_state: OAuthState,
    authorization_server_issuer: Option<String>,
    rx: oneshot::Receiver<CallbackResult>,
    guard: CallbackServerGuard,
    server_name: String,
    server_url: String,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    launch_browser: bool,
    timeout: Duration,
}

fn resolve_callback_port(callback_port: Option<u16>) -> Result<Option<u16>> {
    if let Some(config_port) = callback_port {
        if config_port == 0 {
            bail!(
                "invalid MCP OAuth callback port `{config_port}`: port must be between 1 and 65535"
            );
        }
        return Ok(Some(config_port));
    }

    Ok(None)
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
    let mut parsed = Url::parse(callback_url)
        .with_context(|| format!("invalid MCP OAuth callback URL `{callback_url}`"))?;

    // Registered loopback callbacks omit the temporary listener port because
    // the OS can assign a different port on every login. Add the active port
    // only to this authorization request; RFC 8252 requires authorization
    // servers to accept any request-time port for loopback IP redirects.
    // https://www.rfc-editor.org/rfc/rfc8252#section-7.3
    if parsed.scheme() == "http"
        && parsed.host_str() == Some("127.0.0.1")
        && parsed.port().is_none()
    {
        let listener_port = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| anyhow!("unable to determine OAuth callback listener port"))?
            .port();
        parsed
            .set_port(Some(listener_port))
            .map_err(|()| anyhow!("unable to set OAuth callback listener port"))?;
        return Ok(parsed.to_string());
    }

    Ok(callback_url.to_string())
}

fn callback_path_from_redirect_uri(redirect_uri: &str) -> Result<String> {
    let parsed = Url::parse(redirect_uri)
        .with_context(|| format!("invalid redirect URI `{redirect_uri}`"))?;
    Ok(parsed.path().to_string())
}

fn callback_bind_host(callback_url: Option<&str>) -> &'static str {
    let Some(callback_url) = callback_url else {
        return "127.0.0.1";
    };

    let Ok(parsed) = Url::parse(callback_url) else {
        return "127.0.0.1";
    };

    match parsed.host_str() {
        Some("localhost" | "127.0.0.1" | "::1") | None => "127.0.0.1",
        Some(_) => "0.0.0.0",
    }
}

impl OauthLoginFlow {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new(
        server_name: &str,
        server_url: &str,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        http_context: OAuthHttpContext,
        scopes: &[String],
        oauth_client_id: Option<&str>,
        purpose: OAuthLoginPurpose,
        client_registration: McpOAuthClientRegistration,
        oauth_resource: Option<&str>,
        launch_browser: bool,
        callback_port: Option<u16>,
        callback_url: Option<&str>,
        global_callback_url: Option<&str>,
        timeout_secs: Option<i64>,
    ) -> Result<Self> {
        const DEFAULT_OAUTH_TIMEOUT_SECS: i64 = 300;

        let callback_port = resolve_callback_port(callback_port)?;
        let is_enterprise_idp = matches!(purpose, OAuthLoginPurpose::EnterpriseIdp);
        let (enterprise_bind_ip, callback_port) = if is_enterprise_idp {
            let (ip, port) = enterprise_callback_settings(
                server_url,
                oauth_client_id,
                callback_url,
                callback_port,
            )?;
            (Some(ip), port)
        } else {
            (None, callback_port)
        };
        let callback_id = callback_id_from_server_url(server_url)?;
        let oauth_client_id = oauth_client_id.filter(|client_id| !client_id.trim().is_empty());
        let configured_callback = if oauth_client_id.is_some() {
            callback_url
                .map(|callback_url| {
                    Url::parse(callback_url)
                        .with_context(|| format!("invalid MCP OAuth callback URL `{callback_url}`"))
                })
                .transpose()?
        } else {
            None
        };

        let OAuthHttpContext {
            http_headers,
            env_http_headers,
            http_client,
            redirect_mode,
        } = http_context;
        let has_configured_headers = http_headers
            .as_ref()
            .is_some_and(|headers| !headers.is_empty())
            || env_http_headers
                .as_ref()
                .is_some_and(|headers| !headers.is_empty());
        let default_headers = build_default_headers(http_headers, env_http_headers)?;
        let oauth_http_client: Arc<dyn OAuthHttpClient> =
            Arc::new(OAuthHttpClientAdapter::new_with_redirect_mode(
                http_client,
                default_headers,
                server_url,
                has_configured_headers,
                redirect_mode,
            )?);
        let registered_authorization = if oauth_client_id.is_some() {
            Some(
                resolve_authorization_manager(server_url, Arc::clone(&oauth_http_client), purpose)
                    .await?,
            )
        } else {
            None
        };
        let registered_callback_mode = registered_authorization
            .as_ref()
            .map(|(_, metadata)| callback_mode(metadata))
            .transpose()?;
        let use_legacy_fallback = !is_enterprise_idp
            && registered_callback_mode == Some(McpOAuthCallbackMode::CallbackSpecific)
            && configured_callback.as_ref().is_some_and(|callback_url| {
                callback_url
                    .path_segments()
                    .and_then(|mut segments| segments.next_back())
                    != Some(callback_id.as_str())
            });
        let callback_url = if use_legacy_fallback {
            // Any preregistered client's callback can lack its required ID when
            // the authorization server does not support issuer binding. This
            // especially affects plugins, whose callbacks are configured before
            // metadata discovery. Preserve compatibility and avoid making every
            // login fail by using the global/default callback instead; its
            // required server-specific callback ID is appended below.
            global_callback_url
        } else {
            callback_url
        };

        let bind_ip = match enterprise_bind_ip {
            Some(ip) => ip,
            None => callback_bind_host(callback_url).parse()?,
        };
        // Port zero asks the OS for a free ephemeral port; the resolved
        // redirect receives that port after the listener has been bound.
        let bind_addr = SocketAddr::new(bind_ip, callback_port.unwrap_or(0));
        let server = Arc::new(Server::http(bind_addr).map_err(|err| anyhow!(err))?);
        let guard = CallbackServerGuard {
            server: Arc::clone(&server),
        };
        let redirect_uri = resolve_redirect_uri(&server, callback_url)?;
        let redirect_uri = if is_enterprise_idp {
            let listener_port = server
                .server_addr()
                .to_ip()
                .ok_or_else(|| anyhow!("unable to determine enterprise callback listener port"))?
                .port();
            let mut redirect = Url::parse(&redirect_uri)?;
            redirect
                .set_port(Some(listener_port))
                .map_err(|()| anyhow!("invalid enterprise callback port"))?;
            redirect.to_string()
        } else {
            redirect_uri
        };

        let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
        let PreparedOAuthLogin {
            oauth_state,
            authorization_server_issuer,
            redirect_uri,
        } = if let Some((oauth_client_id, (auth_manager, metadata))) =
            oauth_client_id.zip(registered_authorization)
        {
            let redirect_uri = if is_enterprise_idp {
                resolve_mcp_oauth_callback_url(
                    server_url,
                    Some(&redirect_uri),
                    callback_mode(&metadata)?,
                )?
            } else if callback_url.is_some() && !use_legacy_fallback {
                redirect_uri
            } else {
                append_callback_id_to_redirect_uri(&redirect_uri, &callback_id)?
            };
            start_authorization(
                auth_manager,
                metadata,
                &scope_refs,
                &redirect_uri,
                &callback_id,
                oauth_client_id,
                purpose,
            )
            .await?
        } else {
            start_client_registration(
                server_url,
                oauth_http_client,
                &scope_refs,
                &redirect_uri,
                &callback_id,
                client_registration,
            )
            .await?
        };
        let callback_path = callback_path_from_redirect_uri(&redirect_uri)?;
        let (tx, rx) = oneshot::channel();
        spawn_callback_server(server, tx, callback_path);
        let auth_url = append_query_param(
            &oauth_state.get_authorization_url().await?,
            "resource",
            oauth_resource,
        );
        let timeout_secs = timeout_secs.unwrap_or(DEFAULT_OAUTH_TIMEOUT_SECS).max(1);
        let timeout = Duration::from_secs(timeout_secs as u64);

        Ok(Self {
            auth_url,
            redirect_uri,
            oauth_state,
            authorization_server_issuer,
            rx,
            guard,
            server_name: server_name.to_string(),
            server_url: server_url.to_string(),
            store_mode,
            keyring_backend_kind,
            launch_browser,
            timeout,
        })
    }

    pub(crate) fn authorization_url(&self) -> String {
        self.auth_url.clone()
    }

    async fn finish(self, emit_browser_url: bool) -> Result<()> {
        let store_mode = self.store_mode;
        let keyring_backend_kind = self.keyring_backend_kind;
        let stored = self.complete(emit_browser_url).await?;
        save_oauth_tokens(
            &stored.server_name,
            &stored,
            store_mode,
            keyring_backend_kind,
        )
        .await
    }

    pub(crate) async fn complete(mut self, emit_browser_url: bool) -> Result<StoredOAuthTokens> {
        if self.launch_browser {
            let server_name = &self.server_name;
            let auth_url = &self.auth_url;
            if emit_browser_url {
                println!(
                    "Authorize `{server_name}` by opening this URL in your browser:\n{auth_url}\n"
                );
            }

            if webbrowser::open(auth_url).is_err() {
                if !emit_browser_url {
                    eprintln!(
                        "Authorize `{server_name}` by opening this URL in your browser:\n{auth_url}\n"
                    );
                }
                eprintln!("(Browser launch failed; please copy the URL above manually.)");
            }
        }

        let callback = timeout(self.timeout, &mut self.rx)
            .await
            .context("timed out waiting for OAuth callback")?
            .context("OAuth callback was cancelled")?;
        self.complete_callback(callback).await
    }

    async fn complete_callback(mut self, callback: CallbackResult) -> Result<StoredOAuthTokens> {
        let result = async {
            let OauthCallbackResult {
                code,
                state: csrf_state,
                issuer,
            } = match callback {
                CallbackResult::Success(callback) => callback,
                CallbackResult::Error(error) => return Err(anyhow!(error)),
            };

            self.oauth_state
                .handle_callback_with_issuer(&code, &csrf_state, issuer.as_deref())
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
            Ok(StoredOAuthTokens {
                server_name: self.server_name.clone(),
                url: self.server_url.clone(),
                issuer: self.authorization_server_issuer.clone(),
                client_id,
                token_response: WrappedOAuthTokenResponse(credentials),
                expires_at,
            })
        }
        .await;

        drop(self.guard);
        result
    }

    fn spawn(self) -> oneshot::Receiver<Result<()>> {
        let server_name = self.server_name.clone();
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let result = self.finish(/*emit_browser_url*/ false).await;
            if let Err(err) = &result {
                eprintln!("Failed to complete OAuth login for '{server_name}': {err:#}");
            }

            let _ = tx.send(result);
        });

        rx
    }
}

async fn resolve_authorization_manager(
    server_url: &str,
    http_client: Arc<dyn OAuthHttpClient>,
    purpose: OAuthLoginPurpose,
) -> Result<(AuthorizationManager, AuthorizationMetadata)> {
    if matches!(purpose, OAuthLoginPurpose::EnterpriseIdp) {
        return resolve_enterprise_authorization_manager(server_url, http_client).await;
    }
    let mut auth_manager =
        AuthorizationManager::new_with_oauth_http_client(server_url, http_client).await?;
    auth_manager.set_allow_missing_issuer(true);
    let metadata = auth_manager.resolve_metadata().await?.metadata;
    validate_authorization_server_endpoints(&metadata)?;
    Ok((auth_manager, metadata))
}

async fn start_authorization(
    mut auth_manager: AuthorizationManager,
    metadata: AuthorizationMetadata,
    scopes: &[&str],
    redirect_uri: &str,
    callback_id: &str,
    oauth_client_id: &str,
    purpose: OAuthLoginPurpose,
) -> Result<PreparedOAuthLogin> {
    let strict_enterprise_idp = matches!(purpose, OAuthLoginPurpose::EnterpriseIdp);
    let authorization_server_issuer = metadata.issuer.clone();
    validate_callback_redirect(redirect_uri, callback_id, callback_mode(&metadata)?)?;
    auth_manager.set_metadata(metadata);
    let client_config = OAuthClientConfig::new(oauth_client_id, redirect_uri)
        .with_scopes(scopes.iter().map(|scope| (*scope).to_string()).collect());
    auth_manager.configure_client(client_config)?;
    let auth_url = auth_manager.get_authorization_url(scopes).await?;
    let auth_url = if strict_enterprise_idp {
        enterprise_authorization_url(&auth_url)?
    } else {
        auth_url
    };

    Ok(PreparedOAuthLogin {
        oauth_state: OAuthState::Session(AuthorizationSession::for_scope_upgrade(
            auth_manager,
            auth_url,
            redirect_uri,
        )),
        authorization_server_issuer,
        redirect_uri: redirect_uri.to_string(),
    })
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
    use std::collections::HashMap;
    use std::io::Read;
    use std::io::Write;
    use std::net::TcpStream;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use axum::Json;
    use axum::Router;
    use axum::routing::get;
    use axum::routing::post;
    use codex_config::types::AuthKeyringBackendKind;
    use codex_config::types::OAuthCredentialsStoreMode;
    use codex_exec_server::ExecServerError;
    use codex_exec_server::HttpClient;
    use codex_exec_server::HttpRequestParams;
    use codex_exec_server::HttpRequestResponse;
    use codex_exec_server::HttpResponseBodyStream;
    use codex_exec_server::RouteAwareHttpClient;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use futures::future::BoxFuture;
    use http::HeaderMap;
    use oauth2::TokenResponse;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tokio::net::TcpListener;
    use url::Url;

    use super::CallbackOutcome;
    use super::McpOAuthClientRegistration;
    use super::OAuthHttpClientAdapter;
    use super::OAuthHttpContext;
    use super::OAuthLoginPurpose;
    use super::OAuthProviderError;
    use super::OauthLoginFlow;
    use super::StreamableHttpRedirectMode;
    use super::append_callback_id_to_redirect_uri;
    use super::append_query_param;
    use super::callback_id_from_server_url;
    use super::callback_path_from_redirect_uri;
    use super::parse_oauth_callback;
    use super::perform_oauth_login;
    use super::perform_oauth_login_silent;
    use super::resolve_authorization_manager;
    use super::start_authorization;
    use crate::oauth::stored_oauth_credentials;
    use crate::oauth::test_support::TempCodexHome;

    #[derive(Default)]
    struct RecordingHttpClient {
        requests: AtomicUsize,
    }

    impl HttpClient for RecordingHttpClient {
        fn http_request(
            &self,
            _params: HttpRequestParams,
        ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
            Box::pin(async {
                Err(ExecServerError::HttpRequest(
                    "unexpected buffered OAuth request".to_string(),
                ))
            })
        }

        fn http_request_stream(
            &self,
            _params: HttpRequestParams,
        ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>>
        {
            self.requests.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Err(ExecServerError::HttpRequest(
                    "configured OAuth client was used".to_string(),
                ))
            })
        }
    }

    async fn spawn_oauth_metadata_server() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind metadata listener");
        let addr = listener.local_addr().expect("read metadata listener addr");
        let base_url = format!("http://{addr}");
        let metadata = json!({
            "issuer": format!("{base_url}/mcp"),
            "authorization_endpoint": format!("{base_url}/oauth/authorize"),
            "token_endpoint": format!("{base_url}/oauth/token"),
            "registration_endpoint": format!("{base_url}/oauth/register"),
            "scopes_supported": ["read", "offline_access"],
        });
        let registration_requests = Arc::new(AtomicUsize::new(0));
        let captured_registration_requests = Arc::clone(&registration_requests);
        let path_scoped_metadata = metadata.clone();
        let app = Router::new()
            .route(
                "/.well-known/oauth-authorization-server/mcp",
                get(move || {
                    let metadata = path_scoped_metadata.clone();
                    async move { Json(metadata) }
                }),
            )
            .route(
                "/.well-known/oauth-authorization-server",
                get(move || {
                    let metadata = metadata.clone();
                    async move { Json(metadata) }
                }),
            )
            .route(
                "/oauth/register",
                post(move || {
                    let registration_requests = Arc::clone(&captured_registration_requests);
                    async move {
                        registration_requests.fetch_add(1, Ordering::SeqCst);
                        Json(json!({"client_id": "unexpected-dynamic-client"}))
                    }
                }),
            )
            .route(
                "/oauth/token",
                post(|| async {
                    Json(json!({
                        "access_token": "test-access-token",
                        "token_type": "Bearer",
                    }))
                }),
            );

        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve oauth metadata");
        });

        (base_url, registration_requests)
    }

    async fn send_oauth_callback(callback_url: Url) -> anyhow::Result<()> {
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let host = callback_url
                .host_str()
                .ok_or_else(|| anyhow::anyhow!("callback URL should include a host"))?;
            let port = callback_url
                .port()
                .ok_or_else(|| anyhow::anyhow!("callback URL should include a port"))?;
            let mut stream = TcpStream::connect((host, port))?;
            let mut path = callback_url.path().to_string();
            if let Some(query) = callback_url.query() {
                path.push('?');
                path.push_str(query);
            }
            write!(
                stream,
                "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
            )?;
            let mut response = String::new();
            stream.read_to_string(&mut response)?;
            anyhow::ensure!(
                response.starts_with("HTTP/1.1 200"),
                "OAuth callback failed: {response}"
            );
            Ok(())
        })
        .await?
    }

    #[tokio::test]
    async fn ordinary_oauth_login_persists_issuer_without_a_refresh_token() -> anyhow::Result<()> {
        let _env = TempCodexHome::new();
        let (base_url, _registration_requests) = spawn_oauth_metadata_server().await;
        let server_url = format!("{base_url}/mcp");
        let flow = OauthLoginFlow::new(
            "issuer-persistence-test",
            &server_url,
            OAuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::Direct,
            OAuthHttpContext {
                http_headers: None,
                env_http_headers: None,
                http_client: Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                    OutboundProxyPolicy::ReqwestDefault,
                ))),
                redirect_mode: StreamableHttpRedirectMode::Legacy,
            },
            &[],
            Some("test-client"),
            OAuthLoginPurpose::Mcp,
            McpOAuthClientRegistration::Auto,
            /*oauth_resource*/ None,
            /*launch_browser*/ false,
            /*callback_port*/ None,
            /*callback_url*/ None,
            /*global_callback_url*/ None,
            Some(/*timeout_secs*/ 5),
        )
        .await?;
        let authorization_url = Url::parse(&flow.authorization_url())?;
        let query = authorization_url.query_pairs().collect::<HashMap<_, _>>();
        let redirect_uri = query
            .get("redirect_uri")
            .ok_or_else(|| anyhow::anyhow!("authorization URL should include redirect_uri"))?;
        let state = query
            .get("state")
            .ok_or_else(|| anyhow::anyhow!("authorization URL should include state"))?;
        let mut callback_url = Url::parse(redirect_uri)?;
        callback_url
            .query_pairs_mut()
            .append_pair("code", "test-code")
            .append_pair("state", state);
        send_oauth_callback(callback_url).await?;
        flow.finish(/*emit_browser_url*/ false).await?;

        let stored = stored_oauth_credentials(
            "issuer-persistence-test",
            &server_url,
            OAuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::Direct,
        )?
        .expect("OAuth login should persist credentials");
        assert_eq!(stored.issuer.as_deref(), Some(server_url.as_str()));
        assert!(stored.token_response.0.refresh_token().is_none());
        Ok(())
    }

    #[tokio::test]
    async fn configured_client_preserves_exact_scopes_and_redirect_without_registration() {
        for (scopes, expected_scope) in [(&[][..], None), (&["read"][..], Some("read"))] {
            let (base_url, registration_requests) = spawn_oauth_metadata_server().await;
            let redirect_uri = "http://127.0.0.1:43123/callback/configured-client";
            let (auth_manager, metadata) = resolve_authorization_manager(
                &format!("{base_url}/mcp"),
                Arc::new(OAuthHttpClientAdapter::new(
                    Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                        OutboundProxyPolicy::ReqwestDefault,
                    ))),
                    HeaderMap::new(),
                    &format!("{base_url}/mcp"),
                )),
                OAuthLoginPurpose::Mcp,
            )
            .await
            .expect("resolve pre-registered OAuth metadata");
            let prepared = start_authorization(
                auth_manager,
                metadata,
                scopes,
                redirect_uri,
                "configured-client",
                "eci-prd-pub-codex-123",
                OAuthLoginPurpose::Mcp,
            )
            .await
            .expect("start pre-registered OAuth authorization");
            let oauth_state = prepared.oauth_state;

            let authorization_url = oauth_state
                .get_authorization_url()
                .await
                .expect("read authorization URL");
            let query = Url::parse(&authorization_url)
                .expect("authorization URL should parse")
                .query_pairs()
                .into_owned()
                .collect::<HashMap<String, String>>();

            assert_eq!(
                query.get("client_id").map(String::as_str),
                Some("eci-prd-pub-codex-123")
            );
            assert_eq!(
                query.get("redirect_uri").map(String::as_str),
                Some(redirect_uri)
            );
            assert_eq!(query.get("scope").map(String::as_str), expected_scope);
            assert_eq!(registration_requests.load(Ordering::SeqCst), 0);
        }
    }
    #[tokio::test]
    async fn oauth_callback_validates_rfc_9207_issuer_before_token_exchange() {
        for (supports_issuer, callback_issuer, expected_token_requests) in [
            (true, Some("matching"), 1),
            (true, Some("mismatched"), 0),
            (true, None, 0),
            (false, Some("mismatched"), 0),
            (false, None, 1),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind authorization metadata listener");
            let issuer = format!(
                "http://{}",
                listener.local_addr().expect("read listener address")
            );
            let token_requests = Arc::new(AtomicUsize::new(0));
            let captured_token_requests = Arc::clone(&token_requests);
            let authorization_issuer = format!("{issuer}/mcp");
            let metadata = json!({
                "issuer": authorization_issuer,
                "authorization_endpoint": format!("{issuer}/authorize"),
                "token_endpoint": format!("{issuer}/token"),
                "authorization_response_iss_parameter_supported": supports_issuer,
            });
            let scoped_metadata = metadata.clone();
            let app = Router::new()
                .route(
                    "/.well-known/oauth-authorization-server/mcp",
                    get(move || {
                        let metadata = scoped_metadata.clone();
                        async move { Json(metadata) }
                    }),
                )
                .route(
                    "/.well-known/oauth-authorization-server",
                    get(move || {
                        let metadata = metadata.clone();
                        async move { Json(metadata) }
                    }),
                )
                .route(
                    "/token",
                    post(move || {
                        let token_requests = Arc::clone(&captured_token_requests);
                        async move {
                            token_requests.fetch_add(1, Ordering::SeqCst);
                            Json(json!({"access_token":"test-token","token_type":"Bearer"}))
                        }
                    }),
                );
            let server = tokio::spawn(async move {
                axum::serve(listener, app)
                    .await
                    .expect("serve authorization metadata fixture");
            });
            let redirect_uri = if supports_issuer {
                "http://127.0.0.1/callback"
            } else {
                "http://127.0.0.1/callback/test-callback"
            };
            let (auth_manager, metadata) = resolve_authorization_manager(
                &format!("{issuer}/mcp"),
                Arc::new(OAuthHttpClientAdapter::new(
                    Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                        OutboundProxyPolicy::ReqwestDefault,
                    ))),
                    HeaderMap::new(),
                    &format!("{issuer}/mcp"),
                )),
                OAuthLoginPurpose::Mcp,
            )
            .await
            .expect("resolve issuer-aware authorization metadata");
            let prepared = start_authorization(
                auth_manager,
                metadata,
                &[],
                redirect_uri,
                "test-callback",
                "test-client",
                OAuthLoginPurpose::Mcp,
            )
            .await
            .expect("start issuer-aware authorization");
            let mut state = prepared.oauth_state;
            let csrf_state = Url::parse(
                &state
                    .get_authorization_url()
                    .await
                    .expect("retrieve authorization URL"),
            )
            .expect("parse authorization URL")
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned())
            .expect("authorization URL should contain state");
            let callback_issuer = match callback_issuer {
                Some("matching") => Some(authorization_issuer.as_str()),
                Some(_) => Some("https://unexpected.example"),
                None => None,
            };
            let result = state
                .handle_callback_with_issuer("test-code", &csrf_state, callback_issuer)
                .await;

            assert_eq!(
                token_requests.load(Ordering::SeqCst),
                expected_token_requests
            );
            assert_eq!(result.is_ok(), expected_token_requests == 1);

            if expected_token_requests == 0 {
                state
                    .handle_callback_with_issuer(
                        "legitimate-code",
                        &csrf_state,
                        Some(authorization_issuer.as_str()),
                    )
                    .await
                    .expect("issuer validation failures must preserve OAuth authorization state");
                assert_eq!(token_requests.load(Ordering::SeqCst), 1);
            }
            server.abort();
        }
    }

    #[tokio::test]
    async fn interactive_oauth_login_uses_supplied_http_client() {
        let http_client = Arc::new(RecordingHttpClient::default());
        perform_oauth_login(
            "configured-client",
            "http://127.0.0.1:1/mcp",
            OAuthCredentialsStoreMode::default(),
            AuthKeyringBackendKind::default(),
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            &[],
            /*oauth_client_id*/ None,
            McpOAuthClientRegistration::Auto,
            /*oauth_resource*/ None,
            /*callback_port*/ None,
            /*callback_url*/ None,
            /*global_callback_url*/ None,
            http_client.clone(),
        )
        .await
        .expect_err("OAuth metadata discovery should fail through the supplied client");

        assert!(http_client.requests.load(Ordering::SeqCst) > 0);
    }

    #[tokio::test]
    async fn silent_oauth_login_uses_supplied_http_client() {
        let http_client = Arc::new(RecordingHttpClient::default());
        perform_oauth_login_silent(
            "configured-client",
            "http://127.0.0.1:1/mcp",
            OAuthCredentialsStoreMode::default(),
            AuthKeyringBackendKind::default(),
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            &[],
            /*oauth_client_id*/ None,
            McpOAuthClientRegistration::Auto,
            /*oauth_resource*/ None,
            /*callback_port*/ None,
            /*callback_url*/ None,
            /*global_callback_url*/ None,
            http_client.clone(),
            StreamableHttpRedirectMode::Legacy,
        )
        .await
        .expect_err("OAuth metadata discovery should fail through the supplied client");

        assert!(http_client.requests.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn parse_oauth_callback_accepts_default_path() {
        let parsed = parse_oauth_callback("/callback?code=abc&state=xyz", "/callback");
        assert!(matches!(parsed, CallbackOutcome::Success(_)));
    }

    #[test]
    fn parse_oauth_callback_preserves_rfc_9207_issuer() {
        let parsed = parse_oauth_callback(
            "/callback?code=abc&state=xyz&iss=https%3A%2F%2Fissuer.example",
            "/callback",
        );
        assert_eq!(
            parsed,
            CallbackOutcome::Success(super::OauthCallbackResult {
                code: "abc".to_string(),
                state: "xyz".to_string(),
                issuer: Some("https://issuer.example".to_string()),
            })
        );
    }

    #[test]
    fn parse_oauth_callback_accepts_custom_path() {
        let parsed = parse_oauth_callback("/oauth/callback?code=abc&state=xyz", "/oauth/callback");
        assert!(matches!(parsed, CallbackOutcome::Success(_)));
    }

    #[test]
    fn parse_oauth_callback_accepts_callback_id_path() {
        let parsed =
            parse_oauth_callback("/callback/abc123?code=abc&state=xyz", "/callback/abc123");
        assert!(matches!(parsed, CallbackOutcome::Success(_)));
    }

    #[test]
    fn parse_oauth_callback_rejects_missing_callback_id_path() {
        let parsed = parse_oauth_callback("/callback?code=abc&state=xyz", "/callback/abc123");
        assert!(matches!(parsed, CallbackOutcome::Invalid));
    }

    #[test]
    fn parse_oauth_callback_rejects_wrong_path() {
        let parsed = parse_oauth_callback("/callback?code=abc&state=xyz", "/oauth/callback");
        assert!(matches!(parsed, CallbackOutcome::Invalid));
    }

    #[test]
    fn parse_oauth_callback_returns_provider_error() {
        let parsed = parse_oauth_callback(
            "/callback?error=invalid_scope&error_description=scope%20rejected",
            "/callback",
        );

        assert_eq!(
            parsed,
            CallbackOutcome::Error(OAuthProviderError::new(
                Some("invalid_scope".to_string()),
                Some("scope rejected".to_string()),
            ))
        );
    }

    #[test]
    fn callback_path_comes_from_redirect_uri() {
        let path = callback_path_from_redirect_uri("https://example.com/oauth/callback")
            .expect("redirect URI should parse");
        assert_eq!(path, "/oauth/callback");
    }

    #[test]
    fn callback_id_is_bound_to_server_url() {
        let callback_id = callback_id_from_server_url("https://mcp.example.com/mcp?tenant=one")
            .expect("server URL should parse");
        let same_without_fragment =
            callback_id_from_server_url("https://mcp.example.com/mcp?tenant=one#unused")
                .expect("server URL should parse");
        let different_path = callback_id_from_server_url("https://mcp.example.com/sse?tenant=one")
            .expect("server URL should parse");
        let different_query = callback_id_from_server_url("https://mcp.example.com/mcp?tenant=two")
            .expect("server URL should parse");
        let different_origin = callback_id_from_server_url("https://mcp.example.com:8443/mcp")
            .expect("server URL should parse");

        assert_eq!(callback_id, same_without_fragment);
        assert_ne!(callback_id, different_path);
        assert_ne!(callback_id, different_query);
        assert_ne!(callback_id, different_origin);
        assert_eq!(callback_id, "XuuuHAzzHOni");
    }

    #[test]
    fn callback_id_is_appended_to_redirect_uri_path() {
        let redirect_uri =
            append_callback_id_to_redirect_uri("http://127.0.0.1:1234/callback", "abc123")
                .expect("redirect URI should parse");

        assert_eq!(redirect_uri, "http://127.0.0.1:1234/callback/abc123");
        assert_eq!(
            append_callback_id_to_redirect_uri(&redirect_uri, "abc123")
                .expect("resolved redirect URI should parse"),
            redirect_uri
        );
    }

    #[test]
    fn callback_id_is_appended_before_redirect_uri_query() {
        let redirect_uri = append_callback_id_to_redirect_uri(
            "https://callbacks.example.com/oauth/callback?provider=github",
            "abc123",
        )
        .expect("redirect URI should parse");

        assert_eq!(
            redirect_uri,
            "https://callbacks.example.com/oauth/callback/abc123?provider=github"
        );
    }

    #[test]
    fn portless_loopback_callbacks_use_the_active_listener_port() {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("start callback listener");
        let listener_port = server
            .server_addr()
            .to_ip()
            .expect("resolve callback listener address")
            .port();

        for path in ["/callback", "/callback/callback-id", "/custom/callback"] {
            let callback = format!("http://127.0.0.1{path}");
            assert_eq!(
                super::resolve_redirect_uri(&server, Some(&callback))
                    .expect("insert active listener port"),
                format!("http://127.0.0.1:{listener_port}{path}")
            );
        }

        for callback in [
            "http://localhost/callback",
            "http://127.0.0.1:3080/callback",
            "https://127.0.0.1/callback",
            "https://devbox.example.com/callback",
        ] {
            assert_eq!(
                super::resolve_redirect_uri(&server, Some(callback))
                    .expect("preserve configured callback origin"),
                callback
            );
        }
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
}
