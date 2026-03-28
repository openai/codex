#![allow(clippy::unwrap_used)]

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::AccountLoginCompletedNotification;
use codex_app_server_protocol::AccountUpdatedNotification;
use codex_app_server_protocol::AuthMode as AppServerAuthMode;
use codex_app_server_protocol::CancelLoginAccountParams;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::LoginAccountParams;
use codex_app_server_protocol::LoginAccountResponse;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::auth::read_openai_api_key_from_env;
use codex_login::DeviceCode;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use codex_protocol::config_types::ForcedLoginMethod;
use std::sync::Arc;
use std::sync::RwLock;
use uuid::Uuid;

use crate::LoginStatus;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::shimmer::shimmer_spans;
use crate::tui::FrameRequester;

/// Marks buffer cells that have cyan+underlined style as an OSC 8 hyperlink.
///
/// Terminal emulators recognise the OSC 8 escape sequence and treat the entire
/// marked region as a single clickable link, regardless of row wrapping.  This
/// is necessary because ratatui's cell-based rendering emits `MoveTo` at every
/// row boundary, which breaks normal terminal URL detection for long URLs that
/// wrap across multiple rows.
pub(crate) fn mark_url_hyperlink(buf: &mut Buffer, area: Rect, url: &str) {
    // Sanitize: strip any characters that could break out of the OSC 8
    // sequence (ESC or BEL) to prevent terminal escape injection from a
    // malformed or compromised upstream URL.
    let safe_url: String = url
        .chars()
        .filter(|&c| c != '\x1B' && c != '\x07')
        .collect();
    if safe_url.is_empty() {
        return;
    }

    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            // Only mark cells that carry the URL's distinctive style.
            if cell.fg != Color::Cyan || !cell.modifier.contains(Modifier::UNDERLINED) {
                continue;
            }
            let sym = cell.symbol().to_string();
            if sym.trim().is_empty() {
                continue;
            }
            cell.set_symbol(&format!("\x1B]8;;{safe_url}\x07{sym}\x1B]8;;\x07"));
        }
    }
}
use std::path::PathBuf;
use tokio::sync::Notify;

use super::onboarding_screen::StepState;

mod headless_chatgpt_login;

