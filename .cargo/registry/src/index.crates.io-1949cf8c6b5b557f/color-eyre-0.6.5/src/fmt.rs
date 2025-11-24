//! Module for new types that isolate complext formatting
use std::fmt;

use owo_colors::OwoColorize;

pub(crate) struct LocationSection<'a>(
    pub(crate) Option<&'a std::panic::Location<'a>>,
    pub(crate) crate::config::Theme,
);

impl fmt::Display for LocationSection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let theme = self.1;
        // If known, print panic location.
        if let Some(loc) = self.0 {
            write!(f, "{}", loc.file().style(theme.panic_file))?;
            write!(f, ":")?;
            write!(f, "{}", loc.line().style(theme.panic_line_number))?;
        } else {
            write!(f, "<unknown>")?;
        }

        Ok(())
    }
}
