pub mod is_dangerous_command;
pub mod is_safe_command;
pub mod windows_safe_commands;

mod bash_parse;
#[cfg(windows)]
#[path = "windows_dangerous_commands.rs"]
mod windows_dangerous_commands;
