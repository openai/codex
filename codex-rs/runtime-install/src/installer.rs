use std::future::Future;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RuntimeInstallManifestParams;
use codex_app_server_protocol::RuntimeInstallParams;
use codex_app_server_protocol::RuntimeInstallPaths;
use codex_app_server_protocol::RuntimeInstallProgressNotification;
use codex_app_server_protocol::RuntimeInstallProgressPhase;
use codex_app_server_protocol::RuntimeInstallResponse;
use codex_app_server_protocol::RuntimeInstallStatus;
use codex_utils_absolute_path::AbsolutePathBuf;
use futures::StreamExt;
use serde::Deserialize;
use sha2::Digest;
use sha2::Sha256;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::errors::internal_error;
use crate::errors::invalid_params;

const PUBLISHED_ARTIFACT_NAME: &str = "codex-primary-runtime";
const USER_AGENT: &str = "codex-runtime-installer";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeArchiveFormat {
    TarXz,
    Zip,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledRuntimeMetadata {
    bundle_format_version: Option<u32>,
    bundle_version: Option<String>,
    bundled_plugins: Option<Vec<String>>,
    bundled_skills: Option<Vec<String>>,
    skills_to_remove: Option<Vec<String>>,
}

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

pub(crate) async fn install_runtime_with_progress(
    params: RuntimeInstallParams,
    progress: RuntimeInstallProgressSender,
    cancellation: CancellationToken,
) -> Result<RuntimeInstallResponse, JSONRPCErrorError> {
    let install_root = default_install_root()?;
    install_runtime_with_root_and_control(params, install_root, Some(progress), cancellation).await
}

async fn install_runtime_with_root_and_control(
    params: RuntimeInstallParams,
    install_root: PathBuf,
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
    validate_path_segment(&archive_name, "archiveName")?;

    let progress =
        RuntimeInstallProgressReporter::new(params.manifest.bundle_version.clone(), progress);
    progress.phase(RuntimeInstallProgressPhase::Checking);
    ensure_not_cancelled(&cancellation)?;
    if let Some(response) =
        reuse_current_runtime(&params.manifest, &install_root, &progress, &cancellation).await?
    {
        return Ok(response);
    }
    let staging_dir = make_staging_dir(&install_root).await?;
    let archive_path = staging_dir.join(archive_name);
    let result = async {
        progress.download_progress(
            /*downloaded_bytes*/ 0,
            params.manifest.archive_size_bytes,
        );
        download_archive(
            &params.manifest.archive_url,
            &archive_path,
            params.manifest.archive_size_bytes,
            &progress,
            &cancellation,
        )
        .await?;
        install_runtime_from_archive_with_control(
            &params.manifest,
            &archive_path,
            &install_root,
            &progress,
            &cancellation,
        )
        .await
    }
    .await;
    let cleanup_result = fs::remove_dir_all(&staging_dir).await;
    if let Err(err) = cleanup_result
        && err.kind() != ErrorKind::NotFound
    {
        tracing::warn!(
            "failed to remove runtime install staging directory {}: {err}",
            staging_dir.display()
        );
    }
    result
}

#[cfg(test)]
async fn install_runtime_from_archive(
    manifest: &RuntimeInstallManifestParams,
    archive_path: &Path,
    install_root: &Path,
) -> Result<RuntimeInstallResponse, JSONRPCErrorError> {
    install_runtime_from_archive_with_control(
        manifest,
        archive_path,
        install_root,
        &RuntimeInstallProgressReporter::new(manifest.bundle_version.clone(), None),
        &CancellationToken::new(),
    )
    .await
}

async fn install_runtime_from_archive_with_control(
    manifest: &RuntimeInstallManifestParams,
    archive_path: &Path,
    install_root: &Path,
    progress: &RuntimeInstallProgressReporter,
    cancellation: &CancellationToken,
) -> Result<RuntimeInstallResponse, JSONRPCErrorError> {
    let runtime_root_directory_name = runtime_root_directory_name(manifest)?;
    let installed_runtime_root = install_root.join(&runtime_root_directory_name);
    let target_platform = target_platform();

    if let Some(response) =
        reuse_current_runtime(manifest, install_root, progress, cancellation).await?
    {
        return Ok(response);
    }

    fs::create_dir_all(install_root)
        .await
        .map_err(|err| internal_error(format!("failed to create runtime install root: {err}")))?;

    progress.phase(RuntimeInstallProgressPhase::Verifying);
    verify_archive_checksum(
        archive_path,
        &manifest.archive_sha256,
        &manifest.archive_url,
        cancellation,
    )
    .await?;

    let archive_format = runtime_archive_format(manifest)?;
    ensure_not_cancelled(cancellation)?;
    let staging_dir = make_staging_dir(install_root).await?;
    let result = async {
        let extract_dir = staging_dir.join("payload");
        fs::create_dir_all(&extract_dir).await.map_err(|err| {
            internal_error(format!("failed to create runtime extract dir: {err}"))
        })?;

        progress.phase(RuntimeInstallProgressPhase::Extracting);
        ensure_not_cancelled(cancellation)?;
        let entries = list_archive_entries(archive_format, archive_path).await?;
        assert_archive_entries_stay_within_directory(&entries, &extract_dir)?;
        ensure_not_cancelled(cancellation)?;
        extract_archive(archive_format, archive_path, &extract_dir).await?;

        let extracted_runtime_root = extract_dir.join(&runtime_root_directory_name);
        progress.phase(RuntimeInstallProgressPhase::Validating);
        ensure_not_cancelled(cancellation)?;
        validate_runtime_root(
            &extracted_runtime_root,
            manifest.bundle_format_version,
            target_platform,
        )
        .await?;
        ensure_not_cancelled(cancellation)?;

        let previous_runtime_root =
            install_root.join(format!("{runtime_root_directory_name}.previous"));
        remove_dir_if_exists(&previous_runtime_root).await?;
        if path_exists(&installed_runtime_root).await {
            fs::rename(&installed_runtime_root, &previous_runtime_root)
                .await
                .map_err(|err| {
                    internal_error(format!("failed to move previous runtime aside: {err}"))
                })?;
        }

        let install_result = async {
            fs::rename(&extracted_runtime_root, &installed_runtime_root)
                .await
                .map_err(|err| internal_error(format!("failed to install runtime: {err}")))?;
            validate_runtime_root(
                &installed_runtime_root,
                manifest.bundle_format_version,
                target_platform,
            )
            .await
        }
        .await;

        let paths = match install_result {
            Ok(paths) => paths,
            Err(error) => {
                remove_dir_if_exists(&installed_runtime_root).await?;
                if path_exists(&previous_runtime_root).await {
                    fs::rename(&previous_runtime_root, &installed_runtime_root)
                        .await
                        .map_err(|err| {
                            internal_error(format!("failed to restore previous runtime: {err}"))
                        })?;
                }
                return Err(error);
            }
        };
        remove_dir_if_exists(&previous_runtime_root).await?;
        Ok(RuntimeInstallResponse {
            bundle_version: manifest.bundle_version.clone(),
            paths,
            status: RuntimeInstallStatus::Installed,
        })
    }
    .await;
    let cleanup_result = fs::remove_dir_all(&staging_dir).await;
    if let Err(err) = cleanup_result
        && err.kind() != ErrorKind::NotFound
    {
        tracing::warn!(
            "failed to remove runtime install extraction directory {}: {err}",
            staging_dir.display()
        );
    }
    if result.is_ok() {
        progress.phase(RuntimeInstallProgressPhase::Installed);
    }
    result
}

async fn reuse_current_runtime(
    manifest: &RuntimeInstallManifestParams,
    install_root: &Path,
    progress: &RuntimeInstallProgressReporter,
    cancellation: &CancellationToken,
) -> Result<Option<RuntimeInstallResponse>, JSONRPCErrorError> {
    let installed_runtime_root = install_root.join(runtime_root_directory_name(manifest)?);
    ensure_not_cancelled(cancellation)?;
    if let Some(bundle_version) = manifest.bundle_version.as_ref()
        && let Ok(Some(metadata)) = read_installed_runtime_metadata(&installed_runtime_root).await
        && metadata.bundle_version.as_ref() == Some(bundle_version)
        && let Ok(paths) = validate_runtime_root(
            &installed_runtime_root,
            manifest.bundle_format_version,
            target_platform(),
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

fn default_install_root() -> Result<PathBuf, JSONRPCErrorError> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| internal_error("failed to locate home directory for runtime install"))?;
    Ok(home.join(".cache").join("codex-runtimes"))
}

async fn make_staging_dir(install_root: &Path) -> Result<PathBuf, JSONRPCErrorError> {
    fs::create_dir_all(install_root)
        .await
        .map_err(|err| internal_error(format!("failed to create runtime install root: {err}")))?;
    tempfile::Builder::new()
        .prefix("codex-runtime-install-")
        .tempdir_in(install_root)
        .map(tempfile::TempDir::keep)
        .map_err(|err| {
            internal_error(format!(
                "failed to create runtime install staging dir: {err}"
            ))
        })
}

fn validate_manifest(manifest: &RuntimeInstallManifestParams) -> Result<(), JSONRPCErrorError> {
    if manifest.archive_url.trim().is_empty() {
        return Err(invalid_params(
            "runtime manifest archiveUrl must not be empty",
        ));
    }
    if !is_sha256(&manifest.archive_sha256) {
        return Err(invalid_params(
            "runtime manifest archiveSha256 must be a 64-character hex digest",
        ));
    }
    if let Some(archive_name) = manifest.archive_name.as_ref() {
        validate_path_segment(archive_name, "archiveName")?;
    }
    if let Some(runtime_root_directory_name) = manifest.runtime_root_directory_name.as_ref() {
        validate_path_segment(runtime_root_directory_name, "runtimeRootDirectoryName")?;
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_path_segment(value: &str, field_name: &str) -> Result<(), JSONRPCErrorError> {
    let value = value.trim();
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(invalid_params(format!(
            "runtime manifest {field_name} must be a single path segment"
        )));
    }
    Ok(())
}

fn runtime_root_directory_name(
    manifest: &RuntimeInstallManifestParams,
) -> Result<String, JSONRPCErrorError> {
    let runtime_root_directory_name = manifest
        .runtime_root_directory_name
        .clone()
        .unwrap_or_else(|| PUBLISHED_ARTIFACT_NAME.to_string());
    validate_path_segment(&runtime_root_directory_name, "runtimeRootDirectoryName")?;
    Ok(runtime_root_directory_name)
}

fn runtime_archive_format(
    manifest: &RuntimeInstallManifestParams,
) -> Result<RuntimeArchiveFormat, JSONRPCErrorError> {
    if let Some(format) = manifest.format.as_deref() {
        match format.to_ascii_lowercase().as_str() {
            "tar.xz" => return Ok(RuntimeArchiveFormat::TarXz),
            "zip" => return Ok(RuntimeArchiveFormat::Zip),
            _ => {
                return Err(invalid_params(format!(
                    "unsupported runtime archive format: {format}"
                )));
            }
        }
    }
    if manifest
        .archive_name
        .as_deref()
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".zip"))
        || manifest.archive_url.to_ascii_lowercase().ends_with(".zip")
    {
        return Ok(RuntimeArchiveFormat::Zip);
    }
    Ok(RuntimeArchiveFormat::TarXz)
}

