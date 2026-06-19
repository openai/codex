use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

#[test]
fn direct_calls_are_unioned_and_nested_calls_are_diagnostic() {
    let state = ToolTimingState {
        inner: Arc::new(Mutex::new(ToolTimingStateInner {
            origin: Instant::now(),
            next_entry_id: 3,
            next_report_id: 4,
            calls: vec![
                ToolTimingCallState {
                    entry_id: 0,
                    call_id: "direct-a".to_string(),
                    tool_name: "functions.exec".to_string(),
                    source: ToolTimingSource::Direct,
                    started_us: 0,
                    execution_started_us: Some(500_000),
                    completed_us: Some(2_000_000),
                },
                ToolTimingCallState {
                    entry_id: 1,
                    call_id: "nested".to_string(),
                    tool_name: "mcp__example__read".to_string(),
                    source: ToolTimingSource::CodeMode {
                        cell_id: "cell-1".to_string(),
                        runtime_tool_call_id: "runtime-call-1".to_string(),
                    },
                    started_us: 500_000,
                    execution_started_us: Some(1_000_000),
                    completed_us: Some(2_500_000),
                },
                ToolTimingCallState {
                    entry_id: 2,
                    call_id: "direct-b".to_string(),
                    tool_name: "functions.wait".to_string(),
                    source: ToolTimingSource::Direct,
                    started_us: 1_000_000,
                    execution_started_us: Some(1_250_000),
                    completed_us: Some(3_000_000),
                },
            ],
        })),
    };

    assert_eq!(
        serde_json::to_value(state.take_report()).expect("serialize report"),
        json!({
            "version": 1,
            "report_id": 4,
            "tool_active_s": 3.0,
            "calls": [
                {
                    "call_id": "direct-a",
                    "tool_name": "functions.exec",
                    "source": "direct",
                    "started_s": 0.0,
                    "execution_started_s": 0.5,
                    "completed_s": 2.0,
                    "dispatch_s": 0.5,
                    "handler_s": 1.5,
                    "total_s": 2.0,
                },
                {
                    "call_id": "nested",
                    "tool_name": "mcp__example__read",
                    "source": "code_mode",
                    "cell_id": "cell-1",
                    "runtime_tool_call_id": "runtime-call-1",
                    "started_s": 0.5,
                    "execution_started_s": 1.0,
                    "completed_s": 2.5,
                    "dispatch_s": 0.5,
                    "handler_s": 1.5,
                    "total_s": 2.0,
                },
                {
                    "call_id": "direct-b",
                    "tool_name": "functions.wait",
                    "source": "direct",
                    "started_s": 1.0,
                    "execution_started_s": 1.25,
                    "completed_s": 3.0,
                    "dispatch_s": 0.25,
                    "handler_s": 1.75,
                    "total_s": 2.0,
                },
            ],
        })
    );
}

#[test]
fn execution_start_after_completion_is_ignored() {
    let state = ToolTimingState::default();
    let guard = state.start_call(ToolTimingCall {
        call_id: "call-1".to_string(),
        tool_name: "functions.exec".to_string(),
        source: ToolTimingSource::Direct,
    });
    let marker = guard.marker();

    drop(guard);
    marker.mark_execution_started();

    let report = serde_json::to_value(state.take_report()).expect("serialize report");
    assert_eq!(report["calls"][0]["execution_started_s"], json!(null));
    assert_eq!(report["calls"][0]["handler_s"], json!(0.0));
}
