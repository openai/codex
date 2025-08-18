use std::io::Cursor;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::AuthDotJson;
use crate::get_auth_file;
use crate::pkce::PkceCodes;
use crate::pkce::generate_pkce;
use base64::Engine;
use chrono::Utc;
use rand::RngCore;
use tiny_http::Header;
use tiny_http::Request;
use tiny_http::Response;
use tiny_http::Server;

const DEFAULT_ISSUER: &str = "https://auth.openai.com";
const DEFAULT_PORT: u16 = 1455;

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub codex_home: PathBuf,
    pub client_id: String,
    pub issuer: String,
    pub port: u16,
    pub open_browser: bool,
    pub force_state: Option<String>,
    pub login_timeout: Option<Duration>,
}

impl ServerOptions {
    pub fn new(codex_home: PathBuf, client_id: String) -> Self {
        Self {
            codex_home,
            client_id: client_id.to_string(),
            issuer: DEFAULT_ISSUER.to_string(),
            port: DEFAULT_PORT,
            open_browser: true,
            force_state: None,
            login_timeout: None,
        }
    }
}

pub struct LoginServer {
    pub auth_url: String,
    pub actual_port: u16,
    pub server_handle: thread::JoinHandle<io::Result<()>>,
    pub shutdown_flag: Arc<AtomicBool>,
    pub server: Arc<Server>,
}

impl LoginServer {
    pub fn block_until_done(self) -> io::Result<()> {
        self.server_handle
            .join()
            .map_err(|err| io::Error::other(format!("login server thread panicked: {err:?}")))?
    }

    pub fn cancel(&self) {
        shutdown(&self.shutdown_flag, &self.server);
    }

