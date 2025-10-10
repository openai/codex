use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::net::SocketAddr;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::pkce::PkceCodes;
use crate::pkce::generate_pkce;
use base64::Engine;
use chrono::Utc;
use codex_core::auth::AuthDotJson;
use codex_core::auth::get_auth_file;
use codex_core::default_client::originator;
use codex_core::token_data::TokenData;
use codex_core::token_data::parse_id_token;
use rand::RngCore;
use serde_json::Value as JsonValue;
use tiny_http::Header;
use tiny_http::Request;
use tiny_http::Response;
use tiny_http::Server;
use tiny_http::StatusCode;

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
}

impl ServerOptions {
    pub fn new(codex_home: PathBuf, client_id: String) -> Self {
        Self {
            codex_home,
            client_id,
            issuer: DEFAULT_ISSUER.to_string(),
            port: DEFAULT_PORT,
            open_browser: true,
            force_state: None,
        }
    }
}

pub struct LoginServer {
    pub auth_url: String,
    pub actual_port: u16,
    server_handle: tokio::task::JoinHandle<io::Result<()>>,
    shutdown_handle: ShutdownHandle,
}

impl LoginServer {
    pub async fn block_until_done(self) -> io::Result<()> {
        self.server_handle
            .await
            .map_err(|err| io::Error::other(format!("login server thread panicked: {err:?}")))?
    }

    pub fn cancel(&self) {
        self.shutdown_handle.shutdown();
    }

    pub fn cancel_handle(&self) -> ShutdownHandle {
        self.shutdown_handle.clone()
    }
}

#[derive(Clone, Debug)]
pub struct ShutdownHandle {
    shutdown_notify: Arc<tokio::sync::Notify>,
}

impl ShutdownHandle {
    pub fn shutdown(&self) {
        self.shutdown_notify.notify_waiters();
    }
}

pub fn run_login_server(opts: ServerOptions) -> io::Result<LoginServer> {
    let pkce = generate_pkce();
    let state = opts.force_state.clone().unwrap_or_else(generate_state);

    let server = bind_server(opts.port)?;
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

    // Map blocking reads from server.recv() to an async channel.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Request>(16);
    let _server_handle = {
        let server = server.clone();
        thread::spawn(move || -> io::Result<()> {
            while let Ok(request) = server.recv() {
                tx.blocking_send(request).map_err(|e| {
                    eprintln!("Failed to send request to channel: {e}");
                    io::Error::other("Failed to send request to channel")
                })?;
            }
            Ok(())
        })
    };

    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let server_handle = {
        let shutdown_notify = shutdown_notify.clone();
        let server = server;
        tokio::spawn(async move {
            let result = loop {
                tokio::select! {
                    _ = shutdown_notify.notified() => {
                        break Err(io::Error::other("Login was not completed"));
                    }
                    maybe_req = rx.recv() => {
                        let Some(req) = maybe_req else {
                            break Err(io::Error::other("Login was not completed"));
                        };

                        let url_raw = req.url().to_string();
                        let response =
                            process_request(&url_raw, &opts, &redirect_uri, &pkce, actual_port, &state).await;

                        let exit_result = match response {
                            HandledRequest::Response(response) => {
                                let _ = tokio::task::spawn_blocking(move || req.respond(response)).await;
                                None
                            }
                            HandledRequest::ResponseAndExit {
                                headers,
                                body,
                                result,
                            } => {
                                let _ = tokio::task::spawn_blocking(move || {
                                    send_response_with_disconnect(req, headers, body)
                                })
                                .await;
                                Some(result)
                            }
                            HandledRequest::RedirectWithHeader(header) => {
                                let redirect = Response::empty(302).with_header(header);
                                let _ = tokio::task::spawn_blocking(move || req.respond(redirect)).await;
                                None
                            }
                        };

                        if let Some(result) = exit_result {
                            break result;
                        }
                    }
                }
            };

            // Ensure that the server is unblocked so the thread dedicated to
            // running `server.recv()` in a loop exits cleanly.
            server.unblock();
            result
        })
    };

    Ok(LoginServer {
        auth_url,
        actual_port,
        server_handle,
        shutdown_handle: ShutdownHandle { shutdown_notify },
    })
}

enum HandledRequest {
    Response(Response<Cursor<Vec<u8>>>),
    RedirectWithHeader(Header),
    ResponseAndExit {
        headers: Vec<Header>,
        body: Vec<u8>,
        result: io::Result<()>,
    },
}

async fn process_request(
    url_raw: &str,
    opts: &ServerOptions,
    redirect_uri: &str,
    pkce: &PkceCodes,
    actual_port: u16,
    state: &str,
) -> HandledRequest {
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
                .await
            {
                Ok(tokens) => {
                    // Obtain API key via token-exchange and persist
                    let api_key = obtain_api_key(&opts.issuer, &opts.client_id, &tokens.id_token)
                        .await
                        .ok();
                    if let Err(err) = persist_tokens_async(
                        &opts.codex_home,
                        api_key.clone(),
                        tokens.id_token.clone(),
                        tokens.access_token.clone(),
                        tokens.refresh_token.clone(),
                    )
                    .await
                    {
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
            HandledRequest::ResponseAndExit {
                headers: match Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"text/html; charset=utf-8"[..],
                ) {
                    Ok(header) => vec![header],
                    Err(_) => Vec::new(),
                },
                body: body.as_bytes().to_vec(),
                result: Ok(()),
            }
        }
        "/cancel" => HandledRequest::ResponseAndExit {
            headers: Vec::new(),
            body: b"Login cancelled".to_vec(),
            result: Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Login cancelled",
            )),
        },
        _ => HandledRequest::Response(Response::from_string("Not Found").with_status_code(404)),
    }
}

