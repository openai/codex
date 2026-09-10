//! Checks that tool response delivery preserves live results without rooting discarded values.

use std::collections::HashMap;
use std::sync::mpsc as std_mpsc;

use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::mpsc;

use super::super::RuntimeState;
use super::super::value::v8_value_to_json;
use super::resolve_tool_response;
use crate::v8_init::ensure_v8_initialized;

#[test]
fn discarded_tool_response_is_collectible_before_cell_ends() {
    ensure_v8_initialized().expect("initialize V8");
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    v8::scope!(let scope, isolate);
    let context = v8::Context::new(scope, Default::default());
    let scope = &mut v8::ContextScope::new(scope, context);
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let (runtime_command_tx, _runtime_command_rx) = std_mpsc::channel();
    scope.set_slot(RuntimeState {
        event_tx,
        pending_tool_calls: HashMap::new(),
        pending_timeouts: HashMap::new(),
        stored_values: HashMap::new(),
        stored_value_writes: HashMap::new(),
        enabled_tools: Vec::new(),
        next_tool_call_id: 1,
        next_timeout_id: 1,
        tool_call_id: "cell".to_string(),
        runtime_command_tx,
        exit_requested: false,
    });

    let promise = {
        v8::scope!(let scope, scope);
        let resolver = v8::PromiseResolver::new(scope).expect("create tool promise");
        let promise = resolver.get_promise(scope);
        let resolver = v8::Global::new(scope, resolver);
        scope
            .get_slot_mut::<RuntimeState>()
            .expect("runtime state")
            .pending_tool_calls
            .insert("tool".to_string(), resolver);
        v8::Global::new(scope, promise)
    };
    let response = json!({"content": [{"type": "text", "text": "tool result"}]});
    resolve_tool_response(scope, "tool", Ok(response.clone())).expect("resolve tool promise");
    scope.perform_microtask_checkpoint();
    scope.low_memory_notification();

    let weak_result = {
        v8::scope!(let scope, scope);
        let promise = v8::Local::new(scope, &promise);
        assert_eq!(promise.state(), v8::PromiseState::Fulfilled);
        let result = promise.result(scope);
        assert_eq!(v8_value_to_json(scope, result), Ok(Some(response)));
        v8::Weak::new(scope, result)
    };
    drop(promise);
    scope.low_memory_notification();

    assert!(weak_result.is_empty(), "discarded response is still rooted");
}
