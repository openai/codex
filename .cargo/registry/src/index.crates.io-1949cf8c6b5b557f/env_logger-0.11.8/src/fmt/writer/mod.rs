mod buffer;
mod target;

use self::buffer::BufferWriter;
use std::{io, mem, sync::Mutex};

pub(super) use self::buffer::Buffer;

pub use target::Target;

/// Whether or not to print styles to the target.
#[allow(clippy::exhaustive_enums)] // By definition don't need more
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Default)]
pub enum WriteStyle {
    /// Try to print styles, but don't force the issue.
    #[default]
    Auto,
    /// Try very hard to print styles.
    Always,
    /// Never print styles.
    Never,
}

#[cfg(feature = "color")]
impl From<anstream::ColorChoice> for WriteStyle {
    fn from(choice: anstream::ColorChoice) -> Self {
        match choice {
            anstream::ColorChoice::Auto => Self::Auto,
            anstream::ColorChoice::Always => Self::Always,
            anstream::ColorChoice::AlwaysAnsi => Self::Always,
            anstream::ColorChoice::Never => Self::Never,
        }
    }
}

#[cfg(feature = "color")]
impl From<WriteStyle> for anstream::ColorChoice {
    fn from(choice: WriteStyle) -> Self {
        match choice {
            WriteStyle::Auto => anstream::ColorChoice::Auto,
            WriteStyle::Always => anstream::ColorChoice::Always,
            WriteStyle::Never => anstream::ColorChoice::Never,
        }
    }
}

/// A terminal target with color awareness.
#[derive(Debug)]
pub(crate) struct Writer {
    inner: BufferWriter,
}

impl Writer {
    pub(crate) fn write_style(&self) -> WriteStyle {
        self.inner.write_style()
    }

    pub(super) fn buffer(&self) -> Buffer {
        self.inner.buffer()
    }

    pub(super) fn print(&self, buf: &Buffer) -> io::Result<()> {
        self.inner.print(buf)
    }
}

/// A builder for a terminal writer.
///
/// The target and style choice can be configured before building.
#[derive(Debug)]
pub(crate) struct Builder {
    target: Target,
    write_style: WriteStyle,
    is_test: bool,
    built: bool,
}

impl Builder {
    /// Initialize the writer builder with defaults.
    pub(crate) fn new() -> Self {
        Builder {
            target: Default::default(),
            write_style: Default::default(),
            is_test: false,
            built: false,
        }
    }

    /// Set the target to write to.
    pub(crate) fn target(&mut self, target: Target) -> &mut Self {
        self.target = target;
        self
    }

    /// Parses a style choice string.
    ///
    /// See the [Disabling colors] section for more details.
    ///
    /// [Disabling colors]: ../index.html#disabling-colors
    pub(crate) fn parse_write_style(&mut self, write_style: &str) -> &mut Self {
        self.write_style(parse_write_style(write_style))
    }

    /// Whether or not to print style characters when writing.
    pub(crate) fn write_style(&mut self, write_style: WriteStyle) -> &mut Self {
        self.write_style = write_style;
        self
    }

    /// Whether or not to capture logs for `cargo test`.
    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn is_test(&mut self, is_test: bool) -> &mut Self {
        self.is_test = is_test;
        self
    }

    /// Build a terminal writer.
    pub(crate) fn build(&mut self) -> Writer {
        assert!(!self.built, "attempt to re-use consumed builder");
        self.built = true;

        let color_choice = self.write_style;
        #[cfg(feature = "auto-color")]
        let color_choice = if color_choice == WriteStyle::Auto {
            match &self.target {
                Target::Stdout => anstream::AutoStream::choice(&io::stdout()).into(),
                Target::Stderr => anstream::AutoStream::choice(&io::stderr()).into(),
                Target::Pipe(_) => color_choice,
            }
        } else {
            color_choice
        };
        let color_choice = if color_choice == WriteStyle::Auto {
            WriteStyle::Never
        } else {
            color_choice
        };

        let writer = match mem::take(&mut self.target) {
            Target::Stdout => BufferWriter::stdout(self.is_test, color_choice),
            Target::Stderr => BufferWriter::stderr(self.is_test, color_choice),
            Target::Pipe(pipe) => BufferWriter::pipe(Box::new(Mutex::new(pipe)), color_choice),
        };

        Writer { inner: writer }
    }
}

impl Default for Builder {
    fn default() -> Self {
        Builder::new()
    }
}

fn parse_write_style(spec: &str) -> WriteStyle {
    match spec {
        "auto" => WriteStyle::Auto,
        "always" => WriteStyle::Always,
        "never" => WriteStyle::Never,
        _ => Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_write_style_valid() {
        let inputs = vec![
            ("auto", WriteStyle::Auto),
            ("always", WriteStyle::Always),
            ("never", WriteStyle::Never),
        ];

        for (input, expected) in inputs {
            assert_eq!(expected, parse_write_style(input));
        }
    }

    #[test]
    fn parse_write_style_invalid() {
        let inputs = vec!["", "true", "false", "NEVER!!"];

        for input in inputs {
            assert_eq!(WriteStyle::Auto, parse_write_style(input));
        }
    }
}
