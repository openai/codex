//! Turn-scoped state and active turn metadata scaffolding.

use indexmap::IndexMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::models::ApprovalSourceMetadata;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItemMetadata;
use codex_protocol::models::ReviewDecisionMetadata;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::ElicitationResponse;
use rmcp::model::RequestId;
use tokio::sync::oneshot;

use crate::codex::TurnContext;
use crate::protocol::ReviewDecision;
use crate::protocol::TokenUsage;
use crate::sandboxing::merge_permission_profiles;
use crate::tasks::SessionTask;
use codex_protocol::models::PermissionProfile;

/// Metadata about the currently running turn.
pub(crate) struct ActiveTurn {
    pub(crate) tasks: IndexMap<String, RunningTask>,
    pub(crate) turn_state: Arc<Mutex<TurnState>>,
}

impl Default for ActiveTurn {
    fn default() -> Self {
        Self {
            tasks: IndexMap::new(),
            turn_state: Arc::new(Mutex::new(TurnState::default())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskKind {
    Regular,
    Review,
    Compact,
}

pub(crate) struct RunningTask {
    pub(crate) done: Arc<Notify>,
    pub(crate) kind: TaskKind,
    pub(crate) task: Arc<dyn SessionTask>,
    pub(crate) cancellation_token: CancellationToken,
    pub(crate) handle: Arc<AbortOnDropHandle<()>>,
    pub(crate) turn_context: Arc<TurnContext>,
    // Timer recorded when the task drops to capture the full turn duration.
    pub(crate) _timer: Option<codex_otel::Timer>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingInputItem {
    pub(crate) input: ResponseInputItem,
    pub(crate) metadata: Option<ResponseItemMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingApprovalMetadata {
    pub(crate) call_id: String,
    pub(crate) approval_source: ApprovalSourceMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ApprovalOutcomeMetadata {
    pub(crate) review_decision: Option<ReviewDecisionMetadata>,
    pub(crate) approval_source: ApprovalSourceMetadata,
}

impl ApprovalOutcomeMetadata {
    pub(crate) fn reviewed(
        decision: &ReviewDecision,
        approval_source: ApprovalSourceMetadata,
    ) -> Self {
        let review_decision = match decision {
            ReviewDecision::Approved => ReviewDecisionMetadata::Approved,
            ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
                ReviewDecisionMetadata::ApprovedWithAmendment
            }
            ReviewDecision::Denied => ReviewDecisionMetadata::Denied,
            ReviewDecision::Abort => ReviewDecisionMetadata::Abort,
            ReviewDecision::ApprovedForSession => ReviewDecisionMetadata::ApprovedForSession,
            ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment,
            } => match network_policy_amendment.action {
                codex_protocol::protocol::NetworkPolicyRuleAction::Allow => {
                    ReviewDecisionMetadata::ApprovedWithNetworkPolicyAllow
                }
                codex_protocol::protocol::NetworkPolicyRuleAction::Deny => {
                    ReviewDecisionMetadata::DeniedWithNetworkPolicyDeny
                }
            },
        };
        Self {
            review_decision: Some(review_decision),
            approval_source,
        }
    }
}

impl ActiveTurn {
    pub(crate) fn add_task(&mut self, task: RunningTask) {
        let sub_id = task.turn_context.sub_id.clone();
        self.tasks.insert(sub_id, task);
    }

    pub(crate) fn remove_task(&mut self, sub_id: &str) -> bool {
        self.tasks.swap_remove(sub_id);
        self.tasks.is_empty()
    }

    pub(crate) fn drain_tasks(&mut self) -> Vec<RunningTask> {
        self.tasks.drain(..).map(|(_, task)| task).collect()
    }
}

/// Mutable state for a single turn.
#[derive(Default)]
pub(crate) struct TurnState {
    pending_approvals: HashMap<String, oneshot::Sender<ReviewDecision>>,
    pending_approval_metadata_by_id: HashMap<String, PendingApprovalMetadata>,
    approval_outcomes_by_call_id: HashMap<String, ApprovalOutcomeMetadata>,
    pending_request_permissions: HashMap<String, oneshot::Sender<RequestPermissionsResponse>>,
    pending_user_input: HashMap<String, oneshot::Sender<RequestUserInputResponse>>,
    pending_elicitations: HashMap<(String, RequestId), oneshot::Sender<ElicitationResponse>>,
    pending_dynamic_tools: HashMap<String, oneshot::Sender<DynamicToolResponse>>,
    pending_input: Vec<PendingInputItem>,
    granted_permissions: Option<PermissionProfile>,
    pub(crate) tool_calls: u64,
    pub(crate) token_usage_at_turn_start: TokenUsage,
}

impl TurnState {
    pub(crate) fn insert_pending_approval(
        &mut self,
        key: String,
        tx: oneshot::Sender<ReviewDecision>,
    ) -> Option<oneshot::Sender<ReviewDecision>> {
        self.pending_approvals.insert(key, tx)
    }

    pub(crate) fn insert_pending_approval_call_id(
        &mut self,
        approval_key: String,
        pending_metadata: PendingApprovalMetadata,
    ) -> Option<PendingApprovalMetadata> {
        self.pending_approval_metadata_by_id
            .insert(approval_key, pending_metadata)
    }

    pub(crate) fn remove_pending_approval_call_id(
        &mut self,
        approval_key: &str,
    ) -> Option<PendingApprovalMetadata> {
        self.pending_approval_metadata_by_id.remove(approval_key)
    }

    pub(crate) fn record_approval_outcome(
        &mut self,
        call_id: String,
        outcome: ApprovalOutcomeMetadata,
    ) {
        self.approval_outcomes_by_call_id.insert(call_id, outcome);
    }

    pub(crate) fn approval_metadata_snapshot(
        &self,
    ) -> (HashMap<String, ApprovalOutcomeMetadata>, HashSet<String>) {
        (
            self.approval_outcomes_by_call_id.clone(),
            self.pending_approval_metadata_by_id
                .values()
                .map(|metadata| metadata.call_id.clone())
                .collect(),
        )
    }

    pub(crate) fn remove_pending_approval(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<ReviewDecision>> {
        self.pending_approvals.remove(key)
    }

    pub(crate) fn clear_pending(&mut self) {
        self.pending_approvals.clear();
        self.pending_approval_metadata_by_id.clear();
        self.approval_outcomes_by_call_id.clear();
        self.pending_request_permissions.clear();
        self.pending_user_input.clear();
        self.pending_elicitations.clear();
        self.pending_dynamic_tools.clear();
        self.pending_input.clear();
    }

    pub(crate) fn insert_pending_request_permissions(
        &mut self,
        key: String,
        tx: oneshot::Sender<RequestPermissionsResponse>,
    ) -> Option<oneshot::Sender<RequestPermissionsResponse>> {
        self.pending_request_permissions.insert(key, tx)
    }

    pub(crate) fn remove_pending_request_permissions(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<RequestPermissionsResponse>> {
        self.pending_request_permissions.remove(key)
    }

    pub(crate) fn insert_pending_user_input(
        &mut self,
        key: String,
        tx: oneshot::Sender<RequestUserInputResponse>,
    ) -> Option<oneshot::Sender<RequestUserInputResponse>> {
        self.pending_user_input.insert(key, tx)
    }

    pub(crate) fn remove_pending_user_input(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<RequestUserInputResponse>> {
        self.pending_user_input.remove(key)
    }

    pub(crate) fn insert_pending_elicitation(
        &mut self,
        server_name: String,
        request_id: RequestId,
        tx: oneshot::Sender<ElicitationResponse>,
    ) -> Option<oneshot::Sender<ElicitationResponse>> {
        self.pending_elicitations
            .insert((server_name, request_id), tx)
    }

    pub(crate) fn remove_pending_elicitation(
        &mut self,
        server_name: &str,
        request_id: &RequestId,
    ) -> Option<oneshot::Sender<ElicitationResponse>> {
        self.pending_elicitations
            .remove(&(server_name.to_string(), request_id.clone()))
    }

    pub(crate) fn insert_pending_dynamic_tool(
        &mut self,
        key: String,
        tx: oneshot::Sender<DynamicToolResponse>,
    ) -> Option<oneshot::Sender<DynamicToolResponse>> {
        self.pending_dynamic_tools.insert(key, tx)
    }

    pub(crate) fn remove_pending_dynamic_tool(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<DynamicToolResponse>> {
        self.pending_dynamic_tools.remove(key)
    }

    pub(crate) fn push_pending_input(
        &mut self,
        input: ResponseInputItem,
        metadata: Option<ResponseItemMetadata>,
    ) {
        self.pending_input
            .push(PendingInputItem { input, metadata });
    }

    pub(crate) fn prepend_pending_input_with_metadata(&mut self, mut input: Vec<PendingInputItem>) {
        if input.is_empty() {
            return;
        }

        input.append(&mut self.pending_input);
        self.pending_input = input;
    }

    pub(crate) fn take_pending_input_with_metadata(&mut self) -> Vec<PendingInputItem> {
        if self.pending_input.is_empty() {
            Vec::with_capacity(0)
        } else {
            let mut ret = Vec::new();
            std::mem::swap(&mut ret, &mut self.pending_input);
            ret
        }
    }

    pub(crate) fn has_pending_input(&self) -> bool {
        !self.pending_input.is_empty()
    }

    pub(crate) fn record_granted_permissions(&mut self, permissions: PermissionProfile) {
        self.granted_permissions =
            merge_permission_profiles(self.granted_permissions.as_ref(), Some(&permissions));
    }

    pub(crate) fn granted_permissions(&self) -> Option<PermissionProfile> {
        self.granted_permissions.clone()
    }
}

impl ActiveTurn {
    /// Clear any pending approvals and input buffered for the current turn.
    pub(crate) async fn clear_pending(&self) {
        let mut ts = self.turn_state.lock().await;
        ts.clear_pending();
    }
}
