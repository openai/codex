use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::{
    agent_editor_view::AgentEditorView,
    agents_settings_view::SubagentEditorView,
    AutoDriveSettingsView,
    BottomPaneView,
    settings_panel::{render_panel, PanelFrameStyle},
    GithubSettingsView,
    McpSettingsView,
    ModelSelectionView,
    NotificationsSettingsView,
    SettingsSection,
    ThemeSelectionView,
    UpdateSettingsView,
    ValidationSettingsView,
};
use crate::chrome_launch::{ChromeLaunchOption, CHROME_LAUNCH_CHOICES};
use super::limits_overlay::{LimitsOverlay, LimitsOverlayContent};
use crate::live_wrap::take_prefix_by_width;
use crate::util::buffer::fill_rect;

const LABEL_COLUMN_WIDTH: usize = 18;

pub(crate) trait SettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    fn is_complete(&self) -> bool;
    fn on_close(&mut self) {}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MenuState {
    selected: SettingsSection,
}

impl MenuState {
    fn new(selected: SettingsSection) -> Self {
        Self { selected }
    }

    fn selected(&self) -> SettingsSection {
        self.selected
    }

    fn set_selected(&mut self, section: SettingsSection) {
        self.selected = section;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SectionState {
    active: SettingsSection,
}

impl SectionState {
    fn new(active: SettingsSection) -> Self {
        Self { active }
    }

    fn active(&self) -> SettingsSection {
        self.active
    }

    fn set_active(&mut self, section: SettingsSection) {
        self.active = section;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsOverlayMode {
    Menu(MenuState),
    Section(SectionState),
}

#[derive(Clone, Debug)]
pub(crate) struct SettingsOverviewRow {
    pub(crate) section: SettingsSection,
    pub(crate) summary: Option<String>,
}

impl SettingsOverviewRow {
    pub(crate) fn new(section: SettingsSection, summary: Option<String>) -> Self {
        Self { section, summary }
    }
}

#[derive(Clone, Debug)]
struct SettingsHelpOverlay {
    lines: Vec<Line<'static>>,
}

impl SettingsHelpOverlay {
    fn overview() -> Self {
        let title = Style::default()
            .fg(crate::colors::text())
            .add_modifier(Modifier::BOLD);
        let hint = Style::default().fg(crate::colors::text_dim());
        let mut lines = vec![Line::from(vec![Span::styled("Settings Overview", title)]), Line::default()];
        for text in [
            "• ↑/↓  Move between sections",
            "• Enter  Open selected section",
            "• Tab    Jump forward between sections",
            "• Esc    Close settings",
            "• ?      Toggle this help",
        ] {
            lines.push(Line::from(vec![Span::styled(text.to_string(), hint)]));
        }
        lines.push(Line::default());
        lines.push(Line::from(vec![Span::styled(
            "Press Esc to close",
            Style::default().fg(crate::colors::text_dim()),
        )]));
        Self { lines }
    }

    fn section(section: SettingsSection) -> Self {
        let title = Style::default()
            .fg(crate::colors::text())
            .add_modifier(Modifier::BOLD);
        let hint = Style::default().fg(crate::colors::text_dim());
        let mut lines = vec![
            Line::from(vec![Span::styled(
                format!("{} Shortcuts", section.label()),
                title,
            )]),
            Line::default(),
            Line::from(vec![Span::styled("• Esc    Return to overview", hint)]),
            Line::from(vec![Span::styled("• Tab    Cycle sections", hint)]),
            Line::from(vec![Span::styled("• Shift+Tab  Cycle backwards", hint)]),
        ];
        if matches!(section, SettingsSection::Agents | SettingsSection::Mcp) {
            lines.push(Line::from(vec![Span::styled(
                "• Enter  Activate focused action",
                hint,
            )]));
        }
        lines.push(Line::from(vec![Span::styled("• ?      Toggle this help", hint)]));
        lines.push(Line::default());
        lines.push(Line::from(vec![Span::styled(
            "Press Esc to close",
            Style::default().fg(crate::colors::text_dim()),
        )]));
        Self { lines }
    }
}

pub(crate) struct ModelSettingsContent {
    view: ModelSelectionView,
}

impl ModelSettingsContent {
    pub(crate) fn new(view: ModelSelectionView) -> Self {
        Self { view }
    }
}

impl SettingsContent for ModelSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.view.render_without_frame(area, buf);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.view.handle_key_event_direct(key)
    }

    fn is_complete(&self) -> bool {
        self.view.is_complete()
    }
}

pub(crate) struct ThemeSettingsContent {
    view: ThemeSelectionView,
}

impl ThemeSettingsContent {
    pub(crate) fn new(view: ThemeSelectionView) -> Self {
        Self { view }
    }
}

impl SettingsContent for ThemeSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.view.render(area, buf);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.view.handle_key_event_direct(key)
    }

    fn is_complete(&self) -> bool {
        self.view.is_complete()
    }
}

pub(crate) struct NotificationsSettingsContent {
    view: NotificationsSettingsView,
}

impl NotificationsSettingsContent {
    pub(crate) fn new(view: NotificationsSettingsView) -> Self {
        Self { view }
    }
}

impl SettingsContent for NotificationsSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.view.render(area, buf);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.view.handle_key_event_direct(key)
    }

    fn is_complete(&self) -> bool {
        self.view.is_complete()
    }
}

pub(crate) struct UpdatesSettingsContent {
    view: UpdateSettingsView,
}

impl UpdatesSettingsContent {
    pub(crate) fn new(view: UpdateSettingsView) -> Self {
        Self { view }
    }
}

impl SettingsContent for UpdatesSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.view.render_without_frame(area, buf);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.view.handle_key_event_direct(key);
        true
    }

    fn is_complete(&self) -> bool {
        self.view.is_view_complete()
    }
}

pub(crate) struct ValidationSettingsContent {
    view: ValidationSettingsView,
}

impl ValidationSettingsContent {
    pub(crate) fn new(view: ValidationSettingsView) -> Self {
        Self { view }
    }
}

impl SettingsContent for ValidationSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.view.render(area, buf);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.view.handle_key_event_direct(key);
        true
    }

    fn is_complete(&self) -> bool {
        self.view.is_view_complete()
    }
}

pub(crate) struct GithubSettingsContent {
    view: GithubSettingsView,
}

impl GithubSettingsContent {
    pub(crate) fn new(view: GithubSettingsView) -> Self {
        Self { view }
    }
}

impl SettingsContent for GithubSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.view.render(area, buf);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.view.handle_key_event_direct(key);
        true
    }

    fn is_complete(&self) -> bool {
        self.view.is_view_complete()
    }
}

pub(crate) struct AutoDriveSettingsContent {
    view: AutoDriveSettingsView,
}

