//! Shared localhost OAuth callback server machinery.
//!
//! This module owns the reusable bind/listen/response loop used by OAuth-style browser flows.

use std::future::Future;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::net::SocketAddr;
use std::net::TcpStream;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use base64::Engine;
use rand::RngCore;
use tiny_http::Header;
use tiny_http::Request;
use tiny_http::Response;
use tiny_http::Server;
use tiny_http::StatusCode;

use crate::pkce::PkceCodes;
use crate::pkce::generate_pkce;

/// Strategy for handling a callback port that is already in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PortConflictStrategy {
    /// Attempt to cancel a previous callback server on the same port and retry.
    CancelPrevious,
    /// Return an error immediately without sending any request to the occupied port.
    Fail,
}

/// Handle used to signal the callback server loop to exit.
#[derive(Clone, Debug)]
pub struct ShutdownHandle {
    shutdown_notify: Arc<tokio::sync::Notify>,
}

impl ShutdownHandle {
    /// Signals the server loop to terminate.
    pub fn shutdown(&self) {
        self.shutdown_notify.notify_waiters();
    }
}

/// Handle for a running authorization-code callback server.
pub(crate) struct AuthorizationCodeServer {
    pub auth_url: String,
    pub actual_port: u16,
    pub redirect_uri: String,
    code_verifier: String,
    server_handle: tokio::task::JoinHandle<io::Result<String>>,
    pub(crate) shutdown_handle: ShutdownHandle,
}

impl AuthorizationCodeServer {
    pub fn open_browser(&self) -> bool {
        webbrowser::open(&self.auth_url).is_ok()
    }

    pub fn code_verifier(&self) -> &str {
        &self.code_verifier
    }

    pub async fn wait_for_code(self, timeout: Duration) -> io::Result<String> {
        let AuthorizationCodeServer {
            server_handle,
            shutdown_handle,
            ..
        } = self;
        let server_handle = server_handle;
        tokio::pin!(server_handle);

        tokio::select! {
            result = &mut server_handle => {
                result
                    .map_err(|err| io::Error::other(format!("authorization-code server thread panicked: {err:?}")))?
            }
            _ = tokio::time::sleep(timeout) => {
                shutdown_handle.shutdown();
                let _ = server_handle.await;
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "OAuth flow timed out waiting for the browser callback.",
                ))
            }
        }
    }
}

pub(crate) fn start_authorization_code_server<F>(
    port: u16,
    port_conflict_strategy: PortConflictStrategy,
    callback_path: &str,
    force_state: Option<String>,
    auth_url_builder: F,
) -> io::Result<AuthorizationCodeServer>
where
    F: FnOnce(&str, &PkceCodes, &str) -> io::Result<String>,
{
    let pkce = generate_pkce();
    let state = force_state.unwrap_or_else(generate_state);
    let callback_path = callback_path.to_string();

    let (server, actual_port, rx) = bind_server_with_request_channel(port, port_conflict_strategy)?;
    let redirect_uri = format!("http://localhost:{actual_port}{callback_path}");
    let auth_url = match auth_url_builder(&redirect_uri, &pkce, &state) {
        Ok(auth_url) => auth_url,
        Err(err) => {
            server.unblock();
            return Err(err);
        }
    };
    let (server_handle, shutdown_handle) = spawn_callback_server_loop(
        server,
        rx,
        "Authentication was not completed",
        move |url_raw| {
            let callback_path = callback_path.clone();
            let state = state.clone();
            async move { process_authorization_code_request(&url_raw, &callback_path, &state) }
        },
    );

    Ok(AuthorizationCodeServer {
        auth_url,
        actual_port,
        redirect_uri,
        code_verifier: pkce.code_verifier,
        server_handle,
        shutdown_handle,
    })
}

/// Internal callback handling outcome.
pub(crate) enum HandledRequest<T> {
    Response(Response<Cursor<Vec<u8>>>),
    RedirectWithHeader(Header),
    ResponseAndExit {
        status: StatusCode,
        headers: Vec<Header>,
        body: Vec<u8>,
        result: io::Result<T>,
    },
}

pub(crate) fn bind_server_with_request_channel(
    port: u16,
    port_conflict_strategy: PortConflictStrategy,
) -> io::Result<(Arc<Server>, u16, tokio::sync::mpsc::Receiver<Request>)> {
    let server = bind_server(port, port_conflict_strategy)?;
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

    // Map blocking reads from server.recv() to an async channel.
    let (tx, rx) = tokio::sync::mpsc::channel::<Request>(16);
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

    Ok((server, actual_port, rx))
}

