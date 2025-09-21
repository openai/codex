use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::accessibility::animations_enabled;
use crate::ascii_animation::AsciiAnimation;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::tui::FrameRequester;

use super::onboarding_screen::StepState;

const MIN_ANIMATION_HEIGHT: u16 = 20;
const MIN_ANIMATION_WIDTH: u16 = 60;

pub(crate) struct WelcomeWidget {
    pub is_logged_in: bool,
    animation: AsciiAnimation,
}

impl KeyboardHandler for WelcomeWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Press
            && key_event.code == KeyCode::Char('.')
            && key_event.modifiers.contains(KeyModifiers::CONTROL)
        {
            tracing::warn!("Welcome background to press '.'");
            let _ = self.animation.pick_random_variant();
        }
    }
}

impl WelcomeWidget {
    pub(crate) fn new(is_logged_in: bool, request_frame: FrameRequester) -> Self {
        let mut animation = AsciiAnimation::new(request_frame);
        if !animations_enabled() {
            animation.set_enabled(false);
        }

        Self {
            is_logged_in,
            animation,
        }
    }
}

impl WidgetRef for &WelcomeWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        // Skip the animation entirely when the viewport is too small so we don't clip frames.
        let show_animation =
            area.height >= MIN_ANIMATION_HEIGHT && area.width >= MIN_ANIMATION_WIDTH;

        if show_animation && self.animation.is_enabled() {
            self.animation.schedule_next_frame();
        }

        let mut lines: Vec<Line> = Vec::new();
        if show_animation {
            let frame = self.animation.current_frame();
            // let frame_line_count = frame.lines().count();
            // lines.reserve(frame_line_count + 2);
            lines.extend(frame.lines().map(|l| l.into()));
            lines.push("".into());
        }
        lines.push(Line::from(vec![
            "  ".into(),
            "Welcome to ".into(),
            "Codex".bold(),
            ", OpenAI's command-line coding agent".into(),
        ]));

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

