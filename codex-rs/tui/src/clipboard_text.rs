#[cfg(not(target_os = "android"))]
use base64::Engine as _;
#[cfg(not(target_os = "android"))]
use std::ffi::OsStr;
#[cfg(all(not(target_os = "android"), unix))]
use std::fs::OpenOptions;
#[cfg(not(target_os = "android"))]
use std::io::Write;
#[cfg(all(not(target_os = "android"), windows))]
use std::io::stdout;
#[cfg(all(not(target_os = "android"), target_os = "linux"))]
use std::process::Stdio;

#[cfg(all(not(target_os = "android"), target_os = "linux"))]
use crate::clipboard_paste::is_probably_wsl;

#[cfg(not(target_os = "android"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardCopyPath {
    Osc52,
    #[cfg(target_os = "linux")]
    WslClip,
    Native,
}

#[cfg(not(target_os = "android"))]
pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    match clipboard_copy_path(
        std::env::var_os("SSH_CONNECTION").as_deref(),
        std::env::var_os("SSH_TTY").as_deref(),
        wsl_clipboard_supported(),
    ) {
        ClipboardCopyPath::Osc52 => {
            let sequence = osc52_sequence(text, std::env::var_os("TMUX").is_some());
            #[cfg(unix)]
            let mut tty = OpenOptions::new()
                .write(true)
                .open("/dev/tty")
                .map_err(|e| {
                    format!("clipboard unavailable: failed to open /dev/tty for OSC 52 copy: {e}")
                })?;
            #[cfg(unix)]
            tty.write_all(sequence.as_bytes()).map_err(|e| {
                format!("clipboard unavailable: failed to write OSC 52 escape sequence: {e}")
            })?;
            #[cfg(unix)]
            tty.flush().map_err(|e| {
                format!("clipboard unavailable: failed to flush OSC 52 escape sequence: {e}")
            })?;
            #[cfg(windows)]
            stdout().write_all(sequence.as_bytes()).map_err(|e| {
                format!("clipboard unavailable: failed to write OSC 52 escape sequence: {e}")
            })?;
            #[cfg(windows)]
            stdout().flush().map_err(|e| {
                format!("clipboard unavailable: failed to flush OSC 52 escape sequence: {e}")
            })?;
            Ok(())
        }
        #[cfg(target_os = "linux")]
        ClipboardCopyPath::WslClip => {
            let mut child = std::process::Command::new("clip.exe")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("clipboard unavailable: failed to spawn clip.exe: {e}"))?;

            child
                .stdin
                .take()
                .ok_or_else(|| "clipboard unavailable: failed to open clip.exe stdin".to_string())?
                .write_all(text.as_bytes())
                .map_err(|e| format!("clipboard unavailable: failed to write to clip.exe: {e}"))?;

            let output = child
                .wait_with_output()
                .map_err(|e| format!("clipboard unavailable: failed to wait for clip.exe: {e}"))?;

            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    Err(format!(
                        "clipboard unavailable: clip.exe exited with status {}",
                        output.status
                    ))
                } else {
                    Err(format!("clipboard unavailable: clip.exe failed: {stderr}"))
                }
            }
        }
        ClipboardCopyPath::Native => {
            let mut cb =
                arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;
            cb.set_text(text.to_string())
                .map_err(|e| format!("clipboard unavailable: {e}"))
        }
    }
}

#[cfg(not(target_os = "android"))]
fn is_ssh_session(ssh_connection: Option<&OsStr>, ssh_tty: Option<&OsStr>) -> bool {
    ssh_connection.is_some() || ssh_tty.is_some()
}

#[cfg(all(not(target_os = "android"), target_os = "linux"))]
fn clipboard_copy_path(
    ssh_connection: Option<&OsStr>,
    ssh_tty: Option<&OsStr>,
    wsl_clipboard_supported: bool,
) -> ClipboardCopyPath {
    if is_ssh_session(ssh_connection, ssh_tty) {
        ClipboardCopyPath::Osc52
    } else if wsl_clipboard_supported {
        ClipboardCopyPath::WslClip
    } else {
        ClipboardCopyPath::Native
    }
}

#[cfg(all(not(target_os = "android"), not(target_os = "linux")))]
fn clipboard_copy_path(
    ssh_connection: Option<&OsStr>,
    ssh_tty: Option<&OsStr>,
    _wsl_clipboard_supported: bool,
) -> ClipboardCopyPath {
    if is_ssh_session(ssh_connection, ssh_tty) {
        ClipboardCopyPath::Osc52
    } else {
        ClipboardCopyPath::Native
    }
}

#[cfg(not(target_os = "android"))]
fn osc52_sequence(text: &str, tmux: bool) -> String {
    let payload = base64::engine::general_purpose::STANDARD.encode(text);
    if tmux {
        format!("\x1bPtmux;\x1b\x1b]52;c;{payload}\x07\x1b\\")
    } else {
        format!("\x1b]52;c;{payload}\x07")
    }
}

#[cfg(all(not(target_os = "android"), target_os = "linux"))]
fn wsl_clipboard_supported() -> bool {
    is_probably_wsl()
}

#[cfg(all(not(target_os = "android"), not(target_os = "linux")))]
fn wsl_clipboard_supported() -> bool {
    false
}

#[cfg(target_os = "android")]
pub fn copy_text_to_clipboard(_text: &str) -> Result<(), String> {
    Err("clipboard text copy is unsupported on Android".into())
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn detects_ssh_session_when_connection_is_present() {
        assert!(is_ssh_session(Some(OsStr::new("1")), None));
    }

    #[test]
    fn detects_ssh_session_when_tty_is_present() {
        assert!(is_ssh_session(None, Some(OsStr::new("/dev/pts/1"))));
    }

    #[test]
    fn does_not_detect_ssh_session_without_ssh_environment() {
        assert!(!is_ssh_session(None, None));
    }

    #[test]
    fn prefers_osc52_over_wsl_clipboard() {
        assert_eq!(
            clipboard_copy_path(Some(OsStr::new("1")), None, true),
            ClipboardCopyPath::Osc52
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn prefers_wsl_clipboard_when_not_over_ssh() {
        assert_eq!(
            clipboard_copy_path(None, None, true),
            ClipboardCopyPath::WslClip
        );
    }

    #[test]
    fn defaults_to_native_clipboard_without_ssh_or_wsl() {
        assert_eq!(
            clipboard_copy_path(None, None, false),
            ClipboardCopyPath::Native
        );
    }

    #[test]
    fn osc52_sequence_encodes_text_for_terminal_clipboard() {
        assert_eq!(osc52_sequence("hello", false), "\u{1b}]52;c;aGVsbG8=\u{7}");
    }

    #[test]
    fn osc52_sequence_wraps_tmux_passthrough() {
        assert_eq!(
            osc52_sequence("hello", true),
            "\u{1b}Ptmux;\u{1b}\u{1b}]52;c;aGVsbG8=\u{7}\u{1b}\\"
        );
    }
}
