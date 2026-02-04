use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::git_info::get_git_remote_urls_assume_git_repo;
use crate::git_info::get_git_repo_root;
use crate::git_info::get_head_commit_hash;

#[derive(Serialize)]
struct TurnMetadataWorkspace {
    #[serde(skip_serializing_if = "Option::is_none")]
    associated_remote_urls: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_git_commit_hash: Option<String>,
}

#[derive(Serialize)]
struct TurnMetadata {
    workspaces: BTreeMap<String, TurnMetadataWorkspace>,
}

pub async fn build_turn_metadata_header(cwd: &Path) -> Option<String> {
    let repo_root = get_git_repo_root(cwd)?;

    let (latest_git_commit_hash, associated_remote_urls) = tokio::join!(
        get_head_commit_hash(cwd),
        get_git_remote_urls_assume_git_repo(cwd)
    );
    if latest_git_commit_hash.is_none() && associated_remote_urls.is_none() {
        return None;
    }

    let mut workspaces = BTreeMap::new();
    workspaces.insert(
        repo_root.to_string_lossy().into_owned(),
        TurnMetadataWorkspace {
            associated_remote_urls,
            latest_git_commit_hash,
        },
    );
    serde_json::to_string(&TurnMetadata { workspaces }).ok()
}
