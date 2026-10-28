//! Emits Codex lifecycle state through terminal status protocols.
//!
//! OSC 7501 reports lifecycle state and blocking reasons to terminal sessions.
//! Direct iTerm2 sessions also receive OSC 21337 status, including bounded
//! activity text. The process-wide cache is used for exit, panic, and terminal
//! handoff cleanup.

use std::fmt;
use std::io;
use std::io::IsTerminal;
use std::io::stdout;
use std::sync::Mutex;
use std::sync::OnceLock;

use codex_terminal_detection::TerminalBackend;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use crossterm::Command;
use ratatui::crossterm::execute;

static SESSION_STATUS_STATE: SessionStatusState = SessionStatusState::new();
static TERMINAL_PROGRAM_STATUS_SUPPORT: OnceLock<TerminalProgramStatusSupport> = OnceLock::new();

// VTE 0.54 introduced a parser that safely ignores unknown OSC sequences.
const MIN_VTE_VERSION_WITH_SAFE_OSC: u32 = 5400;

struct SessionStatusState {
    last_emitted: Mutex<EmittedSessionStatus>,
}

struct EmittedSessionStatus {
    status: CachedStatus,
    detail: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CachedStatus {
    Cleared,
    Reported(ProgramStatus),
    Invalidated,
}

impl SessionStatusState {
    const fn new() -> Self {
        Self {
            last_emitted: Mutex::new(EmittedSessionStatus {
                status: CachedStatus::Cleared,
                detail: String::new(),
            }),
        }
    }

    fn update(
        &self,
        status: CachedStatus,
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
        if last_emitted.status != CachedStatus::Cleared {
            last_emitted.status = CachedStatus::Invalidated;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TerminalProgramStatusSupport {
    osc_7501: bool,
    osc_21337: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProgramStatus {
    Idle,
    Working,
    Blocked(ProgramStatusKind),
    Done,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProgramStatusKind {
    Permission,
    Question,
    Auth,
}

impl ProgramStatus {
    fn iterm_label(self) -> &'static str {
        match self {
            Self::Idle | Self::Done | Self::Error => "Idle",
            Self::Working => "Working",
            Self::Blocked(_) => "Waiting",
        }
    }

    fn iterm_indicator(self) -> &'static str {
        match self {
            Self::Idle | Self::Done | Self::Error => "#00d75f",
            Self::Working => "#ff9500",
            Self::Blocked(_) => "#5f87ff",
        }
    }

    fn protocol_state(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Blocked(_) => "blocked",
            Self::Done => "done",
            Self::Error => "error",
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
    SESSION_STATUS_STATE.update(CachedStatus::Reported(status), &detail, || {
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

    SESSION_STATUS_STATE.update(CachedStatus::Cleared, "", || {
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
    let osc_7501 = stdout_is_terminal
        && match terminal.backend {
            Some(TerminalBackend::Vte { version }) => {
                version.is_none_or(|version| version >= MIN_VTE_VERSION_WITH_SAFE_OSC)
            }
            None => true,
        };
    let osc_21337 = osc_7501
        && terminal.name == TerminalName::Iterm2
        && terminal.multiplexer.is_none()
        && !screen_active;
    TerminalProgramStatusSupport {
        osc_7501,
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
            "\x1b]7501;state={}:app=codex",
            self.status.protocol_state()
        )?;
        if let ProgramStatus::Blocked(kind) = self.status {
            let kind = match kind {
                ProgramStatusKind::Permission => "permission",
                ProgramStatusKind::Question => "question",
                ProgramStatusKind::Auth => "auth",
            };
            write!(f, ":kind={kind}")?;
        }
        write!(f, "\x1b\\")
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
