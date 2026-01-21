#![cfg(not(target_os = "windows"))]

use anyhow::Ok;
use codex_app_server_protocol::ConfigLayerSource;
use codex_core::config_loader::ConfigLayerEntry;
use codex_core::config_loader::ConfigLayerStack;
use codex_core::config_loader::ConfigRequirements;
use codex_core::config_loader::ConfigRequirementsToml;
use codex_core::features::Feature;
use codex_core::protocol::DeprecationNoticeEvent;
use codex_core::protocol::EventMsg;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_absolute_path;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use toml::Value as TomlValue;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_deprecation_notice_for_legacy_feature_flag() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::UnifiedExec);
        config
            .features
            .record_legacy_usage_force("use_experimental_unified_exec_tool", Feature::UnifiedExec);
        config.use_experimental_unified_exec_tool = true;
    });

    let TestCodex { codex, .. } = builder.build(&server).await?;

    let notice = wait_for_event_match(&codex, |event| match event {
        EventMsg::DeprecationNotice(ev) => Some(ev.clone()),
        _ => None,
    })
    .await;

    let DeprecationNoticeEvent { summary, details } = notice;
    assert_eq!(
        summary,
        "`use_experimental_unified_exec_tool` is deprecated. Use `[features].unified_exec` instead."
            .to_string(),
    );
    assert_eq!(
        details.as_deref(),
        Some(
            "Enable it with `--enable unified_exec` or `[features].unified_exec` in config.toml. See https://github.com/openai/codex/blob/main/docs/config.md#feature-flags for details."
        ),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_deprecation_notice_for_experimental_instructions_file() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        let mut table = toml::map::Map::new();
        table.insert(
            "experimental_instructions_file".to_string(),
            TomlValue::String("legacy.md".to_string()),
        );
        let config_layer = ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: test_absolute_path("/tmp/config.toml"),
            },
            TomlValue::Table(table),
        );
        let config_layer_stack = ConfigLayerStack::new(
            vec![config_layer],
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .expect("build config layer stack");
        config.config_layer_stack = config_layer_stack;
    });

    let TestCodex { codex, .. } = builder.build(&server).await?;

    let notice = wait_for_event_match(&codex, |event| match event {
        EventMsg::DeprecationNotice(ev)
            if ev.summary.contains("experimental_instructions_file") =>
        {
            Some(ev.clone())
        }
        _ => None,
    })
    .await;

    let DeprecationNoticeEvent { summary, details } = notice;
    assert_eq!(
        summary,
        "`experimental_instructions_file` is deprecated and ignored. Use `model_instructions_file` instead."
            .to_string(),
    );
    assert_eq!(
        details.as_deref(),
        Some(
            "Move the setting to `model_instructions_file` in config.toml (or under a profile) to load instructions from a file."
        ),
    );

    Ok(())
}
