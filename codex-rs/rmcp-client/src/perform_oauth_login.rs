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
        scopes: &[String],
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
        let callback_path = callback_path_from_redirect_uri(&redirect_uri)?;

        let (tx, rx) = oneshot::channel();
        spawn_callback_server(server, tx, callback_path);

        let OauthHeaders {
            http_headers,
            env_http_headers,
        } = headers;
        let default_headers = build_default_headers(http_headers, env_http_headers)?;
        let http_client = apply_default_headers(ClientBuilder::new(), &default_headers).build()?;

        let mut oauth_state = OAuthState::new(server_url, Some(http_client)).await?;
        let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
        oauth_state
            .start_authorization(&scope_refs, &redirect_uri, Some("Codex"))
            .await?;
        let auth_url = oauth_state.get_authorization_url().await?;
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

#[cfg(test)]
mod tests {
    use super::CallbackOutcome;
    use super::callback_path_from_redirect_uri;
    use super::parse_oauth_callback;

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
}
