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

pub(crate) struct WelcomeWidget {
    pub is_logged_in: bool,
    pub request_frame: FrameRequester,
    pub start: Instant,
}

impl WidgetRef for &WelcomeWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        self.request_frame.schedule_frame_in(FRAME_TICK);

        let frames = &FRAMES_DEFAULT;
        let idx = if FRAME_TICK.as_millis() > 0 {
            let steps =
                (self.start.elapsed().as_millis() / FRAME_TICK.as_millis()) % frames.len() as u128;
            steps as usize
        } else {
            0
        };

        let mut lines: Vec<Line> = frames[idx].lines().map(|l| l.into()).collect();

        lines.push("".into());
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
