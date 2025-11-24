use std::fmt;

use crate::formatter::style::{Style, StyleClass, Stylesheet};

pub struct NoOpStyle {}

impl Style for NoOpStyle {
    fn paint(&self, text: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(text)
    }

    fn paint_fn<'a>(
        &self,
        c: Box<dyn FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result + 'a>,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        c(f)
    }

    fn bold(&self) -> Box<dyn Style> {
        Box::new(NoOpStyle {})
    }
}

pub struct NoColorStylesheet;

impl Stylesheet for NoColorStylesheet {
    fn get_style(&self, _class: StyleClass) -> Box<dyn Style> {
        Box::new(NoOpStyle {})
    }
}
