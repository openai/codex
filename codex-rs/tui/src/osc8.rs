#[cfg(test)]
use crate::terminal_wrappers::parse_zero_width_terminal_wrapper;
#[cfg(test)]
use crate::terminal_wrappers::strip_zero_width_terminal_wrappers;

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParsedOsc8<'a> {
    pub(crate) destination: &'a str,
    pub(crate) text: &'a str,
}

const OSC8_OPEN_PREFIX: &str = "\u{1b}]8;;";
#[cfg(test)]
const OSC8_PREFIX: &str = "\u{1b}]8;";
const OSC8_CLOSE: &str = "\u{1b}]8;;\u{7}";

pub(crate) fn sanitize_osc8_url(destination: &str) -> String {
    destination
        .chars()
        .filter(|&c| c != '\x1B' && c != '\x07')
        .collect()
}

pub(crate) fn osc8_hyperlink<S: AsRef<str>>(destination: &str, text: S) -> String {
    let safe_destination = sanitize_osc8_url(destination);
    if safe_destination.is_empty() {
        return text.as_ref().to_string();
    }

    format!(
        "{OSC8_OPEN_PREFIX}{safe_destination}\u{7}{}{OSC8_CLOSE}",
        text.as_ref()
    )
}

#[cfg(test)]
pub(crate) fn parse_osc8_hyperlink(text: &str) -> Option<ParsedOsc8<'_>> {
    let wrapped = parse_zero_width_terminal_wrapper(text)?;
    let opener_payload = wrapped.prefix.strip_prefix(OSC8_PREFIX)?;
    let params_end = opener_payload.find(';')?;
    let after_params = &opener_payload[params_end + 1..];
    let destination = after_params
        .strip_suffix('\x07')
        .or_else(|| after_params.strip_suffix("\x1b\\"))?;
    Some(ParsedOsc8 {
        destination,
        text: wrapped.text,
    })
}

#[cfg(test)]
pub(crate) fn strip_osc8_hyperlinks(text: &str) -> String {
    strip_zero_width_terminal_wrappers(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_wrapped_text() {
        let wrapped = osc8_hyperlink("https://example.com", "docs");
        let parsed = parse_osc8_hyperlink(&wrapped).expect("expected osc8 span");
        assert_eq!(parsed.destination, "https://example.com");
        assert_eq!(parsed.text, "docs");
    }

    #[test]
    fn strips_wrapped_text() {
        let wrapped = format!("See {}", osc8_hyperlink("https://example.com", "docs"));
        assert_eq!(strip_osc8_hyperlinks(&wrapped), "See docs");
    }

    #[test]
    fn parses_st_terminated_wrapped_text_with_params() {
        let wrapped = "\u{1b}]8;id=abc;https://example.com\u{1b}\\docs\u{1b}]8;;\u{1b}\\";

        let parsed = parse_osc8_hyperlink(wrapped).expect("expected osc8 span");
        assert_eq!(parsed.destination, "https://example.com");
        assert_eq!(parsed.text, "docs");
    }
}
