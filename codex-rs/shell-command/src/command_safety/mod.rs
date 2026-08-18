mod powershell_parser;
mod powershell_tree_sitter;

pub mod is_dangerous_command;
pub mod is_safe_command;
#[cfg(windows)]
pub(crate) mod windows_safe_commands;
pub(crate) use powershell_parser::try_parse_powershell_ast_commands;
pub(crate) use powershell_tree_sitter::try_parse_powershell_commands;
