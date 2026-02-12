#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use codex_core::default_client::CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR;
use codex_core::models_manager::client_version_to_whole;
use codex_core::test_support::all_model_presets;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::default_input_modalities;
use core_test_support::responses;
use core_test_support::responses::ResponseMock;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use std::path::Path;
use wiremock::matchers::header;

/// Verify that when the server reports an error, `codex-exec` exits with a
/// non-zero status code so automation can detect failures.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_codex_exec_originator() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Hello, world!"),
        responses::ev_completed("response_1"),
    ]);
    responses::mount_sse_once_match(&server, header("Originator", "codex_exec"), body).await;

    test.cmd_with_server(&server)
        .env_remove(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR)
        .arg("--skip-git-repo-check")
        .arg("tell me something")
        .assert()
        .code(0);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supports_originator_override() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Hello, world!"),
        responses::ev_completed("response_1"),
    ]);
    responses::mount_sse_once_match(&server, header("Originator", "codex_exec_override"), body)
        .await;

    test.cmd_with_server(&server)
        .env("CODEX_INTERNAL_ORIGINATOR_OVERRIDE", "codex_exec_override")
        .arg("--skip-git-repo-check")
        .arg("tell me something")
        .assert()
        .code(0);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uses_codex_exec_scoped_cache_and_sends_cached_slug() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let cached_slug = "exec-cache-slug-e2e";
    write_models_cache_for_originator(test.home_path(), "codex_exec", cached_slug)?;

    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("response_1", "Hello, world!"),
        responses::ev_completed("response_1"),
    ]);
    let response_mock = responses::mount_sse_once(&server, body).await;

    test.cmd_with_server(&server)
        .env_remove(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR)
        .arg("--skip-git-repo-check")
        .arg("tell me something")
        .assert()
        .code(0);

    assert_response_model_slug(&response_mock, cached_slug);
    assert!(
        test.home_path()
            .join("models_cache")
            .join("codex_exec")
            .join("models_cache.json")
            .exists()
    );
    Ok(())
}

fn assert_response_model_slug(response_mock: &ResponseMock, expected_slug: &str) {
    let request = response_mock.single_request();
    let request_body = request.body_json();
    assert_eq!(request_body["model"].as_str(), Some(expected_slug));
}

fn write_models_cache_for_originator(
    codex_home: &Path,
    originator: &str,
    slug: &str,
) -> std::io::Result<()> {
    let Some(first_preset) = all_model_presets()
        .into_iter()
        .find(|preset| preset.show_in_picker)
    else {
        return Err(std::io::Error::other("no visible model presets"));
    };
    let mut model = preset_to_info(&first_preset, 0);
    model.slug = slug.to_string();
    let cache_path = codex_home
        .join("models_cache")
        .join(originator)
        .join("models_cache.json");
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let cache = serde_json::json!({
        "fetched_at": chrono::Utc::now(),
        "etag": null,
        "client_version": client_version_to_whole(),
        "models": [model]
    });
    std::fs::write(cache_path, serde_json::to_string_pretty(&cache)?)
}

fn preset_to_info(preset: &ModelPreset, priority: i32) -> ModelInfo {
    ModelInfo {
        slug: preset.id.clone(),
        display_name: preset.display_name.clone(),
        description: Some(preset.description.clone()),
        default_reasoning_level: Some(preset.default_reasoning_effort),
        supported_reasoning_levels: preset.supported_reasoning_efforts.clone(),
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: if preset.show_in_picker {
            ModelVisibility::List
        } else {
            ModelVisibility::Hide
        },
        supported_in_api: true,
        priority,
        upgrade: preset.upgrade.as_ref().map(|upgrade| upgrade.into()),
        base_instructions: "base instructions".to_string(),
        model_messages: None,
        supports_reasoning_summaries: false,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
        context_window: Some(272_000),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: default_input_modalities(),
        prefer_websockets: false,
    }
}
