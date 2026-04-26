//! Terminal keyboard enhancement setup and teardown helpers.
//!
//! The TUI uses crossterm's keyboard enhancement stack while it owns the terminal, but
//! process exit gets a stronger reset so the parent shell does not inherit enhanced key
//! reporting if a terminal misses the normal stack pop.

use std::fmt;
use std::io::stdout;

use crossterm::Command;
use crossterm::event::KeyboardEnhancementFlags;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::event::PushKeyboardEnhancementFlags;
use ratatui::crossterm::execute;

const DISABLE_KEYBOARD_ENHANCEMENT_ENV_VAR: &str = "CODEX_TUI_DISABLE_KEYBOARD_ENHANCEMENT";

pub(super) fn keyboard_enhancement_disabled() -> bool {
    let disable_env = std::env::var(DISABLE_KEYBOARD_ENHANCEMENT_ENV_VAR).ok();
    let is_wsl = running_in_wsl();
    let is_vscode_terminal = is_wsl && running_in_vscode_terminal();
    keyboard_enhancement_disabled_for(disable_env.as_deref(), is_wsl, is_vscode_terminal)
}

fn keyboard_enhancement_disabled_for(
    disable_env: Option<&str>,
    is_wsl: bool,
    is_vscode_terminal: bool,
) -> bool {
    if let Some(disabled) = parse_bool_env(disable_env) {
        return disabled;
    }

    // VS Code running a WSL shell can hide TERM_PROGRAM from the Linux process
    // environment, so `running_in_vscode_terminal` also probes the Windows-side
    // environment through WSL interop.
    is_wsl && is_vscode_terminal
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

fn running_in_vscode_terminal() -> bool {
    vscode_terminal_detected(
        std::env::var("TERM_PROGRAM").ok().as_deref(),
        windows_term_program().as_deref(),
    )
}

fn vscode_terminal_detected(
    linux_term_program: Option<&str>,
    windows_term_program: Option<&str>,
) -> bool {
    term_program_is_vscode(linux_term_program) || term_program_is_vscode(windows_term_program)
}

fn term_program_is_vscode(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.eq_ignore_ascii_case("vscode"))
}

fn windows_term_program() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        static WINDOWS_TERM_PROGRAM: std::sync::OnceLock<Option<String>> =
            std::sync::OnceLock::new();
        WINDOWS_TERM_PROGRAM
            .get_or_init(read_windows_term_program)
            .clone()
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
fn read_windows_term_program() -> Option<String> {
    let output = std::process::Command::new("cmd.exe")
        .args(["/d", "/s", "/c", "set TERM_PROGRAM"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            line.trim_end_matches('\r')
                .strip_prefix("TERM_PROGRAM=")
                .map(str::to_string)
        })
        .filter(|value| !value.trim().is_empty())
}

pub(super) fn enable_keyboard_enhancement() {
    if keyboard_enhancement_disabled() {
        return;
    }

    let _ = execute!(
        stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
        )
    );
}

pub(super) fn restore_keyboard_enhancement_stack() {
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
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
    use super::ResetKeyboardEnhancementFlags;
    use super::keyboard_enhancement_disabled_for;
    use super::parse_bool_env;
    use super::vscode_terminal_detected;
    use crossterm::Command;
    use pretty_assertions::assert_eq;

    fn ansi_for(command: impl Command) -> String {
        let mut out = String::new();
        command.write_ansi(&mut out).unwrap();
        out
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
    fn keyboard_enhancement_auto_disables_for_vscode_in_wsl() {
        assert!(keyboard_enhancement_disabled_for(
            /*disable_env*/ None, /*is_wsl*/ true, /*is_vscode_terminal*/ true
        ));
    }

    #[test]
    fn keyboard_enhancement_auto_disable_requires_wsl_and_vscode() {
        assert!(!keyboard_enhancement_disabled_for(
            /*disable_env*/ None, /*is_wsl*/ true, /*is_vscode_terminal*/ false
        ));
        assert!(!keyboard_enhancement_disabled_for(
            /*disable_env*/ None, /*is_wsl*/ false, /*is_vscode_terminal*/ true
        ));
    }

    #[test]
    fn keyboard_enhancement_env_flag_overrides_auto_detection() {
        assert!(!keyboard_enhancement_disabled_for(
            Some("0"),
            /*is_wsl*/ true,
            /*is_vscode_terminal*/ true
        ));
        assert!(keyboard_enhancement_disabled_for(
            Some("1"),
            /*is_wsl*/ false,
            /*is_vscode_terminal*/ false
        ));
    }

    #[test]
    fn vscode_terminal_detection_uses_linux_and_windows_term_program() {
        assert!(vscode_terminal_detected(
            Some("vscode"),
            /*windows_term_program*/ None
        ));
        assert!(vscode_terminal_detected(
            /*linux_term_program*/ None,
            Some("vscode")
        ));
        assert!(!vscode_terminal_detected(
            /*linux_term_program*/ None,
            Some("WindowsTerminal")
        ));
        assert!(!vscode_terminal_detected(
            /*linux_term_program*/ None, /*windows_term_program*/ None
        ));
    }

    #[test]
    fn reset_keyboard_enhancement_flags_clears_all_pushed_levels() {
        assert_eq!(ansi_for(ResetKeyboardEnhancementFlags), "\x1b[<u");
    }

    #[test]
    fn disable_modify_other_keys_resets_xterm_keyboard_reporting() {
        assert_eq!(ansi_for(DisableModifyOtherKeys), "\x1b[>4;0m");
    }
}
