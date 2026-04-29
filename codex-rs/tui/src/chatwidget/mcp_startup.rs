//! MCP startup state and status handling for the chat widget.
//!
//! This module is a mechanical extraction from `chatwidget.rs`: it keeps the
//! buffered startup-round bookkeeping together while preserving the existing
//! event shapes and rendering behavior.

use std::collections::BTreeSet;

use codex_app_server_protocol::McpServerStartupState;
use codex_app_server_protocol::McpServerStatusUpdatedNotification;
#[cfg(test)]
use codex_protocol::protocol::McpStartupCompleteEvent;
use codex_protocol::protocol::McpStartupStatus;
#[cfg(test)]
use codex_protocol::protocol::McpStartupUpdateEvent;

use super::ChatWidget;

impl ChatWidget {
    /// Record one MCP startup update, promoting it into either the active startup
    /// round or a buffered "next" round.
    ///
    /// This path has to deal with lossy app-server delivery. After
    /// `finish_mcp_startup()` or `finish_mcp_startup_after_lag()`, we briefly
    /// ignore incoming updates so stale events from the just-finished round do not
    /// reopen startup. While that guard is active we buffer updates for a possible
    /// next round, and only reactivate once the buffered set is coherent enough to
    /// treat as a fresh startup round.
    fn update_mcp_startup_status(
        &mut self,
        server: String,
        status: McpStartupStatus,
        complete_when_settled: bool,
    ) {
        let mut activated_pending_round = false;
        let startup_status = if self.mcp_startup_ignore_updates_until_next_start {
            // Ignore-mode buffers the next plausible round so stale post-finish
            // updates cannot immediately reopen startup. A fresh `Starting`
            // update resets the buffer only if we have not already seen a
            // pending-round `Starting`; this preserves valid interleavings like
            // `alpha: Starting -> alpha: Ready -> beta: Starting`.
            if matches!(status, McpStartupStatus::Starting)
                && !self.mcp_startup_pending_next_round_saw_starting
            {
                self.mcp_startup_pending_next_round.clear();
                self.mcp_startup_allow_terminal_only_next_round = false;
            }
            self.mcp_startup_pending_next_round_saw_starting |=
                matches!(status, McpStartupStatus::Starting);
            self.mcp_startup_pending_next_round.insert(server, status);
            let Some(expected_servers) = &self.mcp_startup_expected_servers else {
                return;
            };
            let saw_full_round = expected_servers.is_empty()
                || expected_servers
                    .iter()
                    .all(|name| self.mcp_startup_pending_next_round.contains_key(name));
            let saw_starting = self
                .mcp_startup_pending_next_round
                .values()
                .any(|state| matches!(state, McpStartupStatus::Starting));
            if !(saw_full_round
                && (saw_starting || self.mcp_startup_allow_terminal_only_next_round))
            {
                return;
            }

            // The buffered map now looks like a complete next round, so promote it
            // to the active round and resume normal completion tracking.
            self.mcp_startup_ignore_updates_until_next_start = false;
            self.mcp_startup_allow_terminal_only_next_round = false;
            self.mcp_startup_pending_next_round_saw_starting = false;
            activated_pending_round = true;
            std::mem::take(&mut self.mcp_startup_pending_next_round)
        } else {
            // Normal path: fold the update into the active round and surface
            // per-server failures immediately.
            let mut startup_status = self.mcp_startup_status.take().unwrap_or_default();
            if let McpStartupStatus::Failed { error } = &status {
                self.on_warning(error);
            }
            startup_status.insert(server, status);
            startup_status
        };
        if activated_pending_round {
            // A promoted buffered round may already contain terminal failures.
            for state in startup_status.values() {
                if let McpStartupStatus::Failed { error } = state {
                    self.on_warning(error);
                }
            }
        }
        self.mcp_startup_status = Some(startup_status);
        self.update_task_running_state();

        // App-server-backed startup completes when every expected server has
        // reported a non-Starting status. Lag handling can force an earlier
        // settle via `finish_mcp_startup_after_lag()`.
        if complete_when_settled
            && let Some(current) = &self.mcp_startup_status
            && let Some(expected_servers) = &self.mcp_startup_expected_servers
            && !current.is_empty()
            && expected_servers
                .iter()
                .all(|name| current.contains_key(name))
            && current
                .values()
                .all(|state| !matches!(state, McpStartupStatus::Starting))
        {
            let mut failed = Vec::new();
            let mut cancelled = Vec::new();
            for (name, state) in current {
                match state {
                    McpStartupStatus::Ready => {}
                    McpStartupStatus::Failed { .. } => failed.push(name.clone()),
                    McpStartupStatus::Cancelled => cancelled.push(name.clone()),
                    McpStartupStatus::Starting => {}
                }
            }
            failed.sort();
            cancelled.sort();
            self.finish_mcp_startup(failed, cancelled);
            return;
        }
        if let Some(current) = &self.mcp_startup_status {
            // Otherwise keep the status header focused on the remaining
            // in-progress servers for the active round.
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
            if let Some(first) = starting.first() {
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
                let header = if total > 1 {
                    format!(
                        "Starting MCP servers ({completed}/{total}): {}",
                        to_show.join(", ")
                    )
                } else {
                    format!("Booting MCP server: {first}")
                };
                self.set_status_header(header);
            }
        }
        self.request_redraw();
    }

