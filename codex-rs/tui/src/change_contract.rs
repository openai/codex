use serde::{Deserialize, Serialize};

/// Change contract constraints derived from PRD tasks.
///
/// Defaults favor safety while keeping DX reasonable. Fields marked with
/// `Option` are unbounded when `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeContract {
    // Core identifiers
    pub task_id: String,

    // Path governance
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub deny_paths: Vec<String>,

    // Global budgets
    #[serde(default)]
    pub max_files_changed: Option<usize>,
    #[serde(default)]
    pub max_lines_added: Option<usize>,
    #[serde(default)]
    pub max_lines_removed: Option<usize>,

    // Rename/Delete permissions
    #[serde(default)]
    pub allow_renames: bool,
    #[serde(default)]
    pub allow_copies: bool,
    #[serde(default)]
    pub allow_deletes: bool,

    // Binary/test/commit policy
    #[serde(default = "default_true")] // safer by default
    pub forbid_binary: bool,
    #[serde(default)]
    pub require_tests: bool,
    #[serde(default = "default_commit_prefix")]
    pub commit_prefix: String,
    #[serde(default)]
    pub require_signoff: bool,

    // P1 — Governance extensions
    // Per-file budgets and limits
    #[serde(default)]
    pub max_new_files: Option<usize>,
    #[serde(default)]
    pub max_bytes_per_file: Option<usize>,
    #[serde(default)]
    pub max_lines_added_per_file: Option<usize>,
    #[serde(default)]
    pub max_hunks_per_file: Option<usize>,

    // File-type/metadata constraints
    #[serde(default)]
    pub allowed_extensions: Vec<String>, // e.g., ["rs", "md"]; empty = allow all
    #[serde(default = "default_true")] // safer by default
    pub forbid_symlinks: bool,
    #[serde(default = "default_true")] // safer by default
    pub forbid_permissions_changes: bool,
    #[serde(default = "default_true")] // safer by default
    pub forbid_exec_mode_changes: bool,

    // P1-03 — Secrets & minified guardrails
    #[serde(default = "default_true")] // safer by default
    pub forbid_secrets: bool,
    #[serde(default = "default_true")] // safer by default
    pub forbid_minified: bool,

    // P1-04 — Optional deny presets (e.g., node_modules, dist)
    #[serde(default)]
    pub deny_presets: Vec<String>,
}

fn default_true() -> bool { true }
fn default_commit_prefix() -> String { "chore".to_string() }

impl Default for ChangeContract {
    fn default() -> Self {
        Self {
            task_id: String::new(),
            allowed_paths: vec![],
            deny_paths: vec![],
            max_files_changed: None,
            max_lines_added: None,
            max_lines_removed: None,
            allow_renames: false,
            allow_copies: false,
            allow_deletes: false,
            forbid_binary: true,
            require_tests: false,
            commit_prefix: default_commit_prefix(),
            require_signoff: false,
            max_new_files: None,
            max_bytes_per_file: None,
            max_lines_added_per_file: None,
            max_hunks_per_file: None,
            allowed_extensions: vec![],
            forbid_symlinks: true,
            forbid_permissions_changes: true,
            forbid_exec_mode_changes: true,
            forbid_secrets: true,
            forbid_minified: true,
            deny_presets: vec![],
        }
    }
}
