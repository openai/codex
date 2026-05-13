#[cfg(test)]
use crate::app_command::AppCommand as Op;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_protocol::ThreadId;
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
use textwrap::wrap;
use url::Url;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;
use crate::app::app_server_requests::ResolvedAppServerRequest;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::ListKeymap;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::style::user_message_style;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_lines;

const MCP_CODEX_APPS_SERVER_NAME: &str = "codex_apps";
const MCP_TOOL_CODEX_APPS_META_KEY: &str = "_codex_apps";
const CONNECTOR_AUTH_FAILURE_META_KEY: &str = "connector_auth_failure";
const CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY: &str = "is_auth_failure";
const CONNECTOR_AUTH_FAILURE_CONNECTOR_ID_KEY: &str = "connector_id";
const CONNECTOR_AUTH_FAILURE_CONNECTOR_NAME_KEY: &str = "connector_name";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppLinkScreen {
    Link,
    InstallConfirmation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppLinkSuggestionType {
    Install,
    Enable,
    Auth,
    ExternalAction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppLinkElicitationTarget {
    pub(crate) thread_id: ThreadId,
    pub(crate) server_name: String,
    pub(crate) request_id: AppServerRequestId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AppLinkViewParams {
    pub(crate) app_id: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) instructions: String,
    pub(crate) url: String,
    pub(crate) is_installed: bool,
    pub(crate) is_enabled: bool,
    pub(crate) suggest_reason: Option<String>,
    pub(crate) suggestion_type: Option<AppLinkSuggestionType>,
    pub(crate) elicitation_target: Option<AppLinkElicitationTarget>,
}

impl AppLinkViewParams {
    pub(crate) fn from_url_app_server_request(
        thread_id: ThreadId,
        server_name: &str,
        request_id: AppServerRequestId,
        request: &codex_app_server_protocol::McpServerElicitationRequest,
    ) -> Option<Self> {
        let codex_app_server_protocol::McpServerElicitationRequest::Url {
            meta,
            message,
            url,
            elicitation_id,
        } = request
        else {
            return None;
        };
        if server_name == MCP_CODEX_APPS_SERVER_NAME {
            let url = validate_external_url(url, /*require_chatgpt_host*/ true)?;
            return Self::from_codex_apps_auth_url_parts(
                thread_id,
                server_name,
                request_id,
                meta.as_ref(),
                message,
                url.as_str(),
                elicitation_id,
            );
        }

        let url = validate_external_url(url, /*require_chatgpt_host*/ false)?;
        Some(Self::from_generic_url_parts(
            thread_id,
            server_name,
            request_id,
            message,
            url.as_str(),
            elicitation_id,
        ))
    }

    fn from_codex_apps_auth_url_parts(
        thread_id: ThreadId,
        server_name: &str,
        request_id: AppServerRequestId,
        meta: Option<&serde_json::Value>,
        message: &str,
        url: &str,
        elicitation_id: &str,
    ) -> Option<Self> {
        let auth_failure = meta?
            .as_object()?
            .get(MCP_TOOL_CODEX_APPS_META_KEY)?
            .as_object()?
            .get(CONNECTOR_AUTH_FAILURE_META_KEY)?
            .as_object()?;
        if auth_failure
            .get(CONNECTOR_AUTH_FAILURE_IS_AUTH_FAILURE_KEY)
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            return None;
        }

        let app_id = auth_failure
            .get(CONNECTOR_AUTH_FAILURE_CONNECTOR_ID_KEY)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(elicitation_id)
            .to_string();
        let title = auth_failure
            .get(CONNECTOR_AUTH_FAILURE_CONNECTOR_NAME_KEY)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(app_id.as_str())
            .to_string();

        Some(Self {
            app_id,
            title,
            description: None,
            instructions: "Sign in to this app in your browser, then return here.".to_string(),
            url: url.to_string(),
            is_installed: true,
            is_enabled: true,
            suggest_reason: Some(message.to_string()),
            suggestion_type: Some(AppLinkSuggestionType::Auth),
            elicitation_target: Some(AppLinkElicitationTarget {
                thread_id,
                server_name: server_name.to_string(),
                request_id,
            }),
        })
    }

    fn from_generic_url_parts(
        thread_id: ThreadId,
        server_name: &str,
        request_id: AppServerRequestId,
        message: &str,
        url: &str,
        elicitation_id: &str,
    ) -> Self {
        Self {
            app_id: elicitation_id.to_string(),
            title: "Action required".to_string(),
            description: Some(format!("Server: {server_name}")),
            instructions: "Complete the requested action in your browser, then return here."
                .to_string(),
            url: url.to_string(),
            is_installed: true,
            is_enabled: true,
            suggest_reason: Some(message.to_string()),
            suggestion_type: Some(AppLinkSuggestionType::ExternalAction),
            elicitation_target: Some(AppLinkElicitationTarget {
                thread_id,
                server_name: server_name.to_string(),
                request_id,
            }),
        }
    }
}

fn validate_external_url(url: &str, require_chatgpt_host: bool) -> Option<Url> {
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return None;
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    if require_chatgpt_host && !is_allowed_chatgpt_auth_host(parsed.host_str()?) {
        return None;
    }
    Some(parsed)
}

fn is_allowed_chatgpt_auth_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "chatgpt.com"
        || host == "chatgpt-staging.com"
        || host.ends_with(".chatgpt.com")
        || host.ends_with(".chatgpt-staging.com")
}

