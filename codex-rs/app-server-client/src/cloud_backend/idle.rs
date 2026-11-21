//! Restores Cloud reply streams under the original login and network permit.
//! An active recovery holds its stream until the caller has read history.

use super::CloudRecoverySignInChanged;
use super::ReplyStream;
use super::check_reply_stream;
use crate::CLOUD_RECOVERY_NETWORK_BLOCKED;
use crate::RequestResult;
use crate::cloud::IdleReconnect;
use crate::cloud::MAX_RECOVERY_BACKOFF;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_cloud_client as cloud;
use codex_http_client::NetworkPermit;
use codex_login::AuthConfig;
use codex_login::token_data::parse_jwt_expiration;
use codex_protocol::auth::AuthMode;
use futures::FutureExt;
use futures::future::BoxFuture;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::time::Instant;

pub(super) const SIGN_IN_AGAIN: &str =
    "Your cloud sign-in changed or expired. Sign in again to access this conversation.";

pub(super) fn blocked_recovery(error: &cloud::Error) -> Option<&'static str> {
    let cloud::Error::Rpc(status) = error else {
        return None;
    };
    match status.code {
        5 => Some("Cloud could not find this conversation."),
        7 => Some("Your account no longer has access to this cloud conversation."),
        16 => Some(SIGN_IN_AGAIN),
        _ => None,
    }
}

const RECOVERY_FAILED: &str = "Cloud could not restore the reply stream for this conversation.";

#[derive(Clone)]
pub(super) struct Login {
    pub(super) storage: AuthConfig,
    pub(super) mode: AuthMode,
    pub(super) token: String,
    pub(super) account: String,
    pub(super) permit: NetworkPermit,
}

impl Login {
    pub(super) fn supports_recovery(&self) -> bool {
        matches!(self.mode, AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens)
    }

    pub(super) async fn current(&self) -> bool {
        if !self.supports_recovery() {
            return false;
        }
        let Ok(Some(auth)) = self
            .storage
            .load_auth_with_modes(/*enable_codex_api_key_env*/ false, Some(&[self.mode]))
            .await
        else {
            return false;
        };
        let Ok(token) = auth.get_token() else {
            return false;
        };
        auth.is_chatgpt_auth()
            && !auth.is_fedramp_account()
            && auth.get_account_id().as_deref() == Some(self.account.as_str())
            && token == self.token
            && parse_jwt_expiration(&token).is_ok_and(|expiry| {
                expiry.is_none_or(|expiry| SystemTime::from(expiry) > SystemTime::now())
            })
    }

    /// A missing reason means all attempts failed transiently.
    async fn attach(
        &self,
        client: &cloud::Client,
        thread_id: &str,
    ) -> Result<(ReplyStream, Option<cloud::Event>), Option<&'static str>> {
        for delay in [
            Duration::ZERO,
            Duration::from_millis(500),
            Duration::from_secs(1),
        ] {
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            if !self.current().await {
                return Err(Some(SIGN_IN_AGAIN));
            }
            match client
                .attach(thread_id)
                .await
                .map_err(Some)
                .and_then(|mut stream| {
                    let first = check_reply_stream(&mut stream)?;
                    Ok((stream, first))
                }) {
                Ok(attachment) => {
                    if !self.current().await {
                        return Err(Some(SIGN_IN_AGAIN));
                    }
                    return Ok(attachment);
                }
                Err(Some(error)) if !transient(Some(&error)) => {
                    return Err(Some(blocked_recovery(&error).unwrap_or(RECOVERY_FAILED)));
                }
                Err(_) => {}
            }
        }
        Err(None)
    }
}

type Release = oneshot::Receiver<oneshot::Sender<io::Result<()>>>;

pub(super) struct Opened {
    pub(super) stream: ReplyStream,
    pub(super) first: Option<cloud::Event>,
    pub(super) ack: oneshot::Sender<io::Result<()>>,
}

type Open = BoxFuture<'static, Result<(ReplyStream, Option<cloud::Event>), Option<&'static str>>>;

