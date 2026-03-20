use crate::ArtifactBuildRequest;
use crate::ArtifactCommandOutput;
use crate::ArtifactRuntimeManager;
use crate::ArtifactRuntimeManagerConfig;
use crate::ArtifactRuntimePlatform;
use crate::ArtifactRuntimeReleaseLocator;
use crate::ArtifactsClient;
use crate::DEFAULT_CACHE_ROOT_RELATIVE;
use crate::ReleaseManifest;
use crate::load_cached_runtime;
use codex_package_manager::ArchiveFormat;
use codex_package_manager::PackageReleaseArchive;
use flate2::Compression;
use flate2::write::GzEncoder;
use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use tar::Builder as TarBuilder;
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
        url::Url::parse("https://example.test/releases/").unwrap_or_else(|error| panic!("{error}")),
        "0.1.0",
    );
    let url = locator
        .manifest_url()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        url.as_str(),
        "https://example.test/releases/artifact-runtime-v0.1.0/artifact-runtime-v0.1.0-manifest.json"
    );
}

#[test]
fn default_release_locator_uses_openai_codex_github_releases() {
    let locator = ArtifactRuntimeReleaseLocator::default("0.1.0");
    let url = locator
        .manifest_url()
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        url.as_str(),
        "https://github.com/openai/codex/releases/download/artifact-runtime-v0.1.0/artifact-runtime-v0.1.0-manifest.json"
    );
}

