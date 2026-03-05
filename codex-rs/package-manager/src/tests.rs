use crate::ArchiveFormat;
use crate::ManagedPackage;
use crate::PackageManager;
use crate::PackageManagerConfig;
use crate::PackageManagerError;
use crate::PackagePlatform;
use crate::PackageReleaseArchive;
use crate::archive::detect_single_package_root;
use crate::archive::extract_archive;
use crate::manager::promote_staged_install;
use crate::manager::quarantine_existing_install;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use sha2::Digest;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Cursor;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tar::Builder;
use tar::EntryType;
use tempfile::TempDir;
use tokio::sync::Barrier;
use url::Url;
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
    fail_on_final_install_dir: bool,
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
        if self.fail_on_final_install_dir
            && root_dir
                .file_name()
                .is_some_and(|name| name == platform.as_str())
        {
            return Err(PackageManagerError::ArchiveExtraction(format!(
                "refusing final install dir {}",
                root_dir.display()
            )));
        }
        let manifest_path = root_dir.join("manifest.json");
        let version =
            std::fs::read_to_string(&manifest_path).map_err(|source| PackageManagerError::Io {
                context: format!("failed to read {}", manifest_path.display()),
                source,
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
        fail_on_final_install_dir: false,
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

    #[cfg(unix)]
    {
        let executable_mode = std::fs::metadata(installed.root_dir.join("bin/tool"))
            .unwrap_or_else(|error| panic!("{error}"))
            .permissions()
            .mode();
        assert_eq!(executable_mode & 0o111, 0o111);
    }
}

#[tokio::test]
async fn resolve_cached_uses_custom_cache_root() {
    let platform = PackagePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let cache_root = codex_home.path().join("custom-cache");
    let install_dir = cache_root.join("0.1.0").join(platform.as_str());
    std::fs::create_dir_all(&install_dir).unwrap_or_else(|error| panic!("{error}"));
    std::fs::write(install_dir.join("manifest.json"), "0.1.0")
        .unwrap_or_else(|error| panic!("{error}"));

    let manager = PackageManager::new(
        PackageManagerConfig::new(
            codex_home.path().to_path_buf(),
            TestPackage {
                base_url: Url::parse("https://example.test/")
                    .unwrap_or_else(|error| panic!("{error}")),
                version: "0.1.0".to_string(),
                fail_on_final_install_dir: false,
            },
        )
        .with_cache_root(cache_root.clone()),
    );

    let installed = manager
        .resolve_cached()
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        installed,
        Some(TestInstalledPackage {
            version: "0.1.0".to_string(),
            platform,
            root_dir: cache_root.join("0.1.0").join(platform.as_str()),
        })
    );
}

#[tokio::test]
async fn ensure_installed_replaces_invalid_cached_install() {
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
    let install_dir = codex_home
        .path()
        .join("packages")
        .join("test-package")
        .join(version)
        .join(platform.as_str());
    std::fs::create_dir_all(&install_dir).unwrap_or_else(|error| panic!("{error}"));
    std::fs::write(install_dir.join("broken.txt"), "stale")
        .unwrap_or_else(|error| panic!("{error}"));

    let manager = PackageManager::new(PackageManagerConfig::new(
        codex_home.path().to_path_buf(),
        TestPackage {
            base_url: Url::parse(&format!("{}/", server.uri()))
                .unwrap_or_else(|error| panic!("{error}")),
            version: version.to_string(),
            fail_on_final_install_dir: false,
        },
    ));

    let installed = manager
        .ensure_installed()
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(installed.version, version);
    assert!(installed.root_dir.join("manifest.json").exists());
    assert!(!installed.root_dir.join("broken.txt").exists());
}

#[tokio::test]
async fn ensure_installed_rejects_manifest_version_mismatch() {
    let server = MockServer::start().await;
    let version = "0.1.0";
    let platform = PackagePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let archive_name = format!("test-package-v{version}-{}.zip", platform.as_str());
    let manifest = serde_json::json!({
        "package_version": "0.2.0",
        "platforms": {
            platform.as_str(): {
                "archive": archive_name,
                "sha256": "deadbeef",
                "format": "zip",
                "size_bytes": 1,
            }
        }
    });
    Mock::given(method("GET"))
        .and(path(format!("/test-package-v{version}-manifest.json")))
        .respond_with(ResponseTemplate::new(200).set_body_json(&manifest))
        .mount(&server)
        .await;

    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let manager = PackageManager::new(PackageManagerConfig::new(
        codex_home.path().to_path_buf(),
        TestPackage {
            base_url: Url::parse(&format!("{}/", server.uri()))
                .unwrap_or_else(|error| panic!("{error}")),
            version: version.to_string(),
            fail_on_final_install_dir: false,
        },
    ));

    let error = manager
        .ensure_installed()
        .await
        .expect_err("manifest version mismatch should fail");
    assert!(matches!(
        error,
        PackageManagerError::UnexpectedPackageVersion { expected, actual }
            if expected == "0.1.0" && actual == "0.2.0"
    ));
}

