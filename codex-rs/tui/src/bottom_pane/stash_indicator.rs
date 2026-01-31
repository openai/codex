use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::render::renderable::Renderable;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_lines;

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

        let wrapped = word_wrap_lines(
            vec!["Stashed (restores after current message is sent)"]
                .into_iter()
                .map(|line| line.dim().italic()),
            RtOptions::new(width as usize)
                .initial_indent(Line::from("  ↳ ".dim()))
                .subsequent_indent(Line::from("    ")),
        );

        Paragraph::new(wrapped).into()
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
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn desired_height_no_stash() {
        let stash = StashIndicator::new();
        assert_eq!(stash.desired_height(40), 0);
    }

    #[test]
    fn desired_height_stash() {
        let mut stash = StashIndicator::new();
        stash.stash_exists = true;
        assert_eq!(stash.desired_height(40), 2);
    }

    #[test]
    fn render_wrapped_message() {
        let mut stash = StashIndicator::new();
        stash.stash_exists = true;
        let width = 20;
        let height = stash.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        stash.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_stash_wrapped_message", format!("{buf:?}"));
    }
}
