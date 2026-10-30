//! Turn-scoped state, input waiters, and active turn metadata.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as SyncMutex;
use std::sync::PoisonError;
use std::sync::Weak;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

use codex_diagnostics::GaugeGuard;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::ElicitationResponse;
use rmcp::model::RequestId;
use tokio::sync::oneshot;

use super::TurnTokenUsage;
use crate::agent::types::AgentExecutionGuard;
use crate::session::TurnInputQueue;
use crate::session::step_context::StepContext;
use crate::session::turn_context::TurnContext;
use crate::session::turn_context::TurnEnvironment;
use crate::tasks::AnySessionTask;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::TokenUsage;

/// Metadata about the currently running turn.
pub(crate) struct ActiveTurn {
    pub(crate) task: Option<RunningTask>,
    /// Present when input admission is closed; cancelled when the final batch is recorded.
    /// Reservations that have not started still accept inputs.
    pub(crate) finishing: Option<CancellationToken>,
    pub(crate) turn_state: Arc<Mutex<TurnState>>,
}

/// Whether mailbox deliveries should still be folded into the current turn.
///
/// State machine:
/// - A turn starts in `CurrentTurn`, so queued child mail can join the next
///   model request for that turn.
/// - After a terminal answer is recorded, even an empty one, we switch to
///   `NextTurn` to leave late child mail queued instead of extending the answer.
/// - If the same task later gets explicit same-turn work again (a steered user
///   prompt or a tool call after an untagged preamble), we reopen `CurrentTurn`
///   so that pending child mail is drained into that follow-up request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum MailboxDeliveryPhase {
    /// Incoming mailbox messages can still be consumed by the current turn.
    #[default]
    CurrentTurn,
    /// The current turn already emitted a terminal answer; mailbox
    /// messages should remain queued for a later turn.
    NextTurn,
}

impl Default for ActiveTurn {
    fn default() -> Self {
        Self {
            task: None,
            finishing: None,
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
    pub(crate) task: Arc<dyn AnySessionTask>,
    pub(crate) cancellation_token: CancellationToken,
    pub(crate) handle: AbortOnDropHandle<()>,
    pub(crate) turn_context: Arc<TurnContext>,
    pub(crate) _agent_execution_guard: Option<AgentExecutionGuard>,
    pub(crate) _diagnostics_guard: GaugeGuard,
    // Timer recorded when the task drops to capture the full turn duration.
    pub(crate) _timer: Option<codex_otel::Timer>,
}

/// Mutable state for a single turn.
#[derive(Default)]
pub(crate) struct TurnState {
    pending_approvals: Arc<PendingApprovals>,
    pending_request_permissions: HashMap<String, PendingRequestPermissions>,
    pending_user_input: HashMap<String, oneshot::Sender<AcceptedUserInputResponse>>,
    pending_elicitations: HashMap<(String, RequestId), oneshot::Sender<ElicitationResponse>>,
    pending_dynamic_tools: HashMap<String, oneshot::Sender<DynamicToolResponse>>,
    pub(crate) pending_input: TurnInputQueue,
    mailbox_delivery_phase: MailboxDeliveryPhase,
    pub(crate) tool_calls: u64,
    pub(crate) has_memory_citation: bool,
    pub(crate) token_usage_at_turn_start: TokenUsage,
    pub(crate) token_usage_by_model: TurnTokenUsage,
    /// The last step captured for execution or selected from a speculative fallback.
    /// Remains absent until a step is captured; standalone local compaction has no step.
    pub(crate) last_known_step_context: Option<Arc<StepContext>>,
}

pub(crate) struct PendingApproval {
    pub(crate) sender: oneshot::Sender<ReviewDecision>,
    pub(crate) kind: codex_protocol::approvals::ExecApprovalKind,
    // A yielded cell can emit a prompt for its original turn while a later turn services it.
    pub(crate) turn_id: String,
}

impl PendingApproval {
    pub(crate) fn validate(&self, decision: ReviewDecision) -> ReviewDecision {
        if self.kind == codex_protocol::approvals::ExecApprovalKind::HumanCommand
            && !matches!(
                decision,
                ReviewDecision::Approved | ReviewDecision::Denied { .. } | ReviewDecision::Abort
            )
        {
            ReviewDecision::denied("Managed command policy requires a one-time human approval.")
        } else {
            decision
        }
    }
}

/// Host receipt metadata follows the response through the asynchronous tool waiter.
pub(crate) struct AcceptedUserInputResponse {
    pub(crate) response: RequestUserInputResponse,
    /// Order reserved when the host accepts this response.
    pub(crate) acceptance_order: u64,
}

pub(crate) struct PendingRequestPermissions {
    pub(crate) tx_response: oneshot::Sender<RequestPermissionsResponse>,
    pub(crate) requested_permissions: RequestPermissionProfile,
    pub(crate) environment: TurnEnvironment,
    /// Background cells can request permissions after a different turn becomes active.
    pub(crate) turn_context: Arc<TurnContext>,
}

type PendingApprovals = SyncMutex<HashMap<String, PendingApproval>>;

/// The originating turn and its unique prompt ID stay valid even if the active turn changes.
pub(crate) struct NetworkApprovalPrompt<'a> {
    approvals: Weak<PendingApprovals>,
    events: &'a async_channel::Sender<Event>,
    turn_id: &'a str,
    approval_id: &'a str,
}

impl Drop for NetworkApprovalPrompt<'_> {
    fn drop(&mut self) {
        if let Some(approvals) = self.approvals.upgrade() {
            approvals
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(self.approval_id);
        }
        // The session channel is unbounded. Enqueue resolution before a same-host waiter can prompt.
        let _ = self.events.try_send(Event {
            id: self.turn_id.to_owned(),
            msg: EventMsg::NetworkApprovalResolved {
                approval_id: self.approval_id.to_owned(),
            },
        });
    }
}

