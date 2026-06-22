use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_app_server_protocol::JSONRPCErrorError;
use sha2::Digest;
use sha2::Sha256;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::ExecServerRuntimePaths;
use crate::local_file_system::LocalFileSystem;
use crate::protocol::FileTransferCancelParams;
use crate::protocol::FileTransferCancelResponse;
use crate::protocol::FileTransferDigest;
use crate::protocol::FileTransferDigestAlgorithm;
use crate::protocol::FileTransferOperationState;
use crate::protocol::FileTransferPrepareUploadParams;
use crate::protocol::FileTransferPrepareUploadResponse;
use crate::protocol::FileTransferStartUploadParams;
use crate::protocol::FileTransferStartUploadResponse;
use crate::protocol::FileTransferStatusParams;
use crate::protocol::FileTransferStatusResponse;
use crate::protocol::FileTransferUploadDescriptorKind;
use crate::protocol::MAX_PREPARED_FILE_UPLOAD_BYTES;
use crate::protocol::PREPARED_FILE_UPLOAD_PROTOCOL_VERSION;
use crate::protocol::PreparedFileUploadCapability;
use crate::rpc::file_transfer_session_lost;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;
use crate::rpc::invalid_request;
use crate::rpc::not_found;
use crate::server::file_transfer_http::UploadOutcome;
use crate::server::file_transfer_http::upload_bytes;
use crate::server::file_transfer_http::validate_upload_descriptor;

pub(crate) const FILE_TRANSFER_ENABLED_ENV_VAR: &str =
    "CODEX_EXEC_SERVER_PREPARED_FILE_UPLOAD_ENABLED";
const MAX_PREPARED_BYTES_PER_SESSION: u64 = 32 * 1024 * 1024;
const MAX_OPERATIONS_PER_SESSION: usize = 32;
const MAX_ACTIVE_UPLOADS_PER_SESSION: usize = 2;
#[cfg(test)]
const PREPARED_UPLOAD_TTL: Duration = Duration::from_millis(100);
#[cfg(not(test))]
const PREPARED_UPLOAD_TTL: Duration = Duration::from_secs(10 * 60);
const TERMINAL_RESULT_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Copy)]
pub(crate) enum PreparedFileUploadAvailability {
    Disabled,
    EnabledForDevelopment,
}

#[derive(Clone)]
pub(crate) struct FileTransferHandler {
    inner: Arc<Inner>,
}

struct Inner {
    session_generation_id: String,
    availability: PreparedFileUploadAvailability,
    file_system: LocalFileSystem,
    operations: Mutex<HashMap<String, UploadOperation>>,
    tasks: TaskTracker,
    shutdown: CancellationToken,
    expiry_changed: Notify,
}

struct UploadOperation {
    deadline: Instant,
    bytes: Option<Zeroizing<Vec<u8>>>,
    state: FileTransferOperationState,
    error: Option<String>,
    terminal_at: Option<Instant>,
    cancellation: CancellationToken,
}

impl FileTransferHandler {
    pub(crate) fn new(
        runtime_paths: ExecServerRuntimePaths,
        availability: PreparedFileUploadAvailability,
    ) -> Self {
        let handler = Self {
            inner: Arc::new(Inner {
                // This tag distinguishes logical session generations without
                // exposing the resume session ID, which is a bearer secret.
                session_generation_id: Uuid::new_v4().to_string(),
                availability,
                file_system: LocalFileSystem::with_runtime_paths(runtime_paths),
                operations: Mutex::new(HashMap::new()),
                tasks: TaskTracker::new(),
                shutdown: CancellationToken::new(),
                expiry_changed: Notify::new(),
            }),
        };
        handler.start_expiry_sweeper();
        handler
    }

    pub(crate) fn capability(&self) -> Option<PreparedFileUploadCapability> {
        matches!(
            self.inner.availability,
            PreparedFileUploadAvailability::EnabledForDevelopment
        )
        .then_some(PreparedFileUploadCapability {
            protocol_version: PREPARED_FILE_UPLOAD_PROTOCOL_VERSION,
            max_upload_bytes: MAX_PREPARED_FILE_UPLOAD_BYTES,
            descriptor_kinds: vec![FileTransferUploadDescriptorKind::HttpsPut],
            supports_status_reconciliation: true,
        })
    }

