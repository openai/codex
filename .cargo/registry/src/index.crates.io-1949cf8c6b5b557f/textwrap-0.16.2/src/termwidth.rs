//! Functions related to the terminal size.

use crate::Options;

/// Return the current terminal width.
///
/// If the terminal width cannot be determined (typically because the
/// standard output is not connected to a terminal), a default width
/// of 80 characters will be used.
///
/// # Examples
///
/// Create an [`Options`] for wrapping at the current terminal width
/// with a two column margin to the left and the right:
///
/// ```no_run
/// use textwrap::{termwidth, Options};
///
/// let width = termwidth() - 4; // Two columns on each side.
/// let options = Options::new(width)
///     .initial_indent("  ")
///     .subsequent_indent("  ");
/// ```
///
/// **Note:** Only available when the `terminal_size` Cargo feature is
/// enabled.
pub fn termwidth() -> usize {
    terminal_size::terminal_size().map_or(80, |(terminal_size::Width(w), _)| w.into())
}

impl<'a> Options<'a> {
    /// Creates a new [`Options`] with `width` set to the current
    /// terminal width. If the terminal width cannot be determined
    /// (typically because the standard input and output is not
    /// connected to a terminal), a width of 80 characters will be
    /// used. Other settings use the same defaults as
    /// [`Options::new`].
    ///
    /// Equivalent to:
    ///
    /// ```no_run
    /// use textwrap::{termwidth, Options};
    ///
    /// let options = Options::new(termwidth());
    /// ```
    ///
    /// **Note:** Only available when the `terminal_size` feature is
    /// enabled.
    pub fn with_termwidth() -> Self {
        Self::new(termwidth())
    }
}
