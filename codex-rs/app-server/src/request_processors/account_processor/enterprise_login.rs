//! Owns one explicit enterprise sign-in attempt for a configured MCP server.
//! Start, replacement, cancellation, and local commit admission all pass through this state.
//! The credential store separately fences logins invalidated by another process.

use super::*;
use codex_config::McpEmaRegistration;
use codex_config::types::AuthKeyringBackendKind;
use codex_mcp::ema_auth_scope;
use codex_rmcp_client::EnterpriseOAuthLoginRequest;
use codex_rmcp_client::perform_enterprise_oauth_login_return_url;
use futures::FutureExt;
use tokio::sync::watch;

pub(crate) struct EnterpriseLoginTarget {
    pub(crate) thread_id: String,
    pub(crate) server_name: String,
    pub(crate) registration: McpEmaRegistration,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum EnterpriseLoginCompletion {
    Pending,
    Succeeded,
    Failed,
}

impl EnterpriseLoginCompletion {
    pub(crate) fn is_complete(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }
}

pub(crate) struct EnterpriseLogin {
    pub(crate) login_id: String,
    pub(crate) authorization_url: String,
    pub(crate) completion: watch::Receiver<EnterpriseLoginCompletion>,
}

pub(crate) struct EnterpriseLoginState {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    config_manager: ConfigManager,
    // Cancellation and commit admission use this same lock. A cancelled attempt cannot commit.
    active: Mutex<Option<EnterpriseLoginAttempt>>,
    // Start and cancel hold this through callback shutdown; the worker never acquires it.
    start_gate: Mutex<()>,
    // Account changes cancel queued and running setup before waiting for start_gate.
    setup_cancel: Mutex<CancellationToken>,
}

struct EnterpriseLoginAttempt {
    login_id: String,
    cancel: CancellationToken,
    completion: watch::Receiver<EnterpriseLoginCompletion>,
}

impl Drop for EnterpriseLoginAttempt {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl EnterpriseLoginState {
    pub(super) fn new(
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        config_manager: ConfigManager,
    ) -> Self {
        Self {
            auth_manager,
            thread_manager,
            config_manager,
            active: Mutex::new(None),
            start_gate: Mutex::new(()),
            setup_cancel: Mutex::new(CancellationToken::new()),
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "serialize start/cancel through callback shutdown; the worker uses a separate state lock"
    )]
    pub(super) async fn cancel(&self, login_id: Option<&str>) -> bool {
        if login_id.is_none() {
            let mut setup_cancel = self.setup_cancel.lock().await;
            setup_cancel.cancel();
            *setup_cancel = CancellationToken::new();
        }
        let (login_id, mut completion) = {
            let active = self.active.lock().await;
            if login_id.is_some_and(|id| active.as_ref().is_none_or(|a| a.login_id != id)) {
                return false;
            }
            let Some(attempt) = active.as_ref() else {
                return false;
            };
            attempt.cancel.cancel();
            (attempt.login_id.clone(), attempt.completion.clone())
        };
        // A partial callback request can stall the blocking HTTP worker. Cancellation
        // already fences commit; primary logout must not wait indefinitely for I/O.
        if tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let _start = self.start_gate.lock().await;
            completion.wait_for(|state| state.is_complete()).await
        })
        .await
        .is_ok()
        {
            self.clear(&login_id).await;
        }
        // Retain a timed-out attempt so a replacement still waits for port cleanup.
        true
    }

    async fn clear(&self, login_id: &str) {
        let mut active = self.active.lock().await;
        if active
            .as_ref()
            .is_some_and(|attempt| attempt.login_id == login_id)
        {
            *active = None;
        }
    }