impl AutoDriveSettingsContent {
    pub(crate) fn new(view: AutoDriveSettingsView) -> Self {
        Self { view }
    }
}

impl SettingsContent for AutoDriveSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.view.render_without_frame(area, buf);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.view.handle_key_event_direct(key);
        true
    }

    fn is_complete(&self) -> bool {
        self.view.is_view_complete()
    }
}

pub(crate) struct McpSettingsContent {
    view: McpSettingsView,
}

impl McpSettingsContent {
    pub(crate) fn new(view: McpSettingsView) -> Self {
        Self { view }
    }
}

impl SettingsContent for McpSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.view.render(area, buf);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.view.handle_key_event_direct(key)
    }

    fn is_complete(&self) -> bool {
        self.view.is_complete()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AgentOverviewRow {
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) installed: bool,
}

#[derive(Default)]
struct AgentsOverviewState {
    rows: Vec<AgentOverviewRow>,
    commands: Vec<String>,
    selected: usize,
}

impl AgentsOverviewState {
    fn total_rows(&self) -> usize {
        self.rows.len().saturating_add(self.commands.len()).saturating_add(1)
    }

    fn clamp_selection(&mut self) {
        let total = self.total_rows();
        if total == 0 {
            self.selected = 0;
        } else if self.selected >= total {
            self.selected = total - 1;
        }
    }
}

enum AgentsPane {
    Overview(AgentsOverviewState),
    Subagent(SubagentEditorView),
    Agent(AgentEditorView),
}

pub(crate) struct AgentsSettingsContent {
    pane: AgentsPane,
    app_event_tx: AppEventSender,
}

impl AgentsSettingsContent {
    pub(crate) fn new_overview(
        rows: Vec<AgentOverviewRow>,
        commands: Vec<String>,
        selected: usize,
        app_event_tx: AppEventSender,
    ) -> Self {
        let mut overview = AgentsOverviewState { rows, commands, selected };
        overview.clamp_selection();
        Self { pane: AgentsPane::Overview(overview), app_event_tx }
    }

    pub(crate) fn set_overview(
        &mut self,
        rows: Vec<AgentOverviewRow>,
        commands: Vec<String>,
        selected: usize,
    ) {
        let mut overview = AgentsOverviewState { rows, commands, selected };
        overview.clamp_selection();
        self.pane = AgentsPane::Overview(overview);
    }

    pub(crate) fn set_editor(&mut self, editor: SubagentEditorView) {
        self.pane = AgentsPane::Subagent(editor);
    }

    pub(crate) fn set_overview_selection(&mut self, selected: usize) {
        if let AgentsPane::Overview(state) = &mut self.pane {
            state.selected = selected;
            state.clamp_selection();
        }
    }

    pub(crate) fn set_agent_editor(&mut self, editor: AgentEditorView) {
        self.pane = AgentsPane::Agent(editor);
    }

    #[cfg_attr(not(any(test, feature = "test-helpers")), allow(dead_code))]
    pub(crate) fn is_agent_editor_active(&self) -> bool {
        matches!(self.pane, AgentsPane::Agent(_))
    }

    fn render_overview(&self, area: Rect, buf: &mut Buffer, state: &AgentsOverviewState) {
        use ratatui::widgets::Paragraph;

        let mut lines: Vec<Line<'static>> = Vec::new();

        lines.push(Line::from(Span::styled(
            "Agents",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        let max_name = state
            .rows
            .iter()
            .map(|row| row.name.chars().count())
            .max()
            .unwrap_or(0);

        for (idx, row) in state.rows.iter().enumerate() {
            let selected = idx == state.selected;
            let status = if !row.enabled {
                ("disabled", crate::colors::error())
            } else if !row.installed {
                ("not installed", crate::colors::warning())
            } else {
                ("enabled", crate::colors::success())
            };

            let mut spans = Vec::new();
            spans.push(Span::styled(
                if selected { "› " } else { "  " },
                if selected {
                    Style::default().fg(crate::colors::primary())
                } else {
                    Style::default()
                },
            ));
            spans.push(Span::styled(
                format!("{:<width$}", row.name, width = max_name),
                if selected {
                    Style::default()
                        .fg(crate::colors::primary())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ));
            spans.push(Span::raw("  "));
            spans.push(Span::styled("•", Style::default().fg(status.1)));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(status.0.to_string(), Style::default().fg(status.1)));

            if selected {
                spans.push(Span::raw("  "));
                let hint = if !row.installed {
                    "Enter to install"
                } else {
                    "Enter to configure"
                };
                spans.push(Span::styled(hint, Style::default().fg(crate::colors::text_dim())));
            }

            lines.push(Line::from(spans));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Commands",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        for (offset, cmd) in state.commands.iter().enumerate() {
            let idx = state.rows.len() + offset;
            let selected = idx == state.selected;
            let mut spans = Vec::new();
            spans.push(Span::styled(
                if selected { "› " } else { "  " },
                if selected {
                    Style::default().fg(crate::colors::primary())
                } else {
                    Style::default()
                },
            ));
            spans.push(Span::styled(
                format!("/{}", cmd),
                if selected {
                    Style::default()
                        .fg(crate::colors::primary())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ));
            if selected {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    "Enter to configure",
                    Style::default().fg(crate::colors::text_dim()),
                ));
            }
            lines.push(Line::from(spans));
        }

        let add_idx = state.rows.len() + state.commands.len();
        let add_selected = add_idx == state.selected;
        let mut add_spans = Vec::new();
        add_spans.push(Span::styled(
            if add_selected { "› " } else { "  " },
            if add_selected {
                Style::default().fg(crate::colors::primary())
            } else {
                Style::default()
            },
        ));
        add_spans.push(Span::styled(
            "Add new…",
            if add_selected {
                Style::default()
                    .fg(crate::colors::primary())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        ));
        if add_selected {
            add_spans.push(Span::raw("  "));
            add_spans.push(Span::styled(
                "Enter to create",
                Style::default().fg(crate::colors::text_dim()),
            ));
        }
        lines.push(Line::from(add_spans));

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("↑↓", Style::default().fg(crate::colors::function())),
            Span::styled(" Navigate  ", Style::default().fg(crate::colors::text_dim())),
            Span::styled("Enter", Style::default().fg(crate::colors::success())),
            Span::styled(" Open", Style::default().fg(crate::colors::text_dim())),
            Span::styled("  Esc", Style::default().fg(crate::colors::error())),
            Span::styled(" Close", Style::default().fg(crate::colors::text_dim())),
        ]));

        Paragraph::new(lines)
            .style(Style::default().bg(crate::colors::background()).fg(crate::colors::text()))
            .render(area, buf);
    }

