//! Standard type to use with the `--approval-mode` CLI option.
//! Available when the `cli` feature is enabled for the crate.

use clap::ValueEnum;

use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;

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

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum SandboxModeCliArg {
    /// Network syscalls will be blocked
    NetworkRestricted,
    /// Filesystem writes will be restricted
    FileWriteRestricted,
    /// Network and filesystem writes will be restricted
    NetworkAndFileWriteRestricted,
    /// No restrictions; full "unsandboxed" mode
    DangerousNoRestrictions,
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

impl From<SandboxModeCliArg> for SandboxPolicy {
    fn from(value: SandboxModeCliArg) -> Self {
        match value {
            SandboxModeCliArg::NetworkRestricted => SandboxPolicy::NetworkRestricted,
            SandboxModeCliArg::FileWriteRestricted => SandboxPolicy::FileWriteRestricted,
            SandboxModeCliArg::NetworkAndFileWriteRestricted => {
                SandboxPolicy::NetworkAndFileWriteRestricted
            }
            SandboxModeCliArg::DangerousNoRestrictions => SandboxPolicy::DangerousNoRestrictions,
        }
    }
}
