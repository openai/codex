use std::fmt;
use std::io;
use std::io::stdout;

use crossterm::Command;
use ratatui::crossterm::execute;

#[derive(Debug, Default)]
pub struct BelBackend;

impl BelBackend {
    pub fn notify(&mut self, _message: &str) -> io::Result<()> {
        execute!(stdout(), PostNotification)
    }
}

/// Command that emits a BEL desktop notification.
#[derive(Debug, Clone)]
pub struct PostNotification;

impl Command for PostNotification {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x07")
    }
}