pub(crate) struct AppLinkView {
    app_id: String,
    title: String,
    description: Option<String>,
    instructions: String,
    url: String,
    is_installed: bool,
    is_enabled: bool,
    suggest_reason: Option<String>,
    suggestion_type: Option<AppLinkSuggestionType>,
    elicitation_target: Option<AppLinkElicitationTarget>,
    app_event_tx: AppEventSender,
    screen: AppLinkScreen,
    selected_action: usize,
    complete: bool,
    list_keymap: ListKeymap,
}

impl AppLinkView {
    #[cfg(test)]
    pub(crate) fn new(params: AppLinkViewParams, app_event_tx: AppEventSender) -> Self {
        Self::new_with_keymap(
            params,
            app_event_tx,
            crate::keymap::RuntimeKeymap::defaults().list,
        )
    }

    pub(crate) fn new_with_keymap(
        params: AppLinkViewParams,
        app_event_tx: AppEventSender,
        list_keymap: ListKeymap,
    ) -> Self {
        let AppLinkViewParams {
            app_id,
            title,
            description,
            instructions,
            url,
            is_installed,
            is_enabled,
            suggest_reason,
            suggestion_type,
            elicitation_target,
        } = params;
        Self {
            app_id,
            title,
            description,
            instructions,
            url,
            is_installed,
            is_enabled,
            suggest_reason,
            suggestion_type,
            elicitation_target,
            app_event_tx,
            screen: AppLinkScreen::Link,
            selected_action: 0,
            complete: false,
            list_keymap,
        }
    }

    fn action_labels(&self) -> Vec<&'static str> {
        if self.is_auth_suggestion() {
            return match self.screen {
                AppLinkScreen::Link => vec!["Open sign-in URL", "Back"],
                AppLinkScreen::InstallConfirmation => vec!["I already signed in", "Back"],
            };
        }
        if self.is_external_action_suggestion() {
            return match self.screen {
                AppLinkScreen::Link => vec!["Open link", "Back"],
                AppLinkScreen::InstallConfirmation => vec!["I finished", "Back"],
            };
        }

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

    fn is_tool_suggestion(&self) -> bool {
        self.elicitation_target.is_some()
    }

    fn is_auth_suggestion(&self) -> bool {
        self.is_tool_suggestion() && self.suggestion_type == Some(AppLinkSuggestionType::Auth)
    }

    fn is_external_action_suggestion(&self) -> bool {
        self.is_tool_suggestion()
            && self.suggestion_type == Some(AppLinkSuggestionType::ExternalAction)
    }

    fn is_browser_action_suggestion(&self) -> bool {
        self.is_auth_suggestion() || self.is_external_action_suggestion()
    }

    fn resolve_elicitation(&self, decision: McpServerElicitationAction) {
        let Some(target) = self.elicitation_target.as_ref() else {
            return;
        };
        self.app_event_tx.resolve_elicitation(
            target.thread_id,
            target.server_name.clone(),
            target.request_id.clone(),
            decision,
            /*content*/ None,
            /*meta*/ None,
        );
    }

    fn decline_tool_suggestion(&mut self) {
        self.resolve_elicitation(McpServerElicitationAction::Decline);
        self.complete = true;
    }

    fn open_external_url(&mut self) {
        self.app_event_tx.send(AppEvent::OpenUrlInBrowser {
            url: self.url.clone(),
        });
        if !self.is_installed || self.is_browser_action_suggestion() {
            self.screen = AppLinkScreen::InstallConfirmation;
            self.selected_action = 0;
        }
    }

