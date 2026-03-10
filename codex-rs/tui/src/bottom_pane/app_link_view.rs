use codex_protocol::ThreadId;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::mcp::RequestId as McpRequestId;
use codex_protocol::protocol::Op;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use textwrap::wrap;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::style::user_message_style;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_lines;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppLinkScreen {
    Link,
    InstallConfirmation,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ToolSuggestionToolType {
    Connector,
    Plugin,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ToolSuggestionType {
    Install,
    Enable,
}

impl ToolSuggestionType {
    fn decision(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Enable => "enable",
        }
    }
}

const TOOL_SUGGESTION_META_KIND_VALUE: &str = "tool_suggestion";
pub(crate) const APP_LINK_INSTALL_INSTRUCTIONS: &str =
    "Install this app in your browser, then reload Codex.";
const APP_LINK_ENABLE_INSTRUCTIONS: &str =
    "Enable this app in Codex to use its tools in this session.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppLinkElicitationResolution {
    pub(crate) thread_id: ThreadId,
    pub(crate) server_name: String,
    pub(crate) request_id: McpRequestId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppLinkViewParams {
    pub(crate) app_id: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) suggest_reason: Option<String>,
    pub(crate) instructions: String,
    pub(crate) url: String,
    pub(crate) is_installed: bool,
    pub(crate) is_enabled: bool,
    pub(crate) suggestion_type: Option<ToolSuggestionType>,
    pub(crate) elicitation_resolution: Option<AppLinkElicitationResolution>,
}

#[derive(Deserialize)]
struct ToolSuggestionMeta {
    #[serde(rename = "codex_approval_kind")]
    approval_kind: String,
    tool_type: ToolSuggestionToolType,
    suggestion_type: ToolSuggestionType,
    connector_id: String,
    connector_name: String,
    suggest_reason: Option<String>,
    connector_description: Option<String>,
    install_url: String,
}

pub(crate) fn tool_suggestion_params_from_event(
    thread_id: ThreadId,
    request: &ElicitationRequestEvent,
) -> Option<AppLinkViewParams> {
    let ElicitationRequest::Form {
        meta: Some(meta), ..
    } = &request.request
    else {
        return None;
    };

    let meta = serde_json::from_value::<ToolSuggestionMeta>(meta.clone()).ok()?;
    if meta.approval_kind != TOOL_SUGGESTION_META_KIND_VALUE
        || meta.tool_type != ToolSuggestionToolType::Connector
    {
        return None;
    }

    let (instructions, is_installed, is_enabled) = match meta.suggestion_type {
        ToolSuggestionType::Install => (APP_LINK_INSTALL_INSTRUCTIONS.to_string(), false, false),
        ToolSuggestionType::Enable => (APP_LINK_ENABLE_INSTRUCTIONS.to_string(), true, false),
    };

    Some(AppLinkViewParams {
        app_id: meta.connector_id,
        title: meta.connector_name,
        description: meta.connector_description,
        suggest_reason: meta.suggest_reason,
        instructions,
        url: meta.install_url,
        is_installed,
        is_enabled,
        suggestion_type: Some(meta.suggestion_type),
        elicitation_resolution: Some(AppLinkElicitationResolution {
            thread_id,
            server_name: request.server_name.clone(),
            request_id: request.id.clone(),
        }),
    })
}

pub(crate) struct AppLinkView {
    app_id: String,
    title: String,
    description: Option<String>,
    suggest_reason: Option<String>,
    instructions: String,
    url: String,
    is_installed: bool,
    is_enabled: bool,
    suggestion_type: Option<ToolSuggestionType>,
    elicitation_resolution: Option<AppLinkElicitationResolution>,
    app_event_tx: AppEventSender,
    screen: AppLinkScreen,
    selected_action: usize,
    complete: bool,
}

impl AppLinkView {
    pub(crate) fn new(params: AppLinkViewParams, app_event_tx: AppEventSender) -> Self {
        let AppLinkViewParams {
            app_id,
            title,
            description,
            suggest_reason,
            instructions,
            url,
            is_installed,
            is_enabled,
            suggestion_type,
            elicitation_resolution,
        } = params;
        Self {
            app_id,
            title,
            description,
            suggest_reason,
            instructions,
            url,
            is_installed,
            is_enabled,
            suggestion_type,
            elicitation_resolution,
            app_event_tx,
            screen: AppLinkScreen::Link,
            selected_action: 0,
            complete: false,
        }
    }

