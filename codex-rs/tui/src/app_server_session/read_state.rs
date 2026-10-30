//! A focused conversation acknowledges only the receipt revision it observed from this backend.

use super::*;
use codex_app_server_protocol::ThreadReadState;

pub(crate) fn unread_revision(state: Option<ThreadReadState>) -> Option<String> {
    state.and_then(|s| s.first_unread.map(|_| s.revision))
}

impl AppServerSession {
    pub(crate) fn remember_read_state(&mut self, id: ThreadId, state: Option<ThreadReadState>) {
        if let Some(revision) = unread_revision(state) {
            self.unread_revisions.insert(id, revision);
        } else {
            self.unread_revisions.remove(&id);
        }
    }

    pub(crate) fn freeze_viewed_receipt(&mut self, thread_id: ThreadId) {
        self.frozen_receipt = self
            .unread_revisions
            .remove(&thread_id)
            .map(|revision| (thread_id, revision));
    }

    /// Keep the observed revision for a later visit if the write fails, but send it
    /// only once per focused view. The server notification clears successful live reads.
    pub(crate) fn viewed_revision(&mut self, id: ThreadId, frozen: bool) -> Option<String> {
        let revision = if frozen {
            self.frozen_receipt
                .as_ref()
                .filter(|(thread, _)| *thread == id)
                .map(|(_, revision)| revision)
        } else {
            self.unread_revisions.get(&id)
        }?;
        if self.attempted_revision.as_ref() == Some(revision) {
            return None;
        }
        self.attempted_revision = Some(revision.clone());
        Some(revision.clone())
    }

    /// A conflict returns the current receipt. A newer result requires a fresh view.
    pub(crate) fn acknowledge_read(
        &self,
        thread_id: ThreadId,
        expected_revision: String,
    ) -> impl Future<Output = Result<ThreadReadState>> + use<> {
        use codex_app_server_protocol::ThreadReadStateOperation;
        use codex_app_server_protocol::ThreadReadStateUpdateParams;
        use codex_app_server_protocol::ThreadReadStateUpdateResponse;
        let handle = self.request_handle();
        let params = ThreadReadStateUpdateParams {
            thread_id: thread_id.to_string(),
            expected_revision,
            operation: ThreadReadStateOperation::Read,
        };
        async move {
            let send = || {
                handle.request_typed::<ThreadReadStateUpdateResponse>(
                    ClientRequest::ThreadReadStateUpdate {
                        request_id: RequestId::String(Uuid::new_v4().to_string()),
                        params: params.clone(),
                    },
                )
            };
            let mut result = send().await;
            // Retrying the same revision cannot clear a result published in the meantime.
            if matches!(&result, Err(TypedRequestError::Server { source, .. }) if matches!(source.code, -32603 | -32001))
            {
                tokio::time::sleep(Duration::from_millis(250)).await;
                result = send().await;
            }
            match result {
                Ok(response) => Ok(response.read_state),
                Err(TypedRequestError::Server { ref source, .. })
                    if let Some(data) = source.data.as_ref()
                        && data["reason"] == "readStateConflict" =>
                {
                    Ok(serde_json::from_value(data["readState"].clone())?)
                }
                Err(error) => Err(error.into()),
            }
        }
    }
}
