use alloc::borrow::Cow;
use core::fmt;

use crate::text::Text;

/// A wrapper around a string that is masked when displayed.
///
/// The masked string is displayed as a series of the same character. This might be used to display
/// a password field or similar secure data.
///
/// # Examples
///
/// ```rust
/// use ratatui_core::buffer::Buffer;
/// use ratatui_core::layout::Rect;
/// use ratatui_core::text::{Masked, Text};
/// use ratatui_core::widgets::Widget;
///
/// let mut buffer = Buffer::empty(Rect::new(0, 0, 5, 1));
/// let password = Masked::new("12345", 'x');
///
/// Text::from(password).render(buffer.area, &mut buffer);
/// assert_eq!(buffer, Buffer::with_lines(["xxxxx"]));
/// ```
#[derive(Default, Clone, Eq, PartialEq, Hash)]
pub struct Masked<'a> {
    inner: Cow<'a, str>,
    mask_char: char,
}

impl<'a> Masked<'a> {
    pub fn new(s: impl Into<Cow<'a, str>>, mask_char: char) -> Self {
        Self {
            inner: s.into(),
            mask_char,
        }
    }

    /// The character to use for masking.
    pub const fn mask_char(&self) -> char {
        self.mask_char
    }

    /// The underlying string, with all characters masked.
    pub fn value(&self) -> Cow<'a, str> {
        self.inner.chars().map(|_| self.mask_char).collect()
    }
}

impl fmt::Debug for Masked<'_> {
    /// Debug representation of a masked string is the underlying string
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // note that calling display instead of Debug here is intentional
        fmt::Display::fmt(&self.inner, f)
    }
}

impl fmt::Display for Masked<'_> {
    /// Display representation of a masked string is the masked string
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.value(), f)
    }
}

impl<'a> From<&'a Masked<'a>> for Cow<'a, str> {
    fn from(masked: &'a Masked) -> Self {
        masked.value()
    }
}

impl<'a> From<Masked<'a>> for Cow<'a, str> {
    fn from(masked: Masked<'a>) -> Self {
        masked.value()
    }
}

impl<'a> From<&'a Masked<'_>> for Text<'a> {
    fn from(masked: &'a Masked) -> Self {
        Text::raw(masked.value())
    }
}

impl<'a> From<Masked<'a>> for Text<'a> {
    fn from(masked: Masked<'a>) -> Self {
        Text::raw(masked.value())
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;

    use super::*;
    use crate::text::Line;

    #[test]
    fn new() {
        let masked = Masked::new("12345", 'x');
        assert_eq!(masked.inner, "12345");
        assert_eq!(masked.mask_char, 'x');
    }

    #[test]
    fn value() {
        let masked = Masked::new("12345", 'x');
        assert_eq!(masked.value(), "xxxxx");
    }

    #[test]
    fn mask_char() {
        let masked = Masked::new("12345", 'x');
        assert_eq!(masked.mask_char(), 'x');
    }

    #[test]
    fn debug() {
        let masked = Masked::new("12345", 'x');
        assert_eq!(format!("{masked:?}"), "12345");
        assert_eq!(format!("{masked:.3?}"), "123", "Debug truncates");
    }

    #[test]
    fn display() {
        let masked = Masked::new("12345", 'x');
        assert_eq!(format!("{masked}"), "xxxxx");
        assert_eq!(format!("{masked:.3}"), "xxx", "Display truncates");
    }

    #[test]
    fn into_text() {
        let masked = Masked::new("12345", 'x');

        let text: Text = (&masked).into();
        assert_eq!(text.lines, [Line::from("xxxxx")]);

        let text: Text = masked.into();
        assert_eq!(text.lines, [Line::from("xxxxx")]);
    }

    #[test]
    fn into_cow() {
        let masked = Masked::new("12345", 'x');
        let cow: Cow<str> = (&masked).into();
        assert_eq!(cow, "xxxxx");

        let cow: Cow<str> = masked.into();
        assert_eq!(cow, "xxxxx");
    }
}
