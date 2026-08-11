use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::grpc;
use codex_code_mode_protocol::host::MAX_PENDING_DELEGATE_CALLS;
use codex_protocol::ToolName;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const MAX_RECENT_CALLBACK_IDS: usize = 4_096;

struct ActiveCallback {
    execution_id: String,
    cancellation: CancellationToken,
}

pub(super) enum CallbackAdmission {
    Active(CancellationToken),
    Cancelled,
    Closed,
    Rejected(String),
}

#[derive(Default)]
struct ExecutionRecord {
    cell_id: Option<CellId>,
    tool_call_id: String,
    enabled_tools: HashMap<ToolName, i32>,
    started: bool,
    ready: bool,
    closed: bool,
    notifications: usize,
    cancellation: CancellationToken,
}

impl ExecutionRecord {
    fn accept_cell(&mut self, cell_id: &str) -> Result<(), String> {
        super::validate_identifier(cell_id, "cell ID")?;
        if let Some(current) = self.cell_id.as_ref() {
            if current.as_str() != cell_id {
                return Err(format!(
                    "code-mode execution changed cell ID from {current} to {cell_id}"
                ));
            }
        } else {
            self.cell_id = Some(CellId::new(cell_id.to_string()));
        }
        Ok(())
    }
}

#[derive(Default)]
struct RecentIds {
    values: HashSet<Uuid>,
    order: VecDeque<Uuid>,
}

impl RecentIds {
    fn remember(&mut self, value: Uuid) {
        if !self.values.insert(value) {
            return;
        }
        self.order.push_back(value);
        while self.order.len() > MAX_RECENT_CALLBACK_IDS {
            if let Some(expired) = self.order.pop_front() {
                self.values.remove(&expired);
            }
        }
    }

    fn remove(&mut self, value: &Uuid) -> bool {
        self.values.remove(value)
    }

    fn contains(&self, value: &Uuid) -> bool {
        self.values.contains(value)
    }
}

#[derive(Default)]
pub(super) struct SessionState {
    executions: HashMap<String, ExecutionRecord>,
    invocations: HashMap<String, ActiveCallback>,
    notifications: usize,
    seen_invocations: RecentIds,
    cancelled_invocations: RecentIds,
    failure: Option<String>,
    closed: bool,
}

impl SessionState {
    pub(super) fn require_open(&self) -> Result<(), String> {
        if self.closed {
            return Err(self
                .failure
                .clone()
                .unwrap_or_else(|| "code-mode gRPC session is closed".to_string()));
        }
        Ok(())
    }

    pub(super) fn begin_execution(&mut self, request: &grpc::ExecuteRequest) -> Result<(), String> {
        self.require_open()?;
        if request.execution_id.is_empty() || self.executions.contains_key(&request.execution_id) {
            return Err("code-mode execution ID was empty or reused".to_string());
        }
        super::validate_identifier(&request.tool_call_id, "tool call ID")?;
        let enabled_tools = request
            .enabled_tools
            .iter()
            .map(|definition| {
                let name = definition
                    .tool_name
                    .as_ref()
                    .ok_or_else(|| "code-mode enabled tool omitted its tool name".to_string())?;
                Ok((
                    ToolName::new(name.namespace.clone(), name.name.clone())
                        .with_default_namespace(),
                    definition.kind,
                ))
            })
            .collect::<Result<HashMap<_, _>, String>>()?;
        self.executions.insert(
            request.execution_id.clone(),
            ExecutionRecord {
                tool_call_id: request.tool_call_id.clone(),
                enabled_tools,
                ..ExecutionRecord::default()
            },
        );
        Ok(())
    }

    pub(super) fn admit_execution(
        &mut self,
        execution_id: &str,
        cell_id: &str,
    ) -> Result<(), String> {
        self.require_open()?;
        self.check_cell_ownership(execution_id, cell_id)?;
        let execution = self
            .executions
            .get_mut(execution_id)
            .ok_or_else(|| format!("unknown code-mode execution {execution_id}"))?;
        if execution.started {
            return Err(format!("code-mode execution {execution_id} started twice"));
        }
        execution.accept_cell(cell_id)?;
        execution.started = true;
        Ok(())
    }

