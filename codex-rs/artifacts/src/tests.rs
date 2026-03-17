use crate::ArtifactBuildRequest;
use crate::ArtifactCommandOutput;
use crate::ArtifactRenderCommandRequest;
use crate::ArtifactRenderTarget;
use crate::ArtifactRuntimeManager;
use crate::ArtifactRuntimeManagerConfig;
use crate::ArtifactRuntimePlatform;
use crate::ArtifactRuntimeReleaseLocator;
use crate::ArtifactsClient;
use crate::DEFAULT_CACHE_ROOT_RELATIVE;
use crate::ExtractedRuntimeManifest;
use crate::InstalledArtifactRuntime;
use crate::JsRuntime;
use crate::PresentationRenderTarget;
use crate::ReleaseManifest;
use crate::RuntimeEntrypoints;
use crate::RuntimePathEntry;
use crate::SpreadsheetRenderTarget;
use crate::client::VISUAL_ARTIFACTS_MANIFEST_PREFIX;
use crate::load_cached_runtime;
use codex_package_manager::ArchiveFormat;
use codex_package_manager::PackageReleaseArchive;
use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
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
    let runtime_version = "0.1.0";
    let platform =
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let install_dir = codex_home
        .path()
        .join(DEFAULT_CACHE_ROOT_RELATIVE)
        .join(runtime_version)
        .join(platform.as_str());
    write_installed_runtime(
        &install_dir,
        runtime_version,
        Some(PathBuf::from("node/bin/node")),
    );

    let runtime = load_cached_runtime(
        &codex_home.path().join(DEFAULT_CACHE_ROOT_RELATIVE),
        runtime_version,
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(runtime.runtime_version(), runtime_version);
    assert_eq!(runtime.platform(), platform);
    assert!(runtime.node_path().ends_with(Path::new("node/bin/node")));
    assert!(
        runtime
            .build_js_path()
            .ends_with(Path::new("artifact-tool/dist/artifact_tool.mjs"))
    );
}

#[test]
fn load_cached_runtime_rejects_parent_relative_paths() {
    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let runtime_version = "0.1.0";
    let platform =
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let install_dir = codex_home
        .path()
        .join(DEFAULT_CACHE_ROOT_RELATIVE)
        .join(runtime_version)
        .join(platform.as_str());
    write_installed_runtime(
        &install_dir,
        runtime_version,
        Some(PathBuf::from("../node/bin/node")),
    );

    let error = load_cached_runtime(
        &codex_home.path().join(DEFAULT_CACHE_ROOT_RELATIVE),
        runtime_version,
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "runtime path `../node/bin/node` is invalid"
    );
}

#[test]
fn load_cached_runtime_requires_build_entrypoint() {
    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let runtime_version = "0.1.0";
    let platform =
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let install_dir = codex_home
        .path()
        .join(DEFAULT_CACHE_ROOT_RELATIVE)
        .join(runtime_version)
        .join(platform.as_str());
    write_installed_runtime(
        &install_dir,
        runtime_version,
        Some(PathBuf::from("node/bin/node")),
    );
    fs::remove_file(install_dir.join("artifact-tool/dist/artifact_tool.mjs"))
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
            install_dir
                .join("artifact-tool/dist/artifact_tool.mjs")
                .display()
        )
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
    assert!(runtime.node_path().ends_with(Path::new("node/bin/node")));
    assert_eq!(
        runtime.resolve_js_runtime().expect("resolve js runtime"),
        JsRuntime::node(runtime.node_path().to_path_buf())
    );
}

