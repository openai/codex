//! Clipboard copy backend for the TUI's `/copy` command and `Ctrl+O` hotkey.
//!
//! Local copying uses the native clipboard, with WSL PowerShell as a fallback.
//! In tmux, also forward to the attached terminal so clients attached after Codex
//! started receive the copy. Over SSH without tmux, send OSC 52 directly.
//!
//! Terminal writes have no delivery acknowledgement: a successful send must not
//! suppress native copying. Both the host and attached client's clipboards may
//! change. Outside tmux and SSH, use OSC 52 only if native copying fails.
//!
//! On Linux, X11 and some Wayland compositors require the process that wrote the
//! clipboard to keep its handle open. `ClipboardLease` wraps the `arboard::Clipboard`
//! so callers can store it for the lifetime of the TUI. On other platforms the lease
//! is always `None`.
//!
//! Empty selections fail without modifying a clipboard.
//! Markdown copies also offer HTML on the native clipboard. Terminal and WSL
//! fallbacks retain the original text. Image paste lives in `clipboard_paste`.

mod tmux;

use base64::Engine;
use std::io::Write;
use tmux::copy as tmux_clipboard_copy;

/// Maximum raw bytes sent through tmux or directly encoded into OSC 52.
/// Large payloads are rejected before encoding to avoid overwhelming the terminal.
const OSC52_MAX_RAW_BYTES: usize = 100_000;
#[cfg(target_os = "macos")]
static STDERR_SUPPRESSION_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> =
    std::sync::OnceLock::new();

/// Whether copied text should also have a rendered HTML representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CopyFormat {
    PlainText,
    Markdown,
}

/// Copy text to the system clipboard.
///
/// Try native copying, then independently attempt terminal forwarding in tmux or
/// SSH. A terminal send is best effort and does not confirm clipboard delivery.
/// Terminal forwarding may replace native HTML with plain text.
///
/// A returned lease replaces the previous native lease. `None` means no new
/// lease is needed; callers must retain any existing lease so terminal copies
/// do not release ownership of an earlier native clipboard value.
///
/// OSC 52 is supported by kitty, WezTerm, iTerm2, Ghostty, and others.
pub(crate) fn copy_to_clipboard(
    text: &str,
    format: CopyFormat,
) -> Result<Option<ClipboardLease>, String> {
    copy_to_clipboard_with(
        text,
        format,
        CopyEnvironment {
            ssh_session: is_ssh_session(),
            wsl_session: is_wsl_session(),
            tmux_session: is_tmux_session(),
        },
        tmux_clipboard_copy,
        osc52_copy,
        arboard_copy,
        wsl_clipboard_copy,
    )
}

/// Keeps a platform clipboard owner alive when the backend requires one.
///
/// On Linux/X11 and some Wayland compositors, clipboard contents are served by the
/// owning process. Dropping the `arboard::Clipboard` before the user pastes causes
/// the content to vanish. Store this lease on the widget that triggered the copy so
/// the handle lives as long as the TUI does. On non-Linux native paths and OSC 52
/// paths the lease is `None` — those backends do not require process-lifetime
/// ownership.
pub(crate) struct ClipboardLease {
    #[cfg(target_os = "linux")]
    _clipboard: Option<arboard::Clipboard>,
}

impl ClipboardLease {
    #[cfg(target_os = "linux")]
    fn native_linux(clipboard: arboard::Clipboard) -> Self {
        Self {
            _clipboard: Some(clipboard),
        }
    }

    #[cfg(test)]
    pub(crate) fn test() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            _clipboard: None,
        }
    }
}

/// Core copy logic with injected backends, enabling deterministic unit tests
/// without touching real clipboards or terminal I/O.
#[derive(Clone, Copy)]
struct CopyEnvironment {
    ssh_session: bool,
    wsl_session: bool,
    tmux_session: bool,
}

