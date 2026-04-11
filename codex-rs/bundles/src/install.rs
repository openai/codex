use crate::archive::assert_archive_entries_stay_within_directory;
use crate::archive::download_file;
use crate::archive::extract_tar_archive;
use crate::archive::list_tar_entries;
use crate::archive::validate_sha256;
use crate::archive::verify_archive_checksum;
use crate::manifest::BundleSelection;
use crate::manifest::CodexRuntimesConfig;
use crate::manifest::DEFAULT_ARTIFACT_NAME;
use crate::manifest::DEFAULT_RUNTIME_ROOT_DIRECTORY_NAME;
use crate::manifest::resolve_archive_manifest;
use crate::platform::RuntimeTarget;
use crate::runtime::RuntimePaths;
use crate::runtime::validate_runtime_root;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::fs;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallState {
    pub current_version: String,
    pub runtime_root: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallStatus {
    AlreadyCurrent,
    Installed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallBundleResult {
    pub artifact_name: String,
    pub bundle_version: String,
    pub paths: RuntimePaths,
    pub runtime_root: PathBuf,
    pub status: InstallStatus,
}

#[derive(Clone, Debug)]
pub struct InstallBundleOptions {
    pub artifact_name: String,
    pub install_root: PathBuf,
    pub target: RuntimeTarget,
}

impl InstallBundleOptions {
    pub fn for_current_target() -> Result<Self> {
        Ok(Self {
            artifact_name: DEFAULT_ARTIFACT_NAME.to_string(),
            install_root: default_install_root()?,
            target: RuntimeTarget::current(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledBundle {
    pub artifact_name: String,
    pub bundle_version: String,
    pub current: bool,
    pub runtime_root: PathBuf,
    pub valid: bool,
}

pub fn default_install_root() -> Result<PathBuf> {
    let home = dirs_home().context("could not find home directory")?;
    Ok(home.join(".cache").join("codex-runtimes"))
}

pub async fn install_bundle(
    config: &CodexRuntimesConfig,
    selection: BundleSelection,
    options: InstallBundleOptions,
) -> Result<InstallBundleResult> {
    let runtime_options = config
        .runtimes
        .get(&options.artifact_name)
        .with_context(|| {
            format!(
                "runtime manifest does not contain artifact `{}`",
                options.artifact_name
            )
        })?;
    let manifest = resolve_archive_manifest(
        runtime_options,
        &selection,
        &options.artifact_name,
        &options.target,
    )
    .await?;
    let runtime_root_directory_name = manifest
        .runtime_root_directory_name
        .as_deref()
        .unwrap_or(DEFAULT_RUNTIME_ROOT_DIRECTORY_NAME);
    validate_path_segment(runtime_root_directory_name)?;
    validate_sha256(&manifest.archive_sha256)?;

    let bundle_version = manifest
        .bundle_version
        .clone()
        .or_else(|| match &selection {
            BundleSelection::Version(version) => Some(version.clone()),
            BundleSelection::Channel(_) => None,
        })
        .context("bundle manifest is missing bundleVersion")?;
    validate_path_segment(&bundle_version)?;

    let artifact_root = artifact_root(&options.install_root, &options.artifact_name);
    let runtime_root = version_runtime_root(
        &options.install_root,
        &options.artifact_name,
        &bundle_version,
        runtime_root_directory_name,
    );
    let bundle_format_version = manifest.bundle_format_version.unwrap_or(1);

    if let Ok(paths) =
        validate_runtime_root(&runtime_root, bundle_format_version, &options.target).await
    {
        let status = if read_install_state(&artifact_root)
            .await
            .is_ok_and(|state| state.current_version == bundle_version)
        {
            InstallStatus::AlreadyCurrent
        } else {
            write_install_state(
                &artifact_root,
                &InstallState {
                    current_version: bundle_version.clone(),
                    runtime_root: runtime_root.clone(),
                },
            )
            .await?;
            InstallStatus::Installed
        };
        return Ok(InstallBundleResult {
            artifact_name: options.artifact_name,
            bundle_version,
            paths,
            runtime_root,
            status,
        });
    }

    fs::create_dir_all(&artifact_root)
        .await
        .with_context(|| format!("failed to create {}", artifact_root.display()))?;
    let staging_dir = create_staging_dir(&artifact_root).await?;
    let previous_dir = artifact_root
        .join(".previous")
        .join(sanitize_path_segment(&bundle_version));

    let install_result =
        async {
            let archive_name = manifest
                .archive_name
                .as_deref()
                .unwrap_or("node-runtime.tar.xz");
            validate_path_segment(archive_name)?;
            let archive_path = staging_dir.join(archive_name);
            let extract_dir = staging_dir.join("payload");
            fs::create_dir_all(&extract_dir).await?;

            download_file(&manifest.archive_url, &archive_path).await?;
            verify_archive_checksum(
                &archive_path,
                &manifest.archive_sha256,
                &manifest.archive_url,
            )
            .await?;
            let entries = list_tar_entries(&archive_path).await?;
            assert_archive_entries_stay_within_directory(&entries)?;
            extract_tar_archive(&archive_path, &extract_dir).await?;

            let extracted_runtime_root = extract_dir.join(runtime_root_directory_name);
            validate_runtime_root(
                &extracted_runtime_root,
                bundle_format_version,
                &options.target,
            )
            .await?;

            fs::create_dir_all(runtime_root.parent().ok_or_else(|| {
                anyhow!("runtime root has no parent: {}", runtime_root.display())
            })?)
            .await?;
            let _ = fs::remove_dir_all(&previous_dir).await;
            if path_exists(&runtime_root).await {
                if let Some(parent) = previous_dir.parent() {
                    fs::create_dir_all(parent).await?;
                }
                fs::rename(&runtime_root, &previous_dir).await?;
            }

            match fs::rename(&extracted_runtime_root, &runtime_root).await {
                Ok(()) => {}
                Err(err) => {
                    let _ = fs::remove_dir_all(&runtime_root).await;
                    if path_exists(&previous_dir).await {
                        let _ = fs::rename(&previous_dir, &runtime_root).await;
                    }
                    return Err(err).with_context(|| {
                        format!("failed to install runtime at {}", runtime_root.display())
                    });
                }
            }

            let paths =
                match validate_runtime_root(&runtime_root, bundle_format_version, &options.target)
                    .await
                {
                    Ok(paths) => paths,
                    Err(err) => {
                        let _ = fs::remove_dir_all(&runtime_root).await;
                        if path_exists(&previous_dir).await {
                            let _ = fs::rename(&previous_dir, &runtime_root).await;
                        }
                        return Err(err);
                    }
                };
            let _ = fs::remove_dir_all(&previous_dir).await;
            write_install_state(
                &artifact_root,
                &InstallState {
                    current_version: bundle_version.clone(),
                    runtime_root: runtime_root.clone(),
                },
            )
            .await?;

            Ok(paths)
        }
        .await;

    let _ = fs::remove_dir_all(&staging_dir).await;

    let paths = install_result?;
    Ok(InstallBundleResult {
        artifact_name: options.artifact_name,
        bundle_version,
        paths,
        runtime_root,
        status: InstallStatus::Installed,
    })
}

pub async fn list_installed_bundles(install_root: &Path) -> Result<Vec<InstalledBundle>> {
    let mut bundles = Vec::new();
    let mut artifacts = match fs::read_dir(install_root).await {
        Ok(artifacts) => artifacts,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(bundles),
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "failed to list runtime install root {}",
                    install_root.display()
                )
            });
        }
    };

    while let Some(artifact_entry) = artifacts.next_entry().await? {
        let artifact_path = artifact_entry.path();
        if !artifact_entry.file_type().await?.is_dir() {
            continue;
        }
        let artifact_name = artifact_entry.file_name().to_string_lossy().to_string();
        let state = read_install_state(&artifact_path).await.ok();
        let versions_dir = artifact_path.join("versions");
        let mut versions = match fs::read_dir(&versions_dir).await {
            Ok(versions) => versions,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to list runtime versions in {}",
                        versions_dir.display()
                    )
                });
            }
        };

        while let Some(version_entry) = versions.next_entry().await? {
            let version_path = version_entry.path();
            if !version_entry.file_type().await?.is_dir() {
                continue;
            }
            let bundle_version = version_entry.file_name().to_string_lossy().to_string();
            let runtime_root = find_runtime_root_in_version_dir(&version_path).await?;
            let valid = match &runtime_root {
                Some(runtime_root) => runtime_root.join("runtime.json").is_file(),
                None => false,
            };
            bundles.push(InstalledBundle {
                artifact_name: artifact_name.clone(),
                current: state
                    .as_ref()
                    .is_some_and(|state| state.current_version == bundle_version),
                bundle_version,
                runtime_root: runtime_root.unwrap_or(version_path),
                valid,
            });
        }
    }
    bundles.sort_by(|left, right| {
        left.artifact_name
            .cmp(&right.artifact_name)
            .then_with(|| left.bundle_version.cmp(&right.bundle_version))
    });
    Ok(bundles)
}