    fn handle_overview_key(
        state: &mut AgentsOverviewState,
        key: KeyEvent,
        app_event_tx: &AppEventSender,
    ) -> bool {
        match key.code {
            KeyCode::Up => {
                if state.total_rows() == 0 {
                    return true;
                }
                if state.selected == 0 {
                    state.selected = state.total_rows().saturating_sub(1);
                } else {
                    state.selected -= 1;
                }
                app_event_tx
                    .send(AppEvent::AgentsOverviewSelectionChanged { index: state.selected });
                true
            }
            KeyCode::Down => {
                let total = state.total_rows();
                if total == 0 {
                    return true;
                }
                state.selected = (state.selected + 1) % total;
                app_event_tx
                    .send(AppEvent::AgentsOverviewSelectionChanged { index: state.selected });
                true
            }
            KeyCode::Enter => {
                let idx = state.selected;
                if idx < state.rows.len() {
                    let row = &state.rows[idx];
                    if !row.installed {
                        app_event_tx
                            .send(AppEvent::RequestAgentInstall { name: row.name.clone(), selected_index: idx });
                    } else {
                        app_event_tx
                            .send(AppEvent::ShowAgentEditor { name: row.name.clone() });
                    }
                } else {
                    let cmd_idx = idx.saturating_sub(state.rows.len());
                    if cmd_idx < state.commands.len() {
                        if let Some(name) = state.commands.get(cmd_idx) {
                            app_event_tx
                                .send(AppEvent::ShowSubagentEditorForName { name: name.clone() });
                        }
                    } else {
                        app_event_tx.send(AppEvent::ShowSubagentEditorNew);
                    }
                }
                true
            }
            _ => false,
        }
    }
}

impl SettingsContent for AgentsSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        match &self.pane {
            AgentsPane::Overview(state) => {
                self.render_overview(area, buf, state);
            }
            AgentsPane::Subagent(view) => {
                view.render(area, buf);
            }
            AgentsPane::Agent(view) => {
                view.render(area, buf);
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match &mut self.pane {
            AgentsPane::Overview(state) => {
                Self::handle_overview_key(state, key, &self.app_event_tx)
            }
            AgentsPane::Subagent(view) => view.handle_key_event_direct(key),
            AgentsPane::Agent(view) => view.handle_key_event_direct(key),
        }
    }

    fn is_complete(&self) -> bool {
        false
    }
}

pub(crate) struct LimitsSettingsContent {
    overlay: LimitsOverlay,
}

impl LimitsSettingsContent {
    pub(crate) fn new(content: LimitsOverlayContent) -> Self {
        Self { overlay: LimitsOverlay::new(content) }
    }

    pub(crate) fn set_content(&mut self, content: LimitsOverlayContent) {
        self.overlay.set_content(content);
    }

    fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::widgets::Paragraph;

        if area.width == 0 || area.height == 0 {
            return;
        }

        if let Some(tabs) = self.overlay.tabs() {
            let mut spans = Vec::new();
            for (idx, tab) in tabs.iter().enumerate() {
                let selected = idx == self.overlay.selected_tab();
                let style = if selected {
                    Style::default()
                        .fg(crate::colors::text())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(crate::colors::text_dim())
                };
                spans.push(Span::styled(format!(" {} ", tab.title), style));
                spans.push(Span::raw(" "));
            }
            Paragraph::new(Line::from(spans))
                .style(Style::default().bg(crate::colors::background()))
                .render(area, buf);
        }
    }

    fn render_body(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::widgets::Paragraph;
        use ratatui::widgets::Wrap;

        if area.width == 0 || area.height == 0 {
            self.overlay.set_visible_rows(0);
            self.overlay.set_max_scroll(0);
            return;
        }

        self.overlay.set_visible_rows(area.height);

        let lines = self.overlay.lines_for_width(area.width);
        let max_scroll = lines.len().saturating_sub(area.height as usize) as u16;
        self.overlay.set_max_scroll(max_scroll);

        let start = self.overlay.scroll() as usize;
        let end = (start + area.height as usize).min(lines.len());
        let viewport = if start < end {
            lines[start..end].to_vec()
        } else {
            Vec::new()
        };

        Paragraph::new(viewport)
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(crate::colors::background()).fg(crate::colors::text()))
            .render(area, buf);
    }

    fn render_hint_row(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::widgets::Paragraph;

        if area.width == 0 || area.height == 0 {
            return;
        }

        let hint_style = Style::default().fg(crate::colors::text_dim());
        let accent_style = Style::default().fg(crate::colors::function());
        let line = Line::from(vec![
            Span::styled("↑↓", accent_style),
            Span::styled(" scroll  ", hint_style),
            Span::styled("PgUp/PgDn", accent_style),
            Span::styled(" page  ", hint_style),
            Span::styled("◂ ▸", accent_style),
            Span::styled(" change tab", hint_style),
        ]);

        Paragraph::new(line)
            .alignment(Alignment::Left)
            .style(Style::default().bg(crate::colors::background()).fg(crate::colors::text_dim()))
            .render(area, buf);
    }
}

impl SettingsContent for LimitsSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            self.overlay.set_visible_rows(0);
            self.overlay.set_max_scroll(0);
            return;
        }

        fill_rect(
            buf,
            area,
            Some(' '),
            Style::default().bg(crate::colors::background()),
        );

        let has_tabs = self.overlay.tab_count() > 1;
        let constraints = if has_tabs {
            vec![Constraint::Length(1), Constraint::Length(2), Constraint::Fill(1)]
        } else {
            vec![Constraint::Length(1), Constraint::Fill(1)]
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let hint_area = chunks[0];
        let (tabs_area, body_area) = if has_tabs {
            (Some(chunks[1]), chunks[2])
        } else {
            (None, chunks[1])
        };

        self.render_hint_row(hint_area, buf);

        if let Some(tabs_rect) = tabs_area {
            self.render_tabs(tabs_rect, buf);
        }

        self.render_body(body_area, buf);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up => {
                let current = self.overlay.scroll();
                if current > 0 {
                    self.overlay.set_scroll(current - 1);
                }
                true
            }
            KeyCode::Down => {
                let current = self.overlay.scroll();
                let next = current.saturating_add(1).min(self.overlay.max_scroll());
                self.overlay.set_scroll(next);
                true
            }
            KeyCode::PageUp => {
                let step = self.overlay.visible_rows().max(1);
                let current = self.overlay.scroll();
                self.overlay.set_scroll(current.saturating_sub(step));
                true
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                let step = self.overlay.visible_rows().max(1);
                let current = self.overlay.scroll();
                let next = current.saturating_add(step).min(self.overlay.max_scroll());
                self.overlay.set_scroll(next);
                true
            }
            KeyCode::Home => {
                self.overlay.set_scroll(0);
                true
            }
            KeyCode::End => {
                self.overlay.set_scroll(self.overlay.max_scroll());
                true
            }
            KeyCode::Left | KeyCode::Char('[') => self.overlay.select_prev_tab(),
            KeyCode::Right | KeyCode::Char(']') => self.overlay.select_next_tab(),
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.overlay.select_prev_tab()
                } else {
                    self.overlay.select_next_tab()
                }
            }
            KeyCode::BackTab => self.overlay.select_prev_tab(),
            _ => false,
        }
    }

    fn is_complete(&self) -> bool {
        false
    }
}

