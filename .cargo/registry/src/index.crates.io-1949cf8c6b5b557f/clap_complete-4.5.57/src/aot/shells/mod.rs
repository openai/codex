//! Shell-specific generators

mod bash;
mod elvish;
mod fish;
mod powershell;
mod shell;
mod zsh;

pub use bash::Bash;
pub use elvish::Elvish;
pub use fish::Fish;
pub use powershell::PowerShell;
pub use shell::Shell;
pub use zsh::Zsh;
