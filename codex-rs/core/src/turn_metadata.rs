//! Helpers for computing and resolving optional per-turn metadata headers.
//!
//! This module owns both metadata construction and the shared timeout policy used by
//! turn execution and startup websocket prewarm. Keeping timeout behavior centralized
//! ensures both call sites treat timeout as the same best-effort fallback condition.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tracing::warn;

use crate::git_info::get_git_remote_urls_assume_git_repo;
use crate::git_info::get_git_repo_root;
use crate::git_info::get_head_commit_hash;

pub(crate) const TURN_METADATA_HEADER_TIMEOUT: Duration = Duration::from_millis(250);

/// Resolves turn metadata with a shared timeout policy.
///
/// On timeout, this logs a warning and returns the provided fallback header.
///
/// Keeping this helper centralized avoids drift between turn-time metadata resolution and startup
/// websocket prewarm, both of which need identical timeout semantics.
pub(crate) async fn resolve_turn_metadata_header_with_timeout<F>(
    build_header: F,
    fallback_on_timeout: Option<String>,
) -> Option<String>
where
    F: Future<Output = Option<String>>,
{
    match tokio::time::timeout(TURN_METADATA_HEADER_TIMEOUT, build_header).await {
        Ok(header) => header,
        Err(_) => {
            warn!(
                "timed out after {}ms while building turn metadata header",
                TURN_METADATA_HEADER_TIMEOUT.as_millis()
            );
            fallback_on_timeout
        }
    }
}

#[derive(Serialize)]
struct TurnMetadataWorkspace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    associated_remote_urls: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest_git_commit_hash: Option<String>,
}

#[derive(Serialize)]
struct TurnMetadata {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    workspaces: BTreeMap<String, TurnMetadataWorkspace>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sandbox: Option<String>,
}

pub async fn build_turn_metadata_header(cwd: &Path, sandbox: Option<&str>) -> Option<String> {
    let repo_root = get_git_repo_root(cwd);

    let (latest_git_commit_hash, associated_remote_urls) = tokio::join!(
        get_head_commit_hash(cwd),
        get_git_remote_urls_assume_git_repo(cwd)
    );
    if latest_git_commit_hash.is_none() && associated_remote_urls.is_none() && sandbox.is_none() {
        return None;
    }

    let mut workspaces = BTreeMap::new();
    if let Some(repo_root) = repo_root {
        workspaces.insert(
            repo_root.to_string_lossy().into_owned(),
            TurnMetadataWorkspace {
                associated_remote_urls,
                latest_git_commit_hash,
            },
        );
    }
    serde_json::to_string(&TurnMetadata {
        workspaces,
        sandbox: sandbox.map(ToString::to_string),
    })
    .ok()
}
