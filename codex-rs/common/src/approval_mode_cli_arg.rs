//! Standard type to use with the `--approval-mode` CLI option.
//! Available when the `cli` feature is enabled for the crate.

use clap::ArgAction;
use clap::Parser;
use clap::ValueEnum;

use codex_core::config::parse_sandbox_permission_with_base_path;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPermission;

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ApprovalModeCliArg {
    /// Run all commands without asking for user approval.
    /// Only asks for approval if a command fails to execute, in which case it
    /// will escalate to the user to ask for un-sandboxed execution.
    OnFailure,

    /// Only run "known safe" commands (e.g. ls, cat, sed) without
    /// asking for user approval. Will escalate to the user if the model
    /// proposes a command that is not allow-listed.
    UnlessAllowListed,

    /// Never ask for user approval
    /// Execution failures are immediately returned to the model.
    Never,
}

impl From<ApprovalModeCliArg> for AskForApproval {
    fn from(value: ApprovalModeCliArg) -> Self {
        match value {
            ApprovalModeCliArg::OnFailure => AskForApproval::OnFailure,
            ApprovalModeCliArg::UnlessAllowListed => AskForApproval::UnlessAllowListed,
            ApprovalModeCliArg::Never => AskForApproval::Never,
        }
    }
}

#[derive(Parser, Debug)]
pub struct SandboxPermissionOption {
    /// Specify this flag multiple times to specify the full set of permissions
    /// to grant to Codex.
    ///
    /// ```shell
    /// codex -s disk-full-read-access \
    ///       -s disk-write-cwd \
    ///       -s disk-write-platform-user-temp-folder \
    ///       -s disk-write-platform-global-temp-folder
    /// ```
    ///
    /// Note disk-write-folder takes a value:
    ///
    /// ```shell
    ///     -s disk-write-folder=$HOME/.pyenv/shims
    /// ```
    ///
    /// These permissions are quite broad and should be used with caution:
    ///
    /// ```shell
    ///     -s disk-full-write-access
    ///     -s network-full-access
    /// ```
    #[arg(long = "sandbox-permission", short = 's', action = ArgAction::Append, value_parser = parse_sandbox_permission)]
    pub permissions: Option<Vec<SandboxPermission>>,
}

/// Custom value-parser so we can keep the CLI surface small *and*
/// still handle the parameterised `disk-write-folder` case.
fn parse_sandbox_permission(raw: &str) -> std::io::Result<SandboxPermission> {
    let base_path = std::env::current_dir()?;
    parse_sandbox_permission_with_base_path(raw, base_path)
}