#[derive(Clone)]
pub(crate) enum SignInState {
    PickMode,
    ChatGptContinueInBrowser(ContinueInBrowserState),
    #[allow(dead_code)]
    ChatGptDeviceCode(ContinueWithDeviceCodeState),
    ChatGptSuccessMessage,
    ChatGptSuccess,
    ApiKeyEntry(ApiKeyInputState),
    ApiKeyConfigured,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SignInOption {
    ChatGpt,
    DeviceCode,
    ApiKey,
}

const API_KEY_DISABLED_MESSAGE: &str = "API key login is disabled.";
fn onboarding_request_id() -> codex_app_server_protocol::RequestId {
    codex_app_server_protocol::RequestId::String(Uuid::new_v4().to_string())
}

#[derive(Clone, Default)]
pub(crate) struct ApiKeyInputState {
    value: String,
    prepopulated_from_env: bool,
}

#[derive(Clone)]
/// Used to manage the lifecycle of SpawnedLogin and ensure it gets cleaned up.
pub(crate) struct ContinueInBrowserState {
    login_id: String,
    auth_url: String,
}

#[derive(Clone)]
pub(crate) struct ContinueWithDeviceCodeState {
    device_code: Option<DeviceCode>,
    cancel: Option<Arc<Notify>>,
}

impl KeyboardHandler for AuthModeWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.handle_api_key_entry_key_event(&key_event) {
            return;
        }

        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_highlight(/*delta*/ -1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_highlight(/*delta*/ 1);
            }
            KeyCode::Char('1') => {
                self.select_option_by_index(/*index*/ 0);
            }
            KeyCode::Char('2') => {
                self.select_option_by_index(/*index*/ 1);
            }
            KeyCode::Char('3') => {
                self.select_option_by_index(/*index*/ 2);
            }
            KeyCode::Enter => {
                let sign_in_state = { (*self.sign_in_state.read().unwrap()).clone() };
                match sign_in_state {
                    SignInState::PickMode => {
                        self.handle_sign_in_option(self.highlighted_mode);
                    }
                    SignInState::ChatGptSuccessMessage => {
                        *self.sign_in_state.write().unwrap() = SignInState::ChatGptSuccess;
                    }
                    _ => {}
                }
            }
            KeyCode::Esc => {
                tracing::info!("Esc pressed");
                self.cancel_active_attempt();
            }
            _ => {}
        }
    }

    fn handle_paste(&mut self, pasted: String) {
        let _ = self.handle_api_key_entry_paste(pasted);
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct AuthModeWidget {
    pub request_frame: FrameRequester,
    pub highlighted_mode: SignInOption,
    pub error: Arc<RwLock<Option<String>>>,
    pub sign_in_state: Arc<RwLock<SignInState>>,
    pub codex_home: PathBuf,
    pub cli_auth_credentials_store_mode: AuthCredentialsStoreMode,
    pub login_status: LoginStatus,
    pub app_server_request_handle: AppServerRequestHandle,
    pub forced_chatgpt_workspace_id: Option<String>,
    pub forced_login_method: Option<ForcedLoginMethod>,
    pub animations_enabled: bool,
}

impl AuthModeWidget {
    pub(crate) fn cancel_active_attempt(&self) {
        let mut sign_in_state = self.sign_in_state.write().unwrap();
        match &*sign_in_state {
            SignInState::ChatGptContinueInBrowser(state) => {
                let request_handle = self.app_server_request_handle.clone();
                let login_id = state.login_id.clone();
                tokio::spawn(async move {
                    let _ = request_handle
                        .request_typed::<codex_app_server_protocol::CancelLoginAccountResponse>(
                            ClientRequest::CancelLoginAccount {
                                request_id: onboarding_request_id(),
                                params: CancelLoginAccountParams { login_id },
                            },
                        )
                        .await;
                });
            }
            SignInState::ChatGptDeviceCode(state) => {
                if let Some(cancel) = &state.cancel {
                    cancel.notify_one();
                }
            }
            _ => return,
        }
        *sign_in_state = SignInState::PickMode;
        drop(sign_in_state);
        self.set_error(/*message*/ None);
        self.request_frame.schedule_frame();
    }

    fn set_error(&self, message: Option<String>) {
        *self.error.write().unwrap() = message;
    }

    fn error_message(&self) -> Option<String> {
        self.error.read().unwrap().clone()
    }

    fn is_api_login_allowed(&self) -> bool {
        !matches!(self.forced_login_method, Some(ForcedLoginMethod::Chatgpt))
    }

    fn is_chatgpt_login_allowed(&self) -> bool {
        !matches!(self.forced_login_method, Some(ForcedLoginMethod::Api))
    }

    fn displayed_sign_in_options(&self) -> Vec<SignInOption> {
        let mut options = vec![SignInOption::ChatGpt];
        if self.is_chatgpt_login_allowed() {
            options.push(SignInOption::DeviceCode);
        }
        if self.is_api_login_allowed() {
            options.push(SignInOption::ApiKey);
        }
        options
    }

    fn selectable_sign_in_options(&self) -> Vec<SignInOption> {
        let mut options = Vec::new();
        if self.is_chatgpt_login_allowed() {
            options.push(SignInOption::ChatGpt);
            options.push(SignInOption::DeviceCode);
        }
        if self.is_api_login_allowed() {
            options.push(SignInOption::ApiKey);
        }
        options
    }

    fn move_highlight(&mut self, delta: isize) {
        let options = self.selectable_sign_in_options();
        if options.is_empty() {
            return;
        }

        let current_index = options
            .iter()
            .position(|option| *option == self.highlighted_mode)
            .unwrap_or(0);
        let next_index =
            (current_index as isize + delta).rem_euclid(options.len() as isize) as usize;
        self.highlighted_mode = options[next_index];
    }

    fn select_option_by_index(&mut self, index: usize) {
        let options = self.displayed_sign_in_options();
        if let Some(option) = options.get(index).copied() {
            self.handle_sign_in_option(option);
        }
    }

    fn handle_sign_in_option(&mut self, option: SignInOption) {
        match option {
            SignInOption::ChatGpt => {
                if self.is_chatgpt_login_allowed() {
                    self.start_chatgpt_login();
                }
            }
            SignInOption::DeviceCode => {
                if self.is_chatgpt_login_allowed() {
                    self.start_device_code_login();
                }
            }
            SignInOption::ApiKey => {
                if self.is_api_login_allowed() {
                    self.start_api_key_entry();
                } else {
                    self.disallow_api_login();
                }
            }
        }
    }

    fn disallow_api_login(&mut self) {
        self.highlighted_mode = SignInOption::ChatGpt;
        self.set_error(Some(API_KEY_DISABLED_MESSAGE.to_string()));
        *self.sign_in_state.write().unwrap() = SignInState::PickMode;
        self.request_frame.schedule_frame();
    }

    fn render_pick_mode(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                "  ".into(),
                "Sign in with ChatGPT to use Codex as part of your paid plan".into(),
            ]),
            Line::from(vec![
                "  ".into(),
                "or connect an API key for usage-based billing".into(),
            ]),
            "".into(),
        ];

        let create_mode_item = |idx: usize,
                                selected_mode: SignInOption,
                                text: &str,
                                description: &str|
         -> Vec<Line<'static>> {
            let is_selected = self.highlighted_mode == selected_mode;
            let caret = if is_selected { ">" } else { " " };

            let line1 = if is_selected {
                Line::from(vec![
                    format!("{caret} {index}. ", index = idx + 1).cyan().dim(),
                    text.to_string().cyan(),
                ])
            } else {
                format!("  {index}. {text}", index = idx + 1).into()
            };

            let line2 = if is_selected {
                Line::from(format!("     {description}"))
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::DIM)
            } else {
                Line::from(format!("     {description}"))
                    .style(Style::default().add_modifier(Modifier::DIM))
            };

            vec![line1, line2]
        };

        let chatgpt_description = if !self.is_chatgpt_login_allowed() {
            "ChatGPT login is disabled"
        } else {
            "Usage included with Plus, Pro, Business, and Enterprise plans"
        };
        let device_code_description = "Sign in from another device with a one-time code";

        for (idx, option) in self.displayed_sign_in_options().into_iter().enumerate() {
            match option {
                SignInOption::ChatGpt => {
                    lines.extend(create_mode_item(
                        idx,
                        option,
                        "Sign in with ChatGPT",
                        chatgpt_description,
                    ));
                }
                SignInOption::DeviceCode => {
                    lines.extend(create_mode_item(
                        idx,
                        option,
                        "Sign in with Device Code",
                        device_code_description,
                    ));
                }
                SignInOption::ApiKey => {
                    lines.extend(create_mode_item(
                        idx,
                        option,
                        "Provide your own API key",
                        "Pay for what you use",
                    ));
                }
            }
            lines.push("".into());
        }

        if !self.is_api_login_allowed() {
            lines.push(
                "  API key login is disabled by this workspace. Sign in with ChatGPT to continue."
                    .dim()
                    .into(),
            );
            lines.push("".into());
        }
        lines.push(
            // AE: Following styles.md, this should probably be Cyan because it's a user input tip.
            //     But leaving this for a future cleanup.
            "  Press Enter to continue".dim().into(),
        );
        if let Some(err) = self.error_message() {
            lines.push("".into());
            lines.push(err.red().into());
        }

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_continue_in_browser(&self, area: Rect, buf: &mut Buffer) {
        let mut spans = vec!["  ".into()];
        if self.animations_enabled {
            // Schedule a follow-up frame to keep the shimmer animation going.
            self.request_frame
                .schedule_frame_in(std::time::Duration::from_millis(100));
            spans.extend(shimmer_spans("Finish signing in via your browser"));
        } else {
            spans.push("Finish signing in via your browser".into());
        }
        let mut lines = vec![spans.into(), "".into()];

        let sign_in_state = self.sign_in_state.read().unwrap();
        let auth_url = if let SignInState::ChatGptContinueInBrowser(state) = &*sign_in_state
            && !state.auth_url.is_empty()
        {
            lines.push("  If the link doesn't open automatically, open the following link to authenticate:".into());
            lines.push("".into());
            lines.push(Line::from(vec![
                "  ".into(),
                state.auth_url.as_str().cyan().underlined(),
            ]));
            lines.push("".into());
            lines.push(Line::from(vec![
                "  On a remote or headless machine? Press Esc and choose ".into(),
                "Sign in with Device Code".cyan(),
                ".".into(),
            ]));
            lines.push("".into());
            Some(state.auth_url.clone())
        } else {
            None
        };

        lines.push("  Press Esc to cancel".dim().into());
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);

        // Wrap cyan+underlined URL cells with OSC 8 so the terminal treats
        // the entire region as a single clickable hyperlink.
        if let Some(url) = &auth_url {
            mark_url_hyperlink(buf, area, url);
        }
    }

    fn render_chatgpt_success_message(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            "✓ Signed in with your ChatGPT account".fg(Color::Green).into(),
            "".into(),
            "  Before you start:".into(),
            "".into(),
            "  Decide how much autonomy you want to grant Codex".into(),
            Line::from(vec![
                "  For more details see the ".into(),
                "\u{1b}]8;;https://developers.openai.com/codex/security\u{7}Codex docs\u{1b}]8;;\u{7}".underlined(),
            ])
            .dim(),
            "".into(),
            "  Codex can make mistakes".into(),
            "  Review the code it writes and commands it runs".dim().into(),
            "".into(),
            "  Powered by your ChatGPT account".into(),
            Line::from(vec![
                "  Uses your plan's rate limits and ".into(),
                "\u{1b}]8;;https://chatgpt.com/#settings\u{7}training data preferences\u{1b}]8;;\u{7}".underlined(),
            ])
            .dim(),
            "".into(),
            "  Press Enter to continue".fg(Color::Cyan).into(),
        ];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_chatgpt_success(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            "✓ Signed in with your ChatGPT account"
                .fg(Color::Green)
                .into(),
        ];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_api_key_configured(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            "✓ API key configured".fg(Color::Green).into(),
            "".into(),
            "  Codex will use usage-based billing with your API key.".into(),
        ];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_api_key_entry(&self, area: Rect, buf: &mut Buffer, state: &ApiKeyInputState) {
        let [intro_area, input_area, footer_area] = Layout::vertical([
            Constraint::Min(4),
            Constraint::Length(3),
            Constraint::Min(2),
        ])
        .areas(area);

        let mut intro_lines: Vec<Line> = vec![
            Line::from(vec![
                "> ".into(),
                "Use your own OpenAI API key for usage-based billing".bold(),
            ]),
            "".into(),
            "  Paste or type your API key below. It will be stored locally in auth.json.".into(),
            "".into(),
        ];
        if state.prepopulated_from_env {
            intro_lines.push("  Detected OPENAI_API_KEY environment variable.".into());
            intro_lines.push(
                "  Paste a different key if you prefer to use another account."
                    .dim()
                    .into(),
            );
            intro_lines.push("".into());
        }
        Paragraph::new(intro_lines)
            .wrap(Wrap { trim: false })
            .render(intro_area, buf);

        let content_line: Line = if state.value.is_empty() {
            vec!["Paste or type your API key".dim()].into()
        } else {
            Line::from(state.value.clone())
        };
        Paragraph::new(content_line)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .title("API key")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .render(input_area, buf);

        let mut footer_lines: Vec<Line> = vec![
            "  Press Enter to save".dim().into(),
            "  Press Esc to go back".dim().into(),
        ];
        if let Some(error) = self.error_message() {
            footer_lines.push("".into());
            footer_lines.push(error.red().into());
        }
        Paragraph::new(footer_lines)
            .wrap(Wrap { trim: false })
            .render(footer_area, buf);
    }

    fn handle_api_key_entry_key_event(&mut self, key_event: &KeyEvent) -> bool {
        let mut should_save: Option<String> = None;
        let mut should_request_frame = false;

        {
            let mut guard = self.sign_in_state.write().unwrap();
            if let SignInState::ApiKeyEntry(state) = &mut *guard {
                match key_event.code {
                    KeyCode::Esc => {
                        *guard = SignInState::PickMode;
                        self.set_error(/*message*/ None);
                        should_request_frame = true;
                    }
                    KeyCode::Enter => {
                        let trimmed = state.value.trim().to_string();
                        if trimmed.is_empty() {
                            self.set_error(Some("API key cannot be empty".to_string()));
                            should_request_frame = true;
                        } else {
                            should_save = Some(trimmed);
                        }
                    }
                    KeyCode::Backspace => {
                        if state.prepopulated_from_env {
                            state.value.clear();
                            state.prepopulated_from_env = false;
                        } else {
                            state.value.pop();
                        }
                        self.set_error(/*message*/ None);
                        should_request_frame = true;
                    }
                    KeyCode::Char(c)
                        if key_event.kind == KeyEventKind::Press
                            && !key_event.modifiers.contains(KeyModifiers::SUPER)
                            && !key_event.modifiers.contains(KeyModifiers::CONTROL)
                            && !key_event.modifiers.contains(KeyModifiers::ALT) =>
                    {
                        if state.prepopulated_from_env {
                            state.value.clear();
                            state.prepopulated_from_env = false;
                        }
                        state.value.push(c);
                        self.set_error(/*message*/ None);
                        should_request_frame = true;
                    }
                    _ => {}
                }
                // handled; let guard drop before potential save
            } else {
                return false;
            }
        }

        if let Some(api_key) = should_save {
            self.save_api_key(api_key);
        } else if should_request_frame {
            self.request_frame.schedule_frame();
        }
        true
    }

    fn handle_api_key_entry_paste(&mut self, pasted: String) -> bool {
        let trimmed = pasted.trim();
        if trimmed.is_empty() {
            return false;
        }

        let mut guard = self.sign_in_state.write().unwrap();
        if let SignInState::ApiKeyEntry(state) = &mut *guard {
            if state.prepopulated_from_env {
                state.value = trimmed.to_string();
                state.prepopulated_from_env = false;
            } else {
                state.value.push_str(trimmed);
            }
            self.set_error(/*message*/ None);
        } else {
            return false;
        }

        drop(guard);
        self.request_frame.schedule_frame();
        true
    }

    fn start_api_key_entry(&mut self) {
        if !self.is_api_login_allowed() {
            self.disallow_api_login();
            return;
        }
        self.set_error(/*message*/ None);
        let prefill_from_env = read_openai_api_key_from_env();
        let mut guard = self.sign_in_state.write().unwrap();
        match &mut *guard {
            SignInState::ApiKeyEntry(state) => {
                if state.value.is_empty() {
                    if let Some(prefill) = prefill_from_env {
                        state.value = prefill;
                        state.prepopulated_from_env = true;
                    } else {
                        state.prepopulated_from_env = false;
                    }
                }
            }
            _ => {
                *guard = SignInState::ApiKeyEntry(ApiKeyInputState {
                    value: prefill_from_env.clone().unwrap_or_default(),
                    prepopulated_from_env: prefill_from_env.is_some(),
                });
            }
        }
        drop(guard);
        self.request_frame.schedule_frame();
    }

    fn save_api_key(&mut self, api_key: String) {
        if !self.is_api_login_allowed() {
            self.disallow_api_login();
            return;
        }
        self.set_error(/*message*/ None);
        let request_handle = self.app_server_request_handle.clone();
        let sign_in_state = self.sign_in_state.clone();
        let error = self.error.clone();
        let request_frame = self.request_frame.clone();
        tokio::spawn(async move {
            match request_handle
                .request_typed::<LoginAccountResponse>(ClientRequest::LoginAccount {
                    request_id: onboarding_request_id(),
                    params: LoginAccountParams::ApiKey {
                        api_key: api_key.clone(),
                    },
                })
                .await
            {
                Ok(LoginAccountResponse::ApiKey {}) => {
                    *error.write().unwrap() = None;
                    *sign_in_state.write().unwrap() = SignInState::ApiKeyConfigured;
                }
                Ok(other) => {
                    *error.write().unwrap() = Some(format!(
                        "Unexpected account/login/start response: {other:?}"
                    ));
                    *sign_in_state.write().unwrap() = SignInState::ApiKeyEntry(ApiKeyInputState {
                        value: api_key,
                        prepopulated_from_env: false,
                    });
                }
                Err(err) => {
                    *error.write().unwrap() = Some(format!("Failed to save API key: {err}"));
                    *sign_in_state.write().unwrap() = SignInState::ApiKeyEntry(ApiKeyInputState {
                        value: api_key,
                        prepopulated_from_env: false,
                    });
                }
            }
            request_frame.schedule_frame();
        });
        self.request_frame.schedule_frame();
    }

    fn handle_existing_chatgpt_login(&mut self) -> bool {
        if matches!(
            self.login_status,
            LoginStatus::AuthMode(AppServerAuthMode::Chatgpt)
                | LoginStatus::AuthMode(AppServerAuthMode::ChatgptAuthTokens)
        ) {
            *self.sign_in_state.write().unwrap() = SignInState::ChatGptSuccess;
            self.request_frame.schedule_frame();
            true
        } else {
            false
        }
    }

    /// Kicks off the ChatGPT auth flow and keeps the UI state consistent with the attempt.
    fn start_chatgpt_login(&mut self) {
        // If we're already authenticated with ChatGPT, don't start a new login –
        // just proceed to the success message flow.
        if self.handle_existing_chatgpt_login() {
            return;
        }

        self.set_error(/*message*/ None);
        let request_handle = self.app_server_request_handle.clone();
        let sign_in_state = self.sign_in_state.clone();
        let error = self.error.clone();
        let request_frame = self.request_frame.clone();
        tokio::spawn(async move {
            match request_handle
                .request_typed::<LoginAccountResponse>(ClientRequest::LoginAccount {
                    request_id: onboarding_request_id(),
                    params: LoginAccountParams::Chatgpt,
                })
                .await
            {
                Ok(LoginAccountResponse::Chatgpt { login_id, auth_url }) => {
                    maybe_open_auth_url_in_browser(&request_handle, &auth_url);
                    *error.write().unwrap() = None;
                    *sign_in_state.write().unwrap() =
                        SignInState::ChatGptContinueInBrowser(ContinueInBrowserState {
                            login_id,
                            auth_url,
                        });
                }
                Ok(other) => {
                    *sign_in_state.write().unwrap() = SignInState::PickMode;
                    *error.write().unwrap() = Some(format!(
                        "Unexpected account/login/start response: {other:?}"
                    ));
                }
                Err(err) => {
                    *sign_in_state.write().unwrap() = SignInState::PickMode;
                    *error.write().unwrap() = Some(err.to_string());
                }
            }
            request_frame.schedule_frame();
        });
    }

    fn start_device_code_login(&mut self) {
        if self.handle_existing_chatgpt_login() {
            return;
        }

        self.set_error(/*message*/ None);
        headless_chatgpt_login::start_headless_chatgpt_login(self);
    }

    pub(crate) fn on_account_login_completed(
        &mut self,
        notification: AccountLoginCompletedNotification,
    ) {
        let Some(login_id) = notification.login_id else {
            return;
        };
        let guard = self.sign_in_state.read().unwrap();
        let is_matching_login = matches!(
            &*guard,
            SignInState::ChatGptContinueInBrowser(state) if state.login_id == login_id
        );
        drop(guard);
        if !is_matching_login {
            return;
        }

        if notification.success {
            self.set_error(/*message*/ None);
            *self.sign_in_state.write().unwrap() = SignInState::ChatGptSuccessMessage;
        } else {
            self.set_error(notification.error);
            *self.sign_in_state.write().unwrap() = SignInState::PickMode;
        }
        self.request_frame.schedule_frame();
    }

    pub(crate) fn on_account_updated(&mut self, notification: AccountUpdatedNotification) {
        self.login_status = notification
            .auth_mode
            .map(LoginStatus::AuthMode)
            .unwrap_or(LoginStatus::NotAuthenticated);
    }
}

