use std::collections::HashMap;
use std::string::String;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpRequestParams;
use oauth2::AsyncHttpClient;
use oauth2::AuthUrl;
use oauth2::AuthorizationCode;
use oauth2::ClientId;
use oauth2::ClientSecret;
use oauth2::CsrfToken;
use oauth2::EndpointNotSet;
use oauth2::EndpointSet;
use oauth2::HttpRequest;
use oauth2::HttpResponse;
use oauth2::PkceCodeChallenge;
use oauth2::PkceCodeVerifier;
use oauth2::RedirectUrl;
use oauth2::RequestTokenError;
use oauth2::RevocationErrorResponseType;
use oauth2::Scope;
use oauth2::StandardErrorResponse;
use oauth2::StandardRevocableToken;
use oauth2::StandardTokenIntrospectionResponse;
use oauth2::TokenUrl;
use oauth2::basic::BasicErrorResponseType;
use oauth2::basic::BasicTokenType;
use reqwest::Url;
use reqwest::header::HeaderMap;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use sha2::Digest;
use sha2::Sha256;
use tiny_http::Response;
use tiny_http::Server;
use tokio::sync::oneshot;
use tokio::time::timeout;
use urlencoding::decode;

use crate::StoredOAuthTokens;
use crate::WrappedOAuthTokenResponse;
use crate::auth_status::StreamableHttpOAuthMetadata;
use crate::auth_status::discover_streamable_http_oauth_metadata;
use crate::oauth::compute_expires_at_millis;
use crate::save_oauth_tokens;
use crate::utils::build_default_headers;
use crate::utils::oauth_token_headers;
use crate::utils::protocol_headers;
use codex_config::types::OAuthCredentialsStoreMode;

struct OauthHeaders {
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
}

type OAuthClient = oauth2::Client<
    StandardErrorResponse<BasicErrorResponseType>,
    OAuthTokenResponse,
    StandardTokenIntrospectionResponse<VendorExtraTokenFields, BasicTokenType>,
    StandardRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

type OAuthClientBuilder = oauth2::Client<
    StandardErrorResponse<BasicErrorResponseType>,
    OAuthTokenResponse,
    StandardTokenIntrospectionResponse<VendorExtraTokenFields, BasicTokenType>,
    StandardRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
>;

struct OAuthState {
    client: OAuthClient,
    client_id: String,
    pkce_verifier: PkceCodeVerifier,
    csrf_state: CsrfToken,
    authorization_url: String,
    default_headers: HeaderMap,
    http_client: Arc<dyn HttpClient>,
}

impl OAuthState {
    fn new(
        metadata: StreamableHttpOAuthMetadata,
        client: OAuthClientConfig,
        redirect_uri: &str,
        scopes: &[&str],
        default_headers: HeaderMap,
        http_client: Arc<dyn HttpClient>,
    ) -> Result<Self> {
        let OAuthClientConfig {
            client_id,
            client_secret,
        } = client;
        let mut client = OAuthClientBuilder::new(ClientId::new(client_id.clone()))
            .set_auth_uri(AuthUrl::new(metadata.authorization_endpoint)?)
            .set_token_uri(TokenUrl::new(metadata.token_endpoint)?)
            .set_redirect_uri(RedirectUrl::new(redirect_uri.to_string())?);
        if let Some(client_secret) = client_secret {
            client = client.set_client_secret(ClientSecret::new(client_secret));
        }
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let mut request = client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);
        for scope in scopes {
            request = request.add_scope(Scope::new((*scope).to_string()));
        }
        let (authorization_url, csrf_state) = request.url();

        Ok(Self {
            client,
            client_id,
            pkce_verifier,
            csrf_state,
            authorization_url: authorization_url.to_string(),
            default_headers,
            http_client,
        })
    }

    fn authorization_url(&self) -> &str {
        &self.authorization_url
    }

    async fn handle_callback(
        self,
        code: &str,
        csrf_state: &str,
    ) -> Result<(String, OAuthTokenResponse)> {
        if self.csrf_state.secret() != csrf_state {
            bail!("OAuth callback state did not match login request");
        }
        let http_client = RoutedOAuthHttpClient::new(self.http_client, self.default_headers);
        let credentials = match self
            .client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(self.pkce_verifier)
            .request_async(&http_client)
            .await
        {
            Ok(credentials) => credentials,
            Err(RequestTokenError::Parse(_, body)) => {
                serde_json::from_slice::<OAuthTokenResponse>(&body)?
            }
            Err(error) => return Err(anyhow!("OAuth token exchange failed: {error}")),
        };
        Ok((self.client_id, credentials))
    }
}