    pub(crate) async fn prepare_upload(
        &self,
        params: FileTransferPrepareUploadParams,
    ) -> Result<FileTransferPrepareUploadResponse, JSONRPCErrorError> {
        self.require_enabled()?;
        if params.max_bytes == 0 || params.max_bytes > MAX_PREPARED_FILE_UPLOAD_BYTES {
            return Err(invalid_params(format!(
                "file transfer maxBytes must be between 1 and {MAX_PREPARED_FILE_UPLOAD_BYTES}"
            )));
        }
        self.prune_operations().await;
        {
            let operations = self.inner.operations.lock().await;
            ensure_prepare_quota(&operations, /*additional_bytes*/ 0)?;
        }

        let bytes = self
            .inner
            .file_system
            .read_file_with_limit(&params.path, Some(&params.sandbox), params.max_bytes)
            .await
            .map_err(map_prepare_error)?;
        let size = bytes.len() as u64;
        let name = params
            .path
            .basename()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "upload".to_string());
        let digest = FileTransferDigest {
            algorithm: FileTransferDigestAlgorithm::Sha256,
            value: URL_SAFE_NO_PAD.encode(Sha256::digest(&bytes)),
        };
        let transfer_id = format!("{}:{}", self.inner.session_generation_id, Uuid::new_v4());
        let deadline = Instant::now() + PREPARED_UPLOAD_TTL;
        let expires_at_unix_seconds = unix_seconds(SystemTime::now() + PREPARED_UPLOAD_TTL);
        let operation = UploadOperation {
            deadline,
            bytes: Some(Zeroizing::new(bytes)),
            state: FileTransferOperationState::Prepared,
            error: None,
            terminal_at: None,
            cancellation: CancellationToken::new(),
        };

        let mut operations = self.inner.operations.lock().await;
        ensure_prepare_quota(&operations, size)?;
        operations.insert(transfer_id.clone(), operation);
        drop(operations);
        self.inner.expiry_changed.notify_one();

