use base64::Engine;
use std::io::Write;

/// Copy text to the system clipboard.
///
/// Over SSH, uses OSC 52 so the text reaches the *local* terminal emulator's
/// clipboard rather than a remote X11/Wayland clipboard that the user cannot
/// access. On a local session, tries `arboard` (native clipboard) first and
/// falls back to OSC 52 if that fails.
///
/// OSC 52 is supported by kitty, WezTerm, iTerm2, Ghostty, and others.
pub(crate) fn copy_to_clipboard(text: &str) -> Result<(), String> {
    copy_to_clipboard_with(text, is_ssh_session(), osc52_copy, arboard_copy)
}

fn copy_to_clipboard_with(
    text: &str,
    ssh_session: bool,
    osc52_copy_fn: impl Fn(&str) -> Result<(), String>,
    arboard_copy_fn: impl Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    if ssh_session {
        // Over SSH the native clipboard writes to the remote machine which is
        // useless. Use OSC 52, which travels through the SSH tunnel to the
        // local terminal emulator.
        return osc52_copy_fn(text).map_err(|osc_err| {
            tracing::warn!("OSC 52 clipboard copy failed over SSH: {osc_err}");
            format!("OSC 52 clipboard copy failed over SSH: {osc_err}")
        });
    }

    match arboard_copy_fn(text) {
        Ok(()) => Ok(()),
        Err(native_err) => {
            tracing::warn!("native clipboard copy failed: {native_err}, falling back to OSC 52");
            osc52_copy_fn(text).map_err(|osc_err| {
                format!("native clipboard: {native_err}; OSC 52 fallback: {osc_err}")
            })
        }
    }
}

/// Detect whether the current process is running inside an SSH session.
fn is_ssh_session() -> bool {
    std::env::var_os("SSH_TTY").is_some() || std::env::var_os("SSH_CONNECTION").is_some()
}

/// Run arboard with stderr suppressed.
///
/// On macOS, `arboard::Clipboard::new()` initializes `NSPasteboard` which
/// triggers `os_log` / `NSLog` output on stderr. Because the TUI owns the
/// terminal, that stray output corrupts the display. We temporarily redirect
/// fd 2 to `/dev/null` around the call to keep the screen clean.
fn arboard_copy(text: &str) -> Result<(), String> {
    let _guard = SuppressStderr::new();
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
    clipboard
        .set_text(text)
        .map_err(|e| format!("failed to set clipboard text: {e}"))
}

/// RAII guard that redirects stderr (fd 2) to `/dev/null` on creation and
/// restores the original fd on drop.
struct SuppressStderr {
    saved_fd: Option<libc::c_int>,
}

impl SuppressStderr {
    fn new() -> Self {
        unsafe {
            // Save the current stderr fd.
            let saved = libc::dup(2);
            if saved < 0 {
                return Self { saved_fd: None };
            }
            // Open /dev/null and point fd 2 at it.
            let devnull = libc::open(b"/dev/null\0".as_ptr().cast(), libc::O_WRONLY);
            if devnull >= 0 {
                libc::dup2(devnull, 2);
                libc::close(devnull);
            }
            Self {
                saved_fd: Some(saved),
            }
        }
    }
}

impl Drop for SuppressStderr {
    fn drop(&mut self) {
        if let Some(saved) = self.saved_fd {
            unsafe {
                libc::dup2(saved, 2);
                libc::close(saved);
            }
        }
    }
}

/// Write text to the clipboard via the OSC 52 terminal escape sequence.
fn osc52_copy(text: &str) -> Result<(), String> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let sequence = format!("\x1b]52;c;{encoded}\x07");
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(sequence.as_bytes())
        .map_err(|e| format!("failed to write OSC 52: {e}"))?;
    stdout
        .flush()
        .map_err(|e| format!("failed to flush OSC 52: {e}"))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::cell::Cell;

    use super::copy_to_clipboard_with;

    #[test]
    fn osc52_encoding_roundtrips() {
        use base64::Engine;
        let text = "# Hello\n\n```rust\nfn main() {}\n```\n";
        let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .unwrap();
        assert_eq!(decoded, text.as_bytes());
    }

    #[test]
    fn ssh_uses_osc52_and_skips_native_on_success() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            true,
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Ok(())
            },
            |_| {
                native_calls.set(native_calls.get() + 1);
                Ok(())
            },
        );

        assert_eq!(result, Ok(()));
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 0);
    }

    #[test]
    fn ssh_returns_osc52_error_and_skips_native() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            true,
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Err("blocked".into())
            },
            |_| {
                native_calls.set(native_calls.get() + 1);
                Ok(())
            },
        );

        assert_eq!(
            result,
            Err("OSC 52 clipboard copy failed over SSH: blocked".into())
        );
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 0);
    }

    #[test]
    fn local_uses_native_clipboard_first() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            false,
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Ok(())
            },
            |_| {
                native_calls.set(native_calls.get() + 1);
                Ok(())
            },
        );

        assert_eq!(result, Ok(()));
        assert_eq!(osc_calls.get(), 0);
        assert_eq!(native_calls.get(), 1);
    }

    #[test]
    fn local_falls_back_to_osc52_when_native_fails() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            false,
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Ok(())
            },
            |_| {
                native_calls.set(native_calls.get() + 1);
                Err("native unavailable".into())
            },
        );

        assert_eq!(result, Ok(()));
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
    }

    #[test]
    fn local_reports_both_errors_when_native_and_osc52_fail() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            false,
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Err("osc blocked".into())
            },
            |_| {
                native_calls.set(native_calls.get() + 1);
                Err("native unavailable".into())
            },
        );

        assert_eq!(
            result,
            Err("native clipboard: native unavailable; OSC 52 fallback: osc blocked".into())
        );
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
    }
}