    pub fn cancel_handle(&self) -> ShutdownHandle {
        ShutdownHandle {
            shutdown_flag: self.shutdown_flag.clone(),
            server: self.server.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ShutdownHandle {
    shutdown_flag: Arc<AtomicBool>,
    server: Arc<Server>,
}

impl ShutdownHandle {
    pub fn cancel(&self) {
        shutdown(&self.shutdown_flag, &self.server);
    }
}

pub fn shutdown(shutdown_flag: &AtomicBool, server: &Server) {
    shutdown_flag.store(true, Ordering::SeqCst);
    server.unblock();
}

pub fn run_login_server(
    opts: ServerOptions,
    shutdown_flag: Option<Arc<AtomicBool>>,
) -> io::Result<LoginServer> {
    let pkce = generate_pkce();
    let state = opts.force_state.clone().unwrap_or_else(generate_state);

    let server = Server::http(format!("127.0.0.1:{}", opts.port)).map_err(io::Error::other)?;
    let actual_port = match server.server_addr().to_ip() {
        Some(addr) => addr.port(),
        None => {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "Unable to determine the server port",
            ));
        }
    };
    let server = Arc::new(server);

    let redirect_uri = format!("http://localhost:{actual_port}/auth/callback");
    let auth_url = build_authorize_url(&opts.issuer, &opts.client_id, &redirect_uri, &pkce, &state);

    if opts.open_browser {
        let _ = webbrowser::open(&auth_url);
    }
    let shutdown_flag = shutdown_flag.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    let shutdown_flag_clone = shutdown_flag.clone();
    let timeout_flag = Arc::new(AtomicBool::new(false));

    // Channel used to signal completion to timeout watcher.
    let (done_tx, done_rx) = mpsc::channel::<()>();

    if let Some(timeout) = opts.login_timeout {
        spawn_timeout_watcher(
            done_rx,
            timeout,
            shutdown_flag.clone(),
            timeout_flag.clone(),
            server.clone(),
        );
    }

    let server_for_thread = server.clone();
    let server_handle = thread::spawn(move || {
        while !shutdown_flag.load(Ordering::SeqCst) {
            let req = match server_for_thread.recv() {
                Ok(r) => r,
                Err(e) => {
                    // If we've been asked to shut down, break gracefully so that
                    // we can report timeout or cancellation status uniformly.
                    if shutdown_flag.load(Ordering::SeqCst) {
                        break;
                    } else {
                        return Err(io::Error::other(e));
                    }
                }
            };

            let response = process_request(&req, &opts, &redirect_uri, &pkce, actual_port, &state);
            let is_login_complete = matches!(response, HandledRequest::ResponseAndExit(_));
            match response {
                HandledRequest::Response(r) | HandledRequest::ResponseAndExit(r) => {
                    let _ = req.respond(r);
                }
                HandledRequest::RedirectWithHeader(header) => {
                    let redirect = Response::empty(302).with_header(header);
                    let _ = req.respond(redirect);
                }
            }

            if is_login_complete {
                shutdown_flag.store(true, Ordering::SeqCst);
                // Login has succeeded, so disarm the timeout watcher.
                let _ = done_tx.send(());
                return Ok(());
            }
        }

        // Login has failed or timed out, so disarm the timeout watcher.
        let _ = done_tx.send(());

        if timeout_flag.load(Ordering::SeqCst) {
            Err(io::Error::other("Login timed out"))
        } else {
            Err(io::Error::other("Login was not completed"))
        }
    });

    Ok(LoginServer {
        auth_url: auth_url.clone(),
        actual_port,
        server_handle,
        shutdown_flag: shutdown_flag_clone,
        server,
    })
}

enum HandledRequest {
    Response(Response<Cursor<Vec<u8>>>),
    RedirectWithHeader(Header),
    ResponseAndExit(Response<Cursor<Vec<u8>>>),
}

fn process_request(
    req: &Request,
    opts: &ServerOptions,
    redirect_uri: &str,
    pkce: &PkceCodes,
    actual_port: u16,
    state: &str,
) -> HandledRequest {
    let url_raw = req.url().to_string();
    let parsed_url = match url::Url::parse(&format!("http://localhost{url_raw}")) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("URL parse error: {e}");
            return HandledRequest::Response(
                Response::from_string("Bad Request").with_status_code(400),
            );
        }
    };
    let path = parsed_url.path().to_string();

    match path.as_str() {
        "/auth/callback" => {
            let params: std::collections::HashMap<String, String> =
                parsed_url.query_pairs().into_owned().collect();
            if params.get("state").map(String::as_str) != Some(state) {
                return HandledRequest::Response(
                    Response::from_string("State mismatch").with_status_code(400),
                );
            }
            let code = match params.get("code") {
                Some(c) if !c.is_empty() => c.clone(),
                _ => {
                    return HandledRequest::Response(
                        Response::from_string("Missing authorization code").with_status_code(400),
                    );
                }
            };

            match exchange_code_for_tokens(&opts.issuer, &opts.client_id, redirect_uri, pkce, &code)
            {
                Ok(tokens) => {
                    // Obtain API key via token-exchange and persist
                    let api_key =
                        obtain_api_key(&opts.issuer, &opts.client_id, &tokens.id_token).ok();
                    if let Err(err) = persist_tokens(
                        &opts.codex_home,
                        api_key.clone(),
                        tokens.id_token.clone(),
                        Some(tokens.access_token.clone()),
                        Some(tokens.refresh_token.clone()),
                    ) {
                        eprintln!("Persist error: {err}");
                        return HandledRequest::Response(
                            Response::from_string(format!("Unable to persist auth file: {err}"))
                                .with_status_code(500),
                        );
                    }

                    let success_url = compose_success_url(
                        actual_port,
                        &opts.issuer,
                        &tokens.id_token,
                        &tokens.access_token,
                    );
                    match tiny_http::Header::from_bytes(&b"Location"[..], success_url.as_bytes()) {
                        Ok(header) => HandledRequest::RedirectWithHeader(header),
                        Err(_) => HandledRequest::Response(
                            Response::from_string("Internal Server Error").with_status_code(500),
                        ),
                    }
                }
                Err(err) => {
                    eprintln!("Token exchange error: {err}");
                    HandledRequest::Response(
                        Response::from_string(format!("Token exchange failed: {err}"))
                            .with_status_code(500),
                    )
                }
            }
        }
        "/success" => {
            let body = include_str!("assets/success.html");
            let mut resp = Response::from_data(body.as_bytes());
            if let Ok(h) = tiny_http::Header::from_bytes(
                &b"Content-Type"[..],
                &b"text/html; charset=utf-8"[..],
            ) {
                resp.add_header(h);
            }
            HandledRequest::ResponseAndExit(resp)
        }
        _ => HandledRequest::Response(Response::from_string("Not Found").with_status_code(404)),
    }
}

/// Spawns a detached thread that waits for either a completion signal on `done_rx`
/// or the specified `timeout` to elapse. If the timeout elapses first it marks
/// the `shutdown_flag`, records `timeout_flag`, and unblocks the HTTP server so
/// that the main server loop can exit promptly.
fn spawn_timeout_watcher(
    done_rx: mpsc::Receiver<()>,
    timeout: Duration,
    shutdown_flag: Arc<AtomicBool>,
    timeout_flag: Arc<AtomicBool>,
    server: Arc<Server>,
) {
    thread::spawn(move || {
        if done_rx.recv_timeout(timeout).is_err()
            && shutdown_flag
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            timeout_flag.store(true, Ordering::SeqCst);
            server.unblock();
        }
    });
}

fn build_authorize_url(
    issuer: &str,
    client_id: &str,
    redirect_uri: &str,
    pkce: &PkceCodes,
    state: &str,
) -> String {
    let query = vec![
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("scope", "openid profile email offline_access"),
        ("code_challenge", &pkce.code_challenge),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("state", state),
    ];
    let qs = query
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{issuer}/oauth/authorize?{qs}")
}

fn generate_state() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

struct ExchangedTokens {
    id_token: String,
    access_token: String,
    refresh_token: String,
}

