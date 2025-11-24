//! Fork of `textwrap` crate
//!
//! Benefits of forking:
//! - Pull in only what we need rather than relying on the compiler to remove what we don't need
//! - `LineWrapper` is able to incrementally wrap which will help with `StyledStr`

pub(crate) mod core;
#[cfg(feature = "wrap_help")]
pub(crate) mod word_separators;
#[cfg(feature = "wrap_help")]
pub(crate) mod wrap_algorithms;

#[cfg(feature = "wrap_help")]
pub(crate) fn wrap(content: &str, hard_width: usize) -> String {
    let mut wrapper = wrap_algorithms::LineWrapper::new(hard_width);
    let mut total = Vec::new();
    for line in content.split_inclusive('\n') {
        wrapper.reset();
        let line = word_separators::find_words_ascii_space(line).collect::<Vec<_>>();
        total.extend(wrapper.wrap(line));
    }
    total.join("")
}

#[cfg(not(feature = "wrap_help"))]
pub(crate) fn wrap(content: &str, _hard_width: usize) -> String {
    content.to_owned()
}

#[cfg(test)]
#[cfg(feature = "wrap_help")]
mod test {
    /// Compatibility shim to keep textwrap's tests
    fn wrap(content: &str, hard_width: usize) -> Vec<String> {
        super::wrap(content, hard_width)
            .trim_end()
            .split('\n')
            .map(|s| s.to_owned())
            .collect::<Vec<_>>()
    }

    #[test]
    fn no_wrap() {
        assert_eq!(wrap("foo", 10), vec!["foo"]);
    }

    #[test]
    fn wrap_simple() {
        assert_eq!(wrap("foo bar baz", 5), vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn to_be_or_not() {
        assert_eq!(
            wrap("To be, or not to be, that is the question.", 10),
            vec!["To be, or", "not to be,", "that is", "the", "question."]
        );
    }

    #[test]
    fn multiple_words_on_first_line() {
        assert_eq!(wrap("foo bar baz", 10), vec!["foo bar", "baz"]);
    }

    #[test]
    fn long_word() {
        assert_eq!(wrap("foo", 0), vec!["foo"]);
    }

    #[test]
    fn long_words() {
        assert_eq!(wrap("foo bar", 0), vec!["foo", "bar"]);
    }

    #[test]
    fn max_width() {
        assert_eq!(wrap("foo bar", usize::MAX), vec!["foo bar"]);

        let text = "Hello there! This is some English text. \
                    It should not be wrapped given the extents below.";
        assert_eq!(wrap(text, usize::MAX), vec![text]);
    }

    #[test]
    fn leading_whitespace() {
        assert_eq!(wrap("  foo bar", 6), vec!["  foo", "  bar"]);
    }

    #[test]
    fn leading_whitespace_empty_first_line() {
        // If there is no space for the first word, the first line
        // will be empty. This is because the string is split into
        // words like [" ", "foobar ", "baz"], which puts "foobar " on
        // the second line. We never output trailing whitespace
        assert_eq!(wrap(" foobar baz", 6), vec!["", " foobar", " baz"]);
    }

    #[test]
    fn trailing_whitespace() {
        // Whitespace is only significant inside a line. After a line
        // gets too long and is broken, the first word starts in
        // column zero and is not indented.
        assert_eq!(wrap("foo     bar     baz  ", 5), vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn issue_99() {
        // We did not reset the in_whitespace flag correctly and did
        // not handle single-character words after a line break.
        assert_eq!(
            wrap("aaabbbccc x yyyzzzwww", 9),
            vec!["aaabbbccc", "x", "yyyzzzwww"]
        );
    }

    #[test]
    fn issue_129() {
        // The dash is an em-dash which takes up four bytes. We used
        // to panic since we tried to index into the character.
        assert_eq!(wrap("x – x", 1), vec!["x", "–", "x"]);
    }
}
