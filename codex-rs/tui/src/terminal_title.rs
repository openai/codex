use std::fmt;
use std::io;
use std::io::IsTerminal;
use std::io::stdout;

use crossterm::Command;
use ratatui::crossterm::execute;

const MAX_TERMINAL_TITLE_CHARS: usize = 240;

pub(crate) fn set_terminal_title(title: &str) -> io::Result<()> {
    if !stdout().is_terminal() {
        return Ok(());
    }

    let title = sanitize_terminal_title(title);
    if title.is_empty() {
        return Ok(());
    }

    execute!(stdout(), SetWindowTitle(title))
}

#[derive(Debug, Clone)]
struct SetWindowTitle(String);

impl Command for SetWindowTitle {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        // xterm/ctlseqs documents OSC 0/2 title sequences with ST (ESC \) termination.
        // Most terminals also accept BEL for compatibility, but ST is the canonical form.
        write!(f, "\x1b]0;{}\x1b\\", self.0)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute SetWindowTitle using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

fn sanitize_terminal_title(title: &str) -> String {
    let mut sanitized = String::new();
    let mut chars_written = 0;
    let mut pending_space = false;

    for ch in title.chars() {
        if ch.is_whitespace() {
            pending_space = !sanitized.is_empty();
            continue;
        }

        if is_disallowed_terminal_title_char(ch) {
            continue;
        }

        if pending_space && chars_written < MAX_TERMINAL_TITLE_CHARS {
            sanitized.push(' ');
            chars_written += 1;
            pending_space = false;
        }

        if chars_written >= MAX_TERMINAL_TITLE_CHARS {
            break;
        }

        sanitized.push(ch);
        chars_written += 1;
    }

    sanitized
}

fn is_disallowed_terminal_title_char(ch: char) -> bool {
    if ch.is_control() {
        return true;
    }

    // Strip Trojan-Source-related bidi controls plus common non-rendering
    // formatting characters so title text cannot smuggle terminal control
    // semantics or visually misleading content.
    matches!(
        ch,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{E0100}'..='\u{E01EF}'
    )
}

#[cfg(test)]
mod tests {
    use super::MAX_TERMINAL_TITLE_CHARS;
    use super::SetWindowTitle;
    use super::sanitize_terminal_title;
    use crossterm::Command;
    use pretty_assertions::assert_eq;

    #[test]
    fn sanitizes_terminal_title() {
        let sanitized =
            sanitize_terminal_title("  Project\t|\nWorking\x1b\x07\u{009D}\u{009C} |  Thread  ");
        assert_eq!(sanitized, "Project | Working | Thread");
    }

    #[test]
    fn strips_invisible_format_chars_from_terminal_title() {
        let sanitized = sanitize_terminal_title(
            "Pro\u{202E}j\u{2066}e\u{200F}c\u{061C}t\u{200B} \u{FEFF}T\u{2060}itle",
        );
        assert_eq!(sanitized, "Project Title");
    }

    #[test]
    fn truncates_terminal_title() {
        let input = "a".repeat(MAX_TERMINAL_TITLE_CHARS + 10);
        let sanitized = sanitize_terminal_title(&input);
        assert_eq!(sanitized.len(), MAX_TERMINAL_TITLE_CHARS);
    }

    #[test]
    fn writes_osc_title_with_string_terminator() {
        let mut out = String::new();
        SetWindowTitle("hello".to_string())
            .write_ansi(&mut out)
            .expect("encode terminal title");
        assert_eq!(out, "\x1b]0;hello\x1b\\");
    }
}
