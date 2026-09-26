//! One expendable tip row shared by working status and completed-response presentation.
//! Drawing records exposure; measuring or clipping the row does not.

use std::cell::Cell;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use crate::render::renderable::Renderable;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::HyperlinkParagraph;

pub(crate) struct TurnTip {
    pub(crate) line: HyperlinkLine,
    pub(crate) rendered: Cell<bool>,
}

impl Renderable for TurnTip {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if !area.is_empty() && self.line.width() <= usize::from(area.width) {
            HyperlinkParagraph::new(std::slice::from_ref(&self.line), Style::default())
                .render(Rect { height: 1, ..area }, buf);
            self.rendered.set(/*val*/ true);
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        u16::from(self.line.width() <= usize::from(width))
    }
}
