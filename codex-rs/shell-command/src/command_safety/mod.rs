mod powershell_parser;
// Production safety and exec-policy callers migrate to this lowerer in a follow-up.
#[allow(dead_code)]
mod powershell_tree_sitter;

pub mod is_dangerous_command;
pub mod is_safe_command;
#[cfg(windows)]
pub(crate) mod windows_safe_commands;
pub(crate) use powershell_parser::try_parse_powershell_ast_commands;
