use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::WidgetRef;

pub(crate) struct WelcomeWidget {}

impl WidgetRef for &WelcomeWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let line = Line::from(vec![
            Span::raw("> "),
            Span::styled(
                "Welcome to Codex, OpenAI's coding agent that runs in your terminal",
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]);
        line.render(area, buf);
    }
}