    fn complete_external_flow_and_close(&mut self) {
        let should_refresh_connectors = self
            .elicitation_target
            .as_ref()
            .is_none_or(|target| target.server_name == MCP_CODEX_APPS_SERVER_NAME);
        if should_refresh_connectors {
            self.app_event_tx.send(AppEvent::RefreshConnectors {
                force_refetch: true,
            });
        }
        if self.is_tool_suggestion() {
            self.resolve_elicitation(McpServerElicitationAction::Accept);
        }
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
        if self.is_tool_suggestion() {
            self.resolve_elicitation(McpServerElicitationAction::Accept);
            self.complete = true;
        }
    }

    fn activate_selected_action(&mut self) {
        if self.is_tool_suggestion() {
            match self.suggestion_type {
                Some(AppLinkSuggestionType::Enable) => match self.screen {
                    AppLinkScreen::Link => match self.selected_action {
                        0 => self.open_external_url(),
                        1 if self.is_installed => self.toggle_enabled(),
                        _ => self.decline_tool_suggestion(),
                    },
                    AppLinkScreen::InstallConfirmation => match self.selected_action {
                        0 => self.complete_external_flow_and_close(),
                        _ => self.decline_tool_suggestion(),
                    },
                },
                Some(AppLinkSuggestionType::Auth) => match self.screen {
                    AppLinkScreen::Link => match self.selected_action {
                        0 => self.open_external_url(),
                        _ => self.decline_tool_suggestion(),
                    },
                    AppLinkScreen::InstallConfirmation => match self.selected_action {
                        0 => self.complete_external_flow_and_close(),
                        _ => self.decline_tool_suggestion(),
                    },
                },
                Some(AppLinkSuggestionType::ExternalAction) => match self.screen {
                    AppLinkScreen::Link => match self.selected_action {
                        0 => self.open_external_url(),
                        _ => self.decline_tool_suggestion(),
                    },
                    AppLinkScreen::InstallConfirmation => match self.selected_action {
                        0 => self.complete_external_flow_and_close(),
                        _ => self.decline_tool_suggestion(),
                    },
                },
                Some(AppLinkSuggestionType::Install) | None => match self.screen {
                    AppLinkScreen::Link => match self.selected_action {
                        0 => self.open_external_url(),
                        _ => self.decline_tool_suggestion(),
                    },
                    AppLinkScreen::InstallConfirmation => match self.selected_action {
                        0 => self.complete_external_flow_and_close(),
                        _ => self.decline_tool_suggestion(),
                    },
                },
            }
            return;
        }

        match self.screen {
            AppLinkScreen::Link => match self.selected_action {
                0 => self.open_external_url(),
                1 if self.is_installed => self.toggle_enabled(),
                _ => self.complete = true,
            },
            AppLinkScreen::InstallConfirmation => match self.selected_action {
                0 => self.complete_external_flow_and_close(),
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

        lines.push(Line::from(""));
        if let Some(suggest_reason) = self
            .suggest_reason
            .as_deref()
            .map(str::trim)
            .filter(|suggest_reason| !suggest_reason.is_empty())
        {
            for line in wrap(suggest_reason, usable_width) {
                lines.push(Line::from(line.into_owned().italic()));
            }
            lines.push(Line::from(""));
        }
        let is_browser_action_suggestion = self.is_browser_action_suggestion();
        if self.is_installed && !is_browser_action_suggestion {
            for line in wrap("Use $ to insert this app into the prompt.", usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
            lines.push(Line::from(""));
        }

        if is_browser_action_suggestion {
            lines.push(Line::from("URL".dim()));
            for line in wrap(&self.url, usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
            lines.push(Line::from(""));
        }

        let instructions = self.instructions.trim();
        if !instructions.is_empty() {
            for line in wrap(instructions, usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
            if !is_browser_action_suggestion {
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
            }
            lines.push(Line::from(""));
        }

        lines
    }

    fn install_confirmation_lines(&self, width: u16) -> Vec<Line<'static>> {
        let usable_width = width.max(1) as usize;
        let mut lines: Vec<Line<'static>> = Vec::new();

        let is_auth_suggestion = self.is_auth_suggestion();
        let is_external_action_suggestion = self.is_external_action_suggestion();
        let is_codex_apps_auth = is_auth_suggestion
            && self
                .elicitation_target
                .as_ref()
                .is_some_and(|target| target.server_name == MCP_CODEX_APPS_SERVER_NAME);
        lines.push(Line::from(
            if is_auth_suggestion {
                if is_codex_apps_auth {
                    "Finish App Sign In"
                } else {
                    "Finish Authentication"
                }
            } else if is_external_action_suggestion {
                "Finish in Browser"
            } else {
                "Finish App Setup"
            }
            .bold(),
        ));
        lines.push(Line::from(""));

        if is_auth_suggestion {
            for line in wrap(
                if is_codex_apps_auth {
                    "Sign in to the app on ChatGPT in the browser window that just opened."
                } else {
                    "Complete authentication in the browser window that just opened."
                },
                usable_width,
            ) {
                lines.push(Line::from(line.into_owned()));
            }
            for line in wrap(
                "Then return here and select \"I already signed in\".",
                usable_width,
            ) {
                lines.push(Line::from(line.into_owned()));
            }
        } else if is_external_action_suggestion {
            for line in wrap(
                "Complete the requested action in the browser window that just opened.",
                usable_width,
            ) {
                lines.push(Line::from(line.into_owned()));
            }
            for line in wrap("Then return here and select \"I finished\".", usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
        } else {
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
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            if is_auth_suggestion {
                "Sign-in URL:"
            } else if is_external_action_suggestion {
                "Link:"
            } else {
                "Setup URL:"
            }
            .dim(),
        ]));
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
                code: KeyCode::BackTab,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_selection_prev(),
            _ if self.list_keymap.move_left.is_pressed(key_event) => self.move_selection_prev(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Tab, ..
            }
            | KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.move_selection_next(),
            _ if self.list_keymap.move_right.is_pressed(key_event) => self.move_selection_next(),
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
        if self.is_tool_suggestion() {
            self.resolve_elicitation(McpServerElicitationAction::Decline);
        }
        self.complete = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn dismiss_app_server_request(&mut self, request: &ResolvedAppServerRequest) -> bool {
        let ResolvedAppServerRequest::McpElicitation {
            server_name,
            request_id,
        } = request
        else {
            return false;
        };
        let Some(target) = self.elicitation_target.as_ref() else {
            return false;
        };
        if target.server_name != *server_name || target.request_id != *request_id {
            return false;
        }

        self.complete = true;
        true
    }

    fn terminal_title_requires_action(&self) -> bool {
        self.is_tool_suggestion()
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

        let inner = content_area.inset(Insets::vh(/*v*/ 1, /*h*/ 2));
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
    use crate::app::app_server_requests::ResolvedAppServerRequest;
    use crate::app_event::AppEvent;
    use crate::render::renderable::Renderable;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    fn suggestion_target() -> AppLinkElicitationTarget {
        AppLinkElicitationTarget {
            thread_id: ThreadId::try_from("00000000-0000-0000-0000-000000000001")
                .expect("valid thread id"),
            server_name: "codex_apps".to_string(),
            request_id: AppServerRequestId::String("request-1".to_string()),
        }
    }

    fn generic_url_target() -> AppLinkElicitationTarget {
        AppLinkElicitationTarget {
            thread_id: ThreadId::try_from("00000000-0000-0000-0000-000000000002")
                .expect("valid thread id"),
            server_name: "payments".to_string(),
            request_id: AppServerRequestId::String("request-2".to_string()),
        }
    }

    fn auth_url_request(url: &str) -> codex_app_server_protocol::McpServerElicitationRequest {
        codex_app_server_protocol::McpServerElicitationRequest::Url {
            meta: Some(serde_json::json!({
                "_codex_apps": {
                    "connector_auth_failure": {
                        "is_auth_failure": true,
                        "connector_id": "connector_calendar",
                        "connector_name": "Google Calendar",
                    },
                },
            })),
            message: "Reconnect Google Calendar on ChatGPT.".to_string(),
            url: url.to_string(),
            elicitation_id: "codex_apps_auth_call_123".to_string(),
        }
    }

    #[test]
    fn codex_apps_auth_url_elicitation_builds_auth_app_link_params() {
        let target = suggestion_target();
        let request =
            auth_url_request("https://chatgpt.com/apps/google-calendar/connector_calendar");

        let params = AppLinkViewParams::from_url_app_server_request(
            target.thread_id,
            &target.server_name,
            target.request_id.clone(),
            &request,
        )
        .expect("expected auth app link params");

        assert_eq!(params.app_id, "connector_calendar");
        assert_eq!(params.title, "Google Calendar");
        assert_eq!(
            params.url,
            "https://chatgpt.com/apps/google-calendar/connector_calendar"
        );
        assert_eq!(params.suggestion_type, Some(AppLinkSuggestionType::Auth));
        assert_eq!(params.elicitation_target, Some(target));
    }

    #[test]
    fn non_codex_apps_url_elicitation_builds_generic_app_link_params() {
        let target = generic_url_target();
        let request = codex_app_server_protocol::McpServerElicitationRequest::Url {
            meta: None,
            message: "Review the payment details to continue.".to_string(),
            url: "https://payments.example/checkout/123".to_string(),
            elicitation_id: "payment-123".to_string(),
        };

        let params = AppLinkViewParams::from_url_app_server_request(
            target.thread_id,
            &target.server_name,
            target.request_id.clone(),
            &request,
        )
        .expect("expected generic URL app link params");

        assert_eq!(
            params,
            AppLinkViewParams {
                app_id: "payment-123".to_string(),
                title: "Action required".to_string(),
                description: Some("Server: payments".to_string()),
                instructions: "Complete the requested action in your browser, then return here."
                    .to_string(),
                url: "https://payments.example/checkout/123".to_string(),
                is_installed: true,
                is_enabled: true,
                suggest_reason: Some("Review the payment details to continue.".to_string()),
                suggestion_type: Some(AppLinkSuggestionType::ExternalAction),
                elicitation_target: Some(target),
            }
        );
    }

    #[test]
    fn codex_apps_auth_url_elicitation_rejects_untrusted_urls() {
        let target = suggestion_target();
        for url in [
            "http://chatgpt.com/apps/google-calendar/connector_calendar",
            "https://user:pass@chatgpt.com/apps/google-calendar/connector_calendar",
            "https://chatgpt.com.evil.example/apps/google-calendar/connector_calendar",
            "https://evilchatgpt.com/apps/google-calendar/connector_calendar",
        ] {
            let request = auth_url_request(url);
            let params = AppLinkViewParams::from_url_app_server_request(
                target.thread_id,
                &target.server_name,
                target.request_id.clone(),
                &request,
            );
            assert!(params.is_none(), "expected {url} to be rejected");
        }
    }

    #[test]
    fn generic_url_elicitation_rejects_untrusted_urls() {
        let target = generic_url_target();
        for url in [
            "http://payments.example/checkout/123",
            "https://user:pass@payments.example/checkout/123",
        ] {
            let request = codex_app_server_protocol::McpServerElicitationRequest::Url {
                meta: None,
                message: "Review the payment details to continue.".to_string(),
                url: url.to_string(),
                elicitation_id: "payment-123".to_string(),
            };
            let params = AppLinkViewParams::from_url_app_server_request(
                target.thread_id,
                &target.server_name,
                target.request_id.clone(),
                &request,
            );
            assert!(params.is_none(), "expected {url} to be rejected");
        }
    }

    fn render_snapshot(view: &AppLinkView, area: Rect) -> String {
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        (0..area.height)
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
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn installed_app_has_toggle_action() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: "https://example.test/notion".to_string(),
                is_installed: true,
                is_enabled: true,
                suggest_reason: None,
                suggestion_type: None,
                elicitation_target: None,
            },
            tx,
        );

        assert_eq!(
            view.action_labels(),
            vec!["Manage on ChatGPT", "Disable app", "Back"]
        );
    }

    #[test]
    fn regular_app_link_does_not_require_terminal_title_action() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: "https://example.test/notion".to_string(),
                is_installed: true,
                is_enabled: true,
                suggest_reason: None,
                suggestion_type: None,
                elicitation_target: None,
            },
            tx,
        );

        assert!(!view.terminal_title_requires_action());
    }

