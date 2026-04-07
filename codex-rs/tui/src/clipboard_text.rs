//! Clipboard text copy support for `/copy` in the TUI.
//!
//! This module owns the policy for getting plain text from the running Codex
//! process into the user's system clipboard. It prefers the direct native
//! clipboard path when the current machine is also the user's desktop, but it
//! intentionally changes strategy in environments where a "local" clipboard
//! would be the wrong one: SSH sessions use OSC 52 so the user's terminal can
//! proxy the copy back to the client, and WSL shells fall back to
//! `powershell.exe` because Linux-side clipboard providers often cannot reach
//! the Windows clipboard reliably.
//!
//! The module is deliberately narrow. It only handles text copy, returns
//! user-facing error strings for the chat UI, and does not try to expose a
//! reusable clipboard abstraction for the rest of the application. Image paste
//! and WSL environment detection live in neighboring modules.
//!
//! The main operational contract is that callers get one best-effort copy
//! attempt and a readable failure message. The selection between native copy,
//! OSC 52, and WSL fallback is centralized here so `/copy` does not have to
//! understand platform-specific clipboard behavior.

#[cfg(not(target_os = "android"))]
use base64::Engine as _;
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

/// Copies user-visible text into the most appropriate clipboard for the
/// current environment.
///
/// In a normal desktop session this targets the host clipboard through
/// `arboard`. In SSH sessions it emits an OSC 52 sequence instead, because the
/// process-local clipboard would belong to the remote machine rather than the
/// user's terminal. On Linux under WSL, a failed native copy falls back to
/// `powershell.exe` so the Windows clipboard still works when Linux clipboard
/// integrations are unavailable.
///
/// The returned error is intended for display in the TUI rather than for
/// programmatic branching. Callers should treat it as user-facing text. A
/// caller that assumes a specific substring means a stable failure category
/// will be brittle if the fallback policy or wording changes later.
///
/// # Errors
///
/// Returns a descriptive error string when the selected clipboard mechanism is
/// unavailable or the fallback path also fails.
#[cfg(not(target_os = "android"))]
pub fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    if std::env::var_os("SSH_CONNECTION").is_some() || std::env::var_os("SSH_TTY").is_some() {
        return copy_via_osc52(text);
    }

    let error = match arboard::Clipboard::new() {
        Ok(mut clipboard) => match clipboard.set_text(text.to_string()) {
            Ok(()) => return Ok(()),
            Err(err) => format!("clipboard unavailable: {err}"),
        },
        Err(err) => format!("clipboard unavailable: {err}"),
    };

    #[cfg(target_os = "linux")]
    let error = if is_probably_wsl() {
        match copy_via_wsl_clipboard(text) {
            Ok(()) => return Ok(()),
            Err(wsl_err) => format!("{error}; WSL fallback failed: {wsl_err}"),
        }
    } else {
        error
    };

    Err(error)
}

/// Writes text through OSC 52 so the controlling terminal can own the copy.
///
/// This path exists for remote sessions where the process-local clipboard is
/// not the clipboard the user actually wants. On Unix it writes directly to the
/// controlling TTY so the escape sequence reaches the terminal even if stdout
/// is redirected; on Windows it writes to stdout because the console is the
/// transport.
#[cfg(not(target_os = "android"))]
fn copy_via_osc52(text: &str) -> Result<(), String> {
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

/// Copies text into the Windows clipboard from a WSL process.
///
/// This is a Linux-only fallback for the case where `arboard` cannot talk to
/// the Windows clipboard from inside WSL. It shells out to `powershell.exe`,
/// streams the text over stdin as UTF-8, and waits for the process to report
/// success before returning to the caller.
#[cfg(all(not(target_os = "android"), target_os = "linux"))]
fn copy_via_wsl_clipboard(text: &str) -> Result<(), String> {
    let mut child = std::process::Command::new("powershell.exe")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .args([
            "-NoProfile",
            "-Command",
            "[Console]::InputEncoding = [System.Text.Encoding]::UTF8; $ErrorActionPreference = 'Stop'; $text = [Console]::In.ReadToEnd(); Set-Clipboard -Value $text",
        ])
        .spawn()
        .map_err(|e| format!("clipboard unavailable: failed to spawn powershell.exe: {e}"))?;

    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("clipboard unavailable: failed to open powershell.exe stdin".to_string());
    };

    if let Err(err) = stdin.write_all(text.as_bytes()) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "clipboard unavailable: failed to write to powershell.exe: {err}"
        ));
    }

    drop(stdin);

    let output = child
        .wait_with_output()
        .map_err(|e| format!("clipboard unavailable: failed to wait for powershell.exe: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            let status = output.status;
            Err(format!(
                "clipboard unavailable: powershell.exe exited with status {status}"
            ))
        } else {
            Err(format!(
                "clipboard unavailable: powershell.exe failed: {stderr}"
            ))
        }
    }
}

/// Encodes text as an OSC 52 clipboard sequence.
///
/// When `tmux` is true the sequence is wrapped in the tmux passthrough form so
/// nested terminals still receive the clipboard escape.
#[cfg(not(target_os = "android"))]
fn osc52_sequence(text: &str, tmux: bool) -> String {
    let payload = base64::engine::general_purpose::STANDARD.encode(text);
    if tmux {
        format!("\x1bPtmux;\x1b\x1b]52;c;{payload}\x07\x1b\\")
    } else {
        format!("\x1b]52;c;{payload}\x07")
    }
}

/// Reports that clipboard text copy is unavailable on Android builds.
///
/// The TUI's clipboard implementation depends on host integrations that are not
/// available in the supported Android/Termux environment.
#[cfg(target_os = "android")]
pub fn copy_text_to_clipboard(_text: &str) -> Result<(), String> {
    Err("clipboard text copy is unsupported on Android".into())
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn osc52_sequence_encodes_text_for_terminal_clipboard() {
        assert_eq!(
            osc52_sequence("hello", /*tmux*/ false),
            "\u{1b}]52;c;aGVsbG8=\u{7}"
        );
    }

    #[test]
    fn osc52_sequence_wraps_tmux_passthrough() {
        assert_eq!(
            osc52_sequence("hello", /*tmux*/ true),
            "\u{1b}Ptmux;\u{1b}\u{1b}]52;c;aGVsbG8=\u{7}\u{1b}\\"
        );
    }
}