fn default_archive_name(format: RuntimeArchiveFormat) -> &'static str {
    match format {
        RuntimeArchiveFormat::TarXz => "node-runtime.tar.xz",
        RuntimeArchiveFormat::Zip => "node-runtime.zip",
    }
}

async fn download_archive(
    url: &str,
    destination: &Path,
    expected_size_bytes: Option<u64>,
    progress: &RuntimeInstallProgressReporter,
    cancellation: &CancellationToken,
) -> Result<(), JSONRPCErrorError> {
    let response = tokio::select! {
        _ = cancellation.cancelled() => return Err(runtime_install_canceled()),
        response = reqwest::Client::new()
            .get(url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send() => response
    }
    .map_err(|err| internal_error(format!("failed to download runtime archive: {err}")))?;
    if !response.status().is_success() {
        return Err(internal_error(format!(
            "failed to download runtime archive ({} {})",
            response.status().as_u16(),
            response
                .status()
                .canonical_reason()
                .unwrap_or("unknown status")
        )));
    }

    let mut file = fs::File::create(destination)
        .await
        .map_err(|err| internal_error(format!("failed to create runtime archive file: {err}")))?;
    let total_bytes = response.content_length().or(expected_size_bytes);
    let mut downloaded_bytes = 0_u64;
    let mut stream = response.bytes_stream();
    loop {
        let chunk = tokio::select! {
            _ = cancellation.cancelled() => return Err(runtime_install_canceled()),
            chunk = stream.next() => chunk
        };
        let Some(chunk) = chunk else {
            break;
        };
        let chunk = chunk.map_err(|err| {
            internal_error(format!("failed to read runtime archive bytes: {err}"))
        })?;
        tokio::select! {
            _ = cancellation.cancelled() => return Err(runtime_install_canceled()),
            result = file.write_all(&chunk) => result
        }
        .map_err(|err| internal_error(format!("failed to write runtime archive: {err}")))?;
        downloaded_bytes += chunk.len() as u64;
        progress.download_progress(downloaded_bytes, total_bytes);
    }
    file.flush()
        .await
        .map_err(|err| internal_error(format!("failed to flush runtime archive: {err}")))?;
    Ok(())
}

async fn verify_archive_checksum(
    archive_path: &Path,
    expected_sha256: &str,
    source_url: &str,
    cancellation: &CancellationToken,
) -> Result<(), JSONRPCErrorError> {
    let actual_sha256 = compute_sha256_with_cancellation(archive_path, cancellation).await?;
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(invalid_params(format!(
            "checksum mismatch for '{source_url}': expected {expected_sha256}, got {actual_sha256}"
        )));
    }
    Ok(())
}

