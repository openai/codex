//! Observe the latest startup attempt without polling it or initiating a retry.

use codex_protocol::mcp::McpServerConnectionStatus as Status;

use super::AsyncManagedClient;
use super::StartupOutcomeError;

impl AsyncManagedClient {
    pub(crate) async fn connection_status(&self) -> Status {
        if self.cancel_token.is_cancelled() {
            return Status::Cancelled;
        }
        let reconnect_outcome = if let Some(reconnect) = &self.startup_reconnect {
            let state = reconnect
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.reconnect_in_flight {
                return Status::Starting;
            }
            state
                .current_client
                .clone()
                .map(Ok)
                .or_else(|| state.last_error.clone().map(Err))
        } else {
            None
        };
        let outcome = reconnect_outcome.or_else(|| self.client.peek().cloned());
        match outcome {
            Some(Ok(client)) => {
                if client.client.is_closed().await {
                    Status::Failed
                } else {
                    Status::Connected
                }
            }
            Some(Err(error)) if error.is_authentication_required() => {
                Status::AuthenticationRequired
            }
            Some(Err(StartupOutcomeError::Failed { .. })) => Status::Failed,
            Some(Err(StartupOutcomeError::Cancelled)) => Status::Cancelled,
            None => Status::Starting,
        }
    }
}
