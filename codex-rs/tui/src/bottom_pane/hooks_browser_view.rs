use codex_app_server_protocol::HookErrorInfo;
use codex_app_server_protocol::HookEventName;
use codex_app_server_protocol::HookMetadata;
use codex_app_server_protocol::HookSource;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use strum::IntoEnumIterator;
use unicode_width::UnicodeWidthStr;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::render_menu_surface;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::renderable::Renderable;
use crate::status::format_directory_display;

const EVENT_COLUMN_WIDTH: usize = 22;
const COUNT_COLUMN_WIDTH: usize = 12;
const MAX_COMMAND_DETAIL_LINES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HooksBrowserPage {
    Events,
    Handlers(HookEventName),
}

pub(crate) struct HooksBrowserView {
    hooks: Vec<HookMetadata>,
    warnings: Vec<String>,
    errors: Vec<HookErrorInfo>,
    page: HooksBrowserPage,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
}

impl HooksBrowserView {
    pub(crate) fn new(
        mut hooks: Vec<HookMetadata>,
        warnings: Vec<String>,
        errors: Vec<HookErrorInfo>,
        app_event_tx: AppEventSender,
    ) -> Self {
        hooks.sort_by_key(|hook| hook.display_order);
        let mut view = Self {
            hooks,
            warnings,
            errors,
            page: HooksBrowserPage::Events,
            state: ScrollState::new(),
            complete: false,
            app_event_tx,
        };
        if view.page_len() > 0 {
            view.state.selected_idx = Some(0);
        }
        view
    }

    fn event_rows(&self) -> Vec<EventRow> {
        codex_protocol::protocol::HookEventName::iter()
            .map(|event_name| {
                let event_name: HookEventName = event_name.into();
                let installed = self
                    .hooks
                    .iter()
                    .filter(|hook| hook.event_name == event_name)
                    .count();
                let active = self
                    .hooks
                    .iter()
                    .filter(|hook| {
                        hook.event_name == event_name && (hook.enabled || hook.is_managed)
                    })
                    .count();
                EventRow {
                    event_name,
                    installed,
                    active,
                }
            })
            .collect()
    }

    fn handlers_for_event(&self, event_name: HookEventName) -> impl Iterator<Item = &HookMetadata> {
        self.hooks
            .iter()
            .filter(move |hook| hook.event_name == event_name)
    }

    fn selected_event(&self) -> Option<HookEventName> {
        self.state
            .selected_idx
            .and_then(|idx| codex_protocol::protocol::HookEventName::iter().nth(idx))
            .map(Into::into)
    }

    fn selected_hook_index(&self, event_name: HookEventName) -> Option<usize> {
        let selected_visible_idx = self.state.selected_idx?;
        self.hooks
            .iter()
            .enumerate()
            .filter(|(_, hook)| hook.event_name == event_name)
            .nth(selected_visible_idx)
            .map(|(idx, _)| idx)
    }

    fn selected_hook(&self, event_name: HookEventName) -> Option<&HookMetadata> {
        self.selected_hook_index(event_name)
            .and_then(|idx| self.hooks.get(idx))
    }

