//! Checks array element kinds when a sort comparator mutates its receiver.

use codex_code_mode_runtime::ExecuteRequest;
use codex_code_mode_runtime::FunctionCallOutputContentItem;
use codex_code_mode_runtime::InProcessCodeModeSession;
use codex_code_mode_runtime::NoopCodeModeSessionDelegate;
use codex_code_mode_runtime::RuntimeResponse;
use pretty_assertions::assert_eq;
use std::sync::Arc;

#[tokio::test]
async fn array_sort_preserves_element_kinds_after_comparator_mutation() {
    // Native syntax is process-wide, so keep this in its own integration target.
    // Request Turbolev so initialization must also disable its Maglev frontend.
    v8::V8::set_flags_from_string("--allow-natives-syntax --turbolev");
    let service = InProcessCodeModeSession::new();
    let started = service
        .execute(
            ExecuteRequest {
                tool_call_id: "call_1".to_string(),
                enabled_tools: Vec::new(),
                source: r#"
function sortTopTier(values) {
    return values.sort(() => {
        values.fill(0);
        return 0;
    });
}
function sortMaglev(values) {
    return values.sort(() => {
        values.fill(0);
        return 0;
    });
}
function prepare(sort) {
    %PrepareFunctionForOptimization(sort);
    for (let i = 0; i < 100; ++i) {
        sort([1, 2]);
        sort([{}, {}]);
    }
}
function check(sort) {
    sort([1, 2]);
    const object = {};
    const values = [object, {}];
    sort(values);
    if (%HasSmiElements(values) && values[0] === object) {
        throw new Error("sort stored an object in an integer-elements array");
    }
}
prepare(sortTopTier);
%OptimizeFunctionOnNextCall(sortTopTier);
check(sortTopTier);
prepare(sortMaglev);
%OptimizeMaglevOnNextCall(sortMaglev);
check(sortMaglev);
text(JSON.stringify([3, 1, 2].sort((a, b) => a - b)));
"#
                .to_string(),
                yield_time_ms: None,
                max_output_tokens: None,
            },
            Arc::new(NoopCodeModeSessionDelegate),
            /*preempt*/ None,
        )
        .await
        .expect("start code-mode cell");
    let cell_id = started.cell_id.clone();
    let response = started
        .initial_response()
        .await
        .expect("execute code-mode cell");

    assert_eq!(
        response,
        RuntimeResponse::Result {
            code_mode_host_duration: None,
            cell_id,
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "[1,2,3]".to_string(),
            }],
            error_text: None,
        }
    );
}
