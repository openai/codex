use std::time::Duration;

use codex_file_system::FileSystemSandboxContext;
use codex_protocol::models::PermissionProfile;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::time::timeout;

use super::*;
use crate::rpc::FILE_TRANSFER_SESSION_LOST_ERROR_CODE;

fn test_runtime_paths() -> ExecServerRuntimePaths {
    ExecServerRuntimePaths::new(
        std::env::current_exe().expect("current exe"),
        /*codex_linux_sandbox_exe*/ None,
    )
    .expect("runtime paths")
}

fn test_handler() -> FileTransferHandler {
    FileTransferHandler::new(
        test_runtime_paths(),
        PreparedFileUploadAvailability::EnabledForDevelopment,
    )
}

fn prepare_params(path: &std::path::Path, max_bytes: u64) -> FileTransferPrepareUploadParams {
    FileTransferPrepareUploadParams {
        path: PathUri::from_path(path).expect("path URI"),
        sandbox: full_access_context(),
        max_bytes,
    }
}

fn full_access_context() -> FileSystemSandboxContext {
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::Disabled)
}

#[tokio::test]
async fn prepare_captures_stable_bytes_and_metadata() {
    let source_dir = tempfile::tempdir().expect("source tempdir");
    let source = source_dir.path().join("report.txt");
    tokio::fs::write(&source, b"prepared bytes")
        .await
        .expect("write source");
    let handler = test_handler();
    let prepared = handler
        .prepare_upload(prepare_params(&source, /*max_bytes*/ 1024))
        .await
        .expect("prepare upload");
    assert_eq!(prepared.name, "report.txt");
    assert_eq!(prepared.size, 14);
    assert_eq!(
        prepared.digest.algorithm,
        FileTransferDigestAlgorithm::Sha256
    );

    tokio::fs::write(&source, b"different bytes")
        .await
        .expect("mutate source");
    {
        let operations = handler.inner.operations.lock().await;
        let snapshot = operations
            .get(&prepared.transfer_id)
            .and_then(|operation| operation.bytes.as_ref())
            .expect("prepared snapshot should remain present");
        assert_eq!(snapshot.as_slice(), b"prepared bytes");
    }
    handler.shutdown().await;
}

#[tokio::test]
async fn prepare_enforces_byte_limit_and_cancel_drops_snapshot() {
    let source_dir = tempfile::tempdir().expect("source tempdir");
    let source = source_dir.path().join("report.txt");
    tokio::fs::write(&source, b"nine-byte")
        .await
        .expect("write source");
    let handler = test_handler();

    let oversized = handler
        .prepare_upload(prepare_params(&source, /*max_bytes*/ 8))
        .await
        .expect_err("source exceeds requested limit");
    assert_eq!(oversized.code, -32602);

    let prepared = handler
        .prepare_upload(prepare_params(&source, /*max_bytes*/ 1024))
        .await
        .expect("prepare upload");
    let canceled = handler
        .cancel(FileTransferCancelParams {
            transfer_id: prepared.transfer_id.clone(),
        })
        .await
        .expect("cancel prepared upload");
    assert_eq!(canceled.state, FileTransferOperationState::Canceled);
    let status = handler
        .status(FileTransferStatusParams {
            transfer_id: prepared.transfer_id,
        })
        .await
        .expect("canceled status");
    assert_eq!(status.state, FileTransferOperationState::Canceled);
    {
        let operations = handler.inner.operations.lock().await;
        assert_eq!(prepared_bytes(&operations), 0);
    }
    handler.shutdown().await;
}

#[tokio::test]
async fn prepared_snapshot_expires_without_a_follow_up_rpc() {
    let source_dir = tempfile::tempdir().expect("source tempdir");
    let source = source_dir.path().join("report.txt");
    tokio::fs::write(&source, b"sensitive bytes")
        .await
        .expect("write source");
    let handler = test_handler();
    let prepared = handler
        .prepare_upload(prepare_params(&source, /*max_bytes*/ 1024))
        .await
        .expect("prepare upload");

    timeout(PREPARED_UPLOAD_TTL + Duration::from_secs(1), async {
        loop {
            let expired = handler
                .inner
                .operations
                .lock()
                .await
                .get(&prepared.transfer_id)
                .is_some_and(|operation| operation.state == FileTransferOperationState::Expired);
            if expired {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("sweeper should expire the prepared snapshot");
    {
        let operations = handler.inner.operations.lock().await;
        let operation = operations
            .get(&prepared.transfer_id)
            .expect("terminal status should be retained");
        assert!(operation.bytes.is_none());
    }
    handler.shutdown().await;
}

#[tokio::test]
async fn terminal_records_do_not_poison_the_session_quota() {
    let source_dir = tempfile::tempdir().expect("source tempdir");
    let source = source_dir.path().join("report.txt");
    tokio::fs::write(&source, b"bytes")
        .await
        .expect("write source");
    let handler = test_handler();
    for _ in 0..(MAX_OPERATIONS_PER_SESSION + 8) {
        let prepared = handler
            .prepare_upload(prepare_params(&source, /*max_bytes*/ 1024))
            .await
            .expect("terminal records should be pruned under pressure");
        handler
            .cancel(FileTransferCancelParams {
                transfer_id: prepared.transfer_id,
            })
            .await
            .expect("cancel upload");
    }
    assert_eq!(handler.inner.tasks.len(), 1);
    handler.shutdown().await;
}

#[tokio::test]
async fn disabled_handler_rejects_before_reading_and_detects_old_session_ids() {
    let disabled = FileTransferHandler::new(
        test_runtime_paths(),
        PreparedFileUploadAvailability::Disabled,
    );
    let temp_dir = tempfile::tempdir().expect("create temp directory");
    let missing = temp_dir.path().join("missing");
    let error = disabled
        .prepare_upload(prepare_params(&missing, /*max_bytes*/ 1024))
        .await
        .expect_err("disabled handler must reject before filesystem access");
    assert_eq!(error.code, -32600);

    let handler = test_handler();
    let error = handler
        .status(FileTransferStatusParams {
            transfer_id: format!("old-generation:{}", Uuid::new_v4()),
        })
        .await
        .expect_err("old session ID should be distinguishable from an unknown operation");
    assert_eq!(error.code, FILE_TRANSFER_SESSION_LOST_ERROR_CODE);
    disabled.shutdown().await;
    handler.shutdown().await;
}