    pub(super) fn mark_execution_ready(
        &mut self,
        execution_id: &str,
    ) -> Result<Option<CellId>, String> {
        self.require_open()?;
        let execution = self
            .executions
            .get_mut(execution_id)
            .ok_or_else(|| format!("unknown code-mode execution {execution_id}"))?;
        if !execution.started || execution.ready {
            return Err(format!(
                "code-mode execution {execution_id} was not ready to be claimed"
            ));
        }
        execution.ready = true;
        Ok(self.close_execution_if_ready(execution_id))
    }

    pub(super) fn admit_invocation(
        &mut self,
        call: &grpc::ToolCall,
    ) -> Result<CallbackAdmission, String> {
        self.require_open()?;
        let invocation_id = Uuid::parse_str(&call.invocation_id)
            .map_err(|_| "code-mode tool invocation ID must be a UUID".to_string())?;
        if self.invocations.contains_key(&call.invocation_id)
            || self.seen_invocations.contains(&invocation_id)
        {
            return Err("code-mode tool invocation ID was reused".to_string());
        }
        self.check_cell_ownership(&call.execution_id, &call.cell_id)?;
        let Some(execution) = self.executions.get_mut(&call.execution_id) else {
            self.seen_invocations.remember(invocation_id);
            self.cancelled_invocations.remove(&invocation_id);
            return Ok(CallbackAdmission::Closed);
        };
        execution.accept_cell(&call.cell_id)?;
        let execution_closed = execution.closed;
        self.seen_invocations.remember(invocation_id);

        let invocation_cancelled = self.cancelled_invocations.remove(&invocation_id);
        if execution_closed {
            return Ok(CallbackAdmission::Closed);
        }
        if invocation_cancelled {
            return Ok(CallbackAdmission::Cancelled);
        }
        let Some(name) = call.tool_name.as_ref() else {
            return Ok(CallbackAdmission::Rejected(
                "code-mode tool invocation omitted its tool name".to_string(),
            ));
        };
        let tool_name =
            ToolName::new(name.namespace.clone(), name.name.clone()).with_default_namespace();
        if execution.enabled_tools.get(&tool_name) != Some(&call.tool_kind) {
            return Ok(CallbackAdmission::Rejected(format!(
                "code-mode tool {tool_name} is not enabled for this execution"
            )));
        }
        if self.invocations.len() + self.notifications >= MAX_PENDING_DELEGATE_CALLS {
            return Ok(CallbackAdmission::Rejected(
                "code-mode host exceeded its pending delegate callback limit".to_string(),
            ));
        }
        let cancellation = CancellationToken::new();
        self.invocations.insert(
            call.invocation_id.clone(),
            ActiveCallback {
                execution_id: call.execution_id.clone(),
                cancellation: cancellation.clone(),
            },
        );
        Ok(CallbackAdmission::Active(cancellation))
    }

    pub(super) fn admit_notification(
        &mut self,
        notification: &grpc::Notification,
    ) -> Result<CallbackAdmission, String> {
        self.require_open()?;
        Uuid::parse_str(&notification.notification_id)
            .map_err(|_| "code-mode notification ID must be a UUID".to_string())?;
        super::validate_identifier(&notification.call_id, "notification call ID")?;
        self.check_cell_ownership(&notification.execution_id, &notification.cell_id)?;
        let Some(execution) = self.executions.get_mut(&notification.execution_id) else {
            return Ok(CallbackAdmission::Closed);
        };
        execution.accept_cell(&notification.cell_id)?;
        if notification.call_id != execution.tool_call_id {
            return Err("code-mode notification call ID does not match its execution".to_string());
        }
        if execution.closed {
            return Ok(CallbackAdmission::Closed);
        }
        if self.invocations.len() + self.notifications >= MAX_PENDING_DELEGATE_CALLS {
            return Ok(CallbackAdmission::Rejected(
                "code-mode host exceeded its pending delegate callback limit".to_string(),
            ));
        }
        execution.notifications += 1;
        self.notifications += 1;
        Ok(CallbackAdmission::Active(
            execution.cancellation.child_token(),
        ))
    }

