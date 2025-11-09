use std::collections::HashMap;
use std::string::String;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
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
) -> Result<()> {
    let server = Arc::new(Server::http("127.0.0.1:0").map_err(|err| anyhow!(err))?);
    let guard = CallbackServerGuard {
        server: Arc::clone(&server),
    };

    let redirect_uri = match server.server_addr() {
        tiny_http::ListenAddr::IP(std::net::SocketAddr::V4(addr)) => {
            format!("http://{}:{}/callback", addr.ip(), addr.port())
        }
        tiny_http::ListenAddr::IP(std::net::SocketAddr::V6(addr)) => {
            format!("http://[{}]:{}/callback", addr.ip(), addr.port())
        }
        #[cfg(not(target_os = "windows"))]
        _ => return Err(anyhow!("unable to determine callback address")),
    };

    let (tx, rx) = oneshot::channel();
    spawn_callback_server(server, tx);

    let default_headers = build_default_headers(http_headers, env_http_headers)?;
    let http_client = apply_default_headers(ClientBuilder::new(), &default_headers).build()?;

    let mut oauth_state = OAuthState::new(server_url, Some(http_client)).await?;
    let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
    oauth_state
        .start_authorization(&scope_refs, &redirect_uri, Some("Codex"))
        .await?;
    let auth_url = oauth_state.get_authorization_url().await?;

    println!("Authorize `{server_name}` by opening this URL in your browser:\n{auth_url}\n");

    if webbrowser::open(&auth_url).is_err() {
        println!("(Browser launch failed; please copy the URL above manually.)");
    }

    let (code, csrf_state) = timeout(Duration::from_secs(300), rx)
        .await
        .context("timed out waiting for OAuth callback")?
        .context("OAuth callback was cancelled")?;

    oauth_state
        .handle_callback(&code, &csrf_state)
        .await
        .context("failed to handle OAuth callback")?;

    let (client_id, credentials_opt) = oauth_state
        .get_credentials()
        .await
        .context("failed to retrieve OAuth credentials")?;
    let credentials =
        credentials_opt.ok_or_else(|| anyhow!("OAuth provider did not return credentials"))?;

    let expires_at = compute_expires_at_millis(&credentials);
    let stored = StoredOAuthTokens {
        server_name: server_name.to_string(),
        url: server_url.to_string(),
        client_id,
        token_response: WrappedOAuthTokenResponse(credentials),
        expires_at,
    };
    save_oauth_tokens(server_name, &stored, store_mode)?;

    drop(guard);
    Ok(())
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
