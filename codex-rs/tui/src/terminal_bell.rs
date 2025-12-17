use std::fmt;
use std::io;
use std::io::stdout;

use crossterm::Command;
use ratatui::crossterm::execute;

#[derive(Debug, Default, Clone, Copy)]
pub struct TerminalBell;

impl TerminalBell {
    pub fn ring(self) -> io::Result<()> {
        execute!(stdout(), RingBell)
    }
}

#[derive(Debug, Clone, Copy)]
struct RingBell;

impl Command for RingBell {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x07")
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::RingBell;
    use crossterm::Command;
    use pretty_assertions::assert_eq;

    #[test]
    fn ring_bell_emits_bel() {
        let mut out = String::new();
        RingBell.write_ansi(&mut out).expect("write_ansi");
        assert_eq!(out, "\u{0007}");
    }
}
