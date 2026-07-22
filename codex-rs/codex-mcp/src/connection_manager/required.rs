use anyhow::Result;
use anyhow::anyhow;
use codex_protocol::protocol::McpStartupFailure;
use tracing::Instrument;
use tracing::info_span;

use super::McpConnectionSet;
use crate::rmcp_client::StartupOutcomeError;

impl McpConnectionSet {
    /// Waits for every required server and reports their startup failures together.
    ///
    /// Callers must make the manager reachable to request handlers before awaiting this method,
    /// because server initialization may require client elicitation.
    pub async fn validate_required_servers(&self) -> Result<()> {
        let failures = async {
            let mut failures = Vec::new();
            for server_name in &self.required_servers {
                let Some(async_managed_client) = self.clients.get(server_name).cloned() else {
                    failures.push(McpStartupFailure {
                        server: server_name.clone(),
                        error: format!("required MCP server `{server_name}` was not initialized"),
                    });
                    continue;
                };

                match async_managed_client.client().await {
                    Ok(_) => {}
                    Err(error) => failures.push(McpStartupFailure {
                        server: server_name.clone(),
                        error: startup_outcome_error_message(error),
                    }),
                }
            }
            failures
        }
        .instrument(info_span!(
            "session_init.required_mcp_wait",
            otel.name = "session_init.required_mcp_wait",
            session_init.required_mcp_server_count = self.required_servers.len(),
        ))
        .await;
        if failures.is_empty() {
            return Ok(());
        }

        let details = failures
            .iter()
            .map(|failure| format!("{}: {}", failure.server, failure.error))
            .collect::<Vec<_>>()
            .join("; ");
        Err(anyhow!(
            "required MCP servers failed to initialize: {details}"
        ))
    }
}

pub(super) fn startup_outcome_error_message(error: StartupOutcomeError) -> String {
    match error {
        StartupOutcomeError::Cancelled => "MCP startup cancelled".to_string(),
        StartupOutcomeError::Failed { error, .. } => error,
    }
}
