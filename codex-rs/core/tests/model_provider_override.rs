use std::collections::HashMap;

use codex_core::ModelProviderInfo;
use codex_core::WireApi;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use tempfile::TempDir;

#[test]
fn user_defined_provider_overrides_builtin() {
    let tmp = TempDir::new().unwrap();

    let mut providers = HashMap::new();
    providers.insert(
        "oss".to_string(),
        ModelProviderInfo {
            name: "Custom".into(),
            base_url: Some("https://example.com/v1".into()),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
        },
    );
    let cfg = ConfigToml {
        model_provider: Some("oss".to_string()),
        model: Some("gpt-oss:20b".to_string()),
        model_providers: providers,
        ..Default::default()
    };

    let config = Config::load_from_base_config_with_overrides(
        cfg,
        ConfigOverrides::default(),
        tmp.path().to_path_buf(),
    )
    .unwrap();

    assert_eq!(config.model_provider.name, "Custom");
    assert_eq!(
        config.model_provider.base_url.as_deref(),
        Some("https://example.com/v1")
    );
}
