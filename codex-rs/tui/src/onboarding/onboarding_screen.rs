//! Onboarding screen orchestration and top-level keyboard routing.
//!
//! The onboarding flow is a small state machine over visible steps
//! (welcome/auth). This module decides which step receives key/paste
//! events and enforces flow-level safety rules that cut across individual step
//! widgets.
//!
//! In particular, onboarding quit handling has a text-entry guard for API-key
//! input: the printable `q` quit key is treated as text input while the user is
//! editing a non-empty API-key field, while control/alt chords remain available
//! as explicit exit shortcuts.

use codex_app_server_client::AppServerEvent;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ServerNotification;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Color;
use ratatui::widgets::Clear;
use ratatui::widgets::WidgetRef;

use codex_protocol::config_types::ForcedLoginMethod;

use crate::LoginStatus;
use crate::app_server_session::AppServerSession;
use crate::key_hint::KeyBindingListExt;
use crate::legacy_core::config::Config;
use crate::onboarding::auth::AuthModeWidget;
use crate::onboarding::auth::SignInOption;
use crate::onboarding::auth::SignInState;
use crate::onboarding::keys;
use crate::onboarding::welcome::WelcomeWidget;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use color_eyre::eyre::Result;
use std::sync::Arc;
use std::sync::RwLock;

#[allow(clippy::large_enum_variant)]
enum Step {
    Welcome(WelcomeWidget),
    Auth(AuthModeWidget),
}

pub(crate) trait KeyboardHandler {
    fn handle_key_event(&mut self, key_event: KeyEvent);
    fn handle_paste(&mut self, _pasted: String) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepState {
    Hidden,
    InProgress,
    Complete,
}

pub(crate) trait StepStateProvider {
    fn get_step_state(&self) -> StepState;
}

pub(crate) struct OnboardingScreen {
    request_frame: FrameRequester,
    steps: Vec<Step>,
    is_done: bool,
    should_exit: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ApiKeyEntryContext {
    /// True when onboarding is currently rendering the API-key entry state.
    active: bool,
    /// True when the API-key input field currently contains user text.
    has_text: bool,
}

impl OnboardingScreen {
    fn new(
        tui: &mut Tui,
        app_server_request_handle: AppServerRequestHandle,
        config: Config,
    ) -> Self {
        let forced_login_method = config.forced_login_method;
        let mut steps: Vec<Step> = Vec::new();
        steps.push(Step::Welcome(WelcomeWidget::new(
            /*is_logged_in*/ false,
            tui.frame_requester(),
            config.animations,
        )));
        let highlighted_mode = match forced_login_method {
            Some(ForcedLoginMethod::Api) => SignInOption::ApiKey,
            _ => SignInOption::ChatGpt,
        };
        steps.push(Step::Auth(AuthModeWidget {
            request_frame: tui.frame_requester(),
            highlighted_mode,
            error: Arc::new(RwLock::new(None)),
            sign_in_state: Arc::new(RwLock::new(SignInState::PickMode)),
            login_status: LoginStatus::NotAuthenticated,
            app_server_request_handle,
            forced_login_method,
            animations_enabled: config.animations,
            animations_suppressed: std::cell::Cell::new(false),
        }));
        Self {
            request_frame: tui.frame_requester(),
            steps,
            is_done: false,
            should_exit: false,
        }
    }

    fn current_steps_mut(&mut self) -> Vec<&mut Step> {
        let mut out: Vec<&mut Step> = Vec::new();
        for step in self.steps.iter_mut() {
            match step.get_step_state() {
                StepState::Hidden => continue,
                StepState::Complete => out.push(step),
                StepState::InProgress => {
                    out.push(step);
                    break;
                }
            }
        }
        out
    }

