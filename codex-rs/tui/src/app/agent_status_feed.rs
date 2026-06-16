//! Bounded, best-effort previews for the v2 `/agent` status output.

use super::ThreadBufferedEvent;
use super::ThreadEventStore;
use crate::history_cell::HistoryCell;
use crate::history_cell::plain_lines;
use crate::text_formatting::truncate_text;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SubAgentActivityKind;
use codex_app_server_protocol::ThreadItem;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::collections::HashSet;

const AGENT_STATUS_PREVIEW_LINES: usize = 3;
const AGENT_STATUS_PREVIEW_ITEMS: usize = 6;
const AGENT_STATUS_PREVIEW_GRAPHEMES: usize = 240;
const AGENT_STATUS_PREVIEW_INDENT: u16 = 4;

#[derive(Debug)]
pub(super) struct AgentStatusHistoryCell {
    entries: Vec<AgentStatusThreadPreview>,
}

impl AgentStatusHistoryCell {
    pub(super) fn new(entries: Vec<AgentStatusThreadPreview>) -> Self {
        Self { entries }
    }
}

impl HistoryCell for AgentStatusHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = vec![
            "/agent".magenta().into(),
            "Sub-agents running".bold().into(),
            "".into(),
        ];

        if self.entries.is_empty() {
            lines.push("  • No sub-agents running.".italic().into());
            return lines;
        }

        for entry in &self.entries {
            lines.push(entry.title_line());
            let preview_width = width.saturating_sub(AGENT_STATUS_PREVIEW_INDENT).max(1);
            let preview_lines = entry.preview_lines(preview_width);
            if preview_lines.is_empty() {
                lines.push(vec!["    ".into(), "No recent activity yet.".dim().italic()].into());
            } else {
                lines.extend(preview_lines.into_iter().map(indent_preview_line));
            }
            lines.push("".into());
        }
        let _ = lines.pop();
        lines
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        plain_lines(self.display_lines(u16::MAX))
    }
}

#[derive(Debug)]
pub(super) struct AgentStatusThreadPreview {
    agent_path: String,
    activity: Vec<String>,
}

impl AgentStatusThreadPreview {
    pub(super) fn from_store(agent_path: String, store: &ThreadEventStore) -> Self {
        Self::from_events(agent_path, store.buffer.iter().rev())
    }

    pub(super) fn empty(agent_path: String) -> Self {
        Self::from_events(agent_path, std::iter::empty())
    }

    fn from_events<'a>(
        agent_path: String,
        events: impl Iterator<Item = &'a ThreadBufferedEvent>,
    ) -> Self {
        let mut seen_item_ids = HashSet::new();
        let mut activity = Vec::new();
        for event in events {
            let item = match event {
                ThreadBufferedEvent::Notification(ServerNotification::ItemCompleted(event)) => {
                    &event.item
                }
                ThreadBufferedEvent::Notification(ServerNotification::ItemStarted(event)) => {
                    &event.item
                }
                ThreadBufferedEvent::Notification(_)
                | ThreadBufferedEvent::Request(_)
                | ThreadBufferedEvent::HistoryEntryResponse(_)
                | ThreadBufferedEvent::FeedbackSubmission(_) => continue,
            };
            if !seen_item_ids.insert(item.id().to_string()) {
                continue;
            }
            if let Some(summary) = activity_summary(item) {
                activity.push(summary);
                if activity.len() == AGENT_STATUS_PREVIEW_ITEMS {
                    break;
                }
            }
        }
        activity.reverse();
        Self {
            agent_path,
            activity,
        }
    }

    fn title_line(&self) -> Line<'static> {
        vec!["  • ".dim(), format!("`{}`", self.agent_path).cyan()].into()
    }

    fn preview_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = self
            .activity
            .iter()
            .flat_map(|activity| textwrap::wrap(activity, width as usize))
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.into_owned().dim().into())
            .collect::<Vec<_>>();
        if lines.len() > AGENT_STATUS_PREVIEW_LINES {
            lines.drain(..lines.len() - AGENT_STATUS_PREVIEW_LINES);
        }
        lines
    }
}

fn activity_summary(item: &ThreadItem) -> Option<String> {
    let summary = match item {
        ThreadItem::AgentMessage { text, .. } | ThreadItem::Plan { text, .. } => text,
        ThreadItem::InterAgentCommunication {
            sender,
            content,
            encrypted,
            ..
        } => {
            if *encrypted {
                return bounded_summary(&format!("Encrypted message from {sender}"));
            }
            return bounded_summary(&format!("Message from {sender}: {content}"));
        }
        ThreadItem::Reasoning { summary, .. } => summary.last()?,
        ThreadItem::CommandExecution { command, .. } => {
            let command = truncate_text(
                command,
                AGENT_STATUS_PREVIEW_GRAPHEMES.saturating_sub("$ ".len()),
            );
            return bounded_summary(&format!("$ {command}"));
        }
        ThreadItem::FileChange { changes, .. } => {
            return bounded_summary(&format!("Updated {} file(s)", changes.len()));
        }
        ThreadItem::McpToolCall { server, tool, .. } => {
            return bounded_summary(&format!("MCP {server}/{tool}"));
        }
        ThreadItem::DynamicToolCall {
            namespace, tool, ..
        } => {
            let tool = namespace
                .as_ref()
                .map(|namespace| format!("{namespace}/{tool}"))
                .unwrap_or_else(|| tool.clone());
            return bounded_summary(&format!("Tool {tool}"));
        }
        ThreadItem::CollabAgentToolCall { tool, .. } => {
            let action = match tool {
                CollabAgentTool::SpawnAgent => "Spawned an agent",
                CollabAgentTool::SendInput => "Sent input to an agent",
                CollabAgentTool::ResumeAgent => "Resumed an agent",
                CollabAgentTool::Wait => "Waited for an agent",
                CollabAgentTool::CloseAgent => "Closed an agent",
            };
            return Some(action.to_string());
        }
        ThreadItem::SubAgentActivity {
            kind, agent_path, ..
        } => {
            let action = match kind {
                SubAgentActivityKind::Started => "Started",
                SubAgentActivityKind::Interacted => "Contacted",
                SubAgentActivityKind::Interrupted => "Interrupted",
            };
            return bounded_summary(&format!("{action} {agent_path}"));
        }
        ThreadItem::WebSearch { query, .. } => {
            return bounded_summary(&format!("Web search: {query}"));
        }
        ThreadItem::ImageView { path, .. } => {
            return bounded_summary(&format!("Viewed {}", path.display()));
        }
        ThreadItem::ImageGeneration { .. } => return Some("Generated an image".to_string()),
        ThreadItem::EnteredReviewMode { .. } => return Some("Entered review mode".to_string()),
        ThreadItem::ExitedReviewMode { .. } => return Some("Exited review mode".to_string()),
        ThreadItem::ContextCompaction { .. } => return Some("Compacted context".to_string()),
        ThreadItem::UserMessage { .. }
        | ThreadItem::HookPrompt { .. }
        | ThreadItem::Sleep { .. } => return None,
    };
    bounded_summary(summary)
}

fn bounded_summary(summary: &str) -> Option<String> {
    let summary = truncate_text(summary, AGENT_STATUS_PREVIEW_GRAPHEMES);
    let summary = summary.split_whitespace().collect::<Vec<_>>().join(" ");
    (!summary.is_empty()).then_some(summary)
}

fn indent_preview_line(mut line: Line<'static>) -> Line<'static> {
    line.spans.insert(0, "    ".into());
    line
}

#[cfg(test)]
#[path = "agent_status_feed_tests.rs"]
mod tests;
