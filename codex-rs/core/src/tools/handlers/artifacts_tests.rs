use super::*;
use crate::packages::versions;
use tempfile::TempDir;

#[test]
fn parse_freeform_args_without_pragma() {
    let args = parse_freeform_args("console.log('ok');").expect("parse args");
    assert_eq!(args.source, "console.log('ok');");
    assert_eq!(args.timeout_ms, None);
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
        versions::ARTIFACT_RUNTIME
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
        .join(versions::ARTIFACT_RUNTIME)
        .join(platform.as_str());
    std::fs::create_dir_all(&install_dir).expect("create install dir");
    std::fs::create_dir_all(install_dir.join("dist")).expect("create build entrypoint dir");
    std::fs::write(
        install_dir.join("package.json"),
        serde_json::json!({
            "name": "@oai/artifact-tool",
            "version": versions::ARTIFACT_RUNTIME,
            "type": "module",
            "exports": {
                ".": "./dist/artifact_tool.mjs"
            }
        })
        .to_string(),
    )
    .expect("write package json");
    std::fs::write(
        install_dir.join("dist/artifact_tool.mjs"),
        "export const ok = true;\n",
    )
    .expect("write build entrypoint");

    let runtime = codex_artifacts::load_cached_runtime(
        &codex_home
            .path()
            .join(codex_artifacts::DEFAULT_CACHE_ROOT_RELATIVE),
        versions::ARTIFACT_RUNTIME,
    )
    .expect("resolve runtime");
    assert_eq!(runtime.runtime_version(), versions::ARTIFACT_RUNTIME);
    assert_eq!(
        runtime.build_js_path(),
        install_dir.join("dist/artifact_tool.mjs")
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