#[tokio::test]
async fn ensure_installed_serializes_concurrent_installs() {
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
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/{archive_name}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive_bytes))
        .expect(1)
        .mount(&server)
        .await;

    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let config = PackageManagerConfig::new(
        codex_home.path().to_path_buf(),
        TestPackage {
            base_url: Url::parse(&format!("{}/", server.uri()))
                .unwrap_or_else(|error| panic!("{error}")),
            version: version.to_string(),
            fail_on_final_install_dir: false,
        },
    );
    let manager_one = PackageManager::new(config.clone());
    let manager_two = PackageManager::new(config);
    let barrier = Arc::new(Barrier::new(2));
    let barrier_one = Arc::clone(&barrier);
    let barrier_two = Arc::clone(&barrier);

    let (first, second) = tokio::join!(
        async {
            barrier_one.wait().await;
            manager_one.ensure_installed().await
        },
        async {
            barrier_two.wait().await;
            manager_two.ensure_installed().await
        }
    );

    let first = first.unwrap_or_else(|error| panic!("{error}"));
    let second = second.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(first, second);
}

#[tokio::test]
async fn ensure_installed_rejects_unexpected_archive_size() {
    let server = MockServer::start().await;
    let version = "0.1.0";
    let platform = PackagePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let archive_name = format!("test-package-v{version}-{}.zip", platform.as_str());
    let archive_bytes = build_zip_archive(version);
    let actual_size = archive_bytes.len() as u64;
    let expected_size = (archive_bytes.len() + 1) as u64;
    let archive_sha = format!("{:x}", Sha256::digest(&archive_bytes));
    let manifest = serde_json::json!({
        "package_version": version,
        "platforms": {
            platform.as_str(): {
                "archive": archive_name,
                "sha256": archive_sha,
                "format": "zip",
                "size_bytes": expected_size,
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
    let manager = PackageManager::new(PackageManagerConfig::new(
        codex_home.path().to_path_buf(),
        TestPackage {
            base_url: Url::parse(&format!("{}/", server.uri()))
                .unwrap_or_else(|error| panic!("{error}")),
            version: version.to_string(),
            fail_on_final_install_dir: false,
        },
    ));

    let error = manager
        .ensure_installed()
        .await
        .expect_err("archive size mismatch should fail");
    assert!(matches!(
        error,
        PackageManagerError::UnexpectedArchiveSize { expected, actual }
            if expected == expected_size && actual == actual_size
    ));
}

#[tokio::test]
async fn staged_install_restore_keeps_previous_install_on_failed_promotion() {
    let temp = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let install_dir = temp.path().join("install");
    let staged_dir = temp.path().join("missing-staged");
    std::fs::create_dir_all(&install_dir).unwrap_or_else(|error| panic!("{error}"));
    std::fs::write(install_dir.join("manifest.json"), "0.1.0")
        .unwrap_or_else(|error| panic!("{error}"));

    let quarantined = quarantine_existing_install(&install_dir)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let promotion_error = promote_staged_install(&staged_dir, &install_dir)
        .await
        .expect_err("promotion should fail");
    crate::manager::restore_quarantined_install(
        &install_dir,
        quarantined.as_deref(),
        &promotion_error,
    )
    .await
    .unwrap_or_else(|error| panic!("{error}"));

    assert!(install_dir.join("manifest.json").exists());
    assert_eq!(
        std::fs::read_to_string(install_dir.join("manifest.json"))
            .unwrap_or_else(|error| panic!("{error}")),
        "0.1.0"
    );
}

#[tokio::test]
async fn ensure_installed_restores_previous_install_when_final_validation_fails() {
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
    let install_dir = codex_home
        .path()
        .join("packages")
        .join("test-package")
        .join(version)
        .join(platform.as_str());
    std::fs::create_dir_all(&install_dir).unwrap_or_else(|error| panic!("{error}"));
    std::fs::write(install_dir.join("manifest.json"), "0.0.9")
        .unwrap_or_else(|error| panic!("{error}"));

    let error = PackageManager::new(PackageManagerConfig::new(
        codex_home.path().to_path_buf(),
        TestPackage {
            base_url: Url::parse(&format!("{}/", server.uri()))
                .unwrap_or_else(|error| panic!("{error}")),
            version: version.to_string(),
            fail_on_final_install_dir: true,
        },
    ))
    .ensure_installed()
    .await
    .expect_err("final validation should fail");

    assert!(
        matches!(error, PackageManagerError::ArchiveExtraction(message) if message.contains("refusing final install dir"))
    );
    assert_eq!(
        std::fs::read_to_string(install_dir.join("manifest.json"))
            .unwrap_or_else(|error| panic!("{error}")),
        "0.0.9"
    );
    assert!(
        !install_dir
            .parent()
            .unwrap_or_else(|| panic!("install dir should have a parent"))
            .read_dir()
            .unwrap_or_else(|error| panic!("{error}"))
            .any(|entry| {
                entry
                    .unwrap_or_else(|error| panic!("{error}"))
                    .file_name()
                    .to_string_lossy()
                    .contains(".replaced-")
            })
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

#[test]
fn tar_gz_extraction_rejects_symlinks() {
    let temp = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let archive_path = temp.path().join("package.tar.gz");
    let extraction_root = temp.path().join("extract");
    std::fs::create_dir_all(&extraction_root).unwrap_or_else(|error| panic!("{error}"));
    write_tar_gz_archive_with_symlink(&archive_path);

    let error = extract_archive(&archive_path, &extraction_root, ArchiveFormat::TarGz)
        .expect_err("symlink entry should fail");
    assert!(
        matches!(error, PackageManagerError::ArchiveExtraction(message) if message.contains("unsupported type"))
    );
}

#[test]
fn zip_extraction_rejects_parent_paths() {
    let temp = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let archive_path = temp.path().join("package.zip");
    let extraction_root = temp.path().join("extract");
    std::fs::create_dir_all(&extraction_root).unwrap_or_else(|error| panic!("{error}"));
    write_zip_archive_with_parent_path(&archive_path);

    let error = extract_archive(&archive_path, &extraction_root, ArchiveFormat::Zip)
        .expect_err("parent path entry should fail");
    assert!(
        matches!(error, PackageManagerError::ArchiveExtraction(message) if message.contains("escapes extraction root"))
    );
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
        zip.start_file("test-package/bin/tool", options.unix_permissions(0o755))
            .unwrap_or_else(|error| panic!("{error}"));
        zip.write_all(b"#!/bin/sh\n")
            .unwrap_or_else(|error| panic!("{error}"));
        zip.finish().unwrap_or_else(|error| panic!("{error}"));
    }
    bytes.into_inner()
}

fn write_zip_archive_with_parent_path(archive_path: &Path) {
    let file = File::create(archive_path).unwrap_or_else(|error| panic!("{error}"));
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    zip.start_file("../escape.txt", options)
        .unwrap_or_else(|error| panic!("{error}"));
    zip.write_all(b"escape")
        .unwrap_or_else(|error| panic!("{error}"));
    zip.finish().unwrap_or_else(|error| panic!("{error}"));
}

fn write_tar_gz_archive(archive_path: &Path, version: &str) {
    let file = File::create(archive_path).unwrap_or_else(|error| panic!("{error}"));
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = Builder::new(encoder);

    append_tar_file(
        &mut builder,
        "test-package/manifest.json",
        version.as_bytes(),
    );
    builder.finish().unwrap_or_else(|error| panic!("{error}"));
}

fn write_tar_gz_archive_with_symlink(archive_path: &Path) {
    let file = File::create(archive_path).unwrap_or_else(|error| panic!("{error}"));
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = Builder::new(encoder);

    append_tar_file(&mut builder, "test-package/manifest.json", b"0.2.0");

    let mut header = tar::Header::new_gnu();
    header.set_entry_type(EntryType::Symlink);
    header.set_size(0);
    header.set_mode(0o777);
    header
        .set_link_name("/tmp/escape")
        .unwrap_or_else(|error| panic!("{error}"));
    header.set_cksum();
    builder
        .append_data(&mut header, "test-package/link", std::io::empty())
        .unwrap_or_else(|error| panic!("{error}"));

    builder.finish().unwrap_or_else(|error| panic!("{error}"));
}

fn append_tar_file(
    builder: &mut Builder<flate2::write::GzEncoder<File>>,
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
