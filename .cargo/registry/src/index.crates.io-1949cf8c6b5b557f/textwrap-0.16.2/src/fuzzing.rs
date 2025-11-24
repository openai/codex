//! Fuzzing helpers.

use super::Options;
use std::borrow::Cow;

/// Exposed for fuzzing so we can check the slow path is correct.
pub fn fill_slow_path<'a>(text: &str, options: Options<'_>) -> String {
    crate::fill::fill_slow_path(text, options)
}

/// Exposed for fuzzing so we can check the slow path is correct.
pub fn wrap_single_line<'a>(line: &'a str, options: &Options<'_>, lines: &mut Vec<Cow<'a, str>>) {
    crate::wrap::wrap_single_line(line, options, lines);
}

/// Exposed for fuzzing so we can check the slow path is correct.
pub fn wrap_single_line_slow_path<'a>(
    line: &'a str,
    options: &Options<'_>,
    lines: &mut Vec<Cow<'a, str>>,
) {
    crate::wrap::wrap_single_line_slow_path(line, options, lines)
}
