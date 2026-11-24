//! Shared hosted worker and root evidence provider for Guardian integration tests.
use codex_core::AgentExecutionGuard;
use codex_core::AgentGuardianContext;
use codex_core::AgentRuntimeOwnership;
use codex_core::AgentTurnAdmission;
use codex_core::AgentTurnAdmissionRequest;
use codex_core::GuardianRootEvidence;
use codex_core::GuardianRootSnapshot;
use codex_core::InheritedSessionId;
use codex_core::StartThreadOptions;
use codex_protocol::AgentPath;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::SessionSource;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::Notify;

#[derive(Clone, Copy)]
pub(super) enum HostRead {
    Current,
    Unavailable,
    Reloaded,
    Stalled,
}

pub(super) struct HostGuardianContext {
    evidence: GuardianRootEvidence,
    worker_path: AgentPath,
    pub(super) read: Mutex<HostRead>,
    pub(super) stalled: Notify,
}

impl AgentGuardianContext for HostGuardianContext {
    fn root_snapshot(&self) -> BoxFuture<'_, CodexResult<GuardianRootSnapshot>> {
        Box::pin(async move {
            let mut snapshot = self.evidence.project(&self.worker_path)?;
            let read = *self.read.lock().expect("host read mode");
            match read {
                HostRead::Current => {}
                HostRead::Unavailable => {
                    return Err(CodexErr::Fatal("root context unavailable".into()));
                }
                // Keep every counter unchanged across a root reload.
                HostRead::Reloaded => {
                    snapshot.authorization_version.root_runtime_id = Some([1; 16])
                }
                HostRead::Stalled => {
                    // A concurrent startup prewarm must not release the review gate.
                    if codex_guardian_reviewer::root_evidence_limits().is_some() {
                        self.stalled.notify_one();
                        std::future::pending::<()>().await;
                    }
                }
            }
            Ok(snapshot)
        })
    }
}

struct AllowAdmission;

impl AgentTurnAdmission for AllowAdmission {
    fn admit_turn(
        &self,
        _request: AgentTurnAdmissionRequest,
    ) -> BoxFuture<'_, CodexResult<AgentExecutionGuard>> {
        Box::pin(async { Ok(AgentExecutionGuard::new(())) })
    }
}

impl HostGuardianContext {
    pub(super) fn install(
        options: &mut StartThreadOptions,
        evidence: GuardianRootEvidence,
    ) -> Arc<Self> {
        let context = Arc::new(HostGuardianContext {
            evidence,
            worker_path: AgentPath::try_from("/root/worker").expect("worker path"),
            read: Mutex::new(HostRead::Current),
            stalled: Notify::new(),
        });
        let admission: Arc<dyn AgentTurnAdmission> = Arc::new(AllowAdmission);
        options.session_source = Some(SessionSource::Custom("host".into()));
        options
            .thread_extension_init
            .insert(InheritedSessionId(context.evidence.root_thread_id.into()));
        options
            .thread_extension_init
            .insert(AgentRuntimeOwnership::Host);
        options.thread_extension_init.insert(admission);
        options
            .thread_extension_init
            .insert(context.clone() as Arc<dyn AgentGuardianContext>);
        context
    }
}
