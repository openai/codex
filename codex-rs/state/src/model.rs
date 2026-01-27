use chrono::DateTime;
use chrono::Timelike;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use std::path::PathBuf;
use uuid::Uuid;

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
    pub ts: DateTime<Utc>,
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
    /// The creation timestamp.
    pub created_at: DateTime<Utc>,
    /// The last update timestamp.
    pub updated_at: DateTime<Utc>,
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
    pub archived_at: Option<DateTime<Utc>>,
    /// The git commit SHA, if known.
    pub git_sha: Option<String>,
    /// The git branch name, if known.
    pub git_branch: Option<String>,
    /// The git origin URL, if known.
    pub git_origin_url: Option<String>,
}

/// Builder data required to construct [`ThreadMetadata`] without parsing filenames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMetadataBuilder {
    /// The thread identifier.
    pub id: ThreadId,
    /// The absolute rollout path on disk.
    pub rollout_path: PathBuf,
    /// The creation timestamp.
    pub created_at: DateTime<Utc>,
    /// The last update timestamp, if known.
    pub updated_at: Option<DateTime<Utc>>,
    /// The session source.
    pub source: SessionSource,
    /// The model provider identifier, if known.
    pub model_provider: Option<String>,
    /// The working directory for the thread.
    pub cwd: PathBuf,
    /// The sandbox policy.
    pub sandbox_policy: SandboxPolicy,
    /// The approval mode.
    pub approval_mode: AskForApproval,
    /// The archive timestamp, if the thread is archived.
    pub archived_at: Option<DateTime<Utc>>,
    /// The git commit SHA, if known.
    pub git_sha: Option<String>,
    /// The git branch name, if known.
    pub git_branch: Option<String>,
    /// The git origin URL, if known.
    pub git_origin_url: Option<String>,
}

impl ThreadMetadataBuilder {
    /// Create a new builder with required fields and sensible defaults.
    pub fn new(
        id: ThreadId,
        rollout_path: PathBuf,
        created_at: DateTime<Utc>,
        source: SessionSource,
    ) -> Self {
        Self {
            id,
            rollout_path,
            created_at,
            updated_at: None,
            source,
            model_provider: None,
            cwd: PathBuf::new(),
            sandbox_policy: SandboxPolicy::ReadOnly,
            approval_mode: AskForApproval::OnRequest,
            archived_at: None,
            git_sha: None,
            git_branch: None,
            git_origin_url: None,
        }
    }

    /// Build canonical thread metadata, filling missing values from defaults.
    pub fn build(&self, default_provider: &str) -> ThreadMetadata {
        let source = crate::extract::enum_to_string(&self.source);
        let sandbox_policy = crate::extract::enum_to_string(&self.sandbox_policy);
        let approval_mode = crate::extract::enum_to_string(&self.approval_mode);
        let created_at = canonicalize_datetime(self.created_at);
        let updated_at = self
            .updated_at
            .map(canonicalize_datetime)
            .unwrap_or(created_at);
        ThreadMetadata {
            id: self.id,
            rollout_path: self.rollout_path.clone(),
            created_at,
            updated_at,
            source,
            model_provider: self
                .model_provider
                .clone()
                .unwrap_or_else(|| default_provider.to_string()),
            cwd: self.cwd.clone(),
            title: String::new(),
            sandbox_policy,
            approval_mode,
            tokens_used: 0,
            archived_at: self.archived_at.map(canonicalize_datetime),
            git_sha: self.git_sha.clone(),
            git_branch: self.git_branch.clone(),
            git_origin_url: self.git_origin_url.clone(),
        }
    }
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
}

fn canonicalize_datetime(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.with_nanosecond(0).unwrap_or(dt)
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
