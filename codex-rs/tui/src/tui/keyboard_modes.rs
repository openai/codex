//! Terminal keyboard enhancement setup and teardown helpers.
//!
//! The TUI uses crossterm's keyboard enhancement stack while it owns the terminal, but
//! process exit gets a stronger reset so the parent shell does not inherit enhanced key
//! reporting if a terminal misses the normal stack pop.
//! Windows terminal detection has a deadline, including WSL interop process launch.
//! Inconclusive detection keeps keyboard enhancements disabled on WSL.

use std::fmt;
use std::io::stdout;

use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use crossterm::Command;
use crossterm::event::KeyboardEnhancementFlags;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::event::PushKeyboardEnhancementFlags;
use ratatui::crossterm::execute;

const DISABLE_KEYBOARD_ENHANCEMENT_ENV_VAR: &str = "CODEX_TUI_DISABLE_KEYBOARD_ENHANCEMENT";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VscodeDetection {
    VsCode,
    Other,
    Unknown,
}

pub(super) fn keyboard_enhancement_disabled() -> bool {
    let disable_env = std::env::var(DISABLE_KEYBOARD_ENHANCEMENT_ENV_VAR).ok();
    let is_wsl = running_in_wsl();
    let vscode_detection = if is_wsl {
        detect_vscode_terminal()
    } else {
        VscodeDetection::Other
    };
    keyboard_enhancement_disabled_for(disable_env.as_deref(), is_wsl, vscode_detection)
}

fn keyboard_enhancement_disabled_for(
    disable_env: Option<&str>,
    is_wsl: bool,
    vscode_detection: VscodeDetection,
) -> bool {
    if let Some(disabled) = parse_bool_env(disable_env) {
        return disabled;
    }

    // VS Code running a WSL shell can hide TERM_PROGRAM from the Linux process
    // environment. If the Windows-side probe is inconclusive, avoid enabling the
    // keyboard mode that can break dead-key composition in VS Code on WSL.
    is_wsl
        && matches!(
            vscode_detection,
            VscodeDetection::VsCode | VscodeDetection::Unknown
        )
}

fn parse_bool_env(value: Option<&str>) -> Option<bool> {
    match value.map(str::trim) {
        Some("1") => Some(true),
        Some(value) if value.eq_ignore_ascii_case("true") => Some(true),
        Some(value) if value.eq_ignore_ascii_case("yes") => Some(true),
        Some("0") => Some(false),
        Some(value) if value.eq_ignore_ascii_case("false") => Some(false),
        Some(value) if value.eq_ignore_ascii_case("no") => Some(false),
        _ => None,
    }
}

fn running_in_wsl() -> bool {
    #[cfg(target_os = "linux")]
    {
        crate::clipboard_paste::is_probably_wsl()
    }

    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

pub(super) fn running_in_vscode_terminal() -> bool {
    detect_vscode_terminal() == VscodeDetection::VsCode
}

fn detect_vscode_terminal() -> VscodeDetection {
    if term_program_is_vscode(std::env::var("TERM_PROGRAM").ok().as_deref()) {
        return VscodeDetection::VsCode;
    }
    vscode_terminal_detected(
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        windows_vscode_detection(),
    )
}

fn vscode_terminal_detected(
    linux_term_program: Option<&str>,
    windows_detection: VscodeDetection,
) -> VscodeDetection {
    if term_program_is_vscode(linux_term_program) {
        VscodeDetection::VsCode
    } else {
        windows_detection
    }
}

fn term_program_is_vscode(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.eq_ignore_ascii_case("vscode"))
}

fn windows_vscode_detection() -> VscodeDetection {
    #[cfg(target_os = "linux")]
    {
        static WINDOWS_VSCODE_DETECTION: std::sync::OnceLock<VscodeDetection> =
            std::sync::OnceLock::new();
        *WINDOWS_VSCODE_DETECTION.get_or_init(read_windows_vscode_detection)
    }

    #[cfg(not(target_os = "linux"))]
    {
        VscodeDetection::Unknown
    }
}

