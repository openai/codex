use std::path::Path;

use codex_app_server_protocol::RuntimeInstallManifestParams;
use codex_app_server_protocol::RuntimeInstallParams;
use codex_app_server_protocol::RuntimeInstallProgressPhase;
use codex_app_server_protocol::RuntimeInstallStatus;
use codex_exec_server::Environment;
use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;
use tokio::fs;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::validation::PUBLISHED_ARTIFACT_NAME;
use crate::validation::node_executable_name;
use crate::validation::python_executable_name;

#[test]
fn archive_traversal_entries_are_rejected() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let entries = vec![
        "codex-primary-runtime/runtime.json".to_string(),
        "../x".to_string(),
    ];

    let error = assert_archive_entries_stay_within_directory(&entries, temp_dir.path())
        .expect_err("entry should be rejected");

    assert!(error.message.contains("would extract outside target"));
}

#[tokio::test]
async fn install_runtime_reuses_current_runtime_without_downloading_archive() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let executor = test_executor().await;
    let install_root = absolute_path(temp_dir.path().join("install")).expect("install root");
    let runtime_root =
        absolute_path(install_root.as_path().join(PUBLISHED_ARTIFACT_NAME)).expect("runtime root");
    create_runtime_root(runtime_root.as_path(), "v1").await;
    let archive_path = temp_dir.path().join("unused.tar.xz");
    fs::write(&archive_path, b"not used")
        .await
        .expect("write archive");
    let mut manifest = manifest_for_archive(&archive_path, "v1").await;
    manifest.archive_url = "not a valid archive URL".to_string();

    let response = install_runtime_with_root_and_control(
        &executor,
        RuntimeInstallParams {
            environment_id: None,
            manifest: Box::new(manifest),
            release: "primary".to_string(),
        },
        install_root,
        local_platform(),
        /*progress*/ None,
        CancellationToken::new(),
    )
    .await
    .expect("installed runtime should be reused without downloading");

    assert_eq!(response.status, RuntimeInstallStatus::AlreadyCurrent);
    assert_eq!(response.bundle_version.as_deref(), Some("v1"));
}

#[tokio::test]
async fn validate_runtime_root_rejects_missing_node_executable() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let executor = test_executor().await;
    let runtime_root =
        absolute_path(temp_dir.path().join(PUBLISHED_ARTIFACT_NAME)).expect("runtime root");
    create_runtime_root(runtime_root.as_path(), "v1").await;
    fs::remove_file(
        runtime_root
            .as_path()
            .join("dependencies")
            .join("node")
            .join("bin")
            .join(node_executable_name(local_platform().runtime_name())),
    )
    .await
    .expect("remove node");

    let error = validate_runtime_root(&executor, &runtime_root, Some(2), local_platform())
        .await
        .expect_err("node executable should be required");

    assert!(error.message.contains("node executable is missing"));
}

#[tokio::test]
async fn validate_runtime_root_rejects_missing_node_modules_directory() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let executor = test_executor().await;
    let runtime_root =
        absolute_path(temp_dir.path().join(PUBLISHED_ARTIFACT_NAME)).expect("runtime root");
    create_runtime_root(runtime_root.as_path(), "v1").await;
    fs::remove_dir(
        runtime_root
            .as_path()
            .join("dependencies")
            .join("node")
            .join("node_modules"),
    )
    .await
    .expect("remove node_modules");

    let error = validate_runtime_root(&executor, &runtime_root, Some(2), local_platform())
        .await
        .expect_err("node_modules directory should be required");

    assert!(error.message.contains("node modules directory is missing"));
}

#[tokio::test]
async fn validate_runtime_root_rejects_missing_python_executable() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let executor = test_executor().await;
    let runtime_root =
        absolute_path(temp_dir.path().join(PUBLISHED_ARTIFACT_NAME)).expect("runtime root");
    create_runtime_root(runtime_root.as_path(), "v1").await;
    fs::remove_file(
        runtime_root
            .as_path()
            .join("dependencies")
            .join("python")
            .join("bin")
            .join(python_executable_name(local_platform().runtime_name())),
    )
    .await
    .expect("remove python");

    let error = validate_runtime_root(&executor, &runtime_root, Some(2), local_platform())
        .await
        .expect_err("python executable should be required");

    assert!(error.message.contains("python executable is missing"));
}

#[tokio::test]
async fn install_from_archive_rejects_checksum_mismatch() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let executor = test_executor().await;
    let archive_path = absolute_path(temp_dir.path().join("archive.tar.xz")).expect("archive path");
    fs::write(archive_path.as_path(), b"archive")
        .await
        .expect("write archive");
    let manifest = RuntimeInstallManifestParams {
        archive_name: None,
        archive_sha256: "0".repeat(64),
        archive_size_bytes: None,
        archive_url: "https://example.com/archive.tar.xz".to_string(),
        bundle_format_version: Some(2),
        bundle_version: Some("v1".to_string()),
        format: Some("tar.xz".to_string()),
        runtime_root_directory_name: None,
    };
    let install_root = absolute_path(temp_dir.path().join("install")).expect("install root");

    let error = install_runtime_from_archive_with_control(
        &executor,
        &manifest,
        &archive_path,
        &install_root,
        local_platform(),
        &RuntimeInstallProgressReporter::new(manifest.bundle_version.clone(), None),
        &CancellationToken::new(),
    )
    .await
    .expect_err("checksum mismatch should fail");

    assert!(error.message.contains("checksum mismatch"));
}

