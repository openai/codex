use super::ArtifactRuntimeError;
use super::ArtifactRuntimePlatform;
use super::InstalledArtifactRuntime;
use super::ReleaseManifest;
use super::detect_runtime_root;
use codex_package_manager::ManagedPackage;
use codex_package_manager::PackageManager;
use codex_package_manager::PackageManagerConfig;
use codex_package_manager::PackageManagerError;
use codex_package_manager::PackageReleaseArchive;
use reqwest::Client;
use std::path::Path;
use std::path::PathBuf;
use url::Url;

/// Release tag prefix used for artifact runtime assets.
pub const DEFAULT_RELEASE_TAG_PREFIX: &str = "artifact-runtime-v";

/// Relative cache root for installed artifact runtimes under `codex_home`.
pub const DEFAULT_CACHE_ROOT_RELATIVE: &str = "packages/artifacts";

/// Base URL used by default when downloading runtime assets from GitHub releases.
pub const DEFAULT_RELEASE_BASE_URL: &str = "https://github.com/openai/codex/releases/download/";

/// Describes where a particular artifact runtime release can be downloaded from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactRuntimeReleaseLocator {
    base_url: Url,
    runtime_version: String,
    release_tag_prefix: String,
}

impl ArtifactRuntimeReleaseLocator {
    /// Creates a locator for a runtime version under a release base URL.
    pub fn new(base_url: Url, runtime_version: impl Into<String>) -> Self {
        Self {
            base_url,
            runtime_version: runtime_version.into(),
            release_tag_prefix: DEFAULT_RELEASE_TAG_PREFIX.to_string(),
        }
    }

    /// Overrides the release-tag prefix used when constructing asset names.
    pub fn with_tag_prefix(mut self, release_tag_prefix: impl Into<String>) -> Self {
        self.release_tag_prefix = release_tag_prefix.into();
        self
    }

    /// Returns the release asset base URL.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Returns the expected runtime version.
    pub fn runtime_version(&self) -> &str {
        &self.runtime_version
    }

    /// Returns the full release tag for the runtime version.
    pub fn release_tag(&self) -> String {
        format!("{}{}", self.release_tag_prefix, self.runtime_version)
    }

    /// Returns the expected manifest filename for the release.
    pub fn manifest_file_name(&self) -> String {
        format!("{}-manifest.json", self.release_tag())
    }

    /// Returns the manifest URL for this runtime release.
    pub fn manifest_url(&self) -> Result<Url, PackageManagerError> {
        self.base_url
            .join(&format!(
                "{}/{}",
                self.release_tag(),
                self.manifest_file_name()
            ))
            .map_err(PackageManagerError::InvalidBaseUrl)
    }

    /// Returns the default GitHub-release locator for a runtime version.
    pub fn default(runtime_version: impl Into<String>) -> Self {
        Self::new(
            Url::parse(DEFAULT_RELEASE_BASE_URL).unwrap_or_else(|error| {
                panic!("hard-coded artifact runtime release base URL must be valid: {error}")
            }),
            runtime_version,
        )
    }
}

/// Configuration for resolving artifact runtimes under a Codex home directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactRuntimeManagerConfig {
    package_manager: PackageManagerConfig<ArtifactRuntimePackage>,
    release: ArtifactRuntimeReleaseLocator,
}

impl ArtifactRuntimeManagerConfig {
    /// Creates a runtime-manager config from a Codex home and explicit release locator.
    pub fn new(codex_home: PathBuf, release: ArtifactRuntimeReleaseLocator) -> Self {
        Self {
            package_manager: PackageManagerConfig::new(
                codex_home,
                ArtifactRuntimePackage::new(release.clone()),
            ),
            release,
        }
    }

    /// Creates a runtime-manager config that downloads from the default GitHub release location.
    pub fn with_default_release(codex_home: PathBuf, runtime_version: impl Into<String>) -> Self {
        Self::new(
            codex_home,
            ArtifactRuntimeReleaseLocator::default(runtime_version),
        )
    }

    /// Overrides the runtime cache root.
    pub fn with_cache_root(mut self, cache_root: PathBuf) -> Self {
        self.package_manager = self.package_manager.with_cache_root(cache_root);
        self
    }