pub(crate) struct ChromeSettingsContent {
    selected_index: usize,
    app_event_tx: AppEventSender,
    port: Option<u16>,
    is_complete: bool,
}

impl ChromeSettingsContent {
    pub(crate) fn new(app_event_tx: AppEventSender, port: Option<u16>) -> Self {
        Self {
            selected_index: 0,
            app_event_tx,
            port,
            is_complete: false,
        }
    }

    fn options() -> &'static [(ChromeLaunchOption, &'static str, &'static str)] {
        CHROME_LAUNCH_CHOICES
    }

    fn move_up(&mut self) {
        let len = Self::options().len();
        if self.selected_index == 0 {
            self.selected_index = len.saturating_sub(1);
        } else {
            self.selected_index -= 1;
        }
    }

    fn move_down(&mut self) {
        let len = Self::options().len();
        if len > 0 {
            self.selected_index = (self.selected_index + 1) % len;
        }
    }

    fn confirm(&mut self) {
        if let Some((option, _, _)) = Self::options().get(self.selected_index) {
            let _ = self
                .app_event_tx
                .send(AppEvent::ChromeLaunchOptionSelected(*option, self.port));
            self.is_complete = true;
        }
    }

    fn cancel(&mut self) {
        let _ = self.app_event_tx.send(AppEvent::ChromeLaunchOptionSelected(
            ChromeLaunchOption::Cancel,
            self.port,
        ));
        self.is_complete = true;
    }
}

impl SettingsContent for ChromeSettingsContent {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(" Chrome Launch Options "))
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(crate::colors::background()).fg(crate::colors::text()))
            .border_style(Style::default().fg(crate::colors::border()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![Span::styled(
            "Chrome is already running or CDP connection failed",
            Style::default()
                .fg(crate::colors::warning())
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));
        lines.push(Line::from("Select an option:"));
        lines.push(Line::from(""));

        for (idx, (_, label, description)) in Self::options().iter().enumerate() {
            let selected = idx == self.selected_index;
            if selected {
                lines.push(Line::from(vec![Span::styled(
                    format!("› {}", label),
                    Style::default()
                        .fg(crate::colors::success())
                        .add_modifier(Modifier::BOLD),
                )]));
                lines.push(Line::from(vec![Span::styled(
                    format!("  {}", description),
                    Style::default().fg(crate::colors::secondary()),
                )]));
            } else {
                lines.push(Line::from(vec![Span::styled(
                    format!("  {}", label),
                    Style::default().fg(crate::colors::text()),
                )]));
                lines.push(Line::from(vec![Span::styled(
                    format!("  {}", description),
                    Style::default().fg(crate::colors::text_dim()),
                )]));
            }
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::styled("↑↓/jk", Style::default().fg(crate::colors::function())),
            Span::styled(" move  ", Style::default().fg(crate::colors::text_dim())),
            Span::styled("Enter", Style::default().fg(crate::colors::function())),
            Span::styled(" select  ", Style::default().fg(crate::colors::text_dim())),
            Span::styled("Esc/q", Style::default().fg(crate::colors::function())),
            Span::styled(" cancel", Style::default().fg(crate::colors::text_dim())),
        ]));

        let content_area = inner.inner(Margin::new(1, 1));
        if content_area.width == 0 || content_area.height == 0 {
            return;
        }

        Paragraph::new(lines)
            .alignment(Alignment::Left)
            .style(Style::default().bg(crate::colors::background()).fg(crate::colors::text()))
            .render(content_area, buf);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_up();
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_down();
                true
            }
            KeyCode::Enter => {
                self.confirm();
                true
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.cancel();
                true
            }
            _ => false,
        }
    }

    fn is_complete(&self) -> bool {
        self.is_complete
    }
}

/// Full-screen settings overlay rendered by the chat widget.
pub(crate) struct SettingsOverlayView {
    overview_rows: Vec<SettingsOverviewRow>,
    mode: SettingsOverlayMode,
    last_section: SettingsSection,
    help: Option<SettingsHelpOverlay>,
    model_content: Option<ModelSettingsContent>,
    theme_content: Option<ThemeSettingsContent>,
    updates_content: Option<UpdatesSettingsContent>,
    notifications_content: Option<NotificationsSettingsContent>,
    mcp_content: Option<McpSettingsContent>,
    agents_content: Option<AgentsSettingsContent>,
    validation_content: Option<ValidationSettingsContent>,
    github_content: Option<GithubSettingsContent>,
    auto_drive_content: Option<AutoDriveSettingsContent>,
    limits_content: Option<LimitsSettingsContent>,
    chrome_content: Option<ChromeSettingsContent>,
}

impl SettingsOverlayView {
    pub(crate) fn new(section: SettingsSection) -> Self {
        let section_state = SectionState::new(section);
        Self {
            overview_rows: Vec::new(),
            mode: SettingsOverlayMode::Section(section_state),
            last_section: section,
            help: None,
            model_content: None,
            theme_content: None,
            updates_content: None,
            notifications_content: None,
            mcp_content: None,
            agents_content: None,
            validation_content: None,
            github_content: None,
            auto_drive_content: None,
            limits_content: None,
            chrome_content: None,
        }
    }

    pub(crate) fn active_section(&self) -> SettingsSection {
        match self.mode {
            SettingsOverlayMode::Menu(state) => state.selected(),
            SettingsOverlayMode::Section(state) => state.active(),
        }
    }

    pub(crate) fn is_menu_active(&self) -> bool {
        matches!(self.mode, SettingsOverlayMode::Menu(_))
    }

    pub(crate) fn set_mode_menu(&mut self, selected: Option<SettingsSection>) {
        let section = selected.unwrap_or(self.last_section);
        self.mode = SettingsOverlayMode::Menu(MenuState::new(section));
        if self.help.is_some() {
            self.show_help(true);
        }
    }

    pub(crate) fn set_mode_section(&mut self, section: SettingsSection) {
        self.mode = SettingsOverlayMode::Section(SectionState::new(section));
        self.last_section = section;
        if self.help.is_some() {
            self.show_help(false);
        }
    }

    pub(crate) fn is_help_visible(&self) -> bool {
        self.help.is_some()
    }