    fn action_labels(&self) -> Vec<&'static str> {
        match self.screen {
            AppLinkScreen::Link => {
                if self.is_installed {
                    vec![
                        "Manage on ChatGPT",
                        if self.is_enabled {
                            "Disable app"
                        } else {
                            "Enable app"
                        },
                        "Back",
                    ]
                } else {
                    vec!["Install on ChatGPT", "Back"]
                }
            }
            AppLinkScreen::InstallConfirmation => vec!["I already Installed it", "Back"],
        }
    }

    fn move_selection_prev(&mut self) {
        self.selected_action = self.selected_action.saturating_sub(1);
    }

    fn move_selection_next(&mut self) {
        self.selected_action = (self.selected_action + 1).min(self.action_labels().len() - 1);
    }

    fn open_chatgpt_link(&mut self) {
        self.app_event_tx.send(AppEvent::OpenUrlInBrowser {
            url: self.url.clone(),
        });
        if !self.is_installed {
            self.screen = AppLinkScreen::InstallConfirmation;
            self.selected_action = 0;
        }
    }

    fn refresh_connectors_and_close(&mut self) {
        self.app_event_tx.send(AppEvent::RefreshConnectors {
            force_refetch: true,
        });
        self.resolve_elicitation(
            ElicitationAction::Accept,
            Some(json!({
                "decision": self
                    .suggestion_type
                    .unwrap_or(ToolSuggestionType::Install)
                    .decision(),
            })),
        );
        self.complete = true;
    }

    fn close_flow(&mut self) {
        self.resolve_elicitation(ElicitationAction::Cancel, None);
        self.complete = true;
    }

    fn back_to_link_screen(&mut self) {
        self.screen = AppLinkScreen::Link;
        self.selected_action = 0;
    }

    fn toggle_enabled(&mut self) {
        self.is_enabled = !self.is_enabled;
        self.app_event_tx.send(AppEvent::SetAppEnabled {
            id: self.app_id.clone(),
            enabled: self.is_enabled,
        });
        if self.is_enabled
            && self.elicitation_resolution.is_some()
            && self.suggestion_type == Some(ToolSuggestionType::Enable)
        {
            self.resolve_elicitation(
                ElicitationAction::Accept,
                Some(json!({
                    "decision": ToolSuggestionType::Enable.decision(),
                })),
            );
            self.complete = true;
        }
    }

    fn resolve_elicitation(&self, decision: ElicitationAction, content: Option<Value>) {
        let Some(resolution) = self.elicitation_resolution.as_ref() else {
            return;
        };

        self.app_event_tx.send(AppEvent::SubmitThreadOp {
            thread_id: resolution.thread_id,
            op: Op::ResolveElicitation {
                server_name: resolution.server_name.clone(),
                request_id: resolution.request_id.clone(),
                decision,
                content,
                meta: None,
            },
        });
    }

    fn activate_selected_action(&mut self) {
        match self.screen {
            AppLinkScreen::Link => match self.selected_action {
                0 => self.open_chatgpt_link(),
                1 if self.is_installed => self.toggle_enabled(),
                _ => self.close_flow(),
            },
            AppLinkScreen::InstallConfirmation => match self.selected_action {
                0 => self.refresh_connectors_and_close(),
                _ => self.back_to_link_screen(),
            },
        }
    }

    fn content_lines(&self, width: u16) -> Vec<Line<'static>> {
        match self.screen {
            AppLinkScreen::Link => self.link_content_lines(width),
            AppLinkScreen::InstallConfirmation => self.install_confirmation_lines(width),
        }
    }

    fn link_content_lines(&self, width: u16) -> Vec<Line<'static>> {
        let usable_width = width.max(1) as usize;
        let mut lines: Vec<Line<'static>> = Vec::new();

        lines.push(Line::from(self.title.clone().bold()));
        if let Some(description) = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|description| !description.is_empty())
        {
            for line in wrap(description, usable_width) {
                lines.push(Line::from(line.into_owned().dim()));
            }
        }
        if let Some(suggest_reason) = self
            .suggest_reason
            .as_deref()
            .map(str::trim)
            .filter(|suggest_reason| !suggest_reason.is_empty())
        {
            let suggest_reason = format!("Reason: {suggest_reason}");
            for line in wrap(&suggest_reason, usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
        }

        lines.push(Line::from(""));
        if self.is_installed {
            for line in wrap("Use $ to insert this app into the prompt.", usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
            lines.push(Line::from(""));
        }

        let instructions = self.instructions.trim();
        if !instructions.is_empty() {
            for line in wrap(instructions, usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
            for line in wrap(
                "Newly installed apps can take a few minutes to appear in /apps.",
                usable_width,
            ) {
                lines.push(Line::from(line.into_owned()));
            }
            if !self.is_installed {
                for line in wrap(
                    "After installed, use $ to insert this app into the prompt.",
                    usable_width,
                ) {
                    lines.push(Line::from(line.into_owned()));
                }
            }
            lines.push(Line::from(""));
        }

        lines
    }

    fn install_confirmation_lines(&self, width: u16) -> Vec<Line<'static>> {
        let usable_width = width.max(1) as usize;
        let mut lines: Vec<Line<'static>> = Vec::new();

        lines.push(Line::from("Finish App Setup".bold()));
        lines.push(Line::from(""));

        for line in wrap(
            "Complete app setup on ChatGPT in the browser window that just opened.",
            usable_width,
        ) {
            lines.push(Line::from(line.into_owned()));
        }
        for line in wrap(
            "Sign in there if needed, then return here and select \"I already Installed it\".",
            usable_width,
        ) {
            lines.push(Line::from(line.into_owned()));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec!["Setup URL:".dim()]));
        let url_line = Line::from(vec![self.url.clone().cyan().underlined()]);
        lines.extend(adaptive_wrap_lines(
            vec![url_line],
            RtOptions::new(usable_width),
        ));

        lines
    }

    fn action_rows(&self) -> Vec<GenericDisplayRow> {
        self.action_labels()
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                let prefix = if self.selected_action == index {
                    '›'
                } else {
                    ' '
                };
                GenericDisplayRow {
                    name: format!("{prefix} {}. {label}", index + 1),
                    ..Default::default()
                }
            })
            .collect()
    }

    fn action_state(&self) -> ScrollState {
        let mut state = ScrollState::new();
        state.selected_idx = Some(self.selected_action);
        state
    }

    fn action_rows_height(&self, width: u16) -> u16 {
        let rows = self.action_rows();
        let state = self.action_state();
        measure_rows_height(&rows, &state, rows.len().max(1), width.max(1))
    }

    fn hint_line(&self) -> Line<'static> {
        Line::from(vec![
            "Use ".into(),
            key_hint::plain(KeyCode::Tab).into(),
            " / ".into(),
            key_hint::plain(KeyCode::Up).into(),
            " ".into(),
            key_hint::plain(KeyCode::Down).into(),
            " to move, ".into(),
            key_hint::plain(KeyCode::Enter).into(),
            " to select, ".into(),
            key_hint::plain(KeyCode::Esc).into(),
            " to close".into(),
        ])
    }
}

