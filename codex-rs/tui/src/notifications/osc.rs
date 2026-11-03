//! Writes terminal desktop notifications, wrapping the OSC sequence for tmux when needed.

use std::fmt;
use std::io;
use std::io::stdout;

use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use crossterm::Command;
use ratatui::crossterm::execute;

// Ghostty copies the parsed notification body into a 255-byte buffer.
const GHOSTTY_BODY_BYTES: usize = 255;

#[derive(Debug, Clone, Copy)]
pub(super) enum OscProtocol {
    Osc9,
    Osc777,
}

#[derive(Debug)]
pub struct OscBackend {
    pub(super) protocol: OscProtocol,
    terminal_name: TerminalName,
    dcs_passthrough: bool,
}

impl OscBackend {
    pub(super) fn new(protocol: OscProtocol) -> Self {
        let terminal = terminal_info();
        Self {
            protocol,
            terminal_name: terminal.name,
            dcs_passthrough: matches!(terminal.multiplexer, Some(Multiplexer::Tmux { .. })),
        }
    }

    pub fn notify(&mut self, message: &str) -> io::Result<()> {
        execute!(
            stdout(),
            PostNotification {
                protocol: self.protocol,
                message,
                terminal_name: self.terminal_name,
                dcs_passthrough: self.dcs_passthrough,
            }
        )
    }
}

struct PostNotification<'a> {
    protocol: OscProtocol,
    message: &'a str,
    terminal_name: TerminalName,
    dcs_passthrough: bool,
}

impl Command for PostNotification<'_> {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        let mut body: String = self
            .message
            .chars()
            .map(|c| match c {
                // ASCII semicolons delimit OSC 777 fields.
                ';' if matches!(self.protocol, OscProtocol::Osc777) => '；',
                c if c.is_control() => ' ',
                c => c,
            })
            .collect();
        if self.terminal_name == TerminalName::Ghostty && body.len() > GHOSTTY_BODY_BYTES {
            body.truncate(body.floor_char_boundary(GHOSTTY_BODY_BYTES - '…'.len_utf8()));
            body.push('…');
        }
        let sequence = match self.protocol {
            OscProtocol::Osc9 => format!("\x1b]9;{body}\x07"),
            OscProtocol::Osc777 => format!("\x1b]777;notify;Codex;{body}\x1b\\"),
        };
        if self.dcs_passthrough {
            let escaped = sequence.replace('\x1b', "\x1b\x1b");
            write!(f, "\x1bPtmux;{escaped}\x1b\\")
        } else {
            f.write_str(&sequence)
        }
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute PostNotification using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use codex_terminal_detection::TerminalName;
    use crossterm::Command;
    use pretty_assertions::assert_eq;

    use super::OscProtocol;
    use super::PostNotification;

    #[test]
    fn post_notification_writes_plain_osc9_sequence() {
        let message = "a".repeat(2048);
        let mut ansi = String::new();
        let command = PostNotification {
            protocol: OscProtocol::Osc9,
            message: &message,
            terminal_name: TerminalName::Iterm2,
            dcs_passthrough: false,
        };

        command
            .write_ansi(&mut ansi)
            .expect("OSC 9 command should format");

        assert_eq!(ansi, format!("\u{1b}]9;{message}\u{7}"));
    }

    #[test]
    fn long_approval_in_ghostty_fits_the_notification_body() {
        // The next UTF-8 character crosses the cutoff for the ellipsis.
        let prefix = format!("Approval requested: {}", "a".repeat(231));
        let message = format!("{prefix}🦀{}", "b".repeat(2048));
        for (protocol, expected) in [
            (OscProtocol::Osc9, format!("\x1b]9;{prefix}…\x07")),
            (
                OscProtocol::Osc777,
                format!("\x1b]777;notify;Codex;{prefix}…\x1b\\"),
            ),
        ] {
            let mut ansi = String::new();
            PostNotification {
                protocol,
                message: &message,
                terminal_name: TerminalName::Ghostty,
                dcs_passthrough: false,
            }
            .write_ansi(&mut ansi)
            .expect("format Ghostty notification");
            assert_eq!(ansi, expected);
        }
    }

    #[test]
    fn post_notification_writes_tmux_dcs_wrapped_osc9_sequence() {
        let mut ansi = String::new();
        let command = PostNotification {
            protocol: OscProtocol::Osc9,
            message: "done",
            terminal_name: TerminalName::Iterm2,
            dcs_passthrough: true,
        };

        command
            .write_ansi(&mut ansi)
            .expect("OSC 9 command should format");

        assert_eq!(ansi, "\u{1b}Ptmux;\u{1b}\u{1b}]9;done\u{7}\u{1b}\\");
    }

    #[test]
    fn post_notification_keeps_control_characters_out_of_osc9_body() {
        let mut ansi = String::new();
        let command = PostNotification {
            protocol: OscProtocol::Osc9,
            message: "before\x07\x1bafter",
            terminal_name: TerminalName::Iterm2,
            dcs_passthrough: true,
        };

        command
            .write_ansi(&mut ansi)
            .expect("OSC 9 command should format");

        assert_eq!(
            ansi,
            "\u{1b}Ptmux;\u{1b}\u{1b}]9;before  after\u{7}\u{1b}\\"
        );
    }

    #[test]
    fn osc777_sends_title_and_plain_text_body_directly_and_through_tmux() {
        let mut sequences = Vec::new();
        for dcs_passthrough in [false, true] {
            let command = PostNotification {
                protocol: OscProtocol::Osc777,
                message: "Approval requested: cargo test; git clean\x07\x1b\u{009c}-fd",
                terminal_name: TerminalName::WindowsTerminal,
                dcs_passthrough,
            };
            let mut ansi = String::new();
            command.write_ansi(&mut ansi).expect("format OSC 777");
            sequences.push(ansi);
        }
        insta::assert_debug_snapshot!(sequences, @r#"
        [
            "\u{1b}]777;notify;Codex;Approval requested: cargo test； git clean   -fd\u{1b}\\",
            "\u{1b}Ptmux;\u{1b}\u{1b}]777;notify;Codex;Approval requested: cargo test； git clean   -fd\u{1b}\u{1b}\\\u{1b}\\",
        ]
        "#);
    }
}