pub(crate) fn spawn_callback_server_loop<T, F, Fut>(
    server: Arc<Server>,
    mut rx: tokio::sync::mpsc::Receiver<Request>,
    incomplete_message: &'static str,
    mut process_request: F,
) -> (tokio::task::JoinHandle<io::Result<T>>, ShutdownHandle)
where
    T: Send + 'static,
    F: FnMut(String) -> Fut + Send + 'static,
    Fut: Future<Output = HandledRequest<T>> + Send + 'static,
{
    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let server_handle = {
        let shutdown_notify = shutdown_notify.clone();
        tokio::spawn(async move {
            let result = loop {
                tokio::select! {
                    _ = shutdown_notify.notified() => {
                        break Err(io::Error::other(incomplete_message));
                    }
                    maybe_req = rx.recv() => {
                        let Some(req) = maybe_req else {
                            break Err(io::Error::other(incomplete_message));
                        };

                        let url_raw = req.url().to_string();
                        let response = process_request(url_raw).await;

                        if let Some(result) = respond_to_request(req, response).await {
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

    (server_handle, ShutdownHandle { shutdown_notify })
}

async fn respond_to_request<T>(req: Request, response: HandledRequest<T>) -> Option<io::Result<T>> {
    match response {
        HandledRequest::Response(response) => {
            let _ = tokio::task::spawn_blocking(move || req.respond(response)).await;
            None
        }
        HandledRequest::RedirectWithHeader(header) => {
            let redirect = Response::empty(302).with_header(header);
            let _ = tokio::task::spawn_blocking(move || req.respond(redirect)).await;
            None
        }
        HandledRequest::ResponseAndExit {
            status,
            headers,
            body,
            result,
        } => {
            let _ = tokio::task::spawn_blocking(move || {
                send_response_with_disconnect(req, status, headers, body)
            })
            .await;
            Some(result)
        }
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

pub(crate) fn generate_state() -> String {
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

fn bind_server(port: u16, port_conflict_strategy: PortConflictStrategy) -> io::Result<Server> {
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

                // If the address is in use, there is probably another instance of the callback
                // server running. Attempt to cancel it and retry.
                if is_addr_in_use {
                    if port_conflict_strategy == PortConflictStrategy::Fail {
                        return Err(io::Error::new(
                            io::ErrorKind::AddrInUse,
                            format!("Port {bind_address} is already in use"),
                        ));
                    }

                    if !cancel_attempted {
                        cancel_attempted = true;
                        if let Err(cancel_err) = send_cancel_request(port) {
                            eprintln!("Failed to cancel previous callback server: {cancel_err}");
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

fn process_authorization_code_request(
    url_raw: &str,
    callback_path: &str,
    expected_state: &str,
) -> HandledRequest<String> {
    let parsed_url = match url::Url::parse(&format!("http://localhost{url_raw}")) {
        Ok(u) => u,
        Err(err) => {
            return HandledRequest::Response(
                Response::from_string(format!("Bad Request: {err}")).with_status_code(400),
            );
        }
    };

    match parsed_url.path() {
        "/cancel" => HandledRequest::ResponseAndExit {
            status: StatusCode(200),
            headers: Vec::new(),
            body: b"Authentication cancelled".to_vec(),
            result: Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Authentication cancelled",
            )),
        },
        path if path == callback_path => {
            let params: std::collections::HashMap<String, String> =
                parsed_url.query_pairs().into_owned().collect();

            if params.get("state").map(String::as_str) != Some(expected_state) {
                return HandledRequest::ResponseAndExit {
                    status: StatusCode(400),
                    headers: html_headers(),
                    body: b"<h1>State mismatch</h1><p>Return to your terminal and try again.</p>"
                        .to_vec(),
                    result: Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "State mismatch in OAuth callback.",
                    )),
                };
            }

            if let Some(error_code) = params.get("error") {
                let message = authorization_code_error_message(
                    error_code,
                    params.get("error_description").map(String::as_str),
                );
                return HandledRequest::ResponseAndExit {
                    status: StatusCode(403),
                    headers: html_headers(),
                    body: b"<h1>Sign-in failed</h1><p>Return to your terminal.</p>".to_vec(),
                    result: Err(io::Error::new(io::ErrorKind::PermissionDenied, message)),
                };
            }

            match params.get("code") {
                Some(code) if !code.is_empty() => HandledRequest::ResponseAndExit {
                    status: StatusCode(200),
                    headers: html_headers(),
                    body:
                        b"<h1>Authentication complete</h1><p>You can return to your terminal.</p>"
                            .to_vec(),
                    result: Ok(code.clone()),
                },
                _ => HandledRequest::ResponseAndExit {
                    status: StatusCode(400),
                    headers: html_headers(),
                    body: b"<h1>Missing authorization code</h1><p>Return to your terminal.</p>"
                        .to_vec(),
                    result: Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Missing authorization code. Sign-in could not be completed.",
                    )),
                },
            }
        }
        _ => HandledRequest::Response(Response::from_string("Not Found").with_status_code(404)),
    }
}

fn html_headers() -> Vec<Header> {
    match Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]) {
        Ok(header) => vec![header],
        Err(_) => Vec::new(),
    }
}

fn authorization_code_error_message(error_code: &str, error_description: Option<&str>) -> String {
    if let Some(description) = error_description
        && !description.trim().is_empty()
    {
        return format!("Sign-in failed: {description}");
    }

    format!("Sign-in failed: {error_code}")
}