#[test]
fn load_cached_runtime_uses_custom_cache_root() {
    let codex_home = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let runtime_version = "0.1.0";
    let custom_cache_root = codex_home.path().join("runtime-cache");
    let platform =
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}"));
    let install_dir = custom_cache_root
        .join(runtime_version)
        .join(platform.as_str());
    write_installed_runtime(
        &install_dir,
        runtime_version,
        Some(PathBuf::from("node/bin/node")),
    );

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
    let output_path = temp.path().join("build-output.txt");
    let wrapped_script_path = temp.path().join("wrapped-script.mjs");
    let runtime = fake_installed_runtime(temp.path(), &output_path, &wrapped_script_path);
    let client = ArtifactsClient::from_installed_runtime(runtime);

    let output = client
        .execute_build(ArtifactBuildRequest {
            source: "console.log('hello');".to_string(),
            cwd: temp.path().to_path_buf(),
            timeout: Some(Duration::from_secs(5)),
            env: BTreeMap::from([
                (
                    "CODEX_TEST_OUTPUT".to_string(),
                    output_path.display().to_string(),
                ),
                ("CUSTOM_ENV".to_string(), "custom-value".to_string()),
            ]),
        })
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    assert_success(&output);
    let command_log = fs::read_to_string(&output_path).unwrap_or_else(|error| panic!("{error}"));
    assert!(command_log.contains("arg0="));
    assert!(command_log.contains("CODEX_ARTIFACT_BUILD_ENTRYPOINT="));
    assert!(command_log.contains("CODEX_ARTIFACT_RENDER_ENTRYPOINT="));
    assert!(command_log.contains("CUSTOM_ENV=custom-value"));

    let wrapped_script =
        fs::read_to_string(wrapped_script_path).unwrap_or_else(|error| panic!("{error}"));
    assert!(wrapped_script.contains("const emitVisual = (spec) => {"));
    assert!(wrapped_script.contains("const setDocument = (spec) => {"));
    assert!(
        wrapped_script.contains(
            "globalThis.codex = { ...(globalThis.codex ?? {}), emitVisual, setDocument };"
        )
    );
    assert!(wrapped_script.contains("globalThis.artifacts = artifactTool;"));
    assert!(wrapped_script.contains("globalThis.codexArtifacts = artifactTool;"));
    assert!(wrapped_script.contains(VISUAL_ARTIFACTS_MANIFEST_PREFIX));
    assert!(wrapped_script.contains("console.log('hello');"));
}

#[tokio::test]
#[cfg(unix)]
async fn artifacts_client_execute_render_passes_expected_args() {
    let temp = TempDir::new().unwrap_or_else(|error| panic!("{error}"));
    let output_path = temp.path().join("render-output.txt");
    let wrapped_script_path = temp.path().join("unused-script-copy.mjs");
    let runtime = fake_installed_runtime(temp.path(), &output_path, &wrapped_script_path);
    let client = ArtifactsClient::from_installed_runtime(runtime.clone());
    let render_output = temp.path().join("slide.png");

    let output = client
        .execute_render(ArtifactRenderCommandRequest {
            cwd: temp.path().to_path_buf(),
            timeout: Some(Duration::from_secs(5)),
            env: BTreeMap::from([(
                "CODEX_TEST_OUTPUT".to_string(),
                output_path.display().to_string(),
            )]),
            target: ArtifactRenderTarget::Presentation(PresentationRenderTarget {
                input_path: temp.path().join("deck.pptx"),
                output_path: render_output.clone(),
                slide_number: 3,
            }),
        })
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    assert_success(&output);
    let command_log = fs::read_to_string(&output_path).unwrap_or_else(|error| panic!("{error}"));
    assert!(command_log.contains(&format!("arg0={}", runtime.render_cli_path().display())));
    assert!(command_log.contains("arg1=pptx"));
    assert!(command_log.contains("arg2=render"));
    assert!(command_log.contains("arg5=--slide"));
    assert!(command_log.contains("arg6=3"));
    assert!(command_log.contains("arg7=--out"));
    assert!(command_log.contains(&format!("arg8={}", render_output.display())));
}

#[test]
fn spreadsheet_render_target_to_args_includes_optional_range() {
    let target = ArtifactRenderTarget::Spreadsheet(SpreadsheetRenderTarget {
        input_path: PathBuf::from("/tmp/input.xlsx"),
        output_path: PathBuf::from("/tmp/output.png"),
        sheet_name: "Summary".to_string(),
        range: Some("A1:C8".to_string()),
    });

    assert_eq!(
        target.to_args(),
        vec![
            "xlsx".to_string(),
            "render".to_string(),
            "--in".to_string(),
            "/tmp/input.xlsx".to_string(),
            "--sheet".to_string(),
            "Summary".to_string(),
            "--out".to_string(),
            "/tmp/output.png".to_string(),
            "--range".to_string(),
            "A1:C8".to_string(),
        ]
    );
}

fn assert_success(output: &ArtifactCommandOutput) {
    assert!(output.success());
    assert_eq!(output.exit_code, Some(0));
}

