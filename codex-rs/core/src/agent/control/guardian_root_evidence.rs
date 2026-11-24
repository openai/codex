//! Shared Guardian evidence and worker projection. Only trusted hosts may transport these
//! root messages and select a worker path. Never insert them into the worker's conversation.

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use super::root_handoff::RootHandoff;
use super::root_handoff::handoff_boundaries;
use super::root_handoff::handoffs;
use crate::codex_thread::CodexThread;
use crate::codex_thread::GuardianAuthorizationVersion;
use crate::codex_thread::GuardianRootSnapshot;
use crate::context::GuardianReviewEvidence;
use crate::context_manager::ContextManager;
use codex_guardian_context::ContextBudgeting;
use codex_guardian_context::PreparedRootCandidates;
use codex_guardian_context::RootConversationInput;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use serde::Deserialize;
use serde::Serialize;

/// Root evidence reconciled for both budgeting policies, before worker-specific selection.
#[derive(Clone, Serialize, Deserialize)]
pub struct GuardianRootEvidence {
    /// Runtime-local change sequence at capture start. Refresh on a higher sequence.
    pub sequence: u64,
    pub root_thread_id: ThreadId,
    #[serde(flatten)]
    capture: Arc<RootCapture>,
    elastic: Arc<PreparedRootCandidates>,
}

#[derive(Clone, Serialize, Deserialize)]
struct RootCapture {
    history_reset_version: u64,
    #[serde(flatten)]
    authorization_version: GuardianAuthorizationVersion,
    root_review_revision: u64,
    trusted_skill_paths: Vec<String>,
    fixed: PreparedRootCandidates,
    handoffs: Arc<[RootHandoff]>,
}

impl std::fmt::Debug for GuardianRootEvidence {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GuardianRootEvidence")
            .field("root_thread_id", &self.root_thread_id)
            .field("sequence", &self.sequence)
            .finish_non_exhaustive()
    }
}

impl GuardianRootEvidence {
    /// The host must verify the source and resolve the worker's path from session membership.
    pub fn project(&self, worker_path: &AgentPath) -> CodexResult<GuardianRootSnapshot> {
        if worker_path.is_root() {
            return Err(CodexErr::InvalidRequest(
                "Guardian root evidence requires a non-root worker path".into(),
            ));
        }
        let elastic = Arc::clone(&self.elastic);
        Ok(self.capture.as_ref().clone().project(
            self.root_thread_id,
            Some(worker_path),
            move || elastic.as_ref().clone(),
        ))
    }
}

impl CodexThread {
    /// Captures shared root evidence once. As in native review, turn-matched skills
    /// are read after freezing history. Subscribe before capture to observe concurrent changes.
    pub async fn capture_guardian_root_evidence(&self) -> CodexResult<GuardianRootEvidence> {
        let root_thread_id = self.session.thread_id();
        if self.session.session_id() != SessionId::from(root_thread_id) {
            return Err(CodexErr::InvalidRequest(
                "Guardian root evidence must be captured on the root thread".into(),
            ));
        }
        let evidence = self
            .thread_extension_data()
            .get_or_init(GuardianReviewEvidence::default);
        let sequence = evidence.changes.sequence();
        let (mut capture, history) = self
            .capture_root(
                &evidence,
                self.enabled(codex_features::Feature::GuardianRootHandoffContext),
            )
            .await;
        capture.authorization_version.root_runtime_id = Some(self.agent_tree_id().into_bytes());
        Ok(GuardianRootEvidence {
            sequence,
            root_thread_id,
            capture: capture.into(),
            elastic: history
                .guardian_root_candidates(ContextBudgeting::Elastic)
                .into(),
        })
    }

    pub(super) async fn local_guardian_snapshot(
        &self,
        worker_path: Option<&AgentPath>,
    ) -> GuardianRootSnapshot {
        let evidence = self
            .thread_extension_data()
            .get_or_init(GuardianReviewEvidence::default);
        let (capture, history) = self.capture_root(&evidence, worker_path.is_some()).await;
        capture.project(self.session.thread_id(), worker_path, move || {
            history.guardian_root_candidates(ContextBudgeting::Elastic)
        })
    }

    async fn capture_root(
        &self,
        evidence: &GuardianReviewEvidence,
        include_handoffs: bool,
    ) -> (RootCapture, ContextManager) {
        let history = self.session.clone_history().await;
        let handoffs = if include_handoffs {
            handoffs(
                history.annotated_items(),
                self.config().await.multi_agent_v2.tool_namespace.as_deref(),
            )
        } else {
            Vec::new()
        };
        let fixed = history.guardian_root_candidates(ContextBudgeting::Fixed);
        let trusted_skill_paths = fixed
            .latest_user_turn_id
            .as_deref()
            .map(|turn_id| evidence.trusted_skill_paths(turn_id))
            .unwrap_or_default();
        let snapshot = history.conversation_history_snapshot();
        let capture = RootCapture {
            history_reset_version: history.reset_version,
            authorization_version: evidence.authorization_version(snapshot.as_ref()),
            root_review_revision: snapshot.guardian_review_context_revision(),
            trusted_skill_paths,
            fixed,
            handoffs: handoffs.into(),
        };
        (capture, history)
    }
}

impl RootCapture {
    fn project(
        self,
        root_thread_id: ThreadId,
        worker_path: Option<&AgentPath>,
        elastic: impl Fn() -> PreparedRootCandidates + Send + Sync + 'static,
    ) -> GuardianRootSnapshot {
        let boundaries = worker_path
            .map(|path| handoff_boundaries(&self.handoffs, path))
            .unwrap_or_default();
        let request_candidates = RootConversationInput::new(self.fixed, boundaries, elastic);
        let prepared = request_candidates.prepare(ContextBudgeting::Fixed);
        let mut authorization_version = self.authorization_version;
        let mut review_context_revision = self.root_review_revision;
        if prepared.missing_root_instructions {
            authorization_version.retained_context_complete = false;
        }
        if let Some(selected) = &prepared.handoff_selection {
            let mut revision = DefaultHasher::new();
            review_context_revision.hash(&mut revision);
            selected.hash(&mut revision);
            review_context_revision = revision.finish();
        }
        GuardianRootSnapshot {
            root_thread_id,
            history_reset_version: self.history_reset_version,
            authorization_version,
            review_context_revision,
            messages: prepared.messages,
            trusted_skill_paths: self.trusted_skill_paths,
            request_candidates: Some(request_candidates),
        }
    }
}
