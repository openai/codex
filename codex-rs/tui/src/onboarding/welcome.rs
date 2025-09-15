use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::frames::FRAME_TICK_DEFAULT;
use crate::frames::FRAMES_DEFAULT;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::tui::FrameRequester;

use super::onboarding_screen::StepState;
use std::time::Duration;
use std::time::Instant;

const FRAME_TICK: Duration = FRAME_TICK_DEFAULT;
const MIN_ANIMATION_HEIGHT: u16 = 21;
const MIN_ANIMATION_WIDTH: u16 = 60;

pub(crate) struct WelcomeWidget {
    pub is_logged_in: bool,
    pub request_frame: FrameRequester,
    pub start: Instant,
}

impl WidgetRef for &WelcomeWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let elapsed_ms = self.start.elapsed().as_millis();

        // Align next draw to the next FRAME_TICK boundary to reduce jitter.
        {
            let tick_ms = FRAME_TICK.as_millis();
            let rem_ms = elapsed_ms % tick_ms;
            let delay_ms = if rem_ms == 0 {
                tick_ms
            } else {
                tick_ms - rem_ms
            };
            // Safe cast: delay_ms < tick_ms and FRAME_TICK is small.
            self.request_frame
                .schedule_frame_in(Duration::from_millis(delay_ms as u64));
        }

        let frames = &FRAMES_DEFAULT;
        let idx = ((elapsed_ms / FRAME_TICK.as_millis()) % frames.len() as u128) as usize;
        // Skip the animation entirely when the viewport is too small so we don't clip frames.
        let show_animation =
            area.height >= MIN_ANIMATION_HEIGHT && area.width >= MIN_ANIMATION_WIDTH;

        let mut lines: Vec<Line> = Vec::new();
        if show_animation {
            let frame_line_count = frames[idx].lines().count();
            lines.reserve(frame_line_count + 2);
            lines.extend(frames[idx].lines().map(|l| l.into()));
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

    /// A number of things break down if FRAME_TICK is zero.
    #[test]
    fn frame_tick_must_be_nonzero() {
        assert!(FRAME_TICK.as_millis() > 0);
    }
}
