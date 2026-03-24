//! The codex-fs-ops crate provides a helper binary for performing various
//! filesystem operations when `codex` is invoked with `--codex-run-as-fs-ops`
//! as the first argument. By exposing this functionality via a CLI, this makes
//! it possible to execute the CLI within a sandboxed context in order to ensure
//! the filesystem restrictions of the sandbox are honored.

mod command;
mod constants;
mod runner;

pub use constants::CODEX_CORE_FS_OPS_ARG1;
pub use constants::READ_FILE_OPERATION_ARG2;
pub use runner::run_from_args_and_exit;
