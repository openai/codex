use std::cmp;
use std::error;
use std::fmt;
use std::result;

use crate::ast;
use crate::hir;

/// A type alias for dealing with errors returned by this crate.
pub type Result<T> = result::Result<T, Error>;

/// This error type encompasses any error that can be returned by this crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// An error that occurred while translating concrete syntax into abstract
    /// syntax (AST).
    Parse(ast::Error),
    /// An error that occurred while translating abstract syntax into a high
    /// level intermediate representation (HIR).
    Translate(hir::Error),
    /// Hints that destructuring should not be exhaustive.
    ///
    /// This enum may grow additional variants, so this makes sure clients
    /// don't count on exhaustive matching. (Otherwise, adding a new variant
    /// could break existing code.)
    #[doc(hidden)]
    __Nonexhaustive,
}

impl From<ast::Error> for Error {
    fn from(err: ast::Error) -> Error {
        Error::Parse(err)
    }
}

impl From<hir::Error> for Error {
    fn from(err: hir::Error) -> Error {
        Error::Translate(err)
    }
}

impl error::Error for Error {
    // TODO: Remove this method entirely on the next breaking semver release.
    #[allow(deprecated)]
    fn description(&self) -> &str {
        match *self {
            Error::Parse(ref x) => x.description(),
            Error::Translate(ref x) => x.description(),
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::Parse(ref x) => x.fmt(f),
            Error::Translate(ref x) => x.fmt(f),
            _ => unreachable!(),
        }
    }
}

/// A helper type for formatting nice error messages.
///
/// This type is responsible for reporting regex parse errors in a nice human
/// readable format. Most of its complexity is from interspersing notational
/// markers pointing out the position where an error occurred.
#[derive(Debug)]
pub struct Formatter<'e, E> {
    /// The original regex pattern in which the error occurred.
    pattern: &'e str,
    /// The error kind. It must impl fmt::Display.
    err: &'e E,
    /// The primary span of the error.
    span: &'e ast::Span,
    /// An auxiliary and optional span, in case the error needs to point to
    /// two locations (e.g., when reporting a duplicate capture group name).
    aux_span: Option<&'e ast::Span>,
}

impl<'e> From<&'e ast::Error> for Formatter<'e, ast::ErrorKind> {
    fn from(err: &'e ast::Error) -> Self {
        Formatter {
            pattern: err.pattern(),
            err: err.kind(),
            span: err.span(),
            aux_span: err.auxiliary_span(),
        }
    }
}

impl<'e> From<&'e hir::Error> for Formatter<'e, hir::ErrorKind> {
    fn from(err: &'e hir::Error) -> Self {
        Formatter {
            pattern: err.pattern(),
            err: err.kind(),
            span: err.span(),
            aux_span: None,
        }
    }
}

impl<'e, E: fmt::Display> fmt::Display for Formatter<'e, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let spans = Spans::from_formatter(self);
        if self.pattern.contains('\n') {
            let divider = repeat_char('~', 79);

            writeln!(f, "regex parse error:")?;
            writeln!(f, "{}", divider)?;
            let notated = spans.notate();
            write!(f, "{}", notated)?;
            writeln!(f, "{}", divider)?;
            // If we have error spans that cover multiple lines, then we just
            // note the line numbers.
            if !spans.multi_line.is_empty() {
                let mut notes = vec![];
                for span in &spans.multi_line {
                    notes.push(format!(
                        "on line {} (column {}) through line {} (column {})",
                        span.start.line,
                        span.start.column,
                        span.end.line,
                        span.end.column - 1
                    ));
                }
                writeln!(f, "{}", notes.join("\n"))?;
            }
            write!(f, "error: {}", self.err)?;
        } else {
            writeln!(f, "regex parse error:")?;
            let notated = Spans::from_formatter(self).notate();
            write!(f, "{}", notated)?;
            write!(f, "error: {}", self.err)?;
        }
        Ok(())
    }
}

/// This type represents an arbitrary number of error spans in a way that makes
/// it convenient to notate the regex pattern. ("Notate" means "point out
/// exactly where the error occurred in the regex pattern.")
///
/// Technically, we can only ever have two spans given our current error
/// structure. However, after toiling with a specific algorithm for handling
/// two spans, it became obvious that an algorithm to handle an arbitrary
/// number of spans was actually much simpler.
struct Spans<'p> {
    /// The original regex pattern string.
    pattern: &'p str,
    /// The total width that should be used for line numbers. The width is
    /// used for left padding the line numbers for alignment.
    ///
    /// A value of `0` means line numbers should not be displayed. That is,
    /// the pattern is itself only one line.
    line_number_width: usize,
    /// All error spans that occur on a single line. This sequence always has
    /// length equivalent to the number of lines in `pattern`, where the index
    /// of the sequence represents a line number, starting at `0`. The spans
    /// in each line are sorted in ascending order.
    by_line: Vec<Vec<ast::Span>>,
    /// All error spans that occur over one or more lines. That is, the start
    /// and end position of the span have different line numbers. The spans are
    /// sorted in ascending order.
    multi_line: Vec<ast::Span>,
}