#[cfg(test)]
async fn compute_sha256(path: &Path) -> Result<String, JSONRPCErrorError> {
    compute_sha256_with_cancellation(path, &CancellationToken::new()).await
}

async fn compute_sha256_with_cancellation(
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<String, JSONRPCErrorError> {
    let mut file = fs::File::open(path)
        .await
        .map_err(|err| internal_error(format!("failed to open runtime archive: {err}")))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes_read = tokio::select! {
            _ = cancellation.cancelled() => return Err(runtime_install_canceled()),
            bytes_read = file.read(&mut buffer) => bytes_read
        }
        .map_err(|err| internal_error(format!("failed to read runtime archive: {err}")))?;
        if bytes_read == 0 {
            break;
        }
        digest.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), JSONRPCErrorError> {
    if cancellation.is_cancelled() {
        Err(runtime_install_canceled())
    } else {
        Ok(())
    }
}

fn runtime_install_canceled() -> JSONRPCErrorError {
    internal_error("runtime install canceled")
}

async fn list_archive_entries(
    format: RuntimeArchiveFormat,
    archive_path: &Path,
) -> Result<Vec<String>, JSONRPCErrorError> {
    match format {
        RuntimeArchiveFormat::TarXz => list_tar_entries(archive_path).await,
        RuntimeArchiveFormat::Zip => list_zip_entries(archive_path).await,
    }
}

async fn extract_archive(
    format: RuntimeArchiveFormat,
    archive_path: &Path,
    extract_dir: &Path,
) -> Result<(), JSONRPCErrorError> {
    match format {
        RuntimeArchiveFormat::TarXz => extract_tar_archive(archive_path, extract_dir).await,
        RuntimeArchiveFormat::Zip => extract_zip_archive(archive_path, extract_dir).await,
    }
}

async fn list_tar_entries(archive_path: &Path) -> Result<Vec<String>, JSONRPCErrorError> {
    let output = Command::new("tar")
        .arg("-tf")
        .arg(archive_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|err| internal_error(format!("failed to list runtime archive: {err}")))?;
    if !output.status.success() {
        return Err(invalid_params(format!(
            "failed to list runtime archive: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(parse_archive_entries(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

async fn extract_tar_archive(
    archive_path: &Path,
    extract_dir: &Path,
) -> Result<(), JSONRPCErrorError> {
    let output = Command::new("tar")
        .arg("-xJf")
        .arg(archive_path)
        .arg("-C")
        .arg(extract_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|err| internal_error(format!("failed to extract runtime archive: {err}")))?;
    if !output.status.success() {
        return Err(invalid_params(format!(
            "failed to extract runtime archive: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn list_zip_entries(
    archive_path: &Path,
) -> impl Future<Output = Result<Vec<String>, JSONRPCErrorError>> + Send + 'static {
    let archive_path = archive_path.to_path_buf();
    async move {
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&archive_path).map_err(|err| {
                internal_error(format!("failed to open runtime zip archive: {err}"))
            })?;
            let mut archive = zip::ZipArchive::new(file).map_err(|err| {
                invalid_params(format!("failed to read runtime zip archive: {err}"))
            })?;
            let mut entries = Vec::with_capacity(archive.len());
            for index in 0..archive.len() {
                let file = archive.by_index(index).map_err(|err| {
                    invalid_params(format!("failed to read runtime zip entry: {err}"))
                })?;
                entries.push(file.name().to_string());
            }
            Ok(entries)
        })
        .await
        .map_err(|err| internal_error(format!("failed to join zip listing task: {err}")))?
    }
}

fn extract_zip_archive(
    archive_path: &Path,
    extract_dir: &Path,
) -> impl Future<Output = Result<(), JSONRPCErrorError>> + Send + 'static {
    let archive_path = archive_path.to_path_buf();
    let extract_dir = extract_dir.to_path_buf();
    async move {
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&archive_path).map_err(|err| {
                internal_error(format!("failed to open runtime zip archive: {err}"))
            })?;
            let mut archive = zip::ZipArchive::new(file).map_err(|err| {
                invalid_params(format!("failed to read runtime zip archive: {err}"))
            })?;
            archive.extract(&extract_dir).map_err(|err| {
                invalid_params(format!("failed to extract runtime zip archive: {err}"))
            })?;
            Ok(())
        })
        .await
        .map_err(|err| internal_error(format!("failed to join zip extraction task: {err}")))?
    }
}

fn parse_archive_entries(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect()
}

fn assert_archive_entries_stay_within_directory(
    entries: &[String],
    extract_dir: &Path,
) -> Result<(), JSONRPCErrorError> {
    let resolved_extract_dir = normalize_path(extract_dir);
    for entry in entries {
        let resolved_entry_path = normalize_path(extract_dir.join(entry));
        if resolved_entry_path != resolved_extract_dir
            && !resolved_entry_path.starts_with(&resolved_extract_dir)
        {
            return Err(invalid_params(format!(
                "archive entry '{entry}' would extract outside target"
            )));
        }
    }
    Ok(())
}

fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

async fn read_installed_runtime_metadata(
    runtime_root: &Path,
) -> Result<Option<InstalledRuntimeMetadata>, JSONRPCErrorError> {
    let raw = match fs::read_to_string(runtime_root.join("runtime.json")).await {
        Ok(raw) => raw,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(internal_error(format!(
                "failed to read installed runtime metadata: {err}"
            )));
        }
    };
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|err| invalid_params(format!("failed to parse installed runtime metadata: {err}")))
}