    pub(crate) fn show_help(&mut self, menu_active: bool) {
        self.help = Some(if menu_active {
            SettingsHelpOverlay::overview()
        } else {
            SettingsHelpOverlay::section(self.active_section())
        });
    }

    pub(crate) fn hide_help(&mut self) {
        self.help = None;
    }

    pub(crate) fn set_overview_rows(&mut self, rows: Vec<SettingsOverviewRow>) {
        let fallback = rows.first().map(|row| row.section).unwrap_or(self.last_section);
        if let SettingsOverlayMode::Menu(state) = &mut self.mode {
            if !rows.iter().any(|row| row.section == state.selected()) {
                state.set_selected(fallback);
            }
        }
        self.overview_rows = rows;
    }

    pub(crate) fn set_model_content(&mut self, content: ModelSettingsContent) {
        self.model_content = Some(content);
    }

    pub(crate) fn set_theme_content(&mut self, content: ThemeSettingsContent) {
        self.theme_content = Some(content);
    }

    pub(crate) fn set_updates_content(&mut self, content: UpdatesSettingsContent) {
        self.updates_content = Some(content);
    }

    pub(crate) fn set_notifications_content(&mut self, content: NotificationsSettingsContent) {
        self.notifications_content = Some(content);
    }

    pub(crate) fn set_mcp_content(&mut self, content: McpSettingsContent) {
        self.mcp_content = Some(content);
    }

    pub(crate) fn set_agents_content(&mut self, content: AgentsSettingsContent) {
        self.agents_content = Some(content);
    }

    pub(crate) fn set_validation_content(&mut self, content: ValidationSettingsContent) {
        self.validation_content = Some(content);
    }

    pub(crate) fn set_github_content(&mut self, content: GithubSettingsContent) {
        self.github_content = Some(content);
    }

    pub(crate) fn set_auto_drive_content(&mut self, content: AutoDriveSettingsContent) {
        self.auto_drive_content = Some(content);
    }

    pub(crate) fn set_limits_content(&mut self, content: LimitsSettingsContent) {
        self.limits_content = Some(content);
    }

    pub(crate) fn set_chrome_content(&mut self, content: ChromeSettingsContent) {
        self.chrome_content = Some(content);
    }

    #[cfg_attr(not(any(test, feature = "test-helpers")), allow(dead_code))]
    pub(crate) fn agents_content(&self) -> Option<&AgentsSettingsContent> {
        self.agents_content.as_ref()
    }

    pub(crate) fn agents_content_mut(&mut self) -> Option<&mut AgentsSettingsContent> {
        self.agents_content.as_mut()
    }

    pub(crate) fn limits_content_mut(&mut self) -> Option<&mut LimitsSettingsContent> {
        self.limits_content.as_mut()
    }

    pub(crate) fn set_section(&mut self, section: SettingsSection) -> bool {
        if self.active_section() == section {
            return false;
        }
        self.last_section = section;
        match &mut self.mode {
            SettingsOverlayMode::Menu(state) => state.set_selected(section),
            SettingsOverlayMode::Section(state) => state.set_active(section),
        }
        if self.help.is_some() {
            self.show_help(self.is_menu_active());
        }
        true
    }

    pub(crate) fn select_next(&mut self) -> bool {
        if !self.overview_rows.is_empty() {
            let sections: Vec<SettingsSection> =
                self.overview_rows.iter().map(|row| row.section).collect();
            if let Some(idx) = sections
                .iter()
                .position(|section| *section == self.active_section())
            {
                let next = sections[(idx + 1) % sections.len()];
                return self.set_section(next);
            }
        }
        let mut idx = self.index_of(self.active_section());
        idx = (idx + 1) % SettingsSection::ALL.len();
        self.set_section(SettingsSection::ALL[idx])
    }

    pub(crate) fn select_previous(&mut self) -> bool {
        if !self.overview_rows.is_empty() {
            let sections: Vec<SettingsSection> =
                self.overview_rows.iter().map(|row| row.section).collect();
            if let Some(idx) = sections
                .iter()
                .position(|section| *section == self.active_section())
            {
                let new_idx = idx.checked_sub(1).unwrap_or(sections.len() - 1);
                return self.set_section(sections[new_idx]);
            }
        }
        let mut idx = self.index_of(self.active_section());
        idx = idx.checked_sub(1).unwrap_or(SettingsSection::ALL.len() - 1);
        self.set_section(SettingsSection::ALL[idx])
    }

    fn index_of(&self, section: SettingsSection) -> usize {
        SettingsSection::ALL
            .iter()
            .position(|s| *s == section)
            .unwrap_or(0)
    }

    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let block = Block::default()
            .title(self.block_title_line())
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .style(Style::default().bg(crate::colors::background()))
            .border_style(
                Style::default()
                    .fg(crate::colors::border())
                    .bg(crate::colors::background()),
            );
        let inner = block.inner(area);
        block.render(area, buf);

        let bg = Style::default().bg(crate::colors::background());
        for y in inner.y..inner.y.saturating_add(inner.height) {
            for x in inner.x..inner.x.saturating_add(inner.width) {
                buf[(x, y)].set_style(bg);
            }
        }

        let content = inner.inner(Margin::new(1, 1));
        if content.width == 0 || content.height == 0 {
            return;
        }

        if self.is_menu_active() {
            self.render_overview(content, buf);
        } else {
            self.render_section_layout(content, buf);
        }

