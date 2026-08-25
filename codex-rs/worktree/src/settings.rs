//! Parses and validates worktree settings from the existing `[desktop]` config.
//!
//! Resolves the managed worktree root and the automatic-cleanup and retention
//! settings without introducing a separate configuration format.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

const WORKTREE_ROOT: &str = "git-worktree-root";
const AUTO_CLEANUP: &str = "worktree-auto-cleanup-enabled";
const KEEP_COUNT: &str = "worktree-keep-count";

pub const DEFAULT_WORKTREE_KEEP_COUNT: usize = 15;

/// Effective host-local settings already understood by Codex Desktop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeSettings {
    pub root: PathBuf,
    pub auto_cleanup_enabled: bool,
    pub keep_count: usize,
}

impl WorktreeSettings {
    /// Resolves existing `[desktop]` values without introducing another config format.
    pub fn from_desktop_config(
        codex_home: &Path,
        desktop: Option<&HashMap<String, Value>>,
    ) -> Result<Self> {
        let root = match desktop.and_then(|settings| settings.get(WORKTREE_ROOT)) {
            None | Some(Value::Null) => codex_home.join("worktrees"),
            Some(value) => {
                let configured = value
                    .as_str()
                    .context("desktop.git-worktree-root must be a string")?
                    .trim();
                if configured.is_empty() {
                    codex_home.join("worktrees")
                } else {
                    let path = PathBuf::from(configured);
                    if !path.is_absolute() {
                        bail!("desktop.git-worktree-root must be an absolute path");
                    }
                    path
                }
            }
        };

        let auto_cleanup_enabled = desktop
            .and_then(|settings| settings.get(AUTO_CLEANUP))
            .map(|value| {
                value
                    .as_bool()
                    .context("desktop.worktree-auto-cleanup-enabled must be a boolean")
            })
            .transpose()?
            .unwrap_or(true);

        let keep_count = desktop
            .and_then(|settings| settings.get(KEEP_COUNT))
            .map(|value| {
                let count = value
                    .as_u64()
                    .context("desktop.worktree-keep-count must be a positive integer")?;
                if count == 0 {
                    bail!("desktop.worktree-keep-count must be a positive integer");
                }
                usize::try_from(count).context("desktop.worktree-keep-count is too large")
            })
            .transpose()?
            .unwrap_or(DEFAULT_WORKTREE_KEEP_COUNT);

        Ok(Self {
            root: dunce::simplified(&root).to_path_buf(),
            auto_cleanup_enabled,
            keep_count,
        })
    }
}
