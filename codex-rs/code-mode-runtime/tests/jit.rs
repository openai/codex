use codex_code_mode_runtime::ExecuteRequest;
use codex_code_mode_runtime::InProcessCodeModeSession;
use codex_code_mode_runtime::RuntimeResponse;
use codex_code_mode_runtime::V8JitMode;
use codex_code_mode_runtime::initialize_v8;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn code_mode_runs_with_jit_disabled() {
    initialize_v8(V8JitMode::Disabled).expect("initialize V8 without JIT");

    let service = InProcessCodeModeSession::new();
    let started = service
        .execute(ExecuteRequest {
            tool_call_id: "call_1".to_string(),
            enabled_tools: Vec::new(),
            source: "21 * 2;".to_string(),
            yield_time_ms: None,
            max_output_tokens: None,
        })
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
            cell_id,
            content_items: Vec::new(),
            error_text: None,
        }
    );
    assert_eq!(
        initialize_v8(V8JitMode::Enabled),
        Err("V8 was already initialized with JIT disabled".to_string())
    );
}
