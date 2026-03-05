use flate2::read::GzDecoder;
use reqwest::Client;
use serde::de::DeserializeOwned;
use sha2::Digest;
use sha2::Sha256;
use std::fs::File;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use tar::Archive;
use tempfile::tempdir_in;
use thiserror::Error;
use tokio::fs;
use url::Url;
use zip::ZipArchive;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageManagerConfig<P> {
    codex_home: PathBuf,
    package: P,
    cache_root: Option<PathBuf>,
}

impl<P> PackageManagerConfig<P> {
    pub fn new(codex_home: PathBuf, package: P) -> Self {
        Self {
            codex_home,
            package,
            cache_root: None,
        }
    }

    pub fn with_cache_root(mut self, cache_root: PathBuf) -> Self {
        self.cache_root = Some(cache_root);
        self
    }

    pub fn codex_home(&self) -> &Path {
        &self.codex_home
    }

    pub fn package(&self) -> &P {
        &self.package
    }
}

impl<P: ManagedPackage> PackageManagerConfig<P> {
    pub fn cache_root(&self) -> PathBuf {
        self.cache_root.clone().unwrap_or_else(|| {
            self.codex_home.join(
                self.package
                    .default_cache_root_relative()
                    .replace('/', std::path::MAIN_SEPARATOR_STR),
            )
        })
    }
}

#[derive(Clone, Debug)]
pub struct PackageManager<P> {
    client: Client,
    config: PackageManagerConfig<P>,
}

impl<P> PackageManager<P> {
    pub fn new(config: PackageManagerConfig<P>) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub fn with_client(config: PackageManagerConfig<P>, client: Client) -> Self {
        Self { client, config }
    }

    pub fn config(&self) -> &PackageManagerConfig<P> {
        &self.config
    }
}

