//! Gate forced-ANSI TUI output that can leak raw bytes on Windows.

use std::io;
use std::io::stdout;

use codex_terminal_detection::WindowsStdoutVtState;
use crossterm::SynchronizedUpdate;

/// Whether raw ANSI writes should be emitted through the current stdout path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RawAnsiCapability {
    /// Raw ANSI should follow the existing TUI write path.
    Available,
    /// Stdout is an inspectable Windows console handle whose VT mode is still disabled.
    Unavailable,
}

impl RawAnsiCapability {
    pub(crate) fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Enables and verifies Windows stdout VT processing when that mode is inspectable.
///
/// Handles that do not expose Windows console mode inspection preserve the
/// existing PTY behavior and continue to allow raw ANSI output.
pub(crate) fn stdout_capability() -> RawAnsiCapability {
    #[cfg(windows)]
    {
        let state = codex_terminal_detection::enable_windows_stdout_vt_processing();
        let capability = capability_for_windows_stdout_vt_state(state);
        if !capability.is_available() {
            warn_windows_stdout_vt_disabled(state);
        }
        capability
    }

    #[cfg(not(windows))]
    {
        capability_for_windows_stdout_vt_state(WindowsStdoutVtState::Unavailable)
    }
}

/// Runs a draw update with synchronized-update wrappers only when raw ANSI is safe.
pub(crate) fn synchronized_update<T>(update: impl FnOnce() -> T) -> io::Result<T> {
    match stdout_capability() {
        RawAnsiCapability::Available => stdout().sync_update(|_| update()),
        RawAnsiCapability::Unavailable => Ok(update()),
    }
}

fn capability_for_windows_stdout_vt_state(state: WindowsStdoutVtState) -> RawAnsiCapability {
    match state {
        WindowsStdoutVtState::Disabled { .. } => RawAnsiCapability::Unavailable,
        WindowsStdoutVtState::Enabled { .. } | WindowsStdoutVtState::Unavailable => {
            RawAnsiCapability::Available
        }
    }
}

#[cfg(windows)]
fn warn_windows_stdout_vt_disabled(state: WindowsStdoutVtState) {
    use std::sync::Once;

    static WARNED: Once = Once::new();

    WARNED.call_once(|| {
        tracing::warn!(
            ?state,
            "Windows stdout VT processing is disabled; skipping raw ANSI TUI output"
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_windows_stdout_state_keeps_raw_ansi_available() {
        assert_eq!(
            capability_for_windows_stdout_vt_state(WindowsStdoutVtState::Unavailable),
            RawAnsiCapability::Available
        );
    }

    #[test]
    fn disabled_windows_stdout_state_blocks_raw_ansi() {
        assert_eq!(
            capability_for_windows_stdout_vt_state(WindowsStdoutVtState::Disabled {
                console_mode: 3,
            }),
            RawAnsiCapability::Unavailable
        );
    }
}
