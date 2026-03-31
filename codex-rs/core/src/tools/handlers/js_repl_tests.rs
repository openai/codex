use std::time::Duration;

use super::parse_freeform_args;
use crate::codex::make_session_and_context_with_rx;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandSource;
use pretty_assertions::assert_eq;

#[test]
fn parse_freeform_args_without_pragma() {
    let args = parse_freeform_args("console.log('ok');").expect("parse args");
    assert_eq!(args.code, "console.log('ok');");
    assert_eq!(args.timeout_ms, None);
}

#[test]
fn parse_freeform_args_with_pragma() {
    let input = "// codex-js-repl: timeout_ms=15000\nconsole.log('ok');";
    let args = parse_freeform_args(input).expect("parse args");
    assert_eq!(args.code, "console.log('ok');");
    assert_eq!(args.timeout_ms, Some(15_000));
}

#[test]
fn parse_freeform_args_rejects_unknown_key() {
    let err = parse_freeform_args("// codex-js-repl: nope=1\nconsole.log('ok');")
        .expect_err("expected error");
    assert_eq!(
        err.to_string(),
        "js_repl pragma only supports timeout_ms; got `nope`"
    );
}

#[test]
fn parse_freeform_args_rejects_reset_key() {
    let err = parse_freeform_args("// codex-js-repl: reset=true\nconsole.log('ok');")
        .expect_err("expected error");
    assert_eq!(
        err.to_string(),
        "js_repl pragma only supports timeout_ms; got `reset`"
    );
}

#[test]
fn parse_freeform_args_rejects_json_wrapped_code() {
    let err = parse_freeform_args(r#"{"code":"await doThing()"}"#).expect_err("expected error");
    assert_eq!(
        err.to_string(),
        "js_repl is a freeform tool and expects raw JavaScript source. Resend plain JS only (optional first line `// codex-js-repl: ...`); do not send JSON (`{\"code\":...}`), quoted code, or markdown fences."
    );
}

#[tokio::test]
async fn emit_js_repl_exec_end_sends_event() {
    let (session, turn, rx) = make_session_and_context_with_rx().await;
    super::emit_js_repl_exec_end(
        session.as_ref(),
        turn.as_ref(),
        "call-1",
        "hello",
        /*error*/ None,
        Duration::from_millis(12),
    )
    .await;

    let event = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = rx.recv().await.expect("event");
            if let EventMsg::ExecCommandEnd(end) = event.msg {
                break end;
            }
        }
    })
    .await
    .expect("timed out waiting for exec end");

    assert_eq!(event.call_id, "call-1");
    assert_eq!(event.turn_id, turn.sub_id);
    assert_eq!(event.command, vec!["js_repl".to_string()]);
    assert_eq!(event.cwd, turn.cwd.to_path_buf());
    assert_eq!(event.source, ExecCommandSource::Agent);
    assert_eq!(event.interaction_input, None);
    assert_eq!(event.stdout, "hello");
    assert_eq!(event.stderr, "");
    assert!(event.aggregated_output.contains("hello"));
    assert_eq!(event.exit_code, 0);
    assert_eq!(event.duration, Duration::from_millis(12));
    assert!(event.formatted_output.contains("hello"));
    assert!(!event.parsed_cmd.is_empty());
}