    fn move_up(&mut self) {
        let len = self.page_len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, self.max_visible_rows());
    }

    fn move_down(&mut self) {
        let len = self.page_len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, self.max_visible_rows());
    }

    fn page_len(&self) -> usize {
        match self.page {
            HooksBrowserPage::Events => codex_protocol::protocol::HookEventName::iter().count(),
            HooksBrowserPage::Handlers(event_name) => self.handlers_for_event(event_name).count(),
        }
    }

    fn max_visible_rows(&self) -> usize {
        MAX_POPUP_ROWS.min(self.page_len().max(1))
    }

    fn open_selected_event(&mut self) {
        let Some(event_name) = self.selected_event() else {
            return;
        };
        self.page = HooksBrowserPage::Handlers(event_name);
        self.state = ScrollState::new();
        if self.page_len() > 0 {
            self.state.selected_idx = Some(0);
        }
    }

    fn toggle_selected_hook(&mut self, event_name: HookEventName) {
        let Some(idx) = self.selected_hook_index(event_name) else {
            return;
        };
        let Some(hook) = self.hooks.get_mut(idx) else {
            return;
        };
        if hook.is_managed {
            return;
        }

        hook.enabled = !hook.enabled;
        self.app_event_tx.send(AppEvent::SetHookEnabled {
            key: hook.key.clone(),
            enabled: hook.enabled,
        });
    }

    fn close(&mut self) {
        self.complete = true;
    }

    fn return_to_events(&mut self) {
        let selected_event_name = match self.page {
            HooksBrowserPage::Events => None,
            HooksBrowserPage::Handlers(event_name) => Some(event_name),
        };
        self.page = HooksBrowserPage::Events;
        self.state = ScrollState::new();
        self.state.selected_idx = selected_event_name
            .and_then(|event_name| {
                codex_protocol::protocol::HookEventName::iter()
                    .position(|candidate| HookEventName::from(candidate) == event_name)
            })
            .or_else(|| (self.page_len() > 0).then_some(0));
    }

    fn event_header_lines() -> Vec<Line<'static>> {
        vec![
            "Hooks".bold().into(),
            "Lifecycle hooks from config and enabled plugins."
                .dim()
                .into(),
        ]
    }

    fn handler_header_lines(event_name: HookEventName) -> Vec<Line<'static>> {
        vec![
            format!("{} hooks", event_label(event_name)).bold().into(),
            "Turn hooks on or off. Your changes are saved automatically."
                .dim()
                .into(),
        ]
    }

    fn event_table_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            format!("{:<EVENT_COLUMN_WIDTH$}", "Event").into(),
            format!("{:<COUNT_COLUMN_WIDTH$}", "Installed").into(),
            format!("{:<COUNT_COLUMN_WIDTH$}", "Active").into(),
            "Description".into(),
        ]));
        for (idx, row) in self.event_rows().into_iter().enumerate() {
            if self.state.selected_idx == Some(idx) {
                lines.push(Line::from(vec![
                    Span::from(format!(
                        "{:<EVENT_COLUMN_WIDTH$}",
                        event_label(row.event_name)
                    ))
                    .cyan()
                    .bold(),
                    Span::from(format!("{:<COUNT_COLUMN_WIDTH$}", row.installed))
                        .cyan()
                        .bold(),
                    Span::from(format!("{:<COUNT_COLUMN_WIDTH$}", row.active))
                        .cyan()
                        .bold(),
                    Span::from(event_description(row.event_name)).cyan().bold(),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::from(format!(
                        "{:<EVENT_COLUMN_WIDTH$}",
                        event_label(row.event_name)
                    )),
                    Span::from(format!("{:<COUNT_COLUMN_WIDTH$}", row.installed)).dim(),
                    Span::from(format!("{:<COUNT_COLUMN_WIDTH$}", row.active)).dim(),
                    Span::from(event_description(row.event_name)).dim(),
                ]));
            }
        }
        lines
    }

    fn event_issue_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        if self.warnings.is_empty() && self.errors.is_empty() {
            return lines;
        }

        lines.push("Issues".bold().into());
        lines.extend(
            self.warnings
                .iter()
                .map(|warning| format!("⚠ {warning}").into()),
        );
        lines.extend(self.errors.iter().map(|error| {
            format!("■ {}: {}", error.path.display(), error.message)
                .red()
                .into()
        }));
        lines
    }

    fn event_page_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Self::event_header_lines();
        lines.push(Line::default());

        let issue_lines = self.event_issue_lines();
        if !issue_lines.is_empty() {
            lines.extend(issue_lines);
            lines.push(Line::default());
        }

        lines.extend(self.event_table_lines());
        lines
    }

    fn handler_row_lines(&self, event_name: HookEventName, width: usize) -> Vec<Line<'static>> {
        self.handlers_for_event(event_name)
            .enumerate()
            .map(|(idx, hook)| {
                let marker = if hook.enabled || hook.is_managed {
                    'x'
                } else {
                    ' '
                };
                let row = format!("[{marker}] {}", hook_title(idx));
                let mut line = Line::from(row);
                line = truncate_line_with_ellipsis_if_overflow(line, width);
                if hook.is_managed {
                    line = line.dim();
                }
                if self.state.selected_idx == Some(idx) {
                    line = line.cyan().bold();
                }
                line
            })
            .collect()
    }

    fn detail_lines(&self, event_name: HookEventName, width: usize) -> Vec<Line<'static>> {
        let Some(hook) = self.selected_hook(event_name) else {
            return vec!["No hooks installed for this event.".dim().into()];
        };

        let mut lines = vec![detail_line("Event", event_label(event_name))];
        if let Some(matcher) = hook.matcher.as_deref() {
            lines.extend(detail_wrapped_lines(
                "Matcher", matcher, width, /*max_lines*/ None,
            ));
        }
        lines.extend(detail_wrapped_lines(
            "Source",
            &detail_source_value(hook),
            width,
            /*max_lines*/ None,
        ));
        lines.extend(detail_wrapped_lines(
            "Command",
            hook.command.as_deref().unwrap_or("-"),
            width,
            Some(MAX_COMMAND_DETAIL_LINES),
        ));
        lines.push(detail_line("Timeout", &format!("{}s", hook.timeout_sec)));
        lines
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let hint_area = Rect {
            x: area.x + 2,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        };
        let footer = match self.page {
            HooksBrowserPage::Events => Line::from(vec![
                "Press ".into(),
                key_hint::plain(KeyCode::Enter).into(),
                " to view hooks; ".into(),
                key_hint::plain(KeyCode::Esc).into(),
                " to close".into(),
            ]),
            HooksBrowserPage::Handlers(event_name) => {
                let selected_hook = self.selected_hook(event_name);
                if selected_hook.is_none() {
                    Line::from(vec![
                        "Press ".into(),
                        key_hint::plain(KeyCode::Esc).into(),
                        " to go back".into(),
                    ])
                } else if selected_hook.is_some_and(|hook| hook.is_managed) {
                    Line::from(vec![
                        "Managed hooks are always on; press ".into(),
                        key_hint::plain(KeyCode::Esc).into(),
                        " to go back".into(),
                    ])
                } else {
                    Line::from(vec![
                        "Press ".into(),
                        key_hint::plain(KeyCode::Char(' ')).into(),
                        " or ".into(),
                        key_hint::plain(KeyCode::Enter).into(),
                        " to toggle; ".into(),
                        key_hint::plain(KeyCode::Esc).into(),
                        " to go back".into(),
                    ])
                }
            }
        };
        footer.dim().render(hint_area, buf);
    }
}