async fn validate_runtime_root(
    runtime_root: &Path,
    manifest_bundle_format_version: Option<u32>,
    target_platform: &str,
) -> Result<RuntimeInstallPaths, JSONRPCErrorError> {
    let metadata = read_installed_runtime_metadata(runtime_root)
        .await?
        .ok_or_else(|| invalid_params("runtime metadata is missing"))?;
    let bundle_format_version = manifest_bundle_format_version
        .or(metadata.bundle_format_version)
        .unwrap_or(1);
    let node_root = if bundle_format_version >= 2 {
        runtime_root.join("dependencies").join("node")
    } else {
        runtime_root.to_path_buf()
    };
    let node_path = node_root
        .join("bin")
        .join(node_executable_name(target_platform));
    let node_modules_path = node_root.join("node_modules");
    require_runtime_file(&node_path, "node executable").await?;
    require_runtime_directory(&node_modules_path, "node modules directory").await?;
    let python_path =
        find_python_path(runtime_root, bundle_format_version, target_platform).await?;
    let bundled_plugin_marketplace_paths = runtime_contained_paths(
        runtime_root,
        metadata.bundled_plugins.unwrap_or_default(),
        &[],
    )?;
    let bundled_skill_paths = runtime_contained_paths(
        runtime_root,
        metadata.bundled_skills.unwrap_or_default(),
        &["SKILL.md"],
    )?;

    Ok(RuntimeInstallPaths {
        bundled_plugin_marketplace_paths,
        bundled_skill_paths,
        node_modules_path: absolute_path(node_modules_path)?,
        node_path: absolute_path(node_path)?,
        python_path: absolute_path(python_path)?,
        skills_to_remove: metadata.skills_to_remove.unwrap_or_default(),
    })
}

