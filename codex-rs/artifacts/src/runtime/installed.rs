use super::ArtifactRuntimeError;
use super::ArtifactRuntimePlatform;
use super::JsRuntime;
use super::codex_app_runtime_candidates;
use super::resolve_js_runtime_from_candidates;
use super::system_electron_runtime;
use super::system_node_runtime;
use std::collections::BTreeMap;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

const ARTIFACT_TOOL_PACKAGE_NAME: &str = "@oai/artifact-tool";

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
    build_js_path: PathBuf,
}

impl InstalledArtifactRuntime {
    /// Creates an installed-runtime value from prevalidated paths.
    pub fn new(
        root_dir: PathBuf,
        runtime_version: String,
        platform: ArtifactRuntimePlatform,
        build_js_path: PathBuf,
    ) -> Self {
        Self {
            root_dir,
            runtime_version,
            platform,
            build_js_path,
        }
    }

    /// Loads and validates an extracted runtime directory.
    pub fn load(
        root_dir: PathBuf,
        platform: ArtifactRuntimePlatform,
    ) -> Result<Self, ArtifactRuntimeError> {
        let package_metadata = load_package_metadata(&root_dir)?;
        let build_js_path =
            resolve_relative_runtime_path(&root_dir, &package_metadata.build_js_relative_path)?;
        verify_required_runtime_path(&build_js_path)?;

        Ok(Self::new(
            root_dir,
            package_metadata.version,
            platform,
            build_js_path,
        ))
    }

    /// Returns the extracted runtime root directory.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Returns the runtime version recorded in `package.json`.
    pub fn runtime_version(&self) -> &str {
        &self.runtime_version
    }

    /// Returns the platform this runtime was installed for.
    pub fn platform(&self) -> ArtifactRuntimePlatform {
        self.platform
    }

    /// Returns the artifact build entrypoint path.
    pub fn build_js_path(&self) -> &Path {
        &self.build_js_path
    }

    /// Resolves the best executable to use for artifact commands.
    ///
    /// Preference order is a machine Node install, then Electron from the
    /// machine or a Codex desktop app bundle.
    pub fn resolve_js_runtime(&self) -> Result<JsRuntime, ArtifactRuntimeError> {
        resolve_js_runtime_from_candidates(
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

pub(crate) fn detect_runtime_root(extraction_root: &Path) -> Result<PathBuf, ArtifactRuntimeError> {
    if is_runtime_root(extraction_root) {
        return Ok(extraction_root.to_path_buf());
    }

    let mut directory_candidates = Vec::new();
    for entry in std::fs::read_dir(extraction_root).map_err(|source| ArtifactRuntimeError::Io {
        context: format!("failed to read {}", extraction_root.display()),
        source,
    })? {
        let entry = entry.map_err(|source| ArtifactRuntimeError::Io {
            context: format!("failed to read entry in {}", extraction_root.display()),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            directory_candidates.push(path);
        }
    }

    if directory_candidates.len() == 1 {
        let candidate = &directory_candidates[0];
        if is_runtime_root(candidate) {
            return Ok(candidate.clone());
        }
    }

    Err(ArtifactRuntimeError::Io {
        context: format!(
            "failed to detect artifact runtime root under {}",
            extraction_root.display()
        ),
        source: std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing artifact runtime root",
        ),
    })
}

fn is_runtime_root(root_dir: &Path) -> bool {
    let Ok(package_metadata) = load_package_metadata(root_dir) else {
        return false;
    };
    let Ok(build_js_path) =
        resolve_relative_runtime_path(root_dir, &package_metadata.build_js_relative_path)
    else {
        return false;
    };

    build_js_path.is_file()
}

struct PackageMetadata {
    version: String,
    build_js_relative_path: String,
}

fn load_package_metadata(root_dir: &Path) -> Result<PackageMetadata, ArtifactRuntimeError> {
    #[derive(serde::Deserialize)]
    struct PackageJson {
        name: String,
        version: String,
        exports: PackageExports,
    }

    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum PackageExports {
        Main(String),
        Map(BTreeMap<String, String>),
    }

    impl PackageExports {
        fn build_entrypoint(&self) -> Option<&str> {
            match self {
                Self::Main(path) => Some(path),
                Self::Map(exports) => exports.get(".").map(String::as_str),
            }
        }
    }

    let package_json_path = root_dir.join("package.json");
    let package_json_bytes =
        std::fs::read(&package_json_path).map_err(|source| ArtifactRuntimeError::Io {
            context: format!("failed to read {}", package_json_path.display()),
            source,
        })?;
    let package_json =
        serde_json::from_slice::<PackageJson>(&package_json_bytes).map_err(|source| {
            ArtifactRuntimeError::InvalidPackageMetadata {
                path: package_json_path.clone(),
                source,
            }
        })?;

    if package_json.name != ARTIFACT_TOOL_PACKAGE_NAME {
        return Err(ArtifactRuntimeError::Io {
            context: format!(
                "unsupported artifact runtime package at {}; expected name `{ARTIFACT_TOOL_PACKAGE_NAME}`, got `{}`",
                package_json_path.display(),
                package_json.name
            ),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported package name",
            ),
        });
    }

    let Some(build_js_relative_path) = package_json.exports.build_entrypoint() else {
        return Err(ArtifactRuntimeError::Io {
            context: format!(
                "unsupported artifact runtime package at {}; expected `exports[\".\"]` to point at the JS entrypoint",
                package_json_path.display()
            ),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, "missing package export"),
        });
    };

    Ok(PackageMetadata {
        version: package_json.version,
        build_js_relative_path: build_js_relative_path.trim_start_matches("./").to_string(),
    })
}