impl<P: ManagedPackage> PackageManager<P> {
    pub async fn resolve_cached(&self) -> Result<Option<P::Installed>, P::Error> {
        let platform = PackagePlatform::detect_current().map_err(P::Error::from)?;
        let install_dir = self
            .config
            .package()
            .install_dir(&self.config.cache_root(), platform);
        if !fs::try_exists(&install_dir)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to read {}", install_dir.display()),
                source,
            })
            .map_err(P::Error::from)?
        {
            return Ok(None);
        }

        let package = self
            .config
            .package()
            .load_installed(install_dir, platform)?;
        if self.config.package().installed_version(&package) != self.config.package().version() {
            return Ok(None);
        }
        Ok(Some(package))
    }

    pub async fn ensure_installed(&self) -> Result<P::Installed, P::Error> {
        if let Some(package) = self.resolve_cached().await? {
            return Ok(package);
        }

        let platform = PackagePlatform::detect_current().map_err(P::Error::from)?;
        let manifest = self.fetch_release_manifest().await?;
        if self.config.package().release_version(&manifest) != self.config.package().version() {
            return Err(PackageManagerError::UnexpectedPackageVersion {
                expected: self.config.package().version().to_string(),
                actual: self.config.package().release_version(&manifest).to_string(),
            }
            .into());
        }

        let platform_archive = self
            .config
            .package()
            .platform_archive(&manifest, platform)?;
        let archive_url = self
            .config
            .package()
            .archive_url(&platform_archive)
            .map_err(P::Error::from)?;
        let archive_bytes = self.download_bytes(&archive_url).await?;
        verify_sha256(&archive_bytes, &platform_archive.sha256).map_err(P::Error::from)?;

        let install_dir = self
            .config
            .package()
            .install_dir(&self.config.cache_root(), platform);
        if fs::try_exists(&install_dir)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to read {}", install_dir.display()),
                source,
            })
            .map_err(P::Error::from)?
        {
            fs::remove_dir_all(&install_dir)
                .await
                .map_err(|source| PackageManagerError::Io {
                    context: format!("failed to remove {}", install_dir.display()),
                    source,
                })
                .map_err(P::Error::from)?;
        }

        let cache_root = self.config.cache_root();
        fs::create_dir_all(&cache_root)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", cache_root.display()),
                source,
            })
            .map_err(P::Error::from)?;

        let staging_dir = tempdir_in(&cache_root)
            .map_err(|source| PackageManagerError::Io {
                context: format!(
                    "failed to create staging directory in {}",
                    cache_root.display()
                ),
                source,
            })
            .map_err(P::Error::from)?;
        let archive_path = staging_dir.path().join(&platform_archive.archive);
        fs::write(&archive_path, &archive_bytes)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to write {}", archive_path.display()),
                source,
            })
            .map_err(P::Error::from)?;
        let extraction_root = staging_dir.path().join("extract");
        fs::create_dir_all(&extraction_root)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", extraction_root.display()),
                source,
            })
            .map_err(P::Error::from)?;

        extract_archive(&archive_path, &extraction_root, platform_archive.format)
            .map_err(P::Error::from)?;
        let extracted_root = self
            .config
            .package()
            .detect_extracted_root(&extraction_root)?;
        let package = self
            .config
            .package()
            .load_installed(extracted_root.clone(), platform)?;
        if self.config.package().installed_version(&package) != self.config.package().version() {
            return Err(PackageManagerError::UnexpectedPackageVersion {
                expected: self.config.package().version().to_string(),
                actual: self
                    .config
                    .package()
                    .installed_version(&package)
                    .to_string(),
            }
            .into());
        }

        if let Some(parent) = install_dir.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|source| PackageManagerError::Io {
                    context: format!("failed to create {}", parent.display()),
                    source,
                })
                .map_err(P::Error::from)?;
        }

        fs::rename(&extracted_root, &install_dir)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!(
                    "failed to move {} to {}",
                    extracted_root.display(),
                    install_dir.display()
                ),
                source,
            })
            .map_err(P::Error::from)?;

        self.config.package().load_installed(install_dir, platform)
    }

    async fn fetch_release_manifest(&self) -> Result<P::ReleaseManifest, P::Error> {
        let manifest_url = self
            .config
            .package()
            .manifest_url()
            .map_err(P::Error::from)?;
        let response = self
            .client
            .get(manifest_url.clone())
            .send()
            .await
            .map_err(|source| PackageManagerError::Http {
                context: format!("failed to fetch {manifest_url}"),
                source,
            })
            .map_err(P::Error::from)?
            .error_for_status()
            .map_err(|source| PackageManagerError::Http {
                context: format!("manifest request failed for {manifest_url}"),
                source,
            })
            .map_err(P::Error::from)?;

        response
            .json::<P::ReleaseManifest>()
            .await
            .map_err(|source| PackageManagerError::Http {
                context: format!("failed to decode manifest from {manifest_url}"),
                source,
            })
            .map_err(P::Error::from)
    }

    async fn download_bytes(&self, url: &Url) -> Result<Vec<u8>, P::Error> {
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|source| PackageManagerError::Http {
                context: format!("failed to download {url}"),
                source,
            })
            .map_err(P::Error::from)?
            .error_for_status()
            .map_err(|source| PackageManagerError::Http {
                context: format!("archive request failed for {url}"),
                source,
            })
            .map_err(P::Error::from)?;
        let bytes = response
            .bytes()
            .await
            .map_err(|source| PackageManagerError::Http {
                context: format!("failed to read response body for {url}"),
                source,
            })
            .map_err(P::Error::from)?;
        Ok(bytes.to_vec())
    }
}

pub trait ManagedPackage: Clone {
    type Error: From<PackageManagerError>;
    type Installed: Clone;
    type ReleaseManifest: DeserializeOwned;