#[cfg(unix)]
fn fake_installed_runtime(
    root: &Path,
    output_path: &Path,
    wrapped_script_path: &Path,
) -> InstalledArtifactRuntime {
    let runtime_root = root.join("runtime");
    write_installed_runtime(&runtime_root, "0.1.0", Some(PathBuf::from("node/bin/node")));
    write_fake_node_script(
        &runtime_root.join("node/bin/node"),
        output_path,
        wrapped_script_path,
    );
    InstalledArtifactRuntime::load(
        runtime_root,
        ArtifactRuntimePlatform::detect_current().unwrap_or_else(|error| panic!("{error}")),
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

fn write_installed_runtime(
    install_dir: &Path,
    runtime_version: &str,
    node_relative: Option<PathBuf>,
) {
    fs::create_dir_all(install_dir.join("node/bin")).unwrap_or_else(|error| panic!("{error}"));
    fs::create_dir_all(install_dir.join("artifact-tool/dist"))
        .unwrap_or_else(|error| panic!("{error}"));
    fs::create_dir_all(install_dir.join("granola-render/dist"))
        .unwrap_or_else(|error| panic!("{error}"));
    let node_relative = node_relative.unwrap_or_else(|| PathBuf::from("node/bin/node"));
    fs::write(
        install_dir.join("manifest.json"),
        serde_json::json!(sample_extracted_manifest(runtime_version, node_relative)).to_string(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    fs::write(install_dir.join("node/bin/node"), "#!/bin/sh\n")
        .unwrap_or_else(|error| panic!("{error}"));
    fs::write(
        install_dir.join("artifact-tool/dist/artifact_tool.mjs"),
        "export const ok = true;\n",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    fs::write(
        install_dir.join("granola-render/dist/render_cli.mjs"),
        "export const ok = true;\n",
    )
    .unwrap_or_else(|error| panic!("{error}"));
}

#[cfg(unix)]
fn write_fake_node_script(script_path: &Path, output_path: &Path, wrapped_script_path: &Path) {
    fs::write(
        script_path,
        format!(
            concat!(
                "#!/bin/sh\n",
                "printf 'arg0=%s\\n' \"$1\" > \"{}\"\n",
                "cp \"$1\" \"{}\"\n",
                "shift\n",
                "i=1\n",
                "for arg in \"$@\"; do\n",
                "  printf 'arg%s=%s\\n' \"$i\" \"$arg\" >> \"{}\"\n",
                "  i=$((i + 1))\n",
                "done\n",
                "printf 'CODEX_ARTIFACT_BUILD_ENTRYPOINT=%s\\n' \"$CODEX_ARTIFACT_BUILD_ENTRYPOINT\" >> \"{}\"\n",
                "printf 'CODEX_ARTIFACT_RENDER_ENTRYPOINT=%s\\n' \"$CODEX_ARTIFACT_RENDER_ENTRYPOINT\" >> \"{}\"\n",
                "printf 'CUSTOM_ENV=%s\\n' \"$CUSTOM_ENV\" >> \"{}\"\n",
                "echo stdout-ok\n",
                "echo stderr-ok >&2\n"
            ),
            output_path.display(),
            wrapped_script_path.display(),
            output_path.display(),
            output_path.display(),
            output_path.display(),
            output_path.display(),
        ),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(script_path)
            .unwrap_or_else(|error| panic!("{error}"))
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(script_path, permissions).unwrap_or_else(|error| panic!("{error}"));
    }
}

fn build_zip_archive(runtime_version: &str) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut bytes);
        let options = SimpleFileOptions::default();
        let manifest = serde_json::to_vec(&sample_extracted_manifest(
            runtime_version,
            PathBuf::from("node/bin/node"),
        ))
        .unwrap_or_else(|error| panic!("{error}"));
        zip.start_file("artifact-runtime/manifest.json", options)
            .unwrap_or_else(|error| panic!("{error}"));
        zip.write_all(&manifest)
            .unwrap_or_else(|error| panic!("{error}"));
        zip.start_file(
            "artifact-runtime/node/bin/node",
            options.unix_permissions(0o755),
        )
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
        zip.start_file(
            "artifact-runtime/granola-render/dist/render_cli.mjs",
            options,
        )
        .unwrap_or_else(|error| panic!("{error}"));
        zip.write_all(b"export const ok = true;\n")
            .unwrap_or_else(|error| panic!("{error}"));
        zip.finish().unwrap_or_else(|error| panic!("{error}"));
    }
    bytes.into_inner()
}

fn sample_extracted_manifest(
    runtime_version: &str,
    node_relative: PathBuf,
) -> ExtractedRuntimeManifest {
    ExtractedRuntimeManifest {
        schema_version: 1,
        runtime_version: runtime_version.to_string(),
        node: RuntimePathEntry {
            relative_path: node_relative.display().to_string(),
        },
        entrypoints: RuntimeEntrypoints {
            build_js: RuntimePathEntry {
                relative_path: "artifact-tool/dist/artifact_tool.mjs".to_string(),
            },
            render_cli: RuntimePathEntry {
                relative_path: "granola-render/dist/render_cli.mjs".to_string(),
            },
        },
    }
}
