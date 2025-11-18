use clap::Parser;
use clap::ValueHint;
use std::path::PathBuf;

use crate::SandboxModeCliArg;

/// Flags shared across multiple Codex CLIs.
#[derive(Parser, Debug, Default, Clone)]
pub struct CommonCli {
    /// Optional image(s) to attach to the initial prompt.
    #[arg(
        long = "image",
        short = 'i',
        value_name = "FILE",
        value_delimiter = ',',
        num_args = 1..,
        global = true
    )]
    pub images: Vec<PathBuf>,

    /// Model the agent should use.
    #[arg(long, short = 'm', global = true)]
    pub model: Option<String>,

    /// Convenience flag to select the local open source model provider. Equivalent to -c
    /// model_provider=oss; verifies a local LM Studio or Ollama server is running.
    #[arg(long = "oss", default_value_t = false, global = true)]
    pub oss: bool,

    /// Specify which local provider to use (lmstudio or ollama).
    /// If not specified with --oss, will use config default or show selection.
    #[arg(long = "local-provider", global = true)]
    pub oss_provider: Option<String>,

    /// Configuration profile from config.toml to specify default options.
    #[arg(long = "profile", short = 'p', global = true)]
    pub config_profile: Option<String>,

    /// Select the sandbox policy to use when executing model-generated shell
    /// commands.
    #[arg(long = "sandbox", short = 's', value_enum, global = true)]
    pub sandbox_mode: Option<SandboxModeCliArg>,

    /// Convenience alias for low-friction sandboxed automatic execution (-a on-request, --sandbox workspace-write).
    #[arg(long = "full-auto", default_value_t = false, global = true)]
    pub full_auto: bool,

    /// Skip all confirmation prompts and execute commands without sandboxing.
    /// EXTREMELY DANGEROUS. Intended solely for running in environments that are externally sandboxed.
    #[arg(
        long = "dangerously-bypass-approvals-and-sandbox",
        alias = "yolo",
        default_value_t = false,
        conflicts_with = "full_auto",
        global = true
    )]
    pub dangerously_bypass_approvals_and_sandbox: bool,

    /// Tell the agent to use the specified directory as its working root.
    #[clap(long = "cd", short = 'C', value_name = "DIR", global = true)]
    pub cwd: Option<PathBuf>,

    /// Additional directories that should be writable alongside the primary workspace.
    #[arg(
        long = "add-dir",
        value_name = "DIR",
        value_hint = ValueHint::DirPath,
        global = true
    )]
    pub add_dir: Vec<PathBuf>,
}

impl CommonCli {
    /// Apply overrides from another `CommonCli`, giving precedence to fields that
    /// are explicitly set on `other`.
    pub fn apply_overrides(&mut self, other: &CommonCli) {
        if let Some(model) = &other.model {
            self.model = Some(model.clone());
        }
        if other.oss {
            self.oss = true;
        }
        if let Some(provider) = &other.oss_provider {
            self.oss_provider = Some(provider.clone());
        }
        if let Some(profile) = &other.config_profile {
            self.config_profile = Some(profile.clone());
        }
        if let Some(sandbox) = other.sandbox_mode {
            self.sandbox_mode = Some(sandbox);
        }
        if other.full_auto {
            self.full_auto = true;
        }
        if other.dangerously_bypass_approvals_and_sandbox {
            self.dangerously_bypass_approvals_and_sandbox = true;
        }
        if let Some(cwd) = &other.cwd {
            self.cwd = Some(cwd.clone());
        }
        if !other.images.is_empty() {
            self.images = other.images.clone();
        }
        if !other.add_dir.is_empty() {
            self.add_dir.extend(other.add_dir.clone());
        }
    }
}
