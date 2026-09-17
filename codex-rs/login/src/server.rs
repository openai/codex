//! Local OAuth callback server for CLI login.
//!
//! This module runs the short-lived localhost server used by interactive sign-in.
//!
//! The callback flow has two competing responsibilities:
//!
//! - preserve enough backend and transport detail for developers, sysadmins, and support
//!   engineers to diagnose failed sign-ins
//! - avoid persisting secrets or sensitive URL/query data into normal application logs
//!
//! This module therefore keeps the user-facing error path and the structured-log path separate.
//! Returned `io::Error` values still carry the detail needed by CLI/browser callers, while
//! structured logs only emit explicitly reviewed fields plus redacted URL/error values.
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::net::SocketAddr;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::thread;
use std::time::Duration;

use crate::auth::AuthDotJson;
use crate::auth::AuthKeyringBackendKind;
use crate::auth::save_auth;
use crate::callback_params::LIFE_SCIENCES_OAUTH_STATE_SUFFIX;
use crate::callback_params::LoginCallbackResult;
use crate::callback_params::LoginOnboardingEntrypoint;
use crate::default_client::create_raw_auth_client;
use crate::default_client::originator;
use crate::oauth::AuthorizationCodeGrant;
use crate::oauth::AuthorizationRequest;
use crate::oauth::CallbackError;
use crate::oauth::CallbackParameters;
use crate::oauth::ErrorBodyLimit;
use crate::oauth::OAuthClient;
use crate::oauth::OAuthError;
use crate::oauth::TokenEncoding;
use crate::oauth::TokenEndpoint;
use crate::oauth::build_authorization_url;
use crate::oauth::generate_state;
use crate::oauth::sanitize_url_for_logging;
use crate::outbound_proxy::AuthRouteConfig;
use crate::pkce::PkceCodes;
use crate::pkce::generate_pkce;
use crate::success_page::LoginSuccessPage;
use crate::success_page::LoginSuccessRedirect;
use crate::success_page::compose_success_url;
use crate::success_page::jwt_auth_claims;
use crate::token_data::TokenData;
use crate::token_data::parse_chatgpt_jwt_claims;
use chrono::Utc;
use codex_config::types::AuthCredentialsStoreMode;
use codex_protocol::auth::AuthMode;
use codex_utils_template::Template;
use serde_json::Value as JsonValue;
use tiny_http::Header;
use tiny_http::Request;
use tiny_http::Response;
use tiny_http::Server;
use tiny_http::StatusCode;
use tracing::error;
use tracing::info;
use tracing::warn;

pub(super) const DEFAULT_ISSUER: &str = "https://auth.openai.com";
const DEFAULT_PORT: u16 = 1455;
// Keep in sync with the Codex CLI Hydra redirect URI allow-list.
const FALLBACK_PORT: u16 = 1457;
static LOGIN_ERROR_PAGE_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(include_str!("assets/error.html"))
        .unwrap_or_else(|err| panic!("login error page template must parse: {err}"))
});

/// Options for launching the local login callback server.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub codex_home: PathBuf,
    pub client_id: String,
    pub issuer: String,
    pub port: u16,
    pub open_browser: bool,
    pub force_state: Option<String>,
    pub forced_chatgpt_workspace_id: Option<Vec<String>>,
    pub codex_streamlined_login: bool,
    pub login_success_page: LoginSuccessPage,
    pub cli_auth_credentials_store_mode: AuthCredentialsStoreMode,
    pub auth_keyring_backend_kind: AuthKeyringBackendKind,
    pub auth_route_config: AuthRouteConfig,
}

impl ServerOptions {
    /// Creates a server configuration with the default issuer and port.
    pub fn new(
        codex_home: PathBuf,
        client_id: String,
        forced_chatgpt_workspace_id: Option<Vec<String>>,
        cli_auth_credentials_store_mode: AuthCredentialsStoreMode,
        auth_keyring_backend_kind: AuthKeyringBackendKind,
        auth_route_config: AuthRouteConfig,
    ) -> Self {
        Self {
            codex_home,
            client_id,
            issuer: DEFAULT_ISSUER.to_string(),
            port: DEFAULT_PORT,
            open_browser: true,
            force_state: None,
            forced_chatgpt_workspace_id,
            codex_streamlined_login: false,
            login_success_page: LoginSuccessPage::default(),
            cli_auth_credentials_store_mode,
            auth_keyring_backend_kind,
            auth_route_config,
        }
    }
}