#[test]
fn load_cached_runtime_reads_installed_runtime() {
    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let runtime_version = "2.5.6";
    let platform =
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let install_dir = codex_home
        .path()
        .join(DEFAULT_CACHE_ROOT_RELATIVE)
        .join(runtime_version)
        .join(platform.as_str());
    write_installed_runtime(&install_dir, runtime_version);

    let runtime = load_cached_runtime(
        &codex_home.path().join(DEFAULT_CACHE_ROOT_RELATIVE),
        runtime_version,
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(runtime.runtime_version(), runtime_version);
    assert_eq!(runtime.platform(), platform);
    assert!(
        runtime
            .build_js_path()
            .ends_with(Path::new("dist/artifact_tool.mjs"))
    );
}

#[test]
fn load_cached_runtime_requires_build_entrypoint() {
    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let runtime_version = "2.5.6";
    let platform =
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let install_dir = codex_home
        .path()
        .join(DEFAULT_CACHE_ROOT_RELATIVE)
        .join(runtime_version)
        .join(platform.as_str());
    write_installed_runtime(&install_dir, runtime_version);
    fs::remove_file(install_dir.join("dist/artifact_tool.mjs"))
        .unwrap_or_else(|error| panic!("{error}"));

    let error = load_cached_runtime(
        &codex_home.path().join(DEFAULT_CACHE_ROOT_RELATIVE),
        runtime_version,
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        format!(
            "required runtime file is missing: {}",
            install_dir.join("dist/artifact_tool.mjs").display()
        )
    );
}

#[tokio::test]
async fn ensure_installed_downloads_and_extracts_zip_runtime() {
    let server = MockServer::start().await;
    let runtime_version = "2.5.6";
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
        node_version: None,
        platforms: BTreeMap::from([(
            platform.as_str().to_string(),
            PackageReleaseArchive {
                archive: archive_name.clone(),
                sha256: archive_sha,
                format: ArchiveFormat::Zip,
                size_bytes: Some(archive_bytes.len() as u64),
            },
        )]),
    };
    Mock::given(method("GET"))
        .and(path(format!(
            "/artifact-runtime-v{runtime_version}/artifact-runtime-v{runtime_version}-manifest.json"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(&manifest))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/artifact-runtime-v{runtime_version}/{archive_name}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive_bytes))
        .mount(&server)
        .await;

    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let locator = ArtifactRuntimeReleaseLocator::new(
        url::Url::parse(&format!("{}/", server.uri())).unwrap_or_else(|error| panic!("{error}")),
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
    assert!(
        runtime
            .build_js_path()
            .ends_with(Path::new("dist/artifact_tool.mjs"))
    );
}

#[test]
fn load_cached_runtime_requires_package_export() {
    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let runtime_version = "2.5.6";
    let platform =
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let install_dir = codex_home
        .path()
        .join(DEFAULT_CACHE_ROOT_RELATIVE)
        .join(runtime_version)
        .join(platform.as_str());
    write_installed_runtime(&install_dir, runtime_version);
    fs::write(
        install_dir.join("package.json"),
        serde_json::json!({
            "name": "@oai/artifact-tool",
            "version": runtime_version,
            "type": "module",
        })
        .to_string(),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    let error = load_cached_runtime(
        &codex_home.path().join(DEFAULT_CACHE_ROOT_RELATIVE),
        runtime_version,
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        format!(
            "invalid package metadata at {}",
            install_dir.join("package.json").display()
        )
    );
}

#[tokio::test]
async fn ensure_installed_downloads_and_extracts_tar_gz_runtime() {
    let server = MockServer::start().await;
    let runtime_version = "2.5.6";
    let platform =
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let archive_name = format!(
        "artifact-runtime-v{runtime_version}-{}.tar.gz",
        platform.as_str()
    );
    let archive_bytes = build_tar_gz_archive(runtime_version);
    let archive_sha = format!("{:x}", Sha256::digest(&archive_bytes));
    let manifest = ReleaseManifest {
        schema_version: 1,
        runtime_version: runtime_version.to_string(),
        release_tag: format!("artifact-runtime-v{runtime_version}"),
        node_version: None,
        platforms: BTreeMap::from([(
            platform.as_str().to_string(),
            PackageReleaseArchive {
                archive: archive_name.clone(),
                sha256: archive_sha,
                format: ArchiveFormat::TarGz,
                size_bytes: Some(archive_bytes.len() as u64),
            },
        )]),
    };
    Mock::given(method("GET"))
        .and(path(format!(
            "/artifact-runtime-v{runtime_version}/artifact-runtime-v{runtime_version}-manifest.json"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(&manifest))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/artifact-runtime-v{runtime_version}/{archive_name}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive_bytes))
        .mount(&server)
        .await;

    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let locator = ArtifactRuntimeReleaseLocator::new(
        url::Url::parse(&format!("{}/", server.uri())).unwrap_or_else(|error| panic!("{error}")),
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
    assert!(
        runtime
            .build_js_path()
            .ends_with(Path::new("dist/artifact_tool.mjs"))
    );
}

#[test]
fn load_cached_runtime_uses_custom_cache_root() {
    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let runtime_version = "2.5.6";
    let custom_cache_root = codex_home.path().join("runtime-cache");
    let platform =
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let install_dir = custom_cache_root
        .join(runtime_version)
        .join(platform.as_str());
    write_installed_runtime(&install_dir, runtime_version);

    let config = ArtifactRuntimeManagerConfig::with_default_release(
        codex_home.path().to_path_buf(),
        runtime_version,
    )
    .with_cache_root(custom_cache_root);

    let runtime = load_cached_runtime(&config.cache_root(), runtime_version)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(runtime.runtime_version(), runtime_version);
    assert_eq!(runtime.platform(), platform);
}

#[tokio::test]
#[cfg(unix)]
async fn artifacts_client_execute_build_writes_wrapped_script_and_env() {
    let temp = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let runtime_root = temp.path().join("runtime");
    write_installed_runtime(&runtime_root, "2.5.6");
    let runtime = crate::InstalledArtifactRuntime::load(
        runtime_root,
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}")),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let client = ArtifactsClient::from_installed_runtime(runtime);

    let output = client
        .execute_build(ArtifactBuildRequest {
            source: concat!(
                "console.log(typeof artifacts);\n",
                "console.log(typeof codexArtifacts);\n",
                "console.log(artifactTool.ok);\n",
                "console.log(ok);\n",
                "console.error('stderr-ok');\n",
                "console.log('stdout-ok');\n"
            )
            .to_string(),
            cwd: temp.path().to_path_buf(),
            timeout: Some(Duration::from_secs(5)),
            env: BTreeMap::new(),
        })
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    assert_success(&output);
    assert_eq!(output.stderr.trim(), "stderr-ok");
    assert_eq!(
        output.stdout.lines().collect::<Vec<_>>(),
        vec!["undefined", "undefined", "true", "true", "stdout-ok"]
    );
}

fn assert_success(output: &ArtifactCommandOutput) {
    assert!(output.success());
    assert_eq!(output.exit_code, Some(0));
}

fn write_installed_runtime(install_dir: &Path, runtime_version: &str) {
    fs::create_dir_all(install_dir.join("dist")).unwrap_or_else(|error| panic!("{error}"));
    fs::write(
        install_dir.join("package.json"),
        serde_json::json!({
            "name": "@oai/artifact-tool",
            "version": runtime_version,
            "type": "module",
            "exports": {
                ".": "./dist/artifact_tool.mjs",
            }
        })
        .to_string(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    fs::write(
        install_dir.join("dist/artifact_tool.mjs"),
        "export const ok = true;\n",
    )
    .unwrap_or_else(|error| panic!("{error}"));
}

fn build_zip_archive(runtime_version: &str) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut bytes);
        let options = SimpleFileOptions::default();
        let package_json = serde_json::json!({
            "name": "@oai/artifact-tool",
            "version": runtime_version,
            "type": "module",
            "exports": {
                ".": "./dist/artifact_tool.mjs",
            }
        })
        .to_string()
        .into_bytes();
        zip.start_file("artifact-runtime/package.json", options)
            .unwrap_or_else(|error| panic!("{error}"));
        zip.write_all(&package_json)
            .unwrap_or_else(|error| panic!("{error}"));
        zip.start_file("artifact-runtime/dist/artifact_tool.mjs", options)
            .unwrap_or_else(|error| panic!("{error}"));
        zip.write_all(b"export const ok = true;\n")
            .unwrap_or_else(|error| panic!("{error}"));
        zip.finish().unwrap_or_else(|error| panic!("{error}"));
    }
    bytes.into_inner()
}

fn build_tar_gz_archive(runtime_version: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let encoder = GzEncoder::new(&mut bytes, Compression::default());
        let mut archive = TarBuilder::new(encoder);

        let package_json = serde_json::json!({
            "name": "@oai/artifact-tool",
            "version": runtime_version,
            "type": "module",
            "exports": {
                ".": "./dist/artifact_tool.mjs",
            }
        })
        .to_string()
        .into_bytes();
        let mut package_header = tar::Header::new_gnu();
        package_header.set_mode(0o644);
        package_header.set_size(package_json.len() as u64);
        package_header.set_cksum();
        archive
            .append_data(
                &mut package_header,
                "package/package.json",
                package_json.as_slice(),
            )
            .unwrap_or_else(|error| panic!("{error}"));

        let build_js = b"export const ok = true;\n";
        let mut build_header = tar::Header::new_gnu();
        build_header.set_mode(0o644);
        build_header.set_size(build_js.len() as u64);
        build_header.set_cksum();
        archive
            .append_data(
                &mut build_header,
                "package/dist/artifact_tool.mjs",
                &build_js[..],
            )
            .unwrap_or_else(|error| panic!("{error}"));

        archive.finish().unwrap_or_else(|error| panic!("{error}"));
    }
    bytes
}
