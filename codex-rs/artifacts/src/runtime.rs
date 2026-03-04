use codex_package_manager::ManagedPackage;
use codex_package_manager::PackageManager;
use codex_package_manager::PackageManagerConfig;
use codex_package_manager::PackageManagerError;
pub use codex_package_manager::PackagePlatform as ArtifactRuntimePlatform;
use codex_package_manager::PackageReleaseArchive;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;
use url::Url;

pub const DEFAULT_RELEASE_TAG_PREFIX: &str = "artifact-runtime-v";
pub const DEFAULT_CACHE_ROOT_RELATIVE: &str = "packages/artifacts";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactRuntimeReleaseLocator {
    base_url: Url,
    runtime_version: String,
    release_tag_prefix: String,
}

impl ArtifactRuntimeReleaseLocator {
    pub fn new(base_url: Url, runtime_version: impl Into<String>) -> Self {
        Self {
            base_url,
            runtime_version: runtime_version.into(),
            release_tag_prefix: DEFAULT_RELEASE_TAG_PREFIX.to_string(),
        }
    }

    pub fn with_tag_prefix(mut self, release_tag_prefix: impl Into<String>) -> Self {
        self.release_tag_prefix = release_tag_prefix.into();
        self
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub fn runtime_version(&self) -> &str {
        &self.runtime_version
    }

    pub fn release_tag(&self) -> String {
        format!("{}{}", self.release_tag_prefix, self.runtime_version)
    }

    pub fn manifest_file_name(&self) -> String {
        format!("{}-manifest.json", self.release_tag())
    }

    pub fn manifest_url(&self) -> Result<Url, PackageManagerError> {
        self.base_url
            .join(&self.manifest_file_name())
            .map_err(PackageManagerError::InvalidBaseUrl)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactRuntimeManagerConfig {
    package_manager: PackageManagerConfig<ArtifactRuntimePackage>,
}

impl ArtifactRuntimeManagerConfig {
    pub fn new(codex_home: PathBuf, release: ArtifactRuntimeReleaseLocator) -> Self {
        Self {
            package_manager: PackageManagerConfig::new(
                codex_home,
                ArtifactRuntimePackage::new(release),
            ),
        }
    }

    pub fn with_cache_root(mut self, cache_root: PathBuf) -> Self {
        self.package_manager = self.package_manager.with_cache_root(cache_root);
        self
    }

    pub fn cache_root(&self) -> PathBuf {
        self.package_manager.cache_root()
    }

    pub fn release(&self) -> &ArtifactRuntimeReleaseLocator {
        &self.package_manager.package().release
    }

    pub fn codex_home(&self) -> &Path {
        self.package_manager.codex_home()
    }
}

#[derive(Clone, Debug)]
pub struct ArtifactRuntimeManager {
    package_manager: PackageManager<ArtifactRuntimePackage>,
    config: ArtifactRuntimeManagerConfig,
}

impl ArtifactRuntimeManager {
    pub fn new(config: ArtifactRuntimeManagerConfig) -> Self {
        let package_manager = PackageManager::new(config.package_manager.clone());
        Self {
            package_manager,
            config,
        }
    }

    pub fn with_client(config: ArtifactRuntimeManagerConfig, client: Client) -> Self {
        let package_manager = PackageManager::with_client(config.package_manager.clone(), client);
        Self {
            package_manager,
            config,
        }
    }

    pub fn config(&self) -> &ArtifactRuntimeManagerConfig {
        &self.config
    }

    pub async fn resolve_cached(
        &self,
    ) -> Result<Option<InstalledArtifactRuntime>, ArtifactRuntimeError> {
        self.package_manager.resolve_cached().await
    }

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
            .join(&archive.archive)
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
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReleaseManifest {
    pub schema_version: u32,
    pub runtime_version: String,
    pub release_tag: String,
    #[serde(default)]
    pub node_version: Option<String>,
    pub platforms: BTreeMap<String, PackageReleaseArchive>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ExtractedRuntimeManifest {
    pub schema_version: u32,
    pub runtime_version: String,
    pub node: RuntimePathEntry,
    pub entrypoints: RuntimeEntrypoints,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RuntimePathEntry {
    pub relative_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RuntimeEntrypoints {
    pub build_js: RuntimePathEntry,
    pub render_cli: RuntimePathEntry,
}

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

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn runtime_version(&self) -> &str {
        &self.runtime_version
    }

    pub fn platform(&self) -> ArtifactRuntimePlatform {
        self.platform
    }

    pub fn manifest(&self) -> &ExtractedRuntimeManifest {
        &self.manifest
    }

    pub fn node_path(&self) -> &Path {
        &self.node_path
    }

    pub fn build_js_path(&self) -> &Path {
        &self.build_js_path
    }

    pub fn render_cli_path(&self) -> &Path {
        &self.render_cli_path
    }
}

#[derive(Debug, Error)]
pub enum ArtifactRuntimeError {
    #[error(transparent)]
    PackageManager(#[from] PackageManagerError),
    #[error("{context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid manifest at {path}")]
    InvalidManifest {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("runtime path `{0}` is invalid")]
    InvalidRuntimePath(String),
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use sha2::Digest;
    use sha2::Sha256;
    use std::io::Cursor;
    use std::io::Write;
    use tempfile::TempDir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    #[test]
    fn release_locator_builds_manifest_url() {
        let locator = ArtifactRuntimeReleaseLocator::new(
            Url::parse("https://example.test/releases/").unwrap_or_else(|error| panic!("{error}")),
            "0.1.0",
        );
        let url = locator
            .manifest_url()
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            url.as_str(),
            "https://example.test/releases/artifact-runtime-v0.1.0-manifest.json"
        );
    }

    #[tokio::test]
    async fn ensure_installed_downloads_and_extracts_zip_runtime() {
        let server = MockServer::start().await;
        let runtime_version = "0.1.0";
        let platform =
            ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
        let archive_name = format!(
            "artifact-runtime-v{runtime_version}-{}.zip",
            platform.as_str()
        );
        let archive_bytes = build_zip_archive(runtime_version);
        let archive_sha = format!("{:x}", Sha256::digest(&archive_bytes));
        let manifest = ReleaseManifest {
            schema_version: 1,
            runtime_version: runtime_version.to_string(),
            release_tag: format!("artifact-runtime-v{runtime_version}"),
            node_version: Some("22.0.0".to_string()),
            platforms: BTreeMap::from([(
                platform.as_str().to_string(),
                PackageReleaseArchive {
                    archive: archive_name.clone(),
                    sha256: archive_sha,
                    format: codex_package_manager::ArchiveFormat::Zip,
                    size_bytes: Some(archive_bytes.len() as u64),
                },
            )]),
        };
        Mock::given(method("GET"))
            .and(path(format!(
                "/artifact-runtime-v{runtime_version}-manifest.json"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(&manifest))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/{archive_name}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(archive_bytes))
            .mount(&server)
            .await;

        let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
        let locator = ArtifactRuntimeReleaseLocator::new(
            Url::parse(&format!("{}/", server.uri())).unwrap_or_else(|error| panic!("{error}")),
            runtime_version,
        );
        let manager = ArtifactRuntimeManager::new(ArtifactRuntimeManagerConfig::new(
            codex_home.path().to_path_buf(),
            locator,
        ));

        let runtime = manager
            .ensure_installed()
            .await
            .unwrap_or_else(|error| panic!("{error}"));

        assert_eq!(runtime.runtime_version(), runtime_version);
        assert_eq!(runtime.platform(), platform);
        assert!(runtime.node_path().ends_with(Path::new("node/bin/node")));
        assert!(
            runtime
                .build_js_path()
                .ends_with(Path::new("artifact-tool/dist/artifact_tool.mjs"))
        );
        assert!(
            runtime
                .render_cli_path()
                .ends_with(Path::new("granola-render/dist/cli.mjs"))
        );
    }

    fn build_zip_archive(runtime_version: &str) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut bytes);
            let options = SimpleFileOptions::default();
            let manifest = serde_json::to_vec(&sample_extracted_manifest(runtime_version))
                .unwrap_or_else(|error| panic!("{error}"));
            zip.start_file("artifact-runtime/manifest.json", options)
                .unwrap_or_else(|error| panic!("{error}"));
            zip.write_all(&manifest)
                .unwrap_or_else(|error| panic!("{error}"));
            zip.start_file("artifact-runtime/node/bin/node", options)
                .unwrap_or_else(|error| panic!("{error}"));
            zip.write_all(b"#!/bin/sh\n")
                .unwrap_or_else(|error| panic!("{error}"));
            zip.start_file(
                "artifact-runtime/artifact-tool/dist/artifact_tool.mjs",
                options,
            )
            .unwrap_or_else(|error| panic!("{error}"));
            zip.write_all(b"export const ok = true;\n")
                .unwrap_or_else(|error| panic!("{error}"));
            zip.start_file("artifact-runtime/granola-render/dist/cli.mjs", options)
                .unwrap_or_else(|error| panic!("{error}"));
            zip.write_all(b"export const ok = true;\n")
                .unwrap_or_else(|error| panic!("{error}"));
            zip.finish().unwrap_or_else(|error| panic!("{error}"));
        }
        bytes.into_inner()
    }

    fn sample_extracted_manifest(runtime_version: &str) -> ExtractedRuntimeManifest {
        ExtractedRuntimeManifest {
            schema_version: 1,
            runtime_version: runtime_version.to_string(),
            node: RuntimePathEntry {
                relative_path: "node/bin/node".to_string(),
            },
            entrypoints: RuntimeEntrypoints {
                build_js: RuntimePathEntry {
                    relative_path: "artifact-tool/dist/artifact_tool.mjs".to_string(),
                },
                render_cli: RuntimePathEntry {
                    relative_path: "granola-render/dist/cli.mjs".to_string(),
                },
            },
        }
    }
}