/// Handle for a running login callback server.
pub struct LoginServer {
    pub auth_url: String,
    pub actual_port: u16,
    server_handle: tokio::task::JoinHandle<io::Result<LoginCallbackResult>>,
    shutdown_handle: ShutdownHandle,
}

impl LoginServer {
    /// Waits for the login callback loop to finish.
    pub async fn block_until_done(self) -> io::Result<()> {
        self.block_until_done_with_callback_result()
            .await
            .map(|_| ())
    }

    /// Waits for login to finish and returns allowlisted callback metadata.
    pub async fn block_until_done_with_callback_result(self) -> io::Result<LoginCallbackResult> {
        self.server_handle
            .await
            .map_err(|err| io::Error::other(format!("login server thread panicked: {err:?}")))?
    }

    /// Requests shutdown of the callback server.
    pub fn cancel(&self) {
        self.shutdown_handle.shutdown();
    }

    /// Returns a cloneable cancel handle for the running server.
    pub fn cancel_handle(&self) -> ShutdownHandle {
        self.shutdown_handle.clone()
    }
}

/// Handle used to signal the login server loop to exit.
#[derive(Clone, Debug)]
pub struct ShutdownHandle {
    shutdown_notify: Arc<tokio::sync::Notify>,
}

impl ShutdownHandle {
    /// Signals the login loop to terminate.
    pub fn shutdown(&self) {
        self.shutdown_notify.notify_one();
    }
}

