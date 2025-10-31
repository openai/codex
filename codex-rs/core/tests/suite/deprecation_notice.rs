#![cfg(not(target_os = "windows"))]

use anyhow::Ok;
use codex_core::features::Feature;
use codex_core::protocol::DeprecationNoticeEvent;
use codex_core::protocol::EventMsg;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_deprecation_notice_for_legacy_feature_flag() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::StreamableShell);
        config.features.record_legacy_usage_force(
            "experimental_use_exec_command_tool",
            Feature::StreamableShell,
        );
        config.use_experimental_streamable_shell_tool = true;
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
        "`experimental_use_exec_command_tool` is deprecated. Use `streamable_shell` instead."
            .to_string(),
    );
    assert_eq!(
        details.as_deref(),
        Some(
            "You can either enable it using the CLI with `--enable streamable_shell` or through the config.toml file with `[features].streamable_shell`"
        ),
    );

    Ok(())
}
