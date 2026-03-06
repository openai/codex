use super::ArtifactRuntimeError;
use super::ArtifactRuntimePlatform;
use super::ExtractedRuntimeManifest;
use super::JsRuntime;
use super::codex_app_runtime_candidates;
use super::resolve_js_runtime_from_candidates;
use super::system_electron_runtime;
use super::system_node_runtime;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

/// Loads a previously installed runtime from a caller-provided cache root.
pub fn load_cached_runtime(
    cache_root: &Path,
    runtime_version: &str,
) -> Result<InstalledArtifactRuntime, ArtifactRuntimeError> {
    let platform = ArtifactRuntimePlatform::detect_current()?;
    let install_dir = cached_runtime_install_dir(cache_root, runtime_version, platform);
    if !install_dir.exists() {
        return Err(ArtifactRuntimeError::Io {
            context: format!(
                "artifact runtime {runtime_version} is not installed at {}",
                install_dir.display()
            ),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing artifact runtime"),
        });
    }

    InstalledArtifactRuntime::load(install_dir, platform)
}

/// A validated runtime installation extracted into the local package cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledArtifactRuntime {
    root_dir: PathBuf,
    runtime_version: String,
    platform: ArtifactRuntimePlatform,
    manifest: ExtractedRuntimeManifest,
    node_path: PathBuf,
    build_js_path: PathBuf,
    render_cli_path: PathBuf,
}

impl InstalledArtifactRuntime {
    /// Creates an installed-runtime value from prevalidated paths.
    pub fn new(
        root_dir: PathBuf,
        runtime_version: String,
        platform: ArtifactRuntimePlatform,
        manifest: ExtractedRuntimeManifest,
        node_path: PathBuf,
        build_js_path: PathBuf,
        render_cli_path: PathBuf,
    ) -> Self {
        Self {
            root_dir,
            runtime_version,
            platform,
            manifest,
            node_path,
            build_js_path,
            render_cli_path,
        }
    }

    /// Loads and validates an extracted runtime directory.
    pub fn load(
        root_dir: PathBuf,
        platform: ArtifactRuntimePlatform,
    ) -> Result<Self, ArtifactRuntimeError> {
        let manifest_path = root_dir.join("manifest.json");
        let manifest_bytes =
            std::fs::read(&manifest_path).map_err(|source| ArtifactRuntimeError::Io {
                context: format!("failed to read {}", manifest_path.display()),
                source,
            })?;
        let manifest = serde_json::from_slice::<ExtractedRuntimeManifest>(&manifest_bytes)
            .map_err(|source| ArtifactRuntimeError::InvalidManifest {
                path: manifest_path,
                source,
            })?;
        let node_path = resolve_relative_runtime_path(&root_dir, &manifest.node.relative_path)?;
        let build_js_path =
            resolve_relative_runtime_path(&root_dir, &manifest.entrypoints.build_js.relative_path)?;
        let render_cli_path = resolve_relative_runtime_path(
            &root_dir,
            &manifest.entrypoints.render_cli.relative_path,
        )?;
        verify_required_runtime_path(&build_js_path)?;
        verify_required_runtime_path(&render_cli_path)?;

        Ok(Self::new(
            root_dir,
            manifest.runtime_version.clone(),
            platform,
            manifest,
            node_path,
            build_js_path,
            render_cli_path,
        ))
    }

    /// Returns the extracted runtime root directory.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Returns the runtime version recorded in the extracted manifest.
    pub fn runtime_version(&self) -> &str {
        &self.runtime_version
    }

    /// Returns the platform this runtime was installed for.
    pub fn platform(&self) -> ArtifactRuntimePlatform {
        self.platform
    }

    /// Returns the parsed extracted-runtime manifest.
    pub fn manifest(&self) -> &ExtractedRuntimeManifest {
        &self.manifest
    }

    /// Returns the bundled Node executable path advertised by the runtime manifest.
    pub fn node_path(&self) -> &Path {
        &self.node_path
    }

    /// Returns the artifact build entrypoint path.
    pub fn build_js_path(&self) -> &Path {
        &self.build_js_path
    }

    /// Returns the artifact render CLI entrypoint path.
    pub fn render_cli_path(&self) -> &Path {
        &self.render_cli_path
    }

    /// Resolves the best executable to use for artifact commands.
    ///
    /// Preference order is the bundled Node path, then a machine Node install,
    /// then Electron from the machine or a Codex desktop app bundle.
    pub fn resolve_js_runtime(&self) -> Result<JsRuntime, ArtifactRuntimeError> {
        resolve_js_runtime_from_candidates(
            Some(self.node_path()),
            system_node_runtime(),
            system_electron_runtime(),
            codex_app_runtime_candidates(),
        )
        .ok_or_else(|| ArtifactRuntimeError::MissingJsRuntime {
            root_dir: self.root_dir.clone(),
        })
    }
}

pub(crate) fn cached_runtime_install_dir(
    cache_root: &Path,
    runtime_version: &str,
    platform: ArtifactRuntimePlatform,
) -> PathBuf {
    cache_root.join(runtime_version).join(platform.as_str())
}

pub(crate) fn default_cached_runtime_root(codex_home: &Path) -> PathBuf {
    codex_home.join(super::DEFAULT_CACHE_ROOT_RELATIVE)
}

fn resolve_relative_runtime_path(
    root_dir: &Path,
    relative_path: &str,
) -> Result<PathBuf, ArtifactRuntimeError> {
    let relative = Path::new(relative_path);
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(ArtifactRuntimeError::InvalidRuntimePath(
            relative_path.to_string(),
        ));
    }
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    }) {
        return Err(ArtifactRuntimeError::InvalidRuntimePath(
            relative_path.to_string(),
        ));
    }
    Ok(root_dir.join(relative))
}

fn verify_required_runtime_path(path: &Path) -> Result<(), ArtifactRuntimeError> {
    if path.is_file() {
        return Ok(());
    }

    Err(ArtifactRuntimeError::Io {
        context: format!("required runtime file is missing: {}", path.display()),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing runtime file"),
    })
}
