//! Sanitizes untrusted text before it is persisted or written to a terminal.

use std::borrow::Cow;

/// Remove terminal escape sequences and non-whitespace control characters.
///
/// Newlines and tabs remain intact so callers can safely use this for pasted source text. Terminal
/// rendering expands tabs separately and represents newlines as distinct lines.
pub(crate) fn sanitize_untrusted_text(text: &str) -> Cow<'_, str> {
    let needs_sanitizing = text.contains('\x1b')
        || text
            .chars()
            .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\t'));
    if !needs_sanitizing {
        return Cow::Borrowed(text);
    }

    let mut sanitized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\x1b' => match chars.next() {
                Some('[') => {
                    let _ = chars.find(|ch| ('@'..='~').contains(ch));
                }
                Some(introducer @ (']' | 'P' | 'X' | '^' | '_')) => {
                    while let Some(ch) = chars.next() {
                        if ch == '\u{9c}'
                            || introducer == ']' && ch == '\x07'
                            || ch == '\x1b' && chars.next_if_eq(&'\\').is_some()
                        {
                            break;
                        }
                    }
                }
                Some(' '..='/') => {
                    let _ = chars.find(|ch| ('0'..='~').contains(ch));
                }
                Some(_) | None => {}
            },
            '\n' | '\t' => sanitized.push(ch),
            _ if !ch.is_control() => sanitized.push(ch),
            _ => {}
        }
    }
    Cow::Owned(sanitized)
}
