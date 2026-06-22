use std::time::Duration;

use codex_file_system::FileSystemSandboxContext;
use codex_protocol::models::PermissionProfile;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_bytes;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;
use crate::protocol::FileTransferHeader;
use crate::protocol::FileTransferUploadDescriptor;
use crate::rpc::FILE_TRANSFER_SESSION_LOST_ERROR_CODE;
use crate::server::file_transfer_http::validate_upload_descriptor;

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

fn upload_descriptor(url: String) -> FileTransferUploadDescriptor {
    FileTransferUploadDescriptor::HttpsPut {
        url,
        headers: Vec::new(),
        expires_at_unix_seconds: unix_seconds(SystemTime::now() + Duration::from_secs(60)),
    }
}

#[tokio::test]
async fn prepared_upload_uses_stable_executor_owned_bytes() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/upload"))
        .and(body_bytes(b"prepared bytes".as_slice()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

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
    let started = handler
        .start_upload(FileTransferStartUploadParams {
            transfer_id: prepared.transfer_id.clone(),
            descriptor: upload_descriptor(format!("{}/upload", server.uri())),
        })
        .await
        .expect("start upload");
    assert_eq!(started.transfer_id, prepared.transfer_id);

    let status = timeout(Duration::from_secs(2), async {
        loop {
            let status = handler
                .status(FileTransferStatusParams {
                    transfer_id: started.transfer_id.clone(),
                })
                .await
                .expect("upload status");
            if status.state != FileTransferOperationState::Uploading {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("upload should finish");
    assert_eq!(
        status,
        FileTransferStatusResponse {
            transfer_id: started.transfer_id.clone(),
            state: FileTransferOperationState::Succeeded,
            error: None,
        }
    );

    let replay = handler
        .start_upload(FileTransferStartUploadParams {
            transfer_id: started.transfer_id,
            descriptor: upload_descriptor(format!("{}/replay", server.uri())),
        })
        .await
        .expect_err("prepared upload is single use");
    assert_eq!(replay.code, -32600);
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
            transfer_id: prepared.transfer_id.clone(),
        })
        .await
        .expect("canceled status");
    assert_eq!(status.state, FileTransferOperationState::Canceled);
    {
        let operations = handler.inner.operations.lock().await;
        assert_eq!(prepared_bytes(&operations), 0);
    }

    let start = handler
        .start_upload(FileTransferStartUploadParams {
            transfer_id: prepared.transfer_id,
            descriptor: upload_descriptor("https://safe.blob.core.windows.net/object".to_string()),
        })
        .await
        .expect_err("canceled snapshot cannot be uploaded");
    assert_eq!(start.code, -32600);
    handler.shutdown().await;
}

#[tokio::test]
async fn cancel_after_remote_receives_body_reports_unknown_completion() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/delayed"))
        .and(body_bytes(b"committable bytes".as_slice()))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&server)
        .await;
    let source_dir = tempfile::tempdir().expect("source tempdir");
    let source = source_dir.path().join("report.txt");
    tokio::fs::write(&source, b"committable bytes")
        .await
        .expect("write source");
    let handler = test_handler();
    let prepared = handler
        .prepare_upload(prepare_params(&source, /*max_bytes*/ 1024))
        .await
        .expect("prepare upload");
    handler
        .start_upload(FileTransferStartUploadParams {
            transfer_id: prepared.transfer_id.clone(),
            descriptor: upload_descriptor(format!("{}/delayed", server.uri())),
        })
        .await
        .expect("start upload");
    timeout(Duration::from_secs(1), async {
        loop {
            if server
                .received_requests()
                .await
                .is_some_and(|requests| !requests.is_empty())
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("server should receive the complete request before cancellation");

    let canceled = handler
        .cancel(FileTransferCancelParams {
            transfer_id: prepared.transfer_id.clone(),
        })
        .await
        .expect("request cancellation");
    assert_eq!(canceled.state, FileTransferOperationState::CancelRequested);
    let status = timeout(Duration::from_secs(1), async {
        loop {
            let status = handler
                .status(FileTransferStatusParams {
                    transfer_id: prepared.transfer_id.clone(),
                })
                .await
                .expect("upload status");
            if status.state != FileTransferOperationState::CancelRequested {
                break status;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancellation should settle");
    assert_eq!(status.state, FileTransferOperationState::CompletionUnknown);
    handler.shutdown().await;
}

#[tokio::test]
async fn concurrent_starts_cannot_exceed_the_active_upload_quota() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .mount(&server)
        .await;
    let source_dir = tempfile::tempdir().expect("source tempdir");
    let source = source_dir.path().join("report.txt");
    tokio::fs::write(&source, b"bytes")
        .await
        .expect("write source");
    let handler = test_handler();
    let first = handler
        .prepare_upload(prepare_params(&source, /*max_bytes*/ 1024))
        .await
        .expect("prepare first upload");
    let second = handler
        .prepare_upload(prepare_params(&source, /*max_bytes*/ 1024))
        .await
        .expect("prepare second upload");
    let third = handler
        .prepare_upload(prepare_params(&source, /*max_bytes*/ 1024))
        .await
        .expect("prepare third upload");

    let (first, second, third) = tokio::join!(
        handler.start_upload(FileTransferStartUploadParams {
            transfer_id: first.transfer_id,
            descriptor: upload_descriptor(format!("{}/first", server.uri())),
        }),
        handler.start_upload(FileTransferStartUploadParams {
            transfer_id: second.transfer_id,
            descriptor: upload_descriptor(format!("{}/second", server.uri())),
        }),
        handler.start_upload(FileTransferStartUploadParams {
            transfer_id: third.transfer_id,
            descriptor: upload_descriptor(format!("{}/third", server.uri())),
        }),
    );
    let results = [first, second, third];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 2);
    let error = results
        .iter()
        .find_map(|result| result.as_ref().err())
        .expect("one concurrent start should be rejected");
    assert_eq!(error.code, -32600);
    assert_eq!(error.message, "active file upload quota exceeded");
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
        assert_eq!(operation.state, FileTransferOperationState::Expired);
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
async fn disabled_handler_rejects_before_reading_and_does_not_accept_old_session_ids() {
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
            transfer_id: format!("old-session:{}", Uuid::new_v4()),
        })
        .await
        .expect_err("old session ID should be distinguishable from an unknown operation");
    assert_eq!(error.code, FILE_TRANSFER_SESSION_LOST_ERROR_CODE);
    disabled.shutdown().await;
    handler.shutdown().await;
}

#[tokio::test]
async fn descriptor_policy_rejects_unsafe_transport_and_headers() {
    let insecure =
        validate_upload_descriptor(upload_descriptor("http://example.com/upload".to_string()))
            .await
            .expect_err("non-local HTTP URL must be rejected");
    assert_eq!(insecure.code, -32602);

    let server = MockServer::start().await;
    let forbidden_header = validate_upload_descriptor(FileTransferUploadDescriptor::HttpsPut {
        url: format!("{}/upload", server.uri()),
        headers: vec![FileTransferHeader {
            name: "Authorization".to_string(),
            value: "secret".to_string(),
        }],
        expires_at_unix_seconds: unix_seconds(SystemTime::now() + Duration::from_secs(60)),
    })
    .await
    .expect_err("authorization header must be rejected");
    assert_eq!(forbidden_header.code, -32602);

    let missing_expiry = validate_upload_descriptor(FileTransferUploadDescriptor::HttpsPut {
        url: format!("{}/upload", server.uri()),
        headers: Vec::new(),
        expires_at_unix_seconds: 0,
    })
    .await
    .expect_err("expired descriptor must be rejected");
    assert_eq!(missing_expiry.code, -32602);
}
