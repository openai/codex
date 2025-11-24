//! Functions related to adding and removing indentation from lines of
//! text.
//!
//! The functions here can be used to uniformly indent or dedent
//! (unindent) word wrapped lines of text.

/// Add prefix to each non-empty line.
///
/// ```
/// use textwrap::indent;
///
/// assert_eq!(indent("
/// Foo
/// Bar
/// ", "  "), "
///   Foo
///   Bar
/// ");
/// ```
///
/// Empty lines (lines consisting only of whitespace) are not indented
/// and the whitespace is replaced by a single newline (`\n`):
///
/// ```
/// use textwrap::indent;
///
/// assert_eq!(indent("
/// Foo
///
/// Bar
///   \t
/// Baz
/// ", "->"), "
/// ->Foo
///
/// ->Bar
///
/// ->Baz
/// ");
/// ```
///
/// Leading and trailing whitespace on non-empty lines is kept
/// unchanged:
///
/// ```
/// use textwrap::indent;
///
/// assert_eq!(indent(" \t  Foo   ", "->"), "-> \t  Foo   \n");
/// ```
pub fn indent(s: &str, prefix: &str) -> String {
    let mut result = String::new();
    for line in s.lines() {
        if line.chars().any(|c| !c.is_whitespace()) {
            result.push_str(prefix);
            result.push_str(line);
        }
        result.push('\n');
    }
    result
}

/// Removes common leading whitespace from each line.
///
/// This function will look at each non-empty line and determine the
/// maximum amount of whitespace that can be removed from all lines:
///
/// ```
/// use textwrap::dedent;
///
/// assert_eq!(dedent("
///     1st line
///       2nd line
///     3rd line
/// "), "
/// 1st line
///   2nd line
/// 3rd line
/// ");
/// ```
pub fn dedent(s: &str) -> String {
    let mut prefix = "";
    let mut lines = s.lines();

    // We first search for a non-empty line to find a prefix.
    for line in &mut lines {
        let mut whitespace_idx = line.len();
        for (idx, ch) in line.char_indices() {
            if !ch.is_whitespace() {
                whitespace_idx = idx;
                break;
            }
        }

        // Check if the line had anything but whitespace
        if whitespace_idx < line.len() {
            prefix = &line[..whitespace_idx];
            break;
        }
    }

    // We then continue looking through the remaining lines to
    // possibly shorten the prefix.
    for line in &mut lines {
        let mut whitespace_idx = line.len();
        for ((idx, a), b) in line.char_indices().zip(prefix.chars()) {
            if a != b {
                whitespace_idx = idx;
                break;
            }
        }

        // Check if the line had anything but whitespace and if we
        // have found a shorter prefix
        if whitespace_idx < line.len() && whitespace_idx < prefix.len() {
            prefix = &line[..whitespace_idx];
        }
    }

    // We now go over the lines a second time to build the result.
    let mut result = String::new();
    for line in s.lines() {
        if line.starts_with(&prefix) && line.chars().any(|c| !c.is_whitespace()) {
            let (_, tail) = line.split_at(prefix.len());
            result.push_str(tail);
        }
        result.push('\n');
    }

    if result.ends_with('\n') && !s.ends_with('\n') {
        let new_len = result.len() - 1;
        result.truncate(new_len);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Add newlines. Ensures that the final line in the vector also
    /// has a newline.
    fn add_nl(lines: &[&str]) -> String {
        lines.join("\n") + "\n"
    }

    #[test]
    fn indent_empty() {
        assert_eq!(indent("\n", "  "), "\n");
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn indent_nonempty() {
        let x = vec!["  foo",
                     "bar",
                     "  baz"];
        let y = vec!["//  foo",
                     "//bar",
                     "//  baz"];
        assert_eq!(indent(&add_nl(&x), "//"), add_nl(&y));
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn indent_empty_line() {
        let x = vec!["  foo",
                     "bar",
                     "",
                     "  baz"];
        let y = vec!["//  foo",
                     "//bar",
                     "",
                     "//  baz"];
        assert_eq!(indent(&add_nl(&x), "//"), add_nl(&y));
    }

    #[test]
    fn dedent_empty() {
        assert_eq!(dedent(""), "");
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn dedent_multi_line() {
        let x = vec!["    foo",
                     "  bar",
                     "    baz"];
        let y = vec!["  foo",
                     "bar",
                     "  baz"];
        assert_eq!(dedent(&add_nl(&x)), add_nl(&y));
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn dedent_empty_line() {
        let x = vec!["    foo",
                     "  bar",
                     "   ",
                     "    baz"];
        let y = vec!["  foo",
                     "bar",
                     "",
                     "  baz"];
        assert_eq!(dedent(&add_nl(&x)), add_nl(&y));
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn dedent_blank_line() {
        let x = vec!["      foo",
                     "",
                     "        bar",
                     "          foo",
                     "          bar",
                     "          baz"];
        let y = vec!["foo",
                     "",
                     "  bar",
                     "    foo",
                     "    bar",
                     "    baz"];
        assert_eq!(dedent(&add_nl(&x)), add_nl(&y));
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn dedent_whitespace_line() {
        let x = vec!["      foo",
                     " ",
                     "        bar",
                     "          foo",
                     "          bar",
                     "          baz"];
        let y = vec!["foo",
                     "",
                     "  bar",
                     "    foo",
                     "    bar",
                     "    baz"];
        assert_eq!(dedent(&add_nl(&x)), add_nl(&y));
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn dedent_mixed_whitespace() {
        let x = vec!["\tfoo",
                     "  bar"];
        let y = vec!["\tfoo",
                     "  bar"];
        assert_eq!(dedent(&add_nl(&x)), add_nl(&y));
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn dedent_tabbed_whitespace() {
        let x = vec!["\t\tfoo",
                     "\t\t\tbar"];
        let y = vec!["foo",
                     "\tbar"];
        assert_eq!(dedent(&add_nl(&x)), add_nl(&y));
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn dedent_mixed_tabbed_whitespace() {
        let x = vec!["\t  \tfoo",
                     "\t  \t\tbar"];
        let y = vec!["foo",
                     "\tbar"];
        assert_eq!(dedent(&add_nl(&x)), add_nl(&y));
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn dedent_mixed_tabbed_whitespace2() {
        let x = vec!["\t  \tfoo",
                     "\t    \tbar"];
        let y = vec!["\tfoo",
                     "  \tbar"];
        assert_eq!(dedent(&add_nl(&x)), add_nl(&y));
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn dedent_preserve_no_terminating_newline() {
        let x = vec!["  foo",
                     "    bar"].join("\n");
        let y = vec!["foo",
                     "  bar"].join("\n");
        assert_eq!(dedent(&x), y);
    }
}