        Ok(FileTransferPrepareUploadResponse {
            transfer_id,
            name,
            size,
            digest,
            expires_at_unix_seconds,
        })
    }

    pub(crate) async fn start_upload(
        &self,
        params: FileTransferStartUploadParams,
    ) -> Result<FileTransferStartUploadResponse, JSONRPCErrorError> {
        self.require_enabled()?;
        self.validate_transfer_id(&params.transfer_id)?;
        {
            let mut operations = self.inner.operations.lock().await;
            let active_upload_count = active_uploads(&operations);
            let operation = operations
                .get_mut(&params.transfer_id)
                .ok_or_else(|| not_found("unknown prepared upload".to_string()))?;
            expire_operation(operation, Instant::now());
            if operation.state != FileTransferOperationState::Prepared {
                return Err(invalid_request(format!(
                    "prepared upload is {}",
                    state_name(operation.state)
                )));
            }
            if active_upload_count >= MAX_ACTIVE_UPLOADS_PER_SESSION {
                return Err(invalid_request(
                    "active file upload quota exceeded".to_string(),
                ));
            }
        }
        let descriptor = validate_upload_descriptor(params.descriptor).await?;
        let (bytes, cancellation) = {
            let mut operations = self.inner.operations.lock().await;
            let active_upload_count = active_uploads(&operations);
            let operation = operations
                .get_mut(&params.transfer_id)
                .ok_or_else(|| not_found("unknown prepared upload".to_string()))?;
            expire_operation(operation, Instant::now());
            if operation.state != FileTransferOperationState::Prepared {
                return Err(invalid_request(format!(
                    "prepared upload is {}",
                    state_name(operation.state)
                )));
            }
            if active_upload_count >= MAX_ACTIVE_UPLOADS_PER_SESSION {
                return Err(invalid_request(
                    "active file upload quota exceeded".to_string(),
                ));
            }
            let bytes = operation
                .bytes
                .take()
                .ok_or_else(|| internal_error("prepared upload bytes are missing".to_string()))?;
            operation.state = FileTransferOperationState::Uploading;
            (bytes, operation.cancellation.clone())
        };
        self.inner.expiry_changed.notify_one();

        let transfer_id = params.transfer_id;
        let handler = self.clone();
        let task_transfer_id = transfer_id.clone();
        let _task = self.inner.tasks.spawn(async move {
            let outcome = upload_bytes(bytes, descriptor, cancellation).await;
            handler.finish_upload(&task_transfer_id, outcome).await;
        });

        Ok(FileTransferStartUploadResponse { transfer_id })
    }

    pub(crate) async fn status(
        &self,
        params: FileTransferStatusParams,
    ) -> Result<FileTransferStatusResponse, JSONRPCErrorError> {
        self.require_enabled()?;
        self.validate_transfer_id(&params.transfer_id)?;
        let mut operations = self.inner.operations.lock().await;
        let operation = operations
            .get_mut(&params.transfer_id)
            .ok_or_else(|| not_found("unknown file transfer operation".to_string()))?;
        expire_operation(operation, Instant::now());
        Ok(status_response(&params.transfer_id, operation))
    }

    pub(crate) async fn cancel(
        &self,
        params: FileTransferCancelParams,
    ) -> Result<FileTransferCancelResponse, JSONRPCErrorError> {
        self.require_enabled()?;
        self.validate_transfer_id(&params.transfer_id)?;
        let mut operations = self.inner.operations.lock().await;
        let operation = operations
            .get_mut(&params.transfer_id)
            .ok_or_else(|| not_found("unknown file transfer operation".to_string()))?;
        expire_operation(operation, Instant::now());
        match operation.state {
            FileTransferOperationState::Prepared => {
                operation.bytes = None;
                set_terminal(
                    operation,
                    FileTransferOperationState::Canceled,
                    /*error*/ None,
                );
                self.inner.expiry_changed.notify_one();
            }
            FileTransferOperationState::Uploading => {
                operation.cancellation.cancel();
                operation.state = FileTransferOperationState::CancelRequested;
                operation.error = None;
            }
            _ => {}
        }
        Ok(FileTransferCancelResponse {
            state: operation.state,
        })
    }

    pub(crate) async fn shutdown(&self) {
        self.inner.shutdown.cancel();
        self.inner.tasks.close();
        {
            let mut operations = self.inner.operations.lock().await;
            for operation in operations.values_mut() {
                operation.cancellation.cancel();
                operation.bytes = None;
                if matches!(
                    operation.state,
                    FileTransferOperationState::Uploading
                        | FileTransferOperationState::CancelRequested
                ) {
                    set_terminal(
                        operation,
                        FileTransferOperationState::CompletionUnknown,
                        Some("upload completion was lost with the executor session".to_string()),
                    );
                }
            }
        }
        self.inner.tasks.wait().await;
    }

    async fn finish_upload(&self, transfer_id: &str, outcome: UploadOutcome) {
        let mut operations = self.inner.operations.lock().await;
        let Some(operation) = operations.get_mut(transfer_id) else {
            return;
        };
        match outcome {
            UploadOutcome::Succeeded => set_terminal(
                operation,
                FileTransferOperationState::Succeeded,
                /*error*/ None,
            ),
            UploadOutcome::Failed(error) => {
                set_terminal(operation, FileTransferOperationState::Failed, Some(error))
            }
            UploadOutcome::CompletionUnknown(error) => set_terminal(
                operation,
                FileTransferOperationState::CompletionUnknown,
                Some(error),
            ),
        }
    }

    fn require_enabled(&self) -> Result<(), JSONRPCErrorError> {
        if matches!(
            self.inner.availability,
            PreparedFileUploadAvailability::EnabledForDevelopment
        ) {
            Ok(())
        } else {
            Err(invalid_request(
                "prepared file upload is disabled on this executor".to_string(),
            ))
        }
    }

    fn validate_transfer_id(&self, transfer_id: &str) -> Result<(), JSONRPCErrorError> {
        let Some((session_generation_id, opaque_id)) = transfer_id.split_once(':') else {
            return Err(invalid_params("invalid file transfer id".to_string()));
        };
        if session_generation_id != self.inner.session_generation_id {
            return Err(file_transfer_session_lost(
                "file transfer belongs to an expired executor session".to_string(),
            ));
        }
        if Uuid::parse_str(opaque_id).is_err() {
            return Err(invalid_params("invalid file transfer id".to_string()));
        }
        Ok(())
    }

    fn start_expiry_sweeper(&self) {
        let inner = Arc::clone(&self.inner);
        let tasks = self.inner.tasks.clone();
        let _task = tasks.spawn(async move {
            loop {
                let notified = inner.expiry_changed.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                let next_deadline = inner
                    .operations
                    .lock()
                    .await
                    .values()
                    .filter(|operation| operation.state == FileTransferOperationState::Prepared)
                    .map(|operation| operation.deadline)
                    .min();
                match next_deadline {
                    Some(deadline) => tokio::select! {
                        _ = tokio::time::sleep_until(deadline) => {
                            let now = Instant::now();
                            let mut operations = inner.operations.lock().await;
                            for operation in operations.values_mut() {
                                expire_operation(operation, now);
                            }
                        }
                        _ = &mut notified => {}
                        _ = inner.shutdown.cancelled() => break,
                    },
                    None => tokio::select! {
                        _ = &mut notified => {}
                        _ = inner.shutdown.cancelled() => break,
                    },
                }
            }
        });
    }

    async fn prune_operations(&self) {
        let now = Instant::now();
        let mut operations = self.inner.operations.lock().await;
        for operation in operations.values_mut() {
            expire_operation(operation, now);
        }
        operations.retain(|_, operation| {
            operation
                .terminal_at
                .is_none_or(|terminal_at| now.duration_since(terminal_at) < TERMINAL_RESULT_TTL)
        });
        while operations.len() >= MAX_OPERATIONS_PER_SESSION {
            let oldest_terminal = operations
                .iter()
                .filter_map(|(id, operation)| operation.terminal_at.map(|at| (id.clone(), at)))
                .min_by_key(|(_, at)| *at)
                .map(|(id, _)| id);
            let Some(oldest_terminal) = oldest_terminal else {
                break;
            };
            operations.remove(&oldest_terminal);
        }
    }
}

