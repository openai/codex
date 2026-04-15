use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

use crate::GitSha;

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, TS)]
pub struct GitInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<GitSha>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitDiffToRemote {
    pub sha: GitSha,
    pub diff: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitLogEntry {
    pub sha: String,
    pub timestamp: i64,
    pub subject: String,
}

pub async fn collect_git_info(_cwd: &Path) -> Option<GitInfo> {
    None
}

pub async fn current_branch_name(_cwd: &Path) -> Option<String> {
    None
}

pub async fn default_branch_name(_cwd: &Path) -> Option<String> {
    None
}

pub async fn get_git_remote_urls(_cwd: &Path) -> Option<BTreeMap<String, String>> {
    None
}

pub async fn get_git_remote_urls_assume_git_repo(_cwd: &Path) -> Option<BTreeMap<String, String>> {
    None
}

pub fn get_git_repo_root(_base_dir: &Path) -> Option<PathBuf> {
    None
}

pub async fn get_has_changes(_cwd: &Path) -> Option<bool> {
    None
}

pub async fn get_head_commit_hash(_cwd: &Path) -> Option<GitSha> {
    None
}

pub async fn git_diff_to_remote(_cwd: &Path) -> Option<GitDiffToRemote> {
    None
}

pub async fn local_git_branches(_cwd: &Path) -> Vec<String> {
    Vec::new()
}

pub async fn recent_commits(_cwd: &Path, _limit: usize) -> Vec<CommitLogEntry> {
    Vec::new()
}

pub fn resolve_root_git_project_for_trust(_cwd: &Path) -> Option<PathBuf> {
    None
}
