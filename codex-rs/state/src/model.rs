use anyhow::Result;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;

use crate::paths::parse_timestamp_uuid_from_filename;

/// The sort key to use when listing threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    /// Sort by the thread's creation timestamp.
    CreatedAt,
    /// Sort by the thread's last update timestamp.
    UpdatedAt,
}

/// A pagination anchor used for keyset pagination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// The timestamp component of the anchor.
    pub ts: String,
    /// The UUID component of the anchor.
    pub id: Uuid,
}

/// A single page of thread metadata results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadsPage {
    /// The thread metadata items in this page.
    pub items: Vec<ThreadMetadata>,
    /// The next anchor to use for pagination, if any.
    pub next_anchor: Option<Anchor>,
    /// The number of rows scanned to produce this page.
    pub num_scanned_rows: usize,
}

/// The outcome of extracting metadata from a rollout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionOutcome {
    /// The extracted thread metadata.
    pub metadata: ThreadMetadata,
    /// The number of rollout lines that failed to parse.
    pub parse_errors: usize,
}

/// Canonical thread metadata derived from rollout files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMetadata {
    /// The thread identifier.
    pub id: ThreadId,
    /// The absolute rollout path on disk.
    pub rollout_path: PathBuf,
    /// The creation timestamp in RFC3339 seconds precision.
    pub created_at: String,
    /// The last update timestamp in RFC3339 seconds precision.
    pub updated_at: String,
    /// The session source (stringified enum).
    pub source: String,
    /// The model provider identifier.
    pub model_provider: String,
    /// The working directory for the thread.
    pub cwd: PathBuf,
    /// A best-effort thread title.
    pub title: String,
    /// The sandbox policy (stringified enum).
    pub sandbox_policy: String,
    /// The approval mode (stringified enum).
    pub approval_mode: String,
    /// The last observed token usage.
    pub tokens_used: i64,
    /// The archive timestamp, if the thread is archived.
    pub archived_at: Option<String>,
    /// The git commit SHA, if known.
    pub git_sha: Option<String>,
    /// The git branch name, if known.
    pub git_branch: Option<String>,
    /// The git origin URL, if known.
    pub git_origin_url: Option<String>,
}

impl ThreadMetadata {
    /// Return the list of field names that differ between `self` and `other`.
    pub fn diff_fields(&self, other: &Self) -> Vec<&'static str> {
        let mut diffs = Vec::new();
        if self.id != other.id {
            diffs.push("id");
        }
        if self.rollout_path != other.rollout_path {
            diffs.push("rollout_path");
        }
        if self.created_at != other.created_at {
            diffs.push("created_at");
        }
        if self.updated_at != other.updated_at {
            diffs.push("updated_at");
        }
        if self.source != other.source {
            diffs.push("source");
        }
        if self.model_provider != other.model_provider {
            diffs.push("model_provider");
        }
        if self.cwd != other.cwd {
            diffs.push("cwd");
        }
        if self.title != other.title {
            diffs.push("title");
        }
        if self.sandbox_policy != other.sandbox_policy {
            diffs.push("sandbox_policy");
        }
        if self.approval_mode != other.approval_mode {
            diffs.push("approval_mode");
        }
        if self.tokens_used != other.tokens_used {
            diffs.push("tokens_used");
        }
        if self.archived_at != other.archived_at {
            diffs.push("archived_at");
        }
        if self.git_sha != other.git_sha {
            diffs.push("git_sha");
        }
        if self.git_branch != other.git_branch {
            diffs.push("git_branch");
        }
        if self.git_origin_url != other.git_origin_url {
            diffs.push("git_origin_url");
        }
        diffs
    }

    pub(crate) fn from_path_defaults(path: &Path, default_provider: &str) -> Result<Self> {
        let file_name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| anyhow::anyhow!("rollout path missing file name: {}", path.display()))?;
        let (created_at, uuid) =
            parse_timestamp_uuid_from_filename(file_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "rollout filename missing timestamp/uuid: {}",
                    path.display()
                )
            })?;
        let id = ThreadId::from_string(&uuid.to_string())?;
        let source = crate::extract::enum_to_string(&SessionSource::default());
        let sandbox_policy = crate::extract::enum_to_string(&SandboxPolicy::ReadOnly);
        let approval_mode = crate::extract::enum_to_string(&AskForApproval::OnRequest);
        Ok(Self {
            id,
            rollout_path: path.to_path_buf(),
            created_at: created_at.clone(),
            updated_at: created_at,
            source,
            model_provider: default_provider.to_string(),
            cwd: PathBuf::new(),
            title: String::new(),
            sandbox_policy,
            approval_mode,
            tokens_used: 0,
            archived_at: None,
            git_sha: None,
            git_branch: None,
            git_origin_url: None,
        })
    }
}

/// Statistics about a backfill operation.
#[derive(Debug, Clone)]
pub struct BackfillStats {
    /// The number of rollout files scanned.
    pub scanned: usize,
    /// The number of rows upserted successfully.
    pub upserted: usize,
    /// The number of rows that failed to upsert.
    pub failed: usize,
}
