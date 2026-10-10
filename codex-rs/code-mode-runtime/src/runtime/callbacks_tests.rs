//! Exercises the host guard separately from V8's interrupt mechanism. Simulate
//! V8 continuing to execute after exit was requested, including during getters.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc as std_mpsc;

use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::EnabledToolMetadata;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::mpsc;

use super::super::RuntimeEvent;
use super::super::RuntimeState;
use super::super::ToolCallbackMetadata;
use super::super::globals;
use crate::FunctionCallOutputContentItem;
use crate::v8_init::ensure_v8_initialized;

// Deliberately leave JS running: the test must verify host-side admission even
// when V8 has not processed its interrupt. Returning a value exercises coercion.
fn mark_exit_requested(
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments,
    mut retval: v8::ReturnValue<v8::Value>,
) {
    scope
        .get_slot_mut::<RuntimeState>()
        .expect("runtime state")
        .exit_requested = true;
    retval.set(args.get(0));
}

#[tokio::test]
async fn host_callbacks_admit_no_new_effects_after_exit_is_requested() {
    for (label, source) in [
        (
            "ordinary calls",
            r#"
markExit();
text("still working");
image("data:image/png;base64,AAA");
audio("data:audio/mpeg;base64,YXVkaW8=");
generatedImage({image_url: "data:image/png;base64,AAA", output_hint: "late"});
notify("late"); store("phase", "late");
tools.echo("late"); yield_control();
clearTimeout(earlyTimer); setTimeout(() => {}, 60000);
"#,
        ),
        (
            "text conversion",
            r#"text({toJSON() { return markExit("late"); }});"#,
        ),
        (
            "notify conversion",
            r#"notify({toJSON() { return markExit("late"); }});"#,
        ),
        (
            "store conversion",
            r#"store("phase", {toJSON() { return markExit("late"); }});"#,
        ),
        (
            "store key",
            r#"store({toString() { return markExit("phase"); }}, "late");"#,
        ),
        (
            "tool conversion",
            r#"tools.echo({toJSON() { return markExit("late"); }});"#,
        ),
        (
            "image getter",
            r#"image({get image_url() { return markExit("data:image/png;base64,AAA"); }});"#,
        ),
        (
            "audio getter",
            r#"audio({get audio_url() { return markExit("data:audio/mpeg;base64,YXVkaW8="); }});"#,
        ),
        (
            "generated image hint",
            r#"generatedImage({image_url: "data:image/png;base64,AAA", get output_hint() {return markExit("late"); }});"#,
        ),
        (
            "timer delay",
            r#"setTimeout(() => {}, {valueOf() {return markExit(60000); }});"#,
        ),
        (
            "clear timer id",
            r#"clearTimeout({valueOf() {return markExit(earlyTimer); }});"#,
        ),
    ] {
        ensure_v8_initialized().expect("initialize V8");
        let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
        v8::scope!(let scope, isolate);
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let (runtime_command_tx, _runtime_command_rx) = std_mpsc::channel();
        let tool_name = ToolName::plain("echo");
        globals::install_globals(
            scope,
            &[EnabledToolMetadata {
                global_name: "echo".to_string(),
                tool_name: tool_name.clone(),
                description: String::new(),
                kind: CodeModeToolKind::Function,
            }],
        )
        .expect("install globals");
        scope.set_slot(RuntimeState {
            event_tx,
            pending_tool_calls: HashMap::new(),
            pending_timeouts: HashMap::new(),
            stored_values: HashMap::new(),
            stored_value_writes: HashMap::new(),
            enabled_tools: vec![ToolCallbackMetadata {
                tool_name,
                kind: CodeModeToolKind::Function,
            }],
            next_tool_call_id: 1,
            next_timeout_id: 1,
            tool_call_id: "cell".to_string(),
            runtime_command_tx,
            exit_requested: false,
        });

        let marker = v8::Function::new(scope, mark_exit_requested).expect("exit marker");
        let key = v8::String::new(scope, "markExit").expect("marker name");
        context.global(scope).set(scope, key.into(), marker.into());
        let program = v8::String::new(
            scope,
            &format!(
                r#"
text("before"); store("phase", "before"); notify("before");
tools.echo("before");
const earlyTimer = setTimeout(() => text("early timer"), 60000);
{source}
"#
            ),
        )
        .expect("test program");
        let tc = std::pin::pin!(v8::TryCatch::new(scope));
        let tc = tc.init();
        let result =
            v8::Script::compile(&tc, program, /*origin*/ None).and_then(|script| script.run(&tc));
        assert!(
            result.is_some(),
            "{label}: {:?}",
            tc.exception().map(|e| e.to_rust_string_lossy(&tc))
        );

        assert!(
            matches!(event_rx.try_recv(), Ok(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText { text }
        )) if text == "before"),
            "{label}: pre-exit output"
        );
        assert!(
            matches!(event_rx.try_recv(), Ok(RuntimeEvent::Notify { text, .. }) if text == "before"),
            "{label}: pre-exit notification"
        );
        assert!(
            matches!(event_rx.try_recv(), Ok(RuntimeEvent::ToolCall { id, input, .. })
            if id == "tool-1" && input == Some(json!("before"))),
            "{label}: pre-exit tool"
        );
        assert!(
            matches!(event_rx.try_recv(), Err(mpsc::error::TryRecvError::Empty)),
            "{label}: emitted an effect after exit"
        );

        let state = tc.get_slot::<RuntimeState>().expect("runtime state");
        assert_eq!(
            state.stored_value_writes,
            HashMap::from([("phase".to_string(), Arc::new(json!("before")))]),
            "{label}: stored writes"
        );
        assert_eq!(
            state.stored_values, state.stored_value_writes,
            "{label}: cell storage"
        );
        assert_eq!(state.next_tool_call_id, 2, "{label}: allocated tool ID");
        assert_eq!(state.next_timeout_id, 2, "{label}: allocated timeout ID");
        assert!(
            state.pending_tool_calls.contains_key("tool-1"),
            "{label}: lost prior tool"
        );
        assert!(
            state.pending_timeouts.contains_key(&1),
            "{label}: lost prior timer"
        );
    }
}
