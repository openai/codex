//! Forward OAuth progress for the app's current gateway.

use crate::config_manager::ConfigManager;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::GatewayOAuthChangedNotification;
use codex_app_server_protocol::GatewayOAuthStatus;
use codex_app_server_protocol::ServerNotification;
use codex_login::AuthManager;
use codex_login::GatewayAuthStatus;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tokio_util::task::AbortOnDropHandle;

pub(crate) fn status_fields(status: GatewayAuthStatus) -> (GatewayOAuthStatus, Option<String>) {
    match status {
        GatewayAuthStatus::NotReady => (GatewayOAuthStatus::NotReady, None),
        GatewayAuthStatus::Started => (GatewayOAuthStatus::Started, None),
        GatewayAuthStatus::Succeeded => (GatewayOAuthStatus::Succeeded, None),
        GatewayAuthStatus::Failed { message } => (GatewayOAuthStatus::Failed, Some(message)),
    }
}

pub(crate) fn spawn(
    auth_manager: Arc<AuthManager>,
    config_manager: ConfigManager,
    outgoing: Arc<OutgoingMessageSender>,
) -> AbortOnDropHandle<()> {
    let mut events = codex_login::subscribe_gateway_auth_status(&auth_manager.runtime_config());
    AbortOnDropHandle::new(tokio::spawn(async move {
        loop {
            let change = match events.recv().await {
                Ok(change) => change,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            };
            let Ok(config) = config_manager
                .load_latest_config(/*fallback_cwd*/ None)
                .await
            else {
                continue;
            };
            let Ok(Some(manager)) = codex_model_provider::create_model_provider(
                config.model_provider.clone(),
                Some(Arc::clone(&auth_manager)),
            )
            .gateway_auth_manager() else {
                continue;
            };
            if manager.config() != &change.config {
                continue;
            }
            let (status, error) = status_fields(change.status);
            outgoing
                .send_server_notification(ServerNotification::GatewayOAuthChanged(
                    GatewayOAuthChangedNotification {
                        auth_url: None,
                        provider_id: config.model_provider_id,
                        status,
                        error,
                    },
                ))
                .await;
        }
    }))
}