    /// Returns the runtime cache root.
    pub fn cache_root(&self) -> PathBuf {
        self.package_manager.cache_root()
    }

    /// Returns the release locator used by this config.
    pub fn release(&self) -> &ArtifactRuntimeReleaseLocator {
        &self.release
    }
}

/// Package-manager-backed artifact runtime resolver and installer.
#[derive(Clone, Debug)]
pub struct ArtifactRuntimeManager {
    package_manager: PackageManager<ArtifactRuntimePackage>,
    config: ArtifactRuntimeManagerConfig,
}

impl ArtifactRuntimeManager {
    /// Creates a runtime manager using the default `reqwest` client.
    pub fn new(config: ArtifactRuntimeManagerConfig) -> Self {
        let package_manager = PackageManager::new(config.package_manager.clone());
        Self {
            package_manager,
            config,
        }
    }

    /// Creates a runtime manager with a caller-provided HTTP client.
    pub fn with_client(config: ArtifactRuntimeManagerConfig, client: Client) -> Self {
        let package_manager = PackageManager::with_client(config.package_manager.clone(), client);
        Self {
            package_manager,
            config,
        }
    }

    /// Returns the manager configuration.
    pub fn config(&self) -> &ArtifactRuntimeManagerConfig {
        &self.config
    }

    /// Returns the installed runtime if it is already present and valid.
    pub async fn resolve_cached(
        &self,
    ) -> Result<Option<InstalledArtifactRuntime>, ArtifactRuntimeError> {
        self.package_manager.resolve_cached().await
    }

    /// Returns the installed runtime, downloading and caching it if necessary.
    pub async fn ensure_installed(&self) -> Result<InstalledArtifactRuntime, ArtifactRuntimeError> {
        self.package_manager.ensure_installed().await
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArtifactRuntimePackage {
    release: ArtifactRuntimeReleaseLocator,
}

impl ArtifactRuntimePackage {
    fn new(release: ArtifactRuntimeReleaseLocator) -> Self {
        Self { release }
    }
}

impl ManagedPackage for ArtifactRuntimePackage {
    type Error = ArtifactRuntimeError;
    type Installed = InstalledArtifactRuntime;
    type ReleaseManifest = ReleaseManifest;

    fn default_cache_root_relative(&self) -> &str {
        DEFAULT_CACHE_ROOT_RELATIVE
    }

    fn version(&self) -> &str {
        self.release.runtime_version()
    }

    fn manifest_url(&self) -> Result<Url, PackageManagerError> {
        self.release.manifest_url()
    }

    fn archive_url(&self, archive: &PackageReleaseArchive) -> Result<Url, PackageManagerError> {
        self.release
            .base_url()
            .join(&format!(
                "{}/{}",
                self.release.release_tag(),
                archive.archive
            ))
            .map_err(PackageManagerError::InvalidBaseUrl)
    }

    fn release_version<'a>(&self, manifest: &'a Self::ReleaseManifest) -> &'a str {
        &manifest.runtime_version
    }

    fn platform_archive(
        &self,
        manifest: &Self::ReleaseManifest,
        platform: ArtifactRuntimePlatform,
    ) -> Result<PackageReleaseArchive, Self::Error> {
        manifest
            .platforms
            .get(platform.as_str())
            .cloned()
            .ok_or_else(|| {
                PackageManagerError::MissingPlatform(platform.as_str().to_string()).into()
            })
    }

    fn install_dir(&self, cache_root: &Path, platform: ArtifactRuntimePlatform) -> PathBuf {
        cache_root.join(self.version()).join(platform.as_str())
    }

    fn installed_version<'a>(&self, package: &'a Self::Installed) -> &'a str {
        package.runtime_version()
    }

    fn load_installed(
        &self,
        root_dir: PathBuf,
        platform: ArtifactRuntimePlatform,
    ) -> Result<Self::Installed, Self::Error> {
        InstalledArtifactRuntime::load(root_dir, platform)
    }

    fn detect_extracted_root(&self, extraction_root: &Path) -> Result<PathBuf, Self::Error> {
        detect_runtime_root(extraction_root)
    }
}
