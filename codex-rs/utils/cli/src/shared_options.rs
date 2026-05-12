//! Shared command-line flags used by both interactive and non-interactive Codex entry points.

use crate::SandboxModeCliArg;
use crate::WorktreeDirtyCliArg;
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

    /// Tell the agent to use the specified directory as its working root.
    #[clap(long = "cd", short = 'C', value_name = "DIR")]
    pub cwd: Option<PathBuf>,

    /// Create or reuse a Codex-managed Git worktree for this branch and run from that workspace.
    #[arg(long = "worktree", value_name = "BRANCH")]
    pub worktree: Option<String>,

    /// Base ref for a newly created managed worktree.
    #[arg(long = "worktree-base", value_name = "REF")]
    pub worktree_base: Option<String>,

    /// How to handle uncommitted source checkout changes when creating a worktree.
    #[arg(long = "worktree-dirty", value_enum, default_value_t = WorktreeDirtyCliArg::Fail)]
    pub worktree_dirty: WorktreeDirtyCliArg,

    /// Additional directories that should be writable alongside the primary workspace.
    #[arg(long = "add-dir", value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    pub add_dir: Vec<PathBuf>,
}

impl SharedCliOptions {
    pub fn inherit_exec_root_options(&mut self, root: &Self) {
        let self_selected_sandbox_mode =
            self.sandbox_mode.is_some() || self.dangerously_bypass_approvals_and_sandbox;
        let Self {
            images,
            model,
            oss,
            oss_provider,
            config_profile,
            sandbox_mode,
            dangerously_bypass_approvals_and_sandbox,
            cwd,
            worktree,
            worktree_base,
            worktree_dirty,
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
            cwd: root_cwd,
            worktree: root_worktree,
            worktree_base: root_worktree_base,
            worktree_dirty: root_worktree_dirty,
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
        if !self_selected_sandbox_mode {
            *dangerously_bypass_approvals_and_sandbox =
                *root_dangerously_bypass_approvals_and_sandbox;
        }
        if cwd.is_none() {
            cwd.clone_from(root_cwd);
        }
        if worktree.is_none() {
            worktree.clone_from(root_worktree);
        }
        if worktree_base.is_none() {
            worktree_base.clone_from(root_worktree_base);
        }
        if *worktree_dirty == WorktreeDirtyCliArg::Fail {
            *worktree_dirty = *root_worktree_dirty;
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
        let subcommand_selected_sandbox_mode = subcommand.sandbox_mode.is_some()
            || subcommand.dangerously_bypass_approvals_and_sandbox;
        let Self {
            images,
            model,
            oss,
            oss_provider,
            config_profile,
            sandbox_mode,
            dangerously_bypass_approvals_and_sandbox,
            cwd,
            worktree,
            worktree_base,
            worktree_dirty,
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
        if subcommand_selected_sandbox_mode {
            self.sandbox_mode = sandbox_mode;
            self.dangerously_bypass_approvals_and_sandbox =
                dangerously_bypass_approvals_and_sandbox;
        }
        if let Some(cwd) = cwd {
            self.cwd = Some(cwd);
        }
        if let Some(worktree) = worktree {
            self.worktree = Some(worktree);
        }
        if let Some(worktree_base) = worktree_base {
            self.worktree_base = Some(worktree_base);
        }
        if worktree_dirty != WorktreeDirtyCliArg::Fail {
            self.worktree_dirty = worktree_dirty;
        }
        if !images.is_empty() {
            self.images = images;
        }
        if !add_dir.is_empty() {
            self.add_dir.extend(add_dir);
        }
    }
}