fn copy_to_clipboard_with(
    text: &str,
    format: CopyFormat,
    environment: CopyEnvironment,
    tmux_copy_fn: impl Fn(&str) -> Result<(), String>,
    osc52_copy_fn: impl Fn(&str) -> Result<(), String>,
    arboard_copy_fn: impl Fn(&str, Option<&str>) -> Result<Option<ClipboardLease>, String>,
    wsl_copy_fn: impl Fn(&str) -> Result<(), String>,
) -> Result<Option<ClipboardLease>, String> {
    if text.is_empty() {
        return Err("nothing to copy: the selected content is empty".to_string());
    }

    let terminal_copy = || {
        if environment.tmux_session {
            let raw_bytes = text.len();
            let result = if raw_bytes > OSC52_MAX_RAW_BYTES {
                Err(format!(
                    "tmux clipboard payload too large ({raw_bytes} bytes; max {OSC52_MAX_RAW_BYTES})"
                ))
            } else {
                tmux_copy_fn(text)
            };
            result.or_else(|tmux_error| {
                osc52_copy_fn(text).map_err(|osc_error| {
                    format!("tmux clipboard: {tmux_error}; OSC 52 fallback: {osc_error}")
                })
            })
        } else {
            osc52_copy_fn(text)
        }
    };
    let html = match format {
        CopyFormat::PlainText => None,
        CopyFormat::Markdown => Some(crate::clipboard_html::render_markdown(text)),
    };
    let native_result = arboard_copy_fn(text, html.as_deref()).or_else(|native_error| {
        if environment.wsl_session {
            wsl_copy_fn(text).map(|()| None).map_err(|wsl_error| {
                format!("native clipboard: {native_error}; WSL fallback: {wsl_error}")
            })
        } else {
            Err(format!("native clipboard: {native_error}"))
        }
    });
    // Copy natively first: an X11 SelectionClear from a terminal write can otherwise
    // race with arboard reusing its ownership window and clear the new native data.
    // Persistent tmux sessions may gain remote clients after Codex starts, so still
    // forward even when no SSH variables were inherited or native copying succeeded.
    let terminal_result = (environment.tmux_session || environment.ssh_session).then(terminal_copy);
    match native_result {
        Ok(lease) => Ok(lease),
        Err(native_error) => terminal_result
            .unwrap_or_else(terminal_copy)
            .map(|()| None)
            .map_err(|terminal_error| {
                format!("{native_error}; terminal clipboard: {terminal_error}")
            }),
    }
}

/// Detect whether the current process is running inside an SSH session.
fn is_ssh_session() -> bool {
    std::env::var_os("SSH_TTY").is_some() || std::env::var_os("SSH_CONNECTION").is_some()
}

/// Detect whether the current process is running inside tmux.
fn is_tmux_session() -> bool {
    std::env::var_os("TMUX").is_some() || std::env::var_os("TMUX_PANE").is_some()
}

#[cfg(target_os = "linux")]
fn is_wsl_session() -> bool {
    crate::clipboard_paste::is_probably_wsl()
}

#[cfg(not(target_os = "linux"))]
fn is_wsl_session() -> bool {
    false
}

/// Run arboard with stderr suppressed.
///
/// On macOS, `arboard::Clipboard::new()` initializes `NSPasteboard` which
/// triggers `os_log` / `NSLog` output on stderr. Because the TUI owns the
/// terminal, that stray output corrupts the display. We temporarily redirect
/// fd 2 to `/dev/null` around the call to keep the screen clean.
#[cfg(not(target_os = "android"))]
fn arboard_copy(text: &str, html: Option<&str>) -> Result<Option<ClipboardLease>, String> {
    #[cfg(target_os = "macos")]
    let _stderr_lock = STDERR_SUPPRESSION_MUTEX
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .map_err(|_| "stderr suppression lock poisoned".to_string())?;
    let _guard = SuppressStderr::new();
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
    match html {
        Some(html) => clipboard
            .set_html(html, Some(text))
            .or_else(|_| clipboard.set_text(text)),
        None => clipboard.set_text(text),
    }
    .map_err(|e| format!("failed to set clipboard text: {e}"))?;
    // Linux clipboard owners must stay alive until the user pastes.
    #[cfg(target_os = "linux")]
    {
        Ok(Some(ClipboardLease::native_linux(clipboard)))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(None)
    }
}

#[cfg(target_os = "android")]
fn arboard_copy(_text: &str, _html: Option<&str>) -> Result<Option<ClipboardLease>, String> {
    Err("native clipboard unavailable on Android".to_string())
}