pub(super) struct Recovery {
    pub(super) state: watch::Sender<IdleReconnect>,
    pub(super) pending: Option<(Arc<str>, Open)>,
    pub(super) installed: Option<Instant>,
    pub(super) held: Option<(String, Duration, BoxFuture<'static, Option<Opened>>)>,
    pub(super) attempts: u8,
    // None until either the initial Attach event or the activation snapshot establishes status.
    pub(super) observed_idle: Option<bool>,
    pub(super) client: Arc<cloud::Client>,
    pub(super) login: Arc<Login>,
}

pub(super) fn transient(error: Option<&cloud::Error>) -> bool {
    match error {
        None | Some(cloud::Error::Timeout) => true,
        Some(cloud::Error::Rpc(status)) => matches!(status.code, 1 | 2 | 4 | 13 | 14),
        Some(_) => false,
    }
}

pub(super) fn reply_stream_error(error: Option<&cloud::Error>) -> io::Error {
    match error {
        Some(cloud::Error::Timeout) => io::ErrorKind::TimedOut.into(),
        Some(cloud::Error::Rpc(status)) if status.code == 4 => io::ErrorKind::TimedOut.into(),
        error if transient(error) => io::ErrorKind::ConnectionAborted.into(),
        error => io::Error::other(error.and_then(blocked_recovery).unwrap_or(RECOVERY_FAILED)),
    }
}

fn recovery_backoff(attempts: u8) -> Duration {
    if attempts == 0 {
        return Duration::ZERO;
    }
    Duration::from_millis(500 << (attempts - 1).min(6)).min(MAX_RECOVERY_BACKOFF)
}

impl Recovery {
    pub(super) fn check_permit(&self) -> Result<(), &'static str> {
        self.login
            .permit
            .check()
            .map_err(|_| CLOUD_RECOVERY_NETWORK_BLOCKED)
    }

    pub(super) fn history_login(&self, thread_id: &str) -> Option<(Arc<Login>, Duration)> {
        self.held
            .as_ref()
            .filter(|(root, _, _)| root == thread_id)
            .map(|(_, timeout, _)| (Arc::clone(&self.login), *timeout))
    }

    pub(super) fn start(
        &mut self,
        thread_id: String,
        cached: Box<ThreadResumeResponse>,
        _require_history: bool,
        timeout: Duration,
        release: Release,
        mut response: oneshot::Sender<io::Result<RequestResult>>,
    ) {
        if response.is_closed() {
            return;
        }
        if self.login.mode == AuthMode::PersonalAccessToken {
            let _ = response.send(Ok(Err(recovery_error(
                "Reopen this cloud conversation to reconnect with your personal access token.",
            ))));
            return;
        }
        self.cancel();
        let backoff = recovery_backoff(self.attempts);
        self.attempts = self.attempts.saturating_add(1);
        let client = Arc::clone(&self.client);
        let login = Arc::clone(&self.login);
        let root = thread_id.clone();
        let open = async move {
            let attach = login.permit.run(login.attach(&client, &root));
            let attempt = async {
                if !backoff.is_zero() {
                    tokio::time::sleep(backoff).await;
                }
                tokio::time::timeout(timeout, attach).await
            };
            let opened = tokio::select! {
                biased;
                _ = response.closed() => return None,
                result = attempt => match result {
                    Ok(Ok(Ok(attachment))) => Ok(attachment),
                    Ok(Ok(Err(None))) => Err(Err(io::ErrorKind::ConnectionAborted.into())),
                    Ok(Ok(Err(Some(reason)))) => Err(Ok(Err(recovery_error(reason)))),
                    Ok(Err(_)) => Err(Ok(Err(recovery_error(CLOUD_RECOVERY_NETWORK_BLOCKED)))),
                    Err(error) => Err(Err(error.into())),
                },
            };
            let (stream, first) = match opened {
                Ok(opened) => opened,
                Err(failure) => {
                    let _ = response.send(failure);
                    return None;
                }
            };
            // Attach cannot prove that no complete turn was missed during the outage.
            // Attach resumes the conversation without configuration overrides. Keep
            // the settings already loaded for this thread and refresh its history.
            let settings = Some(cached);
            let value = match serde_json::to_value(&settings) {
                Ok(value) => value,
                Err(error) => {
                    let _ = response.send(Err(io::Error::other(error)));
                    return None;
                }
            };
            response.send(Ok(Ok(value))).ok()?;
            let mut ack = release.await.ok()?;
            let checked = login.permit.run(login.current());
            let authorized = tokio::select! {
                biased;
                _ = ack.closed() => return None,
                result = tokio::time::timeout(timeout, checked) => match result {
                    Ok(Ok(true)) => login.permit.check()
                        .map_err(|_| io::Error::other(CLOUD_RECOVERY_NETWORK_BLOCKED)),
                    Ok(Ok(false)) => Err(io::Error::other(CloudRecoverySignInChanged)),
                    Ok(Err(_)) => Err(io::Error::other(CLOUD_RECOVERY_NETWORK_BLOCKED)),
                    Err(error) => Err(error.into()),
                },
            };
            if let Err(reason) = authorized {
                let _ = ack.send(Err(reason));
                return None;
            }
            Some(Opened { stream, first, ack })
        };
        self.held = Some((thread_id, timeout, open.boxed()));
    }

