use super::*;
use crate::legacy_core::config::ConfigBuilder;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::ModelsRequirementsToml;
use codex_config::NewThreadModelDefaultsToml;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ReasoningEffort;
use insta::assert_debug_snapshot;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

async fn config_with_user_and_managed_defaults() -> Config {
    let codex_home = TempDir::new().expect("temporary Codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config");
    config.model = Some("user-model".to_string());
    config.model_reasoning_effort = Some(ReasoningEffort::Low);
    config.service_tier = Some("flex".to_string());
    config.config_layer_stack = ConfigLayerStack::new(
        Vec::new(),
        ConfigRequirements::default(),
        ConfigRequirementsToml {
            models: Some(ModelsRequirementsToml {
                new_thread: Some(NewThreadModelDefaultsToml {
                    model: Some("managed-model".to_string()),
                    model_reasoning_effort: Some(ReasoningEffort::Medium),
                    service_tier: Some("fast".to_string()),
                }),
            }),
            ..ConfigRequirementsToml::default()
        },
    )
    .expect("managed requirements stack");
    config
}

#[tokio::test]
async fn managed_defaults_override_persisted_user_defaults() {
    let config = config_with_user_and_managed_defaults().await;
    let mut expected = config.clone();
    expected.model = Some("managed-model".to_string());
    expected.model_reasoning_effort = Some(ReasoningEffort::Medium);
    expected.service_tier = Some(ServiceTier::Fast.request_value().to_string());

    let actual = config_with_managed_new_thread_defaults(config, &[], &ConfigOverrides::default());

    assert_eq!(actual, expected);
    assert_debug_snapshot!(
        "managed_new_thread_defaults",
        (
            actual.model,
            actual.model_reasoning_effort,
            actual.service_tier,
        )
    );
}

#[tokio::test]
async fn explicit_tui_launch_overrides_win_over_managed_defaults() {
    let mut config = config_with_user_and_managed_defaults().await;
    config.model = Some("cli-model".to_string());
    config.model_reasoning_effort = Some(ReasoningEffort::High);
    config.service_tier = Some("flex".to_string());
    let expected = config.clone();
    let cli_overrides = vec![(
        "model_reasoning_effort".to_string(),
        TomlValue::String("high".to_string()),
    )];
    let harness_overrides = ConfigOverrides {
        model: Some("cli-model".to_string()),
        service_tier: Some(Some("flex".to_string())),
        ..ConfigOverrides::default()
    };

    let actual =
        config_with_managed_new_thread_defaults(config, &cli_overrides, &harness_overrides);

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn explicit_config_flags_win_over_managed_defaults() {
    let config = config_with_user_and_managed_defaults().await;
    let expected = config.clone();
    let cli_overrides = vec![
        (
            "model".to_string(),
            TomlValue::String("user-model".to_string()),
        ),
        (
            "model_reasoning_effort".to_string(),
            TomlValue::String("low".to_string()),
        ),
        (
            "service_tier".to_string(),
            TomlValue::String("flex".to_string()),
        ),
    ];

    let actual = config_with_managed_new_thread_defaults(
        config,
        &cli_overrides,
        &ConfigOverrides::default(),
    );

    assert_eq!(actual, expected);
}
