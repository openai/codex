//! Owns the loopback listener lifecycle; OAuth callback parsing and state validation are shared.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use crate::oauth::CallbackError;
use crate::oauth::CallbackParameters;
use tiny_http::Response;
use tiny_http::Server;
use tokio::sync::oneshot;
use tokio::time::timeout;
use url::Url;

const BROWSER_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 180);

pub(super) struct CallbackListener {
    server: Arc<Server>,
    receiver: oneshot::Receiver<io::Result<String>>,
    redirect_uri: String,
}

impl CallbackListener {
    pub(super) fn new(redirect_port: Option<u16>, expected_state: String) -> io::Result<Self> {
        let callback_address = format!("127.0.0.1:{}", redirect_port.unwrap_or_default());
        let server =
            Arc::new(Server::http(callback_address).map_err(|_| {
                io::Error::other("failed to bind provider OAuth loopback callback")
            })?);
        let redirect_uri = match server.server_addr() {
            tiny_http::ListenAddr::IP(address) => format!("http://{address}/callback"),
            #[cfg(not(target_os = "windows"))]
            _ => return Err(io::Error::other("invalid provider OAuth loopback address")),
        };

        let (sender, receiver) = oneshot::channel();
        let callback_server = Arc::clone(&server);
        tokio::task::spawn_blocking(move || {
            while let Ok(request) = callback_server.recv() {
                let Ok(callback) = Url::parse(&format!("http://127.0.0.1{}", request.url())) else {
                    let _ = request.respond(
                        Response::from_string("Invalid OAuth callback")
                            .with_status_code(/*code*/ 400),
                    );
                    continue;
                };
                if callback.path() != "/callback" {
                    let _ = request
                        .respond(Response::from_string("Not found").with_status_code(/*code*/ 404));
                    continue;
                }

                let params = CallbackParameters::from_url(&callback);
                let result = match params.validate(&expected_state) {
                    Ok(code) => Ok(code.to_string()),
                    Err(CallbackError::StateMismatch) => {
                        let _ = request.respond(
                            Response::from_string("OAuth callback state did not match")
                                .with_status_code(/*code*/ 400),
                        );
                        continue;
                    }
                    Err(CallbackError::Provider { .. }) => Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "provider OAuth authorization was denied",
                    )),
                    Err(CallbackError::MissingCode) => {
                        Err(io::Error::other("provider OAuth callback omitted its code"))
                    }
                };
                let mut response = if result.is_ok() {
                    Response::from_string(
                        "<!doctype html><html><body><p>Sign-in complete. You may close this window.</p><script>window.close()</script></body></html>",
                    )
                } else {
                    Response::from_string("Sign-in failed.").with_status_code(/*code*/ 400)
                };
                if result.is_ok()
                    && let Ok(header) = tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        &b"text/html; charset=utf-8"[..],
                    )
                {
                    response.add_header(header);
                }
                let _ = request.respond(response);
                let _ = sender.send(result);
                break;
            }
        });

        Ok(Self {
            server,
            receiver,
            redirect_uri,
        })
    }

    pub(super) fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub(super) async fn wait(&mut self) -> io::Result<String> {
        timeout(BROWSER_TIMEOUT, &mut self.receiver)
            .await
            .map_err(|_| io::Error::other("timed out waiting for provider OAuth sign-in"))?
            .map_err(|_| io::Error::other("provider OAuth sign-in was cancelled"))?
    }
}

impl Drop for CallbackListener {
    fn drop(&mut self) {
        self.server.unblock();
    }
}
