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
        let result = {
            let _thread_list_state_permit = self.acquire_thread_list_state_permit().await?;
            self.thread_delete_response(params).await
        };
        match result {
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
            Err(CodexErr::ThreadNotFound(_)) => {}
            Err(err) => return Err(core_thread_write_error("delete thread", err)),
        }

        self.validate_root_thread_delete(thread_id).await?;
        for thread_id_to_delete in thread_ids.iter().copied() {
            self.prepare_thread_for_delete(thread_id_to_delete).await;
        }

        let mut deleted_thread_ids = Vec::new();
        for descendant_thread_id in thread_ids.iter().skip(1).rev().copied() {
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
                Err(ThreadStoreError::ThreadNotFound { .. }) => {
                    warn!(
                        "spawned descendant thread {descendant_thread_id} was already missing while deleting {thread_id}"
                    );
                }
                Err(err) => {
                    return Err(thread_store_delete_error(err));
                }
            }
        }

        self.thread_store
            .delete_thread(StoreDeleteThreadParams { thread_id })
            .await
            .map_err(thread_store_delete_error)?;
        deleted_thread_ids.push(thread_id.to_string());

        Ok((ThreadDeleteResponse {}, deleted_thread_ids))
    }

    async fn validate_root_thread_delete(
        &self,
        thread_id: ThreadId,
    ) -> Result<(), JSONRPCErrorError> {
        if let Ok(thread) = self.thread_manager.get_thread(thread_id).await
            && thread.config_snapshot().await.ephemeral
        {
            return Err(invalid_request(format!(
                "thread is not persisted and cannot be deleted: {thread_id}"
            )));
        }
        self.thread_store
            .read_thread(StoreReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await
            .map(|_| ())
            .map_err(thread_store_delete_error)
    }

    async fn prepare_thread_for_delete(&self, thread_id: ThreadId) {
        self.prepare_thread_for_removal(thread_id, "delete").await;
        if let Some(log_db) = self.log_db.as_ref() {
            log_db.flush().await;
        }
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
