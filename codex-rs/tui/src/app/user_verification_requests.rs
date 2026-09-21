//! Keeps the approved request bytes separate from the view and rejects late signing results.

use std::collections::HashMap;

use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::UserVerificationVerifyParams;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Default)]
pub(super) struct UserVerificationRequests {
    pending: HashMap<(String, RequestId), PendingVerification>,
}

#[derive(Debug)]
struct PendingVerification {
    thread_id: String,
    params: UserVerificationVerifyParams,
    attempt: Option<Uuid>,
    cancelled: CancellationToken,
}

pub(super) struct UserVerificationAttempt {
    pub(super) id: Uuid,
    pub(super) params: UserVerificationVerifyParams,
    pub(super) cancelled: CancellationToken,
}

impl Drop for PendingVerification {
    fn drop(&mut self) {
        self.cancelled.cancel();
    }
}

impl UserVerificationRequests {
    pub(super) fn note_request(
        &mut self,
        request_id: &RequestId,
        params: &McpServerElicitationRequestParams,
    ) {
        if let McpServerElicitationRequest::UserVerification {
            title,
            description,
            challenge,
            ..
        } = &params.request
        {
            self.pending.insert(
                (params.server_name.clone(), request_id.clone()),
                PendingVerification {
                    thread_id: params.thread_id.clone(),
                    params: UserVerificationVerifyParams {
                        title: title.clone(),
                        description: description.clone(),
                        challenge: challenge.clone(),
                    },
                    attempt: None,
                    cancelled: CancellationToken::new(),
                },
            );
        }
    }

    pub(super) fn begin(
        &mut self,
        server_name: &str,
        request_id: &RequestId,
    ) -> Option<UserVerificationAttempt> {
        let pending = self
            .pending
            .get_mut(&(server_name.to_string(), request_id.clone()))?;
        if pending.attempt.is_some() {
            return None;
        }
        let attempt = Uuid::new_v4();
        pending.attempt = Some(attempt);
        Some(UserVerificationAttempt {
            id: attempt,
            params: pending.params.clone(),
            cancelled: pending.cancelled.clone(),
        })
    }

    pub(super) fn is_pending(
        &self,
        server_name: &str,
        request_id: &RequestId,
        attempt: Uuid,
    ) -> bool {
        self.pending
            .get(&(server_name.to_string(), request_id.clone()))
            .is_some_and(|pending| pending.attempt == Some(attempt))
    }

    pub(super) fn remove(&mut self, server_name: &str, request_id: &RequestId) {
        self.pending
            .remove(&(server_name.to_string(), request_id.clone()));
    }

    pub(super) fn cancel_thread(&mut self, thread_id: &str) -> Vec<(String, RequestId)> {
        let discarded_keys = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.thread_id == thread_id)
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for (server_name, request_id) in &discarded_keys {
            self.remove(server_name, request_id);
        }
        discarded_keys
    }

    pub(super) fn clear(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
#[path = "user_verification_requests_tests.rs"]
mod tests;