/// Copy text into the Windows clipboard from a WSL process.
#[cfg(target_os = "linux")]
fn wsl_clipboard_copy(text: &str) -> Result<(), String> {
    let executable = codex_utils_path::system_executable("powershell.exe")
        .ok_or_else(|| "PowerShell is unavailable in the system PATH".to_string())?;
    let path = codex_utils_path::system_path()
        .map_err(|error| format!("failed to resolve system PATH: {error}"))?;
    let mut child = std::process::Command::new(executable)
        .env("PATH", path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .args([
            "-NoProfile",
            "-Command",
            "[Console]::InputEncoding = [System.Text.Encoding]::UTF8; $ErrorActionPreference = 'Stop'; $text = [Console]::In.ReadToEnd(); Set-Clipboard -Value $text",
        ])
        .spawn()
        .map_err(|e| format!("failed to spawn powershell.exe: {e}"))?;

    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("failed to open powershell.exe stdin".to_string());
    };

    if let Err(err) = stdin.write_all(text.as_bytes()) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("failed to write to powershell.exe: {err}"));
    }

    drop(stdin);

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for powershell.exe: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            let status = output.status;
            Err(format!("powershell.exe exited with status {status}"))
        } else {
            Err(format!("powershell.exe failed: {stderr}"))
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn wsl_clipboard_copy(_text: &str) -> Result<(), String> {
    Err("WSL clipboard fallback unavailable on this platform".to_string())
}

/// RAII guard that redirects stderr (fd 2) to `/dev/null` on creation and
/// restores the original fd on drop.
#[cfg(target_os = "macos")]
struct SuppressStderr {
    saved_fd: Option<libc::c_int>,
}

#[cfg(target_os = "macos")]
impl SuppressStderr {
    fn new() -> Self {
        unsafe {
            // Save the current stderr fd.
            let saved = libc::dup(2);
            if saved < 0 {
                return Self { saved_fd: None };
            }
            // Open /dev/null and point fd 2 at it.
            let devnull = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY);
            if devnull < 0 {
                libc::close(saved);
                return Self { saved_fd: None };
            }
            if libc::dup2(devnull, 2) < 0 {
                libc::close(saved);
                libc::close(devnull);
                return Self { saved_fd: None };
            }
            libc::close(devnull);
            Self {
                saved_fd: Some(saved),
            }
        }
    }
}

#[cfg(target_os = "macos")]
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

#[cfg(not(target_os = "macos"))]
struct SuppressStderr;

#[cfg(not(target_os = "macos"))]
impl SuppressStderr {
    fn new() -> Self {
        Self
    }
}

/// Write text to the clipboard via the OSC 52 terminal escape sequence.
fn osc52_copy(text: &str) -> Result<(), String> {
    let sequence = osc52_sequence(text, std::env::var_os("TMUX").is_some())?;
    #[cfg(unix)]
    {
        match std::fs::OpenOptions::new().write(true).open("/dev/tty") {
            Ok(tty) => match write_osc52_to_writer(tty, &sequence) {
                Ok(()) => return Ok(()),
                Err(err) => tracing::debug!(
                    "failed to write OSC 52 to /dev/tty: {err}; falling back to stdout"
                ),
            },
            Err(err) => {
                tracing::debug!("failed to open /dev/tty for OSC 52: {err}; falling back to stdout")
            }
        }
    }

    write_osc52_to_writer(std::io::stdout().lock(), &sequence)
}