impl BottomPaneView for HooksBrowserView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.move_up(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.move_down(),
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } if self.page == HooksBrowserPage::Events => self.open_selected_event(),
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let HooksBrowserPage::Handlers(event_name) = self.page {
                    self.toggle_selected_hook(event_name);
                }
            }
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let HooksBrowserPage::Handlers(event_name) = self.page {
                    self.toggle_selected_hook(event_name);
                }
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => match self.page {
                HooksBrowserPage::Events => self.close(),
                HooksBrowserPage::Handlers(_) => self.return_to_events(),
            },
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.close();
        CancellationEvent::Handled
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }
}

impl Renderable for HooksBrowserView {
    fn desired_height(&self, width: u16) -> u16 {
        let content_width = width.saturating_sub(4) as usize;
        let height = match self.page {
            HooksBrowserPage::Events => self.event_page_lines().len(),
            HooksBrowserPage::Handlers(event_name) => {
                let row_count = self.handler_row_lines(event_name, content_width).len();
                if row_count == 0 {
                    Self::handler_header_lines(event_name).len() + 2
                } else {
                    let visible_row_count = row_count.min(MAX_POPUP_ROWS);
                    Self::handler_header_lines(event_name).len()
                        + 1
                        + visible_row_count
                        + 1
                        + self.detail_lines(event_name, content_width).len()
                }
            }
        };
        (height + 3).try_into().unwrap_or(u16::MAX)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let [content_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        let content_area = render_menu_surface(content_area, buf);
        let width = content_area.width as usize;
        let lines = match self.page {
            HooksBrowserPage::Events => self.event_page_lines(),
            HooksBrowserPage::Handlers(event_name) => {
                let mut lines = Self::handler_header_lines(event_name);
                let rows = self.handler_row_lines(event_name, width);
                if rows.is_empty() {
                    lines.push(Line::default());
                    lines.push(Line::from(
                        "No hooks installed for this event.".dim().italic(),
                    ));
                    lines.push(Line::default());
                    Paragraph::new(lines).render(content_area, buf);
                    self.render_footer(footer_area, buf);
                    return;
                }
                let list_height = rows.len().clamp(1, MAX_POPUP_ROWS) as u16;
                lines.push(Line::default());
                let header_height = lines.len() as u16;
                let [header_area, list_area, detail_area] = Layout::vertical([
                    Constraint::Length(header_height),
                    Constraint::Length(list_height),
                    Constraint::Fill(1),
                ])
                .areas(content_area);
                Paragraph::new(lines.clone()).render(header_area, buf);
                let visible_rows = rows
                    .into_iter()
                    .skip(self.state.scroll_top)
                    .take(list_height as usize)
                    .collect::<Vec<_>>();
                Paragraph::new(visible_rows).render(list_area, buf);
                let mut detail_lines = vec![Line::default()];
                detail_lines.extend(self.detail_lines(event_name, width));
                Paragraph::new(detail_lines).render(detail_area, buf);
                self.render_footer(footer_area, buf);
                return;
            }
        };
        Paragraph::new(lines).render(content_area, buf);
        self.render_footer(footer_area, buf);
    }
}

struct EventRow {
    event_name: HookEventName,
    installed: usize,
    active: usize,
}

fn event_label(event_name: HookEventName) -> &'static str {
    match event_name {
        HookEventName::PreToolUse => "PreToolUse",
        HookEventName::PermissionRequest => "PermissionRequest",
        HookEventName::PostToolUse => "PostToolUse",
        HookEventName::SessionStart => "SessionStart",
        HookEventName::UserPromptSubmit => "UserPromptSubmit",
        HookEventName::Stop => "Stop",
    }
}

