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
        let mut deleted_thread_ids = Vec::new();
        let result = {
            let _thread_list_state_permit = self.acquire_thread_list_state_permit().await?;
            self.thread_delete_response(params, &mut deleted_thread_ids)
                .await
        };
        match result {
            Ok(response) => {
                self.outgoing
                    .send_response(request_id.clone(), response)
                    .await;
                self.send_thread_deleted_notifications(deleted_thread_ids)
                    .await;
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    async fn thread_delete_response(
        &self,
        params: ThreadDeleteParams,
        deleted_thread_ids: &mut Vec<String>,
    ) -> Result<ThreadDeleteResponse, JSONRPCErrorError> {
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
            Err(err) => return Err(core_thread_write_error("delete thread", err)),
        }

        self.validate_root_thread_delete(thread_id, thread_ids.len() > 1)
            .await?;
        for thread_id_to_delete in thread_ids.iter().copied() {
            self.prepare_thread_for_delete(thread_id_to_delete).await;
        }

        let mut delete_order: Vec<_> = thread_ids.iter().skip(1).rev().copied().collect();
        delete_order.push(thread_id);

        for thread_id_to_delete in delete_order.iter().copied() {
            match self
                .thread_store
                .delete_thread(StoreDeleteThreadParams {
                    thread_id: thread_id_to_delete,
                })
                .await
            {
                Ok(()) => {}
                Err(ThreadStoreError::ThreadNotFound { .. }) => {
                    warn!(
                        "thread {thread_id_to_delete} was already missing while deleting {thread_id}"
                    );
                }
                Err(err) => {
                    return Err(thread_store_delete_error(err));
                }
            }
        }

        if let Some(state_db) = self.state_db.as_ref() {
            state_db
                .delete_threads_strict(thread_ids.as_slice())
                .await
                .map_err(|err| {
                    internal_error(format!(
                        "failed to delete app-server state for {thread_id}: {err}"
                    ))
                })?;
        }

        deleted_thread_ids.extend(
            delete_order
                .into_iter()
                .map(|thread_id| thread_id.to_string()),
        );
        Ok(ThreadDeleteResponse {})
    }

    async fn send_thread_deleted_notifications(&self, deleted_thread_ids: Vec<String>) {
        for thread_id in deleted_thread_ids {
            self.outgoing
                .send_server_notification(ServerNotification::ThreadDeleted(
                    ThreadDeletedNotification { thread_id },
                ))
                .await;
        }
    }

    async fn validate_root_thread_delete(
        &self,
        thread_id: ThreadId,
        has_descendants: bool,
    ) -> Result<(), JSONRPCErrorError> {
        if let Ok(thread) = self.thread_manager.get_thread(thread_id).await {
            if !thread.config_snapshot().await.ephemeral {
                return Ok(());
            }
            return Err(invalid_request(format!(
                "thread is not persisted and cannot be deleted: {thread_id}"
            )));
        }
        match self
            .thread_store
            .read_thread(StoreReadThreadParams {
                thread_id,
                include_archived: true,
                include_history: false,
            })
            .await
        {
            Ok(_) => Ok(()),
            Err(ThreadStoreError::ThreadNotFound { .. }) => {
                if has_descendants {
                    return Ok(());
                }
                let Some(state_db) = self.state_db.as_ref() else {
                    return Err(thread_store_delete_error(
                        ThreadStoreError::ThreadNotFound { thread_id },
                    ));
                };
                if state_db
                    .get_thread(thread_id)
                    .await
                    .map_err(|err| {
                        internal_error(format!(
                            "failed to read app-server state for {thread_id}: {err}"
                        ))
                    })?
                    .is_some()
                {
                    Ok(())
                } else {
                    Err(thread_store_delete_error(
                        ThreadStoreError::ThreadNotFound { thread_id },
                    ))
                }
            }
            Err(err) => Err(thread_store_delete_error(err)),
        }
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
