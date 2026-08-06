use codex_protocol::error::CodexErr;
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

/// Explains why a submitted user message did not become durable.
#[derive(Debug)]
pub enum UserMessageAdmissionError {
    /// Core rejected the submission before admitting it to a turn.
    Admission(CodexErr),
    /// A user-prompt hook rejected the submitted message.
    RejectedByHook,
    /// The owning task finished before the submitted message was persisted.
    TaskEndedBeforePersistence,
    /// The admitted message could not be durably written to the rollout.
    PersistenceFailed(CodexErr),
}

impl From<UserMessageAdmissionError> for CodexErr {
    fn from(error: UserMessageAdmissionError) -> Self {
        match error {
            UserMessageAdmissionError::Admission(error) => error,
            UserMessageAdmissionError::RejectedByHook => {
                Self::InvalidRequest("user message was rejected by a hook".to_string())
            }
            UserMessageAdmissionError::TaskEndedBeforePersistence => Self::InternalAgentDied,
            UserMessageAdmissionError::PersistenceFailed(error) => error,
        }
    }
}

#[derive(Default)]
pub(crate) struct PendingUserMessageAdmissions {
    pending: Mutex<HashMap<String, PendingUserMessageAdmission>>,
}

struct PendingUserMessageAdmission {
    client_id: Option<String>,
    sender: oneshot::Sender<Result<UserMessageAdmission, UserMessageAdmissionError>>,
    state: PendingUserMessageAdmissionState,
}

pub(crate) enum PendingUserMessageAdmissionState {
    Immediate,
    WaitingForAdmission,
    Admitted(UserMessageAdmission),
    Persisted,
}

impl PendingUserMessageAdmissions {
    pub(crate) fn register(
        &self,
        submission_id: String,
        client_id: Option<String>,
        state: PendingUserMessageAdmissionState,
    ) -> (
        PendingUserMessageAdmissionGuard<'_>,
        oneshot::Receiver<Result<UserMessageAdmission, UserMessageAdmissionError>>,
    ) {
        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                submission_id.clone(),
                PendingUserMessageAdmission {
                    client_id,
                    sender,
                    state,
                },
            );
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
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(entry) = pending.get_mut(submission_id) else {
            return;
        };

        match admission {
            Ok(admission) => match &entry.state {
                PendingUserMessageAdmissionState::WaitingForAdmission => {
                    entry.state = PendingUserMessageAdmissionState::Admitted(admission);
                }
                PendingUserMessageAdmissionState::Immediate
                | PendingUserMessageAdmissionState::Persisted => {
                    if let Some(entry) = pending.remove(submission_id) {
                        let _ = entry.sender.send(Ok(admission));
                    }
                }
                PendingUserMessageAdmissionState::Admitted(_) => {}
            },
            Err(error) => {
                if let Some(entry) = pending.remove(submission_id) {
                    let _ = entry
                        .sender
                        .send(Err(UserMessageAdmissionError::Admission(error)));
                }
            }
        }
    }

    pub(crate) fn contains_client_id(&self, client_id: &str) -> bool {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .any(|entry| {
                !matches!(&entry.state, PendingUserMessageAdmissionState::Immediate)
                    && entry.client_id.as_deref() == Some(client_id)
            })
    }

    pub(crate) fn associate_steered_by_client_id(&self, client_id: &str, turn_id: &str) {
        let submission_id = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find_map(|(submission_id, entry)| {
                (matches!(
                    &entry.state,
                    PendingUserMessageAdmissionState::WaitingForAdmission
                        | PendingUserMessageAdmissionState::Persisted
                ) && entry.client_id.as_deref() == Some(client_id))
                .then(|| submission_id.clone())
            });
        if let Some(submission_id) = submission_id {
            self.complete(
                &submission_id,
                Ok(UserMessageAdmission::Steered {
                    turn_id: turn_id.to_string(),
                }),
            );
        }
    }

    pub(crate) fn complete_persistence(
        &self,
        client_id: &str,
        result: Result<(), UserMessageAdmissionError>,
    ) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(submission_id) = pending.iter().find_map(|(submission_id, entry)| {
            (!matches!(&entry.state, PendingUserMessageAdmissionState::Immediate)
                && entry.client_id.as_deref() == Some(client_id))
            .then(|| submission_id.clone())
        }) else {
            return;
        };

        if let Err(error) = result {
            if let Some(entry) = pending.remove(&submission_id) {
                let _ = entry.sender.send(Err(error));
            }
            return;
        }

        let Some(entry) = pending.get_mut(&submission_id) else {
            return;
        };
        match std::mem::replace(
            &mut entry.state,
            PendingUserMessageAdmissionState::Persisted,
        ) {
            PendingUserMessageAdmissionState::Admitted(admission) => {
                if let Some(entry) = pending.remove(&submission_id) {
                    let _ = entry.sender.send(Ok(admission));
                }
            }
            PendingUserMessageAdmissionState::Immediate
            | PendingUserMessageAdmissionState::WaitingForAdmission
            | PendingUserMessageAdmissionState::Persisted => {}
        }
    }

    pub(crate) fn complete_task_end(&self, turn_id: &str) -> bool {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let submission_ids = pending
            .iter()
            .filter_map(|(submission_id, entry)| {
                let belongs_to_turn = match &entry.state {
                    PendingUserMessageAdmissionState::WaitingForAdmission => {
                        submission_id == turn_id
                    }
                    PendingUserMessageAdmissionState::Admitted(
                        UserMessageAdmission::Started {
                            turn_id: admission_turn_id,
                        }
                        | UserMessageAdmission::Steered {
                            turn_id: admission_turn_id,
                        },
                    ) => admission_turn_id == turn_id,
                    PendingUserMessageAdmissionState::Immediate
                    | PendingUserMessageAdmissionState::Persisted => false,
                };
                belongs_to_turn.then(|| submission_id.clone())
            })
            .collect::<Vec<_>>();

        let had_pending_admissions = !submission_ids.is_empty();
        for submission_id in submission_ids {
            if let Some(entry) = pending.remove(&submission_id) {
                let _ = entry
                    .sender
                    .send(Err(UserMessageAdmissionError::TaskEndedBeforePersistence));
            }
        }
        had_pending_admissions
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
