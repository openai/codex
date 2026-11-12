//! Reopens only the lost reply stream of a displayed, idle Cloud conversation.

use super::ReplyStream;
use crate::cloud::IdleReconnect;
use codex_cloud_client as cloud;
use codex_http_client::NetworkPermit;
use codex_login::AuthConfig;
use codex_login::token_data::parse_jwt_expiration;
use codex_protocol::auth::AuthMode;
use futures::FutureExt;
use futures::StreamExt;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tokio::sync::watch;
use tokio::time::Instant;

pub(super) struct Login {
    pub(super) storage: AuthConfig,
    pub(super) mode: AuthMode,
    pub(super) token: String,
    pub(super) account: String,
    pub(super) permit: NetworkPermit,
}

impl Login {
    async fn current(&self) -> bool {
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
}

type Open = BoxFuture<'static, Option<(ReplyStream, Option<cloud::Event>)>>;

pub(super) struct Recovery {
    pub(super) state: watch::Sender<IdleReconnect>,
    pub(super) pending: Option<(Arc<str>, Open)>,
    pub(super) installed: Option<Instant>,
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

impl Recovery {
    pub(super) fn cancel(&mut self) -> Option<Arc<str>> {
        drop(self.pending.take()?);
        let state = self.state.send_replace(IdleReconnect::Inactive);
        if let IdleReconnect::Opening(root) = state {
            Some(root)
        } else {
            None
        }
    }

    pub(super) fn reset(&mut self) {
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

    pub(super) fn offer(&mut self, id: &str) {
        // Only OAuth credentials can be rechecked locally. PATs use ordinary explicit reopen.
        if !matches!(
            self.login.mode,
            AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens
        ) {
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
                .ok()?;
            // Use the policy bound to the frozen serving credential when this client opened.
            let attempts = login
                .permit
                // A revoke-then-allow cannot restart this episode or release a buffered first event.
                .run(async {
                    for delay in [
                        Duration::ZERO,
                        Duration::from_millis(500),
                        Duration::from_secs(1),
                    ] {
                        if !delay.is_zero() {
                            tokio::time::sleep(delay).await;
                        }
                        if !login.current().await {
                            return None;
                        }
                        match client.attach(&selected).await {
                            Ok(mut stream) => {
                                let first =
                                    tokio::task::unconstrained(stream.next()).now_or_never();
                                let event = match first {
                                    None => None,
                                    Some(Some(Ok(event))) => Some(event),
                                    Some(Some(Err(error))) if !transient(Some(&error)) => {
                                        return None;
                                    }
                                    Some(_) => continue,
                                };
                                return login.current().await.then_some((stream, event));
                            }
                            Err(error) if transient(Some(&error)) => {}
                            Err(_) => return None,
                        }
                    }
                    None
                });
            // Bound this accepted recovery episode, including credential checks and retry backoff.
            tokio::time::timeout(Duration::from_secs(/*secs*/ 8), attempts)
                .await
                .ok()?
                .ok()
                .flatten()
        };
        self.pending = Some((root, open.boxed()));
    }
}
