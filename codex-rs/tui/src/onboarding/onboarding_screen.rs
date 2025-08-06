use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use codex_login::AuthMode;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::onboarding::auth::AuthModeWidget;
use crate::onboarding::auth::SignInState;
use crate::onboarding::welcome::WelcomeWidget;
use std::path::PathBuf;

enum Step {
    Welcome(WelcomeWidget),
    Auth(AuthModeWidget),
}

pub(crate) trait KeyboardHandler {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> KeyEventResult;
}

pub(crate) enum KeyEventResult {
    Continue,
    Quit,
    None,
}

pub(crate) struct OnboardingScreen {
    event_tx: AppEventSender,
    steps: Vec<Step>,
}

impl OnboardingScreen {
    pub(crate) fn new(event_tx: AppEventSender, codex_home: PathBuf) -> Self {
        let steps: Vec<Step> = vec![
            Step::Welcome(WelcomeWidget {}),
            Step::Auth(AuthModeWidget {
                event_tx: event_tx.clone(),
                mode: AuthMode::ChatGPT,
                error: None,
                sign_in_state: SignInState::PickMode,
                codex_home,
            }),
        ];
        Self { event_tx, steps }
    }

    pub(crate) fn on_auth_complete(&mut self, result: Result<(), String>) -> KeyEventResult {
        if let Some(Step::Auth(state)) = self.steps.last_mut() {
            match result {
                Ok(()) => {
                    state.sign_in_state = SignInState::ChatGptSuccess;
                    self.event_tx.send(AppEvent::RequestRedraw);
                    KeyEventResult::None
                }
                Err(e) => {
                    state.sign_in_state = SignInState::PickMode;
                    state.error = Some(e);
                    self.event_tx.send(AppEvent::RequestRedraw);
                    KeyEventResult::None
                }
            }
        } else {
            KeyEventResult::None
        }
    }
}

impl KeyboardHandler for OnboardingScreen {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> KeyEventResult {
        if let Some(last_step) = self.steps.last_mut() {
            self.event_tx.send(AppEvent::RequestRedraw);
            last_step.handle_key_event(key_event)
        } else {
            KeyEventResult::None
        }
    }
}

impl WidgetRef for &OnboardingScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
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
        while i < self.steps.len() && y < bottom {
            let step = &self.steps[i];
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
                step.render_ref(target, buf);
                y = y.saturating_add(h);
            }
            i += 1;
        }
    }
}

impl KeyboardHandler for Step {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> KeyEventResult {
        match self {
            Step::Welcome(_) => KeyEventResult::None,
            Step::Auth(widget) => widget.handle_key_event(key_event),
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
