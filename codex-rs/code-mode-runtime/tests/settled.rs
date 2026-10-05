//! Exercises settlement ordering and callback behavior through V8 cell execution.

use std::sync::Arc;
use std::time::Duration;

use codex_code_mode_runtime::ExecuteRequest;
use codex_code_mode_runtime::ExecuteToPendingOutcome;
use codex_code_mode_runtime::FunctionCallOutputContentItem;
use codex_code_mode_runtime::InProcessCodeModeSession;
use codex_code_mode_runtime::NoopCodeModeSessionDelegate;
use codex_code_mode_runtime::RuntimeResponse;
use codex_code_mode_runtime::WaitOutcome;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

#[allow(clippy::expect_used)]
async fn assert_output(source: &str, expected: Value) {
    let session = InProcessCodeModeSession::new();
    let started = session
        .execute(
            ExecuteRequest {
                tool_call_id: "settled".to_string(),
                enabled_tools: Vec::new(),
                source: source.to_string(),
                yield_time_ms: Some(/*value*/ 5_000),
                max_output_tokens: None,
            },
            Arc::new(NoopCodeModeSessionDelegate),
            /*preempt*/ None,
        )
        .await
        .expect("start cell");
    let response = started.initial_response().await.expect("execute cell");
    let RuntimeResponse::Result {
        content_items,
        error_text: None,
        ..
    } = response
    else {
        panic!("unexpected cell response: {response:?}");
    };
    let actual: Vec<Value> = content_items
        .into_iter()
        .map(|item| {
            let FunctionCallOutputContentItem::InputText { text } = item else {
                panic!("unexpected output: {item:?}");
            };
            serde_json::from_str(&text).expect("JSON output")
        })
        .collect();
    assert_eq!(actual, vec![expected]);
}

#[tokio::test]
async fn preserves_settlement_order_before_and_during_iteration() {
    assert_output(
        r#"
const resolve = [];
const results = as_settled(Array.from({length: 4}, (_, index) =>
    new Promise(done => { resolve[index] = done; })));
resolve[2]("two");
await Promise.resolve();
resolve[0]("zero");
await Promise.resolve();
const seen = [];
for await (const result of results) {
    seen.push(result);
    if (seen.length === 1) {
        resolve[3]("three");
        resolve[1]("one");
        await Promise.resolve();
    }
}
text(seen);
"#,
        json!([
            {"index": 2, "status": "fulfilled", "value": "two"},
            {"index": 0, "status": "fulfilled", "value": "zero"},
            {"index": 3, "status": "fulfilled", "value": "three"},
            {"index": 1, "status": "fulfilled", "value": "one"},
        ]),
    )
    .await;
}

#[tokio::test]
async fn accepts_iterables_values_thenables_and_duplicate_promises() {
    assert_output(
        r#"
const shared = Promise.resolve("shared");
function* inputs() {
    yield 42;
    yield {then(resolve) { resolve("thenable"); }};
    yield shared;
    yield shared;
    yield Promise.reject({message: "failed"});
    yield {get then() { throw "getter failed"; }};
}
const seen = [];
for await (const result of as_settled(inputs())) seen.push(result);
text(seen.sort((a, b) => a.index - b.index));
"#,
        json!([
            {"index": 0, "status": "fulfilled", "value": 42},
            {"index": 1, "status": "fulfilled", "value": "thenable"},
            {"index": 2, "status": "fulfilled", "value": "shared"},
            {"index": 3, "status": "fulfilled", "value": "shared"},
            {"index": 4, "status": "rejected", "reason": {"message": "failed"}},
            {"index": 5, "status": "rejected", "reason": "getter failed"},
        ]),
    )
    .await;
}

#[tokio::test]
async fn preserves_map_keys_and_completes_empty_inputs() {
    assert_output(
        r#"
const seen = [];
await stream_settled([], result => seen.push(result));
for await (const result of as_settled(new Map())) seen.push(result);
await stream_settled(new Map([
    ["success", 7],
    ["failure", Promise.reject("failed")],
]), result => seen.push(result));
text(seen);
"#,
        json!([
            {"index": "success", "status": "fulfilled", "value": 7},
            {"index": "failure", "status": "rejected", "reason": "failed"},
        ]),
    )
    .await;
}

#[tokio::test]
async fn awaits_each_callback_including_rejected_inputs() {
    assert_output(
        r#"
const seen = [];
await stream_settled([Promise.reject("failed"), 2], async result => {
    seen.push(result);
    await new Promise(resolve => setTimeout(resolve, 1));
    seen.push(["callback finished", result.index]);
});
seen.push("stream finished");
text(seen);
"#,
        json!([
            {"index": 0, "status": "rejected", "reason": "failed"},
            ["callback finished", 0],
            {"index": 1, "status": "fulfilled", "value": 2},
            ["callback finished", 1],
            "stream finished",
        ]),
    )
    .await;
}

#[tokio::test]
async fn propagates_callback_errors_and_closes_early_iterations() {
    assert_output(
        r#"
const seen = [];
const failure = new Error("callback failed");
try {
    await stream_settled([1, 2], async result => {
        seen.push(result);
        await new Promise(resolve => setTimeout(resolve, 1));
        throw failure;
    });
} catch (error) {
    seen.push(error === failure);
}
const results = as_settled([3, new Promise(() => {}), Promise.reject("unconsumed")]);
for await (const result of results) {
    seen.push(result);
    break;
}
seen.push(await results.next());
text(seen);
"#,
        json!([
            {"index": 0, "status": "fulfilled", "value": 1},
            true,
            {"index": 0, "status": "fulfilled", "value": 3},
            {"done": true},
        ]),
    )
    .await;
}

#[tokio::test]
async fn preserves_identity_of_keys_values_and_rejections() {
    assert_output(
        r#"
const key = {};
const value = {method() { return 7; }};
value.self = value;
const failure = new Error("failed");
const seen = [];
await stream_settled(new Map([
    [key, Promise.resolve(value)],
    [failure, Promise.reject(failure)],
]), result => {
    seen.push(result.status === "fulfilled"
        ? result.index === key && result.value === value && result.value.self === value
            && result.value.method() === 7
        : result.index === failure && result.reason === failure);
});
text(seen);
"#,
        json!([true, true]),
    )
    .await;
}

#[tokio::test]
async fn terminates_helpers_waiting_for_unsettled_inputs() {
    let session = InProcessCodeModeSession::new();
    for source in [
        "for await (const result of as_settled([new Promise(() => {})])) text(result);",
        "await stream_settled([new Promise(() => {})], text);",
    ] {
        let response = session
            .execute_to_pending(
                ExecuteRequest {
                    tool_call_id: "settled".to_string(),
                    enabled_tools: Vec::new(),
                    source: source.to_string(),
                    yield_time_ms: None,
                    max_output_tokens: None,
                },
                Arc::new(NoopCodeModeSessionDelegate),
            )
            .await
            .expect("execute to pending");
        let ExecuteToPendingOutcome::Pending { cell_id, .. } = response else {
            panic!("expected pending helper: {response:?}");
        };
        assert_eq!(
            tokio::time::timeout(
                Duration::from_secs(/*secs*/ 5),
                session.terminate(cell_id.clone())
            )
            .await
            .expect("termination timed out")
            .expect("terminate cell"),
            WaitOutcome::LiveCell(RuntimeResponse::Terminated {
                cell_id,
                content_items: Vec::new(),
                code_mode_host_duration: None,
            }),
        );
    }
}