impl StepStateProvider for WelcomeWidget {
    fn get_step_state(&self) -> StepState {
        match self.is_logged_in {
            true => StepState::Hidden,
            false => StepState::Complete,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::reset_cache_for_tests;
    use crate::accessibility::with_cli_animations_disabled_for_tests;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use serial_test::serial;

    static VARIANT_A: [&str; 1] = ["frame-a"];
    static VARIANT_B: [&str; 1] = ["frame-b"];
    static VARIANTS: [&[&str]; 2] = [&VARIANT_A, &VARIANT_B];

    const SCREEN_READER_ENV_VARS: [&str; 5] = [
        "NVDA_RUNNING",
        "JAWS",
        "ORCA_RUNNING",
        "SPEECHD_RUNNING",
        "ACCESSIBILITY_ENABLED",
    ];

    fn set_env(var: &str, value: &str) {
        // Safety: Tests using this helper are serialised via #[serial].
        unsafe {
            std::env::set_var(var, value);
        }
    }

    fn remove_env(var: &str) {
        // Safety: Tests using this helper are serialised via #[serial].
        unsafe {
            std::env::remove_var(var);
        }
    }

    fn with_env<F, R>(screen_reader_env: Option<(&str, &str)>, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let saved: Vec<(&'static str, Option<String>)> = SCREEN_READER_ENV_VARS
            .iter()
            .map(|&var| (var, std::env::var(var).ok()))
            .collect();

        for &var in &SCREEN_READER_ENV_VARS {
            remove_env(var);
        }

        if let Some((var, value)) = screen_reader_env {
            set_env(var, value);
        }

        reset_cache_for_tests();
        let result = f();

        for (var, value) in saved {
            if let Some(value) = value {
                set_env(var, &value);
            } else {
                remove_env(var);
            }
        }

        reset_cache_for_tests();
        result
    }

    fn with_screen_reader_env<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        with_env(Some(("NVDA_RUNNING", "1")), f)
    }

    fn with_no_screen_reader_env<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        with_env(None, f)
    }

    fn buffer_contains(buf: &Buffer, area: Rect, needle: &str) -> bool {
        for y in 0..area.height {
            let mut row = String::new();
            for x in 0..area.width {
                row.push_str(buf[(x, y)].symbol());
            }
            if row.contains(needle) {
                return true;
            }
        }

        false
    }

    #[test]
    #[serial]
    fn disables_animation_when_screen_reader_detected() {
        with_cli_animations_disabled_for_tests(false, || {
            with_screen_reader_env(|| {
                let widget = WelcomeWidget::new(false, FrameRequester::test_dummy());
                assert_eq!(widget.animation.is_enabled(), false);
                assert_eq!(widget.animation.current_frame(), "");
            });
        });
    }

    #[test]
    #[serial]
    fn disables_animation_when_cli_flag_set() {
        with_cli_animations_disabled_for_tests(true, || {
            with_no_screen_reader_env(|| {
                let widget = WelcomeWidget::new(false, FrameRequester::test_dummy());
                assert_eq!(widget.animation.is_enabled(), false);
            });
        });
    }

    #[test]
    #[serial]
    fn keeps_animation_enabled_when_no_screen_reader_detected() {
        with_cli_animations_disabled_for_tests(false, || {
            with_no_screen_reader_env(|| {
                let widget = WelcomeWidget::new(false, FrameRequester::test_dummy());
                assert_eq!(widget.animation.is_enabled(), true);
                assert!(
                    !widget.animation.current_frame().is_empty(),
                    "expected animation frame when no screen reader is active"
                );
            });
        });
    }

    #[test]
    #[serial]
    fn render_omits_animation_when_screen_reader_is_active() {
        with_cli_animations_disabled_for_tests(false, || {
            with_screen_reader_env(|| {
                let mut widget = WelcomeWidget::new(false, FrameRequester::test_dummy());
                let was_enabled = widget.animation.is_enabled();
                assert_eq!(was_enabled, false);

                widget.animation =
                    AsciiAnimation::with_variants(FrameRequester::test_dummy(), &VARIANTS, 0);
                widget.animation.set_enabled(was_enabled);

                let area = Rect::new(0, 0, MIN_ANIMATION_WIDTH, MIN_ANIMATION_HEIGHT);
                let mut buf = Buffer::empty(area);
                (&widget).render(area, &mut buf);

                assert!(
                    !buffer_contains(&buf, area, "frame-a"),
                    "expected animation content to be hidden when screen reader is active"
                );
                assert!(
                    buffer_contains(&buf, area, "Codex"),
                    "expected welcome text to remain visible when screen reader is active"
                );
            });
        });
    }

    #[test]
    #[serial]
    fn render_does_not_schedule_frame_when_animation_disabled() {
        with_cli_animations_disabled_for_tests(false, || {
            with_screen_reader_env(|| {
                let (requester, mut counter) = FrameRequester::test_with_counter();
                let widget = WelcomeWidget::new(false, requester);
                assert_eq!(widget.animation.is_enabled(), false);

                let area = Rect::new(0, 0, MIN_ANIMATION_WIDTH, MIN_ANIMATION_HEIGHT);
                let mut buf = Buffer::empty(area);
                (&widget).render(area, &mut buf);

                assert_eq!(
                    counter.take_count(),
                    0,
                    "expected no frame scheduling when animation is disabled"
                );
            });
        });
    }

    #[test]
    #[serial]
    fn render_includes_animation_when_screen_reader_is_inactive() {
        with_cli_animations_disabled_for_tests(false, || {
            with_no_screen_reader_env(|| {
                let mut widget = WelcomeWidget::new(false, FrameRequester::test_dummy());
                let was_enabled = widget.animation.is_enabled();
                assert_eq!(was_enabled, true);

                widget.animation =
                    AsciiAnimation::with_variants(FrameRequester::test_dummy(), &VARIANTS, 0);
                widget.animation.set_enabled(was_enabled);

                let area = Rect::new(0, 0, MIN_ANIMATION_WIDTH, MIN_ANIMATION_HEIGHT);
                let mut buf = Buffer::empty(area);
                (&widget).render(area, &mut buf);

                assert!(
                    buffer_contains(&buf, area, "frame-a"),
                    "expected animation content to render when no screen reader is active"
                );
                assert!(
                    buffer_contains(&buf, area, "Codex"),
                    "expected welcome text to render when no screen reader is active"
                );
            });
        });
    }

    #[test]
    #[serial]
    fn welcome_renders_animation_on_first_draw() {
        with_cli_animations_disabled_for_tests(false, || {
            with_no_screen_reader_env(|| {
                let widget = WelcomeWidget::new(false, FrameRequester::test_dummy());
                let area = Rect::new(0, 0, MIN_ANIMATION_WIDTH, MIN_ANIMATION_HEIGHT);
                let mut buf = Buffer::empty(area);
                (&widget).render(area, &mut buf);

                let mut found = false;
                let mut last_non_empty: Option<u16> = None;
                for y in 0..area.height {
                    for x in 0..area.width {
                        if !buf[(x, y)].symbol().trim().is_empty() {
                            found = true;
                            last_non_empty = Some(y);
                            break;
                        }
                    }
                }

                assert!(found, "expected welcome animation to render characters");
                let measured_rows = last_non_empty.map(|v| v + 2).unwrap_or(0);
                assert!(
                    measured_rows >= MIN_ANIMATION_HEIGHT,
                    "expected measurement to report at least {MIN_ANIMATION_HEIGHT} rows, got {measured_rows}"
                );
            });
        });
    }

    #[test]
    #[serial]
    fn ctrl_dot_changes_animation_variant() {
        with_cli_animations_disabled_for_tests(false, || {
            with_no_screen_reader_env(|| {
                let mut widget = WelcomeWidget {
                    is_logged_in: false,
                    animation: AsciiAnimation::with_variants(
                        FrameRequester::test_dummy(),
                        &VARIANTS,
                        0,
                    ),
                };

                let before = widget.animation.current_frame();
                widget.handle_key_event(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::CONTROL));
                let after = widget.animation.current_frame();

                assert_ne!(
                    before, after,
                    "expected ctrl+. to switch welcome animation variant"
                );
            });
        });
    }
}