fn ensure_prepare_quota(
    operations: &HashMap<String, UploadOperation>,
    additional_bytes: u64,
) -> Result<(), JSONRPCErrorError> {
    if operations.len() >= MAX_OPERATIONS_PER_SESSION {
        return Err(invalid_request(
            "file transfer operation quota exceeded".to_string(),
        ));
    }
    if prepared_bytes(operations).saturating_add(additional_bytes) > MAX_PREPARED_BYTES_PER_SESSION
    {
        return Err(invalid_request(
            "prepared upload session quota exceeded".to_string(),
        ));
    }
    Ok(())
}

fn prepared_bytes(operations: &HashMap<String, UploadOperation>) -> u64 {
    operations
        .values()
        .filter_map(|operation| operation.bytes.as_ref())
        .map(|bytes| bytes.len() as u64)
        .sum()
}

fn active_uploads(operations: &HashMap<String, UploadOperation>) -> usize {
    operations
        .values()
        .filter(|operation| {
            matches!(
                operation.state,
                FileTransferOperationState::Uploading | FileTransferOperationState::CancelRequested
            )
        })
        .count()
}

fn status_response(transfer_id: &str, operation: &UploadOperation) -> FileTransferStatusResponse {
    FileTransferStatusResponse {
        transfer_id: transfer_id.to_string(),
        state: operation.state,
        error: operation.error.clone(),
    }
}

fn expire_operation(operation: &mut UploadOperation, now: Instant) {
    if operation.state == FileTransferOperationState::Prepared && now >= operation.deadline {
        operation.bytes = None;
        set_terminal(
            operation,
            FileTransferOperationState::Expired,
            /*error*/ None,
        );
    }
}

fn set_terminal(
    operation: &mut UploadOperation,
    state: FileTransferOperationState,
    error: Option<String>,
) {
    operation.state = state;
    operation.error = error;
    operation.terminal_at = Some(Instant::now());
}

fn state_name(state: FileTransferOperationState) -> &'static str {
    match state {
        FileTransferOperationState::Prepared => "prepared",
        FileTransferOperationState::Uploading => "uploading",
        FileTransferOperationState::CancelRequested => "cancel requested",
        FileTransferOperationState::Succeeded => "succeeded",
        FileTransferOperationState::Failed => "failed",
        FileTransferOperationState::Canceled => "canceled",
        FileTransferOperationState::CompletionUnknown => "completion unknown",
        FileTransferOperationState::Expired => "expired",
    }
}

fn unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn map_prepare_error(error: std::io::Error) -> JSONRPCErrorError {
    match error.kind() {
        std::io::ErrorKind::NotFound => not_found("upload source was not found".to_string()),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::PermissionDenied => {
            invalid_params("upload source is unavailable or exceeds the byte limit".to_string())
        }
        _ => internal_error("failed to prepare upload source".to_string()),
    }
}

#[cfg(test)]
#[path = "file_transfer_handler_tests.rs"]
mod tests;
