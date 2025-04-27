use clap::Parser;
use codex_core::ApprovalModeCliArg;
use codex_core::SandboxModeCliArg;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version)]
pub struct Cli {
    /// Optional image(s) to attach to the initial prompt.
    #[arg(long = "image", short = 'i', value_name = "FILE", value_delimiter = ',', num_args = 1..)]
    pub images: Vec<PathBuf>,

    /// Model the agent should use.
    #[arg(long, short = 'm')]
    pub model: Option<String>,

    /// Configure when the model requires human approval before executing a command.
    #[arg(long = "ask-for-approval", short = 'a', value_enum, default_value_t = ApprovalModeCliArg::OnFailure)]
    pub approval_policy: ApprovalModeCliArg,

    /// Configure the process restrictions when a command is executed.
    ///
    /// Uses OS-specific sandboxing tools; Seatbelt on OSX, landlock+seccomp on Linux.
    #[arg(long = "sandbox", short = 's', value_enum, default_value_t = SandboxModeCliArg::NetworkAndFileWriteRestricted)]
    pub sandbox_policy: SandboxModeCliArg,

    /// Allow running Codex outside a Git repository.
    #[arg(long = "skip-git-repo-check", default_value_t = false)]
    pub skip_git_repo_check: bool,

    /// Initial instructions for the agent.
    pub prompt: Option<String>,
}
