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
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            sanitized.push('\n');
        }
        for span in codex_ansi_escape::ansi_escape(line)
            .lines
            .into_iter()
            .flat_map(|line| line.spans)
        {
            sanitized.extend(
                span.content
                    .chars()
                    .filter(|ch| *ch == '\t' || !ch.is_control()),
            );
        }
    }
    Cow::Owned(sanitized)
}
