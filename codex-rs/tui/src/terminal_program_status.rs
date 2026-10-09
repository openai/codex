//! Emits coarse Codex lifecycle state through terminal status protocols.
//!
//! OSC 7501 is safe to send to any terminal and reports only the lifecycle
//! state and app name. Direct iTerm2 sessions also receive the existing OSC
//! 21337 status, including bounded activity text. A process-wide cache matches
//! the terminal session's ownership and prevents stale state when the active
//! chat changes.

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
static TERMINAL_PROGRAM_STATUS_SUPPORT: OnceLock<TerminalProgramStatusSupport> = OnceLock::new();

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
struct TerminalProgramStatusSupport {
    osc_7501: bool,
    osc_21337: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum ProgramStatus {
    Idle = 1,
    Working = 2,
    Blocked = 3,
}

impl ProgramStatus {
    fn iterm_label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Working => "Working",
            Self::Blocked => "Waiting",
        }
    }

    fn iterm_indicator(self) -> &'static str {
        match self {
            Self::Idle => "#00d75f",
            Self::Working => "#ff9500",
            Self::Blocked => "#5f87ff",
        }
    }

    fn protocol_state(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Blocked => "blocked",
        }
    }
}

pub(crate) fn set_terminal_program_status(
    status: ProgramStatus,
    detail: Option<&str>,
) -> io::Result<()> {
    if cfg!(test) {
        return Ok(());
    }

    let support = current_terminal_program_status_support();
    if !support.osc_7501 {
        return Ok(());
    }

    let detail = if support.osc_21337 {
        sanitize_iterm_session_detail(detail.unwrap_or_default())
    } else {
        String::new()
    };
    SESSION_STATUS_STATE.update(status as u8, &detail, || {
        execute!(
            stdout(),
            SetTerminalProgramStatus {
                status,
                iterm_detail: &detail,
                emit_iterm_status: support.osc_21337,
            }
        )
    })
}

pub(crate) fn clear_terminal_program_status() -> io::Result<()> {
    if cfg!(test) {
        return Ok(());
    }

    let support = current_terminal_program_status_support();
    if !support.osc_7501 {
        return Ok(());
    }

    SESSION_STATUS_STATE.update(NO_EMITTED_STATUS, "", || {
        execute!(
            stdout(),
            ClearTerminalProgramStatus {
                emit_iterm_status: support.osc_21337,
            }
        )
    })
}

/// Forgets the cached state after another process has owned the terminal.
///
/// The next active draw will re-emit Codex's current status even if its
/// lifecycle state did not change during the handoff.
pub(crate) fn invalidate_terminal_program_status() {
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

fn current_terminal_program_status_support() -> TerminalProgramStatusSupport {
    *TERMINAL_PROGRAM_STATUS_SUPPORT.get_or_init(|| {
        terminal_program_status_support(
            &terminal_info(),
            /*stdout_is_terminal*/ stdout().is_terminal(),
            /*screen_active*/ std::env::var_os("STY").is_some(),
        )
    })
}

fn terminal_program_status_support(
    terminal: &TerminalInfo,
    stdout_is_terminal: bool,
    screen_active: bool,
) -> TerminalProgramStatusSupport {
    let osc_21337 = stdout_is_terminal
        && terminal.name == TerminalName::Iterm2
        && terminal.multiplexer.is_none()
        && !screen_active;
    TerminalProgramStatusSupport {
        osc_7501: stdout_is_terminal,
        osc_21337,
    }
}

#[derive(Clone, Copy, Debug)]
struct SetTerminalProgramStatus<'a> {
    status: ProgramStatus,
    iterm_detail: &'a str,
    emit_iterm_status: bool,
}

impl Command for SetTerminalProgramStatus<'_> {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if self.emit_iterm_status {
            write!(
                f,
                "\x1b]21337;status={};indicator={};status-color=;detail={}\x07",
                self.status.iterm_label(),
                self.status.iterm_indicator(),
                self.iterm_detail,
            )?;
        }
        write!(
            f,
            "\x1b]7501;state={}:app=codex\x1b\\",
            self.status.protocol_state(),
        )
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::other(
            "tried to set terminal program status using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug)]
struct ClearTerminalProgramStatus {
    emit_iterm_status: bool,
}

impl Command for ClearTerminalProgramStatus {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if self.emit_iterm_status {
            write!(f, "\x1b]21337;status=;indicator=;status-color=;detail=\x07")?;
        }
        write!(f, "\x1b]7501;state=clear\x1b\\")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::other(
            "tried to clear terminal program status using WinAPI; use ANSI instead",
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
