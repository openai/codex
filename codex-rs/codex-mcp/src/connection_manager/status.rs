//! Read-only connection state; observing a server must not start it.

use std::collections::HashMap;

use codex_protocol::mcp::McpServerConnectionStatus;

use super::McpConnectionSet;
use crate::McpServerKey;
use crate::server::McpServerConnectionIdentity;

impl McpConnectionSet {
    /// Reads readiness without starting deferred clients or consulting the OAuth store.
    pub(crate) async fn has_reusable_status_catalog(&self, key: &McpServerKey) -> bool {
        let Some(view) = self.servers.get(key) else {
            return false;
        };
        let connection = &view.connection;
        connection
            .identity
            .as_ref()
            .is_some_and(McpServerConnectionIdentity::has_fixed_status_credentials)
            && connection.client.connection_status().await == McpServerConnectionStatus::Connected
    }

    pub(crate) async fn connection_statuses(
        &self,
    ) -> HashMap<McpServerKey, McpServerConnectionStatus> {
        use McpServerConnectionStatus as Status;

        let mut statuses = self
            .disabled_servers
            .iter()
            .map(|name| (name.clone(), Status::Disabled))
            .collect::<HashMap<_, _>>();
        for (name, view) in &self.servers {
            let connection = &view.connection;
            let client = &connection.client;
            let status = if connection.startup_is_dormant() && !client.cancel_token.is_cancelled() {
                Status::NotStarted
            } else {
                client.connection_status().await
            };
            statuses.insert(name.to_owned(), status);
        }
        statuses
    }
}