#[cfg(target_os = "linux")]
fn read_windows_vscode_detection() -> VscodeDetection {
    if !running_in_wsl() {
        return VscodeDetection::Other;
    }
    read_windows_vscode_detection_with_timeout(
        || {
            let executable = codex_utils_path::system_executable("cmd.exe")?;
            std::process::Command::new(executable)
                .args(["/d", "/s", "/c", "set TERM_PROGRAM"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok()
        },
        std::time::Duration::from_secs(/*secs*/ 1),
    )
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn read_windows_vscode_detection_with_timeout(
    spawn: impl FnOnce() -> Option<std::process::Child> + Send + 'static,
    timeout: std::time::Duration,
) -> VscodeDetection {
    let deadline = std::time::Instant::now() + timeout;
    let (sender, receiver) = std::sync::mpsc::channel();
    // WSL interop can block inside spawn itself. Keep launch and cleanup off the
    // startup thread; the OnceLock caches a timeout so this worker is not retried.
    let worker = std::thread::Builder::new()
        .name("windows-term-program".into())
        .spawn(move || {
            let output = (|| {
                let mut child = spawn()?;
                while std::time::Instant::now() < deadline {
                    match child.try_wait() {
                        Ok(Some(_)) => return child.wait_with_output().ok(),
                        Ok(None) => {
                            std::thread::sleep(std::time::Duration::from_millis(/*millis*/ 10))
                        }
                        Err(_) => break,
                    }
                }
                let _ = child.kill();
                let _ = child.wait();
                None
            })();
            let _ = sender.send(output);
        });
    if worker.is_err() {
        return VscodeDetection::Unknown;
    }

    let Ok(Some(output)) =
        receiver.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
    else {
        return VscodeDetection::Unknown;
    };
    if !output.status.success() {
        // `cmd.exe /c set TERM_PROGRAM` exits with 1 when the variable is absent.
        return if output.status.code() == Some(1) {
            VscodeDetection::Other
        } else {
            VscodeDetection::Unknown
        };
    }

    let output = String::from_utf8_lossy(&output.stdout);
    let term_program = output
        .lines()
        .find_map(|line| line.trim_end_matches('\r').strip_prefix("TERM_PROGRAM="))
        .filter(|value| !value.trim().is_empty());
    if term_program_is_vscode(term_program) {
        VscodeDetection::VsCode
    } else {
        VscodeDetection::Other
    }
}

#[cfg(all(test, unix))]
#[path = "windows_term_program_tests.rs"]
mod windows_term_program_tests;

pub(super) fn enable_keyboard_enhancement() {
    if keyboard_enhancement_disabled() {
        return;
    }

    let running_in_tmux_session = running_in_tmux_session();
    let tmux_extended_keys_format = if running_in_tmux_session {
        read_tmux_extended_keys_format()
    } else {
        None
    };

    let _ = execute!(
        stdout(),
        DisableModifyOtherKeys,
        PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
            terminal_info().name,
            running_in_tmux_session,
            tmux_extended_keys_format.as_deref()
        ))
    );

    if tmux_should_enable_modify_other_keys_for(
        running_in_tmux_session,
        tmux_extended_keys_format.as_deref(),
    ) {
        let _ = execute!(stdout(), EnableModifyOtherKeys);
    }
}

fn keyboard_enhancement_flags(
    terminal_name: TerminalName,
    running_in_tmux_session: bool,
    tmux_extended_keys_format: Option<&str>,
) -> KeyboardEnhancementFlags {
    let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS;

    // iTerm and Ghostty can leak shortcut release events that the terminal consumes.
    // tmux's xterm key format also loses Shift-Enter when event types are
    // reported. An unavailable/unrecognized tmux probe must take the same safe
    // fallback. Preserve repeat classification on confirmed csi-u transports.
    if matches!(terminal_name, TerminalName::Ghostty | TerminalName::Iterm2)
        || (running_in_tmux_session && !matches!(tmux_extended_keys_format, Some("csi-u")))
    {
        flags
    } else {
        flags | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
    }
}

fn running_in_tmux_session() -> bool {
    tmux_session_detected(
        std::env::var("TMUX").ok().as_deref(),
        std::env::var("TMUX_PANE").ok().as_deref(),
    )
}

fn tmux_session_detected(tmux: Option<&str>, tmux_pane: Option<&str>) -> bool {
    tmux.is_some() || tmux_pane.is_some()
}

fn tmux_should_enable_modify_other_keys_for(
    running_in_tmux_session: bool,
    extended_keys_format: Option<&str>,
) -> bool {
    // Only request mode 2 when tmux confirms csi-u formatting. Older tmux
    // versions do not expose this option and may emit xterm-style sequences,
    // which crossterm does not parse consistently for modified keys.
    running_in_tmux_session && matches!(extended_keys_format, Some("csi-u"))
}

fn read_tmux_extended_keys_format() -> Option<String> {
    let executable = codex_utils_path::system_executable("tmux")?;
    let path = codex_utils_path::system_path().ok()?;
    for args in [
        ["display-message", "-p", "#{extended-keys-format}"],
        ["show-options", "-gqv", "extended-keys-format"],
    ] {
        let output = std::process::Command::new(&executable)
            .env("PATH", &path)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;

        if !output.status.success() {
            continue;
        }

        if let Some(value) = String::from_utf8(output.stdout)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Some(value);
        }
    }

    None
}

