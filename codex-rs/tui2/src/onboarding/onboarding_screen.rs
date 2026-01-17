//! Coordinates the multi-step onboarding flow and renders it as a single screen.
//!
//! The onboarding screen owns the ordered list of steps (welcome, auth, trust)
//! and drives their input handling and rendering. Each step owns its own state,
//! but this module decides which steps are visible, when a step is complete,
//! and when the overall flow should exit early (for example, when auth is
//! cancelled). Rendering is height-aware: each step is drawn into a scratch
//! buffer to determine how many rows it consumes before the real buffer is
//! updated.

use codex_core::AuthManager;
use codex_core::config::Config;
use codex_core::git_info::get_git_repo_root;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Color;
use ratatui::widgets::Clear;
use ratatui::widgets::WidgetRef;

use codex_protocol::config_types::ForcedLoginMethod;

use crate::LoginStatus;
use crate::onboarding::auth::AuthModeWidget;
use crate::onboarding::auth::SignInOption;
use crate::onboarding::auth::SignInState;
use crate::onboarding::trust_directory::TrustDirectorySelection;
use crate::onboarding::trust_directory::TrustDirectoryWidget;
use crate::onboarding::welcome::WelcomeWidget;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use color_eyre::eyre::Result;
use std::sync::Arc;
use std::sync::RwLock;

/// Enumerates the onboarding steps in their display order.
#[allow(clippy::large_enum_variant)]
enum Step {
    /// Welcome screen shown on entry to the onboarding flow.
    Welcome(WelcomeWidget),
    /// Authentication mode picker and sign-in flow.
    Auth(AuthModeWidget),
    /// Directory trust prompt shown for untrusted projects.
    TrustDirectory(TrustDirectoryWidget),
}

/// Handles keyboard input for onboarding widgets.
pub(crate) trait KeyboardHandler {
    /// Consumes a key event for the current widget.
    fn handle_key_event(&mut self, key_event: KeyEvent);

    /// Handles a paste event if the widget accepts pasted input.
    fn handle_paste(&mut self, _pasted: String) {}
}

/// Indicates whether an onboarding step is visible, active, or finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepState {
    /// The step should not be rendered or considered for input.
    Hidden,
    /// The step is active and should receive input.
    InProgress,
    /// The step is finished and may still be rendered above the active step.
    Complete,
}

/// Supplies a step's current progress state.
pub(crate) trait StepStateProvider {
    /// Reports whether the step is hidden, active, or complete.
    fn get_step_state(&self) -> StepState;
}

/// Drives the onboarding sequence and routes input to the active step.
pub(crate) struct OnboardingScreen {
    /// Frame scheduler used to request redraws after input.
    request_frame: FrameRequester,
    /// Ordered list of onboarding steps to show.
    steps: Vec<Step>,
    /// Explicit completion flag used for early exit.
    is_done: bool,
    /// Whether the onboarding flow should exit the app entirely.
    should_exit: bool,
}

/// Supplies configuration and dependencies for onboarding.
pub(crate) struct OnboardingScreenArgs {
    /// Whether the trust prompt should be shown for this run.
    pub show_trust_screen: bool,
    /// Whether the login flow should be shown for this run.
    pub show_login_screen: bool,
    /// Current authentication status used to seed widgets.
    pub login_status: LoginStatus,
    /// Auth manager used to perform login flows.
    pub auth_manager: Arc<AuthManager>,
    /// User configuration controlling onboarding behavior.
    pub config: Config,
}

/// Reports the outcome of the onboarding flow.
pub(crate) struct OnboardingResult {
    /// Trust selection for the working directory, if one was shown.
    pub directory_trust_decision: Option<TrustDirectorySelection>,
    /// Whether the user requested to exit the app during onboarding.
    pub should_exit: bool,
}

impl OnboardingScreen {
    /// Constructs a new onboarding flow with the requested steps.
    pub(crate) fn new(tui: &mut Tui, args: OnboardingScreenArgs) -> Self {
        let OnboardingScreenArgs {
            show_trust_screen,
            show_login_screen,
            login_status,
            auth_manager,
            config,
        } = args;
        let cwd = config.cwd.clone();
        let forced_chatgpt_workspace_id = config.forced_chatgpt_workspace_id.clone();
        let forced_login_method = config.forced_login_method;
        let codex_home = config.codex_home;
        let cli_auth_credentials_store_mode = config.cli_auth_credentials_store_mode;
        let mut steps: Vec<Step> = Vec::new();
        steps.push(Step::Welcome(WelcomeWidget::new(
            !matches!(login_status, LoginStatus::NotAuthenticated),
            tui.frame_requester(),
            config.animations,
        )));
        if show_login_screen {
            let highlighted_mode = match forced_login_method {
                Some(ForcedLoginMethod::Api) => SignInOption::ApiKey,
                _ => SignInOption::ChatGpt,
            };
            steps.push(Step::Auth(AuthModeWidget {
                request_frame: tui.frame_requester(),
                highlighted_mode,
                error: None,
                sign_in_state: Arc::new(RwLock::new(SignInState::PickMode)),
                codex_home: codex_home.clone(),
                cli_auth_credentials_store_mode,
                login_status,
                auth_manager,
                forced_chatgpt_workspace_id,
                forced_login_method,
                animations_enabled: config.animations,
            }))
        }
        let is_git_repo = get_git_repo_root(&cwd).is_some();
        let highlighted = if is_git_repo {
            TrustDirectorySelection::Trust
        } else {
            // Default to not trusting the directory if it's not a git repo.
            TrustDirectorySelection::DontTrust
        };
        if show_trust_screen {
            steps.push(Step::TrustDirectory(TrustDirectoryWidget {
                cwd,
                codex_home,
                is_git_repo,
                selection: None,
                highlighted,
                error: None,
            }))
        }
        // TODO: add git warning.
        Self {
            request_frame: tui.frame_requester(),
            steps,
            is_done: false,
            should_exit: false,
        }
    }