    async fn configuration_is_current(
        &self,
        target: &EnterpriseLoginTarget,
        keyring_backend: AuthKeyringBackendKind,
    ) -> bool {
        let Ok(thread_id) = ThreadId::from_string(&target.thread_id) else {
            return false;
        };
        let Ok(thread) = self.thread_manager.get_thread(thread_id).await else {
            return false;
        };
        let current = thread.config().await;
        let Ok(config) = self
            .config_manager
            .load_latest_config_with_session_layers(&current.config_layer_stack, &current.cwd)
            .await
        else {
            return false;
        };
        let config = thread.runtime_mcp_config_and_context(&config).await.0;
        let auth = self.auth_manager.auth_cached();
        let servers = codex_mcp::effective_mcp_servers(&config, auth.as_ref());
        config.xaa_enabled
            && config.auth_keyring_backend_kind == keyring_backend
            && servers.get(&target.server_name).is_some_and(|server| {
                server.config().enabled
                    && server.config().ema_registration() == Some(&target.registration)
            })
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "serialize start/cancel through callback shutdown; the worker uses a separate state lock"
    )]
    pub(crate) async fn start(
        self: &Arc<Self>,
        request: EnterpriseOAuthLoginRequest<'_>,
        scope: codex_config::McpEmaAuthScope,
        target: EnterpriseLoginTarget,
    ) -> Result<EnterpriseLogin, JSONRPCErrorError> {
        // MCP requests are not in the account request queue. Serialize start and
        // cancellation here; the separate commit gate stays responsive to the worker.
        let setup_cancel = self.setup_cancel.lock().await.child_token();
        let _start = tokio::select! {
            biased;
            _ = setup_cancel.cancelled() => return Err(internal_error("enterprise sign-in was cancelled")),
            guard = self.start_gate.lock() => guard,
        };
        let previous = {
            let active = self.active.lock().await;
            // Keep the cancelled attempt installed until its callback worker closes.
            // A cancelled replacement request leaves the next start responsible for waiting.
            active.as_ref().map(|attempt| {
                attempt.cancel.cancel();
                (attempt.login_id.clone(), attempt.completion.clone())
            })
        };
        if let Some((login_id, mut completion)) = previous {
            // An abandoned setup request drops its flow and closes this channel.
            let _ = tokio::select! {
                biased;
                _ = setup_cancel.cancelled() => return Err(internal_error("enterprise sign-in was cancelled")),
                result = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    completion.wait_for(|state| state.is_complete()),
                ) => result.map_err(|_| internal_error("previous enterprise sign-in did not close"))?,
            };
            self.clear(&login_id).await;
        }
        let keyring_backend = request.keyring_backend_kind;
        // Keep commit cancellation under `active`; cancelling the setup epoch must
        // not race an already-admitted synchronous credential write.
        let cancel = CancellationToken::new();
        let login_id = Uuid::new_v4().to_string();
        let (completed, completion) = watch::channel(EnterpriseLoginCompletion::Pending);
        *self.active.lock().await = Some(EnterpriseLoginAttempt {
            login_id: login_id.clone(),
            cancel: cancel.clone(),
            completion: completion.clone(),
        });
        let handle = tokio::select! {
            biased;
            _ = setup_cancel.cancelled() => Err(internal_error("enterprise sign-in was cancelled")),
            _ = cancel.cancelled() => Err(internal_error("enterprise sign-in was cancelled")),
            result = perform_enterprise_oauth_login_return_url(request) =>
                result.map_err(|_| internal_error("failed to start enterprise sign-in")),
        };
        let handle = match handle {
            Ok(handle) => handle,
            Err(error) => {
                self.clear(&login_id).await;
                completed.send_replace(EnterpriseLoginCompletion::Failed);
                return Err(error);
            }
        };
        let callback_closed = handle.callback_closed().shared();
        let authorization_url = handle.authorization_url();
        let lifecycle = Arc::clone(self);
        let completion_id = login_id.clone();
        tokio::spawn(async move {
            let result = tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(anyhow::anyhow!("Enterprise sign-in was cancelled.")),
                result = async {
                    let credentials = handle.wait().await?;
                    callback_closed.clone().await;
                    let mut active = credentials.commit_if(|| async {
                        lifecycle.auth_manager.reload().await;
                        let eligible = lifecycle.configuration_is_current(&target, keyring_backend).await;
                        // Reread persisted account authority after acquiring the credential
                        // lock, including changes made by another app-server process.
                        lifecycle.auth_manager.reload().await;
                        if !eligible {
                            return None;
                        }
                        // Keep cancellation responsive during the async checks above.
                        // Once admitted, retain this guard through the synchronous write.
                        let active = lifecycle.active.lock().await;
                        (active.as_ref().is_some_and(|attempt| {
                            attempt.login_id == completion_id && !attempt.cancel.is_cancelled()
                        })
                            && ema_auth_scope(lifecycle.auth_manager.auth_cached().as_ref()).as_ref() == Some(&scope))
                            .then_some(active)
                    }).await?;
                    // A successful commit is no longer cancelable. Clear it while
                    // still holding the commit gate.
                    *active = None;
                    Ok(())
                } => result,
            };
            // The canceled wait future has dropped its listener guard. Also wait
            // for the blocking callback worker before allowing a fixed-port retry.
            callback_closed.await;
            if result.is_err() {
                lifecycle.clear(&completion_id).await;
            }
            let success = result.is_ok();
            completed.send_replace(if success {
                EnterpriseLoginCompletion::Succeeded
            } else {
                EnterpriseLoginCompletion::Failed
            });
        });
        Ok(EnterpriseLogin {
            login_id,
            authorization_url,
            completion,
        })
    }
}
