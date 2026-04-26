//! Command parsing and safety utilities shared across Codex crates.

mod shell_detect;

pub mod bash;
pub(crate) mod command_safety;
pub mod parse_command;
pub mod powershell;
mod preserved_path_write;

pub use command_safety::is_dangerous_command;
pub use command_safety::is_safe_command;
pub use preserved_path_write::preserved_path_write_forbidden_reason;
