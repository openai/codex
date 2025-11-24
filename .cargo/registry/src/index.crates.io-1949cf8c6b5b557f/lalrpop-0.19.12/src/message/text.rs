use ascii_canvas::AsciiView;
use crate::style::Style;

use super::*;

/// Text to be display. This will be flowed appropriately depending on
/// the container; e.g., in a Horiz, it will be one unit, but in a
/// Wrap, it will be broken up word by word.
#[derive(Debug)]
pub struct Text {
    text: String,
}

impl Text {
    pub fn new(text: String) -> Self {
        Text { text }
    }
}

impl Content for Text {
    fn min_width(&self) -> usize {
        self.text.chars().count()
    }

    fn emit(&self, view: &mut dyn AsciiView) {
        view.write_chars(0, 0, self.text.chars(), Style::new())
    }

    fn into_wrap_items(self: Box<Self>, wrap_items: &mut Vec<Box<dyn Content>>) {
        wrap_items.extend(
            self.text
                .split_whitespace()
                .map(|word| Text::new(word.to_string()))
                .map(|item| Box::new(item) as Box<dyn Content>),
        );
    }
}
