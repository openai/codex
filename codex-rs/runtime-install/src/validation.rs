use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RuntimeInstallManifestParams;
use codex_app_server_protocol::RuntimeInstallPaths;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;

use crate::errors::internal_error;
use crate::errors::invalid_params;
use crate::executor::RuntimeArchiveFormat;
use crate::executor::RuntimeExecutor;
use crate::executor::TargetPlatform;
use crate::executor::path_uri;

pub(crate) const PUBLISHED_ARTIFACT_NAME: &str = "codex-primary-runtime";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstalledRuntimeMetadata {
    pub(crate) bundle_format_version: Option<u32>,
    pub(crate) bundle_version: Option<String>,
    bundled_plugins: Option<Vec<String>>,
    bundled_skills: Option<Vec<String>>,
    skills_to_remove: Option<Vec<String>>,
}

pub(crate) fn validate_manifest(
    manifest: &RuntimeInstallManifestParams,
) -> Result<(), JSONRPCErrorError> {
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

pub(crate) fn runtime_root_directory_name(
    manifest: &RuntimeInstallManifestParams,
) -> Result<String, JSONRPCErrorError> {
    let runtime_root_directory_name = manifest
        .runtime_root_directory_name
        .clone()
        .unwrap_or_else(|| PUBLISHED_ARTIFACT_NAME.to_string());
    validate_path_segment(&runtime_root_directory_name, "runtimeRootDirectoryName")?;
    Ok(runtime_root_directory_name)
}

pub(crate) fn runtime_archive_format(
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

pub(crate) fn default_archive_name(format: RuntimeArchiveFormat) -> &'static str {
    match format {
        RuntimeArchiveFormat::TarXz => "node-runtime.tar.xz",
        RuntimeArchiveFormat::Zip => "node-runtime.zip",
    }
}

pub(crate) fn assert_archive_entries_stay_within_directory(
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
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

pub(crate) async fn read_installed_runtime_metadata(
    executor: &RuntimeExecutor,
    runtime_root: &AbsolutePathBuf,
) -> Result<Option<InstalledRuntimeMetadata>, JSONRPCErrorError> {
    let metadata_path = absolute_path(runtime_root.as_path().join("runtime.json"))?;
    let raw = match executor
        .filesystem()
        .read_file_text(&path_uri(&metadata_path), /*sandbox*/ None)
        .await
    {
        Ok(raw) => raw,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
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

pub(crate) async fn validate_runtime_root(
    executor: &RuntimeExecutor,
    runtime_root: &AbsolutePathBuf,
    manifest_bundle_format_version: Option<u32>,
    platform: TargetPlatform,
) -> Result<RuntimeInstallPaths, JSONRPCErrorError> {
    let metadata = read_installed_runtime_metadata(executor, runtime_root)
        .await?
        .ok_or_else(|| invalid_params("runtime metadata is missing"))?;
    let bundle_format_version = manifest_bundle_format_version
        .or(metadata.bundle_format_version)
        .unwrap_or(1);
    let node_root = if bundle_format_version >= 2 {
        runtime_root.as_path().join("dependencies").join("node")
    } else {
        runtime_root.as_path().to_path_buf()
    };
    let node_path = absolute_path(
        node_root
            .join("bin")
            .join(node_executable_name(platform.runtime_name())),
    )?;
    let node_modules_path = absolute_path(node_root.join("node_modules"))?;
    require_runtime_file(executor, &node_path, "node executable").await?;
    require_runtime_directory(executor, &node_modules_path, "node modules directory").await?;
    let python_path =
        find_python_path(executor, runtime_root, bundle_format_version, platform).await?;
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
        node_modules_path,
        node_path,
        python_path,
        skills_to_remove: metadata.skills_to_remove.unwrap_or_default(),
    })
}

async fn find_python_path(
    executor: &RuntimeExecutor,
    runtime_root: &AbsolutePathBuf,
    bundle_format_version: u32,
    platform: TargetPlatform,
) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    let python_root = if bundle_format_version >= 2 {
        runtime_root.as_path().join("dependencies").join("python")
    } else {
        runtime_root.as_path().join("python")
    };
    let executable_name = python_executable_name(platform.runtime_name());
    let candidates = if platform == TargetPlatform::Windows {
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
    for candidate in candidates {
        let candidate = absolute_path(candidate)?;
        match executor
            .filesystem()
            .get_metadata(&path_uri(&candidate), /*sandbox*/ None)
            .await
        {
            Ok(metadata) if metadata.is_file => return Ok(candidate),
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
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
    runtime_root: &AbsolutePathBuf,
    directories: Vec<String>,
    suffix: &[&str],
) -> Result<Vec<AbsolutePathBuf>, JSONRPCErrorError> {
    directories
        .into_iter()
        .map(|directory| {
            let mut path = runtime_root.as_path().join(directory);
            for segment in suffix {
                path.push(segment);
            }
            let normalized_runtime_root = normalize_path(runtime_root.as_path());
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

pub(crate) fn absolute_path(path: PathBuf) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    AbsolutePathBuf::from_absolute_path_checked(path)
        .map_err(|err| internal_error(format!("runtime path is not absolute: {err}")))
}

pub(crate) fn node_executable_name(target_platform: &str) -> &'static str {
    if target_platform == "win32" {
        "node.exe"
    } else {
        "node"
    }
}

pub(crate) fn python_executable_name(target_platform: &str) -> &'static str {
    if target_platform == "win32" {
        "python.exe"
    } else {
        "python3"
    }
}

async fn require_runtime_file(
    executor: &RuntimeExecutor,
    path: &AbsolutePathBuf,
    label: &str,
) -> Result<(), JSONRPCErrorError> {
    match executor
        .filesystem()
        .get_metadata(&path_uri(path), /*sandbox*/ None)
        .await
    {
        Ok(metadata) if metadata.is_file => Ok(()),
        Ok(_) => Err(invalid_params(format!(
            "runtime {label} is not a file: {}",
            path.display()
        ))),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(invalid_params(format!(
            "runtime {label} is missing: {}",
            path.display()
        ))),
        Err(err) => Err(internal_error(format!(
            "failed to inspect runtime {label} {}: {err}",
            path.display()
        ))),
    }
}

async fn require_runtime_directory(
    executor: &RuntimeExecutor,
    path: &AbsolutePathBuf,
    label: &str,
) -> Result<(), JSONRPCErrorError> {
    match executor
        .filesystem()
        .get_metadata(&path_uri(path), /*sandbox*/ None)
        .await
    {
        Ok(metadata) if metadata.is_directory => Ok(()),
        Ok(_) => Err(invalid_params(format!(
            "runtime {label} is not a directory: {}",
            path.display()
        ))),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(invalid_params(format!(
            "runtime {label} is missing: {}",
            path.display()
        ))),
        Err(err) => Err(internal_error(format!(
            "failed to inspect runtime {label} {}: {err}",
            path.display()
        ))),
    }
}
