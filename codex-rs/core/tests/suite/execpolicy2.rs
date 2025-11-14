#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::features::Feature;
use codex_core::protocol::EventMsg;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::json;
use std::fs;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execpolicy2_blocks_shell_invocation() -> Result<()> {
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::ExecPolicyV2);
        let policy_dir = config.cwd.join(".codex");
        fs::create_dir_all(&policy_dir).expect("create .codex directory");
        let policy_path = policy_dir.join("policy.codexpolicy");
        fs::write(
            &policy_path,
            r#"prefix_rule(pattern=["echo"], decision="forbidden")"#,
        )
        .expect("write policy file");
    });
    let server = start_mock_server().await;
    let test = builder.build(&server).await?;

    let call_id = "shell-forbidden";
    let args = json!({
        "command": ["echo", "blocked"],
        "timeout_ms": 1_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("run shell command").await?;

    let EventMsg::ExecCommandEnd(end) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ExecCommandEnd(_))
    })
    .await
    else {
        unreachable!()
    };

    assert_eq!(end.exit_code, -1);
    assert!(
        end.aggregated_output
            .contains("execpolicy forbids this command"),
        "unexpected output: {}",
        end.aggregated_output
    );

    Ok(())
}
