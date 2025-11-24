use std::fmt::{self, Display};

use yansi_term::{Color::Fixed, Style as AnsiTermStyle};

use crate::formatter::style::{Style, StyleClass, Stylesheet};

struct AnsiTermStyleWrapper {
    style: AnsiTermStyle,
}

impl Style for AnsiTermStyleWrapper {
    fn paint(&self, text: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.style.paint(text).fmt(f)
    }

    fn paint_fn<'a>(
        &self,
        c: Box<dyn FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result + 'a>,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        self.style.paint_fn(c).fmt(f)
    }

    fn bold(&self) -> Box<dyn Style> {
        Box::new(AnsiTermStyleWrapper { style: self.style })
    }
}

pub struct AnsiTermStylesheet;

impl Stylesheet for AnsiTermStylesheet {
    fn get_style(&self, class: StyleClass) -> Box<dyn Style> {
        let ansi_term_style = match class {
            StyleClass::Error => Fixed(9).bold(),
            StyleClass::Warning => Fixed(11).bold(),
            StyleClass::Info => Fixed(12).bold(),
            StyleClass::Note => AnsiTermStyle::new().bold(),
            StyleClass::Help => Fixed(14).bold(),

            StyleClass::LineNo => Fixed(12).bold(),

            StyleClass::Emphasis => AnsiTermStyle::new().bold(),

            StyleClass::None => AnsiTermStyle::new(),
        };
        Box::new(AnsiTermStyleWrapper {
            style: ansi_term_style,
        })
    }
}
