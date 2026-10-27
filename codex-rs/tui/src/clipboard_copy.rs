//! Clipboard copy backend for the TUI's `/copy` command and `Ctrl+O` hotkey.
//!
//! Local copying uses the native clipboard, or the Windows clipboard directly on WSL.
//! WSL sessions over SSH with a forwarded X11 display use the native clipboard instead.
//! In tmux and Herdr, also forward to the attached terminal so clients attached after Codex
//! started receive the copy. Over SSH without a multiplexer, send OSC 52 directly.
//!
//! Terminal writes have no delivery acknowledgement: a successful send must not
//! suppress native copying. Both the host and attached client's clipboards may
//! change. Outside tmux, Herdr, and SSH, use OSC 52 only if native copying fails.
//!
//! On Linux, X11 and some Wayland compositors require the process that wrote the
//! clipboard to keep its handle open. `ClipboardLease` wraps the `arboard::Clipboard`
//! so the copy worker can retain it for the lifetime of the TUI. On other platforms the lease
//! is always `None`.
//!
//! Empty selections fail without modifying a clipboard.
//! Markdown copies also offer HTML on the native clipboard. Terminal and WSL
//! fallbacks retain the original text. Terminal writes are unacknowledged requests,
//! so callers must distinguish them from confirmed native clipboard writes.
//! Image paste lives in `clipboard_paste`.

pub(crate) mod primary;
mod tmux;
pub(crate) mod worker;
mod wsl;

use base64::Engine;
use std::io::Write;
use std::sync::Arc;
use tmux::copy as tmux_clipboard_copy;

/// Maximum raw bytes sent through tmux or directly encoded into OSC 52.
/// Large payloads are rejected before encoding to avoid overwhelming the terminal.
const OSC52_MAX_RAW_BYTES: usize = 100_000;
/// Whether copied text should also have a rendered HTML representation and its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CopyFormat {
    PlainText,
    Markdown,
    /// Visible selection text is copied literally; only HTML uses the semantic Markdown.
    MarkdownSelection(Arc<str>),
}

/// A native clipboard write or an unacknowledged request to the user's terminal.
pub(crate) enum CopyOutcome {
    Copied(Option<ClipboardLease>),
    Requested,
}

impl CopyOutcome {
    /// Replace native ownership only when a backend supplies a new lease.
    pub(crate) fn store(self, lease: &mut Option<ClipboardLease>) -> CopyStatus {
        match self {
            Self::Copied(next_lease) => {
                if let Some(next_lease) = next_lease {
                    *lease = Some(next_lease);
                }
                CopyStatus::Confirmed
            }
            Self::Requested => CopyStatus::Unconfirmed,
        }
    }
}

/// Submission progress or the delivery status reported by a clipboard backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CopyStatus {
    Confirmed,
    Unconfirmed,
    Pending(u64),
    Busy,
}

impl CopyStatus {
    pub(crate) fn message(self, label: &str) -> String {
        match self {
            Self::Confirmed => format!("Copied {label} to clipboard"),
            Self::Unconfirmed => "Copy unconfirmed; /export saves chat".to_string(),
            Self::Pending(_) => format!("Copying {label}…"),
            Self::Busy => "Copy already in progress".to_string(),
        }
    }
}