fn event_description(event_name: HookEventName) -> &'static str {
    match event_name {
        HookEventName::PreToolUse => "Before a tool executes",
        HookEventName::PermissionRequest => "When permission is requested",
        HookEventName::PostToolUse => "After a tool executes",
        HookEventName::SessionStart => "When a new session starts",
        HookEventName::UserPromptSubmit => "When the user submits a prompt",
        HookEventName::Stop => "Right before Codex ends its turn",
    }
}

fn hook_title(idx: usize) -> String {
    format!("Hook {}", idx + 1)
}

fn hook_source_summary(hook: &HookMetadata) -> String {
    match hook.source {
        HookSource::Plugin => hook
            .plugin_id
            .as_deref()
            .map(|plugin_id| format!("Plugin - {plugin_id}"))
            .unwrap_or_else(|| "Plugin".to_string()),
        _ => config_source_label(hook.source).to_string(),
    }
}

fn detail_source_value(hook: &HookMetadata) -> String {
    match hook.source {
        HookSource::Plugin => hook_source_summary(hook),
        HookSource::System
        | HookSource::Mdm
        | HookSource::CloudRequirements
        | HookSource::LegacyManagedConfigFile
        | HookSource::LegacyManagedConfigMdm => config_source_label(hook.source).to_string(),
        _ => format!(
            "{} - {}",
            config_source_label(hook.source),
            format_directory_display(&hook.source_path, /*max_width*/ None)
        ),
    }
}

