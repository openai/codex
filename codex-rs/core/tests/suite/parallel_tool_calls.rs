#![cfg(unix)]

use std::time::Instant;

use codex_core::built_in_model_providers;
use codex_core::protocol::EventMsg;
use codex_core::ConversationManager;
use codex_login::CodexAuth;
use core_test_support::load_sse_fixture_with_id_from_str;
use tempfile::TempDir;

fn build_parallel_exec_sse() -> String {
    let item1 = serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "local_shell_call",
            "call_id": "c1",
            "status": "in_progress",
            "action": {
                "type": "exec",
                "command": ["/bin/sh", "-c", "echo A"],
                "timeout_ms": 3000,
                "working_directory": null,
                "env": null,
                "user": null
            }
        }
    });
    let item2 = serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "local_shell_call",
            "call_id": "c2",
            "status": "in_progress",
            "action": {
                "type": "exec",
                "command": ["/bin/sh", "-c", "echo B"],
                "timeout_ms": 3000,
                "working_directory": null,
                "env": null,
                "user": null
            }
        }
    });
    let completed = serde_json::json!({
        "type": "response.completed",
        "response": { "id": "__ID__" }
    });

    let raw = serde_json::json!([
        item1, item2, completed
    ])
    .to_string();
    load_sse_fixture_with_id_from_str(&raw, "resp_parallel")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_shell_calls_begin_before_any_end_and_run_concurrently() {
    let tmp_sse = tempfile::NamedTempFile::new().expect("tmp sse");
    std::fs::write(tmp_sse.path(), build_parallel_exec_sse()).expect("write sse");
    std::env::set_var("CODEX_RS_SSE_FIXTURE", tmp_sse.path());

    let home = TempDir::new().unwrap();
    let mut config = core_test_support::load_default_config_for_test(&home);
    let providers = built_in_model_providers();
    config.model_provider = providers
        .get("openai")
        .expect("builtin provider")
        .clone();

    let cm = ConversationManager::with_auth(CodexAuth::from_api_key("test"));
    let conv = cm
        .new_conversation(config)
        .await
        .expect("spawn conversation")
        .conversation;

    conv
        .submit(codex_core::protocol::Op::UserInput {
            items: vec![codex_core::protocol::InputItem::Text {
                text: "go".into(),
            }],
        })
        .await
        .expect("submit");

    let mut begins = 0usize;
    let mut ends = 0usize;
    let mut seen_first_end = false;

    use tokio::time::{sleep, Duration as TokioDuration};
    let deadline = Instant::now() + TokioDuration::from_secs(5);

    while Instant::now() < deadline {
        let ev = conv.next_event().await.expect("event");
        match ev.msg {
            EventMsg::ExecCommandBegin(_) => {
                begins += 1;
            }
            EventMsg::ExecCommandEnd(_) => {
                if !seen_first_end {
                    assert_eq!(begins, 2, "expected both begins before first end");
                    seen_first_end = true;
                }
                ends += 1;
            }
            EventMsg::TaskComplete(_) => break,
            _ => {}
        }
        sleep(TokioDuration::from_millis(5)).await;
    }

    assert_eq!(begins, 2, "expected two parallel exec begins");
    assert_eq!(ends, 2, "expected two exec completions");
}
