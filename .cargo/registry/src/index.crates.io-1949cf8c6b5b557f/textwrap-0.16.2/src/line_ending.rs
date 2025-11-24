//! Line ending detection and conversion.

use std::fmt::Debug;

/// Supported line endings. Like in the Rust standard library, two line
/// endings are supported: `\r\n` and `\n`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    /// _Carriage return and line feed_ – a line ending sequence
    /// historically used in Windows. Corresponds to the sequence
    /// of ASCII control characters `0x0D 0x0A` or `\r\n`
    CRLF,
    /// _Line feed_ – a line ending historically used in Unix.
    ///  Corresponds to the ASCII control character `0x0A` or `\n`
    LF,
}

impl LineEnding {
    /// Turns this [`LineEnding`] value into its ASCII representation.
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CRLF => "\r\n",
            Self::LF => "\n",
        }
    }
}

/// An iterator over the lines of a string, as tuples of string slice
/// and [`LineEnding`] value; it only emits non-empty lines (i.e. having
/// some content before the terminating `\r\n` or `\n`).
///
/// This struct is used internally by the library.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NonEmptyLines<'a>(pub &'a str);

impl<'a> Iterator for NonEmptyLines<'a> {
    type Item = (&'a str, Option<LineEnding>);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(lf) = self.0.find('\n') {
            if lf == 0 || (lf == 1 && self.0.as_bytes()[lf - 1] == b'\r') {
                self.0 = &self.0[(lf + 1)..];
                continue;
            }
            let trimmed = match self.0.as_bytes()[lf - 1] {
                b'\r' => (&self.0[..(lf - 1)], Some(LineEnding::CRLF)),
                _ => (&self.0[..lf], Some(LineEnding::LF)),
            };
            self.0 = &self.0[(lf + 1)..];
            return Some(trimmed);
        }
        if self.0.is_empty() {
            None
        } else {
            let line = std::mem::take(&mut self.0);
            Some((line, None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_lines_full_case() {
        assert_eq!(
            NonEmptyLines("LF\nCRLF\r\n\r\n\nunterminated")
                .collect::<Vec<(&str, Option<LineEnding>)>>(),
            vec![
                ("LF", Some(LineEnding::LF)),
                ("CRLF", Some(LineEnding::CRLF)),
                ("unterminated", None),
            ]
        );
    }

    #[test]
    fn non_empty_lines_new_lines_only() {
        assert_eq!(NonEmptyLines("\r\n\n\n\r\n").next(), None);
    }

    #[test]
    fn non_empty_lines_no_input() {
        assert_eq!(NonEmptyLines("").next(), None);
    }
}
