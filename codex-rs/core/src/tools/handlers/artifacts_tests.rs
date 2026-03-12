use super::*;
use codex_artifacts::RuntimeEntrypoints;
use codex_artifacts::RuntimePathEntry;
use tempfile::TempDir;

#[test]
fn parse_freeform_args_without_pragma() {
    let args = parse_freeform_args("console.log('ok');").expect("parse args");
    assert_eq!(args.source, "console.log('ok');");
    assert_eq!(args.timeout_ms, None);
}

#[test]
fn parse_freeform_args_with_pragma() {
    let args = parse_freeform_args("// codex-artifacts: timeout_ms=45000\nconsole.log('ok');")
        .expect("parse args");
    assert_eq!(args.source, "console.log('ok');");
    assert_eq!(args.timeout_ms, Some(45_000));
}

#[test]
fn parse_freeform_args_with_artifact_tool_pragma() {
    let args = parse_freeform_args("// codex-artifact-tool: timeout_ms=45000\nconsole.log('ok');")
        .expect("parse args");
    assert_eq!(args.source, "console.log('ok');");
    assert_eq!(args.timeout_ms, Some(45_000));
}

#[test]
fn parse_freeform_args_rejects_json_wrapped_code() {
    let err = parse_freeform_args("{\"code\":\"console.log('ok')\"}").expect_err("expected error");
    assert!(
        err.to_string()
            .contains("artifacts is a freeform tool and expects raw JavaScript source")
    );
}

#[test]
fn default_runtime_manager_uses_openai_codex_release_base() {
    let codex_home = TempDir::new().expect("create temp codex home");
    let manager = default_runtime_manager(codex_home.path().to_path_buf());

    assert_eq!(
        manager.config().release().base_url().as_str(),
        "https://github.com/openai/codex/releases/download/"
    );
    assert_eq!(
        manager.config().release().runtime_version(),
        PINNED_ARTIFACT_RUNTIME_VERSION
    );
}

#[test]
fn load_cached_runtime_reads_pinned_cache_path() {
    let codex_home = TempDir::new().expect("create temp codex home");
    let platform =
        codex_artifacts::ArtifactRuntimePlatform::detect_current().expect("detect platform");
    let install_dir = codex_home
        .path()
        .join("packages")
        .join("artifacts")
        .join(PINNED_ARTIFACT_RUNTIME_VERSION)
        .join(platform.as_str());
    std::fs::create_dir_all(&install_dir).expect("create install dir");
    std::fs::write(
        install_dir.join("manifest.json"),
        serde_json::json!({
            "schema_version": 1,
            "runtime_version": PINNED_ARTIFACT_RUNTIME_VERSION,
            "node": { "relative_path": "node/bin/node" },
            "entrypoints": {
                "build_js": { "relative_path": "artifact-tool/dist/artifact_tool.mjs" },
                "render_cli": { "relative_path": "granola-render/dist/render_cli.mjs" }
            }
        })
        .to_string(),
    )
    .expect("write manifest");
    std::fs::create_dir_all(install_dir.join("artifact-tool/dist"))
        .expect("create build entrypoint dir");
    std::fs::create_dir_all(install_dir.join("granola-render/dist"))
        .expect("create render entrypoint dir");
    std::fs::write(
        install_dir.join("artifact-tool/dist/artifact_tool.mjs"),
        "export const ok = true;\n",
    )
    .expect("write build entrypoint");
    std::fs::write(
        install_dir.join("granola-render/dist/render_cli.mjs"),
        "export const ok = true;\n",
    )
    .expect("write render entrypoint");

    let runtime = codex_artifacts::load_cached_runtime(
        &codex_home
            .path()
            .join(codex_artifacts::DEFAULT_CACHE_ROOT_RELATIVE),
        PINNED_ARTIFACT_RUNTIME_VERSION,
    )
    .expect("resolve runtime");
    assert_eq!(runtime.runtime_version(), PINNED_ARTIFACT_RUNTIME_VERSION);
    assert_eq!(
        runtime.manifest().entrypoints,
        RuntimeEntrypoints {
            build_js: RuntimePathEntry {
                relative_path: "artifact-tool/dist/artifact_tool.mjs".to_string(),
            },
            render_cli: RuntimePathEntry {
                relative_path: "granola-render/dist/render_cli.mjs".to_string(),
            },
        }
    );
}

#[test]
fn format_artifact_output_includes_success_message_when_silent() {
    let formatted = format_artifact_output(&ArtifactCommandOutput {
        exit_code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
    });
    assert!(formatted.contains("artifact JS completed successfully."));
}
