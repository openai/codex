use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use textwrap::wrap;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::style::user_message_style;
use crate::wrapping::word_wrap_lines;

pub(crate) struct AppLinkView {
    title: String,
    description: Option<String>,
    instructions: String,
    url: String,
    is_installed: bool,
    complete: bool,
}

impl AppLinkView {
    pub(crate) fn new(
        title: String,
        description: Option<String>,
        instructions: String,
        url: String,
        is_installed: bool,
    ) -> Self {
        Self {
            title,
            description,
            instructions,
            url,
            is_installed,
            complete: false,
        }
    }

    fn content_lines(&self, width: u16) -> Vec<Line<'static>> {
        let usable_width = width.max(1) as usize;
        let mut lines: Vec<Line<'static>> = Vec::new();

        lines.push(Line::from(self.title.clone().bold()));
        if let Some(description) = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|description| !description.is_empty())
        {
            for line in wrap(description, usable_width) {
                lines.push(Line::from(line.into_owned().dim()));
            }
        }
        lines.push(Line::from(""));
        if self.is_installed {
            for line in wrap("Use $ to insert this app into the prompt.", usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
            lines.push(Line::from(""));
        }

        let instructions = self.instructions.trim();
        if !instructions.is_empty() {
            for line in wrap(instructions, usable_width) {
                lines.push(Line::from(line.into_owned()));
            }
            for line in wrap(
                "Newly installed apps can take a few minutes to appear in /apps.",
                usable_width,
            ) {
                lines.push(Line::from(line.into_owned()));
            }
            if !self.is_installed {
                for line in wrap(
                    "After installed, use $ to insert this app into the prompt.",
                    usable_width,
                ) {
                    lines.push(Line::from(line.into_owned()));
                }
            }
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec!["Open:".dim()]));
        let url_line = Line::from(vec![self.url.clone().cyan().underlined()]);
        lines.extend(word_wrap_lines(vec![url_line], usable_width));

        lines
    }
}

impl BottomPaneView for AppLinkView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if let KeyEvent {
            code: KeyCode::Esc, ..
        } = key_event
        {
            self.on_ctrl_c();
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }
}

impl crate::render::renderable::Renderable for AppLinkView {
    fn desired_height(&self, width: u16) -> u16 {
        let content_width = width.saturating_sub(4).max(1);
        let content_lines = self.content_lines(content_width);
        content_lines.len() as u16 + 3
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        Block::default()
            .style(user_message_style())
            .render(area, buf);

        let [content_area, hint_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        let inner = content_area.inset(Insets::vh(1, 2));
        let content_width = inner.width.max(1);
        let lines = self.content_lines(content_width);
        Paragraph::new(lines).render(inner, buf);

        if hint_area.height > 0 {
            let hint_area = Rect {
                x: hint_area.x.saturating_add(2),
                y: hint_area.y,
                width: hint_area.width.saturating_sub(2),
                height: hint_area.height,
            };
            hint_line().dim().render(hint_area, buf);
        }
    }
}

fn hint_line() -> Line<'static> {
    Line::from(vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Esc).into(),
        " to close".into(),
    ])
}
