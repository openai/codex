//! Helpers for computing optional per-turn metadata headers.
//!
//! This module owns both metadata construction and the shared timeout policy used by
//! startup websocket prewarm. Turn-time request attachment is handled via a non-blocking
//! background job (`TurnMetadataHeaderJob`) so request send paths never await metadata.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::git_info::get_git_remote_urls_assume_git_repo;
use crate::git_info::get_git_repo_root;
use crate::git_info::get_has_changes;
use crate::git_info::get_head_commit_hash;

pub(crate) const TURN_METADATA_HEADER_TIMEOUT: Duration = Duration::from_millis(250);

/// Resolves turn metadata with a shared timeout policy.
///
/// On timeout, this logs a warning and returns the provided fallback header.
///
/// Keeping this helper centralized avoids drift between startup websocket prewarm and any other
/// timeout-bounded one-shot metadata call sites.
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
    has_changes: Option<bool>,
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

    let (latest_git_commit_hash, associated_remote_urls, has_changes) = tokio::join!(
        get_head_commit_hash(cwd),
        get_git_remote_urls_assume_git_repo(cwd),
        get_has_changes(cwd)
    );
    if latest_git_commit_hash.is_none()
        && associated_remote_urls.is_none()
        && has_changes.is_none()
        && sandbox.is_none()
    {
        return None;
    }

    let mut workspaces = BTreeMap::new();
    if let Some(repo_root) = repo_root {
        workspaces.insert(
            repo_root.to_string_lossy().into_owned(),
            TurnMetadataWorkspace {
                associated_remote_urls,
                has_changes,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TurnMetadataPoll {
    Pending,
    Ready(Option<String>),
}

#[derive(Debug, Default)]
enum TurnMetadataHeaderJobState {
    #[default]
    NotStarted,
    Pending {
        receiver: oneshot::Receiver<Option<String>>,
        task: JoinHandle<()>,
    },
    Ready(Option<String>),
}

#[derive(Debug, Default)]
pub(crate) struct TurnMetadataHeaderJob {
    state: Mutex<TurnMetadataHeaderJobState>,
}

impl TurnMetadataHeaderJob {
    pub(crate) fn spawn(&self, cwd: PathBuf, sandbox: Option<String>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !matches!(*state, TurnMetadataHeaderJobState::NotStarted) {
            return;
        }

        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            *state = TurnMetadataHeaderJobState::Ready(None);
            return;
        };

        let (tx, rx) = oneshot::channel::<Option<String>>();
        let task = handle.spawn(async move {
            let header = build_turn_metadata_header(cwd.as_path(), sandbox.as_deref()).await;
            let _ = tx.send(header);
        });
        *state = TurnMetadataHeaderJobState::Pending { receiver: rx, task };
    }

    pub(crate) fn poll(&self) -> TurnMetadataPoll {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &mut *state {
            TurnMetadataHeaderJobState::NotStarted => TurnMetadataPoll::Pending,
            TurnMetadataHeaderJobState::Ready(header) => TurnMetadataPoll::Ready(header.clone()),
            TurnMetadataHeaderJobState::Pending { receiver, .. } => match receiver.try_recv() {
                Ok(header) => {
                    *state = TurnMetadataHeaderJobState::Ready(header.clone());
                    TurnMetadataPoll::Ready(header)
                }
                Err(TryRecvError::Empty) => TurnMetadataPoll::Pending,
                Err(TryRecvError::Closed) => {
                    *state = TurnMetadataHeaderJobState::Ready(None);
                    TurnMetadataPoll::Ready(None)
                }
            },
        }
    }

    pub(crate) fn cancel(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let TurnMetadataHeaderJobState::Pending { task, .. } = &mut *state {
            task.abort();
        }
        *state = TurnMetadataHeaderJobState::Ready(None);
    }
}

impl Drop for TurnMetadataHeaderJob {
    fn drop(&mut self) {
        let Ok(state) = self.state.get_mut() else {
            return;
        };
        if let TurnMetadataHeaderJobState::Pending { task, .. } = state {
            task.abort();
        }
    }
}
