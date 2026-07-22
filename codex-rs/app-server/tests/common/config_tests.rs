use super::*;
use tempfile::TempDir;

#[test]
fn mock_responses_config_composes_model_provider_features_and_extra_tables() {
    let home = TempDir::new().expect("temporary CODEX_HOME");
    MockResponsesConfig::new("http://127.0.0.1:1234")
        .with_model("custom-model")
        .with_model_provider("openai-custom")
        .with_provider_name("OpenAI")
        .with_provider_base_url("http://127.0.0.1:1234/api/codex")
        .with_approval_policy("on-request")
        .with_sandbox_mode("workspace-write")
        .enable_feature(Feature::Personality)
        .disable_feature(Feature::ShellSnapshot)
        .with_root_config("chatgpt_base_url = \"http://127.0.0.1:1234\"")
        .with_provider_config("requires_openai_auth = true")
        .with_extra_config("[extra]\nenabled = true")
        .write(home.path())
        .expect("write composable mock Responses config");

    let config =
        std::fs::read_to_string(home.path().join("config.toml")).expect("read config.toml");
    for expected in [
        "model = \"custom-model\"",
        "approval_policy = \"on-request\"",
        "sandbox_mode = \"workspace-write\"",
        "chatgpt_base_url = \"http://127.0.0.1:1234\"",
        "model_provider = \"openai-custom\"",
        "shell_snapshot = false",
        "personality = true",
        "[model_providers.openai-custom]\nname = \"OpenAI\"",
        "base_url = \"http://127.0.0.1:1234/api/codex\"",
        "requires_openai_auth = true",
        "[extra]\nenabled = true",
    ] {
        assert!(config.contains(expected), "config is missing {expected}");
    }
}

#[test]
fn legacy_mock_responses_writer_preserves_provider_auth_and_feature_overrides() {
    let home = TempDir::new().expect("temporary CODEX_HOME");
    write_mock_responses_config_toml(
        home.path(),
        "http://127.0.0.1:1234",
        &BTreeMap::from([
            (Feature::Personality, true),
            (Feature::ShellSnapshot, false),
        ]),
        /*auto_compact_limit*/ 321,
        Some(true),
        "openai",
        "compact this",
    )
    .expect("write legacy-compatible mock Responses config");

    let config =
        std::fs::read_to_string(home.path().join("config.toml")).expect("read config.toml");
    for expected in [
        "compact_prompt = \"compact this\"",
        "model_auto_compact_token_limit = 321",
        "openai_base_url = \"http://127.0.0.1:1234/v1\"",
        "shell_snapshot = false",
        "personality = true",
        "[model_providers.openai]\nname = \"OpenAI\"",
        "supports_websockets = false",
        "requires_openai_auth = true",
    ] {
        assert!(config.contains(expected), "config is missing {expected}");
    }
}