impl BottomPaneView for AppLinkView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Left,
                ..
            }
            | KeyEvent {
                code: KeyCode::BackTab,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_selection_prev(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Right,
                ..
            }
            | KeyEvent {
                code: KeyCode::Tab, ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_selection_next(),
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(index) = c
                    .to_digit(10)
                    .and_then(|digit| digit.checked_sub(1))
                    .map(|index| index as usize)
                    && index < self.action_labels().len()
                {
                    self.selected_action = index;
                    self.activate_selected_action();
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => self.activate_selected_action(),
            _ => {}
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.close_flow();
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }
}

impl crate::render::renderable::Renderable for AppLinkView {
    fn desired_height(&self, width: u16) -> u16 {
        let content_width = width.saturating_sub(4).max(1);
        let content_lines = self.content_lines(content_width);
        let content_rows = Paragraph::new(content_lines)
            .wrap(Wrap { trim: false })
            .line_count(content_width)
            .max(1) as u16;
        let action_rows_height = self.action_rows_height(content_width);
        content_rows + action_rows_height + 3
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        Block::default()
            .style(user_message_style())
            .render(area, buf);

        let actions_height = self.action_rows_height(area.width.saturating_sub(4));
        let [content_area, actions_area, hint_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(actions_height),
            Constraint::Length(1),
        ])
        .areas(area);

        let inner = content_area.inset(Insets::vh(1, 2));
        let content_width = inner.width.max(1);
        let lines = self.content_lines(content_width);
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);

        if actions_area.height > 0 {
            let actions_area = Rect {
                x: actions_area.x.saturating_add(2),
                y: actions_area.y,
                width: actions_area.width.saturating_sub(2),
                height: actions_area.height,
            };
            let action_rows = self.action_rows();
            let action_state = self.action_state();
            render_rows(
                actions_area,
                buf,
                &action_rows,
                &action_state,
                action_rows.len().max(1),
                "No actions",
            );
        }

        if hint_area.height > 0 {
            let hint_area = Rect {
                x: hint_area.x.saturating_add(2),
                y: hint_area.y,
                width: hint_area.width.saturating_sub(2),
                height: hint_area.height,
            };
            self.hint_line().dim().render(hint_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::render::renderable::Renderable;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    fn base_params() -> AppLinkViewParams {
        AppLinkViewParams {
            app_id: "connector_1".to_string(),
            title: "Notion".to_string(),
            description: None,
            suggest_reason: None,
            instructions: "Manage app".to_string(),
            url: "https://example.test/notion".to_string(),
            is_installed: true,
            is_enabled: true,
            suggestion_type: None,
            elicitation_resolution: None,
        }
    }

    fn render_snapshot(view: &AppLinkView, area: Rect) -> String {
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        format!("{buf:?}")
    }

    fn elicitation_resolution(thread_id: ThreadId) -> AppLinkElicitationResolution {
        AppLinkElicitationResolution {
            thread_id,
            server_name: "codex_apps".to_string(),
            request_id: McpRequestId::String("request-1".to_string()),
        }
    }

    #[test]
    fn installed_app_has_toggle_action() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = AppLinkView::new(base_params(), tx);

        assert_eq!(
            view.action_labels(),
            vec!["Manage on ChatGPT", "Disable app", "Back"]
        );
    }

    #[test]
    fn toggle_action_sends_set_app_enabled_and_updates_label() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = AppLinkView::new(base_params(), tx);

        view.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));

        match rx.try_recv() {
            Ok(AppEvent::SetAppEnabled { id, enabled }) => {
                assert_eq!(id, "connector_1");
                assert!(!enabled);
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }

        assert_eq!(
            view.action_labels(),
            vec!["Manage on ChatGPT", "Enable app", "Back"]
        );
    }

    #[test]
    fn install_confirmation_does_not_split_long_url_like_token_without_scheme() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let url_like =
            "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890";
        let mut params = base_params();
        params.url = url_like.to_string();
        let mut view = AppLinkView::new(params, tx);
        view.screen = AppLinkScreen::InstallConfirmation;

        let rendered: Vec<String> = view
            .content_lines(40)
            .into_iter()
            .map(|line| {
                line.spans
                    .into_iter()
                    .map(|span| span.content.into_owned())
                    .collect::<String>()
            })
            .collect();

        assert_eq!(
            rendered
                .iter()
                .filter(|line| line.contains(url_like))
                .count(),
            1,
            "expected full URL-like token in one rendered line, got: {rendered:?}"
        );
    }

    #[test]
    fn install_confirmation_render_keeps_url_tail_visible_when_narrow() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let url = "https://example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890/artifacts/reports/performance/summary/detail/with/a/very/long/path/tail42";
        let mut params = base_params();
        params.url = url.to_string();
        let mut view = AppLinkView::new(params, tx);
        view.screen = AppLinkScreen::InstallConfirmation;

        let width: u16 = 36;
        let height = view.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        let rendered_blob = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| {
                        let symbol = buf[(x, y)].symbol();
                        if symbol.is_empty() {
                            ' '
                        } else {
                            symbol.chars().next().unwrap_or(' ')
                        }
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            rendered_blob.contains("tail42"),
            "expected wrapped setup URL tail to remain visible in narrow pane, got:\n{rendered_blob}"
        );
    }

    #[test]
    fn tool_install_suggestion_event_builds_app_link_params() {
        let thread_id = ThreadId::default();
        let request = ElicitationRequestEvent {
            turn_id: Some("turn-1".to_string()),
            server_name: "codex_apps".to_string(),
            id: McpRequestId::String("request-1".to_string()),
            request: ElicitationRequest::Form {
                meta: Some(json!({
                    "codex_approval_kind": "tool_suggestion",
                    "tool_type": "connector",
                    "suggestion_type": "install",
                    "connector_id": "connector_1",
                    "connector_name": "Notion",
                    "suggest_reason": "The user asked for workspace docs",
                    "connector_description": "Docs and notes",
                    "install_url": "https://example.test/notion",
                })),
                message: "Install Notion to continue?".to_string(),
                requested_schema: json!({
                    "type": "object",
                    "properties": {},
                }),
            },
        };

        let params = tool_suggestion_params_from_event(thread_id, &request);

        assert_eq!(
            params,
            Some(AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: Some("Docs and notes".to_string()),
                suggest_reason: Some("The user asked for workspace docs".to_string()),
                instructions: APP_LINK_INSTALL_INSTRUCTIONS.to_string(),
                url: "https://example.test/notion".to_string(),
                is_installed: false,
                is_enabled: false,
                suggestion_type: Some(ToolSuggestionType::Install),
                elicitation_resolution: Some(elicitation_resolution(thread_id)),
            })
        );
    }

    #[test]
    fn tool_enable_suggestion_event_builds_app_link_params() {
        let thread_id = ThreadId::default();
        let request = ElicitationRequestEvent {
            turn_id: Some("turn-1".to_string()),
            server_name: "codex_apps".to_string(),
            id: McpRequestId::String("request-1".to_string()),
            request: ElicitationRequest::Form {
                meta: Some(json!({
                    "codex_approval_kind": "tool_suggestion",
                    "tool_type": "connector",
                    "suggestion_type": "enable",
                    "connector_id": "connector_1",
                    "connector_name": "Notion",
                    "connector_description": "Docs and notes",
                    "install_url": "https://example.test/notion",
                })),
                message: "Enable Notion to continue?".to_string(),
                requested_schema: json!({
                    "type": "object",
                    "properties": {},
                }),
            },
        };

        let params = tool_suggestion_params_from_event(thread_id, &request);

        assert_eq!(
            params,
            Some(AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: Some("Docs and notes".to_string()),
                suggest_reason: None,
                instructions: APP_LINK_ENABLE_INSTRUCTIONS.to_string(),
                url: "https://example.test/notion".to_string(),
                is_installed: true,
                is_enabled: false,
                suggestion_type: Some(ToolSuggestionType::Enable),
                elicitation_resolution: Some(elicitation_resolution(thread_id)),
            })
        );
    }

    #[test]
    fn enable_suggestion_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut params = base_params();
        params.is_enabled = false;
        params.instructions = APP_LINK_ENABLE_INSTRUCTIONS.to_string();
        params.suggestion_type = Some(ToolSuggestionType::Enable);
        let view = AppLinkView::new(params, tx);
        let area = Rect::new(0, 0, 80, view.desired_height(80));

        insta::assert_snapshot!(
            "app_link_view_enable_suggestion",
            render_snapshot(&view, area)
        );
    }

    #[test]
    fn install_suggestion_with_reason_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut params = base_params();
        params.is_installed = false;
        params.is_enabled = false;
        params.suggest_reason = Some("The user asked to access their workspace docs.".to_string());
        params.instructions = APP_LINK_INSTALL_INSTRUCTIONS.to_string();
        params.suggestion_type = Some(ToolSuggestionType::Install);
        let view = AppLinkView::new(params, tx);
        let area = Rect::new(0, 0, 80, view.desired_height(80));

        insta::assert_snapshot!(
            "app_link_view_install_suggestion_with_reason",
            render_snapshot(&view, area)
        );
    }

    #[test]
    fn install_confirmation_resolves_elicitation_after_refresh() {
        let expected_thread_id = ThreadId::default();
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut params = base_params();
        params.is_installed = false;
        params.is_enabled = false;
        params.instructions = APP_LINK_INSTALL_INSTRUCTIONS.to_string();
        params.suggestion_type = Some(ToolSuggestionType::Install);
        params.elicitation_resolution = Some(elicitation_resolution(expected_thread_id));
        let mut view = AppLinkView::new(params, tx);
        view.screen = AppLinkScreen::InstallConfirmation;

        view.activate_selected_action();

        match rx.try_recv() {
            Ok(AppEvent::RefreshConnectors { force_refetch }) => {
                assert!(force_refetch);
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }

        match rx.try_recv() {
            Ok(AppEvent::SubmitThreadOp {
                thread_id,
                op:
                    Op::ResolveElicitation {
                        server_name,
                        request_id,
                        decision,
                        content,
                        meta,
                    },
            }) => {
                assert_eq!(thread_id, expected_thread_id);
                assert_eq!(server_name, "codex_apps");
                assert_eq!(request_id, McpRequestId::String("request-1".to_string()));
                assert_eq!(decision, ElicitationAction::Accept);
                assert_eq!(
                    content,
                    Some(json!({
                        "decision": "install",
                    }))
                );
                assert_eq!(meta, None);
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }

        assert!(view.is_complete());
    }

    #[test]
    fn closing_link_view_cancels_elicitation_flow() {
        let expected_thread_id = ThreadId::default();
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut params = base_params();
        params.is_installed = false;
        params.is_enabled = false;
        params.instructions = APP_LINK_INSTALL_INSTRUCTIONS.to_string();
        params.suggestion_type = Some(ToolSuggestionType::Install);
        params.elicitation_resolution = Some(elicitation_resolution(expected_thread_id));
        let mut view = AppLinkView::new(params, tx);
        view.selected_action = 1;

        view.activate_selected_action();

        match rx.try_recv() {
            Ok(AppEvent::SubmitThreadOp {
                thread_id,
                op:
                    Op::ResolveElicitation {
                        server_name,
                        request_id,
                        decision,
                        content,
                        meta,
                    },
            }) => {
                assert_eq!(thread_id, expected_thread_id);
                assert_eq!(server_name, "codex_apps");
                assert_eq!(request_id, McpRequestId::String("request-1".to_string()));
                assert_eq!(decision, ElicitationAction::Cancel);
                assert_eq!(content, None);
                assert_eq!(meta, None);
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }

        assert!(view.is_complete());
    }

    #[test]
    fn enabling_suggested_app_resolves_elicitation() {
        let expected_thread_id = ThreadId::default();
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut params = base_params();
        params.is_enabled = false;
        params.instructions = APP_LINK_ENABLE_INSTRUCTIONS.to_string();
        params.suggestion_type = Some(ToolSuggestionType::Enable);
        params.elicitation_resolution = Some(elicitation_resolution(expected_thread_id));
        let mut view = AppLinkView::new(params, tx);

        view.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));

        match rx.try_recv() {
            Ok(AppEvent::SetAppEnabled { id, enabled }) => {
                assert_eq!(id, "connector_1");
                assert!(enabled);
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }

        match rx.try_recv() {
            Ok(AppEvent::SubmitThreadOp {
                thread_id,
                op:
                    Op::ResolveElicitation {
                        server_name,
                        request_id,
                        decision,
                        content,
                        meta,
                    },
            }) => {
                assert_eq!(thread_id, expected_thread_id);
                assert_eq!(server_name, "codex_apps");
                assert_eq!(request_id, McpRequestId::String("request-1".to_string()));
                assert_eq!(decision, ElicitationAction::Accept);
                assert_eq!(
                    content,
                    Some(json!({
                        "decision": "enable",
                    }))
                );
                assert_eq!(meta, None);
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }

        assert!(view.is_complete());
    }
}
