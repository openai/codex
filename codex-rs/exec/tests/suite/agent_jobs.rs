#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use core_test_support::test_codex_exec::test_codex_exec;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use wiremock::Mock;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

struct AgentJobsResponder {
    spawn_args_json: String,
    seen_main: AtomicBool,
    call_counter: AtomicUsize,
}

impl AgentJobsResponder {
    fn new(spawn_args_json: String) -> Self {
        Self {
            spawn_args_json,
            seen_main: AtomicBool::new(false),
            call_counter: AtomicUsize::new(0),
        }
    }
}

impl Respond for AgentJobsResponder {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let body_bytes = decode_body_bytes(request);
        let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);

        if has_function_call_output(&body) {
            return sse_response(sse(vec![
                ev_response_created("resp-tool"),
                ev_completed("resp-tool"),
            ]));
        }

        if let Some((job_id, item_id)) = extract_job_and_item(&body) {
            let call_id = format!(
                "call-worker-{}",
                self.call_counter.fetch_add(1, Ordering::SeqCst)
            );
            let args = json!({
                "job_id": job_id,
                "item_id": item_id,
                "result": { "item_id": item_id }
            });
            let args_json = serde_json::to_string(&args).unwrap_or_else(|err| {
                panic!("worker args serialize: {err}");
            });
            return sse_response(sse(vec![
                ev_response_created("resp-worker"),
                ev_function_call(&call_id, "report_agent_job_result", &args_json),
                ev_completed("resp-worker"),
            ]));
        }

        if !self.seen_main.swap(true, Ordering::SeqCst) {
            return sse_response(sse(vec![
                ev_response_created("resp-main"),
                ev_function_call("call-spawn", "spawn_agents_on_csv", &self.spawn_args_json),
                ev_completed("resp-main"),
            ]));
        }

        sse_response(sse(vec![
            ev_response_created("resp-default"),
            ev_completed("resp-default"),
        ]))
    }
}

fn decode_body_bytes(request: &wiremock::Request) -> Vec<u8> {
    request.body.clone()
}

fn has_function_call_output(body: &Value) -> bool {
    body.get("input")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("type").and_then(Value::as_str) == Some("function_call_output")
            })
        })
}

fn extract_job_and_item(body: &Value) -> Option<(String, String)> {
    let texts = message_input_texts(body);
    let mut combined = texts.join("\n");
    if let Some(instructions) = body.get("instructions").and_then(Value::as_str) {
        combined.push('\n');
        combined.push_str(instructions);
    }
    if !combined.contains("You are processing one item for a generic agent job.") {
        return None;
    }

    let mut job_id = None;
    let mut item_id = None;
    for line in combined.lines() {
        if let Some(value) = line.strip_prefix("Job ID: ") {
            job_id = Some(value.trim().to_string());
        }
        if let Some(value) = line.strip_prefix("Item ID: ") {
            item_id = Some(value.trim().to_string());
        }
    }

    Some((job_id?, item_id?))
}

fn message_input_texts(body: &Value) -> Vec<String> {
    let Some(items) = body.get("input").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter(|span| span.get("type").and_then(Value::as_str) == Some("input_text"))
        .filter_map(|span| span.get("text").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn sse(events: Vec<serde_json::Value>) -> String {
    let mut body = String::new();
    for event in events {
        body.push_str("data: ");
        body.push_str(&event.to_string());
        body.push_str("\n\n");
    }
    body.push_str("data: [DONE]\n\n");
    body
}

fn sse_response(body: String) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_string(body)
}

fn ev_response_created(response_id: &str) -> serde_json::Value {
    json!({
        "type": "response.created",
        "response": {
            "id": response_id,
            "model": "gpt-5",
            "output": []
        }
    })
}

fn ev_function_call(call_id: &str, name: &str, arguments: &str) -> serde_json::Value {
    json!({
        "type": "response.output_item.done",
        "output_index": 0,
        "item": {
            "type": "function_call",
            "id": format!("item-{call_id}"),
            "call_id": call_id,
            "name": name,
            "arguments": arguments,
            "status": "completed"
        }
    })
}

fn ev_completed(response_id: &str) -> serde_json::Value {
    json!({
        "type": "response.completed",
        "response": {
            "id": response_id,
            "usage": {
                "input_tokens": 1,
                "input_tokens_details": {"cached_tokens": 0},
                "output_tokens": 1,
                "output_tokens_details": {"reasoning_tokens": 0},
                "total_tokens": 2
            }
        }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_spawn_agents_on_csv_exits_after_mock_job_completion() -> Result<()> {
    let test = test_codex_exec();
    let server = wiremock::MockServer::start().await;

    let input_path = test.cwd_path().join("agent_jobs_input.csv");
    let output_path = test.cwd_path().join("agent_jobs_output.csv");
    let mut csv = String::from("name\n");
    for index in 1..=100 {
        csv.push_str(&format!("cat_{index}\n"));
    }
    fs::write(&input_path, csv)?;

    let args = json!({
        "csv_path": input_path.display().to_string(),
        "instruction": "Write a playful 2-line poem about the cat named {name}. Return a JSON object with keys name and poem. Call report_agent_job_result exactly once and then stop.",
        "output_csv_path": output_path.display().to_string(),
        "max_concurrency": 64,
    });
    let args_json = serde_json::to_string(&args)?;

    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(AgentJobsResponder::new(args_json))
        .mount(&server)
        .await;

    let mut cmd = test.cmd_with_server(&server);
    cmd.timeout(Duration::from_secs(60));
    cmd.arg("-c")
        .arg("features.enable_fanout=true")
        .arg("-c")
        .arg("agents.max_threads=64")
        .arg("--skip-git-repo-check")
        .arg("Use spawn_agents_on_csv on the provided CSV and do not do work yourself.")
        .assert()
        .success();

    let output = fs::read_to_string(&output_path)?;
    assert_eq!(output.lines().count(), 101);

    Ok(())
}