    pub(crate) fn set_mcp_startup_expected_servers<I>(&mut self, server_names: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.mcp_startup_expected_servers = Some(server_names.into_iter().collect());
    }

    #[cfg(test)]
    pub(super) fn on_mcp_startup_update(&mut self, ev: McpStartupUpdateEvent) {
        self.update_mcp_startup_status(ev.server, ev.status, /*complete_when_settled*/ false);
    }

    pub(super) fn finish_mcp_startup(&mut self, failed: Vec<String>, cancelled: Vec<String>) {
        if !cancelled.is_empty() {
            self.on_warning(format!(
                "MCP startup interrupted. The following servers were not initialized: {}",
                cancelled.join(", ")
            ));
        }
        let mut parts = Vec::new();
        if !failed.is_empty() {
            parts.push(format!("failed: {}", failed.join(", ")));
        }
        if !parts.is_empty() {
            self.on_warning(format!("MCP startup incomplete ({})", parts.join("; ")));
        }

        self.mcp_startup_status = None;
        self.mcp_startup_ignore_updates_until_next_start = true;
        self.mcp_startup_allow_terminal_only_next_round = false;
        self.mcp_startup_pending_next_round.clear();
        self.mcp_startup_pending_next_round_saw_starting = false;
        self.update_task_running_state();
        self.maybe_send_next_queued_input();
        self.request_redraw();
    }

    pub(crate) fn finish_mcp_startup_after_lag(&mut self) {
        if self.mcp_startup_ignore_updates_until_next_start {
            if self.mcp_startup_pending_next_round.is_empty() {
                self.mcp_startup_pending_next_round_saw_starting = false;
            }
            self.mcp_startup_allow_terminal_only_next_round = true;
        }

        let Some(current) = &self.mcp_startup_status else {
            return;
        };

        let mut failed = Vec::new();
        let mut cancelled = Vec::new();

        let mut server_names: BTreeSet<String> = current.keys().cloned().collect();
        if let Some(expected_servers) = &self.mcp_startup_expected_servers {
            server_names.extend(expected_servers.iter().cloned());
        }

        for name in server_names {
            match current.get(&name) {
                Some(McpStartupStatus::Ready) => {}
                Some(McpStartupStatus::Failed { .. }) => failed.push(name),
                Some(McpStartupStatus::Cancelled | McpStartupStatus::Starting) | None => {
                    cancelled.push(name);
                }
            }
        }

        failed.sort();
        failed.dedup();
        cancelled.sort();
        cancelled.dedup();
        self.finish_mcp_startup(failed, cancelled);
    }

    #[cfg(test)]
    pub(super) fn on_mcp_startup_complete(&mut self, ev: McpStartupCompleteEvent) {
        let failed = ev.failed.into_iter().map(|f| f.server).collect();
        self.finish_mcp_startup(failed, ev.cancelled);
    }

    pub(super) fn on_mcp_server_status_updated(
        &mut self,
        notification: McpServerStatusUpdatedNotification,
    ) {
        let status = match notification.status {
            McpServerStartupState::Starting => McpStartupStatus::Starting,
            McpServerStartupState::Ready => McpStartupStatus::Ready,
            McpServerStartupState::Failed => McpStartupStatus::Failed {
                error: notification.error.unwrap_or_else(|| {
                    format!("MCP client for `{}` failed to start", notification.name)
                }),
            },
            McpServerStartupState::Cancelled => McpStartupStatus::Cancelled,
        };
        self.update_mcp_startup_status(
            notification.name,
            status,
            /*complete_when_settled*/ true,
        );
    }
}
