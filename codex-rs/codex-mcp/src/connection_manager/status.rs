//! Read-only connection state; observing a server must not start it.

use std::collections::HashMap;

use codex_protocol::mcp::McpServerConnectionStatus;

use super::McpConnectionSet;

impl McpConnectionSet {
    pub(crate) async fn connection_statuses(&self) -> HashMap<String, McpServerConnectionStatus> {
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
            statuses.insert(name.clone(), status);
        }
        statuses
    }
}
