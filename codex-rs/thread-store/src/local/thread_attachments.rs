//! Local attachment adapter sharing lifecycle exclusion with thread deletion.

use super::LocalThreadStore;
use crate::AddThreadAttachmentOutcome;
use crate::AddThreadAttachmentParams;
use crate::ListThreadAttachmentsParams;
use crate::RemoveThreadAttachmentOutcome;
use crate::RemoveThreadAttachmentParams;
use crate::ThreadAttachmentPage;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use codex_protocol::ThreadId;
use codex_rollout::StateDbHandle;

pub(super) async fn copy_thread_attachments(
    store: &LocalThreadStore,
    source_thread_id: ThreadId,
    destination_thread_id: ThreadId,
) -> ThreadStoreResult<()> {
    let state_db = state_db(store, "copy_thread_attachments")?;
    let _lifecycle_reservation = store
        .live_writer_locks
        .reserve_lifecycle(destination_thread_id)
        .await;
    // Reference-backed forks already reserve their source. Do not recursively acquire that
    // reservation: a queued deletion could deadlock it. SQLite serializes the source snapshot.
    state_db
        .copy_thread_attachments(source_thread_id, destination_thread_id)
        .await
        .map_err(|error| attachment_error("copy", Some(destination_thread_id), error))
}

pub(super) async fn add_thread_attachment(
    store: &LocalThreadStore,
    params: AddThreadAttachmentParams,
) -> ThreadStoreResult<AddThreadAttachmentOutcome> {
    let state_db = state_db(store, "thread/attachment/add")?;
    let _lifecycle_reservation = store
        .live_writer_locks
        .reserve_lifecycle(params.thread_id)
        .await;
    state_db
        .add_thread_attachment(
            params.thread_id,
            &params.attachment_type,
            &params.identity_key,
            &params.payload,
        )
        .await
        .map_err(|error| attachment_error("add", Some(params.thread_id), error))
}

pub(super) async fn list_thread_attachments(
    store: &LocalThreadStore,
    params: ListThreadAttachmentsParams,
) -> ThreadStoreResult<ThreadAttachmentPage> {
    let state_db = state_db(store, "thread/attachment/list")?;
    state_db
        .list_thread_attachments(params.thread_id, params.cursor.as_deref(), params.limit)
        .await
        .map_err(|error| attachment_error("list", /*thread_id*/ None, error))
}

pub(super) async fn remove_thread_attachment(
    store: &LocalThreadStore,
    params: RemoveThreadAttachmentParams,
) -> ThreadStoreResult<RemoveThreadAttachmentOutcome> {
    let state_db = state_db(store, "thread/attachment/remove")?;
    let _lifecycle_reservation = store
        .live_writer_locks
        .reserve_lifecycle(params.thread_id)
        .await;
    state_db
        .remove_thread_attachment(
            params.thread_id,
            &params.attachment_type,
            &params.identity_key,
        )
        .await
        .map_err(|error| attachment_error("remove", Some(params.thread_id), error))
}

fn state_db<'store>(
    store: &'store LocalThreadStore,
    operation: &'static str,
) -> ThreadStoreResult<&'store StateDbHandle> {
    store
        .state_db
        .as_ref()
        .ok_or(ThreadStoreError::Unsupported { operation })
}

fn attachment_error(
    operation: &str,
    thread_id: Option<ThreadId>,
    error: impl std::fmt::Display,
) -> ThreadStoreError {
    let message = error.to_string();
    if let Some(message) = message.strip_prefix("invalid thread attachment request: ") {
        return ThreadStoreError::InvalidRequest {
            message: message.to_string(),
        };
    }
    if message.starts_with("thread not found: ")
        && let Some(thread_id) = thread_id
    {
        return ThreadStoreError::ThreadNotFound { thread_id };
    }
    ThreadStoreError::Internal {
        message: format!("failed to {operation} thread attachment: {message}"),
    }
}

#[cfg(test)]
#[path = "thread_attachments_tests.rs"]
mod tests;
