//! Attachment RPC handling; committed mutations respond before broadcasting updates.

use super::thread_processor::THREAD_LIST_DEFAULT_LIMIT;
use super::thread_processor::ThreadRequestProcessor;
use crate::error_code::internal_error;
use crate::error_code::invalid_params;
use crate::error_code::method_not_found;
use crate::outgoing_message::ConnectionRequestId;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadAttachment;
use codex_app_server_protocol::ThreadAttachmentAddOutcome;
use codex_app_server_protocol::ThreadAttachmentAddParams;
use codex_app_server_protocol::ThreadAttachmentAddResponse;
use codex_app_server_protocol::ThreadAttachmentListParams;
use codex_app_server_protocol::ThreadAttachmentListResponse;
use codex_app_server_protocol::ThreadAttachmentOperation;
use codex_app_server_protocol::ThreadAttachmentRemoveParams;
use codex_app_server_protocol::ThreadAttachmentRemoveResponse;
use codex_app_server_protocol::ThreadAttachmentUpdatedNotification;
use codex_protocol::ThreadId;
use codex_state::MAX_THREAD_ATTACHMENT_IDENTITY_KEY_BYTES;
use codex_state::MAX_THREAD_ATTACHMENT_LIST_PAGE_SIZE;
use codex_state::MAX_THREAD_ATTACHMENT_PAYLOAD_BYTES;
use codex_state::MAX_THREAD_ATTACHMENT_TYPE_BYTES;
use codex_thread_store::AddThreadAttachmentOutcome;
use codex_thread_store::AddThreadAttachmentParams;
use codex_thread_store::ListThreadAttachmentsParams;
use codex_thread_store::RemoveThreadAttachmentOutcome;
use codex_thread_store::RemoveThreadAttachmentParams;
use codex_thread_store::ThreadAttachment as StoredThreadAttachment;
use codex_thread_store::ThreadStoreError;

impl ThreadRequestProcessor {
    pub(crate) async fn thread_attachment_add(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadAttachmentAddParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        const OPERATION: &str = "thread/attachment/add";
        self.ensure_thread_attachments_supported(OPERATION)?;
        validate_attachment_identity(&params.attachment_type, &params.identity_key)?;
        let payload_bytes = serde_json::to_vec(&params.payload)
            .map_err(|error| invalid_params(format!("invalid attachment payload: {error}")))?;
        if payload_bytes.len() > MAX_THREAD_ATTACHMENT_PAYLOAD_BYTES {
            return Err(invalid_params(format!(
                "attachment payload must not exceed {MAX_THREAD_ATTACHMENT_PAYLOAD_BYTES} bytes"
            )));
        }

        let thread_id = parse_attachment_thread_id(&params.thread_id)?;
        let result = self
            .thread_store
            .add_thread_attachment(AddThreadAttachmentParams {
                thread_id,
                attachment_type: params.attachment_type,
                identity_key: params.identity_key,
                payload: params.payload,
            })
            .await
            .map_err(|error| thread_attachment_store_error(OPERATION, error))?;

        let (response, notification) = match result {
            AddThreadAttachmentOutcome::Created(attachment) => {
                let notification = ThreadAttachmentUpdatedNotification {
                    thread_id: attachment.thread_id.to_string(),
                    attachment_type: attachment.attachment_type.clone(),
                    identity_key: attachment.identity_key.clone(),
                    attachment_id: attachment.id.clone(),
                    operation: ThreadAttachmentOperation::Created,
                };
                (
                    ThreadAttachmentAddResponse {
                        outcome: ThreadAttachmentAddOutcome::Created,
                        attachment: api_thread_attachment(attachment),
                    },
                    Some(notification),
                )
            }
            AddThreadAttachmentOutcome::Existing(attachment) => (
                ThreadAttachmentAddResponse {
                    outcome: ThreadAttachmentAddOutcome::Existing,
                    attachment: api_thread_attachment(attachment),
                },
                None,
            ),
        };

        self.outgoing.send_response(request_id, response).await;
        if let Some(notification) = notification {
            self.outgoing
                .send_server_notification(ServerNotification::ThreadAttachmentUpdated(notification))
                .await;
        }
        Ok(None)
    }

