//! Selects authoritative root evidence and its reviewer reuse version.

use crate::GuardianRootSnapshot;
use crate::agent::host::AgentRuntimeServices;
use codex_guardian_reviewer::ReviewContextPolicy;
use codex_guardian_reviewer::ReviewDeadline;
use codex_protocol::SessionId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;

use crate::codex_thread::GuardianAuthorizationVersion;
use crate::session::session::Session;

impl Session {
    pub(crate) fn host_guardian_context(&self) -> Option<&dyn crate::AgentGuardianContext> {
        if let AgentRuntimeServices::Host(services) = &self.agent_runtime
            && self.session_id() != SessionId::from(self.thread_id)
        {
            services.guardian_context.as_deref()
        } else {
            None
        }
    }

    pub(crate) async fn guardian_root_snapshot(&self) -> Result<Option<GuardianRootSnapshot>> {
        if let Some(provider) = self.host_guardian_context() {
            let (deadline, cancel) = match codex_guardian_reviewer::root_evidence_limits() {
                Some(limits) => limits,
                // Background scoring and prewarm can run outside an approval.
                None => (
                    ReviewDeadline::from_timeout(self.get_config().await.guardian_review_timeout),
                    self.history_reset().await.1,
                ),
            };
            let snapshot = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(CodexErr::TurnAborted),
                _ = deadline.wait() => return Err(CodexErr::Timeout),
                result = provider.root_snapshot() => result
                    .map_err(|_| CodexErr::Fatal("Guardian root evidence unavailable".to_owned()))?,
            };
            if SessionId::from(snapshot.root_thread_id) != self.session_id()
                || snapshot.authorization_version.root_runtime_id.is_none()
                || snapshot.request_candidates.is_none()
            {
                return Err(CodexErr::InvalidRequest(
                    "Guardian evidence requires the session root ID, root runtime ID and request candidates"
                        .into(),
                ));
            }
            return Ok(Some(snapshot));
        }
        Ok(self
            .services
            .agent_control
            .get_guardian_package(self.thread_id)
            .await)
    }
}

pub(super) async fn root_review_version(
    policy: ReviewContextPolicy,
    session: &Session,
) -> Result<Option<(GuardianAuthorizationVersion, u64)>> {
    if matches!(
        policy,
        ReviewContextPolicy::Legacy | ReviewContextPolicy::LegacyWithCheckpointReuse
    ) {
        return Ok(None);
    }
    Ok(session.guardian_root_snapshot().await?.map(|snapshot| {
        (
            snapshot.authorization_version,
            snapshot.review_context_revision,
        )
    }))
}
