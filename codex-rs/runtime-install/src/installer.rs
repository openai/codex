use std::io;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RuntimeInstallManifestParams;
use codex_app_server_protocol::RuntimeInstallParams;
use codex_app_server_protocol::RuntimeInstallProgressNotification;
use codex_app_server_protocol::RuntimeInstallProgressPhase;
use codex_app_server_protocol::RuntimeInstallResponse;
use codex_app_server_protocol::RuntimeInstallStatus;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::Environment;
use codex_exec_server::RemoveOptions;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::errors::internal_error;
use crate::errors::invalid_params;
use crate::executor::RuntimeExecutor;
use crate::executor::TargetPlatform;
use crate::executor::path_uri;
use crate::executor::runtime_install_canceled;
use crate::validation::absolute_path;
use crate::validation::assert_archive_entries_stay_within_directory;
use crate::validation::default_archive_name;
use crate::validation::read_installed_runtime_metadata;
use crate::validation::runtime_archive_format;
use crate::validation::runtime_root_directory_name;
use crate::validation::validate_manifest;
use crate::validation::validate_runtime_root;

pub type RuntimeInstallProgressSender = mpsc::UnboundedSender<RuntimeInstallProgressNotification>;

#[derive(Clone)]
struct RuntimeInstallProgressReporter {
    bundle_version: Option<String>,
    sender: Option<RuntimeInstallProgressSender>,
}

impl RuntimeInstallProgressReporter {
    fn new(bundle_version: Option<String>, sender: Option<RuntimeInstallProgressSender>) -> Self {
        Self {
            bundle_version,
            sender,
        }
    }

    fn phase(&self, phase: RuntimeInstallProgressPhase) {
        self.send(
            phase, /*downloaded_bytes*/ None, /*total_bytes*/ None,
        );
    }

    fn download_progress(&self, downloaded_bytes: u64, total_bytes: Option<u64>) {
        self.send(
            RuntimeInstallProgressPhase::Downloading,
            Some(downloaded_bytes),
            total_bytes,
        );
    }

    fn send(
        &self,
        phase: RuntimeInstallProgressPhase,
        downloaded_bytes: Option<u64>,
        total_bytes: Option<u64>,
    ) {
        let Some(sender) = self.sender.as_ref() else {
            return;
        };
        let _ = sender.send(RuntimeInstallProgressNotification {
            bundle_version: self.bundle_version.clone(),
            downloaded_bytes,
            phase,
            total_bytes,
        });
    }
}

pub async fn install_runtime_with_progress(
    environment: &Environment,
    params: RuntimeInstallParams,
    progress: RuntimeInstallProgressSender,
    cancellation: CancellationToken,
) -> Result<RuntimeInstallResponse, JSONRPCErrorError> {
    let executor = RuntimeExecutor::new(environment).await?;
    let target = executor.discover_target(&cancellation).await?;
    install_runtime_with_root_and_control(
        &executor,
        params,
        target.install_root,
        target.platform,
        Some(progress),
        cancellation,
    )
    .await
}

async fn install_runtime_with_root_and_control(
    executor: &RuntimeExecutor,
    params: RuntimeInstallParams,
    install_root: AbsolutePathBuf,
    platform: TargetPlatform,
    progress: Option<RuntimeInstallProgressSender>,
    cancellation: CancellationToken,
) -> Result<RuntimeInstallResponse, JSONRPCErrorError> {
    validate_manifest(&params.manifest)?;
    let archive_format = runtime_archive_format(&params.manifest)?;
    let archive_name = params
        .manifest
        .archive_name
        .clone()
        .unwrap_or_else(|| default_archive_name(archive_format).to_string());

    let progress =
        RuntimeInstallProgressReporter::new(params.manifest.bundle_version.clone(), progress);
    progress.phase(RuntimeInstallProgressPhase::Checking);
    ensure_not_cancelled(&cancellation)?;
    if let Some(response) = reuse_current_runtime(
        executor,
        &params.manifest,
        &install_root,
        platform,
        &progress,
        &cancellation,
    )
    .await?
    {
        return Ok(response);
    }

    let staging_dir = make_staging_dir(executor, &install_root).await?;
    let archive_path = absolute_path(staging_dir.as_path().join(archive_name))?;
    let result = async {
        progress.download_progress(
            /*downloaded_bytes*/ 0,
            params.manifest.archive_size_bytes,
        );
        executor
            .download_archive(
                platform,
                &params.manifest.archive_url,
                &archive_path,
                &cancellation,
            )
            .await?;
        if let Some(total_bytes) = params.manifest.archive_size_bytes {
            progress.download_progress(total_bytes, Some(total_bytes));
        }
        install_runtime_from_archive_with_control(
            executor,
            &params.manifest,
            &archive_path,
            &install_root,
            platform,
            &progress,
            &cancellation,
        )
        .await
    }
    .await;
    cleanup_directory(executor, &staging_dir, "runtime install staging directory").await;
    result
}

