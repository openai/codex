#![cfg(not(target_os = "windows"))]

use anyhow::Ok;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_features::Feature;
use codex_protocol::protocol::DeprecationNoticeEvent;
use codex_protocol::protocol::EventMsg;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_deprecation_notice_for_legacy_feature_flag() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        let mut features = config.features.get().clone();
        features
            .record_legacy_usage_force("use_experimental_unified_exec_tool", Feature::UnifiedExec);
        config
            .features
            .set(features)
            .expect("test config should allow managed feature metadata updates");
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
        "`[features].use_experimental_unified_exec_tool` is deprecated. Use `[features].unified_exec` instead.".to_string(),
    );
    assert_eq!(
        details.as_deref(),
        Some(
            "Enable it with `--enable unified_exec` or `[features].unified_exec` in config.toml. See https://developers.openai.com/codex/config-basic#feature-flags for details."
        ),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_deprecation_notice_for_web_search_feature_flag_values() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    for enabled in [true, false] {
        let server = start_mock_server().await;

        let mut builder = test_codex().with_config(move |config| {
            let mut entries = BTreeMap::new();
            entries.insert("web_search_request".to_string(), enabled);
            let mut features = config.features.get().clone();
            features.apply_map(&entries);
            config
                .features
                .set(features)
                .expect("test config should allow managed feature map updates");
        });

        let TestCodex { codex, .. } = builder.build(&server).await?;

        let notice = wait_for_event_match(&codex, |event| match event {
            EventMsg::DeprecationNotice(ev)
                if ev.summary.contains("[features].web_search_request") =>
            {
                Some(ev.clone())
            }
            _ => None,
        })
        .await;

        let DeprecationNoticeEvent { summary, details } = notice;
        assert_eq!(
            summary,
            "`[features].web_search_request` is deprecated because web search is enabled by default."
                .to_string(),
        );
        assert_eq!(
            details.as_deref(),
            Some(
                "Set `web_search` to `\"live\"`, `\"indexed\"`, `\"cached\"`, or `\"disabled\"` at the top level (or under a profile) in config.toml if you want to override it."
            ),
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_deprecation_notice_for_use_legacy_landlock() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        let mut entries = BTreeMap::new();
        entries.insert("use_legacy_landlock".to_string(), true);
        let mut features = config.features.get().clone();
        features.apply_map(&entries);
        config
            .features
            .set(features)
            .expect("test config should allow managed feature map updates");
    });

    let TestCodex { codex, .. } = builder.build(&server).await?;

    let notice = wait_for_event_match(&codex, |event| match event {
        EventMsg::DeprecationNotice(ev)
            if ev.summary.contains("[features].use_legacy_landlock") =>
        {
            Some(ev.clone())
        }
        _ => None,
    })
    .await;

    let DeprecationNoticeEvent { summary, details } = notice;
    assert_eq!(
        summary,
        "`[features].use_legacy_landlock` is deprecated and will be removed soon.".to_string(),
    );
    assert_eq!(
        details.as_deref(),
        Some("Remove this setting to stop opting into the legacy Linux sandbox behavior."),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_zsh_config_starts_session_with_migration_notices() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                r#"[features]
shell_zsh_fork = true
unified_exec_zsh_fork = true
"#,
            )
            .expect("write legacy configuration");
        })
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                "[features]\nshell_zsh_fork = true\nunified_exec_zsh_fork = true\n",
            ),
        );
    let test = builder.build_with_auto_env(&server).await?;

    for feature in ["shell_zsh_fork", "unified_exec_zsh_fork"] {
        let expected = DeprecationNoticeEvent {
            summary: format!("`[features].{feature}` is deprecated and ignored."),
            details: Some(format!(
                "The patched zsh backend has been removed. Shell commands now use the standard shell executor. Remove `{feature}` from [features] in config.toml or profile configuration files."
            )),
        };
        let notice = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::DeprecationNotice(notice) if notice.summary == expected.summary => {
                Some(notice.clone())
            }
            _ => None,
        })
        .await;
        assert_eq!(
            (notice.summary, notice.details),
            (expected.summary, expected.details)
        );
    }
    let shell = match core_test_support::test_target_os() {
        core_test_support::TestTargetOs::Linux | core_test_support::TestTargetOs::MacOs => {
            "/bin/sh"
        }
        core_test_support::TestTargetOs::Windows => "powershell",
    };
    let arguments = serde_json::json!({
        "cmd": "echo migrated-shell",
        "shell": shell,
        "login": false,
        "yield_time_ms": 10_000,
    });
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call(
                    "shell-migration",
                    "exec_command",
                    &arguments.to_string(),
                ),
                responses::ev_completed("response-1"),
            ]),
            responses::sse(vec![responses::ev_completed("response-2")]),
        ],
    )
    .await;
    test.submit_turn("run a command with the configured shell")
        .await?;
    let output = mock
        .function_call_output_text("shell-migration")
        .expect("shell tool output");
    assert!(output.contains("Process exited with code 0\n"), "{output}");
    assert_eq!(
        output
            .split_once("Output:\n")
            .map(|(_, stdout)| stdout.trim()),
        Some("migrated-shell")
    );
    Ok(())
}
