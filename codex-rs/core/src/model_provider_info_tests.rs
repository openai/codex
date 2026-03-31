use super::*;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use pretty_assertions::assert_eq;
use std::num::NonZeroU64;
use tempfile::tempdir;

#[test]
fn test_deserialize_ollama_model_provider_toml() {
    let azure_provider_toml = r#"
name = "Ollama"
base_url = "http://localhost:11434/v1"
        "#;
    let expected_provider = ModelProviderInfo {
        name: "Ollama".into(),
        base_url: Some("http://localhost:11434/v1".into()),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_openai_auth: false,
        supports_websockets: false,
    };

    let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
    assert_eq!(expected_provider, provider);
}

#[test]
fn test_deserialize_azure_model_provider_toml() {
    let azure_provider_toml = r#"
name = "Azure"
base_url = "https://xxxxx.openai.azure.com/openai"
env_key = "AZURE_OPENAI_API_KEY"
query_params = { api-version = "2025-04-01-preview" }
        "#;
    let expected_provider = ModelProviderInfo {
        name: "Azure".into(),
        base_url: Some("https://xxxxx.openai.azure.com/openai".into()),
        env_key: Some("AZURE_OPENAI_API_KEY".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        wire_api: WireApi::Responses,
        query_params: Some(maplit::hashmap! {
            "api-version".to_string() => "2025-04-01-preview".to_string(),
        }),
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_openai_auth: false,
        supports_websockets: false,
    };

    let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
    assert_eq!(expected_provider, provider);
}

#[test]
fn test_deserialize_example_model_provider_toml() {
    let azure_provider_toml = r#"
name = "Example"
base_url = "https://example.com"
env_key = "API_KEY"
http_headers = { "X-Example-Header" = "example-value" }
env_http_headers = { "X-Example-Env-Header" = "EXAMPLE_ENV_VAR" }
        "#;
    let expected_provider = ModelProviderInfo {
        name: "Example".into(),
        base_url: Some("https://example.com".into()),
        env_key: Some("API_KEY".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: Some(maplit::hashmap! {
            "X-Example-Header".to_string() => "example-value".to_string(),
        }),
        env_http_headers: Some(maplit::hashmap! {
            "X-Example-Env-Header".to_string() => "EXAMPLE_ENV_VAR".to_string(),
        }),
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_openai_auth: false,
        supports_websockets: false,
    };

    let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
    assert_eq!(expected_provider, provider);
}

#[test]
fn test_deserialize_chat_wire_api_shows_helpful_error() {
    let provider_toml = r#"
name = "OpenAI using Chat Completions"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "chat"
        "#;

    let err = toml::from_str::<ModelProviderInfo>(provider_toml).unwrap_err();
    assert!(err.to_string().contains(CHAT_WIRE_API_REMOVED_ERROR));
}

#[test]
fn test_deserialize_websocket_connect_timeout() {
    let provider_toml = r#"
name = "OpenAI"
base_url = "https://api.openai.com/v1"
websocket_connect_timeout_ms = 15000
supports_websockets = true
        "#;

    let provider: ModelProviderInfo = toml::from_str(provider_toml).unwrap();
    assert_eq!(provider.websocket_connect_timeout_ms, Some(15_000));
}

#[test]
fn test_deserialize_provider_auth_config_defaults() {
    let base_dir = tempdir().unwrap();
    let provider_toml = r#"
name = "Corp"

[auth]
command = "./scripts/print-token"
args = ["--format=text"]
        "#;

    let provider: ModelProviderInfo = {
        let _guard = AbsolutePathBufGuard::new(base_dir.path());
        toml::from_str(provider_toml).unwrap()
    };

    assert_eq!(
        provider.auth,
        Some(ModelProviderAuthInfo {
            command: "./scripts/print-token".to_string(),
            args: vec!["--format=text".to_string()],
            timeout_ms: NonZeroU64::new(5_000).unwrap(),
            refresh_interval_ms: NonZeroU64::new(300_000).unwrap(),
            cwd: AbsolutePathBuf::resolve_path_against_base(".", base_dir.path()).unwrap(),
        })
    );
}

#[test]
fn openai_provider_without_base_url_emits_sentry_auth_failures() {
    let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);

    assert!(provider.should_emit_sentry_auth_failures());
}

#[test]
fn openai_provider_with_non_openai_base_url_skips_sentry_auth_failures() {
    let provider =
        ModelProviderInfo::create_openai_provider(Some("https://example.com/v1".to_string()));

    assert!(!provider.should_emit_sentry_auth_failures());
}

#[test]
fn openai_provider_with_openai_subdomain_base_url_emits_sentry_auth_failures() {
    let provider = ModelProviderInfo::create_openai_provider(Some(
        "https://api-staging.openai.com/v1".to_string(),
    ));

    assert!(provider.should_emit_sentry_auth_failures());
}

#[test]
fn openai_provider_with_chatgpt_subdomain_base_url_emits_sentry_auth_failures() {
    let provider =
        ModelProviderInfo::create_openai_provider(Some("https://ab.chatgpt.com/v1".to_string()));

    assert!(provider.should_emit_sentry_auth_failures());
}

#[test]
fn custom_openai_provider_with_env_key_emits_sentry_auth_failures() {
    let provider: ModelProviderInfo = toml::from_str(
        r#"
name = "OpenAI Staging"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "responses"
"#,
    )
    .expect("custom openai provider should deserialize");

    assert!(provider.should_emit_sentry_auth_failures());
}

#[test]
fn custom_openai_provider_with_command_auth_emits_sentry_auth_failures() {
    let provider: ModelProviderInfo = toml::from_str(
        r#"
name = "OpenAI Staging"
base_url = "https://api.openai.com/v1"
wire_api = "responses"

[auth]
command = "print-token"
"#,
    )
    .expect("custom openai provider should deserialize");

    assert!(provider.should_emit_sentry_auth_failures());
}

#[test]
fn custom_openai_provider_with_static_bearer_token_emits_sentry_auth_failures() {
    let provider: ModelProviderInfo = toml::from_str(
        r#"
name = "OpenAI Staging"
base_url = "https://api.openai.com/v1"
wire_api = "responses"
experimental_bearer_token = "token"
"#,
    )
    .expect("custom openai provider should deserialize");

    assert!(provider.should_emit_sentry_auth_failures());
}

#[test]
fn custom_openai_provider_with_http_authorization_header_emits_sentry_auth_failures() {
    let provider: ModelProviderInfo = toml::from_str(
        r#"
name = "OpenAI Staging"
base_url = "https://api.openai.com/v1"
wire_api = "responses"

[http_headers]
Authorization = "Bearer token"
"#,
    )
    .expect("custom openai provider should deserialize");

    assert!(provider.should_emit_sentry_auth_failures());
}

#[test]
fn custom_openai_provider_with_env_authorization_header_emits_sentry_auth_failures() {
    let provider: ModelProviderInfo = toml::from_str(
        r#"
name = "OpenAI Staging"
base_url = "https://api.openai.com/v1"
wire_api = "responses"

[env_http_headers]
authorization = "OPENAI_AUTH_HEADER"
"#,
    )
    .expect("custom openai provider should deserialize");

    assert!(provider.should_emit_sentry_auth_failures());
}

#[test]
fn renamed_openai_provider_still_emits_sentry_auth_failures() {
    let mut provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
    provider.name = "Staging".to_string();

    assert!(provider.should_emit_sentry_auth_failures());
}

#[test]
fn oss_provider_skips_sentry_auth_failures() {
    let provider =
        create_oss_provider_with_base_url("https://api.openai.com/v1", WireApi::Responses);

    assert!(!provider.should_emit_sentry_auth_failures());
}