async fn install_runtime_from_archive_with_control(
    executor: &RuntimeExecutor,
    manifest: &RuntimeInstallManifestParams,
    archive_path: &AbsolutePathBuf,
    install_root: &AbsolutePathBuf,
    platform: TargetPlatform,
    progress: &RuntimeInstallProgressReporter,
    cancellation: &CancellationToken,
) -> Result<RuntimeInstallResponse, JSONRPCErrorError> {
    let runtime_root_directory_name = runtime_root_directory_name(manifest)?;
    let installed_runtime_root =
        absolute_path(install_root.as_path().join(&runtime_root_directory_name))?;

    if let Some(response) = reuse_current_runtime(
        executor,
        manifest,
        install_root,
        platform,
        progress,
        cancellation,
    )
    .await?
    {
        return Ok(response);
    }

    create_directory(executor, install_root).await?;
    progress.phase(RuntimeInstallProgressPhase::Verifying);
    verify_archive_checksum(executor, platform, archive_path, manifest, cancellation).await?;

    let archive_format = runtime_archive_format(manifest)?;
    ensure_not_cancelled(cancellation)?;
    let staging_dir = make_staging_dir(executor, install_root).await?;
    let result = async {
        let extract_dir = absolute_path(staging_dir.as_path().join("payload"))?;
        create_directory(executor, &extract_dir).await?;

        progress.phase(RuntimeInstallProgressPhase::Extracting);
        ensure_not_cancelled(cancellation)?;
        let entries = executor
            .list_archive_entries(archive_format, platform, archive_path, cancellation)
            .await?;
        assert_archive_entries_stay_within_directory(&entries, extract_dir.as_path())?;
        ensure_not_cancelled(cancellation)?;
        executor
            .extract_archive(
                archive_format,
                platform,
                archive_path,
                &extract_dir,
                cancellation,
            )
            .await?;

        let extracted_runtime_root =
            absolute_path(extract_dir.as_path().join(&runtime_root_directory_name))?;
        progress.phase(RuntimeInstallProgressPhase::Validating);
        ensure_not_cancelled(cancellation)?;
        validate_runtime_root(
            executor,
            &extracted_runtime_root,
            manifest.bundle_format_version,
            platform,
        )
        .await?;
        ensure_not_cancelled(cancellation)?;

        let previous_runtime_root = absolute_path(
            install_root
                .as_path()
                .join(format!("{runtime_root_directory_name}.previous")),
        )?;
        remove_dir_if_exists(executor, &previous_runtime_root).await?;
        if path_exists(executor, &installed_runtime_root).await? {
            executor
                .move_directory(
                    platform,
                    &installed_runtime_root,
                    &previous_runtime_root,
                    cancellation,
                )
                .await?;
        }

        let install_result = async {
            executor
                .move_directory(
                    platform,
                    &extracted_runtime_root,
                    &installed_runtime_root,
                    cancellation,
                )
                .await?;
            validate_runtime_root(
                executor,
                &installed_runtime_root,
                manifest.bundle_format_version,
                platform,
            )
            .await
        }
        .await;

        let paths = match install_result {
            Ok(paths) => paths,
            Err(error) => {
                remove_dir_if_exists(executor, &installed_runtime_root).await?;
                if path_exists(executor, &previous_runtime_root).await? {
                    executor
                        .move_directory(
                            platform,
                            &previous_runtime_root,
                            &installed_runtime_root,
                            cancellation,
                        )
                        .await?;
                }
                return Err(error);
            }
        };
        remove_dir_if_exists(executor, &previous_runtime_root).await?;
        Ok(RuntimeInstallResponse {
            bundle_version: manifest.bundle_version.clone(),
            paths,
            status: RuntimeInstallStatus::Installed,
        })
    }
    .await;
    cleanup_directory(
        executor,
        &staging_dir,
        "runtime install extraction directory",
    )
    .await;
    if result.is_ok() {
        progress.phase(RuntimeInstallProgressPhase::Installed);
    }
    result
}

