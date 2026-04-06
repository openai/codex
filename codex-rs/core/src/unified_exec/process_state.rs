use codex_exec_server::ExecApprovalRequest;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ProcessState {
    pub(crate) has_exited: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) failure_message: Option<String>,
    pub(crate) pending_exec_approval: Option<ExecApprovalRequest>,
}

impl ProcessState {
    pub(crate) fn exited(&self, exit_code: Option<i32>) -> Self {
        Self {
            has_exited: true,
            exit_code,
            failure_message: self.failure_message.clone(),
            pending_exec_approval: self.pending_exec_approval.clone(),
        }
    }

    pub(crate) fn failed(&self, message: String) -> Self {
        Self {
            has_exited: true,
            exit_code: self.exit_code,
            failure_message: Some(message),
            pending_exec_approval: self.pending_exec_approval.clone(),
        }
    }

    pub(crate) fn with_pending_exec_approval(
        &self,
        pending_exec_approval: ExecApprovalRequest,
    ) -> Self {
        Self {
            has_exited: self.has_exited,
            exit_code: self.exit_code,
            failure_message: self.failure_message.clone(),
            pending_exec_approval: Some(pending_exec_approval),
        }
    }

    pub(crate) fn clear_pending_exec_approval(&self) -> Self {
        Self {
            has_exited: self.has_exited,
            exit_code: self.exit_code,
            failure_message: self.failure_message.clone(),
            pending_exec_approval: None,
        }
    }
}