    fn default_cache_root_relative(&self) -> &str;
    fn version(&self) -> &str;
    fn manifest_url(&self) -> Result<Url, PackageManagerError>;
    fn archive_url(&self, archive: &PackageReleaseArchive) -> Result<Url, PackageManagerError>;
    fn release_version<'a>(&self, manifest: &'a Self::ReleaseManifest) -> &'a str;
    fn platform_archive(
        &self,
        manifest: &Self::ReleaseManifest,
        platform: PackagePlatform,
    ) -> Result<PackageReleaseArchive, Self::Error>;
    fn install_dir(&self, cache_root: &Path, platform: PackagePlatform) -> PathBuf;
    fn installed_version<'a>(&self, package: &'a Self::Installed) -> &'a str;
    fn load_installed(
        &self,
        root_dir: PathBuf,
        platform: PackagePlatform,
    ) -> Result<Self::Installed, Self::Error>;

    fn detect_extracted_root(&self, extraction_root: &Path) -> Result<PathBuf, Self::Error> {
        detect_single_package_root(extraction_root).map_err(Self::Error::from)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackagePlatform {
    DarwinArm64,
    DarwinX64,
    LinuxArm64,
    LinuxX64,
    WindowsArm64,
    WindowsX64,
}

impl PackagePlatform {
    pub fn detect_current() -> Result<Self, PackageManagerError> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") | ("macos", "arm64") => Ok(Self::DarwinArm64),
            ("macos", "x86_64") => Ok(Self::DarwinX64),
            ("linux", "aarch64") | ("linux", "arm64") => Ok(Self::LinuxArm64),
            ("linux", "x86_64") => Ok(Self::LinuxX64),
            ("windows", "aarch64") | ("windows", "arm64") => Ok(Self::WindowsArm64),
            ("windows", "x86_64") => Ok(Self::WindowsX64),
            (os, arch) => Err(PackageManagerError::UnsupportedPlatform {
                os: os.to_string(),
                arch: arch.to_string(),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DarwinArm64 => "darwin-arm64",
            Self::DarwinX64 => "darwin-x64",
            Self::LinuxArm64 => "linux-arm64",
            Self::LinuxX64 => "linux-x64",
            Self::WindowsArm64 => "windows-arm64",
            Self::WindowsX64 => "windows-x64",
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct PackageReleaseArchive {
    pub archive: String,
    pub sha256: String,
    pub format: ArchiveFormat,
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum ArchiveFormat {
    #[serde(rename = "zip")]
    Zip,
    #[serde(rename = "tar.gz")]
    TarGz,
}

#[derive(Debug, Error)]
pub enum PackageManagerError {
    #[error("unsupported platform: {os}-{arch}")]
    UnsupportedPlatform { os: String, arch: String },
    #[error("invalid release base url")]
    InvalidBaseUrl(#[source] url::ParseError),
    #[error("{context}")]
    Http {
        context: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("missing platform entry `{0}` in release manifest")]
    MissingPlatform(String),
    #[error("unexpected package version: expected `{expected}`, got `{actual}`")]
    UnexpectedPackageVersion { expected: String, actual: String },
    #[error("checksum mismatch: expected `{expected}`, got `{actual}`")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("archive extraction failed: {0}")]
    ArchiveExtraction(String),
    #[error("archive did not contain a package root with manifest.json under {0}")]
    MissingPackageRoot(PathBuf),
}

pub fn detect_single_package_root(extraction_root: &Path) -> Result<PathBuf, PackageManagerError> {
    let direct_manifest = extraction_root.join("manifest.json");
    if direct_manifest.exists() {
        return Ok(extraction_root.to_path_buf());
    }

    let mut directory_candidates = Vec::new();
    for entry in std::fs::read_dir(extraction_root).map_err(|source| PackageManagerError::Io {
        context: format!("failed to read {}", extraction_root.display()),
        source,
    })? {
        let entry = entry.map_err(|source| PackageManagerError::Io {
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
        if candidate.join("manifest.json").exists() {
            return Ok(candidate.clone());
        }
    }

    Err(PackageManagerError::MissingPackageRoot(
        extraction_root.to_path_buf(),
    ))
}

fn verify_sha256(bytes: &[u8], expected: &str) -> Result<(), PackageManagerError> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual == expected.to_ascii_lowercase() {
        return Ok(());
    }
    Err(PackageManagerError::ChecksumMismatch {
        expected: expected.to_string(),
        actual,
    })
}

fn extract_archive(
    archive_path: &Path,
    destination: &Path,
    format: ArchiveFormat,
) -> Result<(), PackageManagerError> {
    match format {
        ArchiveFormat::Zip => extract_zip_archive(archive_path, destination),
        ArchiveFormat::TarGz => extract_tar_gz_archive(archive_path, destination),
    }
}

fn extract_zip_archive(archive_path: &Path, destination: &Path) -> Result<(), PackageManagerError> {
    let file = File::open(archive_path).map_err(|source| PackageManagerError::Io {
        context: format!("failed to open {}", archive_path.display()),
        source,
    })?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?;
        let Some(relative_path) = entry.enclosed_name() else {
            return Err(PackageManagerError::ArchiveExtraction(format!(
                "zip entry `{}` escapes extraction root",
                entry.name()
            )));
        };
        let output_path = destination.join(relative_path);
        if entry.is_dir() {
            std::fs::create_dir_all(&output_path).map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", output_path.display()),
                source,
            })?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", parent.display()),
                source,
            })?;
        }
        let mut output = File::create(&output_path).map_err(|source| PackageManagerError::Io {
            context: format!("failed to create {}", output_path.display()),
            source,
        })?;
        std::io::copy(&mut entry, &mut output).map_err(|source| PackageManagerError::Io {
            context: format!("failed to write {}", output_path.display()),
            source,
        })?;
    }
    Ok(())
}

fn extract_tar_gz_archive(
    archive_path: &Path,
    destination: &Path,
) -> Result<(), PackageManagerError> {
    let file = File::open(archive_path).map_err(|source| PackageManagerError::Io {
        context: format!("failed to open {}", archive_path.display()),
        source,
    })?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    for entry in archive
        .entries()
        .map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?
    {
        let mut entry =
            entry.map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?;
        let path = entry
            .path()
            .map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?;
        let output_path = safe_extract_path(destination, path.as_ref())?;
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", parent.display()),
                source,
            })?;
        }
        entry
            .unpack(&output_path)
            .map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?;
    }
    Ok(())
}

