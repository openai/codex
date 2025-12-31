//! Extension tools module
//!
//! Contains extension tool specifications that can be conditionally registered
//! based on features and model family capabilities.

pub mod ask_user_question;
pub mod bash_output;
pub mod code_search;
pub mod enter_plan_mode;
pub mod exit_plan_mode;
pub mod glob_files;
pub mod kill_shell;
pub mod list_dir;
pub mod lsp;
pub mod ripgrep;
pub mod smart_edit;
pub mod subagent;
pub mod think;
pub mod web_fetch;
pub mod web_search;
pub mod write_file;
