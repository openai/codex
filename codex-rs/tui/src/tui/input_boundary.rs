//! Restore terminal ownership on initialization failures and drain input across screen transitions.

use std::collections::VecDeque;
use std::io;
#[cfg(unix)]
use std::io::IsTerminal;
use std::io::Result;
#[cfg(unix)]
use std::io::stdin;
use std::time::Duration;
use std::time::Instant;

use crossterm::event::Event;

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

    drain_pending_input(/*focus_events*/ None)
}

/// A thread transition drains decoded events first so it can retain focus changes
/// and let a standalone Escape expire before quarantining incomplete sequences.
pub(super) fn drain_pending_input(mut focus_events: Option<&mut VecDeque<Event>>) -> Result<()> {
    let budget = if focus_events.is_some() {
        // Yield to the event loop between batches of continuous typeahead.
        Duration::from_millis(/*millis*/ 100)
    } else {
        Duration::from_secs(/*secs*/ 1)
    };
    let deadline = Instant::now() + budget;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out discarding buffered terminal input",
            ));
        }

        if crossterm::event::poll(remaining.min(Duration::from_millis(/*millis*/ 10)))? {
            let event = crossterm::event::read()?;
            if matches!(event, Event::FocusLost | Event::FocusGained)
                && let Some(events) = &mut focus_events
            {
                events.push_back(event);
            }
            continue;
        }

        #[cfg(unix)]
        if focus_events.is_some() {
            // Crossterm may have read a bare Escape just before poll returned.
            // Wait for its 20 ms parser timeout without consuming more bytes.
            // New bytes restart the drain. Poll once more to resolve the Escape.
            match poll_terminal_input(/*timeout_ms*/ 20) {
                Ok(true) => continue,
                Ok(false) => {}
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(err),
            }
            if crossterm::event::poll(Duration::ZERO)? {
                continue;
            }
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
    poll_terminal_input(/*timeout_ms*/ 0)
}

#[cfg(unix)]
fn poll_terminal_input(timeout_ms: libc::c_int) -> Result<bool> {
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
    // Safety: `fd` is valid for the duration of the poll.
    let result = unsafe {
        libc::poll(&mut fd, /*nfds*/ 1, timeout_ms)
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
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return Err(io::Error::other("terminal input handle is unavailable"));
    }

    let mut pending = 0;
    if unsafe { GetNumberOfConsoleInputEvents(handle, &mut pending) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(pending != 0)
}