/// Copy text to the system clipboard.
///
/// Try native copying, then independently attempt terminal forwarding in tmux,
/// Herdr, or SSH. A terminal send is best effort and does not confirm clipboard delivery.
/// Terminal forwarding may replace native HTML with plain text.
///
/// Native or WSL success returns `Copied`, even if terminal forwarding fails.
/// A terminal-only send returns `Requested`. Store the outcome to retain any
/// existing native lease unless the backend supplies a replacement.
///
/// OSC 52 is supported by kitty, WezTerm, iTerm2, Ghostty, and others.
fn copy_to_clipboard(
    text: &str,
    format: CopyFormat,
    begin_delivery: impl Fn() -> Result<(), String>,
    osc52: impl Fn(&str) -> Result<(), String>,
) -> Result<CopyOutcome, String> {
    let wsl_deadline = std::time::Instant::now() + std::time::Duration::from_secs(/*secs*/ 5);
    let ssh_session = is_ssh_session();
    let environment = CopyEnvironment {
        herdr_session: std::env::var("HERDR_ENV").is_ok_and(|value| value == "1"),
        ssh_session,
        forwarded_x11: ssh_session
            && std::env::var("DISPLAY").is_ok_and(|display| !display.is_empty()),
        wsl_session: is_wsl_session(),
        tmux_session: is_tmux_session(),
    };
    #[cfg(not(target_os = "android"))]
    let native_copy = {
        // Bound WSL native setup: even initialization can block forever.
        let clipboard = (!environment.wsl_session).then(arboard::Clipboard::new);
        move |text: &str, html: Option<&str>| {
            let clipboard = match clipboard {
                Some(clipboard) => clipboard.map_err(|e| format!("clipboard unavailable: {e}"))?,
                None => wsl::native_clipboard()?,
            };
            arboard_copy(clipboard, text, html)
        }
    };
    #[cfg(target_os = "android")]
    let native_copy = |_text: &str, _html: Option<&str>| {
        Err("native clipboard unavailable on Android".to_string())
    };
    // Decide before propagating setup success or failure: either result could otherwise
    // start a native write or fallback long after the user abandoned this request.
    begin_delivery()?;
    copy_to_clipboard_with(
        text,
        format,
        environment,
        tmux_clipboard_copy,
        osc52,
        native_copy,
        |text| wsl::copy(text, wsl_deadline),
    )
}

/// Keeps a platform clipboard owner alive when the backend requires one.
///
/// On Linux/X11 and some Wayland compositors, clipboard contents are served by the
/// owning process. Dropping the `arboard::Clipboard` before the user pastes causes
/// the content to vanish. Store this lease on a session-lived owner so closing a
/// transient overlay does not release the clipboard contents. On non-Linux native paths and OSC 52
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
    herdr_session: bool,
    ssh_session: bool,
    forwarded_x11: bool,
    wsl_session: bool,
    tmux_session: bool,
}

fn copy_to_clipboard_with(
    text: &str,
    format: CopyFormat,
    environment: CopyEnvironment,
    tmux_copy_fn: impl Fn(&str) -> Result<(), String>,
    osc52_copy_fn: impl Fn(&str) -> Result<(), String>,
    arboard_copy_fn: impl FnOnce(&str, Option<&str>) -> Result<Option<ClipboardLease>, String>,
    wsl_copy_fn: impl Fn(&str) -> Result<(), String>,
) -> Result<CopyOutcome, String> {
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
    let native_copy = || {
        let html = match format {
            CopyFormat::PlainText => None,
            CopyFormat::Markdown => Some(crate::clipboard_html::render_markdown(text)),
            CopyFormat::MarkdownSelection(source) => {
                Some(crate::clipboard_html::render_markdown(&source))
            }
        };
        arboard_copy_fn(text, html.as_deref()).map_err(|error| format!("native clipboard: {error}"))
    };
    let native_result = if environment.wsl_session && !environment.forwarded_x11 {
        wsl_copy_fn(text).map(|()| None).or_else(|windows_error| {
            native_copy().map_err(|native_error| {
                format!("Windows clipboard: {windows_error}; {native_error}")
            })
        })
    } else {
        native_copy()
    };
    // Copy natively first: an X11 SelectionClear from a terminal write can otherwise
    // race with arboard reusing its ownership window and clear the new native data.
    // Persistent multiplexer sessions may gain remote clients after Codex starts, so still
    // forward even when no SSH variables were inherited or native copying succeeded.
    let terminal_result =
        (environment.tmux_session || environment.herdr_session || environment.ssh_session)
            .then(terminal_copy);
    match native_result {
        Ok(lease) => Ok(CopyOutcome::Copied(lease)),
        Err(native_error) => terminal_result
            .unwrap_or_else(terminal_copy)
            .map(|()| CopyOutcome::Requested)
            .map_err(|terminal_error| {
                format!("{native_error}; terminal clipboard: {terminal_error}")
            }),
    }
}