    fn current_steps(&self) -> Vec<&Step> {
        let mut out: Vec<&Step> = Vec::new();
        for step in self.steps.iter() {
            match step.get_step_state() {
                StepState::Hidden => continue,
                StepState::Complete => out.push(step),
                StepState::InProgress => {
                    out.push(step);
                    break;
                }
            }
        }
        out
    }

    fn should_suppress_animations(&self) -> bool {
        // Freeze the whole onboarding screen when auth is showing copyable login
        // material so terminal selection is not interrupted by redraws.
        self.current_steps().into_iter().any(|step| match step {
            Step::Auth(widget) => widget.should_suppress_animations(),
            Step::Welcome(_) => false,
        })
    }

    fn is_auth_in_progress(&self) -> bool {
        self.steps.iter().any(|step| {
            matches!(step, Step::Auth(_)) && matches!(step.get_step_state(), StepState::InProgress)
        })
    }

    pub(crate) fn is_done(&self) -> bool {
        self.is_done
            || !self
                .steps
                .iter()
                .any(|step| matches!(step.get_step_state(), StepState::InProgress))
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    fn cancel_auth_if_active(&self) {
        for step in &self.steps {
            if let Step::Auth(widget) = step {
                widget.cancel_active_attempt();
            }
        }
    }

    fn auth_widget_mut(&mut self) -> Option<&mut AuthModeWidget> {
        self.steps.iter_mut().find_map(|step| match step {
            Step::Auth(widget) => Some(widget),
            Step::Welcome(_) => None,
        })
    }

    fn handle_app_server_notification(&mut self, notification: ServerNotification) {
        match notification {
            ServerNotification::AccountLoginCompleted(notification) => {
                if let Some(widget) = self.auth_widget_mut() {
                    widget.on_account_login_completed(notification);
                }
            }
            ServerNotification::AccountUpdated(notification) => {
                if let Some(widget) = self.auth_widget_mut() {
                    widget.on_account_updated(notification);
                }
            }
            _ => {}
        }
    }

    fn api_key_entry_context(&self) -> ApiKeyEntryContext {
        self.steps
            .iter()
            .find_map(|step| {
                if let Step::Auth(widget) = step {
                    Some(ApiKeyEntryContext {
                        active: widget.is_api_key_entry_active(),
                        has_text: widget.api_key_entry_has_text(),
                    })
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }
}

impl KeyboardHandler for OnboardingScreen {
    /// Route key events to onboarding steps while preserving text-entry safety.
    ///
    /// In API-key entry mode, printable quit bindings are suppressed only after
    /// the user has started typing in the API-key field. This keeps the
    /// printable `q` quit key usable on an empty field while protecting in-progress
    /// text entry from accidental exits. Control/alt quit chords still work as
    /// emergency exits.
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }
        let api_key_entry_context = self.api_key_entry_context();
        let should_quit = key_event.kind == KeyEventKind::Press
            && keys::QUIT.is_pressed(key_event)
            && !suppress_quit_while_typing_api_key(key_event, api_key_entry_context);
        if should_quit {
            if self.is_auth_in_progress() {
                self.cancel_auth_if_active();
                // If the user cancels the auth menu, exit the app rather than
                // leave the user at a prompt in an unauthed state.
                self.should_exit = true;
            }
            self.is_done = true;
        } else {
            if let Some(Step::Welcome(widget)) = self
                .steps
                .iter_mut()
                .find(|step| matches!(step, Step::Welcome(_)))
            {
                widget.handle_key_event(key_event);
            }
            if let Some(active_step) = self.current_steps_mut().into_iter().last() {
                active_step.handle_key_event(key_event);
            }
        }
        self.request_frame.schedule_frame();
    }

    fn handle_paste(&mut self, pasted: String) {
        if pasted.is_empty() {
            return;
        }

        if let Some(active_step) = self.current_steps_mut().into_iter().last() {
            active_step.handle_paste(pasted);
        }
        self.request_frame.schedule_frame();
    }
}

/// Returns `true` when a quit shortcut should be ignored as text input.
///
/// This only applies while API-key entry is active and the key is a printable
/// character without control/alt modifiers and there is already text in the
/// input field. Empty input intentionally does not trigger suppression so
/// the printable `q` quit key can still exit onboarding.
fn suppress_quit_while_typing_api_key(
    key_event: KeyEvent,
    api_key_entry_context: ApiKeyEntryContext,
) -> bool {
    api_key_entry_context.active
        && api_key_entry_context.has_text
        && matches!(key_event.code, KeyCode::Char(_))
        && !key_event
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

impl WidgetRef for &OnboardingScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let suppress_animations = self.should_suppress_animations();
        for step in self.current_steps() {
            match step {
                Step::Welcome(widget) => widget.set_animations_suppressed(suppress_animations),
                Step::Auth(widget) => widget.set_animations_suppressed(suppress_animations),
            }
        }

        Clear.render(area, buf);
        // Render steps top-to-bottom, measuring each step's height dynamically.
        let mut y = area.y;
        let bottom = area.y.saturating_add(area.height);
        let width = area.width;

        // Helper to scan a temporary buffer and return number of used rows.
        fn used_rows(tmp: &Buffer, width: u16, height: u16) -> u16 {
            if width == 0 || height == 0 {
                return 0;
            }
            let mut last_non_empty: Option<u16> = None;
            for yy in 0..height {
                let mut any = false;
                for xx in 0..width {
                    let cell = &tmp[(xx, yy)];
                    let has_symbol = !cell.symbol().trim().is_empty();
                    let has_style = cell.fg != Color::Reset
                        || cell.bg != Color::Reset
                        || !cell.modifier.is_empty();
                    if has_symbol || has_style {
                        any = true;
                        break;
                    }
                }
                if any {
                    last_non_empty = Some(yy);
                }
            }
            last_non_empty.map(|v| v + 2).unwrap_or(0)
        }

        let mut i = 0usize;
        let current_steps = self.current_steps();

        while i < current_steps.len() && y < bottom {
            let step = &current_steps[i];
            let max_h = bottom.saturating_sub(y);
            if max_h == 0 || width == 0 {
                break;
            }
            let scratch_area = Rect::new(0, 0, width, max_h);
            let mut scratch = Buffer::empty(scratch_area);
            if let Step::Welcome(widget) = step {
                widget.update_layout_area(scratch_area);
            }
            step.render_ref(scratch_area, &mut scratch);
            let h = used_rows(&scratch, width, max_h).min(max_h);
            if h > 0 {
                let target = Rect {
                    x: area.x,
                    y,
                    width,
                    height: h,
                };
                Clear.render(target, buf);
                step.render_ref(target, buf);
                y = y.saturating_add(h);
            }
            i += 1;
        }
    }
}

impl KeyboardHandler for Step {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self {
            Step::Welcome(widget) => widget.handle_key_event(key_event),
            Step::Auth(widget) => widget.handle_key_event(key_event),
        }
    }