#[tokio::test]
async fn install_from_archive_reports_install_progress() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let executor = test_executor().await;
    let payload_root = temp_dir
        .path()
        .join("payload")
        .join(PUBLISHED_ARTIFACT_NAME);
    create_runtime_root(&payload_root, "v1").await;
    let archive_path = absolute_path(temp_dir.path().join("archive.tar.xz")).expect("archive path");
    create_tar_xz(&temp_dir.path().join("payload"), archive_path.as_path()).await;
    let manifest = manifest_for_archive(archive_path.as_path(), "v1").await;
    let install_root = absolute_path(temp_dir.path().join("install")).expect("install root");
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
    let progress =
        RuntimeInstallProgressReporter::new(manifest.bundle_version.clone(), Some(progress_tx));

    install_runtime_from_archive_with_control(
        &executor,
        &manifest,
        &archive_path,
        &install_root,
        local_platform(),
        &progress,
        &CancellationToken::new(),
    )
    .await
    .expect("install should succeed");

    let mut phases = Vec::new();
    while let Ok(notification) = progress_rx.try_recv() {
        phases.push(notification.phase);
    }
    assert_eq!(
        phases,
        vec![
            RuntimeInstallProgressPhase::Verifying,
            RuntimeInstallProgressPhase::Extracting,
            RuntimeInstallProgressPhase::Validating,
            RuntimeInstallProgressPhase::Installed,
        ]
    );
}

#[tokio::test]
async fn install_from_archive_stops_when_canceled() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let executor = test_executor().await;
    let archive_path = absolute_path(temp_dir.path().join("unused.tar.xz")).expect("archive path");
    fs::write(archive_path.as_path(), b"unused")
        .await
        .expect("write archive");
    let manifest = manifest_for_archive(archive_path.as_path(), "v1").await;
    let install_root = absolute_path(temp_dir.path().join("install")).expect("install root");
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let error = install_runtime_from_archive_with_control(
        &executor,
        &manifest,
        &archive_path,
        &install_root,
        local_platform(),
        &RuntimeInstallProgressReporter::new(manifest.bundle_version.clone(), None),
        &cancellation,
    )
    .await
    .expect_err("canceled install should fail");

    assert_eq!(error.message, "runtime install canceled");
}

async fn test_executor() -> RuntimeExecutor {
    RuntimeExecutor::new(&Environment::default_for_tests())
        .await
        .expect("test executor")
}

fn local_platform() -> TargetPlatform {
    if cfg!(target_os = "windows") {
        TargetPlatform::Windows
    } else {
        TargetPlatform::Unix
    }
}

async fn create_runtime_root(runtime_root: &Path, bundle_version: &str) {
    let node_bin = runtime_root.join("dependencies").join("node").join("bin");
    let python_bin = runtime_root.join("dependencies").join("python").join("bin");
    fs::create_dir_all(&node_bin).await.expect("node bin");
    fs::create_dir_all(
        runtime_root
            .join("dependencies")
            .join("node")
            .join("node_modules"),
    )
    .await
    .expect("node_modules");
    fs::create_dir_all(&python_bin).await.expect("python bin");
    fs::write(
        node_bin.join(node_executable_name(local_platform().runtime_name())),
        b"node",
    )
    .await
    .expect("node");
    fs::write(
        python_bin.join(python_executable_name(local_platform().runtime_name())),
        b"python",
    )
    .await
    .expect("python");
    fs::write(
        runtime_root.join("runtime.json"),
        format!(r#"{{"bundleFormatVersion":2,"bundleVersion":"{bundle_version}"}}"#),
    )
    .await
    .expect("runtime metadata");
}

async fn manifest_for_archive(
    archive_path: &Path,
    bundle_version: &str,
) -> RuntimeInstallManifestParams {
    RuntimeInstallManifestParams {
        archive_name: None,
        archive_sha256: compute_sha256(archive_path).await,
        archive_size_bytes: None,
        archive_url: "https://example.com/archive.tar.xz".to_string(),
        bundle_format_version: Some(2),
        bundle_version: Some(bundle_version.to_string()),
        format: Some("tar.xz".to_string()),
        runtime_root_directory_name: None,
    }
}

async fn compute_sha256(path: &Path) -> String {
    let bytes = fs::read(path).await.expect("read archive");
    format!("{:x}", Sha256::digest(bytes))
}

async fn create_tar_xz(payload_dir: &Path, archive_path: &Path) {
    let output = Command::new("tar")
        .arg("-cJf")
        .arg(archive_path)
        .arg("-C")
        .arg(payload_dir)
        .arg(".")
        .output()
        .await
        .expect("tar should run");
    assert!(
        output.status.success(),
        "tar failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