fn config_source_label(source: HookSource) -> &'static str {
    match source {
        HookSource::System => "Admin config",
        HookSource::User => "User config",
        HookSource::Project => "Project config",
        HookSource::Mdm => "Admin config",
        HookSource::SessionFlags => "Session flags",
        HookSource::Plugin => unreachable!("plugin hooks are handled by summary_source"),
        HookSource::CloudRequirements => "Admin config",
        HookSource::LegacyManagedConfigFile => "Admin config",
        HookSource::LegacyManagedConfigMdm => "Admin config",
        HookSource::Unknown => "Unknown source",
    }
}

fn detail_line(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![format!("{label:<10}").into(), value.to_string().dim()])
}

fn detail_wrapped_lines(
    label: &str,
    value: &str,
    width: usize,
    max_lines: Option<usize>,
) -> Vec<Line<'static>> {
    let prefix = format!("{label:<10}");
    let available = width.saturating_sub(prefix.width()).max(1);
    let mut wrapped = textwrap::wrap(value, available).into_iter();
    let first = wrapped.next().unwrap_or_default().into_owned();
    let mut lines = vec![Line::from(vec![prefix.into(), first.dim()])];
    lines
        .extend(wrapped.map(|line| Line::from(vec!["          ".into(), line.into_owned().dim()])));
    let Some(max_lines) = max_lines else {
        return lines;
    };
    if lines.len() <= max_lines {
        return lines;
    }

    lines.truncate(max_lines);
    if let Some(last_line) = lines.last_mut() {
        let prefix_width = last_line.spans[..last_line.spans.len().saturating_sub(1)]
            .iter()
            .map(ratatui::prelude::Span::width)
            .sum::<usize>();
        let max_width = width.saturating_sub(prefix_width);
        let Some(last_span) = last_line.spans.last_mut() else {
            return lines;
        };
        let truncated = truncate_line_with_ellipsis_if_overflow(
            Line::from(format!("{}…", last_span.content)),
            max_width,
        );
        let content = truncated
            .spans
            .into_iter()
            .map(|span| span.content.into_owned())
            .collect::<String>();
        last_span.content = content.into();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use crate::bottom_pane::bottom_pane_view::BottomPaneView;
    use crate::render::renderable::Renderable;
    use crate::test_support::PathBufExt;
    use crate::test_support::test_path_buf;
    use crate::test_support::test_path_display;
    use codex_app_server_protocol::HookEventName;
    use codex_app_server_protocol::HookHandlerType;
    use codex_app_server_protocol::HookMetadata;
    use codex_app_server_protocol::HookSource;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use insta::assert_snapshot;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::unbounded_channel;

    fn render_lines(view: &HooksBrowserView, width: u16) -> String {
        let height = view.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        (0..area.height)
            .map(|row| {
                let rendered = (0..area.width)
                    .map(|col| {
                        let symbol = buf[(area.x + col, area.y + row)].symbol();
                        if symbol.is_empty() {
                            " ".to_string()
                        } else {
                            symbol.to_string()
                        }
                    })
                    .collect::<String>();
                let normalized = rendered
                    .replace(&test_path_display("/tmp/hooks.json"), "/tmp/hooks.json")
                    .replace(&test_path_display("/tmp/h.json"), "/tmp/h.json");
                format!("{normalized:width$}", width = area.width as usize)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[allow(clippy::too_many_arguments)]
    fn hook(
        key: &str,
        event_name: HookEventName,
        source: HookSource,
        plugin_id: Option<&str>,
        command: &str,
        enabled: bool,
        is_managed: bool,
        display_order: i64,
    ) -> HookMetadata {
        HookMetadata {
            key: key.to_string(),
            event_name,
            handler_type: HookHandlerType::Command,
            is_managed,
            matcher: Some("Bash".to_string()),
            command: Some(command.to_string()),
            timeout_sec: 30,
            status_message: None,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source,
            plugin_id: plugin_id.map(str::to_string),
            display_order,
            enabled,
        }
    }

    fn view() -> HooksBrowserView {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        HooksBrowserView::new(
            vec![
                hook(
                    "plugin:superpowers",
                    HookEventName::PreToolUse,
                    HookSource::Plugin,
                    Some("superpowers@openai-curated"),
                    "${CODEX_PLUGIN_ROOT}/hooks/pre-tool-use-check.sh",
                    /*enabled*/ true,
                    /*is_managed*/ false,
                    /*display_order*/ 0,
                ),
                hook(
                    "path:user-config",
                    HookEventName::PreToolUse,
                    HookSource::User,
                    /*plugin_id*/ None,
                    "~/bin/check-shell-with-a-command-that-is-way-too-long-for-the-summary-column.sh",
                    /*enabled*/ false,
                    /*is_managed*/ false,
                    /*display_order*/ 1,
                ),
                hook(
                    "path:managed",
                    HookEventName::PermissionRequest,
                    HookSource::System,
                    /*plugin_id*/ None,
                    "/enterprise/hooks/permission-check.sh",
                    /*enabled*/ true,
                    /*is_managed*/ true,
                    /*display_order*/ 2,
                ),
            ],
            Vec::new(),
            Vec::new(),
            AppEventSender::new(tx_raw),
        )
    }

    #[test]
    fn renders_event_browser() {
        let view = view();
        assert_snapshot!("hooks_browser_events", render_lines(&view, /*width*/ 112));
    }

    #[test]
    fn renders_event_browser_with_issues() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let view = HooksBrowserView::new(
            Vec::new(),
            vec!["skipped invalid matcher for PreToolUse".to_string()],
            vec![HookErrorInfo {
                path: test_path_buf("/tmp/hooks.json"),
                message: "failed to parse hooks config".to_string(),
            }],
            AppEventSender::new(tx_raw),
        );

        assert_snapshot!(
            "hooks_browser_events_with_issues",
            render_lines(&view, /*width*/ 112)
        );
    }

    #[test]
    fn renders_handler_browser_with_details() {
        let mut view = view();
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_snapshot!("hooks_browser_handlers", render_lines(&view, /*width*/ 112));
    }

    #[test]
    fn renders_managed_handler_without_toggle_hint() {
        let mut view = view();
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_snapshot!(
            "hooks_browser_managed_handler",
            render_lines(&view, /*width*/ 112)
        );
    }

    #[test]
    fn renders_selected_managed_handler() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let mut view = HooksBrowserView::new(
            vec![
                hook(
                    "path:managed-1",
                    HookEventName::PreToolUse,
                    HookSource::System,
                    /*plugin_id*/ None,
                    "/enterprise/hooks/pre-tool-use-1.sh",
                    /*enabled*/ true,
                    /*is_managed*/ true,
                    /*display_order*/ 0,
                ),
                hook(
                    "path:managed-2",
                    HookEventName::PreToolUse,
                    HookSource::System,
                    /*plugin_id*/ None,
                    "/enterprise/hooks/pre-tool-use-2.sh",
                    /*enabled*/ true,
                    /*is_managed*/ true,
                    /*display_order*/ 1,
                ),
            ],
            Vec::new(),
            Vec::new(),
            AppEventSender::new(tx_raw),
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        assert_snapshot!(
            "hooks_browser_selected_managed_handler",
            render_lines(&view, /*width*/ 112)
        );
    }

    #[test]
    fn renders_scrolled_handler_window() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let hooks = (0..=MAX_POPUP_ROWS)
            .map(|idx| {
                hook(
                    &format!("path:hook-{idx}"),
                    HookEventName::PreToolUse,
                    HookSource::User,
                    /*plugin_id*/ None,
                    &format!("/tmp/hook-{idx}.sh"),
                    /*enabled*/ true,
                    /*is_managed*/ false,
                    idx as i64,
                )
            })
            .collect();
        let mut view =
            HooksBrowserView::new(hooks, Vec::new(), Vec::new(), AppEventSender::new(tx_raw));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        for _ in 0..MAX_POPUP_ROWS {
            view.handle_key_event(KeyEvent::from(KeyCode::Down));
        }
        assert_snapshot!(
            "hooks_browser_scrolled_handlers",
            render_lines(&view, /*width*/ 112)
        );
    }

    #[test]
    fn renders_command_details_with_three_line_cap() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let mut capped_command_hook = hook(
            "path:long-command",
            HookEventName::PreToolUse,
            HookSource::User,
            /*plugin_id*/ None,
            "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty",
            /*enabled*/ true,
            /*is_managed*/ false,
            /*display_order*/ 0,
        );
        capped_command_hook.source_path = test_path_buf("/tmp/h.json").abs();
        let mut view = HooksBrowserView::new(
            vec![capped_command_hook],
            Vec::new(),
            Vec::new(),
            AppEventSender::new(tx_raw),
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_snapshot!(
            "hooks_browser_capped_command_details",
            render_lines(&view, /*width*/ 44)
        );
    }

    #[test]
    fn renders_empty_handler_browser_message() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let mut view = HooksBrowserView::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            AppEventSender::new(tx_raw),
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert_snapshot!(
            "hooks_browser_empty_handlers",
            render_lines(&view, /*width*/ 112)
        );
    }

    #[test]
    fn managed_hooks_count_as_active() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let view = HooksBrowserView::new(
            vec![hook(
                "path:managed",
                HookEventName::PreToolUse,
                HookSource::System,
                /*plugin_id*/ None,
                "/enterprise/hooks/pre-tool-use-check.sh",
                /*enabled*/ false,
                /*is_managed*/ true,
                /*display_order*/ 0,
            )],
            Vec::new(),
            Vec::new(),
            AppEventSender::new(tx_raw),
        );

        let rows = view.event_rows();
        let pre_tool_use = rows
            .into_iter()
            .find(|row| row.event_name == HookEventName::PreToolUse)
            .expect("pre tool use row");

        assert_eq!(pre_tool_use.installed, 1);
        assert_eq!(pre_tool_use.active, 1);
    }

    fn assert_unmanaged_toggle_key(key_code: KeyCode) {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let mut view = HooksBrowserView::new(
            vec![hook(
                "plugin:superpowers",
                HookEventName::PreToolUse,
                HookSource::Plugin,
                Some("superpowers@openai-curated"),
                "hooks/pre-tool-use-check.sh",
                /*enabled*/ true,
                /*is_managed*/ false,
                /*display_order*/ 0,
            )],
            Vec::new(),
            Vec::new(),
            AppEventSender::new(tx_raw),
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        view.handle_key_event(KeyEvent::from(key_code));

        match rx.try_recv().expect("toggle event") {
            AppEvent::SetHookEnabled { key, enabled } => {
                assert_eq!(key, "plugin:superpowers");
                assert!(!enabled);
            }
            other => panic!("expected hook toggle event, got {other:?}"),
        }
    }

    #[test]
    fn toggle_keys_toggle_unmanaged_handler() {
        for key_code in [KeyCode::Char(' '), KeyCode::Enter] {
            assert_unmanaged_toggle_key(key_code);
        }
    }

    #[test]
    fn space_does_not_toggle_managed_handler() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let mut view = HooksBrowserView::new(
            vec![hook(
                "path:managed",
                HookEventName::PreToolUse,
                HookSource::System,
                /*plugin_id*/ None,
                "/enterprise/hooks/pre-tool-use-check.sh",
                /*enabled*/ true,
                /*is_managed*/ true,
                /*display_order*/ 0,
            )],
            Vec::new(),
            Vec::new(),
            AppEventSender::new(tx_raw),
        );
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        view.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));

        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn escape_returns_to_the_selected_event() {
        let mut view = view();
        view.handle_key_event(KeyEvent::from(KeyCode::Down));
        view.handle_key_event(KeyEvent::from(KeyCode::Enter));
        view.handle_key_event(KeyEvent::from(KeyCode::Esc));

        assert_eq!(view.page, HooksBrowserPage::Events);
        assert_eq!(
            view.selected_event(),
            Some(HookEventName::PermissionRequest)
        );
    }

    #[test]
    fn esc_routes_through_the_view() {
        assert!(view().prefer_esc_to_handle_key_event());
    }
}
