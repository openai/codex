//! Standard type to use with the `--sandbox` (`-s`) CLI option.
//!
//! This mirrors the variants of [`codex_core::protocol::SandboxPolicy`], but
//! without any of the associated data so it can be expressed as a simple flag
//! on the command-line. Users that need to tweak the advanced options for
//! `workspace-write` can continue to do so via `-c` overrides or their
//! `config.toml`.

use clap::ValueEnum;
use codex_protocol::config_types::SandboxMode;

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum SandboxModeCliArg {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl From<SandboxModeCliArg> for SandboxMode {
    fn from(value: SandboxModeCliArg) -> Self {
        match value {
            SandboxModeCliArg::ReadOnly => SandboxMode::ReadOnly,
            SandboxModeCliArg::WorkspaceWrite => SandboxMode::WorkspaceWrite,
            SandboxModeCliArg::DangerFullAccess => SandboxMode::DangerFullAccess,
        }
    }
}
