use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::WidgetRef;

use crate::key_hint;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
use crate::selection_list::selection_option_row;

use super::onboarding_screen::StepState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SophisticationLevel {
    Low,
    Medium,
    High,
}

pub(crate) struct SophisticationWidget {
    pub selection: Option<SophisticationLevel>,
    pub highlighted: SophisticationLevel,
}

impl SophisticationWidget {
    pub(crate) fn new() -> Self {
        Self {
            selection: None,
            highlighted: SophisticationLevel::Low,
        }
    }

    fn select(&mut self, level: SophisticationLevel) {
        self.highlighted = level;
        self.selection = Some(level);
    }

    fn highlighted_index(&self) -> usize {
        match self.highlighted {
            SophisticationLevel::Low => 0,
            SophisticationLevel::Medium => 1,
            SophisticationLevel::High => 2,
        }
    }

    fn highlight_index(&mut self, index: usize) {
        self.highlighted = match index {
            0 => SophisticationLevel::Low,
            1 => SophisticationLevel::Medium,
            _ => SophisticationLevel::High,
        };
    }
}

impl WidgetRef for &SophisticationWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let mut column = ColumnRenderable::new();

        column.push(Line::from(vec![
            "> ".into(),
            "What's your level of Codex sophistication?".bold(),
        ]));
        column.push("");

        column.push("");

        let options = [
            ("Low", SophisticationLevel::Low),
            ("Medium", SophisticationLevel::Medium),
            ("High", SophisticationLevel::High),
        ];

        for (idx, (label, level)) in options.iter().enumerate() {
            column.push(selection_option_row(
                idx,
                (*label).to_string(),
                self.highlighted == *level,
            ));
        }

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

impl KeyboardHandler for SophisticationWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let index = self.highlighted_index();
                if index > 0 {
                    self.highlight_index(index - 1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let index = self.highlighted_index();
                if index < 2 {
                    self.highlight_index(index + 1);
                }
            }
            KeyCode::Char('1') | KeyCode::Char('l') => self.select(SophisticationLevel::Low),
            KeyCode::Char('2') | KeyCode::Char('m') => self.select(SophisticationLevel::Medium),
            KeyCode::Char('3') | KeyCode::Char('h') => self.select(SophisticationLevel::High),
            KeyCode::Enter => self.select(self.highlighted),
            _ => {}
        }
    }
}

impl StepStateProvider for SophisticationWidget {
    fn get_step_state(&self) -> StepState {
        match self.selection {
            Some(_) => StepState::Complete,
            None => StepState::InProgress,
        }
    }
}
