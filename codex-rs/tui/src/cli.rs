use clap::Parser;
use codex_core::ApprovalModeCliArg;
use codex_core::SandboxModeCliArg;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version)]
pub struct Cli {
    /// Optional user prompt to start the session.
    pub prompt: Option<String>,

    /// Optional image(s) to attach to the initial prompt.
    #[arg(long = "image", short = 'i', value_name = "FILE", value_delimiter = ',', num_args = 1..)]
    pub images: Vec<PathBuf>,

    /// Model the agent should use.
    #[arg(long, short = 'm')]
    pub model: Option<String>,

    /// Configure when the model requires human approval before executing a command.
    #[arg(long = "ask-for-approval", short = 'a')]
    pub approval_policy: Option<ApprovalModeCliArg>,

    /// Configure the process restrictions when a command is executed.
    ///
    /// Uses OS-specific sandboxing tools; Seatbelt on OSX, landlock+seccomp on Linux.
    #[arg(long = "sandbox", short = 's')]
    pub sandbox_policy: Option<SandboxModeCliArg>,

    /// Allow running Codex outside a Git repository.
    #[arg(long = "skip-git-repo-check", default_value_t = false)]
    pub skip_git_repo_check: bool,

    /// Disable server‑side response storage (sends the full conversation context with every request)
    #[arg(long = "disable-response-storage", default_value_t = false)]
    pub disable_response_storage: bool,

    /// Convenience alias for low-friction sandboxed automatic execution (-a on-failure, -s network-and-file-write-restricted)
    #[arg(long = "full-auto", default_value_t = true)]
    pub full_auto: bool,

    /// Convenience alias for supervised sandboxed execution (-a unless-allow-listed, -s network-and-file-write-restricted)
    #[arg(long = "suggest", default_value_t = false)]
    pub suggest: bool,
}
