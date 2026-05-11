//! Shared command-line flags used by both interactive and non-interactive Codex entry points.

use crate::SandboxModeCliArg;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Default)]
pub struct SharedCliOptions {
    /// Optional image(s) to attach to the initial prompt.
    #[arg(
        long = "image",
        short = 'i',
        value_name = "FILE",
        value_delimiter = ',',
        num_args = 1..
    )]
    pub images: Vec<PathBuf>,

    /// Model the agent should use.
    #[arg(long, short = 'm')]
    pub model: Option<String>,

    /// Use open-source provider.
    #[arg(long = "oss", default_value_t = false)]
    pub oss: bool,

    /// Specify which local provider to use (lmstudio or ollama).
    /// If not specified with --oss, will use config default or show selection.
    #[arg(long = "local-provider")]
    pub oss_provider: Option<String>,

    /// Configuration profile from config.toml to specify default options.
    #[arg(long = "profile", short = 'p')]
    pub config_profile: Option<String>,

    /// Select the sandbox policy to use when executing model-generated shell
    /// commands.
    #[arg(long = "sandbox", short = 's')]
    pub sandbox_mode: Option<SandboxModeCliArg>,

    /// Skip all confirmation prompts and execute commands without sandboxing.
    /// EXTREMELY DANGEROUS. Intended solely for running in environments that are externally sandboxed.
    #[arg(
        long = "dangerously-bypass-approvals-and-sandbox",
        alias = "yolo",
        default_value_t = false
    )]
    pub dangerously_bypass_approvals_and_sandbox: bool,

    /// Let Codex auto-review approval requests while running with workspace write access.
    #[arg(
        long = "not-so-yolo",
        default_value_t = false,
        conflicts_with = "dangerously_bypass_approvals_and_sandbox"
    )]
    pub auto_review_cli_mode: bool,

    /// Tell the agent to use the specified directory as its working root.
    #[clap(long = "cd", short = 'C', value_name = "DIR")]
    pub cwd: Option<PathBuf>,

    /// Additional directories that should be writable alongside the primary workspace.
    #[arg(long = "add-dir", value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    pub add_dir: Vec<PathBuf>,
}

impl SharedCliOptions {
    pub fn inherit_exec_root_options(&mut self, root: &Self) {
        let self_selected_permission_mode = self.sandbox_mode.is_some()
            || self.dangerously_bypass_approvals_and_sandbox
            || self.auto_review_cli_mode;
        let Self {
            images,
            model,
            oss,
            oss_provider,
            config_profile,
            sandbox_mode,
            dangerously_bypass_approvals_and_sandbox,
            auto_review_cli_mode,
            cwd,
            add_dir,
        } = self;
        let Self {
            images: root_images,
            model: root_model,
            oss: root_oss,
            oss_provider: root_oss_provider,
            config_profile: root_config_profile,
            sandbox_mode: root_sandbox_mode,
            dangerously_bypass_approvals_and_sandbox: root_dangerously_bypass_approvals_and_sandbox,
            auto_review_cli_mode: root_auto_review_cli_mode,
            cwd: root_cwd,
            add_dir: root_add_dir,
        } = root;

        if model.is_none() {
            model.clone_from(root_model);
        }
        if *root_oss {
            *oss = true;
        }
        if oss_provider.is_none() {
            oss_provider.clone_from(root_oss_provider);
        }
        if config_profile.is_none() {
            config_profile.clone_from(root_config_profile);
        }
        if sandbox_mode.is_none() {
            *sandbox_mode = *root_sandbox_mode;
        }
        if !self_selected_permission_mode {
            *dangerously_bypass_approvals_and_sandbox =
                *root_dangerously_bypass_approvals_and_sandbox;
            *auto_review_cli_mode = *root_auto_review_cli_mode;
        }
        if cwd.is_none() {
            cwd.clone_from(root_cwd);
        }
        if !root_images.is_empty() {
            let mut merged_images = root_images.clone();
            merged_images.append(images);
            *images = merged_images;
        }
        if !root_add_dir.is_empty() {
            let mut merged_add_dir = root_add_dir.clone();
            merged_add_dir.append(add_dir);
            *add_dir = merged_add_dir;
        }
    }

    pub fn apply_subcommand_overrides(&mut self, subcommand: Self) {
        let subcommand_selected_permission_mode = subcommand.sandbox_mode.is_some()
            || subcommand.dangerously_bypass_approvals_and_sandbox
            || subcommand.auto_review_cli_mode;
        let Self {
            images,
            model,
            oss,
            oss_provider,
            config_profile,
            sandbox_mode,
            dangerously_bypass_approvals_and_sandbox,
            auto_review_cli_mode,
            cwd,
            add_dir,
        } = subcommand;

        if let Some(model) = model {
            self.model = Some(model);
        }
        if oss {
            self.oss = true;
        }
        if let Some(oss_provider) = oss_provider {
            self.oss_provider = Some(oss_provider);
        }
        if let Some(config_profile) = config_profile {
            self.config_profile = Some(config_profile);
        }
        if subcommand_selected_permission_mode {
            self.sandbox_mode = sandbox_mode;
            self.dangerously_bypass_approvals_and_sandbox =
                dangerously_bypass_approvals_and_sandbox;
            self.auto_review_cli_mode = auto_review_cli_mode;
        }
        if let Some(cwd) = cwd {
            self.cwd = Some(cwd);
        }
        if !images.is_empty() {
            self.images = images;
        }
        if !add_dir.is_empty() {
            self.add_dir.extend(add_dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use pretty_assertions::assert_eq;

    #[derive(Debug, Parser)]
    struct TestCli {
        #[clap(flatten)]
        shared: SharedCliOptions,
    }

    #[test]
    fn parses_not_so_yolo() {
        let cli = TestCli::parse_from(["test", "--not-so-yolo"]);

        assert!(cli.shared.auto_review_cli_mode);
        assert!(!cli.shared.dangerously_bypass_approvals_and_sandbox);
    }

    #[test]
    fn yolo_and_not_so_yolo_conflict() {
        let err = TestCli::try_parse_from(["test", "--yolo", "--not-so-yolo"])
            .expect_err("permission modes should conflict");

        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn not_so_yolo_inherits_to_exec_subcommand_options() {
        let root = SharedCliOptions {
            auto_review_cli_mode: true,
            ..Default::default()
        };
        let mut exec = SharedCliOptions::default();

        exec.inherit_exec_root_options(&root);

        assert!(exec.auto_review_cli_mode);
        assert!(!exec.dangerously_bypass_approvals_and_sandbox);
    }

    #[test]
    fn subcommand_permission_mode_blocks_root_not_so_yolo() {
        let root = SharedCliOptions {
            auto_review_cli_mode: true,
            ..Default::default()
        };
        let mut exec = SharedCliOptions {
            sandbox_mode: Some(SandboxModeCliArg::ReadOnly),
            ..Default::default()
        };

        exec.inherit_exec_root_options(&root);

        assert!(!exec.auto_review_cli_mode);
        assert!(matches!(
            exec.sandbox_mode,
            Some(SandboxModeCliArg::ReadOnly)
        ));
    }
}
