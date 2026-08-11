use std::collections::HashMap;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::grpc;

#[derive(Default)]
struct ExecutionRecord {
    cell_id: Option<CellId>,
    started: bool,
    ready: bool,
    closed: bool,
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
pub(super) struct SessionState {
    executions: HashMap<String, ExecutionRecord>,
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

    pub(super) fn begin_execution(&mut self, execution_id: String) -> Result<(), String> {
        self.require_open()?;
        if execution_id.is_empty() || self.executions.contains_key(&execution_id) {
            return Err("code-mode execution ID was empty or reused".to_string());
        }
        self.executions
            .insert(execution_id, ExecutionRecord::default());
        Ok(())
    }

    pub(super) fn admit_execution(
        &mut self,
        execution_id: &str,
        cell_id: &str,
    ) -> Result<(), String> {
        self.require_open()?;
        if self.executions.iter().any(|(id, execution)| {
            id != execution_id
                && execution
                    .cell_id
                    .as_ref()
                    .is_some_and(|current| current.as_str() == cell_id)
        }) {
            return Err(format!("code-mode host reused active cell ID {cell_id}"));
        }
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

    pub(super) fn close_cell(
        &mut self,
        closed: grpc::CellClosed,
    ) -> Result<Option<CellId>, String> {
        self.require_open()?;
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
        Ok(self.close_execution_if_ready(&closed.execution_id))
    }

    pub(super) fn close(&mut self, failure: Option<String>) -> Vec<CellId> {
        if self.closed {
            return Vec::new();
        }
        self.closed = true;
        self.failure = failure;
        self.executions
            .drain()
            .filter_map(|(_, execution)| execution.cell_id)
            .collect()
    }

    fn close_execution_if_ready(&mut self, execution_id: &str) -> Option<CellId> {
        self.executions
            .get(execution_id)
            .is_some_and(|execution| execution.started && execution.ready && execution.closed)
            .then(|| self.remove_execution(execution_id))
            .flatten()
    }

    pub(super) fn remove_execution(&mut self, execution_id: &str) -> Option<CellId> {
        self.executions.remove(execution_id)?.cell_id
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