    /// Returns the visible steps up to and including the active step.
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

    /// Returns the visible steps up to and including the active step.
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

    /// Reports whether the authentication step is currently active.
    fn is_auth_in_progress(&self) -> bool {
        self.steps.iter().any(|step| {
            matches!(step, Step::Auth(_)) && matches!(step.get_step_state(), StepState::InProgress)
        })
    }

    /// Returns whether onboarding has finished or no active step remains.
    pub(crate) fn is_done(&self) -> bool {
        self.is_done
            || !self
                .steps
                .iter()
                .any(|step| matches!(step.get_step_state(), StepState::InProgress))
    }

    /// Returns the trust selection if the trust step was shown.
    pub fn directory_trust_decision(&self) -> Option<TrustDirectorySelection> {
        self.steps
            .iter()
            .find_map(|step| {
                if let Step::TrustDirectory(TrustDirectoryWidget { selection, .. }) = step {
                    Some(*selection)
                } else {
                    None
                }
            })
            .flatten()
    }

    /// Returns whether the onboarding flow requested app exit.
    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    /// Reports whether the API key entry sub-state is currently active.
    fn is_api_key_entry_active(&self) -> bool {
        self.steps.iter().any(|step| {
            if let Step::Auth(widget) = step {
                return widget
                    .sign_in_state
                    .read()
                    .is_ok_and(|g| matches!(&*g, SignInState::ApiKeyEntry(_)));
            }
            false
        })
    }
}

impl KeyboardHandler for OnboardingScreen {
    /// Handles onboarding-level key bindings and forwards input to steps.
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        let is_api_key_entry_active = self.is_api_key_entry_active();
        let should_quit = match key_event {
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => true,
            KeyEvent {
                code: KeyCode::Char('q'),
                kind: KeyEventKind::Press,
                ..
            } => !is_api_key_entry_active,
            _ => false,
        };
        if should_quit {
            if self.is_auth_in_progress() {
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

    /// Forwards a paste event to the active step.
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

impl WidgetRef for &OnboardingScreen {
    /// Renders the onboarding steps and sizes each one by rendered height.
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
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
    /// Dispatches key events to the active step widget.
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self {
            Step::Welcome(widget) => widget.handle_key_event(key_event),
            Step::Auth(widget) => widget.handle_key_event(key_event),
            Step::TrustDirectory(widget) => widget.handle_key_event(key_event),
        }
    }

    /// Dispatches paste events to steps that accept pasted input.
    fn handle_paste(&mut self, pasted: String) {
        match self {
            Step::Welcome(_) => {}
            Step::Auth(widget) => widget.handle_paste(pasted),
            Step::TrustDirectory(widget) => widget.handle_paste(pasted),
        }
    }
}

impl StepStateProvider for Step {
    /// Returns the step's current visibility state.
    fn get_step_state(&self) -> StepState {
        match self {
            Step::Welcome(w) => w.get_step_state(),
            Step::Auth(w) => w.get_step_state(),
            Step::TrustDirectory(w) => w.get_step_state(),
        }
    }
}

impl WidgetRef for Step {
    /// Renders the step widget into the given buffer.
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match self {
            Step::Welcome(widget) => {
                widget.render_ref(area, buf);
            }
            Step::Auth(widget) => {
                widget.render_ref(area, buf);
            }
            Step::TrustDirectory(widget) => {
                widget.render_ref(area, buf);
            }
        }
    }
}

/// Runs the onboarding event loop until the flow completes or exits.
///
/// The loop draws the screen, forwards input to the onboarding controller, and
/// performs a one-time full clear after ChatGPT login success to reset terminal
/// styling.
pub(crate) async fn run_onboarding_app(
    args: OnboardingScreenArgs,
    tui: &mut Tui,
) -> Result<OnboardingResult> {
    use tokio_stream::StreamExt;

    let mut onboarding_screen = OnboardingScreen::new(tui, args);
    // One-time guard to fully clear the screen after ChatGPT login success message is shown
    let mut did_full_clear_after_success = false;

    tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&onboarding_screen, frame.area());
    })?;

    let tui_events = tui.event_stream();
    tokio::pin!(tui_events);

    while !onboarding_screen.is_done() {
        if let Some(event) = tui_events.next().await {
            match event {
                TuiEvent::Mouse(_) => {}
                TuiEvent::Key(key_event) => {
                    onboarding_screen.handle_key_event(key_event);
                }
                TuiEvent::Paste(text) => {
                    onboarding_screen.handle_paste(text);
                }
                TuiEvent::Draw => {
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
    Ok(OnboardingResult {
        directory_trust_decision: onboarding_screen.directory_trust_decision(),
        should_exit: onboarding_screen.should_exit(),
    })
}
