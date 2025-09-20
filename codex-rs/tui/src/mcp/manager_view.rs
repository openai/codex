use std::cell::Cell;

use codex_core::mcp::health::HealthReport;
use codex_core::mcp::health::HealthStatus;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Row;
use ratatui::widgets::StatefulWidget;
use ratatui::widgets::Table;
use ratatui::widgets::TableState;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::ScrollState;
use crate::mcp::types::McpServerSnapshot;
use crate::mcp::types::McpWizardDraft;

#[derive(Clone)]
pub(crate) struct McpManagerEntry {
    pub snapshot: McpServerSnapshot,
    pub health: HealthReport,
}

pub(crate) struct McpManagerInit {
    pub app_event_tx: AppEventSender,
    pub entries: Vec<McpManagerEntry>,
    pub template_count: usize,
}

pub(crate) struct McpManagerView {
    app_event_tx: AppEventSender,
    entries: Vec<McpManagerEntry>,
    template_count: usize,
    scroll: ScrollState,
    list_visible_rows: Cell<usize>,
    close_requested: bool,
    confirm_delete: bool,
}

impl McpManagerView {
    pub(crate) fn new(init: McpManagerInit) -> Self {
        let mut scroll = ScrollState::new();
        scroll.clamp_selection(init.entries.len());
        Self {
            app_event_tx: init.app_event_tx,
            entries: init.entries,
            template_count: init.template_count,
            scroll,
            list_visible_rows: Cell::new(1),
            close_requested: false,
            confirm_delete: false,
        }
    }

    fn selected_entry(&self) -> Option<&McpManagerEntry> {
        self.scroll
            .selected_idx
            .and_then(|idx| self.entries.get(idx))
    }

    fn visible_rows(&self) -> usize {
        self.list_visible_rows.get().max(1)
    }

    fn move_selection_up(&mut self) {
        self.scroll.move_up_wrap(self.entries.len());
        self.scroll
            .ensure_visible(self.entries.len(), self.visible_rows());
        self.confirm_delete = false;
    }

    fn move_selection_down(&mut self) {
        self.scroll.move_down_wrap(self.entries.len());
        self.scroll
            .ensure_visible(self.entries.len(), self.visible_rows());
        self.confirm_delete = false;
    }

    fn page_up(&mut self) {
        let steps = self.visible_rows().saturating_sub(1);
        for _ in 0..steps {
            self.scroll.move_up_wrap(self.entries.len());
        }
        self.scroll
            .ensure_visible(self.entries.len(), self.visible_rows());
        self.confirm_delete = false;
    }

    fn page_down(&mut self) {
        let steps = self.visible_rows().saturating_sub(1);
        for _ in 0..steps {
            self.scroll.move_down_wrap(self.entries.len());
        }
        self.scroll
            .ensure_visible(self.entries.len(), self.visible_rows());
        self.confirm_delete = false;
    }

    fn open_new_wizard(&self) {
        self.app_event_tx.send(AppEvent::OpenMcpWizard {
            template_id: None,
            draft: None,
            existing_name: None,
        });
    }

    fn open_edit_wizard(&self) {
        if let Some(entry) = self.selected_entry() {
            let config = entry.snapshot.to_config();
            let draft = McpWizardDraft::from_existing(entry.snapshot.name.clone(), &config);
            self.app_event_tx.send(AppEvent::OpenMcpWizard {
                template_id: entry.snapshot.template_id.clone(),
                draft: Some(draft),
                existing_name: Some(entry.snapshot.name.clone()),
            });
        }
    }

    fn request_remove_selected(&mut self) {
        if self.selected_entry().is_none() {
            return;
        }
        if self.confirm_delete {
            if let Some(entry) = self.selected_entry() {
                self.app_event_tx.send(AppEvent::RemoveMcpServer {
                    name: entry.snapshot.name.clone(),
                });
            }
            self.confirm_delete = false;
        } else {
            self.confirm_delete = true;
        }
    }

    fn refresh(&self) {
        self.app_event_tx.send(AppEvent::ReloadMcpServers);
    }

    fn render_list(&self, area: Rect, buf: &mut Buffer) {
        let header = Row::new(vec!["Name".bold(), "Status".bold(), "Command".bold()]);

        let rows: Vec<Row> = self
            .entries
            .iter()
            .map(|entry| {
                Row::new(vec![
                    Span::raw(entry.snapshot.name.clone()),
                    health_span(&entry.health.status),
                    Span::raw(entry.snapshot.command.clone()),
                ])
            })
            .collect();

        let mut state = TableState::default();
        state.select(self.scroll.selected_idx);
        *state.offset_mut() = self.scroll.scroll_top;

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(28),
                Constraint::Length(10),
                Constraint::Percentage(62),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Configured MCP Servers"),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        StatefulWidget::render(table, area, buf, &mut state);

        let visible_rows = area.height.saturating_sub(3) as usize; // header + borders
        self.list_visible_rows.set(visible_rows.max(1));
    }

