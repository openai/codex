//! Restore terminal ownership on initialization failures and quarantine protected-screen input.

use std::io;
#[cfg(unix)]
use std::io::IsTerminal;
use std::io::Result;
#[cfg(unix)]
use std::io::stdin;
use std::time::Duration;
use std::time::Instant;

/// Restores terminal modes when initialization fails before ownership is transferred.
pub(super) struct TerminalInitializationGuard {
    pub(super) active: bool,
}

impl Drop for TerminalInitializationGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = super::restore_after_exit();
        }
    }
}

/// Drain decoded and unread input before a screen can accept a security-sensitive decision.
///
/// Partial control sequences remain quarantined; an unresolved boundary fails after one second.
pub(crate) fn discard_pending_terminal_input() -> Result<()> {
    #[cfg(unix)]
    let _ = crossterm::event::discard_buffered_input()?;

    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 1);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out discarding buffered terminal input",
            ));
        }

        if crossterm::event::poll(remaining.min(Duration::from_millis(/*millis*/ 10)))? {
            let _ = crossterm::event::read()?;
            continue;
        }

        #[cfg(unix)]
        if crossterm::event::discard_buffered_input()?
            != crossterm::event::InputDiscardStatus::Complete
        {
            continue;
        }

        #[cfg(any(unix, windows))]
        match terminal_input_is_readable() {
            Ok(true) => continue,
            Ok(false) => {}
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }

        return Ok(());
    }
}

/// Check Crossterm's terminal input source without consuming input or changing descriptor flags.
#[cfg(unix)]
pub(super) fn terminal_input_is_readable() -> Result<bool> {
    use std::os::fd::AsRawFd;

    let terminal = if stdin().is_terminal() {
        None
    } else {
        Some(std::fs::File::open("/dev/tty")?)
    };
    let mut fd = libc::pollfd {
        fd: terminal
            .as_ref()
            .map_or(libc::STDIN_FILENO, AsRawFd::as_raw_fd),
        events: libc::POLLIN,
        revents: 0,
    };
    // Safety: `fd` is valid for the duration of this zero-timeout poll.
    let result = unsafe {
        libc::poll(&mut fd, /*nfds*/ 1, /*timeout*/ 0)
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    if fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
        return Err(io::Error::other(format!(
            "terminal input poll failed with events {:#x}",
            fd.revents
        )));
    }
    Ok(result > 0 && fd.revents & libc::POLLIN != 0)
}

/// Check the same Windows console input queue consumed by Crossterm.
#[cfg(windows)]
fn terminal_input_is_readable() -> Result<bool> {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::GetNumberOfConsoleInputEvents;
    use windows_sys::Win32::System::Console::GetStdHandle;
    use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;

    let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if handle == INVALID_HANDLE_VALUE || handle == 0 {
        return Err(io::Error::other("terminal input handle is unavailable"));
    }

    let mut pending = 0;
    if unsafe { GetNumberOfConsoleInputEvents(handle, &mut pending) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(pending != 0)
}
