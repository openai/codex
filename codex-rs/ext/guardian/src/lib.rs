use std::sync::Arc;

use codex_extension_api::AgentSpawnFuture;
use codex_extension_api::AgentSpawner;
use codex_extension_api::ApprovalReviewContributor;
use codex_extension_api::ApprovalReviewInput;
use codex_extension_api::ApprovalReviewOutcome;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;

mod approval_request;
mod truncation;

pub use approval_request::FormattedGuardianAction;
pub use approval_request::format_guardian_action_pretty;
pub use approval_request::guardian_approval_request_to_json;
pub use approval_request::guardian_assessment_action;
pub use approval_request::guardian_request_target_item_id;
pub use approval_request::guardian_request_turn_id;
pub use approval_request::guardian_reviewed_action;
pub use truncation::guardian_truncate_text;

/// Guardian extension dependencies supplied by the host at construction time.
#[derive(Clone, Debug)]
pub struct GuardianExtension<S> {
    agent_spawner: S,
}

impl<S> GuardianExtension<S> {
    /// Creates a guardian extension with its host-provided agent spawn helper.
    pub fn new(agent_spawner: S) -> Self {
        Self { agent_spawner }
    }

    /// Delegates one guardian-owned subagent spawn request to the host helper.
    pub fn spawn_subagent<'a, R>(
        &'a self,
        forked_from_thread_id: ThreadId,
        request: R,
    ) -> AgentSpawnFuture<'a, <S as AgentSpawner<R>>::Spawned, <S as AgentSpawner<R>>::Error>
    where
        S: AgentSpawner<R>,
    {
        self.agent_spawner
            .spawn_subagent(forked_from_thread_id, request)
    }
}

/// Thread-local guardian state captured when the host starts a thread.
#[derive(Clone, Copy, Debug)]
pub struct GuardianThreadContext {
    forked_from_thread_id: ThreadId,
}

impl GuardianThreadContext {
    /// Returns the thread that future guardian subagents should fork from by default.
    pub fn forked_from_thread_id(&self) -> ThreadId {
        self.forked_from_thread_id
    }
}

impl<C, S> ThreadLifecycleContributor<C> for GuardianExtension<S>
where
    C: Sync,
    S: Send + Sync,
{
    fn on_thread_start<'a>(&'a self, input: ThreadStartInput<'a, C>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let Ok(forked_from_thread_id) = ThreadId::from_string(input.thread_store.level_id())
            else {
                return;
            };
            input.thread_store.insert(GuardianThreadContext {
                forked_from_thread_id,
            });
        })
    }
}

impl<S> ApprovalReviewContributor for GuardianExtension<S>
where
    S: Send + Sync,
{
    fn review<'a>(
        &'a self,
        input: ApprovalReviewInput<'a>,
    ) -> ExtensionFuture<'a, Result<ApprovalReviewOutcome, codex_extension_api::ApprovalReviewError>>
    {
        Box::pin(async move {
            if input.reviewer != ApprovalsReviewer::AutoReview
                || !matches!(
                    input.approval_policy,
                    AskForApproval::OnRequest | AskForApproval::Granular(_)
                )
            {
                return Ok(ApprovalReviewOutcome::Abstain);
            }
            input.runner.run().await
        })
    }
}

/// Installs the guardian contributors into the extension registry.
pub fn install<C, S>(registry: &mut ExtensionRegistryBuilder<C>, agent_spawner: S)
where
    C: Sync,
    S: Send + Sync + 'static,
{
    let extension = Arc::new(GuardianExtension::new(agent_spawner));
    registry.thread_lifecycle_contributor(extension.clone());
    registry.approval_review_contributor(extension);
}