    pub(super) fn cancel(&mut self) -> Option<Arc<str>> {
        self.held = None;
        drop(self.pending.take()?);
        let state = self.state.send_replace(IdleReconnect::Inactive);
        if let IdleReconnect::Opening(root) = state {
            Some(root)
        } else {
            None
        }
    }

    pub(super) fn reset(&mut self) {
        self.held = None;
        self.attempts = 0;
        drop(self.pending.take());
        self.state.send_replace(IdleReconnect::Inactive);
        self.installed = None;
        self.observed_idle = None;
    }

    pub(super) fn observe_busy(&mut self) {
        self.observed_idle = Some(false);
        // A status can overtake the wakeup before the App receives it.
        self.state.send_if_modified(|state| {
            if let IdleReconnect::Connected(root) = state {
                *state = IdleReconnect::Opening(Arc::clone(root));
                return true;
            }
            false
        });
    }

    pub(super) fn notify_connected(
        &self,
        id: &str,
        output: &tokio::sync::mpsc::UnboundedSender<crate::AppServerEvent>,
    ) {
        if self.state.send_if_modified(|state| {
            if let IdleReconnect::Opening(root) = state
                && root.as_ref() == id
            {
                *state = IdleReconnect::Connected(Arc::clone(root));
                return true;
            }
            false
        }) {
            let _ = output.send(crate::AppServerEvent::ThreadStreamReconnected(
                id.to_owned(),
            ));
        }
    }

    pub(super) fn new(
        state: watch::Sender<IdleReconnect>,
        client: Arc<cloud::Client>,
        login: Login,
    ) -> Self {
        Self {
            state,
            pending: None,
            installed: None,
            held: None,
            attempts: 0,
            observed_idle: None,
            client,
            login: Arc::new(login),
        }
    }

    pub(super) fn offer(&mut self, id: &str) {
        // Only OAuth credentials can be rechecked locally. PATs use ordinary explicit reopen.
        if !self.login.supports_recovery() {
            return;
        }
        let root: Arc<str> = Arc::from(id);
        self.state
            .send_replace(IdleReconnect::Offered(Arc::clone(&root)));
        let mut accepted = self.state.subscribe();
        let client = Arc::clone(&self.client);
        let login = Arc::clone(&self.login);
        let selected = Arc::clone(&root);
        let open = async move {
            // Hidden and dirty views keep the ordinary explicit-reopen behavior.
            accepted
                .wait_for(
                    |state| matches!(state, IdleReconnect::Opening(id) if Arc::ptr_eq(id, &selected)),
                )
                .await
                .map_err(|_| None)?;
            // Use the policy bound to the frozen serving credential when this client opened.
            let attempt = login.permit.run(login.attach(&client, &selected));
            tokio::time::timeout(Duration::from_secs(/*secs*/ 8), attempt)
                .await
                .map_err(|_| None)?
                .map_err(|_| None)?
                .map_err(|reason| {
                    Some(reason.unwrap_or(
                        "Cloud did not restore the reply stream after several attempts.",
                    ))
                })
        };
        self.pending = Some((root, open.boxed()));
    }
}

pub(super) fn recovery_error(reason: &'static str) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32000,
        message: reason.to_owned(),
        data: None,
    }
}