impl<'p> Spans<'p> {
    /// Build a sequence of spans from a formatter.
    fn from_formatter<'e, E: fmt::Display>(
        fmter: &'p Formatter<'e, E>,
    ) -> Spans<'p> {
        let mut line_count = fmter.pattern.lines().count();
        // If the pattern ends with a `\n` literal, then our line count is
        // off by one, since a span can occur immediately after the last `\n`,
        // which is consider to be an additional line.
        if fmter.pattern.ends_with('\n') {
            line_count += 1;
        }
        let line_number_width =
            if line_count <= 1 { 0 } else { line_count.to_string().len() };
        let mut spans = Spans {
            pattern: &fmter.pattern,
            line_number_width,
            by_line: vec![vec![]; line_count],
            multi_line: vec![],
        };
        spans.add(fmter.span.clone());
        if let Some(span) = fmter.aux_span {
            spans.add(span.clone());
        }
        spans
    }

    /// Add the given span to this sequence, putting it in the right place.
    fn add(&mut self, span: ast::Span) {
        // This is grossly inefficient since we sort after each add, but right
        // now, we only ever add two spans at most.
        if span.is_one_line() {
            let i = span.start.line - 1; // because lines are 1-indexed
            self.by_line[i].push(span);
            self.by_line[i].sort();
        } else {
            self.multi_line.push(span);
            self.multi_line.sort();
        }
    }

    /// Notate the pattern string with carents (`^`) pointing at each span
    /// location. This only applies to spans that occur within a single line.
    fn notate(&self) -> String {
        let mut notated = String::new();
        for (i, line) in self.pattern.lines().enumerate() {
            if self.line_number_width > 0 {
                notated.push_str(&self.left_pad_line_number(i + 1));
                notated.push_str(": ");
            } else {
                notated.push_str("    ");
            }
            notated.push_str(line);
            notated.push('\n');
            if let Some(notes) = self.notate_line(i) {
                notated.push_str(&notes);
                notated.push('\n');
            }
        }
        notated
    }

    /// Return notes for the line indexed at `i` (zero-based). If there are no
    /// spans for the given line, then `None` is returned. Otherwise, an
    /// appropriately space padded string with correctly positioned `^` is
    /// returned, accounting for line numbers.
    fn notate_line(&self, i: usize) -> Option<String> {
        let spans = &self.by_line[i];
        if spans.is_empty() {
            return None;
        }
        let mut notes = String::new();
        for _ in 0..self.line_number_padding() {
            notes.push(' ');
        }
        let mut pos = 0;
        for span in spans {
            for _ in pos..(span.start.column - 1) {
                notes.push(' ');
                pos += 1;
            }
            let note_len = span.end.column.saturating_sub(span.start.column);
            for _ in 0..cmp::max(1, note_len) {
                notes.push('^');
                pos += 1;
            }
        }
        Some(notes)
    }

    /// Left pad the given line number with spaces such that it is aligned with
    /// other line numbers.
    fn left_pad_line_number(&self, n: usize) -> String {
        let n = n.to_string();
        let pad = self.line_number_width.checked_sub(n.len()).unwrap();
        let mut result = repeat_char(' ', pad);
        result.push_str(&n);
        result
    }

    /// Return the line number padding beginning at the start of each line of
    /// the pattern.
    ///
    /// If the pattern is only one line, then this returns a fixed padding
    /// for visual indentation.
    fn line_number_padding(&self) -> usize {
        if self.line_number_width == 0 {
            4
        } else {
            2 + self.line_number_width
        }
    }
}

fn repeat_char(c: char, count: usize) -> String {
    ::std::iter::repeat(c).take(count).collect()
}

#[cfg(test)]
mod tests {
    use crate::ast::parse::Parser;

    fn assert_panic_message(pattern: &str, expected_msg: &str) {
        let result = Parser::new().parse(pattern);
        match result {
            Ok(_) => {
                panic!("regex should not have parsed");
            }
            Err(err) => {
                assert_eq!(err.to_string(), expected_msg.trim());
            }
        }
    }

    // See: https://github.com/rust-lang/regex/issues/464
    #[test]
    fn regression_464() {
        let err = Parser::new().parse("a{\n").unwrap_err();
        // This test checks that the error formatter doesn't panic.
        assert!(!err.to_string().is_empty());
    }

    // See: https://github.com/rust-lang/regex/issues/545
    #[test]
    fn repetition_quantifier_expects_a_valid_decimal() {
        assert_panic_message(
            r"\\u{[^}]*}",
            r#"
regex parse error:
    \\u{[^}]*}
        ^
error: repetition quantifier expects a valid decimal
"#,
        );
    }
}
