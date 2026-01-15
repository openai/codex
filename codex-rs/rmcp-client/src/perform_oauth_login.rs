use std::collections::HashMap;
use std::string::String;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use reqwest::ClientBuilder;
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

pub async fn perform_oauth_login(
    server_name: &str,
    server_url: &str,
    store_mode: OAuthCredentialsStoreMode,
    http_headers: Option<HashMap<String, String>>,
    env_http_headers: Option<HashMap<String, String>>,
    scopes: &[String],
    callback_port: Option<u16>,
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
        timeout_secs,
    )
    .await?;

    let authorization_url = flow.authorization_url();
    let completion = flow.spawn();

    Ok(OauthLoginHandle::new(authorization_url, completion))
}

fn spawn_callback_server(server: Arc<Server>, tx: oneshot::Sender<(String, String)>) {
    tokio::task::spawn_blocking(move || {
        while let Ok(request) = server.recv() {
            let path = request.url().to_string();
            if let Some(OauthCallbackResult { code, state }) = parse_oauth_callback(&path) {
                let response =
                    Response::from_string("Authentication complete. You may close this window.");
                if let Err(err) = request.respond(response) {
                    eprintln!("Failed to respond to OAuth callback: {err}");
                }
                if let Err(err) = tx.send((code, state)) {
                    eprintln!("Failed to send OAuth callback: {err:?}");
                }
                break;
            } else {
                let response =
                    Response::from_string("Invalid OAuth callback").with_status_code(400);
                if let Err(err) = request.respond(response) {
                    eprintln!("Failed to respond to OAuth callback: {err}");
                }
            }
        }
    });
}

struct OauthCallbackResult {
    code: String,
    state: String,
}

fn parse_oauth_callback(path: &str) -> Option<OauthCallbackResult> {
    let (route, query) = path.split_once('?')?;
    if route != "/callback" {
        return None;
    }

    let mut code = None;
    let mut state = None;

    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        let decoded = decode(value).ok()?.into_owned();
        match key {
            "code" => code = Some(decoded),
            "state" => state = Some(decoded),
            _ => {}
        }
    }

    Some(OauthCallbackResult {
        code: code?,
        state: state?,
    })
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
        timeout_secs: Option<i64>,
    ) -> Result<Self> {
        const DEFAULT_OAUTH_TIMEOUT_SECS: i64 = 300;

        let callback_port = resolve_callback_port(callback_port)?;
        let bind_addr = match callback_port {
            Some(port) => format!("127.0.0.1:{port}"),
            None => "127.0.0.1:0".to_string(),
        };

        let server = Arc::new(Server::http(&bind_addr).map_err(|err| anyhow!(err))?);
        let guard = CallbackServerGuard {
            server: Arc::clone(&server),
        };

        let redirect_uri = match server.server_addr() {
            tiny_http::ListenAddr::IP(std::net::SocketAddr::V4(addr)) => {
                let ip = addr.ip();
                let port = addr.port();
                format!("http://{ip}:{port}/callback")
            }
            tiny_http::ListenAddr::IP(std::net::SocketAddr::V6(addr)) => {
                let ip = addr.ip();
                let port = addr.port();
                format!("http://[{ip}]:{port}/callback")
            }
            #[cfg(not(target_os = "windows"))]
            _ => return Err(anyhow!("unable to determine callback address")),
        };

        let (tx, rx) = oneshot::channel();
        spawn_callback_server(server, tx);

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
