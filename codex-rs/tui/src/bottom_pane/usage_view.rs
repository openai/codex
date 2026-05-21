use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::render::renderable::Renderable;
use codex_app_server_protocol::UsageEntry;
use codex_app_server_protocol::UsageRange;
use codex_app_server_protocol::UsageReadResponse;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

pub(crate) const USAGE_VIEW_ID: &str = "usage";

pub(crate) struct UsageView {
    lines: Vec<Line<'static>>,
    range: UsageRange,
    app_event_tx: AppEventSender,
}

impl UsageView {
    pub(crate) fn loading(range: UsageRange, app_event_tx: AppEventSender) -> Self {
        Self {
            lines: vec![Line::from("Usage".bold()), Line::from("Loading...".dim())],
            range,
            app_event_tx,
        }
    }

    pub(crate) fn loaded(response: UsageReadResponse, app_event_tx: AppEventSender) -> Self {
        let range = response.report.range;
        let report = response.report;
        let mut lines = vec![Line::from("Usage".bold())];
        if let Some(headline) = report.headline.as_ref() {
            lines.push(Line::from(format!(
                "{}% of your usage came from {} \"{}\"",
                headline.entry.percent_of_usage,
                contributor_kind_label(headline.entry.kind),
                headline.entry.label
            )));
            if let Some(note) = headline.note.as_ref() {
                lines.push(Line::from(note.clone().dim()));
            }
            lines.push(Line::from(""));
        }
        if report.total_tokens == 0 {
            lines.push(Line::from("No tracked usage in this range yet.".dim()));
        } else {
            push_section(&mut lines, "Skills", &report.skills);
            push_section(&mut lines, "Subagents", &report.subagents);
            push_section(&mut lines, "Apps", &report.apps);
            push_section(&mut lines, "MCP servers", &report.mcp_servers);
            push_section(&mut lines, "Plugins", &report.plugins);
        }
        if lines.last().is_none_or(|line| line.width() != 0) {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(range_footer(range).dim()));
        Self {
            lines,
            range,
            app_event_tx,
        }
    }

    pub(crate) fn error(range: UsageRange, err: String, app_event_tx: AppEventSender) -> Self {
        Self {
            lines: vec![
                Line::from("Usage".bold()),
                Line::from(format!("Failed to load usage: {err}").red()),
                Line::from(""),
                Line::from(range_footer(range).dim()),
            ],
            range,
            app_event_tx,
        }
    }
}

impl BottomPaneView for UsageView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }
        match key_event.code {
            KeyCode::Char('d') if self.range != UsageRange::Day => {
                self.app_event_tx.send(AppEvent::OpenUsage {
                    range: UsageRange::Day,
                });
            }
            KeyCode::Char('w') if self.range != UsageRange::Week => {
                self.app_event_tx.send(AppEvent::OpenUsage {
                    range: UsageRange::Week,
                });
            }
            _ => {}
        }
    }

    fn view_id(&self) -> Option<&'static str> {
        Some(USAGE_VIEW_ID)
    }
}

impl Renderable for UsageView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines.clone()).render_ref(area, buf);
    }

    fn desired_height(&self, _width: u16) -> u16 {
        u16::try_from(self.lines.len()).unwrap_or(u16::MAX)
    }
}

fn push_section(lines: &mut Vec<Line<'static>>, label: &str, entries: &[UsageEntry]) {
    if entries.is_empty() {
        return;
    }
    lines.push(Line::from(label.to_string().bold()));
    for entry in entries {
        lines.push(Line::from(format!(
            "{:<24} {:>3}%",
            entry.label, entry.percent_of_usage
        )));
    }
    lines.push(Line::from(""));
}

fn contributor_kind_label(kind: codex_app_server_protocol::UsageContributorKind) -> &'static str {
    match kind {
        codex_app_server_protocol::UsageContributorKind::Skill => "skill",
        codex_app_server_protocol::UsageContributorKind::Subagent => "subagent",
        codex_app_server_protocol::UsageContributorKind::App => "app",
        codex_app_server_protocol::UsageContributorKind::McpServer => "MCP server",
        codex_app_server_protocol::UsageContributorKind::Plugin => "plugin",
    }
}

fn range_footer(_range: UsageRange) -> &'static str {
    "d to day | w to week"
}