    fn render_detail(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = Vec::new();
        if let Some(entry) = self.selected_entry() {
            lines.extend(detail_lines(&entry.snapshot));
            lines.push(Line::from(vec![
                "Status: ".dim(),
                health_span(&entry.health.status),
            ]));
            if let Some(notes) = entry.health.notes.as_ref() {
                lines.push(Line::from(vec!["  • ".into(), notes.clone().dim()]));
            }
            if self.confirm_delete {
                lines.push(Line::from(""));
                lines.push("Press 'd' again to confirm deletion".red().into());
            }
        } else if self.entries.is_empty() {
            lines.push("No MCP servers configured.".into());
            if self.template_count > 0 {
                lines.push(
                    format!(
                        "Press 'n' to create one using {} available template(s).",
                        self.template_count
                    )
                    .dim()
                    .into(),
                );
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            "Keys: ".dim(),
            "↑/↓".cyan(),
            " move  ".dim(),
            "n".cyan(),
            " new  ".dim(),
            "Enter".cyan(),
            " edit  ".dim(),
            "r".cyan(),
            " reload  ".dim(),
            "d".cyan(),
            " delete  ".dim(),
            "Esc".cyan(),
            " close".dim(),
        ]));

        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Details"))
            .wrap(Wrap { trim: true });
        Widget::render(paragraph, area, buf);
    }
}

impl BottomPaneView for McpManagerView {
    fn handle_key_event(&mut self, _pane: &mut BottomPane, key_event: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        self.confirm_delete &= matches!(key_event.code, KeyCode::Char('d'));

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.close_requested = true;
            }
            KeyCode::Char('n') => self.open_new_wizard(),
            KeyCode::Enter | KeyCode::Char('e') => self.open_edit_wizard(),
            KeyCode::Char('r') => self.refresh(),
            KeyCode::Char('d') => self.request_remove_selected(),
            KeyCode::Up => self.move_selection_up(),
            KeyCode::Down => self.move_selection_down(),
            KeyCode::PageUp => self.page_up(),
            KeyCode::PageDown => self.page_down(),
            _ => {
                if !matches!(key_event.code, KeyCode::Char('d')) {
                    self.confirm_delete = false;
                }
            }
        }
    }

    fn is_complete(&self) -> bool {
        self.close_requested
    }

    fn on_ctrl_c(&mut self, _pane: &mut BottomPane) -> CancellationEvent {
        self.close_requested = true;
        CancellationEvent::Handled
    }

    fn desired_height(&self, _width: u16) -> u16 {
        16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let layout =
            Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).split(area);
        self.render_list(layout[0], buf);
        self.render_detail(layout[1], buf);
    }
}

fn health_span(status: &HealthStatus) -> Span<'static> {
    match status {
        HealthStatus::Unknown => "unknown".dim(),
        HealthStatus::Passing => "passing".green(),
        HealthStatus::Failing => "failing".red(),
    }
}

fn detail_lines(snapshot: &McpServerSnapshot) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        "Name: ".dim(),
        snapshot.name.clone().into(),
        "  Command: ".dim(),
        snapshot.command.clone().into(),
    ]));

    if !snapshot.args.is_empty() {
        lines.push(Line::from(vec![
            "Args: ".dim(),
            snapshot.args.join(" ").into(),
        ]));
    }

    if let Some(desc) = snapshot.description.as_ref() {
        lines.push(Line::from(vec!["Description: ".dim(), desc.clone().into()]));
    }

    if !snapshot.tags.is_empty() {
        lines.push(Line::from(vec![
            "Tags: ".dim(),
            snapshot.tags.join(", ").into(),
        ]));
    }

    if let Some(template_id) = snapshot.template_id.as_ref() {
        lines.push(Line::from(vec![
            "Template: ".dim(),
            template_id.clone().into(),
        ]));
    }

    if let Some(timeout) = snapshot.startup_timeout_ms {
        lines.push(Line::from(vec![
            "Startup timeout (ms): ".dim(),
            timeout.to_string().into(),
        ]));
    }

    if !snapshot.env.is_empty() {
        lines.push("Env:".dim().into());
        for (idx, (k, v)) in snapshot.env.iter().enumerate() {
            if idx >= 6 {
                lines.push(
                    format!("  … {} more", snapshot.env.len() - idx)
                        .dim()
                        .into(),
                );
                break;
            }
            lines.push(Line::from(vec!["  • ".into(), format!("{k}={v}").into()]));
        }
    }

    if let Some(auth) = snapshot.auth.as_ref() {
        lines.extend(auth_lines(auth));
    }

    if let Some(health) = snapshot.health.as_ref() {
        lines.extend(health_lines(health));
    }

    lines
}