pub fn format_status(status: &InstallStatus) -> &'static str {
    match status {
        InstallStatus::AlreadyCurrent => "already-current",
        InstallStatus::Installed => "installed",
    }
}

fn artifact_root(install_root: &Path, artifact_name: &str) -> PathBuf {
    install_root.join(artifact_name)
}

fn version_runtime_root(
    install_root: &Path,
    artifact_name: &str,
    bundle_version: &str,
    runtime_root_directory_name: &str,
) -> PathBuf {
    artifact_root(install_root, artifact_name)
        .join("versions")
        .join(bundle_version)
        .join(runtime_root_directory_name)
}

async fn read_install_state(artifact_root: &Path) -> Result<InstallState> {
    let path = artifact_root.join("install-state.json");
    let raw = fs::read_to_string(&path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))
}

async fn write_install_state(artifact_root: &Path, state: &InstallState) -> Result<()> {
    fs::create_dir_all(artifact_root)
        .await
        .with_context(|| format!("failed to create {}", artifact_root.display()))?;
    let path = artifact_root.join("install-state.json");
    let raw = serde_json::to_string_pretty(state)?;
    fs::write(&path, raw)
        .await
        .with_context(|| format!("failed to write {}", path.display()))
}

async fn create_staging_dir(artifact_root: &Path) -> Result<PathBuf> {
    let staging_root = artifact_root.join(".staging");
    fs::create_dir_all(&staging_root).await?;
    for attempt in 0..100_u32 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_nanos();
        let path = staging_root.join(format!(
            "codex-runtime-install-{}-{stamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path).await {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => {
                return Err(err).with_context(|| format!("failed to create {}", path.display()));
            }
        }
    }
    bail!("failed to allocate runtime staging directory")
}