        if let Some(help) = &self.help {
            self.render_help_overlay(inner, buf, help);
        }
    }

    fn block_title_line(&self) -> Line<'static> {
        if self.is_menu_active() {
            Line::from(vec![
                Span::styled("Settings", Style::default().fg(crate::colors::text())),
                Span::styled(" ▸ ", Style::default().fg(crate::colors::text_dim())),
                Span::styled("Overview", Style::default().fg(crate::colors::text())),
            ])
        } else {
            Line::from(vec![
                Span::styled("Settings", Style::default().fg(crate::colors::text())),
                Span::styled(" ▸ ", Style::default().fg(crate::colors::text_dim())),
                Span::styled(
                    self.active_section().label(),
                    Style::default().fg(crate::colors::text()),
                ),
            ])
        }
    }

    fn render_overview(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let (list_area, hint_area) = match area.height {
            0 => return,
            1 => (area, None),
            _ => {
                let [list, hint] =
                    Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
                (list, Some(hint))
            }
        };

        self.render_overview_list(list_area, buf);
        if let Some(hint_area) = hint_area {
            self.render_footer_hints(hint_area, buf);
        }
    }

    fn render_overview_list(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        fill_rect(
            buf,
            area,
            Some(' '),
            Style::default().bg(crate::colors::background()),
        );

        if self.overview_rows.is_empty() {
            let line = Line::from(vec![Span::styled(
                "No settings available.",
                Style::default().fg(crate::colors::text_dim()),
            )]);
            Paragraph::new(line)
                .style(Style::default().bg(crate::colors::background()))
                .render(area, buf);
            return;
        }

        let active_section = self.active_section();
        let content_width = area.width as usize;
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut selected_range: Option<(usize, usize)> = None;

        for (idx, row) in self.overview_rows.iter().enumerate() {
            let is_active = row.section == active_section;
            let indicator = if is_active { "›" } else { " " };

            let row_start = lines.len();

            if row.section == SettingsSection::Limits && !lines.is_empty() {
                lines.push(Line::from(""));
                let dash_count = content_width.saturating_sub(2);
                if dash_count > 0 {
                    lines.push(Line::from(vec![Span::styled(
                        format!("  {}", "─".repeat(dash_count)),
                        Style::default().fg(crate::colors::border_dim()),
                    )]));
                    lines.push(Line::from(""));
                }
            }

            let label_style = if is_active {
                Style::default()
                    .fg(crate::colors::text_bright())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(crate::colors::text_mid())
            };

            let label_text = format!("{:<width$}", row.section.label(), width = LABEL_COLUMN_WIDTH);

            let summary_src = row.summary.as_deref().unwrap_or("—");
            let base_width = 1 + 1 + LABEL_COLUMN_WIDTH;
            let available_tail = content_width.saturating_sub(base_width);

            let mut summary_line = Line::from(vec![
                Span::styled(indicator.to_string(), Style::default().fg(crate::colors::text())),
                Span::raw(" "),
                Span::styled(label_text, label_style),
            ]);

            if available_tail > 0 {
                summary_line.spans.push(Span::raw(" "));
                let summary_budget = available_tail.saturating_sub(1);

                if summary_budget > 0 {
                    let summary_trimmed = self.trim_with_ellipsis(summary_src, summary_budget);
                    if !summary_trimmed.is_empty() {
                        self.push_summary_spans(&mut summary_line, &summary_trimmed);
                    }
                }
            }

            if is_active {
                summary_line = summary_line.style(
                    Style::default()
                        .bg(crate::colors::selection())
                        .fg(crate::colors::text()),
                );
            }
            lines.push(summary_line);

            let info_text = row.section.help_line();
            let info_trimmed = self.trim_with_ellipsis(info_text, content_width.saturating_sub(8));
            let info_style = Style::default().fg(crate::colors::text_dim());
            let mut info_line = Line::from(vec![
                Span::raw("  "),
                Span::styled("└ ", info_style),
                Span::styled(info_trimmed, info_style),
            ]);
            if is_active {
                info_line = info_line.style(
                    Style::default()
                        .bg(crate::colors::selection())
                        .fg(crate::colors::text()),
                );
            }
            lines.push(info_line);

            if is_active {
                let row_end = lines.len().saturating_sub(1);
                selected_range = Some((row_start, row_end));
            }

            if idx != self.overview_rows.len().saturating_sub(1) {
                lines.push(Line::from(""));
                if matches!(row.section, SettingsSection::Updates) {
                    let dash_count = content_width.saturating_sub(2);
                    if dash_count > 0 {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {}", "─".repeat(dash_count)),
                            Style::default().fg(crate::colors::border_dim()),
                        )]));
                        lines.push(Line::from(""));
                    }
                }
            }
        }

        let total_lines = lines.len();
        let visible_lines = area.height as usize;
        let mut scroll = 0usize;
        if visible_lines > 0 && total_lines > visible_lines {
            if let Some((start, end)) = selected_range {
                let max_scroll = total_lines.saturating_sub(visible_lines);
                let mut candidate = end
                    .saturating_add(1)
                    .saturating_sub(visible_lines);
                if candidate > max_scroll {
                    candidate = max_scroll;
                }
                if start < candidate {
                    candidate = start.min(max_scroll);
                }
                if end >= candidate.saturating_add(visible_lines) {
                    candidate = end
                        .saturating_add(1)
                        .saturating_sub(visible_lines)
                        .min(max_scroll);
                }
                scroll = candidate;
            } else {
                scroll = total_lines.saturating_sub(visible_lines);
            }
        }
        let scroll = scroll.min(u16::MAX as usize) as u16;

        Paragraph::new(lines)
            .alignment(Alignment::Left)
            .style(Style::default().bg(crate::colors::background()))
            .scroll((scroll, 0))
            .render(area, buf);
    }

    fn trim_with_ellipsis(&self, text: &str, max_width: usize) -> String {
        if max_width == 0 || text.is_empty() {
            return String::new();
        }
        if UnicodeWidthStr::width(text) <= max_width {
            return text.to_string();
        }
        if max_width <= 3 {
            return "...".chars().take(max_width).collect();
        }
        let keep = max_width.saturating_sub(3);
        let (prefix, _, _) = take_prefix_by_width(text, keep);
        let mut result = prefix;
        result.push_str("...");
        result
    }

    fn push_summary_spans(&self, line: &mut Line<'static>, summary: &str) {
        let label_style = Style::default().fg(crate::colors::text_mid());
        let dim_style = Style::default().fg(crate::colors::text_dim());
        let mut first = true;
        for raw_segment in summary.split(" · ") {
            let segment = raw_segment.trim();
            if segment.is_empty() {
                continue;
            }
            if !first {
                line.spans
                    .push(Span::styled(" · ".to_string(), dim_style));
            }
            first = false;

            if let Some((label, value)) = segment.split_once(':') {
                let label_trim = label.trim_end();
                let value_trim = value.trim_start();
                line.spans.push(Span::styled(
                    format!("{}:", label_trim),
                    label_style,
                ));
                if !value_trim.is_empty() {
                    line.spans
                        .push(Span::styled(" ".to_string(), dim_style));
                    let value_style = self.summary_value_style(value_trim);
                    line.spans
                        .push(Span::styled(value_trim.to_string(), value_style));
                }
            } else {
                let value_style = self.summary_value_style(segment);
                line.spans
                    .push(Span::styled(segment.to_string(), value_style));
            }
        }
    }

    fn summary_value_style(&self, value: &str) -> Style {
        let trimmed = value.trim();
        let normalized = trimmed
            .trim_end_matches(|c: char| matches!(c, '.' | '!' | ','))
            .to_ascii_lowercase();
        if matches!(normalized.as_str(), "on" | "enabled" | "yes") {
            Style::default().fg(crate::colors::success())
        } else if matches!(normalized.as_str(), "off" | "disabled" | "no") {
            Style::default().fg(crate::colors::error())
        } else {
            Style::default().fg(crate::colors::info())
        }
    }

    fn render_footer_hints(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let line = Line::from(vec![
            Span::styled("↑ ↓", Style::default().fg(crate::colors::text())),
            Span::styled(" Move    ", Style::default().fg(crate::colors::text_dim())),
            Span::styled("Enter", Style::default().fg(crate::colors::text())),
            Span::styled(" Open    ", Style::default().fg(crate::colors::text_dim())),
            Span::styled("Esc", Style::default().fg(crate::colors::text())),
            Span::styled(" Close    ", Style::default().fg(crate::colors::text_dim())),
            Span::styled("?", Style::default().fg(crate::colors::text())),
            Span::styled(" Help", Style::default().fg(crate::colors::text_dim())),
        ]);

        Paragraph::new(line)
            .style(Style::default().bg(crate::colors::background()))
            .alignment(Alignment::Left)
            .render(area, buf);
    }

    fn render_section_layout(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let (main_area, hint_area) = if area.height <= 1 {
            (area, None)
        } else {
            let [main, hint] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
            (main, Some(hint))
        };

        self.render_section_main(main_area, buf);
        if let Some(hint_area) = hint_area {
            self.render_footer_hints(hint_area, buf);
        }
    }

    fn render_section_main(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let [sidebar, main] =
            Layout::horizontal([Constraint::Length(22), Constraint::Fill(1)]).areas(area);

        self.render_sidebar(sidebar, buf);
        self.render_section_panel(main, buf);
    }

    fn render_section_panel(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let title = Self::section_panel_title(self.active_section());
        render_panel(
            area,
            buf,
            title,
            PanelFrameStyle::overlay().with_margin(Margin::new(1, 1)),
            |inner, buf| {
                self.render_content(inner, buf);
                self.strip_child_border(inner, buf);
            },
        );
    }

    fn section_panel_title(section: SettingsSection) -> &'static str {
        match section {
            SettingsSection::Model => "Select Model & Reasoning",
            SettingsSection::Theme => "Theme Settings",
            SettingsSection::Updates => "Upgrade",
            SettingsSection::Agents => "Agents",
            SettingsSection::AutoDrive => "Auto Drive Settings",
            SettingsSection::Validation => "Validation Settings",
            SettingsSection::Github => "GitHub Settings",
            SettingsSection::Limits => "Rate Limits",
            SettingsSection::Chrome => "Chrome Launch Options",
            SettingsSection::Notifications => "Notifications",
            SettingsSection::Mcp => "MCP Servers",
        }
    }

    fn strip_child_border(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let background = Style::default().bg(crate::colors::background()).fg(crate::colors::text());
        let end_x = area.x + area.width - 1;
        let end_y = area.y + area.height - 1;

        let top_left_symbol = buf[(area.x, area.y)].symbol();
        let top_right_symbol = buf[(end_x, area.y)].symbol();
        let bottom_left_symbol = buf[(area.x, end_y)].symbol();
        let bottom_right_symbol = buf[(end_x, end_y)].symbol();

        let top_has_corners = Self::is_corner_symbol(top_left_symbol) && Self::is_corner_symbol(top_right_symbol);
        let bottom_has_corners = Self::is_corner_symbol(bottom_left_symbol) && Self::is_corner_symbol(bottom_right_symbol);

        let top_is_frame = top_has_corners
            && (area.x..=end_x).all(|x| {
                let symbol = buf[(x, area.y)].symbol();
                Self::is_border_symbol(symbol) || Self::is_corner_symbol(symbol)
            });
        let bottom_is_frame = if area.height > 1 {
            Some(
                bottom_has_corners
                    && (area.x..=end_x).all(|x| {
                        let symbol = buf[(x, end_y)].symbol();
                        Self::is_border_symbol(symbol) || Self::is_corner_symbol(symbol)
                    }),
            )
        } else {
            None
        };

        let left_has_corners = Self::is_corner_symbol(top_left_symbol) && Self::is_corner_symbol(bottom_left_symbol);
        let right_has_corners = Self::is_corner_symbol(top_right_symbol) && Self::is_corner_symbol(bottom_right_symbol);

        let left_is_frame = left_has_corners
            && (area.y..=end_y).all(|y| {
                let symbol = buf[(area.x, y)].symbol();
                Self::is_border_symbol(symbol) || Self::is_corner_symbol(symbol)
            });
        let right_is_frame = if area.width > 1 {
            Some(
                right_has_corners
                    && (area.y..=end_y).all(|y| {
                        let symbol = buf[(end_x, y)].symbol();
                        Self::is_border_symbol(symbol) || Self::is_corner_symbol(symbol)
                    }),
            )
        } else {
            None
        };

        if top_is_frame {
            for x in area.x..=end_x {
                let cell = &mut buf[(x, area.y)];
                cell.set_symbol(" ");
                cell.set_style(background);
            }
        }

        if let Some(true) = bottom_is_frame {
            for x in area.x..=end_x {
                let cell = &mut buf[(x, end_y)];
                cell.set_symbol(" ");
                cell.set_style(background);
            }
        }

        if left_is_frame {
            for y in area.y..=end_y {
                let cell = &mut buf[(area.x, y)];
                cell.set_symbol(" ");
                cell.set_style(background);
            }
        }

        if let Some(true) = right_is_frame {
            for y in area.y..=end_y {
                let cell = &mut buf[(end_x, y)];
                cell.set_symbol(" ");
                cell.set_style(background);
            }
        }
    }

    fn is_border_symbol(symbol: &str) -> bool {
        matches!(
            symbol,
            "│" | "┃" | "║" | "╎" | "┆" | "┊" | "┇" | "╏" | "╿"
                | "─" | "━" | "═" | "╼" | "╾" | "┄" | "┈" | "╍"
                | "┬" | "┴" | "├" | "┤" | "┼" | "╞" | "╡" | "╪" | "╫"
        )
    }

    fn is_corner_symbol(symbol: &str) -> bool {
        matches!(symbol, "┌" | "┐" | "└" | "┘" | "╭" | "╮" | "╰" | "╯")
    }

    fn render_help_overlay(&self, area: Rect, buf: &mut Buffer, help: &SettingsHelpOverlay) {
        if area.width < 4 || area.height < 4 {
            return;
        }

        fill_rect(buf, area, None, Style::default().bg(crate::colors::overlay_scrim()));

        let content_width = help
            .lines
            .iter()
            .map(Line::width)
            .max()
            .unwrap_or(0);
        let content_height = help.lines.len() as u16;

        let max_box_width = area.width.saturating_sub(2);
        let mut box_width = content_width
            .saturating_add(4)
            .min(max_box_width as usize)
            .max(20.min(max_box_width as usize));
        if box_width == 0 {
            box_width = max_box_width as usize;
        }
        let box_width = box_width.min(area.width as usize) as u16;

        let max_box_height = area.height.saturating_sub(2);
        let mut box_height = content_height.saturating_add(2).min(max_box_height);
        if box_height < 4 {
            box_height = max_box_height.min(4);
        }
        if box_height == 0 {
            box_height = area.height;
        }

        let box_x = area.x + (area.width.saturating_sub(box_width)) / 2;
        let box_y = area.y + (area.height.saturating_sub(box_height)) / 2;
        let box_area = Rect::new(box_x, box_y, box_width, box_height);

        fill_rect(
            buf,
            box_area,
            Some(' '),
            Style::default().bg(crate::colors::background()),
        );

        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(crate::colors::border()))
            .style(Style::default().bg(crate::colors::background()))
            .render(box_area, buf);

        let inner = box_area.inner(Margin::new(1, 1));
        if inner.width == 0 || inner.height == 0 {
            return;
        }

        Paragraph::new(help.lines.clone())
            .alignment(Alignment::Left)
            .style(Style::default().bg(crate::colors::background()).fg(crate::colors::text()))
            .wrap(Wrap { trim: true })
            .render(inner, buf);
    }

    fn render_sidebar(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let sections: Vec<SettingsSection> = if self.overview_rows.is_empty() {
            SettingsSection::ALL.to_vec()
        } else {
            self.overview_rows.iter().map(|row| row.section).collect()
        };

        fill_rect(
            buf,
            area,
            Some(' '),
            Style::default().bg(crate::colors::background()),
        );

        if sections.is_empty() {
            return;
        }

        let visible = area.height as usize;
        if visible == 0 {
            return;
        }

        let selected_idx = sections
            .iter()
            .position(|section| *section == self.active_section())
            .unwrap_or(0);

        let total = sections.len();
        let mut start = 0usize;
        if total > visible {
            let half = visible / 2;
            if selected_idx > half {
                start = selected_idx - half;
            }
            if start + visible > total {
                start = total - visible;
            }
        }
        let end = (start + visible).min(total);

        let mut lines: Vec<Line<'static>> = Vec::new();
        for idx in start..end {
            let section = sections[idx];
            let is_active = idx == selected_idx;
            let is_first_visible = idx == start;
            let is_last_visible = idx + 1 == end;

            let selection_indicator = if is_active { "›" } else { " " };
            let overflow_indicator = if is_first_visible && start > 0 {
                "↑"
            } else if is_last_visible && end < total {
                "↓"
            } else {
                " "
            };

            let mut spans: Vec<Span<'static>> = Vec::new();
            spans.push(Span::styled(
                selection_indicator.to_string(),
                Style::default().fg(crate::colors::text()),
            ));
            spans.push(Span::styled(
                overflow_indicator.to_string(),
                Style::default().fg(crate::colors::text_dim()),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                section.label(),
                if is_active {
                    Style::default()
                        .fg(crate::colors::text())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(crate::colors::text_dim())
                },
            ));

            let line = if is_active {
                Line::from(spans).style(
                    Style::default()
                        .bg(crate::colors::selection())
                        .fg(crate::colors::text()),
                )
            } else {
                Line::from(spans)
            };
            lines.push(line);
        }

        while lines.len() < visible {
            lines.push(Line::from(" "));
        }

        Paragraph::new(lines)
            .alignment(Alignment::Left)
            .style(Style::default().bg(crate::colors::background()))
            .render(area, buf);
    }

    fn render_content(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        match self.active_section() {
            SettingsSection::Model => {
                if let Some(content) = self.model_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::Model.placeholder());
            }
            SettingsSection::Theme => {
                if let Some(content) = self.theme_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::Theme.placeholder());
            }
            SettingsSection::Updates => {
                if let Some(content) = self.updates_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::Updates.placeholder());
            }
            SettingsSection::Agents => {
                if let Some(content) = self.agents_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::Agents.placeholder());
            }
            SettingsSection::AutoDrive => {
                if let Some(content) = self.auto_drive_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::AutoDrive.placeholder());
            }
            SettingsSection::Validation => {
                if let Some(content) = self.validation_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::Validation.placeholder());
            }
            SettingsSection::Github => {
                if let Some(content) = self.github_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::Github.placeholder());
            }
            SettingsSection::Limits => {
                if let Some(content) = self.limits_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::Limits.placeholder());
            }
            SettingsSection::Chrome => {
                if let Some(content) = self.chrome_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::Chrome.placeholder());
            }
            SettingsSection::Notifications => {
                if let Some(content) = self.notifications_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::Notifications.placeholder());
            }
            SettingsSection::Mcp => {
                if let Some(content) = self.mcp_content.as_ref() {
                    content.render(area, buf);
                    return;
                }
                self.render_placeholder(area, buf, SettingsSection::Mcp.placeholder());
            }
        }
    }

    fn render_placeholder(&self, area: Rect, buf: &mut Buffer, text: &'static str) {
        let paragraph = Paragraph::new(text)
            .wrap(ratatui::widgets::Wrap { trim: true })
            .style(Style::default().fg(crate::colors::text_dim()));
        paragraph.render(area, buf);
    }

    pub(crate) fn active_content_mut(&mut self) -> Option<&mut dyn SettingsContent> {
        if self.is_menu_active() {
            return None;
        }

        match self.active_section() {
            SettingsSection::Model => self
                .model_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
            SettingsSection::Theme => self
                .theme_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
            SettingsSection::Updates => self
                .updates_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
            SettingsSection::Agents => self
                .agents_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
            SettingsSection::AutoDrive => self
                .auto_drive_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
            SettingsSection::Validation => self
                .validation_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
            SettingsSection::Github => self
                .github_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
            SettingsSection::Limits => self
                .limits_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
            SettingsSection::Chrome => self
                .chrome_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
            SettingsSection::Notifications => self
                .notifications_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
            SettingsSection::Mcp => self
                .mcp_content
                .as_mut()
                .map(|content| content as &mut dyn SettingsContent),
        }
    }

    pub(crate) fn notify_close(&mut self) {
        match self.active_section() {
            SettingsSection::Model => {
                if let Some(content) = self.model_content.as_mut() {
                    content.on_close();
                }
            }
            SettingsSection::Theme => {
                if let Some(content) = self.theme_content.as_mut() {
                    content.on_close();
                }
            }
            SettingsSection::Notifications => {
                if let Some(content) = self.notifications_content.as_mut() {
                    content.on_close();
                }
            }
            SettingsSection::Mcp => {
                if let Some(content) = self.mcp_content.as_mut() {
                    content.on_close();
                }
            }
            SettingsSection::Chrome => {
                if let Some(content) = self.chrome_content.as_mut() {
                    content.on_close();
                }
            }
            _ => {}
        }
    }
}
