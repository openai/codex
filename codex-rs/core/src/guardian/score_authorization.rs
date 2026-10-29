//! Binds async and serving assessments to their captured authorization and model policy.
//! A serving assessment lives with its invocation or code cell, without a separate cache.

use crate::CodexThread;
use crate::GuardianAuthorizationVersion;
use crate::context::GuardianReviewEvidence;

/// Authorization and policy captured for a score; consumers must revalidate before approval.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreAuthorization {
    pub permissions: codex_guardian_context::PermissionContext,
    pub settings: codex_protocol::protocol::ThreadSettingsSnapshot,
    pub environments: Vec<codex_protocol::protocol::TurnEnvironmentSelection>,
    pub local: GuardianAuthorizationVersion,
    pub review_context_revision: u64,
    pub root: Option<GuardianAuthorizationVersion>,
    pub root_review_context_revision: Option<u64>,
    pub model: Option<std::sync::Arc<codex_protocol::openai_models::ModelInfo>>,
}

impl ScoreAuthorization {
    /// Checks the captured authorization against the live thread.
    pub async fn is_current(&self, thread: &CodexThread) -> bool {
        self == &Self::current(thread, &self.permissions).await
    }

    /// Captures the thread authorization and policy for the supplied permission evidence.
    pub async fn current(
        thread: &CodexThread,
        permissions: &codex_guardian_context::PermissionContext,
    ) -> Self {
        Self::for_session(&thread.session, permissions).await
    }

    pub(crate) async fn for_session(
        session: &crate::session::session::Session,
        permissions: &codex_guardian_context::PermissionContext,
    ) -> Self {
        let root = session
            .services
            .agent_control
            .get_guardian_package(session.thread_id)
            .await;
        let history = session.conversation_history_snapshot().await;
        let local = session
            .services
            .thread_extension_data
            .get_or_init(GuardianReviewEvidence::default)
            .authorization_version(history.as_ref());
        Self {
            permissions: permissions.clone(),
            settings: session.thread_settings_snapshot().await,
            environments: session
                .thread_config_snapshot()
                .await
                .environment_selections()
                .to_vec(),
            local,
            review_context_revision: history.guardian_review_context_revision(),
            root: root.as_ref().map(|snapshot| snapshot.authorization_version),
            root_review_context_revision: root.map(|snapshot| snapshot.review_context_revision),
            model: session
                .services
                .thread_extension_data
                .get::<codex_protocol::openai_models::ModelInfo>(),
        }
    }
}

/// One final serving value, retained by the issuing tool and any live code cell.
#[derive(Clone)]
pub(crate) struct ServingAssessment {
    pub(crate) requires_review: bool,
    pub(crate) authorization: ScoreAuthorization,
}
