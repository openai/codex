//! Startup diagnostics retained in the transcript and counted in the warning footer.
//! Affected sources are unique; sign-in servers are a subset of MCP servers.

use super::*;
use codex_app_server_protocol::McpServerStartupFailureReason;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default)]
pub(crate) struct StartupWarningsCell {
    pub(crate) messages: Vec<String>,
    pub(crate) other_sources: BTreeSet<String>,
    pub(crate) mcp_servers: BTreeSet<String>,
    pub(crate) mcp_details: BTreeMap<String, Vec<String>>,
    pub(crate) sign_in_servers: BTreeSet<String>,
    /// Whether diagnostics arrived before a session was attached.
    pub(crate) pending_header: bool,
}

impl StartupWarningsCell {
    pub(crate) fn new(messages: Vec<String>) -> Self {
        Self {
            other_sources: messages.iter().cloned().collect(),
            messages,
            ..Self::default()
        }
    }

    pub(crate) fn mcp(
        messages: Vec<String>,
        servers: impl IntoIterator<Item = String>,
        failure_reason: Option<McpServerStartupFailureReason>,
    ) -> Self {
        let mcp_servers: BTreeSet<_> = servers.into_iter().collect();
        let sign_in_servers = match failure_reason {
            Some(McpServerStartupFailureReason::ReauthenticationRequired) => mcp_servers.clone(),
            None => BTreeSet::new(),
        };
        Self {
            mcp_details: mcp_servers
                .iter()
                .map(|server| (server.clone(), messages.clone()))
                .collect(),
            messages,
            mcp_servers,
            sign_in_servers,
            ..Self::default()
        }
    }
}

impl HistoryCell for StartupWarningsCell {
    fn warning_entries(&self) -> Vec<WarningEntry> {
        self.other_sources
            .iter()
            .map(|message| WarningEntry {
                id: WarningId::Message(message.clone()),
                source: "Startup".into(),
                details: message.clone(),
            })
            .chain(self.mcp_servers.iter().map(|server| WarningEntry {
                id: WarningId::McpServer(server.clone()),
                source: format!("MCP · {server}"),
                details: {
                    let mut details = self.mcp_details.get(server).map_or_else(
                        || format!("MCP startup incomplete: {server}"),
                        |messages| messages.join("\n\n"),
                    );
                    if self.sign_in_servers.contains(server) {
                        details.push_str("\n\nSign-in required.");
                    }
                    details
                },
            }))
            .collect()
    }

    fn live_raw_lines(&self) -> Vec<Line<'static>> {
        Vec::new()
    }

    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        Vec::new()
    }

    fn warning_keys(&self) -> Vec<WarningKey<'_>> {
        self.other_sources
            .iter()
            .map(|message| WarningKey::Message(message))
            .chain(
                self.mcp_servers
                    .iter()
                    .map(|server| WarningKey::McpServer(server)),
            )
            .collect()
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.messages
            .iter()
            .flat_map(|message| new_warning_event(message.clone()).transcript_lines(width))
            .collect()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.messages
            .iter()
            .flat_map(|message| raw_lines_from_source(message))
            .collect()
    }
}