    pub(crate) async fn thread_attachment_list(
        &self,
        params: ThreadAttachmentListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        const OPERATION: &str = "thread/attachment/list";
        self.ensure_thread_attachments_supported(OPERATION)?;
        let thread_id = parse_attachment_thread_id(&params.thread_id)?;
        let limit = params
            .limit
            .map(|limit| limit as usize)
            .unwrap_or(THREAD_LIST_DEFAULT_LIMIT)
            .clamp(1, MAX_THREAD_ATTACHMENT_LIST_PAGE_SIZE);
        let page = self
            .thread_store
            .list_thread_attachments(ListThreadAttachmentsParams {
                thread_id,
                cursor: params.cursor,
                limit,
            })
            .await
            .map_err(|error| thread_attachment_store_error(OPERATION, error))?;

        Ok(Some(
            ThreadAttachmentListResponse {
                data: page
                    .attachments
                    .into_iter()
                    .map(api_thread_attachment)
                    .collect(),
                next_cursor: page.next_cursor,
            }
            .into(),
        ))
    }

    pub(crate) async fn thread_attachment_remove(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadAttachmentRemoveParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        const OPERATION: &str = "thread/attachment/remove";
        self.ensure_thread_attachments_supported(OPERATION)?;
        validate_attachment_identity(&params.attachment_type, &params.identity_key)?;
        let thread_id = parse_attachment_thread_id(&params.thread_id)?;
        let result = self
            .thread_store
            .remove_thread_attachment(RemoveThreadAttachmentParams {
                thread_id,
                attachment_type: params.attachment_type,
                identity_key: params.identity_key,
            })
            .await
            .map_err(|error| thread_attachment_store_error(OPERATION, error))?;

        self.outgoing
            .send_response(request_id, ThreadAttachmentRemoveResponse {})
            .await;
        if let RemoveThreadAttachmentOutcome::Removed(attachment) = result {
            self.outgoing
                .send_server_notification(ServerNotification::ThreadAttachmentUpdated(
                    ThreadAttachmentUpdatedNotification {
                        thread_id: attachment.thread_id.to_string(),
                        attachment_type: attachment.attachment_type,
                        identity_key: attachment.identity_key,
                        attachment_id: attachment.id,
                        operation: ThreadAttachmentOperation::Deleted,
                    },
                ))
                .await;
        }

        Ok(None)
    }

    fn ensure_thread_attachments_supported(
        &self,
        operation: &'static str,
    ) -> Result<(), JSONRPCErrorError> {
        if self.thread_store.supports_thread_attachments() {
            Ok(())
        } else {
            Err(unsupported_thread_attachment_operation(operation))
        }
    }
}

fn validate_attachment_identity(
    attachment_type: &str,
    identity_key: &str,
) -> Result<(), JSONRPCErrorError> {
    if attachment_type.trim().is_empty() {
        return Err(invalid_params("attachmentType must not be empty"));
    }
    if attachment_type.len() > MAX_THREAD_ATTACHMENT_TYPE_BYTES {
        return Err(invalid_params(format!(
            "attachmentType must not exceed {MAX_THREAD_ATTACHMENT_TYPE_BYTES} bytes"
        )));
    }
    if identity_key.trim().is_empty() {
        return Err(invalid_params("identityKey must not be empty"));
    }
    if identity_key.len() > MAX_THREAD_ATTACHMENT_IDENTITY_KEY_BYTES {
        return Err(invalid_params(format!(
            "identityKey must not exceed {MAX_THREAD_ATTACHMENT_IDENTITY_KEY_BYTES} bytes"
        )));
    }
    Ok(())
}

fn parse_attachment_thread_id(thread_id: &str) -> Result<ThreadId, JSONRPCErrorError> {
    ThreadId::from_string(thread_id)
        .map_err(|error| invalid_params(format!("invalid thread id: {error}")))
}

fn api_thread_attachment(attachment: StoredThreadAttachment) -> ThreadAttachment {
    ThreadAttachment {
        id: attachment.id,
        attachment_type: attachment.attachment_type,
        identity_key: attachment.identity_key,
        payload: attachment.payload,
        created_at: attachment.created_at,
    }
}

fn unsupported_thread_attachment_operation(operation: &'static str) -> JSONRPCErrorError {
    method_not_found(format!(
        "{operation} is not supported by the configured thread store"
    ))
}

fn thread_attachment_store_error(
    operation: &'static str,
    error: ThreadStoreError,
) -> JSONRPCErrorError {
    match error {
        ThreadStoreError::Unsupported { .. } => unsupported_thread_attachment_operation(operation),
        ThreadStoreError::InvalidRequest { message } => invalid_params(message),
        ThreadStoreError::ThreadNotFound { thread_id } => {
            invalid_params(format!("thread not found: {thread_id}"))
        }
        error @ (ThreadStoreError::Conflict { .. } | ThreadStoreError::Internal { .. }) => {
            internal_error(format!("failed to process {operation}: {error}"))
        }
    }
}
