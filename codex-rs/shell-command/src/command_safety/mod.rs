#[cfg(windows)]
mod powershell_parser;

#[cfg(windows)]
pub(crate) use powershell_parser::try_parse_powershell_ast_commands;

pub mod is_dangerous_command;
pub mod is_safe_command;
#[cfg(windows)]
pub(crate) mod windows_safe_commands;