fn write_osc52_to_writer(mut writer: impl Write, sequence: &str) -> Result<(), String> {
    writer
        .write_all(sequence.as_bytes())
        .map_err(|e| format!("failed to write OSC 52: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("failed to flush OSC 52: {e}"))
}

fn osc52_sequence(text: &str, tmux: bool) -> Result<String, String> {
    let raw_bytes = text.len();
    if raw_bytes > OSC52_MAX_RAW_BYTES {
        return Err(format!(
            "OSC 52 payload too large ({raw_bytes} bytes; max {OSC52_MAX_RAW_BYTES})"
        ));
    }

    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    if tmux {
        Ok(format!("\x1bPtmux;\x1b\x1b]52;c;{encoded}\x07\x1b\\"))
    } else {
        Ok(format!("\x1b]52;c;{encoded}\x07"))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::cell::Cell;

    use super::CopyEnvironment;
    use super::CopyFormat;
    use super::OSC52_MAX_RAW_BYTES;
    use super::copy_to_clipboard_with;
    use super::osc52_sequence;
    use super::write_osc52_to_writer;

    fn remote_environment() -> CopyEnvironment {
        CopyEnvironment {
            ssh_session: true,
            wsl_session: true,
            tmux_session: false,
        }
    }

    fn remote_tmux_environment() -> CopyEnvironment {
        CopyEnvironment {
            tmux_session: true,
            ..remote_environment()
        }
    }

    fn local_environment() -> CopyEnvironment {
        CopyEnvironment {
            ssh_session: false,
            wsl_session: false,
            tmux_session: false,
        }
    }

    fn local_wsl_environment() -> CopyEnvironment {
        CopyEnvironment {
            wsl_session: true,
            ..local_environment()
        }
    }

    #[test]
    fn osc52_encoding_roundtrips() {
        use base64::Engine;
        let text = "# Hello\n\n```rust\nfn main() {}\n```\n";
        let sequence = osc52_sequence(text, /*tmux*/ false).expect("OSC 52 sequence");
        let encoded = sequence
            .trim_start_matches("\u{1b}]52;c;")
            .trim_end_matches('\u{7}');
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        assert_eq!(decoded, text.as_bytes());
    }

    #[test]
    fn osc52_rejects_payload_larger_than_limit() {
        let text = "x".repeat(OSC52_MAX_RAW_BYTES + 1);
        assert_eq!(
            osc52_sequence(&text, /*tmux*/ false),
            Err(format!(
                "OSC 52 payload too large ({} bytes; max {OSC52_MAX_RAW_BYTES})",
                OSC52_MAX_RAW_BYTES + 1
            ))
        );
    }

    #[test]
    fn osc52_wraps_tmux_passthrough() {
        assert_eq!(
            osc52_sequence("hello", /*tmux*/ true),
            Ok("\u{1b}Ptmux;\u{1b}\u{1b}]52;c;aGVsbG8=\u{7}\u{1b}\\".to_string())
        );
    }

    #[test]
    fn write_osc52_to_writer_emits_sequence_verbatim() {
        let sequence = "\u{1b}]52;c;aGVsbG8=\u{7}";
        let mut output = Vec::new();
        assert_eq!(write_osc52_to_writer(&mut output, sequence), Ok(()));
        assert_eq!(output, sequence.as_bytes());
    }

    #[test]
    fn ssh_attempts_both_native_and_osc52() {
        let tmux_calls = Cell::new(0_u8);
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let wsl_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "**hello**",
            CopyFormat::Markdown,
            remote_environment(),
            |_| {
                tmux_calls.set(tmux_calls.get() + 1);
                Ok(())
            },
            |text| {
                assert_eq!(text, "**hello**");
                osc_calls.set(osc_calls.get() + 1);
                Ok(())
            },
            |_, _| {
                native_calls.set(native_calls.get() + 1);
                Ok(None)
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Ok(())
            },
        );

        assert!(matches!(result, Ok(None)));
        assert_eq!(tmux_calls.get(), 0);
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 0);
    }

    #[test]
    fn ssh_reports_failure_when_all_backends_fail() {
        let tmux_calls = Cell::new(0_u8);
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let wsl_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            remote_environment(),
            |_| {
                tmux_calls.set(tmux_calls.get() + 1);
                Ok(())
            },
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Err("blocked".into())
            },
            |_, _| {
                native_calls.set(native_calls.get() + 1);
                Err("native unavailable".into())
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Err("powershell unavailable".into())
            },
        );

        let Err(error) = result else {
            panic!("expected OSC 52 error");
        };
        assert_eq!(
            error,
            "native clipboard: native unavailable; WSL fallback: powershell unavailable; terminal clipboard: blocked"
        );
        assert_eq!(tmux_calls.get(), 0);
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 1);
    }

    #[test]
    fn ssh_inside_tmux_attempts_both_native_and_tmux() {
        let tmux_calls = Cell::new(0_u8);
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let wsl_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            remote_tmux_environment(),
            |_| {
                tmux_calls.set(tmux_calls.get() + 1);
                Ok(())
            },
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Ok(())
            },
            |_, _| {
                native_calls.set(native_calls.get() + 1);
                Ok(None)
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Ok(())
            },
        );

        assert!(matches!(result, Ok(None)));
        assert_eq!(tmux_calls.get(), 1);
        assert_eq!(osc_calls.get(), 0);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 0);
    }

    #[test]
    fn ssh_inside_tmux_falls_back_to_osc52_when_tmux_copy_fails() {
        let tmux_calls = Cell::new(0_u8);
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let wsl_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            remote_tmux_environment(),
            |_| {
                tmux_calls.set(tmux_calls.get() + 1);
                Err("tmux unavailable".into())
            },
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Ok(())
            },
            |_, _| {
                native_calls.set(native_calls.get() + 1);
                Ok(None)
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Ok(())
            },
        );

        assert!(matches!(result, Ok(None)));
        assert_eq!(tmux_calls.get(), 1);
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 0);
    }

    #[test]
    fn ssh_inside_tmux_reports_tmux_and_osc52_errors_when_both_fail() {
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            remote_tmux_environment(),
            |_| Err("tmux unavailable".into()),
            |_| Err("osc blocked".into()),
            |_, _| Err("native unavailable".into()),
            |_| Err("powershell unavailable".into()),
        );

        let Err(error) = result else {
            panic!("expected tmux and OSC 52 errors");
        };
        assert_eq!(
            error,
            "native clipboard: native unavailable; WSL fallback: powershell unavailable; terminal clipboard: tmux clipboard: tmux unavailable; OSC 52 fallback: osc blocked"
        );
        insta::assert_snapshot!("all_copy_backends_fail", error);
    }

    #[test]
    fn local_uses_native_clipboard_first() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let wsl_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            local_wsl_environment(),
            |_| Ok(()),
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Ok(())
            },
            |_, _| {
                native_calls.set(native_calls.get() + 1);
                Ok(Some(super::ClipboardLease::test()))
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Ok(())
            },
        );

        assert!(matches!(result, Ok(Some(_))));
        assert_eq!(osc_calls.get(), 0);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 0);
    }

    #[test]
    fn local_copy_offers_html_only_for_markdown() {
        for (format, html) in [
            (
                CopyFormat::Markdown,
                Some("<p><strong>hello</strong></p>\n"),
            ),
            (CopyFormat::PlainText, None),
        ] {
            let result = copy_to_clipboard_with(
                "**hello**",
                format,
                local_environment(),
                |_| panic!("native copy should succeed"),
                |_| panic!("native copy should succeed"),
                |text, actual_html| {
                    assert_eq!((text, actual_html), ("**hello**", html));
                    Ok(None)
                },
                |_| panic!("native copy should succeed"),
            );
            assert!(matches!(result, Ok(None)));
        }
    }

    #[test]
    fn local_non_wsl_falls_back_to_osc52_when_native_fails() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let wsl_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "**hello**",
            CopyFormat::Markdown,
            local_environment(),
            |_| Ok(()),
            |text| {
                assert_eq!(text, "**hello**");
                osc_calls.set(osc_calls.get() + 1);
                Ok(())
            },
            |_, _| {
                native_calls.set(native_calls.get() + 1);
                Err("native unavailable".into())
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Ok(())
            },
        );

        assert!(matches!(result, Ok(None)));
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 0);
    }

    #[test]
    fn local_wsl_native_failure_uses_powershell_and_skips_osc52_on_success() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let wsl_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            local_wsl_environment(),
            |_| Ok(()),
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Ok(())
            },
            |_, _| {
                native_calls.set(native_calls.get() + 1);
                Err("native unavailable".into())
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Ok(())
            },
        );

        assert!(matches!(result, Ok(None)));
        assert_eq!(osc_calls.get(), 0);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 1);
    }

    #[test]
    fn local_wsl_falls_back_to_osc52_when_native_and_powershell_fail() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let wsl_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            local_wsl_environment(),
            |_| Ok(()),
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Ok(())
            },
            |_, _| {
                native_calls.set(native_calls.get() + 1);
                Err("native unavailable".into())
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Err("powershell unavailable".into())
            },
        );

        assert!(matches!(result, Ok(None)));
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 1);
    }

    #[test]
    fn local_reports_both_errors_when_native_and_osc52_fail() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let wsl_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            local_environment(),
            |_| Ok(()),
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Err("osc blocked".into())
            },
            |_, _| {
                native_calls.set(native_calls.get() + 1);
                Err("native unavailable".into())
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Ok(())
            },
        );

        let Err(error) = result else {
            panic!("expected native and OSC 52 errors");
        };
        assert_eq!(
            error,
            "native clipboard: native unavailable; terminal clipboard: osc blocked"
        );
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 0);
    }

    #[test]
    fn local_wsl_reports_native_powershell_and_osc52_errors_when_all_fail() {
        let osc_calls = Cell::new(0_u8);
        let native_calls = Cell::new(0_u8);
        let wsl_calls = Cell::new(0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            local_wsl_environment(),
            |_| Ok(()),
            |_| {
                osc_calls.set(osc_calls.get() + 1);
                Err("osc blocked".into())
            },
            |_, _| {
                native_calls.set(native_calls.get() + 1);
                Err("native unavailable".into())
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Err("powershell unavailable".into())
            },
        );

        let Err(error) = result else {
            panic!("expected native, WSL, and OSC 52 errors");
        };
        assert_eq!(
            error,
            "native clipboard: native unavailable; WSL fallback: powershell unavailable; terminal clipboard: osc blocked"
        );
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 1);
    }
}

#[cfg(test)]
#[path = "clipboard_copy/routing_tests.rs"]
mod routing_tests;