impl StepStateProvider for AuthModeWidget {
    fn get_step_state(&self) -> StepState {
        let sign_in_state = self.sign_in_state.read().unwrap();
        match &*sign_in_state {
            SignInState::PickMode
            | SignInState::ApiKeyEntry(_)
            | SignInState::ChatGptContinueInBrowser(_)
            | SignInState::ChatGptDeviceCode(_)
            | SignInState::ChatGptSuccessMessage => StepState::InProgress,
            SignInState::ChatGptSuccess | SignInState::ApiKeyConfigured => StepState::Complete,
        }
    }
}

impl WidgetRef for AuthModeWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let sign_in_state = self.sign_in_state.read().unwrap();
        match &*sign_in_state {
            SignInState::PickMode => {
                self.render_pick_mode(area, buf);
            }
            SignInState::ChatGptContinueInBrowser(_) => {
                self.render_continue_in_browser(area, buf);
            }
            SignInState::ChatGptDeviceCode(state) => {
                headless_chatgpt_login::render_device_code_login(self, area, buf, state);
            }
            SignInState::ChatGptSuccessMessage => {
                self.render_chatgpt_success_message(area, buf);
            }
            SignInState::ChatGptSuccess => {
                self.render_chatgpt_success(area, buf);
            }
            SignInState::ApiKeyEntry(state) => {
                self.render_api_key_entry(area, buf, state);
            }
            SignInState::ApiKeyConfigured => {
                self.render_api_key_configured(area, buf);
            }
        }
    }
}