async fn find_python_path(
    runtime_root: &Path,
    bundle_format_version: u32,
    target_platform: &str,
) -> Result<PathBuf, JSONRPCErrorError> {
    let python_root = if bundle_format_version >= 2 {
        runtime_root.join("dependencies").join("python")
    } else {
        runtime_root.join("python")
    };
    let executable_name = python_executable_name(target_platform);
    let candidates = if target_platform == "win32" {
        vec![
            python_root.join(executable_name),
            python_root.join("python").join(executable_name),
            python_root.join("bin").join(executable_name),
        ]
    } else {
        vec![
            python_root.join("bin").join(executable_name),
            python_root.join("bin").join("python"),
        ]
    };
    for candidate in &candidates {
        match fs::metadata(candidate).await {
            Ok(metadata) if metadata.is_file() => return Ok(candidate.clone()),
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(internal_error(format!(
                    "failed to inspect runtime python executable {}: {err}",
                    candidate.display()
                )));
            }
        }
    }
    Err(invalid_params(format!(
        "runtime python executable is missing under {}",
        python_root.display()
    )))
}

fn runtime_contained_paths(
    runtime_root: &Path,
    directories: Vec<String>,
    suffix: &[&str],
) -> Result<Vec<AbsolutePathBuf>, JSONRPCErrorError> {
    directories
        .into_iter()
        .map(|directory| {
            let mut path = runtime_root.join(directory);
            for segment in suffix {
                path.push(segment);
            }
            let normalized_runtime_root = normalize_path(runtime_root);
            let normalized_path = normalize_path(&path);
            if normalized_path != normalized_runtime_root
                && normalized_path.starts_with(&normalized_runtime_root)
            {
                absolute_path(path)
            } else {
                Err(invalid_params(
                    "runtime-contained path must stay within the runtime root",
                ))
            }
        })
        .collect()
}