    fn handle_paste(&mut self, pasted: String) {
        match self {
            Step::Welcome(_) => {}
            Step::Auth(widget) => widget.handle_paste(pasted),
        }
    }
}

impl StepStateProvider for Step {
    fn get_step_state(&self) -> StepState {
        match self {
            Step::Welcome(w) => w.get_step_state(),
            Step::Auth(w) => w.get_step_state(),
        }
    }
}

impl WidgetRef for Step {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match self {
            Step::Welcome(widget) => {
                widget.render_ref(area, buf);
            }
            Step::Auth(widget) => {
                widget.render_ref(area, buf);
            }
        }
    }
}

pub(crate) async fn run_login_onboarding(
    config: Config,
    app_server: &mut AppServerSession,
    tui: &mut Tui,
) -> Result<bool> {
    use tokio_stream::StreamExt;

    let mut onboarding_screen = OnboardingScreen::new(tui, app_server.request_handle(), config);
    // One-time guard to fully clear the screen after ChatGPT login success message is shown
    let mut did_full_clear_after_success = false;

    tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&onboarding_screen, frame.area());
    })?;

    let tui_events = tui.event_stream();
    tokio::pin!(tui_events);

    while !onboarding_screen.is_done() {
        tokio::select! {
            event = tui_events.next() => {
                if let Some(event) = event {
                    tui.screen_size_for_event(&event)?;
                    match event {
                        TuiEvent::Key(key_event) => {
                            onboarding_screen.handle_key_event(key_event);
                        }
                        TuiEvent::Paste(text) => {
                            onboarding_screen.handle_paste(text);
                        }
                        TuiEvent::Draw | TuiEvent::Resume | TuiEvent::Resize(_) => {
                            if !did_full_clear_after_success
                                && onboarding_screen.steps.iter().any(|step| {
                                    if let Step::Auth(w) = step {
                                        w.sign_in_state.read().is_ok_and(|g| {
                                            matches!(&*g, super::auth::SignInState::ChatGptSuccessMessage)
                                        })
                                    } else {
                                        false
                                    }
                                })
                            {
                                // Reset any lingering SGR (underline/color) before clearing
                                let _ = ratatui::crossterm::execute!(
                                    std::io::stdout(),
                                    ratatui::crossterm::style::SetAttribute(
                                        ratatui::crossterm::style::Attribute::Reset
                                    ),
                                    ratatui::crossterm::style::SetAttribute(
                                        ratatui::crossterm::style::Attribute::NoUnderline
                                    ),
                                    ratatui::crossterm::style::SetForegroundColor(
                                        ratatui::crossterm::style::Color::Reset
                                    ),
                                    ratatui::crossterm::style::SetBackgroundColor(
                                        ratatui::crossterm::style::Color::Reset
                                    )
                                );
                                let _ = tui.terminal.clear();
                                did_full_clear_after_success = true;
                            }
                            let _ = tui.draw(u16::MAX, |frame| {
                                frame.render_widget_ref(&onboarding_screen, frame.area());
                            });
                        }
                    }
                }
            }
            event = app_server.next_event() => {
                if let Some(event) = event {
                    match event {
                        AppServerEvent::ServerNotification(notification) => {
                            onboarding_screen.handle_app_server_notification(*notification);
                        }
                        AppServerEvent::Disconnected { message } => {
                            return Err(color_eyre::eyre::eyre!(message));
                        }
                        AppServerEvent::Lagged { .. }
                        | AppServerEvent::ServerRequest(_) => {}
                    }
                }
            }
        }
    }
    Ok(onboarding_screen.should_exit())
}

#[cfg(test)]
mod tests {
    use super::ApiKeyEntryContext;
    use super::suppress_quit_while_typing_api_key;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;

    #[test]
    fn suppresses_printable_quit_key_during_api_key_entry() {
        let suppressed = suppress_quit_while_typing_api_key(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            ApiKeyEntryContext {
                active: true,
                has_text: true,
            },
        );
        assert!(suppressed);
    }

    #[test]
    fn does_not_suppress_printable_quit_key_when_api_key_input_is_empty() {
        let suppressed = suppress_quit_while_typing_api_key(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            ApiKeyEntryContext {
                active: true,
                has_text: false,
            },
        );
        assert!(!suppressed);
    }

    #[test]
    fn does_not_suppress_control_quit_key_during_api_key_entry() {
        let suppressed = suppress_quit_while_typing_api_key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
            ApiKeyEntryContext {
                active: true,
                has_text: true,
            },
        );
        assert!(!suppressed);
    }

    #[test]
    fn does_not_suppress_when_not_in_api_key_entry() {
        let suppressed = suppress_quit_while_typing_api_key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            ApiKeyEntryContext {
                active: false,
                has_text: true,
            },
        );
        assert!(!suppressed);
    }
}