/// Starts a local callback server and returns the browser auth URL.
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
    let auth_url = build_authorize_url(
        &opts.issuer,
        &opts.client_id,
        &redirect_uri,
        &pkce,
        &state,
        opts.forced_chatgpt_workspace_id.as_deref(),
    )?;

    if opts.open_browser {
        let _ = webbrowser::open(&auth_url);
    }

    // Map blocking reads from server.recv() to an async channel.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Request>(16);
    let _server_handle = {
        let server = server.clone();
        thread::spawn(move || -> io::Result<()> {
            while let Ok(request) = server.recv() {
                match tx.blocking_send(request) {
                    Ok(()) => {}
                    Err(error) => {
                        eprintln!("Failed to send request to channel: {error}");
                        return Err(io::Error::other("Failed to send request to channel"));
                    }
                }
            }
            Ok(())
        })
    };

    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let server_handle = {
        let shutdown_notify = shutdown_notify.clone();
        let server = server;
        tokio::spawn(async move {
            let mut callback_result = LoginCallbackResult::default();
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
                            process_request(
                                &url_raw,
                                &opts,
                                &redirect_uri,
                                &pkce,
                                actual_port,
                                &state,
                            )
                            .await;

                        let exit_result = match response {
                            HandledRequest::Response(response) => {
                                let _ = tokio::task::spawn_blocking(move || req.respond(response)).await;
                                None
                            }
                            HandledRequest::RedirectWithHeader { header, result } => {
                                callback_result = result;
                                let redirect = Response::empty(302).with_header(header);
                                let _ = tokio::task::spawn_blocking(move || req.respond(redirect)).await;
                                None
                            }
                            HandledRequest::ResponseAndExit {
                                headers,
                                body,
                                result,
                            } => {
                                let _ = tokio::task::spawn_blocking(move || {
                                    send_response_with_disconnect(
                                        req,
                                        StatusCode(200),
                                        headers,
                                        body,
                                    )
                                })
                                .await;
                                Some(result.map(|()| callback_result))
                            }
                            HandledRequest::RedirectAndExit { header, result } => {
                                match tokio::task::spawn_blocking(move || {
                                    send_response_with_disconnect(
                                        req,
                                        StatusCode(302),
                                        vec![header],
                                        Vec::new(),
                                    )
                                })
                                .await
                                {
                                    Ok(Ok(())) => {}
                                    Ok(Err(err)) => {
                                        warn!("failed to send hosted login redirect: {err}");
                                    }
                                    Err(err) => {
                                        warn!("hosted login redirect task failed: {err}");
                                    }
                                }
                                Some(Ok(result))
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

/// Internal callback handling outcome.
enum HandledRequest {
    Response(Response<Cursor<Vec<u8>>>),
    RedirectWithHeader {
        header: Header,
        result: LoginCallbackResult,
    },
    RedirectAndExit {
        header: Header,
        result: LoginCallbackResult,
    },
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
            let mut params = CallbackParameters::from_url(&parsed_url);
            let mut callback_result = LoginCallbackResult::default();
            // ChatGPT may append onboarding metadata to the otherwise exact callback state.
            if let Some(callback_state) = params.state.as_mut()
                && callback_state.strip_suffix(LIFE_SCIENCES_OAUTH_STATE_SUFFIX) == Some(state)
            {
                callback_state.truncate(state.len());
                callback_result.onboarding_entrypoint =
                    Some(LoginOnboardingEntrypoint::LifeSciences);
            }
            let validation = params.validate(state);
            let has_code = params.code.as_ref().is_some_and(|code| !code.is_empty());
            let has_state = params.state.as_ref().is_some_and(|state| !state.is_empty());
            let has_error = params.error.as_ref().is_some_and(|error| !error.is_empty());
            let state_valid = !matches!(validation, Err(CallbackError::StateMismatch));
            info!(%path, has_code, has_state, has_error, state_valid, "received login callback");
            let code = match validation {
                Ok(code) => code,
                Err(CallbackError::StateMismatch) => {
                    warn!(%path, has_code, has_state, has_error, "login callback state mismatch");
                    return HandledRequest::Response(
                        Response::from_string("State mismatch").with_status_code(400),
                    );
                }
                Err(CallbackError::Provider {
                    code: error_code,
                    description: error_description,
                }) => {
                    let message = oauth_callback_error_message(error_code, error_description);
                    eprintln!("OAuth callback error: {message}");
                    warn!(
                        error_code,
                        has_error_description =
                            error_description.is_some_and(|s| !s.trim().is_empty()),
                        "oauth callback returned error"
                    );
                    return login_error_response(
                        &message,
                        io::ErrorKind::PermissionDenied,
                        Some(error_code),
                        error_description,
                    );
                }
                Err(CallbackError::MissingCode) => {
                    return login_error_response(
                        "Missing authorization code. Sign-in could not be completed.",
                        io::ErrorKind::InvalidData,
                        Some("missing_authorization_code"),
                        /*error_description*/ None,
                    );
                }
            };

            match exchange_code_for_tokens(
                &opts.issuer,
                &opts.client_id,
                redirect_uri,
                pkce,
                code,
                &opts.auth_route_config,
            )
            .await
            {
                Ok(tokens) => {
                    if let Err(message) = ensure_workspace_allowed(
                        opts.forced_chatgpt_workspace_id.as_deref(),
                        &tokens.id_token,
                    ) {
                        eprintln!("Workspace restriction error: {message}");
                        return login_error_response(
                            &message,
                            io::ErrorKind::PermissionDenied,
                            Some("workspace_restriction"),
                            /*error_description*/ None,
                        );
                    }
                    // Obtain API key via token-exchange and persist
                    let api_key = obtain_api_key(
                        &opts.issuer,
                        &opts.client_id,
                        &tokens.id_token,
                        &opts.auth_route_config,
                    )
                    .await
                    .ok();
                    if let Err(err) = persist_tokens_async(
                        &opts.codex_home,
                        api_key.clone(),
                        tokens.id_token.clone(),
                        tokens.access_token.clone(),
                        tokens.refresh_token.clone(),
                        opts.cli_auth_credentials_store_mode,
                        opts.auth_keyring_backend_kind,
                    )
                    .await
                    {
                        eprintln!("Persist error: {err}");
                        return login_error_response(
                            "Sign-in completed but credentials could not be saved locally.",
                            io::ErrorKind::Other,
                            Some("persist_failed"),
                            Some(&err.to_string()),
                        );
                    }

                    let redirect = compose_success_url(
                        actual_port,
                        &opts.issuer,
                        &tokens.id_token,
                        &tokens.access_token,
                        opts.codex_streamlined_login,
                        &opts.login_success_page,
                    );
                    let url = match &redirect {
                        LoginSuccessRedirect::Local(url) | LoginSuccessRedirect::Hosted(url) => url,
                    };
                    match tiny_http::Header::from_bytes(&b"Location"[..], url.as_bytes()) {
                        Ok(header) => match redirect {
                            LoginSuccessRedirect::Local(_) => HandledRequest::RedirectWithHeader {
                                header,
                                result: callback_result,
                            },
                            LoginSuccessRedirect::Hosted(_) => HandledRequest::RedirectAndExit {
                                header,
                                result: callback_result,
                            },
                        },
                        Err(_) => login_error_response(
                            "Sign-in completed but redirecting back to Codex failed.",
                            io::ErrorKind::Other,
                            Some("redirect_failed"),
                            /*error_description*/ None,
                        ),
                    }
                }
                Err(err) => {
                    eprintln!("Token exchange error: {err}");
                    error!("login callback token exchange failed");
                    login_error_response(
                        &format!("Token exchange failed: {err}"),
                        io::ErrorKind::Other,
                        Some("token_exchange_failed"),
                        /*error_description*/ None,
                    )
                }
            }
        }
        "/success" => {
            let use_streamlined_success = parsed_url
                .query_pairs()
                .any(|(key, value)| key == "codex_streamlined_login" && value == "true");
            let body = if use_streamlined_success {
                include_str!("assets/success.html")
            } else {
                include_str!("assets/success_legacy.html")
            };
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
    status: StatusCode,
    mut headers: Vec<Header>,
    body: Vec<u8>,
) -> io::Result<()> {
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
    forced_chatgpt_workspace_ids: Option<&[String]>,
) -> io::Result<String> {
    let originator = originator().value;
    let workspace_ids = forced_chatgpt_workspace_ids.map(|ids| ids.join(","));
    let mut extra_parameters = vec![
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", originator.as_str()),
    ];
    if let Some(workspace_ids) = workspace_ids.as_deref() {
        extra_parameters.push(("allowed_workspace_id", workspace_ids));
    }
    build_authorization_url(AuthorizationRequest {
        endpoint: &format!("{issuer}/oauth/authorize"),
        client_id,
        redirect_uri,
        scope: Some(
            "openid profile email offline_access api.connectors.read api.connectors.invoke",
        ),
        resource: None,
        pkce,
        state,
        extra_parameters: &extra_parameters,
    })
    .map(String::from)
    .map_err(io::Error::other)
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
    let preferred_bind_address = format!("127.0.0.1:{port}");
    let fallback_bind_address = format!("127.0.0.1:{FALLBACK_PORT}");
    let mut bind_address = preferred_bind_address.clone();
    let mut cancel_attempted = false;
    let mut attempts = 0;
    let mut using_fallback_port = false;
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

                // If the address is in use, there may be another instance of the login server
                // running. Attempt to cancel it and retry before falling back.
                if is_addr_in_use {
                    if !cancel_attempted && !using_fallback_port {
                        cancel_attempted = true;
                        if let Err(cancel_err) = send_cancel_request(port) {
                            eprintln!("Failed to cancel previous login server: {cancel_err}");
                        }
                    }

                    thread::sleep(RETRY_DELAY);

                    if attempts >= MAX_ATTEMPTS {
                        if port == DEFAULT_PORT && !using_fallback_port {
                            warn!(
                                %preferred_bind_address,
                                %fallback_bind_address,
                                "default login callback port is unavailable; falling back to the registered fallback port"
                            );
                            bind_address = fallback_bind_address.clone();
                            attempts = 0;
                            using_fallback_port = true;
                            continue;
                        }

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

/// Tokens returned by the OAuth authorization-code exchange.
#[derive(serde::Deserialize)]
pub(crate) struct ExchangedTokens {
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
}

/// Exchanges an authorization code for tokens.
///
/// The returned error remains suitable for user-facing CLI/browser surfaces, so backend-provided
/// non-JSON error text is preserved there. Structured logging stays narrower: it logs reviewed
/// fields from parsed token responses and redacted transport errors, but does not log the final
/// callback-layer `%err` string.
pub(crate) async fn exchange_code_for_tokens(
    issuer: &str,
    client_id: &str,
    redirect_uri: &str,
    pkce: &PkceCodes,
    code: &str,
    auth_route_config: &AuthRouteConfig,
) -> io::Result<ExchangedTokens> {
    // Reuse the route selected for the issuer, rather than resolving the token path again.
    let client = create_raw_auth_client(issuer.trim_end_matches('/'), auth_route_config)?;
    let endpoint = format!("{}/oauth/token", issuer.trim_end_matches('/'));
    let oauth = OAuthClient::new(
        &client,
        TokenEndpoint {
            url: &endpoint,
            client_id,
            encoding: TokenEncoding::Form,
            timeout: None,
            error_body_limit: ErrorBodyLimit::Unlimited,
        },
    );
    info!(
        issuer = %sanitize_url_for_logging(issuer),
        token_endpoint = %sanitize_url_for_logging(&endpoint),
        %redirect_uri,
        "starting oauth token exchange"
    );
    match oauth
        .exchange_code(AuthorizationCodeGrant {
            code,
            redirect_uri,
            pkce,
            resource: None,
        })
        .await
    {
        Ok(tokens) => {
            info!("oauth token exchange succeeded");
            Ok(tokens)
        }
        Err(OAuthError::Rejected(rejection)) => {
            if let Some(error) = rejection.body_read_error {
                return Err(io::Error::other(error));
            }
            warn!(
                status = %rejection.status,
                detail = ?rejection.detail,
                "oauth token exchange returned non-success status"
            );
            Err(io::Error::other(rejection.to_string()))
        }
        Err(OAuthError::Transport(error)) => {
            error!(
                is_timeout = error.is_timeout(),
                is_connect = error.is_connect(),
                is_request = error.is_request(),
                %error,
                "oauth token exchange transport failure"
            );
            Err(io::Error::other(error))
        }
        Err(error @ OAuthError::InvalidResponse) => Err(io::Error::other(error)),
    }
}

/// Persists exchanged credentials using the configured local auth store.
pub(crate) async fn persist_tokens_async(
    codex_home: &Path,
    api_key: Option<String>,
    id_token: String,
    access_token: String,
    refresh_token: String,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
) -> io::Result<()> {
    // Reuse existing synchronous logic but run it off the async runtime.
    let codex_home = codex_home.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut tokens = TokenData {
            id_token: parse_chatgpt_jwt_claims(&id_token).map_err(io::Error::other)?,
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
            auth_mode: Some(AuthMode::Chatgpt),
            openai_api_key: api_key,
            tokens: Some(tokens),
            last_refresh: Some(Utc::now()),
            agent_identity: None,
            personal_access_token: None,
            bedrock_api_key: None,
            bedrock_access_keys: None,
        };
        save_auth(
            &codex_home,
            &auth,
            auth_credentials_store_mode,
            keyring_backend_kind,
        )
    })
    .await
    .map_err(|e| io::Error::other(format!("persist task failed: {e}")))?
}

/// Validates the ID token against an optional workspace restriction.
pub(crate) fn ensure_workspace_allowed(
    expected: Option<&[String]>,
    id_token: &str,
) -> Result<(), String> {
    let Some(expected) = expected else {
        return Ok(());
    };

    let claims = jwt_auth_claims(id_token);
    let Some(actual) = claims.get("chatgpt_account_id").and_then(JsonValue::as_str) else {
        return Err("Login is restricted to a specific workspace, but the token did not include an chatgpt_account_id claim.".to_string());
    };

    ensure_workspace_account_allowed(Some(expected), actual)
}

/// Validates an already known ChatGPT account ID against an optional workspace restriction.
///
/// PAT login calls this directly because `/whoami` supplies the account ID without an ID token.
pub(crate) fn ensure_workspace_account_allowed(
    expected: Option<&[String]>,
    actual: &str,
) -> Result<(), String> {
    let Some(expected) = expected else {
        return Ok(());
    };

    if expected.iter().any(|workspace_id| workspace_id == actual) {
        Ok(())
    } else {
        Err(format!(
            "Login is restricted to workspace id(s) {}.",
            expected.join(", ")
        ))
    }
}

/// Builds a terminal callback response for login failures.
fn login_error_response(
    message: &str,
    kind: io::ErrorKind,
    error_code: Option<&str>,
    error_description: Option<&str>,
) -> HandledRequest {
    let mut headers = Vec::new();
    if let Ok(header) = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]) {
        headers.push(header);
    }
    let body = render_login_error_page(message, error_code, error_description);
    HandledRequest::ResponseAndExit {
        headers,
        body,
        result: Err(io::Error::new(kind, message.to_string())),
    }
}

/// Returns true when the OAuth callback represents a missing Codex entitlement.
fn is_missing_codex_entitlement_error(error_code: &str, error_description: Option<&str>) -> bool {
    error_code == "access_denied"
        && error_description.is_some_and(|description| {
            description
                .to_ascii_lowercase()
                .contains("missing_codex_entitlement")
        })
}

/// Converts OAuth callback errors into a user-facing message.
fn oauth_callback_error_message(error_code: &str, error_description: Option<&str>) -> String {
    if is_missing_codex_entitlement_error(error_code, error_description) {
        return "Codex is not enabled for your workspace. Contact your workspace administrator to request access to Codex.".to_string();
    }

    if let Some(description) = error_description
        && !description.trim().is_empty()
    {
        return format!("Sign-in failed: {description}");
    }

    format!("Sign-in failed: {error_code}")
}

/// Renders the branded error page used by callback failures.
fn render_login_error_page(
    message: &str,
    error_code: Option<&str>,
    error_description: Option<&str>,
) -> Vec<u8> {
    let code = error_code.unwrap_or("unknown_error");
    let (title, display_message, display_description, help_text) =
        if is_missing_codex_entitlement_error(code, error_description) {
            (
                "You do not have access to Codex".to_string(),
                "This account is not currently authorized to use Codex in this workspace."
                    .to_string(),
                "Contact your workspace administrator to request access to Codex.".to_string(),
                "Contact your workspace administrator to get access to Codex, then return to Codex and try again."
                    .to_string(),
            )
        } else {
            (
                "Sign-in could not be completed".to_string(),
                message.to_string(),
                error_description.unwrap_or(message).to_string(),
                "Return to Codex to retry, switch accounts, or contact your workspace admin if access is restricted."
                    .to_string(),
            )
        };
    LOGIN_ERROR_PAGE_TEMPLATE
        .render([
            ("error_title", html_escape(&title)),
            ("error_message", html_escape(&display_message)),
            ("error_code", html_escape(code)),
            ("error_description", html_escape(&display_description)),
            ("error_help", html_escape(&help_text)),
        ])
        .unwrap_or_else(|err| panic!("login error page template must render: {err}"))
        .into_bytes()
}

/// Escapes error strings before inserting them into HTML.
fn html_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Exchanges an authenticated ID token for an API-key style access token.
pub(crate) async fn obtain_api_key(
    issuer: &str,
    client_id: &str,
    id_token: &str,
    auth_route_config: &AuthRouteConfig,
) -> io::Result<String> {
    // Token exchange for an API key access token
    #[derive(serde::Deserialize)]
    struct ExchangeResp {
        access_token: String,
    }
    let token_endpoint = format!("{}/oauth/token", issuer.trim_end_matches('/'));
    let client = create_raw_auth_client(&token_endpoint, auth_route_config)?;
    let resp = client
        .post(token_endpoint)
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
#[cfg(test)]
mod tests {
    use super::html_escape;
    use super::is_missing_codex_entitlement_error;
    use super::render_login_error_page;

    #[test]
    fn render_login_error_page_escapes_dynamic_fields() {
        let body = String::from_utf8(render_login_error_page(
            "<bad>",
            Some("code&value"),
            Some("\"quoted\""),
        ))
        .expect("login error page should be utf-8");

        assert!(body.contains(&html_escape("Sign-in could not be completed")));
        assert!(body.contains("&lt;bad&gt;"));
        assert!(body.contains("code&amp;value"));
        assert!(body.contains("&quot;quoted&quot;"));
    }

    #[test]
    fn render_login_error_page_uses_entitlement_copy() {
        let error_description = Some("missing_codex_entitlement");
        assert!(is_missing_codex_entitlement_error(
            "access_denied",
            error_description
        ));

        let body = String::from_utf8(render_login_error_page(
            "access denied",
            Some("access_denied"),
            error_description,
        ))
        .expect("login error page should be utf-8");

        assert!(body.contains("You do not have access to Codex"));
        assert!(body.contains("Contact your workspace administrator"));
        assert!(!body.contains("missing_codex_entitlement"));
    }
}
