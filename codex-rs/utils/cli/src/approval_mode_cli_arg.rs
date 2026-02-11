//! Standard type to use with the `--approval-mode` CLI option.

use clap::ValueEnum;

use codex_core::protocol::AskForApproval;

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ApprovalModeCliArg {
    /// Only run "trusted" commands (e.g. ls, cat, sed) without asking for user
    /// approval. Will escalate to the user if the model proposes a command that
    /// is not in the "trusted" set.
    Untrusted,

    /// Run all commands without asking for user approval.
    /// Only asks for approval if a command fails to execute, in which case it
    /// will escalate to the user to ask for un-sandboxed execution.
    OnFailure,

    /// The model decides when to ask the user for approval.
    OnRequest,

    /// Never ask for user approval
    /// Execution failures are immediately returned to the model.
    Never,
}

impl From<ApprovalModeCliArg> for AskForApproval {
    fn from(value: ApprovalModeCliArg) -> Self {
        match value {
            ApprovalModeCliArg::Untrusted => AskForApproval::UnlessTrusted,
            ApprovalModeCliArg::OnFailure => AskForApproval::OnFailure,
            ApprovalModeCliArg::OnRequest => AskForApproval::OnRequest,
            ApprovalModeCliArg::Never => AskForApproval::Never,
        }
    }
}