pub(super) fn restore_keyboard_enhancement_stack() {
    let _ = execute!(
        stdout(),
        PopKeyboardEnhancementFlags,
        DisableModifyOtherKeys
    );
}

pub(super) fn reset_keyboard_reporting_after_exit() {
    let _ = execute!(
        stdout(),
        PopKeyboardEnhancementFlags,
        ResetKeyboardEnhancementFlags,
        DisableModifyOtherKeys
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResetKeyboardEnhancementFlags;

impl Command for ResetKeyboardEnhancementFlags {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("\x1b[<u")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "keyboard enhancement reset is not implemented for the legacy Windows API",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnableModifyOtherKeys;

impl Command for EnableModifyOtherKeys {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("\x1b[>4;2m")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "modifyOtherKeys enable is not implemented for the legacy Windows API",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisableModifyOtherKeys;

impl Command for DisableModifyOtherKeys {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("\x1b[>4;0m")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "modifyOtherKeys reset is not implemented for the legacy Windows API",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::DisableModifyOtherKeys;
    use super::EnableModifyOtherKeys;
    use super::ResetKeyboardEnhancementFlags;
    use super::VscodeDetection;
    use super::keyboard_enhancement_disabled_for;
    use super::keyboard_enhancement_flags;
    use super::parse_bool_env;
    use super::tmux_session_detected;
    use super::tmux_should_enable_modify_other_keys_for;
    use super::vscode_terminal_detected;
    use codex_terminal_detection::TerminalName;
    use crossterm::Command;
    use crossterm::event::PushKeyboardEnhancementFlags;
    use pretty_assertions::assert_eq;

    fn ansi_for(command: impl Command) -> String {
        let mut out = String::new();
        command.write_ansi(&mut out).unwrap();
        out
    }

    #[test]
    fn keyboard_enhancement_suppresses_release_reporting_for_iterm() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Iterm2,
                /*running_in_tmux_session*/ false,
                /*tmux_extended_keys_format*/ None
            ))),
            "\x1b[>5u"
        );
    }

    #[test]
    fn keyboard_enhancement_suppresses_release_reporting_for_ghostty() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Ghostty,
                /*running_in_tmux_session*/ false,
                /*tmux_extended_keys_format*/ None
            ))),
            "\x1b[>5u"
        );
    }

    #[test]
    fn keyboard_enhancement_preserves_repeat_reporting_for_kitty() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Kitty,
                /*running_in_tmux_session*/ false,
                /*tmux_extended_keys_format*/ None
            ))),
            "\x1b[>7u"
        );
    }

    #[test]
    fn keyboard_enhancement_preserves_repeat_reporting_for_csi_u_tmux() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Kitty,
                /*running_in_tmux_session*/ true,
                Some("csi-u")
            ))),
            "\x1b[>7u"
        );
    }

    #[test]
    fn keyboard_enhancement_preserves_shift_enter_for_xterm_tmux() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Kitty,
                /*running_in_tmux_session*/ true,
                Some("xterm")
            ))),
            "\x1b[>5u"
        );
    }

    #[test]
    fn keyboard_enhancement_preserves_repeat_reporting_for_unknown_terminals() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Unknown,
                /*running_in_tmux_session*/ false,
                /*tmux_extended_keys_format*/ None
            ))),
            "\x1b[>7u"
        );
    }

    #[test]
    fn keyboard_enhancement_uses_conservative_flags_when_tmux_format_is_unknown() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Unknown,
                /*running_in_tmux_session*/ true,
                /*tmux_extended_keys_format*/ None,
            ))),
            "\x1b[>5u"
        );
    }

    #[test]
    fn keyboard_enhancement_env_flag_parses_common_values() {
        assert_eq!(parse_bool_env(Some("1")), Some(true));
        assert_eq!(parse_bool_env(Some("true")), Some(true));
        assert_eq!(parse_bool_env(Some("YES")), Some(true));
        assert_eq!(parse_bool_env(Some("0")), Some(false));
        assert_eq!(parse_bool_env(Some("false")), Some(false));
        assert_eq!(parse_bool_env(Some("NO")), Some(false));
        assert_eq!(parse_bool_env(Some("unexpected")), None);
        assert_eq!(parse_bool_env(/*value*/ None), None);
    }

    #[test]
    fn keyboard_enhancement_auto_disables_for_vscode_or_unknown_in_wsl() {
        for detection in [
            VscodeDetection::VsCode,
            VscodeDetection::Other,
            VscodeDetection::Unknown,
        ] {
            assert_eq!(
                keyboard_enhancement_disabled_for(
                    /*disable_env*/ None, /*is_wsl*/ true, detection
                ),
                detection != VscodeDetection::Other,
            );
            assert!(!keyboard_enhancement_disabled_for(
                /*disable_env*/ None, /*is_wsl*/ false, detection
            ));
        }
    }

    #[test]
    fn keyboard_enhancement_env_flag_overrides_auto_detection() {
        for detection in [
            VscodeDetection::VsCode,
            VscodeDetection::Other,
            VscodeDetection::Unknown,
        ] {
            assert!(!keyboard_enhancement_disabled_for(
                Some("0"),
                /*is_wsl*/ true,
                detection
            ));
            assert!(keyboard_enhancement_disabled_for(
                Some("1"),
                /*is_wsl*/ false,
                detection
            ));
        }
    }

    #[test]
    fn vscode_terminal_detection_uses_linux_and_windows_term_program() {
        for detection in [
            VscodeDetection::VsCode,
            VscodeDetection::Other,
            VscodeDetection::Unknown,
        ] {
            assert_eq!(
                vscode_terminal_detected(Some("vscode"), detection),
                VscodeDetection::VsCode,
            );
            assert_eq!(
                vscode_terminal_detected(/*linux_term_program*/ None, detection),
                detection,
            );
        }
    }

    #[test]
    fn tmux_session_detection_accepts_tmux_or_tmux_pane() {
        assert!(tmux_session_detected(
            Some("/tmp/tmux-501/default,1,0"),
            /*tmux_pane*/ None
        ));
        assert!(tmux_session_detected(/*tmux*/ None, Some("%0")));
        assert!(!tmux_session_detected(
            /*tmux*/ None, /*tmux_pane*/ None
        ));
    }

    #[test]
    fn tmux_modify_other_keys_only_requests_confirmed_csi_u_format() {
        assert!(tmux_should_enable_modify_other_keys_for(
            /*running_in_tmux_session*/ true,
            Some("csi-u")
        ));
        assert!(!tmux_should_enable_modify_other_keys_for(
            /*running_in_tmux_session*/ true, /*extended_keys_format*/ None
        ));
        assert!(!tmux_should_enable_modify_other_keys_for(
            /*running_in_tmux_session*/ true,
            Some("xterm")
        ));
        assert!(!tmux_should_enable_modify_other_keys_for(
            /*running_in_tmux_session*/ true,
            Some("")
        ));
        assert!(!tmux_should_enable_modify_other_keys_for(
            /*running_in_tmux_session*/ false,
            Some("csi-u")
        ));
    }

    #[test]
    fn reset_keyboard_enhancement_flags_clears_all_pushed_levels() {
        assert_eq!(ansi_for(ResetKeyboardEnhancementFlags), "\x1b[<u");
    }

    #[test]
    fn enable_modify_other_keys_requests_xterm_keyboard_reporting() {
        assert_eq!(ansi_for(EnableModifyOtherKeys), "\x1b[>4;2m");
    }

    #[test]
    fn disable_modify_other_keys_resets_xterm_keyboard_reporting() {
        assert_eq!(ansi_for(DisableModifyOtherKeys), "\x1b[>4;0m");
    }
}
