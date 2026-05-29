//! `thread/delete` request handling.

use super::thread_processor::core_thread_write_error;
use super::thread_processor::unsupported_thread_store_operation;
use super::*;

impl ThreadRequestProcessor {
    pub(crate) async fn thread_delete(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadDeleteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        match self.thread_delete_inner(params).await {
            Ok((response, deleted_thread_ids)) => {
                self.outgoing
                    .send_response(request_id.clone(), response)
                    .await;
                for thread_id in deleted_thread_ids {
                    self.outgoing
                        .send_server_notification(ServerNotification::ThreadDeleted(
                            ThreadDeletedNotification { thread_id },
                        ))
                        .await;
                }
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    async fn thread_delete_inner(
        &self,
        params: ThreadDeleteParams,
    ) -> Result<(ThreadDeleteResponse, Vec<String>), JSONRPCErrorError> {
        let _thread_list_state_permit = self.acquire_thread_list_state_permit().await?;
        self.thread_delete_response(params).await
    }

    async fn thread_delete_response(
        &self,
        params: ThreadDeleteParams,
    ) -> Result<(ThreadDeleteResponse, Vec<String>), JSONRPCErrorError> {
        let thread_id = ThreadId::from_string(&params.thread_id)
            .map_err(|err| invalid_request(format!("invalid thread id: {err}")))?;

        let mut thread_ids = self.state_db_spawn_subtree_thread_ids(thread_id).await?;
        let mut seen = thread_ids.iter().copied().collect::<HashSet<_>>();

        match self
            .thread_manager
            .list_agent_subtree_thread_ids(thread_id)
            .await
        {
            Ok(live_thread_ids) => {
                for live_thread_id in live_thread_ids {
                    if seen.insert(live_thread_id) {
                        thread_ids.push(live_thread_id);
                    }
                }
            }
            Err(CodexErr::ThreadNotFound(_)) if self.state_db.is_some() => {}
            Err(CodexErr::ThreadNotFound(_)) => {
                return Err(internal_error(format!(
                    "cannot delete thread {thread_id}: sqlite state db is unavailable and the thread is not loaded"
                )));
            }
            Err(err) => return Err(core_thread_write_error("delete thread", err)),
        }

        self.prepare_thread_for_removal(thread_id, "delete").await;
        match self
            .thread_store
            .delete_thread(StoreDeleteThreadParams { thread_id })
            .await
        {
            Ok(()) => {}
            Err(err) => return Err(thread_store_delete_error(err)),
        }

        let mut deleted_thread_ids = vec![thread_id.to_string()];
        for descendant_thread_id in thread_ids.iter().skip(1).rev().copied() {
            self.prepare_thread_for_removal(descendant_thread_id, "delete")
                .await;
            match self
                .thread_store
                .delete_thread(StoreDeleteThreadParams {
                    thread_id: descendant_thread_id,
                })
                .await
            {
                Ok(()) => {
                    deleted_thread_ids.push(descendant_thread_id.to_string());
                }
                Err(err) => {
                    warn!(
                        "failed to delete spawned descendant thread {descendant_thread_id} while deleting {thread_id}: {err}"
                    );
                }
            }
        }

        Ok((ThreadDeleteResponse {}, deleted_thread_ids))
    }
}

fn thread_store_delete_error(err: ThreadStoreError) -> JSONRPCErrorError {
    match err {
        ThreadStoreError::ThreadNotFound { thread_id } => {
            invalid_request(format!("thread not found: {thread_id}"))
        }
        ThreadStoreError::InvalidRequest { message } => invalid_request(message),
        ThreadStoreError::Unsupported { operation } => {
            unsupported_thread_store_operation(operation)
        }
        err => internal_error(format!("failed to delete thread: {err}")),
    }
}