fn auth_lines(auth: &crate::mcp::types::AuthDraft) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push("Auth:".dim().into());
    if let Some(kind) = auth.kind.as_ref() {
        lines.push(Line::from(vec!["  • Type: ".into(), kind.clone().into()]));
    }
    if let Some(secret) = auth.secret_ref.as_ref() {
        lines.push(Line::from(vec![
            "  • Secret: ".into(),
            secret.clone().into(),
        ]));
    }
    if !auth.env.is_empty() {
        for (idx, (k, v)) in auth.env.iter().enumerate() {
            if idx >= 4 {
                lines.push(format!("  … {} more", auth.env.len() - idx).dim().into());
                break;
            }
            lines.push(Line::from(vec![
                "     - ".into(),
                format!("{k}={v}").into(),
            ]));
        }
    }
    lines
}

fn health_lines(health: &crate::mcp::types::HealthDraft) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push("Health:".dim().into());
    if let Some(kind) = health.kind.as_ref() {
        lines.push(Line::from(vec!["  • Type: ".into(), kind.clone().into()]));
    }
    if let Some(cmd) = health.command.as_ref() {
        lines.push(Line::from(vec!["  • Command: ".into(), cmd.clone().into()]));
    }
    if !health.args.is_empty() {
        lines.push(Line::from(vec![
            "  • Args: ".into(),
            health.args.join(" ").into(),
        ]));
    }
    if let Some(endpoint) = health.endpoint.as_ref() {
        lines.push(Line::from(vec![
            "  • Endpoint: ".into(),
            endpoint.clone().into(),
        ]));
    }
    if let Some(timeout) = health.timeout_ms {
        lines.push(Line::from(vec![
            "  • Timeout (ms): ".into(),
            timeout.to_string().into(),
        ]));
    }
    if let Some(interval) = health.interval_seconds {
        lines.push(Line::from(vec![
            "  • Interval (s): ".into(),
            interval.to_string().into(),
        ]));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::AuthDraft;
    use crate::mcp::types::HealthDraft;
    use insta::assert_snapshot;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::collections::BTreeMap;
    use tokio::sync::mpsc::unbounded_channel;

    fn render_view(view: &McpManagerView) -> String {
        let width = 72;
        let height = BottomPaneView::desired_height(view, width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        BottomPaneView::render(view, area, &mut buf);

        (0..area.height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..area.width {
                    let symbol = buf[(area.x + col, area.y + row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn sample_entry(name: &str, command: &str, status: HealthStatus) -> McpManagerEntry {
        let mut env = BTreeMap::new();
        env.insert("TOKEN".to_string(), "***".to_string());

        let snapshot = McpServerSnapshot {
            name: name.to_string(),
            command: command.to_string(),
            args: vec!["--serve".to_string()],
            env,
            description: Some("Sample server".to_string()),
            tags: vec!["beta".to_string(), "internal".to_string()],
            template_id: Some("sample-template".to_string()),
            auth: Some(AuthDraft {
                kind: Some("env".to_string()),
                secret_ref: Some("vault:secret".to_string()),
                env: BTreeMap::from([("AUTH_TOKEN".to_string(), "secret".to_string())]),
            }),
            health: Some(HealthDraft {
                kind: Some("stdio".to_string()),
                command: Some("health-check".to_string()),
                args: vec!["--ping".to_string()],
                timeout_ms: Some(5_000),
                interval_seconds: Some(60),
                endpoint: None,
                protocol: None,
            }),
            display_name: Some(format!("{name} MCP")),
            category: Some("general".to_string()),
            metadata: None,
            startup_timeout_ms: Some(15_000),
        };

        McpManagerEntry {
            snapshot,
            health: HealthReport::new(status).with_notes("synthetic status for test rendering"),
        }
    }

    fn make_view(entries: Vec<McpManagerEntry>, template_count: usize) -> McpManagerView {
        let (tx, _rx) = unbounded_channel();
        McpManagerView::new(McpManagerInit {
            app_event_tx: AppEventSender::new(tx),
            entries,
            template_count,
        })
    }

    #[test]
    fn renders_manager_with_entries() {
        let view = make_view(
            vec![
                sample_entry("anthropic", "anthropic-mcp", HealthStatus::Unknown),
                sample_entry("openai", "openai-mcp", HealthStatus::Passing),
            ],
            3,
        );

        assert_snapshot!("mcp_manager_with_entries", render_view(&view));
    }

    #[test]
    fn renders_delete_confirmation() {
        let mut view = make_view(
            vec![sample_entry(
                "perplexity",
                "perplexity-mcp",
                HealthStatus::Failing,
            )],
            1,
        );
        view.scroll.selected_idx = Some(0);
        view.confirm_delete = true;

        assert_snapshot!("mcp_manager_delete_confirm", render_view(&view));
    }

    #[test]
    fn renders_empty_state_with_template_hint() {
        let view = make_view(Vec::new(), 5);

        assert_snapshot!("mcp_manager_empty", render_view(&view));
    }
}