fn absolute_path(path: PathBuf) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    AbsolutePathBuf::from_absolute_path_checked(path)
        .map_err(|err| internal_error(format!("runtime path is not absolute: {err}")))
}

fn target_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}

fn node_executable_name(target_platform: &str) -> &'static str {
    if target_platform == "win32" {
        "node.exe"
    } else {
        "node"
    }
}

fn python_executable_name(target_platform: &str) -> &'static str {
    if target_platform == "win32" {
        "python.exe"
    } else {
        "python3"
    }
}

async fn path_exists(path: &Path) -> bool {
    fs::metadata(path).await.is_ok()
}

async fn require_runtime_file(path: &Path, label: &str) -> Result<(), JSONRPCErrorError> {
    match fs::metadata(path).await {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(invalid_params(format!(
            "runtime {label} is not a file: {}",
            path.display()
        ))),
        Err(err) if err.kind() == ErrorKind::NotFound => Err(invalid_params(format!(
            "runtime {label} is missing: {}",
            path.display()
        ))),
        Err(err) => Err(internal_error(format!(
            "failed to inspect runtime {label} {}: {err}",
            path.display()
        ))),
    }
}

async fn require_runtime_directory(path: &Path, label: &str) -> Result<(), JSONRPCErrorError> {
    match fs::metadata(path).await {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(invalid_params(format!(
            "runtime {label} is not a directory: {}",
            path.display()
        ))),
        Err(err) if err.kind() == ErrorKind::NotFound => Err(invalid_params(format!(
            "runtime {label} is missing: {}",
            path.display()
        ))),
        Err(err) => Err(internal_error(format!(
            "failed to inspect runtime {label} {}: {err}",
            path.display()
        ))),
    }
}