fn exchange_code_for_tokens(
    issuer: &str,
    client_id: &str,
    redirect_uri: &str,
    pkce: &PkceCodes,
    code: &str,
) -> io::Result<ExchangedTokens> {
    #[derive(serde::Deserialize)]
    struct TokenResponse {
        id_token: String,
        access_token: String,
        refresh_token: String,
    }

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{issuer}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
            urlencoding::encode(code),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(client_id),
            urlencoding::encode(&pkce.code_verifier)
        ))
        .send()
        .map_err(io::Error::other)?;

    if !resp.status().is_success() {
        return Err(io::Error::other(format!(
            "token endpoint returned status {}",
            resp.status()
        )));
    }

    let tokens: TokenResponse = resp.json().map_err(io::Error::other)?;
    Ok(ExchangedTokens {
        id_token: tokens.id_token,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    })
}

fn persist_tokens(
    codex_home: &Path,
    api_key: Option<String>,
    id_token: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
) -> io::Result<()> {
    let auth_file = get_auth_file(codex_home);
    if let Some(parent) = auth_file.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(io::Error::other)?;
        }
    }

    let mut auth = read_or_default(&auth_file);
    if let Some(key) = api_key {
        auth.openai_api_key = Some(key);
    }
    let tokens = auth
        .tokens
        .get_or_insert_with(crate::token_data::TokenData::default);
    tokens.id_token = crate::token_data::parse_id_token(&id_token).map_err(io::Error::other)?;
    // Persist chatgpt_account_id if present in claims
    if let Some(acc) = jwt_auth_claims(&id_token)
        .get("chatgpt_account_id")
        .and_then(|v| v.as_str())
    {
        tokens.account_id = Some(acc.to_string());
    }
    if let Some(at) = access_token {
        tokens.access_token = at;
    }
    if let Some(rt) = refresh_token {
        tokens.refresh_token = rt;
    }
    auth.last_refresh = Some(Utc::now());
    super::write_auth_json(&auth_file, &auth)
}

fn read_or_default(path: &Path) -> AuthDotJson {
    match super::try_read_auth_json(path) {
        Ok(auth) => auth,
        Err(_) => AuthDotJson {
            openai_api_key: None,
            tokens: None,
            last_refresh: None,
        },
    }
}

fn compose_success_url(port: u16, issuer: &str, id_token: &str, access_token: &str) -> String {
    let token_claims = jwt_auth_claims(id_token);
    let access_claims = jwt_auth_claims(access_token);

    let org_id = token_claims
        .get("organization_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let project_id = token_claims
        .get("project_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let completed_onboarding = token_claims
        .get("completed_platform_onboarding")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let is_org_owner = token_claims
        .get("is_org_owner")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let needs_setup = (!completed_onboarding) && is_org_owner;
    let plan_type = access_claims
        .get("chatgpt_plan_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let platform_url = if issuer == DEFAULT_ISSUER {
        "https://platform.openai.com"
    } else {
        "https://platform.api.openai.org"
    };

    let mut params = vec![
        ("id_token", id_token.to_string()),
        ("needs_setup", needs_setup.to_string()),
        ("org_id", org_id.to_string()),
        ("project_id", project_id.to_string()),
        ("plan_type", plan_type.to_string()),
        ("platform_url", platform_url.to_string()),
    ];
    let qs = params
        .drain(..)
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(&v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("http://localhost:{port}/success?{qs}")
}

fn jwt_auth_claims(jwt: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut parts = jwt.split('.');
    let (_h, payload_b64, _s) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => {
            eprintln!("Invalid JWT format while extracting claims");
            return serde_json::Map::new();
        }
    };
    match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_b64) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(mut v) => {
                if let Some(obj) = v
                    .get_mut("https://api.openai.com/auth")
                    .and_then(|x| x.as_object_mut())
                {
                    return obj.clone();
                }
                eprintln!("JWT payload missing expected 'https://api.openai.com/auth' object");
            }
            Err(e) => {
                eprintln!("Failed to parse JWT JSON payload: {e}");
            }
        },
        Err(e) => {
            eprintln!("Failed to base64url-decode JWT payload: {e}");
        }
    }
    serde_json::Map::new()
}

fn obtain_api_key(issuer: &str, client_id: &str, id_token: &str) -> io::Result<String> {
    // Token exchange for an API key access token
    #[derive(serde::Deserialize)]
    struct ExchangeResp {
        access_token: String,
    }
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{issuer}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type={}&client_id={}&requested_token={}&subject_token={}&subject_token_type={}",
            urlencoding::encode("urn:ietf:params:oauth:grant-type:token-exchange"),
            urlencoding::encode(client_id),
            urlencoding::encode("openai-api-key"),
            urlencoding::encode(id_token),
            urlencoding::encode("urn:ietf:params:oauth:token-type:id_token")
        ))
        .send()
        .map_err(io::Error::other)?;
    if !resp.status().is_success() {
        return Err(io::Error::other(format!(
            "api key exchange failed with status {}",
            resp.status()
        )));
    }
    let body: ExchangeResp = resp.json().map_err(io::Error::other)?;
    Ok(body.access_token)
}
