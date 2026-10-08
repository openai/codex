//! Emits coarse Codex lifecycle state through iTerm2's OSC 21337 protocol.
//!
//! This first integration is intentionally limited to direct iTerm2 sessions.
//! The payload includes bounded activity text from the existing TUI status.
//! A process-wide cache matches the terminal session's ownership and prevents
//! stale state when the active chat changes.

use std::fmt;
use std::io;
use std::io::IsTerminal;
use std::io::stdout;
use std::sync::Mutex;
use std::sync::OnceLock;

use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use crossterm::Command;
use ratatui::crossterm::execute;

const NO_EMITTED_STATUS: u8 = 0;
const INVALIDATED_EMITTED_STATUS: u8 = u8::MAX;
static SESSION_STATUS_STATE: SessionStatusState = SessionStatusState::new();
static ITERM_SESSION_STATUS_SUPPORTED: OnceLock<bool> = OnceLock::new();

struct SessionStatusState {
    last_emitted: Mutex<EmittedSessionStatus>,
}

struct EmittedSessionStatus {
    status: u8,
    detail: String,
}

impl SessionStatusState {
    const fn new() -> Self {
        Self {
            last_emitted: Mutex::new(EmittedSessionStatus {
                status: NO_EMITTED_STATUS,
                detail: String::new(),
            }),
        }
    }

    fn update(
        &self,
        status: u8,
        detail: &str,
        emit: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<()> {
        // Keep the terminal write and cache mutation together so panic-hook cleanup cannot
        // interleave with a draw from another thread.
        let mut last_emitted = self
            .last_emitted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if last_emitted.status == status && last_emitted.detail == detail {
            return Ok(());
        }
        emit()?;
        last_emitted.status = status;
        detail.clone_into(&mut last_emitted.detail);
        Ok(())
    }

    fn invalidate(&self) {
        let mut last_emitted = self
            .last_emitted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if last_emitted.status != NO_EMITTED_STATUS {
            last_emitted.status = INVALIDATED_EMITTED_STATUS;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum ItermSessionStatus {
    Idle = 1,
    Working = 2,
    Waiting = 3,
}

impl ItermSessionStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Working => "Working",
            Self::Waiting => "Waiting",
        }
    }

    fn indicator(self) -> &'static str {
        match self {
            Self::Idle => "#00d75f",
            Self::Working => "#ff9500",
            Self::Waiting => "#5f87ff",
        }
    }
}

pub(crate) fn set_iterm_session_status(
    status: ItermSessionStatus,
    detail: Option<&str>,
) -> io::Result<()> {
    if cfg!(test) {
        return Ok(());
    }

    if !*ITERM_SESSION_STATUS_SUPPORTED.get_or_init(|| {
        iterm_session_status_supported(
            &terminal_info(),
            /*stdout_is_terminal*/ stdout().is_terminal(),
            /*screen_active*/ std::env::var_os("STY").is_some(),
        )
    }) {
        return Ok(());
    }

    let detail = sanitize_iterm_session_detail(detail.unwrap_or_default());
    SESSION_STATUS_STATE.update(status as u8, &detail, || {
        execute!(stdout(), SetItermSessionStatus(status, &detail))
    })
}

pub(crate) fn clear_iterm_session_status() -> io::Result<()> {
    SESSION_STATUS_STATE.update(NO_EMITTED_STATUS, "", || {
        execute!(stdout(), ClearItermSessionStatus)
    })
}

/// Forgets the cached state after another process has owned the terminal.
///
/// The next active draw will re-emit Codex's current status even if its
/// lifecycle state did not change during the handoff.
pub(crate) fn invalidate_iterm_session_status() {
    SESSION_STATUS_STATE.invalidate();
}

fn sanitize_iterm_session_detail(detail: &str) -> String {
    let detail = crate::terminal_title::sanitize_terminal_title(
        detail,
        crate::terminal_title::TitleEncoding::Unicode,
    );
    let mut escaped = String::with_capacity(detail.len());
    for ch in detail.chars() {
        if matches!(ch, ';' | '\\') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

fn iterm_session_status_supported(
    terminal: &TerminalInfo,
    stdout_is_terminal: bool,
    screen_active: bool,
) -> bool {
    stdout_is_terminal
        && terminal.name == TerminalName::Iterm2
        && terminal.multiplexer.is_none()
        && !screen_active
}

#[derive(Clone, Copy, Debug)]
struct SetItermSessionStatus<'a>(ItermSessionStatus, &'a str);

impl Command for SetItermSessionStatus<'_> {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(
            f,
            "\x1b]21337;status={};indicator={};status-color=;detail={}\x07",
            self.0.label(),
            self.0.indicator(),
            self.1,
        )
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::other(
            "tried to set iTerm2 session status using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug)]
struct ClearItermSessionStatus;

impl Command for ClearItermSessionStatus {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b]21337;status=;indicator=;status-color=;detail=\x07")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::other(
            "tried to clear iTerm2 session status using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "iterm_session_status_tests.rs"]
mod tests;