async fn remove_dir_if_exists(path: &Path) -> Result<(), JSONRPCErrorError> {
    match fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(internal_error(format!(
            "failed to remove runtime directory {}: {err}",
            path.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
    async fn install_from_archive_reuses_current_runtime() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let install_root = temp_dir.path().join("install");
        let runtime_root = install_root.join(PUBLISHED_ARTIFACT_NAME);
        create_runtime_root(&runtime_root, "v1").await;
        let archive_path = temp_dir.path().join("unused.tar.xz");
        fs::write(&archive_path, b"not used")
            .await
            .expect("write archive");
        let manifest = manifest_for_archive(&archive_path, "v1").await;

        let response = install_runtime_from_archive(&manifest, &archive_path, &install_root)
            .await
            .expect("install should succeed");

        assert_eq!(response.status, RuntimeInstallStatus::AlreadyCurrent);
        assert_eq!(response.bundle_version.as_deref(), Some("v1"));
        assert_eq!(
            response.paths.node_path,
            absolute_path(
                runtime_root
                    .join("dependencies")
                    .join("node")
                    .join("bin")
                    .join(node_executable_name(target_platform()))
            )
            .expect("absolute path")
        );
    }

    #[tokio::test]
    async fn install_runtime_reuses_current_runtime_without_downloading_archive() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let install_root = temp_dir.path().join("install");
        let runtime_root = install_root.join(PUBLISHED_ARTIFACT_NAME);
        create_runtime_root(&runtime_root, "v1").await;
        let archive_path = temp_dir.path().join("unused.tar.xz");
        fs::write(&archive_path, b"not used")
            .await
            .expect("write archive");
        let mut manifest = manifest_for_archive(&archive_path, "v1").await;
        manifest.archive_url = "not a valid archive URL".to_string();

        let response = install_runtime_with_root_and_control(
            RuntimeInstallParams {
                environment_id: None,
                manifest: Box::new(manifest),
                release: "primary".to_string(),
            },
            install_root,
            /*progress*/ None,
            CancellationToken::new(),
        )
        .await
        .expect("installed runtime should be reused without downloading");

        assert_eq!(response.status, RuntimeInstallStatus::AlreadyCurrent);
    }

    #[tokio::test]
    async fn install_from_archive_uses_runtime_metadata_bundle_format_when_manifest_omits_it() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let install_root = temp_dir.path().join("install");
        let runtime_root = install_root.join(PUBLISHED_ARTIFACT_NAME);
        create_runtime_root(&runtime_root, "v1").await;
        let archive_path = temp_dir.path().join("unused.tar.xz");
        fs::write(&archive_path, b"not used")
            .await
            .expect("write archive");
        let mut manifest = manifest_for_archive(&archive_path, "v1").await;
        manifest.bundle_format_version = None;

        let response = install_runtime_from_archive(&manifest, &archive_path, &install_root)
            .await
            .expect("install should succeed");

        assert_eq!(
            response.paths.node_modules_path,
            absolute_path(
                runtime_root
                    .join("dependencies")
                    .join("node")
                    .join("node_modules")
            )
            .expect("absolute path")
        );
        assert_eq!(
            response.paths.python_path,
            absolute_path(
                runtime_root
                    .join("dependencies")
                    .join("python")
                    .join("bin")
                    .join(python_executable_name(target_platform()))
            )
            .expect("absolute path")
        );
    }

    #[tokio::test]
    async fn validate_runtime_root_rejects_missing_node_executable() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let runtime_root = temp_dir.path().join(PUBLISHED_ARTIFACT_NAME);
        create_runtime_root(&runtime_root, "v1").await;
        fs::remove_file(
            runtime_root
                .join("dependencies")
                .join("node")
                .join("bin")
                .join(node_executable_name(target_platform())),
        )
        .await
        .expect("remove node");

        let error = validate_runtime_root(&runtime_root, Some(2), target_platform())
            .await
            .expect_err("node executable should be required");

        assert!(error.message.contains("node executable is missing"));
    }

    #[tokio::test]
    async fn validate_runtime_root_rejects_missing_node_modules_directory() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let runtime_root = temp_dir.path().join(PUBLISHED_ARTIFACT_NAME);
        create_runtime_root(&runtime_root, "v1").await;
        fs::remove_dir(
            runtime_root
                .join("dependencies")
                .join("node")
                .join("node_modules"),
        )
        .await
        .expect("remove node_modules");

        let error = validate_runtime_root(&runtime_root, Some(2), target_platform())
            .await
            .expect_err("node_modules directory should be required");

        assert!(error.message.contains("node modules directory is missing"));
    }

    #[tokio::test]
    async fn validate_runtime_root_rejects_missing_python_executable() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let runtime_root = temp_dir.path().join(PUBLISHED_ARTIFACT_NAME);
        create_runtime_root(&runtime_root, "v1").await;
        fs::remove_file(
            runtime_root
                .join("dependencies")
                .join("python")
                .join("bin")
                .join(python_executable_name(target_platform())),
        )
        .await
        .expect("remove python");

        let error = validate_runtime_root(&runtime_root, Some(2), target_platform())
            .await
            .expect_err("python executable should be required");

        assert!(error.message.contains("python executable is missing"));
    }

    #[tokio::test]
    async fn install_from_archive_rejects_checksum_mismatch() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let archive_path = temp_dir.path().join("archive.tar.xz");
        fs::write(&archive_path, b"archive")
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

        let error = install_runtime_from_archive(
            &manifest,
            &archive_path,
            &temp_dir.path().join("install"),
        )
        .await
        .expect_err("checksum mismatch should fail");

        assert!(error.message.contains("checksum mismatch"));
    }

    #[tokio::test]
    async fn install_from_archive_restores_previous_runtime_when_new_runtime_is_invalid() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let install_root = temp_dir.path().join("install");
        let runtime_root = install_root.join(PUBLISHED_ARTIFACT_NAME);
        create_runtime_root(&runtime_root, "old").await;

        let payload_root = temp_dir.path().join("payload").join("wrong-root");
        fs::create_dir_all(&payload_root)
            .await
            .expect("payload root");
        fs::write(
            payload_root.join("runtime.json"),
            r#"{"bundleFormatVersion":2,"bundleVersion":"new"}"#,
        )
        .await
        .expect("runtime metadata");
        let archive_path = temp_dir.path().join("invalid.tar.xz");
        create_tar_xz(temp_dir.path().join("payload").as_path(), &archive_path).await;
        let manifest = manifest_for_archive(&archive_path, "new").await;

        let error = install_runtime_from_archive(&manifest, &archive_path, &install_root)
            .await
            .expect_err("invalid runtime should fail");

        assert!(error.message.contains("runtime metadata is missing"));
        let metadata = read_installed_runtime_metadata(&runtime_root)
            .await
            .expect("read metadata")
            .expect("metadata");
        assert_eq!(metadata.bundle_version.as_deref(), Some("old"));
    }

    #[tokio::test]
    async fn install_from_archive_reports_install_progress() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let payload_root = temp_dir
            .path()
            .join("payload")
            .join(PUBLISHED_ARTIFACT_NAME);
        create_runtime_root(&payload_root, "v1").await;
        let archive_path = temp_dir.path().join("archive.tar.xz");
        create_tar_xz(temp_dir.path().join("payload").as_path(), &archive_path).await;
        let manifest = manifest_for_archive(&archive_path, "v1").await;
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
        let progress =
            RuntimeInstallProgressReporter::new(manifest.bundle_version.clone(), Some(progress_tx));

        install_runtime_from_archive_with_control(
            &manifest,
            &archive_path,
            &temp_dir.path().join("install"),
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
        let archive_path = temp_dir.path().join("unused.tar.xz");
        fs::write(&archive_path, b"unused")
            .await
            .expect("write archive");
        let manifest = manifest_for_archive(&archive_path, "v1").await;
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = install_runtime_from_archive_with_control(
            &manifest,
            &archive_path,
            &temp_dir.path().join("install"),
            &RuntimeInstallProgressReporter::new(manifest.bundle_version.clone(), None),
            &cancellation,
        )
        .await
        .expect_err("canceled install should fail");

        assert_eq!(error.message, "runtime install canceled");
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
            node_bin.join(node_executable_name(target_platform())),
            b"node",
        )
        .await
        .expect("node");
        fs::write(
            python_bin.join(python_executable_name(target_platform())),
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
            archive_sha256: compute_sha256(archive_path).await.expect("sha256"),
            archive_size_bytes: None,
            archive_url: "https://example.com/archive.tar.xz".to_string(),
            bundle_format_version: Some(2),
            bundle_version: Some(bundle_version.to_string()),
            format: Some("tar.xz".to_string()),
            runtime_root_directory_name: None,
        }
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
}
