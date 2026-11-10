//! MCP startup state and status handling for the chat widget.
//!
//! The app server reports MCP server startup as per-server status updates. This
//! module tracks only the starts and outcomes the TUI receives, including
//! partial startup rounds after attaching to a running app server.
//! Each active round includes every outcome it receives, even if the client has
//! already shown the same diagnostic in an earlier round.
//! Initial and runtime diagnostics remain in retained history and the warnings viewer;
//! compact history hides their full text behind the persistent warning notice.

use codex_app_server_protocol::McpServerStartupFailureReason;
use codex_app_server_protocol::McpServerStartupState;
use codex_app_server_protocol::McpServerStatusUpdatedNotification;

use super::ChatWidget;

const MCP_STARTUP_SINGLE_HEADER_PREFIX: &str = "Booting MCP server:";
const MCP_STARTUP_MULTI_HEADER_PREFIX: &str = "Starting MCP servers";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum McpStartupStatus {
    Starting,
    Ready,
    Failed {
        error: String,
        failure_reason: Option<McpServerStartupFailureReason>,
    },
    Cancelled,
}

impl McpStartupStatus {
    fn is_diagnostic(&self) -> bool {
        matches!(self, Self::Failed { .. } | Self::Cancelled)
    }
}

impl ChatWidget {
    /// The server announces eager starts before outcomes. On attach or after lag,
    /// an outcome can arrive without its corresponding start.
    fn update_mcp_startup_status(&mut self, server: String, status: McpStartupStatus) {
        let report_diagnostic = if status.is_diagnostic() {
            self.mcp_startup_last_diagnostics
                .insert(server.clone(), status.clone())
                .as_ref()
                != Some(&status)
        } else {
            self.mcp_startup_last_diagnostics.remove(&server);
            false
        };
        if report_diagnostic {
            self.report_mcp_startup_failure(&server, &status);
        }
        if self.mcp_startup_status.is_none() && !matches!(status, McpStartupStatus::Starting) {
            if report_diagnostic {
                self.report_mcp_startup_summary([(&server, &status)]);
            }
            return;
        }

        self.mcp_startup_status
            .get_or_insert_default()
            .insert(server, status);
        if let Some(header) = self.mcp_startup_status_header() {
            self.update_task_running_state();
            self.set_status_header(header);
            self.request_redraw();
        } else {
            self.finish_mcp_startup();
        }
    }

    pub(super) fn mcp_startup_status_header(&self) -> Option<String> {
        let current = self.mcp_startup_status.as_ref()?;
        let total = current.len();
        let mut starting: Vec<_> = current
            .iter()
            .filter_map(|(name, state)| {
                if matches!(state, McpStartupStatus::Starting) {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
        starting.sort();
        let first = starting.first()?;
        let completed = total.saturating_sub(starting.len());
        let max_to_show = 3;
        let mut to_show: Vec<String> = starting
            .iter()
            .take(max_to_show)
            .map(ToString::to_string)
            .collect();
        if starting.len() > max_to_show {
            to_show.push("…".to_string());
        }
        Some(if total > 1 {
            format!(
                "{MCP_STARTUP_MULTI_HEADER_PREFIX} ({completed}/{total}): {}",
                to_show.join(", ")
            )
        } else {
            format!("{MCP_STARTUP_SINGLE_HEADER_PREFIX} {first}")
        })
    }

    fn report_mcp_startup_failure(&mut self, server: &str, status: &McpStartupStatus) {
        if let McpStartupStatus::Failed {
            error,
            failure_reason,
        } = status
        {
            self.add_mcp_startup_warning(vec![error.clone()], [server.to_owned()], *failure_reason);
        }
    }

    fn report_mcp_startup_summary<'a>(
        &mut self,
        outcomes: impl IntoIterator<Item = (&'a String, &'a McpStartupStatus)>,
    ) {
        let mut failed = Vec::new();
        let mut cancelled = Vec::new();
        for (server, status) in outcomes {
            match status {
                McpStartupStatus::Failed { .. } => failed.push(server.clone()),
                McpStartupStatus::Cancelled => cancelled.push(server.clone()),
                McpStartupStatus::Starting | McpStartupStatus::Ready => {}
            }
        }
        if !cancelled.is_empty() {
            cancelled.sort();
            self.add_mcp_startup_warning(
                vec![format!(
                    "MCP startup interrupted. The following servers were not initialized: {}",
                    cancelled.join(", ")
                )],
                cancelled,
                /*failure_reason*/ None,
            );
        }
        if !failed.is_empty() {
            failed.sort();
            self.add_mcp_startup_warning(
                vec![format!(
                    "MCP startup incomplete (failed: {})",
                    failed.join(", ")
                )],
                failed,
                /*failure_reason*/ None,
            );
        }
    }

    pub(super) fn finish_mcp_startup(&mut self) {
        let mcp_startup_owned_status = self.status_header_is_mcp_startup_owned();
        if let Some(settled) = self.mcp_startup_status.take() {
            self.report_mcp_startup_summary(&settled);
        }
        self.update_task_running_state();
        if self.input_queue.user_turn_pending_start {
            self.bottom_pane.set_task_running(/*running*/ true);
        }
        if self.bottom_pane.is_task_running() && mcp_startup_owned_status {
            self.restore_reasoning_status_header();
        }
        self.maybe_send_next_queued_input();
        self.request_redraw();
    }

    pub(crate) fn finish_mcp_startup_after_lag(&mut self) {
        if self.mcp_startup_status.is_some() {
            self.finish_mcp_startup();
        }
        // A missed Starting may have begun a new attempt with the same error.
        self.mcp_startup_last_diagnostics.clear();
    }

    pub(super) fn status_header_is_mcp_startup_owned(&self) -> bool {
        self.status_state
            .current_status
            .header
            .starts_with(MCP_STARTUP_SINGLE_HEADER_PREFIX)
            || self
                .status_state
                .current_status
                .header
                .starts_with(MCP_STARTUP_MULTI_HEADER_PREFIX)
    }

    fn add_mcp_startup_warning(
        &mut self,
        messages: Vec<String>,
        servers: impl IntoIterator<Item = String>,
        failure_reason: Option<McpServerStartupFailureReason>,
    ) {
        self.add_to_history(crate::history_cell::StartupWarningsCell::mcp(
            messages,
            servers,
            failure_reason,
        ));
    }

    /// Update startup state and retry installed-app discovery when its MCP server becomes ready.
    pub(super) fn on_mcp_server_status_updated(
        &mut self,
        notification: McpServerStatusUpdatedNotification,
    ) {
        let refresh_connector_mentions = notification.name == "codex_apps"
            && notification.status == McpServerStartupState::Ready;
        let status = match notification.status {
            McpServerStartupState::Starting => McpStartupStatus::Starting,
            McpServerStartupState::Ready => McpStartupStatus::Ready,
            McpServerStartupState::Failed => McpStartupStatus::Failed {
                failure_reason: notification.failure_reason,
                error: notification.error.unwrap_or_else(|| {
                    format!("MCP client for `{}` failed to start", notification.name)
                }),
            },
            McpServerStartupState::Cancelled => McpStartupStatus::Cancelled,
        };
        self.update_mcp_startup_status(notification.name, status);
        if refresh_connector_mentions {
            self.refresh_connector_mentions(/*force_refresh*/ false);
        }
    }
}