/// Detect whether the current process is running inside an SSH session.
pub(crate) fn is_ssh_session() -> bool {
    std::env::var_os("SSH_TTY").is_some() || std::env::var_os("SSH_CONNECTION").is_some()
}

/// Detect whether the current process is running inside tmux.
fn is_tmux_session() -> bool {
    std::env::var_os("TMUX").is_some() || std::env::var_os("TMUX_PANE").is_some()
}

#[cfg(target_os = "linux")]
pub(crate) fn is_wsl_session() -> bool {
    crate::clipboard_paste::is_probably_wsl()
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn is_wsl_session() -> bool {
    false
}

/// Write to the native clipboard. The TUI owns any process-wide stderr redirection.
#[cfg(not(target_os = "android"))]
fn arboard_copy(
    mut clipboard: arboard::Clipboard,
    text: &str,
    html: Option<&str>,
) -> Result<Option<ClipboardLease>, String> {
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
    use super::CopyOutcome;
    use super::OSC52_MAX_RAW_BYTES;
    use super::copy_to_clipboard_with;
    use super::osc52_sequence;
    use super::write_osc52_to_writer;

    fn remote_environment() -> CopyEnvironment {
        CopyEnvironment {
            herdr_session: false,
            ssh_session: true,
            forwarded_x11: false,
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
            herdr_session: false,
            ssh_session: false,
            forwarded_x11: false,
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
    fn ssh_on_wsl_with_forwarded_x11_uses_native_clipboard() {
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            CopyEnvironment {
                forwarded_x11: true,
                ..remote_tmux_environment()
            },
            |_| Ok(()),
            |_| Ok(()),
            |_, _| Ok(Some(super::ClipboardLease::test())),
            |_| panic!("forwarded X11 must not write to the Windows host clipboard"),
        );

        assert!(matches!(result, Ok(CopyOutcome::Copied(Some(_)))));
    }

    #[test]
    fn ssh_on_wsl_attempts_windows_and_osc52() {
        let tmux_calls = Cell::new(/*value*/ 0_u8);
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
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

        assert!(matches!(result, Ok(CopyOutcome::Copied(None))));
        assert_eq!(tmux_calls.get(), 0);
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 0);
        assert_eq!(wsl_calls.get(), 1);
    }

    #[test]
    fn ssh_reports_failure_when_all_backends_fail() {
        let tmux_calls = Cell::new(/*value*/ 0_u8);
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
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
            "Windows clipboard: powershell unavailable; native clipboard: native unavailable; terminal clipboard: blocked"
        );
        assert_eq!(tmux_calls.get(), 0);
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 1);
    }

    #[test]
    fn ssh_on_wsl_inside_tmux_attempts_windows_and_tmux() {
        let tmux_calls = Cell::new(/*value*/ 0_u8);
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
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

        assert!(matches!(result, Ok(CopyOutcome::Copied(None))));
        assert_eq!(tmux_calls.get(), 1);
        assert_eq!(osc_calls.get(), 0);
        assert_eq!(native_calls.get(), 0);
        assert_eq!(wsl_calls.get(), 1);
    }

    #[test]
    fn ssh_inside_tmux_falls_back_to_osc52_when_tmux_copy_fails() {
        let tmux_calls = Cell::new(/*value*/ 0_u8);
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
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

        assert!(matches!(result, Ok(CopyOutcome::Copied(None))));
        assert_eq!(tmux_calls.get(), 1);
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 0);
        assert_eq!(wsl_calls.get(), 1);
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
        insta::assert_snapshot!("all_copy_backends_fail", error);
    }

    #[test]
    fn local_uses_native_clipboard_first() {
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            local_environment(),
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

        assert!(matches!(result, Ok(CopyOutcome::Copied(Some(_)))));
        assert_eq!(osc_calls.get(), 0);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 0);
    }

    #[test]
    fn local_copy_offers_html_only_for_markdown() {
        for (text, format, html) in [
            (
                "**hello**",
                CopyFormat::Markdown,
                Some("<p><strong>hello</strong></p>\n"),
            ),
            ("**hello**", CopyFormat::PlainText, None),
            (
                "hello /repo/my_notes!.txt",
                CopyFormat::MarkdownSelection(r"**hello** /repo/my\_notes\!.txt".into()),
                Some("<p><strong>hello</strong> /repo/my_notes!.txt</p>\n"),
            ),
        ] {
            for environment in [local_environment(), local_wsl_environment()] {
                let result = copy_to_clipboard_with(
                    text,
                    format.clone(),
                    environment,
                    |_| panic!("native copy should succeed"),
                    |_| panic!("native copy should succeed"),
                    |actual_text, actual_html| {
                        assert_eq!((actual_text, actual_html), (text, html));
                        Ok(Some(super::ClipboardLease::test()))
                    },
                    |_| Err("Windows clipboard unavailable".into()),
                );
                assert!(matches!(result, Ok(CopyOutcome::Copied(Some(_)))));
            }
        }
    }

    #[test]
    fn local_non_wsl_falls_back_to_osc52_when_native_fails() {
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
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

        assert!(matches!(result, Ok(CopyOutcome::Requested)));
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 0);
    }

    #[test]
    fn local_tmux_fallback_prefers_tmux_when_native_fails() {
        let tmux_calls = Cell::new(/*value*/ 0_u8);
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
        let result = copy_to_clipboard_with(
            "hello",
            CopyFormat::PlainText,
            CopyEnvironment {
                tmux_session: true,
                ..local_environment()
            },
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
                Err("native unavailable".into())
            },
            |_| {
                wsl_calls.set(wsl_calls.get() + 1);
                Ok(())
            },
        );

        assert!(matches!(result, Ok(CopyOutcome::Requested)));
        assert_eq!(tmux_calls.get(), 1);
        assert_eq!(osc_calls.get(), 0);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 0);
    }

    #[test]
    fn local_wsl_uses_windows_and_skips_osc52_on_success() {
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
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

        assert!(matches!(result, Ok(CopyOutcome::Copied(None))));
        assert_eq!(osc_calls.get(), 0);
        assert_eq!(native_calls.get(), 0);
        assert_eq!(wsl_calls.get(), 1);
    }

    #[test]
    fn local_wsl_falls_back_to_osc52_when_powershell_fails() {
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
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

        assert!(matches!(result, Ok(CopyOutcome::Requested)));
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 1);
    }

    #[test]
    fn local_reports_both_errors_when_native_and_osc52_fail() {
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
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
    fn local_wsl_reports_windows_and_osc52_errors_when_all_fail() {
        let osc_calls = Cell::new(/*value*/ 0_u8);
        let native_calls = Cell::new(/*value*/ 0_u8);
        let wsl_calls = Cell::new(/*value*/ 0_u8);
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
            panic!("expected Windows and OSC 52 errors");
        };
        assert_eq!(
            error,
            "Windows clipboard: powershell unavailable; native clipboard: native unavailable; terminal clipboard: osc blocked"
        );
        assert_eq!(osc_calls.get(), 1);
        assert_eq!(native_calls.get(), 1);
        assert_eq!(wsl_calls.get(), 1);
    }
}

#[cfg(test)]
#[path = "clipboard_copy/routing_tests.rs"]
mod routing_tests;
