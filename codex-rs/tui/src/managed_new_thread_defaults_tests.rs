use super::*;
use crate::legacy_core::config::ConfigBuilder;
use codex_config::ConfigLayerSource;
use codex_config::LoaderOverrides;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::openai_models::ReasoningEffort;
use codex_utils_absolute_path::test_support::PathBufExt;
use pretty_assertions::assert_eq;

async fn test_config() -> Config {
    let codex_home = tempfile::tempdir().expect("tempdir").keep();
    ConfigBuilder::default()
        .codex_home(codex_home)
        .build()
        .await
        .expect("config")
}

fn defaults() -> NewThreadModelDefaults {
    NewThreadModelDefaults {
        model: Some("managed-model".to_string()),
        model_reasoning_effort: Some(ReasoningEffort::High),
        service_tier: Some("fast".to_string()),
    }
}

#[tokio::test]
async fn applies_managed_defaults_to_a_new_thread_config() {
    let mut actual = test_config().await;
    actual.model = Some("configured-model".to_string());
    actual.model_reasoning_effort = Some(ReasoningEffort::Low);
    actual.service_tier = Some("flex".to_string());
    let mut expected = actual.clone();
    expected.model = Some("managed-model".to_string());
    expected.model_reasoning_effort = Some(ReasoningEffort::High);
    expected.service_tier = Some(ServiceTier::Fast.request_value().to_string());

    apply_managed_new_thread_defaults(
        &mut actual,
        Some(&defaults()),
        &[],
        &ConfigOverrides::default(),
    );

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn explicit_model_skips_managed_model_and_reasoning_effort() {
    let mut actual = test_config().await;
    actual.model = Some("explicit-model".to_string());
    actual.model_reasoning_effort = None;
    actual.service_tier = Some("flex".to_string());
    let mut expected = actual.clone();
    expected.service_tier = Some(ServiceTier::Fast.request_value().to_string());
    let harness_overrides = ConfigOverrides {
        model: Some("explicit-model".to_string()),
        ..ConfigOverrides::default()
    };

    apply_managed_new_thread_defaults(&mut actual, Some(&defaults()), &[], &harness_overrides);

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn explicit_reasoning_effort_skips_managed_model_and_reasoning_effort() {
    let mut actual = test_config().await;
    actual.model = Some("configured-model".to_string());
    actual.model_reasoning_effort = Some(ReasoningEffort::Low);
    actual.service_tier = Some("flex".to_string());
    let mut expected = actual.clone();
    expected.service_tier = Some(ServiceTier::Fast.request_value().to_string());
    let cli_kv_overrides = vec![(
        "model_reasoning_effort".to_string(),
        TomlValue::String("low".to_string()),
    )];

    apply_managed_new_thread_defaults(
        &mut actual,
        Some(&defaults()),
        &cli_kv_overrides,
        &ConfigOverrides::default(),
    );

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn explicit_launch_overrides_take_precedence() {
    let mut actual = test_config().await;
    actual.model = Some("explicit-model".to_string());
    actual.model_reasoning_effort = Some(ReasoningEffort::Low);
    actual.service_tier = Some("flex".to_string());
    let expected = actual.clone();
    let cli_kv_overrides = vec![(
        "model_reasoning_effort".to_string(),
        TomlValue::String("low".to_string()),
    )];
    let harness_overrides = ConfigOverrides {
        model: Some("explicit-model".to_string()),
        service_tier: Some(Some("flex".to_string())),
        ..ConfigOverrides::default()
    };

    apply_managed_new_thread_defaults(
        &mut actual,
        Some(&defaults()),
        &cli_kv_overrides,
        &harness_overrides,
    );

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn selected_custom_provider_and_service_tier_preserve_the_profile_model() {
    let home = tempfile::tempdir().expect("tempdir");
    std::fs::write(home.path().join("config.toml"), "model = \"base-model\"\n")
        .expect("base config");
    std::fs::write(
        home.path().join("custom.config.toml"),
        "model = \"custom-model\"\nmodel_provider = \"custom\"\nservice_tier = \"flex\"\n\
         [model_providers.custom]\nname = \"Custom\"\nbase_url = \"http://127.0.0.1:1/v1\"\nwire_api = \"responses\"\n",
    )
    .expect("profile");
    let mut actual = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides {
            user_config_path: Some(home.path().join("custom.config.toml").abs()),
            user_config_profile: Some("custom".parse().expect("profile name")),
            ..LoaderOverrides::without_managed_config_for_tests()
        })
        .build()
        .await
        .expect("config");
    let expected = actual.clone();
    assert_eq!(actual.model.as_deref(), Some("custom-model"));
    assert_eq!(actual.model_provider_id, "custom");

    apply_managed_new_thread_defaults(
        &mut actual,
        Some(&defaults()),
        &[],
        &ConfigOverrides::default(),
    );

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn managed_defaults_win_when_a_project_setting_shadows_the_selected_profile() {
    let home = tempfile::tempdir().expect("tempdir");
    let project = tempfile::tempdir().expect("project");
    std::fs::write(
        home.path().join("work.config.toml"),
        "model = \"profile-model\"\n",
    )
    .expect("profile");
    std::fs::create_dir(project.path().join(".codex")).expect("project config directory");
    std::fs::write(
        project.path().join(".codex/config.toml"),
        "model = \"project-model\"\n",
    )
    .expect("project config");
    crate::legacy_core::config::set_project_trust_level(
        home.path(),
        project.path(),
        TrustLevel::Trusted,
    )
    .expect("trusted project");
    let mut actual = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(LoaderOverrides {
            user_config_path: Some(home.path().join("work.config.toml").abs()),
            user_config_profile: Some("work".parse().expect("profile name")),
            ..LoaderOverrides::without_managed_config_for_tests()
        })
        .harness_overrides(ConfigOverrides {
            cwd: Some(project.path().to_path_buf()),
            ..ConfigOverrides::default()
        })
        .build()
        .await
        .expect("config");
    assert_eq!(actual.model.as_deref(), Some("project-model"));
    assert!(actual.config_layer_stack.layers_high_to_low().any(|layer| {
        matches!(
            layer.name,
            ConfigLayerSource::User {
                profile: Some(_),
                ..
            }
        ) && layer.config.get("model").is_some()
    }));
    let mut expected = actual.clone();
    expected.model = Some("managed-model".to_string());
    expected.model_reasoning_effort = Some(ReasoningEffort::High);
    expected.service_tier = Some(ServiceTier::Fast.request_value().to_string());

    apply_managed_new_thread_defaults(
        &mut actual,
        Some(&defaults()),
        &[],
        &ConfigOverrides::default(),
    );

    assert_eq!(actual, expected);
}