    #[test]
    fn tool_suggestion_requires_terminal_title_action() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_google_calendar".to_string(),
                title: "Google Calendar".to_string(),
                description: Some("Plan events and schedules.".to_string()),
                instructions: "Enable this app to use it for the current request.".to_string(),
                url: "https://example.test/google-calendar".to_string(),
                is_installed: true,
                is_enabled: false,
                suggest_reason: Some("Plan and reference events from your calendar".to_string()),
                suggestion_type: Some(AppLinkSuggestionType::Enable),
                elicitation_target: Some(suggestion_target()),
            },
            tx,
        );

        assert!(view.terminal_title_requires_action());
    }

    #[test]
    fn horizontal_list_keys_move_action_selection() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: "https://example.test/notion".to_string(),
                is_installed: true,
                is_enabled: true,
                suggest_reason: None,
                suggestion_type: None,
                elicitation_target: None,
            },
            tx,
        );

        assert_eq!(view.selected_action, 0);
        view.handle_key_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        assert_eq!(view.selected_action, 1);
        view.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        assert_eq!(view.selected_action, 0);
    }

    #[test]
    fn remapped_horizontal_list_keys_control_action_selection() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut list_keymap = crate::keymap::RuntimeKeymap::defaults().list;
        list_keymap.move_left = vec![key_hint::plain(KeyCode::Char('x'))];
        list_keymap.move_right = vec![key_hint::plain(KeyCode::Char('z'))];
        let mut view = AppLinkView::new_with_keymap(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: "https://example.test/notion".to_string(),
                is_installed: true,
                is_enabled: true,
                suggest_reason: None,
                suggestion_type: None,
                elicitation_target: None,
            },
            tx,
            list_keymap,
        );

        assert_eq!(view.selected_action, 0);
        view.handle_key_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        assert_eq!(view.selected_action, 0);
        view.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(view.selected_action, 0);

        view.handle_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
        assert_eq!(view.selected_action, 1);
        view.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert_eq!(view.selected_action, 1);
        view.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(view.selected_action, 1);

        view.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(view.selected_action, 0);
    }

    #[test]
    fn toggle_action_sends_set_app_enabled_and_updates_label() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: "https://example.test/notion".to_string(),
                is_installed: true,
                is_enabled: true,
                suggest_reason: None,
                suggestion_type: None,
                elicitation_target: None,
            },
            tx,
        );

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
    fn generic_url_elicitation_resolves_without_connector_refresh() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let target = generic_url_target();
        let request = codex_app_server_protocol::McpServerElicitationRequest::Url {
            meta: None,
            message: "Review the payment details to continue.".to_string(),
            url: "https://payments.example/checkout/123".to_string(),
            elicitation_id: "payment-123".to_string(),
        };
        let params = AppLinkViewParams::from_url_app_server_request(
            target.thread_id,
            &target.server_name,
            target.request_id.clone(),
            &request,
        )
        .expect("expected generic URL app link params");
        let mut view = AppLinkView::new(params, tx);

        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match rx.try_recv() {
            Ok(AppEvent::OpenUrlInBrowser { url }) => {
                assert_eq!(url, "https://payments.example/checkout/123");
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }
        assert_eq!(view.screen, AppLinkScreen::InstallConfirmation);

        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match rx.try_recv() {
            Ok(AppEvent::SubmitThreadOp { thread_id, op }) => {
                assert_eq!(thread_id, target.thread_id);
                assert_eq!(
                    op,
                    Op::ResolveElicitation {
                        server_name: "payments".to_string(),
                        request_id: AppServerRequestId::String("request-2".to_string()),
                        decision: McpServerElicitationAction::Accept,
                        content: None,
                        meta: None,
                    }
                );
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }
        assert!(rx.try_recv().is_err());
        assert!(view.is_complete());
    }

    #[test]
    fn install_confirmation_does_not_split_long_url_like_token_without_scheme() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let url_like =
            "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890";
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: url_like.to_string(),
                is_installed: true,
                is_enabled: true,
                suggest_reason: None,
                suggestion_type: None,
                elicitation_target: None,
            },
            tx,
        );
        view.screen = AppLinkScreen::InstallConfirmation;

        let rendered: Vec<String> = view
            .content_lines(/*width*/ 40)
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
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_1".to_string(),
                title: "Notion".to_string(),
                description: None,
                instructions: "Manage app".to_string(),
                url: url.to_string(),
                is_installed: true,
                is_enabled: true,
                suggest_reason: None,
                suggestion_type: None,
                elicitation_target: None,
            },
            tx,
        );
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
    fn install_tool_suggestion_resolves_elicitation_after_confirmation() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_google_calendar".to_string(),
                title: "Google Calendar".to_string(),
                description: Some("Plan events and schedules.".to_string()),
                instructions: "Install this app in your browser, then return here.".to_string(),
                url: "https://example.test/google-calendar".to_string(),
                is_installed: false,
                is_enabled: false,
                suggest_reason: Some("Plan and reference events from your calendar".to_string()),
                suggestion_type: Some(AppLinkSuggestionType::Install),
                elicitation_target: Some(suggestion_target()),
            },
            tx,
        );

        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match rx.try_recv() {
            Ok(AppEvent::OpenUrlInBrowser { url }) => {
                assert_eq!(url, "https://example.test/google-calendar".to_string());
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }
        assert_eq!(view.screen, AppLinkScreen::InstallConfirmation);

        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match rx.try_recv() {
            Ok(AppEvent::RefreshConnectors { force_refetch }) => {
                assert!(force_refetch);
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }
        match rx.try_recv() {
            Ok(AppEvent::SubmitThreadOp { thread_id, op }) => {
                assert_eq!(thread_id, suggestion_target().thread_id);
                assert_eq!(
                    op,
                    Op::ResolveElicitation {
                        server_name: "codex_apps".to_string(),
                        request_id: AppServerRequestId::String("request-1".to_string()),
                        decision: McpServerElicitationAction::Accept,
                        content: None,
                        meta: None,
                    }
                );
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }
        assert!(view.is_complete());
    }

    #[test]
    fn declined_tool_suggestion_resolves_elicitation_decline() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_google_calendar".to_string(),
                title: "Google Calendar".to_string(),
                description: None,
                instructions: "Install this app in your browser, then return here.".to_string(),
                url: "https://example.test/google-calendar".to_string(),
                is_installed: false,
                is_enabled: false,
                suggest_reason: Some("Plan and reference events from your calendar".to_string()),
                suggestion_type: Some(AppLinkSuggestionType::Install),
                elicitation_target: Some(suggestion_target()),
            },
            tx,
        );

        view.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));

        match rx.try_recv() {
            Ok(AppEvent::SubmitThreadOp { thread_id, op }) => {
                assert_eq!(thread_id, suggestion_target().thread_id);
                assert_eq!(
                    op,
                    Op::ResolveElicitation {
                        server_name: "codex_apps".to_string(),
                        request_id: AppServerRequestId::String("request-1".to_string()),
                        decision: McpServerElicitationAction::Decline,
                        content: None,
                        meta: None,
                    }
                );
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }
        assert!(view.is_complete());
    }

    #[test]
    fn enable_tool_suggestion_resolves_elicitation_after_enable() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_google_calendar".to_string(),
                title: "Google Calendar".to_string(),
                description: Some("Plan events and schedules.".to_string()),
                instructions: "Enable this app to use it for the current request.".to_string(),
                url: "https://example.test/google-calendar".to_string(),
                is_installed: true,
                is_enabled: false,
                suggest_reason: Some("Plan and reference events from your calendar".to_string()),
                suggestion_type: Some(AppLinkSuggestionType::Enable),
                elicitation_target: Some(suggestion_target()),
            },
            tx,
        );

        view.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));

        match rx.try_recv() {
            Ok(AppEvent::SetAppEnabled { id, enabled }) => {
                assert_eq!(id, "connector_google_calendar");
                assert!(enabled);
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }
        match rx.try_recv() {
            Ok(AppEvent::SubmitThreadOp { thread_id, op }) => {
                assert_eq!(thread_id, suggestion_target().thread_id);
                assert_eq!(
                    op,
                    Op::ResolveElicitation {
                        server_name: "codex_apps".to_string(),
                        request_id: AppServerRequestId::String("request-1".to_string()),
                        decision: McpServerElicitationAction::Accept,
                        content: None,
                        meta: None,
                    }
                );
            }
            Ok(other) => panic!("unexpected app event: {other:?}"),
            Err(err) => panic!("missing app event: {err}"),
        }
        assert!(view.is_complete());
    }

    #[test]
    fn resolved_tool_suggestion_dismisses_matching_view() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_google_calendar".to_string(),
                title: "Google Calendar".to_string(),
                description: Some("Plan events and schedules.".to_string()),
                instructions: "Enable this app to use it for the current request.".to_string(),
                url: "https://example.test/google-calendar".to_string(),
                is_installed: true,
                is_enabled: false,
                suggest_reason: Some("Plan and reference events from your calendar".to_string()),
                suggestion_type: Some(AppLinkSuggestionType::Enable),
                elicitation_target: Some(suggestion_target()),
            },
            tx,
        );

        assert!(
            view.dismiss_app_server_request(&ResolvedAppServerRequest::McpElicitation {
                server_name: "codex_apps".to_string(),
                request_id: AppServerRequestId::String("request-1".to_string()),
            })
        );
        assert!(view.is_complete());
    }

    #[test]
    fn resolved_tool_suggestion_ignores_non_matching_request() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_google_calendar".to_string(),
                title: "Google Calendar".to_string(),
                description: Some("Plan events and schedules.".to_string()),
                instructions: "Enable this app to use it for the current request.".to_string(),
                url: "https://example.test/google-calendar".to_string(),
                is_installed: true,
                is_enabled: false,
                suggest_reason: Some("Plan and reference events from your calendar".to_string()),
                suggestion_type: Some(AppLinkSuggestionType::Enable),
                elicitation_target: Some(suggestion_target()),
            },
            tx,
        );

        assert!(
            !view.dismiss_app_server_request(&ResolvedAppServerRequest::McpElicitation {
                server_name: "other_server".to_string(),
                request_id: AppServerRequestId::String("request-1".to_string()),
            })
        );
        assert!(!view.is_complete());
    }

    #[test]
    fn install_suggestion_with_reason_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_google_calendar".to_string(),
                title: "Google Calendar".to_string(),
                description: Some("Plan events and schedules.".to_string()),
                instructions: "Install this app in your browser, then return here.".to_string(),
                url: "https://example.test/google-calendar".to_string(),
                is_installed: false,
                is_enabled: false,
                suggest_reason: Some("Plan and reference events from your calendar".to_string()),
                suggestion_type: Some(AppLinkSuggestionType::Install),
                elicitation_target: Some(suggestion_target()),
            },
            tx,
        );

        assert_snapshot!(
            "app_link_view_install_suggestion_with_reason",
            render_snapshot(
                &view,
                Rect::new(0, 0, 72, view.desired_height(/*width*/ 72))
            )
        );
    }

    #[test]
    fn enable_suggestion_with_reason_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_google_calendar".to_string(),
                title: "Google Calendar".to_string(),
                description: Some("Plan events and schedules.".to_string()),
                instructions: "Enable this app to use it for the current request.".to_string(),
                url: "https://example.test/google-calendar".to_string(),
                is_installed: true,
                is_enabled: false,
                suggest_reason: Some("Plan and reference events from your calendar".to_string()),
                suggestion_type: Some(AppLinkSuggestionType::Enable),
                elicitation_target: Some(suggestion_target()),
            },
            tx,
        );

        assert_snapshot!(
            "app_link_view_enable_suggestion_with_reason",
            render_snapshot(
                &view,
                Rect::new(0, 0, 72, view.desired_height(/*width*/ 72))
            )
        );
    }

    #[test]
    fn auth_suggestion_with_reason_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let view = AppLinkView::new(
            AppLinkViewParams {
                app_id: "connector_google_calendar".to_string(),
                title: "Google Calendar".to_string(),
                description: None,
                instructions: "Sign in to this app in your browser, then return here.".to_string(),
                url: "https://chatgpt.com/apps/google-calendar/connector_google_calendar"
                    .to_string(),
                is_installed: true,
                is_enabled: true,
                suggest_reason: Some("Reconnect Google Calendar on ChatGPT.".to_string()),
                suggestion_type: Some(AppLinkSuggestionType::Auth),
                elicitation_target: Some(suggestion_target()),
            },
            tx,
        );

        assert_snapshot!(
            "app_link_view_auth_suggestion_with_reason",
            render_snapshot(
                &view,
                Rect::new(0, 0, 72, view.desired_height(/*width*/ 72))
            )
        );
    }

    #[test]
    fn generic_url_elicitation_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let target = generic_url_target();
        let request = codex_app_server_protocol::McpServerElicitationRequest::Url {
            meta: None,
            message: "Review the payment details to continue.".to_string(),
            url: "https://payments.example/checkout/123".to_string(),
            elicitation_id: "payment-123".to_string(),
        };
        let params = AppLinkViewParams::from_url_app_server_request(
            target.thread_id,
            &target.server_name,
            target.request_id.clone(),
            &request,
        )
        .expect("expected generic URL app link params");
        let view = AppLinkView::new(params, tx);

        assert_snapshot!(
            "app_link_view_generic_url_elicitation",
            render_snapshot(
                &view,
                Rect::new(0, 0, 72, view.desired_height(/*width*/ 72))
            )
        );
    }

    #[test]
    fn generic_url_elicitation_confirmation_snapshot() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let target = generic_url_target();
        let request = codex_app_server_protocol::McpServerElicitationRequest::Url {
            meta: None,
            message: "Review the payment details to continue.".to_string(),
            url: "https://payments.example/checkout/123".to_string(),
            elicitation_id: "payment-123".to_string(),
        };
        let params = AppLinkViewParams::from_url_app_server_request(
            target.thread_id,
            &target.server_name,
            target.request_id.clone(),
            &request,
        )
        .expect("expected generic URL app link params");
        let mut view = AppLinkView::new(params, tx);

        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_snapshot!(
            "app_link_view_generic_url_elicitation_confirmation",
            render_snapshot(
                &view,
                Rect::new(0, 0, 72, view.desired_height(/*width*/ 72))
            )
        );
    }
}