#[derive(Clone)]
struct RoutedOAuthHttpClient {
    http_client: Arc<dyn HttpClient>,
    default_headers: HeaderMap,
}

impl RoutedOAuthHttpClient {
    fn new(http_client: Arc<dyn HttpClient>, default_headers: HeaderMap) -> Self {
        Self {
            http_client,
            default_headers,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
struct RoutedOAuthHttpClientError(#[from] anyhow::Error);

impl<'c> AsyncHttpClient<'c> for RoutedOAuthHttpClient {
    type Error = RoutedOAuthHttpClientError;
    type Future = futures::future::BoxFuture<'c, Result<HttpResponse, Self::Error>>;

    fn call(&'c self, request: HttpRequest) -> Self::Future {
        Box::pin(async move {
            let (parts, body) = request.into_parts();
            let mut headers = self.default_headers.clone();
            headers.extend(parts.headers);
            let response = self
                .http_client
                .http_request(HttpRequestParams {
                    method: parts.method.to_string(),
                    url: parts.uri.to_string(),
                    headers: oauth_token_headers(&headers),
                    body: Some(body.into()),
                    timeout_ms: None,
                    request_id: "oauth-request".to_string(),
                    stream_response: false,
                })
                .await
                .map_err(|err| RoutedOAuthHttpClientError(anyhow!(err)))?;
            let mut builder = oauth2::http::Response::builder().status(response.status);
            for header in response.headers {
                builder = builder.header(header.name, header.value);
            }
            builder
                .body(response.body.into_inner())
                .map_err(|err: oauth2::http::Error| RoutedOAuthHttpClientError(anyhow!(err)))
        })
    }
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
    oauth_client_id: Option<&str>,
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
) -> Result<()> {
    perform_oauth_login_with_http_client(
        server_name,
        server_url,
        store_mode,
        http_headers,
        env_http_headers,
        scopes,
        oauth_client_id,
        oauth_resource,
        callback_port,
        callback_url,
        Arc::new(codex_exec_server::ReqwestHttpClient),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login_with_http_client(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
    http_client: Arc<dyn HttpClient>,
) -> Result<()> {
    perform_oauth_login_with_browser_output(
        server_name,
        server_url,
        store_mode,
        http_headers,
        env_http_headers,
        http_client,
        scopes,
        oauth_client_id,
        oauth_resource,
        callback_port,
        callback_url,
        /*emit_browser_url*/ true,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login_silent(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
) -> Result<()> {
    perform_oauth_login_silent_with_http_client(
        server_name,
        server_url,
        store_mode,
        http_headers,
        env_http_headers,
        scopes,
        oauth_client_id,
        oauth_resource,
        callback_port,
        callback_url,
        Arc::new(codex_exec_server::ReqwestHttpClient),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login_silent_with_http_client(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
    http_client: Arc<dyn HttpClient>,
) -> Result<()> {
    perform_oauth_login_with_browser_output(
        server_name,
        server_url,
        store_mode,
        http_headers,
        env_http_headers,
        http_client,
        scopes,
        oauth_client_id,
        oauth_resource,
        callback_port,
        callback_url,
        /*emit_browser_url*/ false,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn perform_oauth_login_with_browser_output(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    http_client: Arc<dyn HttpClient>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    oauth_resource: Option<&str>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
    emit_browser_url: bool,
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
        http_client,
        scopes,
        oauth_client_id,
        oauth_resource,
        /*launch_browser*/ true,
        callback_port,
        callback_url,
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
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    oauth_resource: Option<&str>,
    timeout_secs: Option<i64>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
) -> Result<OauthLoginHandle> {
    perform_oauth_login_return_url_with_http_client(
        server_name,
        server_url,
        store_mode,
        http_headers,
        env_http_headers,
        scopes,
        oauth_client_id,
        oauth_resource,
        timeout_secs,
        callback_port,
        callback_url,
        Arc::new(codex_exec_server::ReqwestHttpClient),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn perform_oauth_login_return_url_with_http_client(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    oauth_client_id: Option<&str>,
    oauth_resource: Option<&str>,
    timeout_secs: Option<i64>,
    callback_port: Option<u16>,
    callback_url: Option<&str>,
    http_client: Arc<dyn HttpClient>,
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
        http_client,
        scopes,
        oauth_client_id,
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
    Url::parse(callback_url)
        .with_context(|| format!("invalid MCP OAuth callback URL `{callback_url}`"))?;
    Ok(callback_url.to_string())
}

fn callback_id_from_server_url(server_url: &str) -> Result<String> {
    let mut parsed =
        Url::parse(server_url).with_context(|| format!("invalid MCP server URL `{server_url}`"))?;
    parsed
        .host_str()
        .ok_or_else(|| anyhow!("MCP server URL `{server_url}` must include a host"))?;
    parsed.set_fragment(None);

    let digest = Sha256::digest(parsed.as_str().as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(&digest[..9]))
}

fn append_callback_id_to_redirect_uri(redirect_uri: &str, callback_id: &str) -> Result<String> {
    let mut parsed = Url::parse(redirect_uri)
        .with_context(|| format!("invalid redirect URI `{redirect_uri}`"))?;
    let path = parsed.path();
    let new_path = if path.ends_with('/') {
        format!("{path}{callback_id}")
    } else {
        format!("{path}/{callback_id}")
    };
    parsed.set_path(&new_path);
    Ok(parsed.to_string())
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
    async fn new(
        server_name: &str,
        server_url: &str,
        store_mode: OAuthCredentialsStoreMode,
        headers: OauthHeaders,
        http_client: Arc<dyn HttpClient>,
        scopes: &[String],
        oauth_client_id: Option<&str>,
        oauth_resource: Option<&str>,
        launch_browser: bool,
        callback_port: Option<u16>,
        callback_url: Option<&str>,
        timeout_secs: Option<i64>,
    ) -> Result<Self> {
        const DEFAULT_OAUTH_TIMEOUT_SECS: i64 = 300;

        let bind_host = callback_bind_host(callback_url);
        let callback_port = resolve_callback_port(callback_port)?;
        let bind_addr = match callback_port {
            Some(port) => format!("{bind_host}:{port}"),
            None => format!("{bind_host}:0"),
        };

        let server = Arc::new(Server::http(&bind_addr).map_err(|err| anyhow!(err))?);
        let guard = CallbackServerGuard {
            server: Arc::clone(&server),
        };

        let redirect_uri = resolve_redirect_uri(&server, callback_url)?;
        let callback_id = callback_id_from_server_url(server_url)?;
        let redirect_uri = append_callback_id_to_redirect_uri(&redirect_uri, &callback_id)?;
        let callback_path = callback_path_from_redirect_uri(&redirect_uri)?;

        let (tx, rx) = oneshot::channel();
        spawn_callback_server(server, tx, callback_path);

        let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
        let oauth_state = start_authorization(
            server_name,
            server_url,
            http_client,
            headers,
            &scope_refs,
            &redirect_uri,
            oauth_client_id,
        )
        .await?;
        let auth_url =
            append_query_param(oauth_state.authorization_url(), "resource", oauth_resource);
        let timeout_secs = timeout_secs.unwrap_or(DEFAULT_OAUTH_TIMEOUT_SECS).max(1);
        let timeout = Duration::from_secs(timeout_secs as u64);

        Ok(Self {
            auth_url,
            oauth_state,
            rx,
            guard,
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

    async fn finish(mut self, emit_browser_url: bool) -> Result<()> {
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

            let (client_id, credentials) = self
                .oauth_state
                .handle_callback(&code, &csrf_state)
                .await
                .context("failed to handle OAuth callback")?;

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
            let result = self.finish(/*emit_browser_url*/ false).await;

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

async fn start_authorization(
    server_name: &str,
    server_url: &str,
    http_client: Arc<dyn HttpClient>,
    headers: OauthHeaders,
    scopes: &[&str],
    redirect_uri: &str,
    oauth_client_id: Option<&str>,
) -> Result<OAuthState> {
    let OauthHeaders {
        http_headers,
        env_http_headers,
    } = headers;
    let metadata = discover_streamable_http_oauth_metadata(
        server_url,
        http_headers.clone(),
        env_http_headers.clone(),
        Arc::clone(&http_client),
    )
    .await?
    .ok_or_else(|| anyhow!("MCP server `{server_name}` does not advertise OAuth metadata"))?;
    let default_headers = build_default_headers(http_headers.clone(), env_http_headers.clone())?;
    let client = match oauth_client_id.filter(|client_id| !client_id.trim().is_empty()) {
        Some(client_id) => OAuthClientConfig {
            client_id: client_id.to_string(),
            client_secret: None,
        },
        None => {
            register_oauth_client(
                &metadata,
                redirect_uri,
                http_headers,
                env_http_headers,
                Arc::clone(&http_client),
            )
            .await?
        }
    };
    OAuthState::new(
        metadata,
        client,
        redirect_uri,
        scopes,
        default_headers,
        http_client,
    )
}

async fn register_oauth_client(
    metadata: &StreamableHttpOAuthMetadata,
    redirect_uri: &str,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    http_client: Arc<dyn HttpClient>,
) -> Result<OAuthClientConfig> {
    let registration_url = metadata
        .registration_endpoint
        .as_ref()
        .ok_or_else(|| anyhow!("OAuth server does not support dynamic client registration"))?;
    let default_headers = build_default_headers(http_headers, env_http_headers)?;
    let registration_request = serde_json::json!({
        "client_name": "Codex",
        "redirect_uris": [redirect_uri],
        "grant_types": ["authorization_code", "refresh_token"],
        "token_endpoint_auth_method": "none",
        "response_types": ["code"],
    });
    let mut headers = default_headers;
    headers.insert(reqwest::header::CONTENT_TYPE, "application/json".parse()?);
    let response = http_client
        .http_request(HttpRequestParams {
            method: "POST".to_string(),
            url: registration_url.clone(),
            headers: protocol_headers(&headers),
            body: Some(serde_json::to_vec(&registration_request)?.into()),
            timeout_ms: None,
            request_id: "oauth-register".to_string(),
            stream_response: false,
        })
        .await?;
    if !(200..300).contains(&response.status) {
        bail!(
            "OAuth dynamic client registration returned HTTP {}",
            response.status
        );
    }
    #[derive(serde::Deserialize)]
    struct ClientRegistrationResponse {
        client_id: String,
        #[serde(default)]
        client_secret: Option<String>,
    }
    let response =
        serde_json::from_slice::<ClientRegistrationResponse>(&response.body.into_inner())?;
    Ok(OAuthClientConfig {
        client_id: response.client_id,
        client_secret: response
            .client_secret
            .filter(|client_secret| !client_secret.trim().is_empty()),
    })
}

struct OAuthClientConfig {
    client_id: String,
    client_secret: Option<String>,
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
    use std::sync::Arc;
    use std::sync::Mutex;

    use axum::Json;
    use axum::Router;
    use axum::routing::get;
    use codex_exec_server::ExecServerError;
    use codex_exec_server::HTTP_REQUEST_NO_REDIRECTS_HEADER;
    use codex_exec_server::HttpClient;
    use codex_exec_server::HttpRequestParams;
    use codex_exec_server::HttpRequestResponse;
    use codex_exec_server::HttpResponseBodyStream;
    use codex_exec_server::ReqwestHttpClient;
    use futures::FutureExt;
    use futures::future::BoxFuture;
    use oauth2::AsyncHttpClient;
    use pretty_assertions::assert_eq;
    use reqwest::Url;
    use reqwest::header::HeaderMap;
    use serde_json::json;
    use tokio::net::TcpListener;

    use super::CallbackOutcome;
    use super::OAuthProviderError;
    use super::OauthHeaders;
    use super::append_callback_id_to_redirect_uri;
    use super::append_query_param;
    use super::callback_id_from_server_url;
    use super::callback_path_from_redirect_uri;
    use super::parse_oauth_callback;
    use super::start_authorization;

    #[derive(Default)]
    struct RemoteOnlyHttpClient {
        requests: Mutex<Vec<HttpRequestParams>>,
    }

    impl RemoteOnlyHttpClient {
        fn requests(&self) -> Vec<HttpRequestParams> {
            self.requests.lock().expect("lock requests").clone()
        }
    }

    impl HttpClient for RemoteOnlyHttpClient {
        fn http_request(
            &self,
            params: HttpRequestParams,
        ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
            let url = params.url.clone();
            self.requests.lock().expect("lock requests").push(params);
            async move {
                let metadata = if url == "https://auth.remote.example/oauth/token" {
                    json!({
                        "access_token": "access-token",
                        "token_type": "bearer",
                    })
                } else {
                    json!({
                        "authorization_endpoint": "https://auth.remote.example/oauth/authorize",
                        "token_endpoint": "https://auth.remote.example/oauth/token",
                        "scopes_supported": ["scope:remote"],
                    })
                };
                Ok(HttpRequestResponse {
                    status: 200,
                    headers: Vec::new(),
                    body: serde_json::to_vec(&metadata)
                        .expect("serialize metadata")
                        .into(),
                })
            }
            .boxed()
        }

        fn http_request_stream(
            &self,
            _params: HttpRequestParams,
        ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>>
        {
            async move {
                Err(ExecServerError::HttpRequest(
                    "unexpected stream".to_string(),
                ))
            }
            .boxed()
        }
    }

    async fn spawn_oauth_metadata_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind metadata listener");
        let addr = listener.local_addr().expect("read metadata listener addr");
        let base_url = format!("http://{addr}");
        let metadata = json!({
            "authorization_endpoint": format!("{base_url}/oauth/authorize"),
            "token_endpoint": format!("{base_url}/oauth/token"),
            "scopes_supported": [""],
        });
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
            );

        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve oauth metadata");
        });

        base_url
    }

    #[tokio::test]
    async fn start_authorization_uses_configured_client_id() {
        let base_url = spawn_oauth_metadata_server().await;
        let oauth_state = start_authorization(
            "server",
            &format!("{base_url}/mcp"),
            Arc::new(ReqwestHttpClient),
            OauthHeaders {
                http_headers: None,
                env_http_headers: None,
            },
            &[],
            "http://127.0.0.1/callback",
            Some("eci-prd-pub-codex-123"),
        )
        .await
        .expect("start oauth authorization");

        let auth_url =
            Url::parse(oauth_state.authorization_url()).expect("authorization url should parse");
        let client_id = auth_url
            .query_pairs()
            .find(|(key, _)| key == "client_id")
            .map(|(_, value)| value.into_owned());

        assert_eq!(client_id.as_deref(), Some("eci-prd-pub-codex-123"));
    }

    #[tokio::test]
    async fn start_authorization_uses_selected_http_client_for_remote_only_server() {
        let http_client = Arc::new(RemoteOnlyHttpClient::default());
        let oauth_state = start_authorization(
            "remote-only",
            "http://remote-only.invalid/mcp",
            http_client.clone(),
            OauthHeaders {
                http_headers: None,
                env_http_headers: None,
            },
            &[],
            "http://127.0.0.1/callback",
            Some("remote-client-id"),
        )
        .await
        .expect("start oauth authorization through selected http client");

        let auth_url =
            Url::parse(oauth_state.authorization_url()).expect("authorization url should parse");
        let client_id = auth_url
            .query_pairs()
            .find(|(key, _)| key == "client_id")
            .map(|(_, value)| value.into_owned());
        assert_eq!(client_id.as_deref(), Some("remote-client-id"));
        assert_eq!(
            http_client
                .requests()
                .into_iter()
                .map(|request| request.url)
                .collect::<Vec<_>>(),
            vec![
                "http://remote-only.invalid/.well-known/oauth-authorization-server/mcp".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn routed_token_request_disables_redirects() {
        let http_client = Arc::new(RemoteOnlyHttpClient::default());
        let client = super::RoutedOAuthHttpClient::new(http_client.clone(), HeaderMap::new());
        let request = oauth2::http::Request::builder()
            .method("POST")
            .uri("https://auth.remote.example/oauth/token")
            .body(Vec::new())
            .expect("build token request");

        client
            .call(request)
            .await
            .expect("token request should succeed");

        assert!(
            http_client.requests()[0]
                .headers
                .iter()
                .any(|header| header.name == HTTP_REQUEST_NO_REDIRECTS_HEADER)
        );
    }

    #[tokio::test]
    async fn token_exchange_preserves_dynamic_registration_client_secret() {
        let http_client = Arc::new(RemoteOnlyHttpClient::default());
        let oauth_state = super::OAuthState::new(
            super::StreamableHttpOAuthMetadata {
                authorization_endpoint: "https://auth.remote.example/oauth/authorize".to_string(),
                token_endpoint: "https://auth.remote.example/oauth/token".to_string(),
                registration_endpoint: None,
                scopes_supported: None,
            },
            super::OAuthClientConfig {
                client_id: "dynamic-client".to_string(),
                client_secret: Some("dynamic-secret".to_string()),
            },
            "http://127.0.0.1/callback",
            &[],
            HeaderMap::new(),
            http_client.clone(),
        )
        .expect("build oauth state");
        let csrf_state = oauth_state.csrf_state.secret().to_string();

        oauth_state
            .handle_callback("code", &csrf_state)
            .await
            .expect("token exchange should succeed");

        let authorization = http_client
            .requests()
            .into_iter()
            .find(|request| request.url == "https://auth.remote.example/oauth/token")
            .and_then(|request| {
                request
                    .headers
                    .into_iter()
                    .find(|header| header.name.eq_ignore_ascii_case("authorization"))
            })
            .expect("token exchange should send authorization header");
        assert!(authorization.value.starts_with("Basic "));
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
        assert_eq!(callback_id.len(), 12);
        assert!(
            callback_id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        );
    }

    #[test]
    fn callback_id_is_appended_to_redirect_uri_path() {
        let redirect_uri =
            append_callback_id_to_redirect_uri("http://127.0.0.1:1234/callback", "abc123")
                .expect("redirect URI should parse");

        assert_eq!(redirect_uri, "http://127.0.0.1:1234/callback/abc123");
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
