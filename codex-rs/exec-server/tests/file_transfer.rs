#![cfg(unix)]

mod common;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_exec_server::Environment;
use codex_exec_server::ExecServerError;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::FileTransferDigestAlgorithm;
use codex_exec_server::FileTransferOperationState;
use codex_exec_server::FileTransferPrepareUploadParams;
use codex_exec_server::FileTransferStartUploadParams;
use codex_exec_server::FileTransferUploadDescriptor;
use codex_exec_server::MAX_PREPARED_FILE_UPLOAD_BYTES;
use codex_exec_server::PREPARED_FILE_UPLOAD_PROTOCOL_VERSION;
use codex_protocol::models::PermissionProfile;
use codex_utils_path_uri::PathUri;
use common::exec_server::exec_server;
use common::exec_server::exec_server_with_env;
use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_bytes;
use wiremock::matchers::method;
use wiremock::matchers::path;

const FILE_TRANSFER_ENABLED_ENV_VAR: &str = "CODEX_EXEC_SERVER_PREPARED_FILE_UPLOAD_ENABLED";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_environment_prepares_and_cancels_upload_without_returning_bytes()
-> anyhow::Result<()> {
    let source_dir = tempfile::tempdir()?;
    let source = source_dir.path().join("report.txt");
    let source_bytes = b"executor-owned bytes";
    tokio::fs::write(&source, source_bytes).await?;

    let mut server = exec_server_with_env([(FILE_TRANSFER_ENABLED_ENV_VAR, "1")]).await?;
    let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
    let capability = environment
        .info()
        .await?
        .capabilities
        .prepared_file_upload
        .expect("development-enabled executor should advertise upload support");
    assert_eq!(
        capability.protocol_version,
        PREPARED_FILE_UPLOAD_PROTOCOL_VERSION
    );
    assert_eq!(capability.max_upload_bytes, MAX_PREPARED_FILE_UPLOAD_BYTES);
    assert!(capability.supports_status_reconciliation);

    let prepared = environment
        .file_transfer_prepare_upload(FileTransferPrepareUploadParams {
            path: PathUri::from_path(&source)?,
            sandbox: full_access_context(),
            max_bytes: 1024,
        })
        .await?;
    assert_eq!(prepared.name, "report.txt");
    assert_eq!(prepared.size, source_bytes.len() as u64);
    assert_eq!(
        prepared.digest.algorithm,
        FileTransferDigestAlgorithm::Sha256
    );
    assert_eq!(
        prepared.digest.value,
        URL_SAFE_NO_PAD.encode(Sha256::digest(source_bytes))
    );

    let status = environment
        .file_transfer_status(prepared.transfer_id.clone())
        .await?;
    assert_eq!(status.state, FileTransferOperationState::Prepared);
    let canceled = environment
        .file_transfer_cancel(prepared.transfer_id.clone())
        .await?;
    assert_eq!(canceled.state, FileTransferOperationState::Canceled);
    assert_eq!(
        environment
            .file_transfer_status(prepared.transfer_id)
            .await?
            .state,
        FileTransferOperationState::Canceled
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_hides_and_rejects_file_transfer_by_default() -> anyhow::Result<()> {
    let source_dir = tempfile::tempdir()?;
    let source = source_dir.path().join("report.txt");
    tokio::fs::write(&source, b"must not be read").await?;
    let mut server = exec_server().await?;
    let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
    assert!(
        environment
            .info()
            .await?
            .capabilities
            .prepared_file_upload
            .is_none()
    );
    let error = environment
        .file_transfer_prepare_upload(FileTransferPrepareUploadParams {
            path: PathUri::from_path(&source)?,
            sandbox: full_access_context(),
            max_bytes: 1024,
        })
        .await
        .expect_err("default-disabled executor must reject prepare");
    assert!(matches!(
        error,
        ExecServerError::Server {
            code: -32600,
            ref message,
        } if message == "prepared file upload is disabled on this executor"
    ));

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn remote_environment_file_transfer_resumes_then_uploads_executor_owned_bytes()
-> anyhow::Result<()> {
    let destination = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/upload"))
        .and(body_bytes(b"bytes across reconnect".as_slice()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&destination)
        .await;
    let source_dir = tempfile::tempdir()?;
    let source = source_dir.path().join("report.txt");
    tokio::fs::write(&source, b"bytes across reconnect").await?;
    let mut server = exec_server_with_env([(FILE_TRANSFER_ENABLED_ENV_VAR, "1")]).await?;
    let mut proxy = server.disconnectable_websocket_proxy().await?;
    let environment = Environment::create_for_tests(Some(proxy.websocket_url().to_string()))?;
    let prepared = environment
        .file_transfer_prepare_upload(FileTransferPrepareUploadParams {
            path: PathUri::from_path(&source)?,
            sandbox: full_access_context(),
            max_bytes: 1024,
        })
        .await?;

    proxy.pause_and_disconnect().await?;
    let environment_for_status = environment.clone();
    let transfer_id_for_status = prepared.transfer_id.clone();
    let mut pending_status = tokio::spawn(async move {
        environment_for_status
            .file_transfer_status(transfer_id_for_status)
            .await
    });
    assert!(
        timeout(Duration::from_millis(200), &mut pending_status)
            .await
            .is_err(),
        "status should wait while the executor session is recovering"
    );
    proxy.resume()?;
    let recovered = timeout(Duration::from_secs(5), pending_status).await???;
    assert_eq!(recovered.state, FileTransferOperationState::Prepared);

    let started = environment
        .file_transfer_start_upload(FileTransferStartUploadParams {
            transfer_id: prepared.transfer_id,
            descriptor: FileTransferUploadDescriptor::HttpsPut {
                url: format!("{}/upload", destination.uri()),
                headers: Vec::new(),
                expires_at_unix_seconds: unix_seconds(SystemTime::now() + Duration::from_secs(60)),
            },
        })
        .await?;
    let terminal = timeout(Duration::from_secs(5), async {
        loop {
            let status = environment
                .file_transfer_status(started.transfer_id.clone())
                .await?;
            if status.state != FileTransferOperationState::Uploading {
                return Ok::<_, ExecServerError>(status);
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;
    assert_eq!(terminal.state, FileTransferOperationState::Succeeded);

    server.shutdown().await?;
    Ok(())
}

fn full_access_context() -> FileSystemSandboxContext {
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::Disabled)
}

fn unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
