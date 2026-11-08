//! MCP startup state and status handling for the chat widget.
//!
//! The app server reports MCP server startup as per-server status updates. This
//! module keeps the TUI's buffered startup round state coherent and translates
//! those updates into status headers, warnings, and queued-input release points.
//! Each active round includes every outcome it receives, even if the client has
//! already shown the same diagnostic in an earlier round.
//! Initial and runtime diagnostics remain in retained history and the warnings viewer;
//! compact history hides their full text behind the persistent warning notice.

use std::collections::HashMap;
use std::collections::HashSet;

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

pub(super) enum ExpectedMcpServers {
    EveryRound(HashSet<String>),
    InitialRound(HashSet<String>),
    /// A subscriber can miss the starts. A repeated diagnostic can still belong to a new round.
    ReportedOnly {
        last_diagnostics: HashMap<String, McpStartupStatus>,
    },
}

impl ExpectedMcpServers {
    fn iter(&self) -> impl Iterator<Item = &String> {
        match self {
            Self::EveryRound(names) | Self::InitialRound(names) => Some(names),
            Self::ReportedOnly { .. } => None,
        }
        .into_iter()
        .flatten()
    }
}

impl ChatWidget {
    /// Record a startup update for the displayed thread. A configured round buffers
    /// possible starts after it settles until there is enough evidence of a new round.
    /// A subscriber that tracks reported servers can also receive a standalone outcome.
    fn update_mcp_startup_status(&mut self, server: String, status: McpStartupStatus) {
        let mut report_diagnostic = true;
        if let Some(ExpectedMcpServers::ReportedOnly { last_diagnostics }) =
            &mut self.mcp_startup_expected_servers
        {
            report_diagnostic = if status.is_diagnostic() {
                last_diagnostics
                    .insert(server.clone(), status.clone())
                    .as_ref()
                    != Some(&status)
            } else {
                last_diagnostics.remove(&server);
                false
            };
            // An idle client can receive a terminal update without ever seeing its start.
            if self.mcp_startup_status.is_none() && !matches!(status, McpStartupStatus::Starting) {
                if report_diagnostic {
                    self.report_mcp_startup_failure(&server, &status);
                    self.report_mcp_startup_summary([(&server, &status)]);
                }
                return;
            }
        }
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
            let saw_full_round = expected_servers
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
            if report_diagnostic && startup_status.get(&server) != Some(&status) {
                self.report_mcp_startup_failure(&server, &status);
            }
            startup_status.insert(server, status);
            startup_status
        };
        if activated_pending_round {
            // A promoted buffered round may already contain terminal failures.
            for (server, state) in &startup_status {
                self.report_mcp_startup_failure(server, state);
            }
        }
        self.mcp_startup_status = Some(startup_status);
        self.update_task_running_state();

        // App-server-backed startup completes when every expected server has
        // reported a non-Starting status. Lag handling can force an earlier
        // settle via `finish_mcp_startup_after_lag()`.
        if let Some(current) = &self.mcp_startup_status
            && let Some(expected_servers) = &self.mcp_startup_expected_servers
            && !current.is_empty()
            && expected_servers
                .iter()
                .all(|name| current.contains_key(name))
            && current
                .values()
                .all(|state| !matches!(state, McpStartupStatus::Starting))
        {
            self.finish_mcp_startup();
            return;
        }
        if let Some(header) = self.mcp_startup_status_header() {
            self.set_status_header(header);
        }
        self.request_redraw();
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

    pub(crate) fn set_mcp_startup_expected_servers<I>(&mut self, server_names: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.mcp_startup_expected_servers = Some(ExpectedMcpServers::EveryRound(
            server_names.into_iter().collect(),
        ));
    }

    pub(crate) fn track_reported_mcp_startup_servers(&mut self) {
        if !matches!(
            self.mcp_startup_expected_servers,
            Some(ExpectedMcpServers::ReportedOnly { .. })
        ) {
            self.mcp_startup_expected_servers = Some(ExpectedMcpServers::ReportedOnly {
                last_diagnostics: HashMap::new(),
            });
        }
    }

    /// After the initial batch a dormant helper can start a round on its own.
    pub(crate) fn set_initial_mcp_startup_expected_servers(
        &mut self,
        names: impl Iterator<Item = String>,
    ) {
        if !matches!(
            self.mcp_startup_expected_servers,
            Some(ExpectedMcpServers::ReportedOnly { .. })
        ) {
            self.mcp_startup_expected_servers =
                Some(ExpectedMcpServers::InitialRound(names.collect()));
        }
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
        let mut settled = self.mcp_startup_status.take().unwrap_or_default();
        self.report_mcp_startup_summary(&settled);
        if matches!(
            self.mcp_startup_expected_servers,
            Some(ExpectedMcpServers::InitialRound(_))
        ) {
            settled.retain(|_, status| status.is_diagnostic());
            self.mcp_startup_expected_servers = Some(ExpectedMcpServers::ReportedOnly {
                last_diagnostics: settled,
            });
        }
        self.mcp_startup_ignore_updates_until_next_start = true;
        self.mcp_startup_allow_terminal_only_next_round = false;
        self.mcp_startup_pending_next_round.clear();
        self.mcp_startup_pending_next_round_saw_starting = false;
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
        if let Some(ExpectedMcpServers::ReportedOnly { last_diagnostics }) =
            &mut self.mcp_startup_expected_servers
        {
            // A missed Starting may have begun a new attempt with the same error.
            last_diagnostics.clear();
        }
        if self.mcp_startup_ignore_updates_until_next_start {
            if self.mcp_startup_pending_next_round.is_empty() {
                self.mcp_startup_pending_next_round_saw_starting = false;
            }
            self.mcp_startup_allow_terminal_only_next_round = true;
        }
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