pub(super) fn maybe_open_auth_url_in_browser(request_handle: &AppServerRequestHandle, url: &str) {
    if !matches!(request_handle, AppServerRequestHandle::InProcess(_)) {
        return;
    }

    if let Err(err) = webbrowser::open(url) {
        tracing::warn!("failed to open browser for login URL: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_client::AppServerRequestHandle;
    use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
    use codex_app_server_client::InProcessAppServerClient;
    use codex_app_server_client::InProcessClientStartArgs;
    use codex_arg0::Arg0DispatchPaths;
    use codex_cloud_requirements::cloud_requirements_loader_for_storage;
    use codex_core::config::ConfigBuilder;

    use codex_protocol::protocol::SessionSource;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn widget_forced_chatgpt() -> (AuthModeWidget, TempDir) {
        let codex_home = TempDir::new().unwrap();
        let codex_home_path = codex_home.path().to_path_buf();
        let config = ConfigBuilder::default()
            .codex_home(codex_home_path.clone())
            .build()
            .await
            .unwrap();
        let client = InProcessAppServerClient::start(InProcessClientStartArgs {
            arg0_paths: Arg0DispatchPaths::default(),
            config: Arc::new(config),
            cli_overrides: Vec::new(),
            loader_overrides: Default::default(),
            cloud_requirements: cloud_requirements_loader_for_storage(
                codex_home_path.clone(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::File,
                "https://chatgpt.com/backend-api/".to_string(),
            ),
            feedback: codex_feedback::CodexFeedback::new(),
            config_warnings: Vec::new(),
            session_source: SessionSource::Cli,
            enable_codex_api_key_env: false,
            client_name: "test".to_string(),
            client_version: "test".to_string(),
            experimental_api: true,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
        })
        .await
        .unwrap();
        let widget = AuthModeWidget {
            request_frame: FrameRequester::test_dummy(),
            highlighted_mode: SignInOption::ChatGpt,
            error: Arc::new(RwLock::new(None)),
            sign_in_state: Arc::new(RwLock::new(SignInState::PickMode)),
            codex_home: codex_home_path.clone(),
            cli_auth_credentials_store_mode: AuthCredentialsStoreMode::File,
            login_status: LoginStatus::NotAuthenticated,
            app_server_request_handle: AppServerRequestHandle::InProcess(client.request_handle()),
            forced_chatgpt_workspace_id: None,
            forced_login_method: Some(ForcedLoginMethod::Chatgpt),
            animations_enabled: true,
        };
        (widget, codex_home)
    }

    #[tokio::test]
    async fn api_key_flow_disabled_when_chatgpt_forced() {
        let (mut widget, _tmp) = widget_forced_chatgpt().await;

        widget.start_api_key_entry();

        assert_eq!(
            widget.error_message().as_deref(),
            Some(API_KEY_DISABLED_MESSAGE)
        );
        assert!(matches!(
            &*widget.sign_in_state.read().unwrap(),
            SignInState::PickMode
        ));
    }

    #[tokio::test]
    async fn saving_api_key_is_blocked_when_chatgpt_forced() {
        let (mut widget, _tmp) = widget_forced_chatgpt().await;

        widget.save_api_key("sk-test".to_string());

        assert_eq!(
            widget.error_message().as_deref(),
            Some(API_KEY_DISABLED_MESSAGE)
        );
        assert!(matches!(
            &*widget.sign_in_state.read().unwrap(),
            SignInState::PickMode
        ));
        assert_eq!(widget.login_status, LoginStatus::NotAuthenticated);
    }

    #[tokio::test]
    async fn existing_chatgpt_auth_tokens_login_counts_as_signed_in() {
        let (mut widget, _tmp) = widget_forced_chatgpt().await;
        widget.login_status = LoginStatus::AuthMode(AppServerAuthMode::ChatgptAuthTokens);

        let handled = widget.handle_existing_chatgpt_login();

        assert_eq!(handled, true);
        assert!(matches!(
            &*widget.sign_in_state.read().unwrap(),
            SignInState::ChatGptSuccess
        ));
    }

    #[tokio::test]
    async fn cancel_active_attempt_resets_browser_login_state() {
        let (widget, _tmp) = widget_forced_chatgpt().await;
        *widget.error.write().unwrap() = Some("still logging in".to_string());
        *widget.sign_in_state.write().unwrap() =
            SignInState::ChatGptContinueInBrowser(ContinueInBrowserState {
                login_id: "login-1".to_string(),
                auth_url: "https://auth.example.com".to_string(),
            });

        widget.cancel_active_attempt();

        assert_eq!(widget.error_message(), None);
        assert!(matches!(
            &*widget.sign_in_state.read().unwrap(),
            SignInState::PickMode
        ));
    }

    #[tokio::test]
    async fn cancel_active_attempt_notifies_device_code_login() {
        let (widget, _tmp) = widget_forced_chatgpt().await;
        let cancel = Arc::new(Notify::new());
        *widget.error.write().unwrap() = Some("still logging in".to_string());
        *widget.sign_in_state.write().unwrap() =
            SignInState::ChatGptDeviceCode(ContinueWithDeviceCodeState {
                device_code: None,
                cancel: Some(cancel.clone()),
            });

        widget.cancel_active_attempt();

        assert_eq!(widget.error_message(), None);
        assert!(matches!(
            &*widget.sign_in_state.read().unwrap(),
            SignInState::PickMode
        ));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), cancel.notified())
                .await
                .is_ok()
        );
    }

    /// Collects all buffer cell symbols that contain the OSC 8 open sequence
    /// for the given URL.  Returns the concatenated "inner" characters.
    fn collect_osc8_chars(buf: &Buffer, area: Rect, url: &str) -> String {
        let open = format!("\x1B]8;;{url}\x07");
        let close = "\x1B]8;;\x07";
        let mut chars = String::new();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let sym = buf[(x, y)].symbol();
                if let Some(rest) = sym.strip_prefix(open.as_str())
                    && let Some(ch) = rest.strip_suffix(close)
                {
                    chars.push_str(ch);
                }
            }
        }
        chars
    }

    #[test]
    fn continue_in_browser_renders_osc8_hyperlink() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (widget, _tmp) = runtime.block_on(widget_forced_chatgpt());
        let url = "https://auth.example.com/login?state=abc123";
        *widget.sign_in_state.write().unwrap() =
            SignInState::ChatGptContinueInBrowser(ContinueInBrowserState {
                login_id: "login-1".to_string(),
                auth_url: url.to_string(),
            });

        // Render into a narrow buffer so the URL wraps across multiple rows.
        let area = Rect::new(0, 0, 30, 20);
        let mut buf = Buffer::empty(area);
        widget.render_continue_in_browser(area, &mut buf);

        // Every character of the URL should be present as an OSC 8 cell.
        let found = collect_osc8_chars(&buf, area, url);
        assert_eq!(found, url, "OSC 8 hyperlink should cover the full URL");
    }

    #[test]
    fn mark_url_hyperlink_wraps_cyan_underlined_cells() {
        let url = "https://example.com";
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);

        // Manually write some cyan+underlined characters to simulate a rendered URL.
        for (i, ch) in "example".chars().enumerate() {
            let cell = &mut buf[(i as u16, 0)];
            cell.set_symbol(&ch.to_string());
            cell.fg = Color::Cyan;
            cell.modifier = Modifier::UNDERLINED;
        }
        // Leave a plain cell that should NOT be marked.
        buf[(7, 0)].set_symbol("X");

        mark_url_hyperlink(&mut buf, area, url);

        // Each cyan+underlined cell should now carry the OSC 8 wrapper.
        let found = collect_osc8_chars(&buf, area, url);
        assert_eq!(found, "example");

        // The plain "X" cell should be untouched.
        assert_eq!(buf[(7, 0)].symbol(), "X");
    }

    #[test]
    fn mark_url_hyperlink_sanitizes_control_chars() {
        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);

        // One cyan+underlined cell to mark.
        let cell = &mut buf[(0, 0)];
        cell.set_symbol("a");
        cell.fg = Color::Cyan;
        cell.modifier = Modifier::UNDERLINED;

        // URL contains ESC and BEL that could break the OSC 8 sequence.
        let malicious_url = "https://evil.com/\x1B]8;;\x07injected";
        mark_url_hyperlink(&mut buf, area, malicious_url);

        let sym = buf[(0, 0)].symbol().to_string();
        // The sanitized URL retains `]` (printable) but strips ESC and BEL.
        let sanitized = "https://evil.com/]8;;injected";
        assert!(
            sym.contains(sanitized),
            "symbol should contain sanitized URL, got: {sym:?}"
        );
        // The injected close-sequence must not survive: \x1B and \x07 are gone.
        assert!(
            !sym.contains("\x1B]8;;\x07injected"),
            "symbol must not contain raw control chars from URL"
        );
    }
}
