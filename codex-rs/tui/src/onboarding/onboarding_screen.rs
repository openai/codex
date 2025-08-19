use codex_core::util::is_inside_git_repo;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::widgets::Clear;
use ratatui::widgets::WidgetRef;

use codex_login::AuthMode;

use crate::LoginStatus;
use crate::app::ChatWidgetArgs;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::onboarding::auth::AuthModeWidget;
use crate::onboarding::auth::SignInState;
use crate::onboarding::continue_to_chat::ContinueToChatWidget;
use crate::onboarding::trust_directory::TrustDirectorySelection;
use crate::onboarding::trust_directory::TrustDirectoryWidget;
use crate::onboarding::welcome::WelcomeWidget;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

#[allow(clippy::large_enum_variant)]
enum Step {
    Welcome(WelcomeWidget),
    Auth(AuthModeWidget),
    TrustDirectory(TrustDirectoryWidget),
    ContinueToChat(ContinueToChatWidget),
}

pub(crate) trait KeyboardHandler {
    fn handle_key_event(&mut self, key_event: KeyEvent);
}

pub(crate) enum StepState {
    Hidden,
    InProgress,
    Complete,
}

pub(crate) trait StepStateProvider {
    fn get_step_state(&self) -> StepState;
}

pub(crate) struct OnboardingScreen {
    event_tx: AppEventSender,
    steps: Vec<Step>,
}

pub(crate) struct OnboardingScreenArgs {
    pub event_tx: AppEventSender,
    pub chat_widget_args: ChatWidgetArgs,
    pub codex_home: PathBuf,
    pub cwd: PathBuf,
    pub show_trust_screen: bool,
    pub show_login_screen: bool,
    pub login_status: LoginStatus,
}

impl OnboardingScreen {
    pub(crate) fn new(args: OnboardingScreenArgs) -> Self {
        let OnboardingScreenArgs {
            event_tx,
            chat_widget_args,
            codex_home,
            cwd,
            show_trust_screen,
            show_login_screen,
            login_status,
        } = args;
        let mut steps: Vec<Step> = vec![Step::Welcome(WelcomeWidget {
            is_logged_in: !matches!(login_status, LoginStatus::NotAuthenticated),
        })];
        if show_login_screen {
            steps.push(Step::Auth(AuthModeWidget {
                event_tx: event_tx.clone(),
                highlighted_mode: AuthMode::ChatGPT,
                error: None,
                sign_in_state: SignInState::PickMode,
                codex_home: codex_home.clone(),
                login_status,
                preferred_auth_method: chat_widget_args.config.preferred_auth_method,
            }))
        }
        let is_git_repo = is_inside_git_repo(&cwd);
        let highlighted = if is_git_repo {
            TrustDirectorySelection::Trust
        } else {
            // Default to not trusting the directory if it's not a git repo.
            TrustDirectorySelection::DontTrust
        };
        // Share ChatWidgetArgs between steps so changes in the TrustDirectory step
        // are reflected when continuing to chat.
        let shared_chat_args = Arc::new(Mutex::new(chat_widget_args));
        if show_trust_screen {
            steps.push(Step::TrustDirectory(TrustDirectoryWidget {
                cwd,
                codex_home,
                is_git_repo,
                selection: None,
                highlighted,
                error: None,
                chat_widget_args: shared_chat_args.clone(),
            }))
        }
        steps.push(Step::ContinueToChat(ContinueToChatWidget {
            event_tx: event_tx.clone(),
            chat_widget_args: shared_chat_args,
        }));
        // TODO: add git warning.
        Self { event_tx, steps }
    }

    pub(crate) fn on_auth_complete(&mut self, result: Result<(), String>) {
        let current_step = self.current_step_mut();
        if let Some(Step::Auth(state)) = current_step {
            match result {
                Ok(()) => {
                    state.sign_in_state = SignInState::ChatGptSuccessMessage;
                    self.event_tx.send(AppEvent::RequestRedraw);
                    let tx1 = self.event_tx.clone();
                    let tx2 = self.event_tx.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        tx1.send(AppEvent::RequestRedraw);
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        tx2.send(AppEvent::RequestRedraw);
                    });
                }
                Err(e) => {
                    state.sign_in_state = SignInState::PickMode;
                    state.error = Some(e);
                    self.event_tx.send(AppEvent::RequestRedraw);
                }
            }
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

    fn current_step_mut(&mut self) -> Option<&mut Step> {
        self.steps
            .iter_mut()
            .find(|step| matches!(step.get_step_state(), StepState::InProgress))
    }
}

impl KeyboardHandler for OnboardingScreen {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if let Some(active_step) = self.current_steps_mut().into_iter().last() {
            active_step.handle_key_event(key_event);
        }
        self.event_tx.send(AppEvent::RequestRedraw);
    }
}

impl WidgetRef for &OnboardingScreen {
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
                    let sym = tmp[(xx, yy)].symbol();
                    if !sym.trim().is_empty() {
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
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self {
            Step::Welcome(_) | Step::ContinueToChat(_) => (),
            Step::Auth(widget) => widget.handle_key_event(key_event),
            Step::TrustDirectory(widget) => widget.handle_key_event(key_event),
        }
    }
}

impl StepStateProvider for Step {
    fn get_step_state(&self) -> StepState {
        match self {
            Step::Welcome(w) => w.get_step_state(),
            Step::Auth(w) => w.get_step_state(),
            Step::TrustDirectory(w) => w.get_step_state(),
            Step::ContinueToChat(w) => w.get_step_state(),
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
            Step::TrustDirectory(widget) => {
                widget.render_ref(area, buf);
            }
            Step::ContinueToChat(widget) => {
                widget.render_ref(area, buf);
            }
        }
    }
}
