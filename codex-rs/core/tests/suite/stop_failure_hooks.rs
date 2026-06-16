use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_failed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;

const INITIAL_MODEL: &str = "gpt-5.2";
const FALLBACK_MODEL: &str = "gpt-5.4";

struct StopFailureRun {
    requests: Vec<ResponsesRequest>,
    hook_inputs: Vec<Value>,
}

fn write_stop_failure_hook(home: &Path, recovery: Value) -> Result<()> {
    let script_path = home.join("stop_failure_hook.py");
    let log_path = home.join("stop_failure_hook_log.jsonl");
    let output_json = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "StopFailure",
            "recovery": recovery,
        }
    })
    .to_string();
    let script = format!(
        r#"import json
from pathlib import Path
import sys

log_path = Path(r"{log_path}")
payload = json.load(sys.stdin)

with log_path.open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")

print({output_json:?})
"#,
        log_path = log_path.display(),
    );
    let hooks = serde_json::json!({
        "hooks": {
            "StopFailure": [{
                "matcher": "overloaded",
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                }]
            }]
        }
    });

    fs::write(&script_path, script).context("write StopFailure hook script")?;
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write hooks.json")?;
    Ok(())
}

fn read_hook_inputs(home: &Path) -> Result<Vec<Value>> {
    fs::read_to_string(home.join("stop_failure_hook_log.jsonl"))?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("parse StopFailure hook input"))
        .collect()
}

fn successful_response() -> String {
    sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-2", "recovered"),
        ev_completed("resp-2"),
    ])
}

async fn run_scenario(recovery: Value, second_response: String) -> Result<StopFailureRun> {
    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse_failed("resp-1", "server_is_overloaded", "capacity"),
            second_response,
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_model(INITIAL_MODEL)
        .with_pre_build_hook(move |home| {
            write_stop_failure_hook(home, recovery)
                .expect("failed to write StopFailure hook fixture");
        })
        .with_config(trust_discovered_hooks);
    let test = builder.build(&server).await?;

    test.submit_turn("recover from overload").await?;

    Ok(StopFailureRun {
        requests: responses.requests(),
        hook_inputs: read_hook_inputs(test.codex_home_path())?,
    })
}

#[tokio::test]
async fn retries_with_an_explicit_model() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let run = run_scenario(
        serde_json::json!({
            "action": "retry",
            "model": { "selector": "id", "id": FALLBACK_MODEL },
        }),
        successful_response(),
    )
    .await?;

    assert_eq!(run.requests.len(), 2);
    assert_eq!(run.requests[0].body_json()["model"], INITIAL_MODEL);
    assert_eq!(run.requests[1].body_json()["model"], FALLBACK_MODEL);
    assert!(run.requests[1].body_contains_text("<model_switch>"));
    assert_eq!(run.hook_inputs.len(), 1);
    let input = &run.hook_inputs[0];
    assert_eq!(input["hook_event_name"], "StopFailure");
    assert_eq!(input["error"], "overloaded");
    assert_eq!(input["model"], INITIAL_MODEL);
    assert_eq!(input["last_assistant_message"], Value::Null);
    assert!(
        input["error_details"]
            .as_str()
            .is_some_and(|details| details.contains("capacity"))
    );
    Ok(())
}

#[tokio::test]
async fn retries_with_the_catalog_default() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let run = run_scenario(
        serde_json::json!({
            "action": "retry",
            "model": { "selector": "catalog_default" },
        }),
        successful_response(),
    )
    .await?;

    let default_model = codex_core::test_support::all_model_presets()
        .iter()
        .find(|preset| preset.is_default)
        .expect("bundled models should include a default")
        .model
        .clone();
    assert_eq!(run.requests[1].body_json()["model"], default_model);
    Ok(())
}

#[tokio::test]
async fn runs_at_most_once_per_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let run = run_scenario(
        serde_json::json!({
            "action": "retry",
            "model": { "selector": "id", "id": FALLBACK_MODEL },
        }),
        sse_failed("resp-2", "server_is_overloaded", "recovery failure"),
    )
    .await?;

    assert_eq!(run.requests.len(), 2);
    assert_eq!(run.hook_inputs.len(), 1);
    Ok(())
}