impl TurnState {
    pub(crate) fn track_network_prompt<'a>(
        &self,
        events: &'a async_channel::Sender<Event>,
        turn_id: &'a str,
        approval_id: &'a str,
    ) -> NetworkApprovalPrompt<'a> {
        NetworkApprovalPrompt {
            approvals: Arc::downgrade(&self.pending_approvals),
            events,
            turn_id,
            approval_id,
        }
    }

    pub(crate) fn insert_pending_approval(
        &mut self,
        key: String,
        tx: oneshot::Sender<ReviewDecision>,
        kind: codex_protocol::approvals::ExecApprovalKind,
        turn_id: String,
    ) -> Option<PendingApproval> {
        self.pending_approvals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                key,
                PendingApproval {
                    sender: tx,
                    kind,
                    turn_id,
                },
            )
    }

    pub(crate) fn remove_pending_approval(
        &mut self,
        key: &str,
        expected_turn_id: Option<&str>,
    ) -> Option<PendingApproval> {
        let mut approvals = self
            .pending_approvals
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(expected) = expected_turn_id
            && approvals.get(key)?.turn_id != expected
        {
            return None;
        }
        approvals.remove(key)
    }

    pub(crate) fn clear_pending_waiters(&mut self) {
        self.pending_approvals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        self.pending_request_permissions.clear();
        self.pending_user_input.clear();
        self.pending_elicitations.clear();
        self.pending_dynamic_tools.clear();
    }

    pub(crate) fn insert_pending_request_permissions(
        &mut self,
        key: String,
        pending_request_permissions: PendingRequestPermissions,
    ) -> Option<PendingRequestPermissions> {
        self.pending_request_permissions
            .insert(key, pending_request_permissions)
    }

    pub(crate) fn remove_pending_request_permissions(
        &mut self,
        key: &str,
    ) -> Option<PendingRequestPermissions> {
        self.pending_request_permissions.remove(key)
    }

    pub(crate) fn insert_pending_user_input(
        &mut self,
        key: String,
        tx: oneshot::Sender<AcceptedUserInputResponse>,
    ) -> Option<oneshot::Sender<AcceptedUserInputResponse>> {
        self.pending_user_input.insert(key, tx)
    }

    pub(crate) fn remove_pending_user_input(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<AcceptedUserInputResponse>> {
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

    pub(crate) fn accept_mailbox_delivery_for_current_turn(&mut self) {
        self.set_mailbox_delivery_phase(MailboxDeliveryPhase::CurrentTurn);
    }

    pub(crate) fn accepts_mailbox_delivery_for_current_turn(&self) -> bool {
        self.mailbox_delivery_phase == MailboxDeliveryPhase::CurrentTurn
    }

    pub(crate) fn set_mailbox_delivery_phase(&mut self, phase: MailboxDeliveryPhase) {
        self.mailbox_delivery_phase = phase;
    }
}