async fn path_exists(path: &Path) -> bool {
    fs::metadata(path).await.is_ok()
}

async fn find_runtime_root_in_version_dir(version_dir: &Path) -> Result<Option<PathBuf>> {
    let mut entries = fs::read_dir(version_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            return Ok(Some(entry.path()));
        }
    }
    Ok(None)
}

fn validate_path_segment(value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
    {
        bail!("expected a single path segment, got `{value}`");
    }
    Ok(())
}

fn sanitize_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::compute_sha256;
    use crate::manifest::DEFAULT_ARTIFACT_NAME;
    use crate::manifest::DEFAULT_RUNTIME_ROOT_DIRECTORY_NAME;
    use crate::manifest::InstallRuntimeOptions;
    use crate::manifest::RuntimePlatformManifest;
    use crate::manifest::RuntimeProvider;
    use crate::manifest::RuntimeReleaseManifest;
    use pretty_assertions::assert_eq;
    #[cfg(unix)]
    use std::collections::HashMap;
    #[cfg(unix)]
    use std::process::Command as StdCommand;
    #[cfg(unix)]
    use tempfile::TempDir;
    #[cfg(unix)]
    use wiremock::Mock;
    #[cfg(unix)]
    use wiremock::MockServer;
    #[cfg(unix)]
    use wiremock::ResponseTemplate;
    #[cfg(unix)]
    use wiremock::matchers::method;
    #[cfg(unix)]
    use wiremock::matchers::path;

    fn test_target() -> RuntimeTarget {
        RuntimeTarget {
            arch: "arm64".to_string(),
            platform: "darwin".to_string(),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn installs_versioned_runtime_and_lists_it() {
        let temp = TempDir::new().expect("tempdir");
        let archive_path = create_test_runtime_archive(temp.path(), "2026.03.26.1");
        let archive_bytes = fs::read(&archive_path).await.expect("read archive");
        let archive_sha256 = compute_sha256(&archive_path).await.expect("archive sha");
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/runtime.tar.xz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(archive_bytes))
            .mount(&server)
            .await;

        let mut versions = HashMap::new();
        versions.insert(
            "2026.03.26.1".to_string(),
            RuntimeReleaseManifest {
                bundle_format_version: Some(2),
                bundle_version: Some("2026.03.26.1".to_string()),
                platforms: HashMap::from([(
                    "macos-aarch64".to_string(),
                    RuntimePlatformManifest {
                        digest: archive_sha256,
                        format: Some("tar.xz".to_string()),
                        hash: "sha256".to_string(),
                        path: Some("runtime.tar.xz".to_string()),
                        providers: vec![RuntimeProvider {
                            url: format!("{}/runtime.tar.xz", server.uri()),
                        }],
                        size: None,
                    },
                )]),
                runtime_root_directory_name: Some(DEFAULT_RUNTIME_ROOT_DIRECTORY_NAME.to_string()),
            },
        );
        let config = CodexRuntimesConfig {
            runtimes: HashMap::from([(
                DEFAULT_ARTIFACT_NAME.to_string(),
                InstallRuntimeOptions {
                    base_url: server.uri(),
                    latest: None,
                    latest_alpha: None,
                    versions,
                },
            )]),
        };
        let install_root = temp.path().join("runtimes");

        let result = install_bundle(
            &config,
            BundleSelection::Version("2026.03.26.1".to_string()),
            InstallBundleOptions {
                artifact_name: DEFAULT_ARTIFACT_NAME.to_string(),
                install_root: install_root.clone(),
                target: test_target(),
            },
        )
        .await
        .expect("install bundle");

        assert_eq!(result.status, InstallStatus::Installed);
        assert_eq!(result.bundle_version, "2026.03.26.1");
        assert_eq!(
            result.runtime_root,
            install_root
                .join(DEFAULT_ARTIFACT_NAME)
                .join("versions")
                .join("2026.03.26.1")
                .join(DEFAULT_RUNTIME_ROOT_DIRECTORY_NAME)
        );

        let installed = list_installed_bundles(&install_root)
            .await
            .expect("list installed bundles");
        assert_eq!(
            installed,
            vec![InstalledBundle {
                artifact_name: DEFAULT_ARTIFACT_NAME.to_string(),
                bundle_version: "2026.03.26.1".to_string(),
                current: true,
                runtime_root: result.runtime_root,
                valid: true,
            }]
        );
    }

    #[cfg(unix)]
    fn create_test_runtime_archive(root: &Path, bundle_version: &str) -> PathBuf {
        let runtime_root = root
            .join("payload")
            .join(DEFAULT_RUNTIME_ROOT_DIRECTORY_NAME);
        std::fs::create_dir_all(runtime_root.join("dependencies/bin")).expect("create bin");
        std::fs::create_dir_all(runtime_root.join("dependencies/node/bin")).expect("create node");
        std::fs::create_dir_all(runtime_root.join("dependencies/node/node_modules"))
            .expect("create node_modules");
        std::fs::create_dir_all(runtime_root.join("dependencies/python/bin"))
            .expect("create python");
        std::fs::write(
            runtime_root.join("runtime.json"),
            format!(r#"{{"bundleVersion":"{bundle_version}"}}"#),
        )
        .expect("write runtime metadata");
        write_executable(
            runtime_root.join("dependencies/bin/node_repl"),
            "#!/bin/sh\nexit 0\n",
        );
        write_executable(
            runtime_root.join("dependencies/node/bin/node"),
            "#!/bin/sh\nexit 0\n",
        );
        write_executable(
            runtime_root.join("dependencies/python/bin/python3"),
            "#!/bin/sh\necho 1.0.0\n",
        );

        let archive_path = root.join("runtime.tar.xz");
        let status = StdCommand::new("tar")
            .arg("-cJf")
            .arg(&archive_path)
            .arg(DEFAULT_RUNTIME_ROOT_DIRECTORY_NAME)
            .current_dir(root.join("payload"))
            .status()
            .expect("run tar");
        assert!(status.success(), "tar should create test archive");
        archive_path
    }

    #[cfg(unix)]
    fn write_executable(path: PathBuf, contents: &str) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(&path, contents).expect("write executable");
        let mut permissions = std::fs::metadata(&path)
            .expect("executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("chmod executable");
    }
}
