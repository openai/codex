use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Request the managed worktrees associated with the repository containing \`cwd\`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeListParams {
    /// Repository-relative workspace cwd to inspect. Omitted uses app-server's effective cwd.
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
}

/// Managed worktrees returned by \`worktree/list\`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeListResponse {
    pub data: Vec<WorktreeInfo>,
}

/// Inspect dirty state for the repository containing \`cwd\`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeInspectSourceParams {
    /// Repository-relative workspace cwd to inspect. Omitted uses app-server's effective cwd.
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
}

/// Dirty-state response returned by \`worktree/inspectSource\`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeInspectSourceResponse {
    pub dirty: WorktreeDirtyState,
}

/// Create or reuse a managed worktree from the repository containing \`cwd\`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeCreateParams {
    /// Repository-relative workspace cwd to use as the source checkout.
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
    pub branch: String,
    #[ts(optional = nullable)]
    pub base_ref: Option<String>,
    pub dirty_policy: WorktreeDirtyPolicy,
}

/// Result returned by \`worktree/create\`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeCreateResponse {
    pub reused: bool,
    pub info: WorktreeInfo,
    pub warnings: Vec<WorktreeWarning>,
}

/// Remove a managed worktree in the repository containing \`cwd\`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeRemoveParams {
    /// Repository-relative workspace cwd to use when resolving \`name_or_path\`.
    #[ts(optional = nullable)]
    pub cwd: Option<String>,
    pub name_or_path: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub delete_branch: bool,
}

/// Result returned by \`worktree/remove\`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeRemoveResponse {
    pub removed_path: String,
    pub deleted_branch: Option<String>,
}

/// Server-native representation of a managed worktree.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeInfo {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub source: WorktreeSource,
    pub location: WorktreeLocation,
    pub repo_name: String,
    pub repo_root: String,
    pub common_git_dir: String,
    pub worktree_git_root: String,
    pub workspace_cwd: String,
    pub original_relative_cwd: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub owner_thread_id: Option<String>,
    pub metadata_path: String,
    pub dirty: WorktreeDirtyState,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorktreeSource {
    Cli,
    App,
    Legacy,
    Git,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorktreeLocation {
    Sibling,
    CodexHome,
    External,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorktreeDirtyPolicy {
    Fail,
    Ignore,
    CopyTracked,
    CopyAll,
    MoveTracked,
    MoveAll,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeDirtyState {
    pub has_staged_changes: bool,
    pub has_unstaged_changes: bool,
    pub has_untracked_files: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorktreeWarning {
    pub message: String,
}
