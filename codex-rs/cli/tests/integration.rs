#![allow(clippy::unwrap_used)]

use codex_core::exec::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn write_config(dir: &Path, server: &MockServer) {
    fs::write(
        dir.join("config.toml"),
        format!(
            r#"model_provider = "mock"
model = "test-model"
[model_providers.mock]
name = "mock"
base_url = "{}/v1"
env_key = "PATH"
wire_api = "responses"
"#,
            server.uri()
        ),
    )
    .unwrap();
}

fn sse_message(text: &str) -> String {
    format!(
        "event: response.output_item.done\n\
data: {{\"type\":\"response.output_item.done\",\"item\":{{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{{\"type\":\"output_text\",\"text\":\"{text}\"}}]}}}}\n\n\
event: response.completed\n\
data: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"resp1\",\"output\":[]}}}}\n\n\n"
    )
}

fn sse_function_call() -> String {
    let call = serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "function_call",
            "name": "shell",
            "arguments": "{\"command\":[\"echo\",\"hi\"]}",
            "call_id": "call1"
        }
    });
    let completed = serde_json::json!({
        "type": "response.completed",
        "response": {"id": "resp1", "output": []}
    });
    format!(
        "event: response.output_item.done\ndata: {call}\n\n\
event: response.completed\ndata: {completed}\n\n\n"
    )
}

fn sse_final_after_call() -> String {
    let msg = serde_json::json!({
        "type": "response.output_item.done",
        "item": {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "done"}]}
    });
    let completed = serde_json::json!({
        "type": "response.completed",
        "response": {"id": "resp2", "output": []}
    });
    format!(
        "event: response.output_item.done\ndata: {msg}\n\n\
event: response.completed\ndata: {completed}\n\n\n"
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_conversation_turn_integration() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping test because network is disabled");
        return;
    }

    let server = MockServer::start().await;
    let resp = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_message("Hello, world."), "text/event-stream");
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(resp)
        .expect(1)
        .mount(&server)
        .await;

    unsafe {
        std::env::set_var("OPENAI_REQUEST_MAX_RETRIES", "0");
        std::env::set_var("OPENAI_STREAM_MAX_RETRIES", "0");
    }

    let home = TempDir::new().unwrap();
    let sandbox = TempDir::new().unwrap();
    write_config(home.path(), &server);

    let mut cmd = assert_cmd::Command::cargo_bin("codex").unwrap();
    cmd.env("CODEX_HOME", home.path());
    cmd.current_dir(sandbox.path());
    cmd.arg("exec").arg("--skip-git-repo-check").arg("Hello");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Hello, world."));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_invocation_flow() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!("Skipping test because network is disabled");
        return;
    }

    let server = MockServer::start().await;

    struct SeqResponder {
        count: std::sync::atomic::AtomicUsize,
    }

    impl wiremock::Respond for SeqResponder {
        fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
            use std::sync::atomic::Ordering;
            let n = self.count.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(sse_function_call(), "text/event-stream")
            } else {
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(sse_final_after_call(), "text/event-stream")
            }
        }
    }

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(SeqResponder {
            count: std::sync::atomic::AtomicUsize::new(0),
        })
        .expect(2)
        .mount(&server)
        .await;

    unsafe {
        std::env::set_var("OPENAI_REQUEST_MAX_RETRIES", "0");
        std::env::set_var("OPENAI_STREAM_MAX_RETRIES", "0");
    }

    let home = TempDir::new().unwrap();
    let sandbox = TempDir::new().unwrap();
    write_config(home.path(), &server);

    let mut cmd = assert_cmd::Command::cargo_bin("codex").unwrap();
    cmd.env("CODEX_HOME", home.path());
    cmd.current_dir(sandbox.path());
    cmd.arg("exec")
        .arg("--skip-git-repo-check")
        .arg("Run shell");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("exec echo hi"))
        .stdout(predicate::str::contains("hi"));
}
