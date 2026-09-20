//! Shared welcome header for the existing sign-in and Bedrock onboarding pickers.
//! The logo occupies a fixed stage so its motion never shifts the choices below it.

use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use std::cell::Cell;
use std::cell::RefCell;

use crate::empty_state_animation::EmptyStateAnimation;
use crate::empty_state_animation::Presentation;
use crate::key_hint::KeyBindingListExt;
use crate::onboarding::keys;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::tui::FrameRequester;

use super::onboarding_screen::StepState;

const MIN_ANIMATION_HEIGHT: u16 = 41;
const MIN_ANIMATION_WIDTH: u16 = 64;
const ANIMATION_WIDTH: u16 = 60;
const ANIMATION_HEIGHT: u16 = 21;

pub(crate) struct WelcomeWidget {
    pub is_logged_in: bool,
    animation: RefCell<EmptyStateAnimation>,
    request_frame: FrameRequester,
    animations_enabled: bool,
    presentation: Cell<Presentation>,
    focused: Cell<bool>,
    layout_area: Cell<Option<Rect>>,
}

impl KeyboardHandler for WelcomeWidget {
    /// Replay the welcome animation when the existing logo shortcut fires.
    ///
    /// The key list includes compatibility variants for terminals that report
    /// modifier bits differently.
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if !self.animations_enabled {
            return;
        }
        if key_event.kind == KeyEventKind::Press && keys::TOGGLE_ANIMATION.is_pressed(key_event) {
            self.animation.get_mut().start_fresh();
            self.request_frame.schedule_frame();
        }
    }
}

impl WelcomeWidget {
    pub(crate) fn new(
        is_logged_in: bool,
        request_frame: FrameRequester,
        animations_enabled: bool,
    ) -> Self {
        let mut animation = EmptyStateAnimation::default();
        animation.start_fresh();
        Self {
            is_logged_in,
            animation: RefCell::new(animation),
            request_frame,
            animations_enabled,
            presentation: Cell::new(Presentation::Animated),
            focused: Cell::new(/*value*/ true),
            layout_area: Cell::new(None),
        }
    }

    pub(crate) fn update_layout_area(&self, area: Rect) {
        self.layout_area.set(Some(area));
    }

    pub(crate) fn set_presentation(&self, presentation: Presentation) {
        if presentation == Presentation::Hidden {
            self.animation.borrow_mut().pause_clock();
        }
        self.presentation.set(presentation);
    }

    pub(crate) fn set_focused(&self, focused: bool) {
        self.focused.set(focused);
    }
}

impl WidgetRef for &WelcomeWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let layout_area = self.layout_area.get().unwrap_or(area);
        // Skip the animation entirely when the viewport is too small so we don't clip frames.
        let show_animation = self.animations_enabled
            && self.presentation.get() != Presentation::Hidden
            && layout_area.height >= MIN_ANIMATION_HEIGHT
            && layout_area.width >= MIN_ANIMATION_WIDTH;

        let presentation = if !show_animation {
            Presentation::Hidden
        } else if !self.focused.get() {
            Presentation::Faded
        } else {
            self.presentation.get()
        };
        let stage = Rect::new(area.x, area.y, ANIMATION_WIDTH, ANIMATION_HEIGHT);
        if let Some(delay) = self
            .animation
            .borrow_mut()
            .render_in(stage, buf, presentation)
        {
            self.request_frame.schedule_frame_in(delay);
        }

        let logo_rows = if show_animation {
            ANIMATION_HEIGHT + 1
        } else {
            0
        };
        let mut lines: Vec<Line> = vec![Line::default(); usize::from(logo_rows)];
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
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn row_containing(buf: &Buffer, needle: &str) -> Option<u16> {
        (0..buf.area.height).find(|&y| {
            let mut row = String::new();
            for x in 0..buf.area.width {
                row.push_str(buf[(x, y)].symbol());
            }
            row.contains(needle)
        })
    }

    #[test]
    fn welcome_renders_animation_on_first_draw() {
        let widget = WelcomeWidget::new(
            /*is_logged_in*/ false,
            FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
        );
        let area = Rect::new(0, 0, MIN_ANIMATION_WIDTH, MIN_ANIMATION_HEIGHT);
        let mut buf = Buffer::empty(area);
        (&widget).render_ref(area, &mut buf);

        let welcome_row = row_containing(&buf, "Welcome");
        assert_eq!(welcome_row, Some(ANIMATION_HEIGHT + 1));
        widget.set_focused(/*focused*/ false);
        (&widget).render_ref(area, &mut buf);
        assert_eq!(row_containing(&buf, "Welcome"), welcome_row);
    }

    #[test]
    fn welcome_skips_animation_below_height_breakpoint() {
        let widget = WelcomeWidget::new(
            /*is_logged_in*/ false,
            FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
        );
        let area = Rect::new(0, 0, MIN_ANIMATION_WIDTH, MIN_ANIMATION_HEIGHT - 1);
        let mut buf = Buffer::empty(area);
        (&widget).render_ref(area, &mut buf);

        let welcome_row = row_containing(&buf, "Welcome");
        assert_eq!(welcome_row, Some(0));
    }

    #[test]
    fn welcome_logo_layout() {
        let (width, height) = (160, 48);
        let widget = WelcomeWidget::new(
            /*is_logged_in*/ false,
            FrameRequester::test_dummy(),
            /*animations_enabled*/ true,
        );
        // The first focused draw shows the intact logo at exactly zero elapsed time.
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut buf = Buffer::empty(area);
        (&widget).render_ref(area, &mut buf);
        let visible = (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!(format!("welcome_logo_{width}x{height}"), visible.trim_end());
    }
}