fn safe_extract_path(root: &Path, relative_path: &Path) -> Result<PathBuf, PackageManagerError> {
    let mut clean_relative = PathBuf::new();
    for component in relative_path.components() {
        match component {
            Component::Normal(segment) => clean_relative.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PackageManagerError::ArchiveExtraction(format!(
                    "entry `{}` escapes extraction root",
                    relative_path.display()
                )));
            }
        }
    }

    if clean_relative.as_os_str().is_empty() {
        return Err(PackageManagerError::ArchiveExtraction(
            "archive entry had an empty path".to_string(),
        ));
    }
    Ok(root.join(clean_relative))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use std::collections::BTreeMap;
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

    #[derive(Clone, Debug)]
    struct TestPackage {
        base_url: Url,
        version: String,
    }

    #[derive(Clone, Debug, Deserialize)]
    struct TestReleaseManifest {
        package_version: String,
        platforms: BTreeMap<String, PackageReleaseArchive>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct TestInstalledPackage {
        version: String,
        platform: PackagePlatform,
        root_dir: PathBuf,
    }

    impl ManagedPackage for TestPackage {
        type Error = PackageManagerError;
        type Installed = TestInstalledPackage;
        type ReleaseManifest = TestReleaseManifest;

        fn default_cache_root_relative(&self) -> &str {
            "packages/test-package"
        }

        fn version(&self) -> &str {
            &self.version
        }

        fn manifest_url(&self) -> Result<Url, PackageManagerError> {
            self.base_url
                .join(&format!("test-package-v{}-manifest.json", self.version))
                .map_err(PackageManagerError::InvalidBaseUrl)
        }

        fn archive_url(&self, archive: &PackageReleaseArchive) -> Result<Url, PackageManagerError> {
            self.base_url
                .join(&archive.archive)
                .map_err(PackageManagerError::InvalidBaseUrl)
        }

        fn release_version<'a>(&self, manifest: &'a Self::ReleaseManifest) -> &'a str {
            &manifest.package_version
        }

        fn platform_archive(
            &self,
            manifest: &Self::ReleaseManifest,
            platform: PackagePlatform,
        ) -> Result<PackageReleaseArchive, Self::Error> {
            manifest
                .platforms
                .get(platform.as_str())
                .cloned()
                .ok_or_else(|| PackageManagerError::MissingPlatform(platform.as_str().to_string()))
        }

        fn install_dir(&self, cache_root: &Path, platform: PackagePlatform) -> PathBuf {
            cache_root.join(self.version()).join(platform.as_str())
        }

        fn installed_version<'a>(&self, package: &'a Self::Installed) -> &'a str {
            &package.version
        }

        fn load_installed(
            &self,
            root_dir: PathBuf,
            platform: PackagePlatform,
        ) -> Result<Self::Installed, Self::Error> {
            let version =
                std::fs::read_to_string(root_dir.join("manifest.json")).map_err(|source| {
                    PackageManagerError::Io {
                        context: format!(
                            "failed to read {}",
                            root_dir.join("manifest.json").display()
                        ),
                        source,
                    }
                })?;
            Ok(TestInstalledPackage {
                version: version.trim().to_string(),
                platform,
                root_dir,
            })
        }
    }

    #[tokio::test]
    async fn ensure_installed_downloads_and_extracts_zip_package() {
        let server = MockServer::start().await;
        let version = "0.1.0";
        let platform = PackagePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
        let archive_name = format!("test-package-v{version}-{}.zip", platform.as_str());
        let archive_bytes = build_zip_archive(version);
        let archive_sha = format!("{:x}", Sha256::digest(&archive_bytes));
        let manifest = serde_json::json!({
            "package_version": version,
            "platforms": {
                platform.as_str(): {
                    "archive": archive_name,
                    "sha256": archive_sha,
                    "format": "zip",
                    "size_bytes": archive_bytes.len(),
                }
            }
        });
        Mock::given(method("GET"))
            .and(path(format!("/test-package-v{version}-manifest.json")))
            .respond_with(ResponseTemplate::new(200).set_body_json(&manifest))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/{archive_name}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(archive_bytes))
            .mount(&server)
            .await;

        let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
        let package = TestPackage {
            base_url: Url::parse(&format!("{}/", server.uri()))
                .unwrap_or_else(|error| panic!("{error}")),
            version: version.to_string(),
        };
        let manager = PackageManager::new(PackageManagerConfig::new(
            codex_home.path().to_path_buf(),
            package,
        ));

        let installed = manager
            .ensure_installed()
            .await
            .unwrap_or_else(|error| panic!("{error}"));

        assert_eq!(
            installed,
            TestInstalledPackage {
                version: version.to_string(),
                platform,
                root_dir: codex_home
                    .path()
                    .join("packages")
                    .join("test-package")
                    .join(version)
                    .join(platform.as_str()),
            }
        );
    }

    #[test]
    fn tar_gz_extraction_supports_default_package_root_detection() {
        let temp = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
        let archive_path = temp.path().join("package.tar.gz");
        let extraction_root = temp.path().join("extract");
        std::fs::create_dir_all(&extraction_root).unwrap_or_else(|error| panic!("{error}"));
        write_tar_gz_archive(&archive_path, "0.2.0");

        extract_archive(&archive_path, &extraction_root, ArchiveFormat::TarGz)
            .unwrap_or_else(|error| panic!("{error}"));
        let package_root =
            detect_single_package_root(&extraction_root).unwrap_or_else(|error| panic!("{error}"));

        assert!(package_root.join("manifest.json").exists());
    }

    fn build_zip_archive(version: &str) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut bytes);
            let options = SimpleFileOptions::default();
            zip.start_file("test-package/manifest.json", options)
                .unwrap_or_else(|error| panic!("{error}"));
            zip.write_all(version.as_bytes())
                .unwrap_or_else(|error| panic!("{error}"));
            zip.finish().unwrap_or_else(|error| panic!("{error}"));
        }
        bytes.into_inner()
    }

    fn write_tar_gz_archive(archive_path: &Path, version: &str) {
        let file = File::create(archive_path).unwrap_or_else(|error| panic!("{error}"));
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);

        append_tar_file(
            &mut builder,
            "test-package/manifest.json",
            version.as_bytes(),
        );
        builder.finish().unwrap_or_else(|error| panic!("{error}"));
    }

    fn append_tar_file(
        builder: &mut tar::Builder<flate2::write::GzEncoder<File>>,
        path: &str,
        contents: &[u8],
    ) {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder
            .append_data(&mut header, path, contents)
            .unwrap_or_else(|error| panic!("{error}"));
    }
}
