//! Emits OSC 9 terminal notifications for lightweight desktop alerts.
//!
//! This module provides a small wrapper around the OSC 9 escape sequence so
//! the notification backend can signal simple status changes without depending
//! on platform-specific notification APIs. It is intentionally minimal: callers
//! are expected to pass already-formatted text and decide when notifications
//! should be emitted.

use std::fmt;
use std::io;
use std::io::stdout;

use crossterm::Command;
use ratatui::crossterm::execute;

/// Sends notifications using the OSC 9 escape sequence.
///
/// This backend writes an ANSI escape sequence to stdout; the terminal decides
/// whether to surface it as a desktop notification.
#[derive(Debug, Default)]
pub struct Osc9Backend;

impl Osc9Backend {
    /// Emit a single OSC 9 notification containing the provided message.
    pub fn notify(&mut self, message: &str) -> io::Result<()> {
        execute!(stdout(), PostNotification(message.to_string()))
    }
}

/// Command that emits an OSC 9 desktop notification with a message.
#[derive(Debug, Clone)]
pub struct PostNotification(
    /// Fully formatted notification text to include in the OSC 9 payload.
    pub String,
);

impl Command for PostNotification {
    /// Write the OSC 9 sequence in ANSI form.
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b]9;{}\x07", self.0)
    }

    #[cfg(windows)]
    /// Reject WinAPI execution so the caller must emit ANSI instead.
    fn execute_winapi(&self) -> io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute PostNotification using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    /// Report ANSI support so crossterm uses the ANSI path.
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}
