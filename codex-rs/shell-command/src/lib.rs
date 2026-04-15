//! Command parsing and safety utilities shared across Codex crates.

mod shell_detect;

#[cfg_attr(target_arch = "wasm32", path = "bash_wasm.rs")]
pub mod bash;
pub mod command_safety;
pub mod parse_command;
#[cfg_attr(target_arch = "wasm32", path = "powershell_wasm.rs")]
pub mod powershell;

pub use command_safety::is_dangerous_command;
pub use command_safety::is_safe_command;
