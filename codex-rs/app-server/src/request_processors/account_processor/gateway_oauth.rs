//! Gateway sign-in RPCs share credentials with inference and bind cancellation to the initiating connection.
//! Cancellation is acknowledged only after the active login releases its slot.

use super::*;
use crate::gateway_oauth_notifications::status_fields;
use crate::transport::ConnectionId;
use codex_app_server_protocol::GatewayOAuthCancelResponse;
use codex_app_server_protocol::GatewayOAuthLoginResponse;
use codex_app_server_protocol::GatewayOAuthReadResponse;
use codex_login::GatewayAuthManager;
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
    ) -> Result<(Config, Option<Arc<GatewayAuthManager>>), JSONRPCErrorError> {
        let config = self
            .config_manager
            .load_latest_config(/*fallback_cwd*/ None)
            .await
            .map_err(|err| {
                internal_error(format!("failed to load gateway configuration: {err}"))
            })?;
        let manager = codex_model_provider::create_model_provider(
            config.model_provider.clone(),
            Some(Arc::clone(&self.auth_manager)),
        )
        .gateway_auth_manager()
        .map_err(|error| internal_error(error.to_string()))?;
        *self
            .gateway_client
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = manager.clone();
        Ok((config, manager))
    }

    pub(crate) async fn gateway_oauth_read(
        &self,
    ) -> Result<GatewayOAuthReadResponse, JSONRPCErrorError> {
        let (config, client) = self.gateway_client().await?;
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
            provider_id: config.model_provider_id,
            provider_name: config.model_provider.name,
            required: state.is_some(),
            status: state.as_ref().map(|state| state.0),
            error: state.and_then(|state| state.1),
        })
    }

    pub(crate) async fn gateway_oauth_login(
        &self,
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
        let (config, client) = self.gateway_client().await?;
        let client = client
            .ok_or_else(|| invalid_request("The current provider does not use gateway OAuth"))?;
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
