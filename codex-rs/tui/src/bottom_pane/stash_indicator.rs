use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::render::renderable::Renderable;

pub(crate) struct StashIndicator {
    pub stash_exists: bool,
}

impl StashIndicator {
    pub(crate) fn new() -> Self {
        Self {
            stash_exists: false,
        }
    }

    fn as_renderable(&self, width: u16) -> Box<dyn Renderable> {
        if !self.stash_exists || width < 4 {
            return Box::new(());
        }

        Paragraph::new(vec![Line::from(" ⬇ Stashed changes ".dim().italic())]).into()
    }
}

impl Renderable for StashIndicator {
    fn render(&self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        if area.is_empty() {
            return;
        }

        self.as_renderable(area.width).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.as_renderable(width).desired_height(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    // use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::{buffer::Buffer, layout::Rect};

    #[test]
    fn desired_height_no_stash() {
        let stash = StashIndicator::new();
        assert_eq!(stash.desired_height(40), 0);
    }

    #[test]
    fn desired_height_stash() {
        let mut stash = StashIndicator::new();
        stash.stash_exists = true;
        assert_eq!(stash.desired_height(40), 1);
    }

    #[test]
    fn render_wrapped_message() {
        let mut stash = StashIndicator::new();
        stash.stash_exists = true;
        let width = 10;
        let height = stash.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        stash.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_stash_wrapped_message", format!("{buf:?}"));
    }
}