async fn reuse_current_runtime(
    executor: &RuntimeExecutor,
    manifest: &RuntimeInstallManifestParams,
    install_root: &AbsolutePathBuf,
    platform: TargetPlatform,
    progress: &RuntimeInstallProgressReporter,
    cancellation: &CancellationToken,
) -> Result<Option<RuntimeInstallResponse>, JSONRPCErrorError> {
    let installed_runtime_root = absolute_path(
        install_root
            .as_path()
            .join(runtime_root_directory_name(manifest)?),
    )?;
    ensure_not_cancelled(cancellation)?;
    if let Some(bundle_version) = manifest.bundle_version.as_ref()
        && let Ok(Some(metadata)) =
            read_installed_runtime_metadata(executor, &installed_runtime_root).await
        && metadata.bundle_version.as_ref() == Some(bundle_version)
        && let Ok(paths) = validate_runtime_root(
            executor,
            &installed_runtime_root,
            manifest.bundle_format_version,
            platform,
        )
        .await
    {
        progress.phase(RuntimeInstallProgressPhase::Installed);
        return Ok(Some(RuntimeInstallResponse {
            bundle_version: Some(bundle_version.clone()),
            paths,
            status: RuntimeInstallStatus::AlreadyCurrent,
        }));
    }
    Ok(None)
}

async fn make_staging_dir(
    executor: &RuntimeExecutor,
    install_root: &AbsolutePathBuf,
) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    create_directory(executor, install_root).await?;
    let staging_dir = absolute_path(
        install_root
            .as_path()
            .join(format!(".codex-runtime-install-{}", Uuid::now_v7())),
    )?;
    create_directory(executor, &staging_dir).await?;
    Ok(staging_dir)
}

async fn verify_archive_checksum(
    executor: &RuntimeExecutor,
    platform: TargetPlatform,
    archive_path: &AbsolutePathBuf,
    manifest: &RuntimeInstallManifestParams,
    cancellation: &CancellationToken,
) -> Result<(), JSONRPCErrorError> {
    let actual_sha256 = executor
        .archive_checksum(platform, archive_path, cancellation)
        .await?;
    if !actual_sha256.eq_ignore_ascii_case(&manifest.archive_sha256) {
        return Err(invalid_params(format!(
            "checksum mismatch for '{}': expected {}, got {actual_sha256}",
            manifest.archive_url, manifest.archive_sha256
        )));
    }
    Ok(())
}

async fn path_exists(
    executor: &RuntimeExecutor,
    path: &AbsolutePathBuf,
) -> Result<bool, JSONRPCErrorError> {
    match executor
        .filesystem()
        .get_metadata(&path_uri(path), /*sandbox*/ None)
        .await
    {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(internal_error(format!(
            "failed to inspect runtime path {}: {err}",
            path.display()
        ))),
    }
}

async fn create_directory(
    executor: &RuntimeExecutor,
    path: &AbsolutePathBuf,
) -> Result<(), JSONRPCErrorError> {
    executor
        .filesystem()
        .create_directory(
            &path_uri(path),
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await
        .map_err(|err| {
            internal_error(format!(
                "failed to create runtime directory {}: {err}",
                path.display()
            ))
        })
}

async fn remove_dir_if_exists(
    executor: &RuntimeExecutor,
    path: &AbsolutePathBuf,
) -> Result<(), JSONRPCErrorError> {
    match executor
        .filesystem()
        .remove(
            &path_uri(path),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await
    {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(internal_error(format!(
            "failed to remove runtime directory {}: {err}",
            path.display()
        ))),
    }
}

async fn cleanup_directory(executor: &RuntimeExecutor, path: &AbsolutePathBuf, label: &str) {
    if let Err(error) = remove_dir_if_exists(executor, path).await {
        tracing::warn!(
            path = %path.display(),
            error = %error.message,
            "failed to clean up {label}"
        );
    }
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), JSONRPCErrorError> {
    if cancellation.is_cancelled() {
        Err(runtime_install_canceled())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