    pub(super) fn finish_notification(&mut self, execution_id: &str) -> Option<CellId> {
        let execution = self.executions.get_mut(execution_id)?;
        execution.notifications = execution.notifications.checked_sub(1)?;
        self.notifications -= 1;
        self.close_execution_if_ready(execution_id)
    }

    pub(super) fn cancel_notifications(&self, cell_id: &CellId) {
        if let Some(cancellation) = self.notification_cancellation(cell_id) {
            cancellation.cancel();
        }
    }

    pub(super) fn notification_cancellation(&self, cell_id: &CellId) -> Option<CancellationToken> {
        self.executions
            .values()
            .find(|execution| execution.cell_id.as_ref() == Some(cell_id))
            .map(|execution| execution.cancellation.clone())
    }

    pub(super) fn cancel_invocation(&mut self, invocation_id: &str) -> Result<(), String> {
        let parsed = Uuid::parse_str(invocation_id)
            .map_err(|_| "code-mode tool invocation ID must be a UUID".to_string())?;
        if let Some(callback) = self.invocations.remove(invocation_id) {
            callback.cancellation.cancel();
        } else if !self.seen_invocations.contains(&parsed) {
            self.cancelled_invocations.remember(parsed);
        }
        Ok(())
    }

    pub(super) fn finish_invocation(&mut self, invocation_id: &str) {
        self.invocations.remove(invocation_id);
    }

    pub(super) fn close_cell(
        &mut self,
        closed: grpc::CellClosed,
    ) -> Result<Option<CellId>, String> {
        self.require_open()?;
        self.check_cell_ownership(&closed.execution_id, &closed.cell_id)?;
        let Some(execution) = self.executions.get_mut(&closed.execution_id) else {
            return Ok(None);
        };
        execution.accept_cell(&closed.cell_id)?;
        if execution.closed {
            return Err(format!(
                "code-mode host returned an invalid closure for cell {}",
                closed.cell_id
            ));
        }
        execution.closed = true;
        if execution.notifications == 0 {
            execution.cancellation.cancel();
        }
        self.revoke_execution_callbacks(&closed.execution_id);
        Ok(self.close_execution_if_ready(&closed.execution_id))
    }

    pub(super) fn close(&mut self, failure: Option<String>) -> Vec<CellId> {
        if self.closed {
            return Vec::new();
        }
        self.closed = true;
        self.failure = failure;
        self.notifications = 0;
        for (_, callback) in self.invocations.drain() {
            callback.cancellation.cancel();
        }
        self.executions
            .drain()
            .filter_map(|(_, execution)| {
                execution.cancellation.cancel();
                execution.cell_id
            })
            .collect()
    }

    fn close_execution_if_ready(&mut self, execution_id: &str) -> Option<CellId> {
        self.executions
            .get(execution_id)
            .is_some_and(|execution| {
                execution.started
                    && execution.ready
                    && execution.closed
                    && execution.notifications == 0
            })
            .then(|| self.remove_execution(execution_id))
            .flatten()
    }

    pub(super) fn remove_execution(&mut self, execution_id: &str) -> Option<CellId> {
        let execution = self.executions.remove(execution_id)?;
        self.notifications -= execution.notifications;
        execution.cancellation.cancel();
        self.revoke_execution_callbacks(execution_id);
        execution.cell_id
    }

    fn check_cell_ownership(&self, execution_id: &str, cell_id: &str) -> Result<(), String> {
        if self.executions.contains_key(execution_id)
            && self.executions.iter().any(|(id, execution)| {
                id != execution_id
                    && execution
                        .cell_id
                        .as_ref()
                        .is_some_and(|current| current.as_str() == cell_id)
            })
        {
            return Err(format!("code-mode host reused active cell ID {cell_id}"));
        }
        Ok(())
    }

    fn revoke_execution_callbacks(&mut self, execution_id: &str) {
        self.invocations.retain(|_, callback| {
            if callback.execution_id != execution_id {
                return true;
            }
            callback.cancellation.cancel();
            false
        });
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
