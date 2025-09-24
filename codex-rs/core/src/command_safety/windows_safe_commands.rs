#![cfg(target_os = "windows")]

pub fn is_safe_command_windows(command: &[String]) -> bool {
    false
}
