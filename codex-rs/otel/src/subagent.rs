//! Records accepted spawns, child turn attempts, and accepted stop commands.
//! Stable IDs in events link the same child across workers. Counter labels are bounded.

use crate::MULTI_AGENT_SPAWNED_METRIC;
use crate::MULTI_AGENT_STOP_ACCEPTED_METRIC;
use crate::MULTI_AGENT_TURN_LIFECYCLE_METRIC;
use crate::SessionTelemetry;
use crate::bounded_originator_tag_value;
use crate::events::shared::trace_event;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::TurnAbortReason;
use strum_macros::IntoStaticStr;

/// A child turn attempt can be suspended and later started again on another worker.
#[derive(Clone, Copy, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum SubagentTurnStatus<'a> {
    Started,
    Completed,
    Failed(Option<&'a CodexErrorInfo>),
    Interrupted {
        reason: &'a TurnAbortReason,
        error: Option<&'a CodexErrorInfo>,
    },
    Suspended,
}

/// The operation accepted by the target's agent backend.
#[derive(Clone, Copy, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum SubagentStopAction {
    Close,
    Interrupt,
}

impl SessionTelemetry {
    /// Links a successful spawn to the child that will report turn attempts.
    pub fn record_multi_agent_spawned(
        &self,
        root_thread_id: SessionId,
        parent_turn_id: &str,
        child_thread_id: ThreadId,
    ) {
        trace_event!(
            self,
            event.name = MULTI_AGENT_SPAWNED_METRIC,
            root_thread_id = %root_thread_id,
            child_thread_id = %child_thread_id,
            parent_turn_id,
            product_sku = self.metadata.product_sku.unwrap_or("unknown"),
        );
        self.subagent_counter(MULTI_AGENT_SPAWNED_METRIC, &[]);
    }

    /// Records a turn boundary using only the error class, never its message or payload.
    pub fn record_multi_agent_turn_lifecycle(
        &self,
        turn_id: &str,
        turn_kind: &'static str,
        status: SubagentTurnStatus<'_>,
    ) {
        let (error, abort_reason) = match status {
            SubagentTurnStatus::Failed(error) => (error, None),
            SubagentTurnStatus::Interrupted { reason, error } => {
                (error, Some(<&str>::from(reason)))
            }
            SubagentTurnStatus::Started
            | SubagentTurnStatus::Completed
            | SubagentTurnStatus::Suspended => (None, None),
        };
        let error_kind = error.map(<&str>::from);
        let status: &str = status.into();
        trace_event!(
            self,
            event.name = MULTI_AGENT_TURN_LIFECYCLE_METRIC,
            child_thread_id = %self.metadata.conversation_id,
            turn_id,
            turn_kind,
            status,
            abort_reason,
            error_kind,
            product_sku = self.metadata.product_sku.unwrap_or("unknown"),
        );
        self.subagent_counter(
            MULTI_AGENT_TURN_LIFECYCLE_METRIC,
            &[
                ("turn_kind", Some(turn_kind)),
                ("status", Some(status)),
                ("error_kind", error_kind),
                ("abort_reason", abort_reason),
            ],
        );
    }

    /// Records an accepted stop even when the target was already idle or unloaded.
    pub fn record_multi_agent_stop_accepted(
        &self,
        root_thread_id: SessionId,
        child_thread_id: ThreadId,
        call_id: &str,
        action: SubagentStopAction,
        previous_status: Option<&AgentStatus>,
    ) {
        let caller_thread_id = self.metadata.conversation_id;
        let caller = if ThreadId::from(root_thread_id) == caller_thread_id {
            "root"
        } else {
            "subagent"
        };
        let action: &str = action.into();
        let previous_status = previous_status.map_or("unknown", <&str>::from);
        trace_event!(
            self,
            event.name = MULTI_AGENT_STOP_ACCEPTED_METRIC,
            root_thread_id = %root_thread_id,
            child_thread_id = %child_thread_id,
            caller_thread_id = %caller_thread_id,
            call_id,
            caller,
            action,
            previous_status,
            product_sku = self.metadata.product_sku.unwrap_or("unknown"),
        );
        self.subagent_counter(
            MULTI_AGENT_STOP_ACCEPTED_METRIC,
            &[
                ("caller", Some(caller)),
                ("action", Some(action)),
                ("previous_status", Some(previous_status)),
            ],
        );
    }

    fn subagent_counter(&self, name: &str, labels: &[(&str, Option<&str>)]) {
        let Some(metrics) = &self.metrics else {
            return;
        };
        // Default session tags can include model and the parent's thread ID.
        let mut tags = Vec::with_capacity(labels.len() + 2);
        tags.extend(
            labels
                .iter()
                .filter_map(|(key, value)| Some((*key, (*value)?))),
        );
        tags.push((
            "product_sku",
            self.metadata.product_sku.unwrap_or("unknown"),
        ));
        tags.push((
            "originator",
            bounded_originator_tag_value(&self.metadata.originator),
        ));
        let _ = metrics.counter(name, /*inc*/ 1, &tags);
    }
}
