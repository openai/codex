use codex_protocol::error::Result as CodexResult;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

/// The turn that accepted a submitted user message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserMessageAdmission {
    /// Core installed a new turn for the submitted message.
    Started { turn_id: String },
    /// Core accepted the submitted message into an already-running turn.
    Steered { turn_id: String },
}

#[derive(Default)]
pub(crate) struct PendingUserMessageAdmissions {
    pending: Mutex<HashMap<String, oneshot::Sender<CodexResult<UserMessageAdmission>>>>,
}

impl PendingUserMessageAdmissions {
    pub(crate) fn register(
        &self,
        submission_id: String,
    ) -> (
        PendingUserMessageAdmissionGuard<'_>,
        oneshot::Receiver<CodexResult<UserMessageAdmission>>,
    ) {
        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(submission_id.clone(), sender);
        (
            PendingUserMessageAdmissionGuard {
                admissions: self,
                submission_id,
            },
            receiver,
        )
    }

    pub(crate) fn complete(
        &self,
        submission_id: &str,
        admission: CodexResult<UserMessageAdmission>,
    ) {
        let admission_sender = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(submission_id);
        if let Some(sender) = admission_sender {
            let _ = sender.send(admission);
        }
    }

    fn remove(&self, submission_id: &str) {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(submission_id);
    }
}

pub(crate) struct PendingUserMessageAdmissionGuard<'a> {
    admissions: &'a PendingUserMessageAdmissions,
    submission_id: String,
}

impl Drop for PendingUserMessageAdmissionGuard<'_> {
    fn drop(&mut self) {
        self.admissions.remove(&self.submission_id);
    }
}
