//! Gateway sign-in RPCs share credentials with inference and bind cancellation to the initiating connection.
//! Cancellation is acknowledged only after the active login releases its slot.

use super::*;
use crate::gateway_oauth_notifications::status_fields;
use crate::transport::ConnectionId;
use codex_app_server_protocol::GatewayOAuthCancelResponse;
use codex_app_server_protocol::GatewayOAuthLoginResponse;
use codex_app_server_protocol::GatewayOAuthReadResponse;
use codex_login::OAuthSessionManager;
use codex_model_provider::GatewayAuthManager;
use std::sync::PoisonError;

pub(super) struct ActiveGatewayLogin {
    owner: ConnectionId,
    cancel: CancellationToken,
    finished: CancellationToken,
}

struct LoginGuard(Arc<std::sync::Mutex<Option<ActiveGatewayLogin>>>);

impl Drop for LoginGuard {
    fn drop(&mut self) {
        if let Some(active) = self.0.lock().unwrap_or_else(PoisonError::into_inner).take() {
            active.finished.cancel();
        }
    }
}

impl AccountRequestProcessor {
    async fn gateway_client(
        &self,
        model_provider: Option<String>,
    ) -> Result<(Arc<Config>, Option<Arc<OAuthSessionManager>>), JSONRPCErrorError> {
        let config = match model_provider.as_deref() {
            Some(provider_id) => {
                let config = config_for_provider(&self.config, provider_id)?;
                // Explicit selectors use startup definitions, so validate against current policy.
                self.config_manager
                    .check_thread_model_provider(&config)
                    .await
                    .map_err(|err| config_load_error(&err))?;
                config
            }
            None => Arc::new(
                self.config_manager
                    .load_latest_config(/*fallback_cwd*/ None)
                    .await
                    .map_err(|err| config_load_error(&err))?,
            ),
        };
        let manager = codex_model_provider::create_model_provider(
            config.model_provider.clone(),
            Some(Arc::clone(&self.auth_manager)),
        )
        .gateway_auth_manager()
        .map_err(|error| internal_error(error.to_string()))?;
        let session = manager
            .as_deref()
            .and_then(GatewayAuthManager::oauth_session);
        // Keep sign-in status alive when clients alternate between providers.
        if let Some(manager) = &manager {
            self.gateway_clients
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(config.model_provider_id.clone(), Arc::clone(manager));
        }
        Ok((config, session))
    }

    pub(crate) async fn gateway_oauth_read(
        &self,
        params: codex_app_server_protocol::GatewayOAuthReadParams,
    ) -> Result<GatewayOAuthReadResponse, JSONRPCErrorError> {
        let (config, client) = self.gateway_client(params.model_provider).await?;
        let state = if let Some(client) = client {
            Some(status_fields(
                client
                    .status()
                    .await
                    .map_err(|err| internal_error(err.to_string()))?,
            ))
        } else {
            None
        };
        Ok(GatewayOAuthReadResponse {
            provider_id: config.model_provider_id.clone(),
            provider_name: config.model_provider.name.clone(),
            required: state.is_some(),
            status: state.as_ref().map(|state| state.0),
            error: state.and_then(|state| state.1),
        })
    }

    pub(crate) async fn gateway_oauth_login(
        &self,
        params: codex_app_server_protocol::GatewayOAuthLoginParams,
        owner: ConnectionId,
        connection_gate: &crate::connection_rpc_gate::ConnectionRpcGate,
    ) -> Result<GatewayOAuthLoginResponse, JSONRPCErrorError> {
        let cancel = CancellationToken::new();
        let guard = {
            let mut active = self
                .gateway_login
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if connection_gate.is_closed() {
                return Err(invalid_request("The connection is closed"));
            }
            if active.is_some() {
                return Err(invalid_request("Gateway sign-in is already in progress"));
            }
            *active = Some(ActiveGatewayLogin {
                owner,
                cancel: cancel.clone(),
                finished: CancellationToken::new(),
            });
            LoginGuard(Arc::clone(&self.gateway_login))
        };
        let (config, client) = self.gateway_client(params.model_provider).await?;
        let client = client
            .ok_or_else(|| invalid_request("The selected provider does not use gateway OAuth"))?;
        if config.model_provider.requires_openai_auth
            && config.model_provider.env_key.is_none()
            && config.model_provider.experimental_bearer_token.is_none()
            && config.model_provider.auth.is_none()
            && self.auth_manager.auth_cached().is_none()
        {
            return Err(invalid_request(
                "Sign in to your primary account before signing in to the gateway",
            ));
        }
        let (url_tx, url_rx) = tokio::sync::oneshot::channel();
        let login = client.login_with_browser(cancel.cancelled(), |url| {
            let _ = url_tx.send(url.to_string());
        });
        tokio::pin!(login);
        let result = tokio::select! {
            result = &mut login => result,
            url = url_rx => {
                if let Ok(url) = url {
                    self.outgoing.send_server_notification_to_connections(&[owner], ServerNotification::GatewayOAuthChanged(codex_app_server_protocol::GatewayOAuthChangedNotification {
                        auth_url: Some(url), provider_id: config.model_provider_id.clone(),
                        status: codex_app_server_protocol::GatewayOAuthStatus::Started, error: None,
                    })).await;
                }
                login.await
            }
        };
        result.map_err(|err| internal_error(err.to_string()))?;
        drop(guard);
        Ok(GatewayOAuthLoginResponse {})
    }

    pub(crate) async fn gateway_oauth_cancel(
        &self,
        owner: ConnectionId,
    ) -> Result<GatewayOAuthCancelResponse, JSONRPCErrorError> {
        let finished = {
            let active = self
                .gateway_login
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let Some(active) = active.as_ref() else {
                return Ok(GatewayOAuthCancelResponse {});
            };
            if active.owner != owner {
                return Err(invalid_request(
                    "Gateway sign-in belongs to another connection",
                ));
            }
            active.cancel.cancel();
            active.finished.clone()
        };
        finished.cancelled().await;
        Ok(GatewayOAuthCancelResponse {})
    }

    pub(crate) fn gateway_connection_closed(&self, owner: ConnectionId) {
        let active = self
            .gateway_login
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(active) = active.as_ref()
            && active.owner == owner
        {
            active.cancel.cancel();
        }
    }

    pub(super) fn cancel_gateway_login(&self) {
        let active = self
            .gateway_login
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(active) = active.as_ref() {
            active.cancel.cancel();
        }
    }
}
