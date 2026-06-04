//! OSC 21337 tab-status output helpers for the TUI.
//!
//! This module owns the low-level write path: callers decide *when* the tab
//! status changes, this only knows how to write the sequence. The
//! `IsTerminal` gate matches the terminal-title module so non-TTY stdout
//! (e.g. piped `codex exec`) doesn't get escape sequences in captured output.
//!
//! OSC 21337 is an iTerm2 extension; other terminals ignore it. The payload
//! is a semicolon-separated list of `key=value` pairs. We emit `status`
//! (label), `indicator` (dot color), and `status-color` (text color).

use std::fmt;
use std::io;
use std::io::IsTerminal;
use std::io::stdout;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crossterm::Command;
use ratatui::crossterm::execute;

/// Whether this process has ever written an OSC 21337 sequence. Gates the
/// shutdown-time `clear_tab_status` so that, when we never emitted, we don't
/// clobber a status set by another tool sharing the tab.
static EMITTED: AtomicBool = AtomicBool::new(/*v*/ false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabStatus {
    Working,
    Waiting,
    Idle,
}

impl TabStatus {
    fn label(self) -> &'static str {
        match self {
            TabStatus::Working => "Working",
            TabStatus::Waiting => "Waiting",
            TabStatus::Idle => "Idle",
        }
    }

    /// Dot color. Matches the palette other agent CLIs emit so multiple
    /// tools read consistently in the tab bar.
    fn indicator(self) -> &'static str {
        match self {
            TabStatus::Working => "#ff9500",
            TabStatus::Waiting => "#5f87ff",
            TabStatus::Idle => "#00d75f",
        }
    }

    /// Status-text color. Working/Waiting match the dot; Idle dims so the
    /// colored dot stays the active affordance once codex is done.
    fn text_color(self) -> &'static str {
        match self {
            TabStatus::Working => "#ff9500",
            TabStatus::Waiting => "#5f87ff",
            TabStatus::Idle => "#888888",
        }
    }
}

pub(crate) fn set_tab_status(status: TabStatus) -> io::Result<()> {
    if !stdout().is_terminal() {
        return Ok(());
    }
    execute!(stdout(), SetTabStatus(status))?;
    EMITTED.store(/*val*/ true, Ordering::Relaxed);
    Ok(())
}

pub(crate) fn clear_tab_status() -> io::Result<()> {
    if !stdout().is_terminal() || !EMITTED.load(Ordering::Relaxed) {
        return Ok(());
    }
    execute!(stdout(), ClearTabStatus)?;
    EMITTED.store(/*val*/ false, Ordering::Relaxed);
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct SetTabStatus(TabStatus);

impl Command for SetTabStatus {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(
            f,
            "\x1b]21337;status={};indicator={};status-color={}\x07",
            self.0.label(),
            self.0.indicator(),
            self.0.text_color()
        )
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute SetTabStatus using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy)]
struct ClearTabStatus;

impl Command for ClearTabStatus {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        // Explicit empty values for every managed field so iTerm clears
        // them rather than leaving stale content visible.
        write!(f, "\x1b]21337;status=;indicator=;status-color=\x07")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute ClearTabStatus using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "tab_status_tests.rs"]
mod tests;
