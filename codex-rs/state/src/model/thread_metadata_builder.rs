//! Constructs canonical metadata from explicit creation inputs and optional attributes.

use super::ThreadMetadata;
use super::thread_metadata::canonicalize_datetime;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::SanitizedGitUrl;
use codex_protocol::ThreadId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSource;
use std::path::PathBuf;

/// Builds metadata without parsing filenames. ID, rollout path, creation time, and source are required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMetadataBuilder {
    id: ThreadId,
    rollout_path: PathBuf,
    created_at: DateTime<Utc>,
    source: SessionSource,
    originator: Option<String>,
    creator_user_id: Option<String>,
    creator_account_id: Option<String>,
    history_mode: ThreadHistoryMode,
    thread_source: Option<ThreadSource>,
    agent_nickname: Option<String>,
    agent_role: Option<String>,
    agent_path: Option<String>,
    model_provider: Option<String>,
    cwd: PathBuf,
    cli_version: String,
    archived_at: Option<DateTime<Utc>>,
    git_sha: Option<String>,
    git_branch: Option<String>,
    git_origin_url: Option<SanitizedGitUrl>,
}

impl ThreadMetadataBuilder {
    /// Create a builder with required creation inputs and optional-field defaults.
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
            source,
            originator: None,
            creator_user_id: None,
            creator_account_id: None,
            history_mode: ThreadHistoryMode::Legacy,
            thread_source: None,
            agent_nickname: None,
            agent_role: None,
            agent_path: None,
            model_provider: None,
            cwd: PathBuf::new(),
            cli_version: String::new(),
            archived_at: None,
            git_sha: None,
            git_branch: None,
            git_origin_url: None,
        }
    }

    pub fn with_originator(mut self, originator: Option<String>) -> Self {
        self.originator = originator;
        self
    }

    pub fn with_creator_user_id(mut self, creator_user_id: Option<String>) -> Self {
        self.creator_user_id = creator_user_id;
        self
    }

    pub fn with_creator_account_id(mut self, creator_account_id: Option<String>) -> Self {
        self.creator_account_id = creator_account_id;
        self
    }

    pub fn with_history_mode(mut self, history_mode: ThreadHistoryMode) -> Self {
        self.history_mode = history_mode;
        self
    }

    pub fn with_thread_source(mut self, thread_source: Option<ThreadSource>) -> Self {
        self.thread_source = thread_source;
        self
    }

    pub fn with_agent_nickname(mut self, agent_nickname: Option<String>) -> Self {
        self.agent_nickname = agent_nickname;
        self
    }

    pub fn with_agent_role(mut self, agent_role: Option<String>) -> Self {
        self.agent_role = agent_role;
        self
    }

    pub fn with_agent_path(mut self, agent_path: Option<String>) -> Self {
        self.agent_path = agent_path;
        self
    }

    pub fn with_model_provider(mut self, model_provider: Option<String>) -> Self {
        self.model_provider = model_provider;
        self
    }

    pub fn with_cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = cwd;
        self
    }

    pub fn with_cli_version(mut self, cli_version: String) -> Self {
        self.cli_version = cli_version;
        self
    }

    pub fn with_archived_at(mut self, archived_at: Option<DateTime<Utc>>) -> Self {
        self.archived_at = archived_at;
        self
    }

    pub fn with_git_sha(mut self, git_sha: Option<String>) -> Self {
        self.git_sha = git_sha;
        self
    }

    pub fn with_git_branch(mut self, git_branch: Option<String>) -> Self {
        self.git_branch = git_branch;
        self
    }

    pub fn with_git_origin_url(mut self, git_origin_url: Option<SanitizedGitUrl>) -> Self {
        self.git_origin_url = git_origin_url;
        self
    }

    /// Build canonical thread metadata, filling missing values from defaults.
    pub fn build(&self, default_provider: &str) -> ThreadMetadata {
        let source = crate::extract::enum_to_string(&self.source);
        let approval_mode = crate::extract::enum_to_string(&AskForApproval::OnRequest);
        let created_at = canonicalize_datetime(self.created_at);
        let guardian_review = crate::is_guardian_review_source(&self.source);
        ThreadMetadata {
            attached_thread_parent_id: None,
            originator: self.originator.clone(),
            creator_user_id: self.creator_user_id.clone(),
            creator_account_id: self.creator_account_id.clone(),
            id: self.id,
            rollout_path: self.rollout_path.clone(),
            created_at,
            updated_at: created_at,
            recency_at: created_at,
            source,
            history_mode: self.history_mode,
            thread_source: self.thread_source.clone(),
            agent_nickname: self.agent_nickname.clone(),
            agent_role: self.agent_role.clone(),
            agent_path: self
                .agent_path
                .clone()
                .or_else(|| self.source.get_agent_path().map(Into::into)),
            model_provider: self
                .model_provider
                .clone()
                .unwrap_or_else(|| default_provider.to_string()),
            model: None,
            reasoning_effort: None,
            cwd: self.cwd.clone(),
            cli_version: self.cli_version.clone(),
            title: if guardian_review {
                crate::GUARDIAN_THREAD_TITLE.to_string()
            } else {
                String::new()
            },
            name: (guardian_review && self.history_mode == ThreadHistoryMode::Paginated)
                .then(|| crate::GUARDIAN_THREAD_TITLE.to_string()),
            preview: guardian_review.then(|| crate::GUARDIAN_THREAD_PREVIEW.to_string()),
            sandbox_policy: serde_json::to_string(&PermissionProfile::read_only())
                .unwrap_or_default(),
            approval_mode,
            tokens_used: 0,
            first_user_message: None,
            archived_at: self.archived_at.map(canonicalize_datetime),
            section: None,
            section_position: None,
            section_entered_at: None,
            project_id: None,
            daybreak_enabled: None,
            git_sha: self.git_sha.clone(),
            git_branch: self.git_branch.clone(),
            git_origin_url: self.git_origin_url.clone(),
        }
    }
}
