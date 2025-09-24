#[cfg(unix)]
use super::unix_safe_commands::is_safe_command_unix;
#[cfg(target_os = "windows")]
use super::windows_safe_commands::is_safe_command_windows;

#[cfg(target_os = "windows")]
pub fn is_known_safe_command(command: &[String]) -> bool {
    is_safe_command_windows(command)
}

#[cfg(not(target_os = "windows"))]
pub fn is_known_safe_command(command: &[String]) -> bool {
    is_safe_command_unix(command)
}
