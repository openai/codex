use unicode_width::UnicodeWidthStr;

/// A balanced zero-width terminal wrapper around visible text.
///
/// This is deliberately narrower than "arbitrary ANSI". It models the shape we
/// need for OSC-8 hyperlinks in layout code: an opener with no display width,
/// visible text that should be measured/wrapped/truncated, and a closer with no
/// display width. Keeping the wrapper bytes separate from the visible text lets
/// us preserve them atomically when a line is split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParsedTerminalWrapper<'a> {
    pub(crate) prefix: &'a str,
    pub(crate) text: &'a str,
    pub(crate) suffix: &'a str,
}

const OSC8_PREFIX: &str = "\u{1b}]8;";
const OSC8_CLOSE_BEL: &str = "\u{1b}]8;;\u{7}";
const OSC8_CLOSE_ST: &str = "\u{1b}]8;;\u{1b}\\";
const OSC_STRING_TERMINATORS: [&str; 2] = ["\u{7}", "\u{1b}\\"];

/// Parse a full-span terminal wrapper.
///
/// Today this recognizes OSC-8 hyperlinks only, but it returns a generic
/// wrapper shape so width and slicing code do not need to know about
/// hyperlink-specific fields like URL or params.
pub(crate) fn parse_zero_width_terminal_wrapper(text: &str) -> Option<ParsedTerminalWrapper<'_>> {
    let after_prefix = text.strip_prefix(OSC8_PREFIX)?;
    let params_end = after_prefix.find(';')?;
    let after_params = &after_prefix[params_end + 1..];
    let (destination_end, opener_terminator) = find_osc_string_terminator(after_params)?;
    let prefix_len = OSC8_PREFIX.len() + params_end + 1 + destination_end + opener_terminator.len();
    let prefix = &text[..prefix_len];
    let after_opener = &text[prefix_len..];

    if let Some(visible) = after_opener.strip_suffix(OSC8_CLOSE_BEL) {
        return Some(ParsedTerminalWrapper {
            prefix,
            text: visible,
            suffix: OSC8_CLOSE_BEL,
        });
    }

    if let Some(visible) = after_opener.strip_suffix(OSC8_CLOSE_ST) {
        return Some(ParsedTerminalWrapper {
            prefix,
            text: visible,
            suffix: OSC8_CLOSE_ST,
        });
    }

    None
}

/// Strip the zero-width wrapper bytes from any recognized wrapped runs.
///
/// Malformed or unterminated escape sequences are preserved verbatim. That
/// keeps layout helpers fail-safe: they may over-measure malformed input, but
/// they will not silently delete bytes from it.
pub(crate) fn strip_zero_width_terminal_wrappers(text: &str) -> String {
    if !text.contains('\x1B') {
        return text.to_string();
    }

    let mut remaining = text;
    let mut rendered = String::with_capacity(text.len());

    while let Some(open_pos) = remaining.find(OSC8_PREFIX) {
        rendered.push_str(&remaining[..open_pos]);
        let candidate = &remaining[open_pos..];
        let Some((consumed, visible)) = consume_wrapped_prefix(candidate) else {
            rendered.push_str(candidate);
            return rendered;
        };
        rendered.push_str(visible);
        remaining = &candidate[consumed..];
    }

    rendered.push_str(remaining);
    rendered
}

/// Measure display width after removing recognized zero-width terminal wrappers.
pub(crate) fn visible_width(text: &str) -> usize {
    UnicodeWidthStr::width(strip_zero_width_terminal_wrappers(text).as_str())
}

fn consume_wrapped_prefix(text: &str) -> Option<(usize, &str)> {
    let after_prefix = text.strip_prefix(OSC8_PREFIX)?;
    let params_end = after_prefix.find(';')?;
    let after_params = &after_prefix[params_end + 1..];
    let (destination_end, opener_terminator) = find_osc_string_terminator(after_params)?;
    let opener_len = OSC8_PREFIX.len() + params_end + 1 + destination_end + opener_terminator.len();
    let after_opener = &text[opener_len..];

    let mut best: Option<(usize, &str)> = None;
    for suffix in [OSC8_CLOSE_BEL, OSC8_CLOSE_ST] {
        if let Some(close_pos) = after_opener.find(suffix)
            && best.is_none_or(|(best_pos, _)| close_pos < best_pos)
        {
            best = Some((close_pos, suffix));
        }
    }

    let (close_pos, suffix) = best?;
    Some((
        opener_len + close_pos + suffix.len(),
        &after_opener[..close_pos],
    ))
}

fn find_osc_string_terminator(text: &str) -> Option<(usize, &'static str)> {
    let mut best: Option<(usize, &'static str)> = None;
    for terminator in OSC_STRING_TERMINATORS {
        if let Some(pos) = text.find(terminator)
            && best.is_none_or(|(best_pos, _)| pos < best_pos)
        {
            best = Some((pos, terminator));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_bel_terminated_wrapper() {
        let wrapped = "\u{1b}]8;;https://example.com\u{7}docs\u{1b}]8;;\u{7}";

        assert_eq!(
            parse_zero_width_terminal_wrapper(wrapped),
            Some(ParsedTerminalWrapper {
                prefix: "\u{1b}]8;;https://example.com\u{7}",
                text: "docs",
                suffix: "\u{1b}]8;;\u{7}",
            })
        );
    }

    #[test]
    fn parses_st_terminated_wrapper_with_params() {
        let wrapped = "\u{1b}]8;id=abc;https://example.com\u{1b}\\docs\u{1b}]8;;\u{1b}\\";

        assert_eq!(
            parse_zero_width_terminal_wrapper(wrapped),
            Some(ParsedTerminalWrapper {
                prefix: "\u{1b}]8;id=abc;https://example.com\u{1b}\\",
                text: "docs",
                suffix: "\u{1b}]8;;\u{1b}\\",
            })
        );
    }

    #[test]
    fn strips_multiple_wrapped_runs_and_keeps_plain_text() {
        let text = concat!(
            "See ",
            "\u{1b}]8;;https://a.example\u{7}alpha\u{1b}]8;;\u{7}",
            " and ",
            "\u{1b}]8;id=1;https://b.example\u{1b}\\beta\u{1b}]8;;\u{1b}\\",
            "."
        );

        assert_eq!(
            strip_zero_width_terminal_wrappers(text),
            "See alpha and beta."
        );
    }

    #[test]
    fn preserves_malformed_unterminated_wrapper_verbatim() {
        let text = "See \u{1b}]8;;https://example.com\u{7}docs";

        assert_eq!(strip_zero_width_terminal_wrappers(text), text);
        assert_eq!(parse_zero_width_terminal_wrapper(text), None);
    }

    #[test]
    fn visible_width_ignores_wrapper_bytes() {
        let text = "\u{1b}]8;;https://example.com\u{7}docs\u{1b}]8;;\u{7}";

        assert_eq!(visible_width(text), 4);
    }
}