/// tiny_http filters `Connection` headers out of `Response` objects, so using
/// `req.respond` never informs the client (or the library) that a keep-alive
/// socket should be closed. That leaves the per-connection worker parked in a
/// loop waiting for more requests, which in turn causes the next login attempt
/// to hang on the old connection. This helper bypasses tiny_http’s response
/// machinery: it extracts the raw writer, prints the HTTP response manually,
/// and always appends `Connection: close`, ensuring the socket is closed from
/// the server side. Ideally, tiny_http would provide an API to control
/// server-side connection persistence, but it does not.
fn send_response_with_disconnect(
    req: Request,
    mut headers: Vec<Header>,
    body: Vec<u8>,
) -> io::Result<()> {
    let status = StatusCode(200);
    let mut writer = req.into_writer();
    let reason = status.default_reason_phrase();
    write!(writer, "HTTP/1.1 {} {}\r\n", status.0, reason)?;
    headers.retain(|h| !h.field.equiv("Connection"));
    if let Ok(close_header) = Header::from_bytes(&b"Connection"[..], &b"close"[..]) {
        headers.push(close_header);
    }

    let content_length_value = format!("{}", body.len());
    if let Ok(content_length_header) =
        Header::from_bytes(&b"Content-Length"[..], content_length_value.as_bytes())
    {
        headers.push(content_length_header);
    }

    for header in headers {
        write!(
            writer,
            "{}: {}\r\n",
            header.field.as_str(),
            header.value.as_str()
        )?;
    }

    writer.write_all(b"\r\n")?;
    writer.write_all(&body)?;
    writer.flush()
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
        ("originator", originator().value.as_str()),
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
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn send_cancel_request(port: u16) -> io::Result<()> {
    let addr: SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    stream.write_all(b"GET /cancel HTTP/1.1\r\n")?;
    stream.write_all(format!("Host: 127.0.0.1:{port}\r\n").as_bytes())?;
    stream.write_all(b"Connection: close\r\n\r\n")?;

    let mut buf = [0u8; 64];
    let _ = stream.read(&mut buf);
    Ok(())
}

fn bind_server(port: u16) -> io::Result<Server> {
    let bind_address = format!("127.0.0.1:{port}");
    let mut cancel_attempted = false;
    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 10;
    const RETRY_DELAY: Duration = Duration::from_millis(200);

    loop {
        match Server::http(&bind_address) {
            Ok(server) => return Ok(server),
            Err(err) => {
                attempts += 1;
                let is_addr_in_use = err
                    .downcast_ref::<io::Error>()
                    .map(|io_err| io_err.kind() == io::ErrorKind::AddrInUse)
                    .unwrap_or(false);

                // If the address is in use, there is probably another instance of the login server
                // running. Attempt to cancel it and retry.
                if is_addr_in_use {
                    if !cancel_attempted {
                        cancel_attempted = true;
                        if let Err(cancel_err) = send_cancel_request(port) {
                            eprintln!("Failed to cancel previous login server: {cancel_err}");
                        }
                    }

                    thread::sleep(RETRY_DELAY);

                    if attempts >= MAX_ATTEMPTS {
                        return Err(io::Error::new(
                            io::ErrorKind::AddrInUse,
                            format!("Port {bind_address} is already in use"),
                        ));
                    }

                    continue;
                }

                return Err(io::Error::other(err));
            }
        }
    }
}

pub(crate) struct ExchangedTokens {
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
}

pub(crate) async fn exchange_code_for_tokens(
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

    let client = reqwest::Client::new();
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
        .await
        .map_err(io::Error::other)?;

    if !resp.status().is_success() {
        return Err(io::Error::other(format!(
            "token endpoint returned status {}",
            resp.status()
        )));
    }

    let tokens: TokenResponse = resp.json().await.map_err(io::Error::other)?;
    Ok(ExchangedTokens {
        id_token: tokens.id_token,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    })
}

pub(crate) async fn persist_tokens_async(
    codex_home: &Path,
    api_key: Option<String>,
    id_token: String,
    access_token: String,
    refresh_token: String,
) -> io::Result<()> {
    // Reuse existing synchronous logic but run it off the async runtime.
    let codex_home = codex_home.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let auth_file = get_auth_file(&codex_home);
        if let Some(parent) = auth_file.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(io::Error::other)?;
        }

        let mut tokens = TokenData {
            id_token: parse_id_token(&id_token).map_err(io::Error::other)?,
            access_token,
            refresh_token,
            account_id: None,
        };
        if let Some(acc) = jwt_auth_claims(&id_token)
            .get("chatgpt_account_id")
            .and_then(|v| v.as_str())
        {
            tokens.account_id = Some(acc.to_string());
        }
        let auth = AuthDotJson {
            openai_api_key: api_key,
            tokens: Some(tokens),
            last_refresh: Some(Utc::now()),
        };
        codex_core::auth::write_auth_json(&auth_file, &auth)
    })
    .await
    .map_err(|e| io::Error::other(format!("persist task failed: {e}")))?
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
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let is_org_owner = token_claims
        .get("is_org_owner")
        .and_then(JsonValue::as_bool)
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

pub(crate) async fn obtain_api_key(
    issuer: &str,
    client_id: &str,
    id_token: &str,
) -> io::Result<String> {
    // Token exchange for an API key access token
    #[derive(serde::Deserialize)]
    struct ExchangeResp {
        access_token: String,
    }
    let client = reqwest::Client::new();
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
        .await
        .map_err(io::Error::other)?;
    if !resp.status().is_success() {
        return Err(io::Error::other(format!(
            "api key exchange failed with status {}",
            resp.status()
        )));
    }
    let body: ExchangeResp = resp.json().await.map_err(io::Error::other)?;
    Ok(body.access_token)
}
