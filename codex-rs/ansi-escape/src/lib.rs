use ansi_to_tui::Error;
use ansi_to_tui::IntoText;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::text::Text;

// Expand tabs in a best-effort way for transcript rendering.
// Tabs can interact poorly with left-gutter prefixes in our TUI and CLI
// transcript views (e.g., `nl` separates line numbers from content with a tab).
// Replacing tabs with spaces avoids odd visual artifacts without changing
// semantics for our use cases.
fn expand_tabs(s: &str) -> std::borrow::Cow<'_, str> {
    if s.contains('\t') {
        // Keep it simple: replace each tab with 4 spaces.
        // We do not try to align to tab stops since most usages (like `nl`)
        // look acceptable with a fixed substitution and this avoids stateful math
        // across spans.
        std::borrow::Cow::Owned(s.replace('\t', "    "))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

/// This function should be used when the contents of `s` are expected to match
/// a single line. If multiple lines are found, a warning is logged and only the
/// first line is returned.
pub fn ansi_escape_line(s: &str) -> Line<'static> {
    // Normalize tabs to spaces to avoid odd gutter collisions in transcript mode.
    let s = expand_tabs(s);
    let text = ansi_escape(&s);
    match text.lines.as_slice() {
        [] => "".into(),
        [only] => only.clone(),
        [first, rest @ ..] => {
            tracing::warn!("ansi_escape_line: expected a single line, got {first:?} and {rest:?}");
            first.clone()
        }
    }
}

pub fn ansi_escape(s: &str) -> Text<'static> {
    // to_text() claims to be faster, but introduces complex lifetime issues
    // such that it's not worth it.
    match s.into_text() {
        Ok(mut text) => {
            normalize_ansi_text_for_tui(&mut text);
            text
        }
        Err(err) => match err {
            Error::NomError(message) => {
                tracing::error!(
                    "ansi_to_tui NomError docs claim should never happen when parsing `{s}`: {message}"
                );
                panic!();
            }
            Error::Utf8Error(utf8error) => {
                tracing::error!("Utf8Error: {utf8error}");
                panic!();
            }
        },
    }
}

fn normalize_ansi_text_for_tui(text: &mut Text<'static>) {
    for line in &mut text.lines {
        for span in &mut line.spans {
            match span.style.fg {
                Some(Color::Reset) => span.style.fg = None,
                Some(Color::Black) if span.style.bg.is_none() => {
                    if span.style.add_modifier.contains(Modifier::BOLD) {
                        span.style.fg = Some(Color::DarkGray);
                    } else {
                        span.style.fg = None;
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_escape_preserves_default_color_for_plain_text() {
        let line = ansi_escape_line("hello");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].style.fg, None);
    }

    #[test]
    fn ansi_escape_rewrites_bold_black_to_dark_gray() {
        let line = ansi_escape_line("\u{1b}[1;30mhello\u{1b}[0m");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "hello");
        assert_eq!(line.spans[0].style.fg, Some(Color::DarkGray));
    }
}
