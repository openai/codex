use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use crate::key_hint;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;

use super::onboarding_screen::StepState;

pub(crate) struct BuildTestCommandsWidget {
    pub value: String,
    submitted: bool,
}

impl BuildTestCommandsWidget {
    pub(crate) fn new() -> Self {
        Self {
            value: String::new(),
            submitted: false,
        }
    }

    pub(crate) fn commands(&self) -> Option<String> {
        if !self.submitted {
            return None;
        }
        let trimmed = self.value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    fn append_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.value.push_str(text);
    }
}

impl WidgetRef for &BuildTestCommandsWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut column = ColumnRenderable::new();

        column.push(Line::from(vec![
            "> ".into(),
            "What are the build and test commands?".bold(),
        ]));
        column.push("");
        column.push(
            Paragraph::new(
                "Add the commands you'd like Codex to run (comma-separated or a single command)."
                    .to_string(),
            )
            .wrap(Wrap { trim: true })
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push("");

        column.push("Build & test commands:".dim());
        let content_line: Line = if self.value.is_empty() {
            vec!["e.g. just fmt, just test, cargo test -p codex-tui2".dim()].into()
        } else {
            Line::from(self.value.clone())
        };
        column.push(content_line.inset(Insets::tlbr(0, 2, 0, 0)));

        column.push("");
        column.push(
            Line::from(vec![
                "Press ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " to continue".dim(),
            ])
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );

        column.render(area, buf);
    }
}

impl KeyboardHandler for BuildTestCommandsWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        match key_event.code {
            KeyCode::Enter => self.submitted = true,
            KeyCode::Backspace => {
                self.value.pop();
            }
            KeyCode::Char(c)
                if key_event.kind == KeyEventKind::Press
                    && !key_event.modifiers.contains(KeyModifiers::SUPER)
                    && !key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && !key_event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.value.push(c);
            }
            _ => {}
        }
    }

    fn handle_paste(&mut self, pasted: String) {
        let trimmed = pasted.trim();
        if trimmed.is_empty() {
            return;
        }
        let cleaned = trimmed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        self.append_text(&cleaned);
    }
}

impl StepStateProvider for BuildTestCommandsWidget {
    fn get_step_state(&self) -> StepState {
        if self.submitted {
            StepState::Complete
        } else {
            StepState::InProgress
        }
    }
}
