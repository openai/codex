use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use std::time::Instant;

use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::protocol::ExecCommandSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventStage;
use crate::tools::handlers::parse_arguments;
use crate::tools::js_repl::JS_REPL_PRAGMA_PREFIX;
use crate::tools::js_repl::JS_REPL_TIMEOUT_ERROR_MESSAGE;
use crate::tools::js_repl::JsExecPollResult;
use crate::tools::js_repl::JsReplArgs;
use crate::tools::js_repl::emit_js_repl_exec_end;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;

pub struct JsReplHandler;
pub struct JsReplResetHandler;
pub struct JsReplPollHandler;
pub struct JsReplCancelHandler;

#[derive(Clone, Debug, Deserialize)]
struct JsReplPollArgs {
    exec_id: String,
    #[serde(default)]
    yield_time_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
struct JsReplCancelArgs {
    exec_id: String,
}

async fn emit_js_repl_exec_begin(
    session: &crate::codex::Session,
    turn: &crate::codex::TurnContext,
    call_id: &str,
) {
    let emitter = ToolEmitter::shell(
        vec!["js_repl".to_string()],
        turn.cwd.clone(),
        ExecCommandSource::Agent,
        false,
    );
    let ctx = ToolEventCtx::new(session, turn, call_id, None);
    emitter.emit(ctx, ToolEventStage::Begin).await;
}

fn is_js_repl_timeout_message(message: &str) -> bool {
    message == JS_REPL_TIMEOUT_ERROR_MESSAGE
}
#[async_trait]
impl ToolHandler for JsReplHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            payload,
            ToolPayload::Function { .. } | ToolPayload::Custom { .. }
        )
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            payload,
            call_id,
            ..
        } = invocation;

        if !session.features().enabled(Feature::JsRepl) {
            return Err(FunctionCallError::RespondToModel(
                "js_repl is disabled by feature flag".to_string(),
            ));
        }

        let args = match payload {
            ToolPayload::Function { arguments } => parse_arguments(&arguments)?,
            ToolPayload::Custom { input } => parse_freeform_args(&input)?,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "js_repl expects custom or function payload".to_string(),
                ));
            }
        };
        if args.poll && !session.features().enabled(Feature::JsReplPolling) {
            return Err(FunctionCallError::RespondToModel(
                "js_repl polling is disabled by feature flag".to_string(),
            ));
        }
        let manager = turn.js_repl.manager().await?;
        if args.poll {
            let started_at = Instant::now();
            emit_js_repl_exec_begin(session.as_ref(), turn.as_ref(), &call_id).await;
            let submission = Arc::clone(&manager)
                .submit(
                    Arc::clone(&session),
                    Arc::clone(&turn),
                    tracker,
                    call_id.clone(),
                    args,
                )
                .await;
            let submission = match submission {
                Ok(submission) => submission,
                Err(err) => {
                    let message = err.to_string();
                    emit_js_repl_exec_end(
                        session.as_ref(),
                        turn.as_ref(),
                        &call_id,
                        "",
                        Some(&message),
                        started_at.elapsed(),
                        false,
                    )
                    .await;
                    return Err(err);
                }
            };
            let content = serde_json::to_string(&serde_json::json!({
                "exec_id": submission.exec_id,
                "status": "running",
            }))
            .map_err(|err| {
                FunctionCallError::Fatal(format!(
                    "failed to serialize js_repl submission result: {err}"
                ))
            })?;
            return Ok(ToolOutput::Function {
                body: FunctionCallOutputBody::Text(content),
                success: Some(true),
            });
        }
        let started_at = Instant::now();
        emit_js_repl_exec_begin(session.as_ref(), turn.as_ref(), &call_id).await;
        let result = manager
            .execute(Arc::clone(&session), Arc::clone(&turn), tracker, args)
            .await;
        let result = match result {
            Ok(result) => result,
            Err(err) => {
                let message = err.to_string();
                let timed_out = is_js_repl_timeout_message(&message);
                emit_js_repl_exec_end(
                    session.as_ref(),
                    turn.as_ref(),
                    &call_id,
                    "",
                    Some(&message),
                    started_at.elapsed(),
                    timed_out,
                )
                .await;
                return Err(err);
            }
        };

        let content = result.output;
        let items = vec![FunctionCallOutputContentItem::InputText {
            text: content.clone(),
        }];

        emit_js_repl_exec_end(
            session.as_ref(),
            turn.as_ref(),
            &call_id,
            &content,
            None,
            started_at.elapsed(),
            false,
        )
        .await;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::ContentItems(items),
            success: Some(true),
        })
    }
}

#[async_trait]
impl ToolHandler for JsReplResetHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        if !invocation.session.features().enabled(Feature::JsRepl) {
            return Err(FunctionCallError::RespondToModel(
                "js_repl is disabled by feature flag".to_string(),
            ));
        }
        let manager = invocation.turn.js_repl.manager().await?;
        manager.reset().await?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text("js_repl kernel reset".to_string()),
            success: Some(true),
        })
    }
}

#[async_trait]
impl ToolHandler for JsReplPollHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            ..
        } = invocation;

        if !session.features().enabled(Feature::JsRepl) {
            return Err(FunctionCallError::RespondToModel(
                "js_repl is disabled by feature flag".to_string(),
            ));
        }
        if !session.features().enabled(Feature::JsReplPolling) {
            return Err(FunctionCallError::RespondToModel(
                "js_repl polling is disabled by feature flag".to_string(),
            ));
        }

        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(
                "js_repl_poll expects function payload".to_string(),
            ));
        };
        let args: JsReplPollArgs = parse_arguments(&arguments)?;
        let manager = turn.js_repl.manager().await?;
        let result = manager.poll(&args.exec_id, args.yield_time_ms).await?;
        let output = format_poll_output(&result)?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(output.content),
            success: Some(true),
        })
    }
}

#[async_trait]
impl ToolHandler for JsReplCancelHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            ..
        } = invocation;

        if !session.features().enabled(Feature::JsRepl) {
            return Err(FunctionCallError::RespondToModel(
                "js_repl is disabled by feature flag".to_string(),
            ));
        }
        if !session.features().enabled(Feature::JsReplPolling) {
            return Err(FunctionCallError::RespondToModel(
                "js_repl polling is disabled by feature flag".to_string(),
            ));
        }

        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(
                "js_repl_cancel expects function payload".to_string(),
            ));
        };
        let args: JsReplCancelArgs = parse_arguments(&arguments)?;
        let manager = turn.js_repl.manager().await?;
        manager.cancel(&args.exec_id).await?;

        let content = serde_json::to_string(&serde_json::json!({
            "exec_id": args.exec_id,
            "status": "canceled",
        }))
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize js_repl cancellation result: {err}"
            ))
        })?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

struct JsReplPollOutput {
    content: String,
}

fn format_poll_output(result: &JsExecPollResult) -> Result<JsReplPollOutput, FunctionCallError> {
    let status = if result.done {
        if result.error.is_some() {
            "error"
        } else {
            "completed"
        }
    } else {
        "running"
    };

    let logs = if result.logs.is_empty() {
        None
    } else {
        Some(result.logs.join("\n"))
    };
    let payload = serde_json::json!({
        "exec_id": result.exec_id,
        "status": status,
        "logs": logs,
        "output": result.output,
        "error": result.error,
    });
    let content = serde_json::to_string(&payload).map_err(|err| {
        FunctionCallError::Fatal(format!("failed to serialize js_repl poll result: {err}"))
    })?;

    Ok(JsReplPollOutput { content })
}

fn parse_freeform_args(input: &str) -> Result<JsReplArgs, FunctionCallError> {
    if input.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "js_repl expects raw JavaScript tool input (non-empty). Provide JS source text, optionally with first-line `// codex-js-repl: ...`."
                .to_string(),
        ));
    }

    let mut args = JsReplArgs {
        code: input.to_string(),
        timeout_ms: None,
        poll: false,
    };

    let mut lines = input.splitn(2, '\n');
    let first_line = lines.next().unwrap_or_default();
    let rest = lines.next().unwrap_or_default();
    let trimmed = first_line.trim_start();
    let Some(pragma) = trimmed.strip_prefix(JS_REPL_PRAGMA_PREFIX) else {
        reject_json_or_quoted_source(&args.code)?;
        return Ok(args);
    };

    let mut timeout_ms: Option<u64> = None;
    let mut poll: Option<bool> = None;
    let directive = pragma.trim();
    if !directive.is_empty() {
        for token in directive.split_whitespace() {
            let (key, value) = token.split_once('=').ok_or_else(|| {
                FunctionCallError::RespondToModel(format!(
                    "js_repl pragma expects space-separated key=value pairs (supported keys: timeout_ms, poll); got `{token}`"
                ))
            })?;
            match key {
                "timeout_ms" => {
                    if timeout_ms.is_some() {
                        return Err(FunctionCallError::RespondToModel(
                            "js_repl pragma specifies timeout_ms more than once".to_string(),
                        ));
                    }
                    let parsed = value.parse::<u64>().map_err(|_| {
                        FunctionCallError::RespondToModel(format!(
                            "js_repl pragma timeout_ms must be an integer; got `{value}`"
                        ))
                    })?;
                    timeout_ms = Some(parsed);
                }
                "poll" => {
                    if poll.is_some() {
                        return Err(FunctionCallError::RespondToModel(
                            "js_repl pragma specifies poll more than once".to_string(),
                        ));
                    }
                    let parsed = match value.to_ascii_lowercase().as_str() {
                        "true" => true,
                        "false" => false,
                        _ => {
                            return Err(FunctionCallError::RespondToModel(format!(
                                "js_repl pragma poll must be true or false; got `{value}`"
                            )));
                        }
                    };
                    poll = Some(parsed);
                }
                _ => {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "js_repl pragma only supports timeout_ms, poll; got `{key}`"
                    )));
                }
            }
        }
    }

    if rest.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "js_repl pragma must be followed by JavaScript source on subsequent lines".to_string(),
        ));
    }

    reject_json_or_quoted_source(rest)?;
    args.code = rest.to_string();
    args.timeout_ms = timeout_ms;
    args.poll = poll.unwrap_or(false);
    Ok(args)
}

fn reject_json_or_quoted_source(code: &str) -> Result<(), FunctionCallError> {
    let trimmed = code.trim();
    if trimmed.starts_with("```") {
        return Err(FunctionCallError::RespondToModel(
            "js_repl expects raw JavaScript source, not markdown code fences. Resend plain JS only (optional first line `// codex-js-repl: ...`)."
                .to_string(),
        ));
    }
    let Ok(value) = serde_json::from_str::<JsonValue>(trimmed) else {
        return Ok(());
    };
    match value {
        JsonValue::Object(_) | JsonValue::String(_) => Err(FunctionCallError::RespondToModel(
            "js_repl is a freeform tool and expects raw JavaScript source. Resend plain JS only (optional first line `// codex-js-repl: ...`); do not send JSON (`{\"code\":...}`), quoted code, or markdown fences."
                .to_string(),
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::format_poll_output;
    use super::is_js_repl_timeout_message;
    use super::parse_freeform_args;
    use crate::codex::make_session_and_context_with_rx;
    use crate::protocol::EventMsg;
    use crate::protocol::ExecCommandSource;
    use crate::tools::js_repl::JS_REPL_TIMEOUT_ERROR_MESSAGE;
    use crate::tools::js_repl::JsExecPollResult;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn parse_freeform_args_without_pragma() {
        let args = parse_freeform_args("console.log('ok');").expect("parse args");
        assert_eq!(args.code, "console.log('ok');");
        assert_eq!(args.timeout_ms, None);
        assert!(!args.poll);
    }

    #[test]
    fn parse_freeform_args_with_pragma() {
        let input = "// codex-js-repl: timeout_ms=15000\nconsole.log('ok');";
        let args = parse_freeform_args(input).expect("parse args");
        assert_eq!(args.code, "console.log('ok');");
        assert_eq!(args.timeout_ms, Some(15_000));
        assert!(!args.poll);
    }

    #[test]
    fn parse_freeform_args_with_poll() {
        let input = "// codex-js-repl: poll=true\nconsole.log('ok');";
        let args = parse_freeform_args(input).expect("parse args");
        assert_eq!(args.code, "console.log('ok');");
        assert_eq!(args.timeout_ms, None);
        assert!(args.poll);
    }

    #[test]
    fn parse_freeform_args_rejects_unknown_key() {
        let err = parse_freeform_args("// codex-js-repl: nope=1\nconsole.log('ok');")
            .expect_err("expected error");
        assert_eq!(
            err.to_string(),
            "js_repl pragma only supports timeout_ms, poll; got `nope`"
        );
    }

    #[test]
    fn parse_freeform_args_rejects_reset_key() {
        let err = parse_freeform_args("// codex-js-repl: reset=true\nconsole.log('ok');")
            .expect_err("expected error");
        assert_eq!(
            err.to_string(),
            "js_repl pragma only supports timeout_ms, poll; got `reset`"
        );
    }

    #[test]
    fn parse_freeform_args_rejects_duplicate_poll() {
        let err = parse_freeform_args("// codex-js-repl: poll=true poll=false\nconsole.log('ok');")
            .expect_err("expected error");
        assert_eq!(
            err.to_string(),
            "js_repl pragma specifies poll more than once"
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

    #[test]
    fn timeout_message_detection_matches_canonical_error() {
        assert!(is_js_repl_timeout_message(JS_REPL_TIMEOUT_ERROR_MESSAGE));
        assert!(!is_js_repl_timeout_message("some other error"));
    }

    #[test]
    fn format_poll_output_serializes_logs_in_json_payload() {
        let result = JsExecPollResult {
            exec_id: "exec-1".to_string(),
            logs: vec!["line 1".to_string(), "line 2".to_string()],
            output: None,
            error: None,
            done: false,
        };
        let output = format_poll_output(&result).expect("format poll output");
        let payload: serde_json::Value =
            serde_json::from_str(&output.content).expect("valid json payload");
        assert_eq!(
            payload,
            json!({
                "exec_id": "exec-1",
                "status": "running",
                "logs": "line 1\nline 2",
                "output": null,
                "error": null,
            })
        );
    }

    #[tokio::test]
    async fn emit_js_repl_exec_end_sends_event() {
        let (session, turn, rx) = make_session_and_context_with_rx().await;
        super::emit_js_repl_exec_begin(session.as_ref(), turn.as_ref(), "call-1").await;
        super::emit_js_repl_exec_end(
            session.as_ref(),
            turn.as_ref(),
            "call-1",
            "hello",
            None,
            Duration::from_millis(12),
            false,
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
        assert_eq!(event.cwd, turn.cwd);
        assert_eq!(event.source, ExecCommandSource::Agent);
        assert_eq!(event.interaction_input, None);
        assert_eq!(event.stdout, "hello");
        assert_eq!(event.stderr, "");
        assert!(event.aggregated_output.contains("hello"));
        assert_eq!(event.exit_code, 0);
        assert_eq!(event.duration, Duration::from_millis(12));
        assert!(event.formatted_output.contains("hello"));
        assert!(!event.formatted_output.contains("command timed out after"));
        assert!(!event.parsed_cmd.is_empty());
    }

    #[tokio::test]
    async fn emit_js_repl_exec_end_sends_timed_out_event() {
        let (session, turn, rx) = make_session_and_context_with_rx().await;
        super::emit_js_repl_exec_begin(session.as_ref(), turn.as_ref(), "call-timeout").await;
        super::emit_js_repl_exec_end(
            session.as_ref(),
            turn.as_ref(),
            "call-timeout",
            "",
            Some(JS_REPL_TIMEOUT_ERROR_MESSAGE),
            Duration::from_millis(50),
            true,
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

        assert_eq!(event.call_id, "call-timeout");
        assert!(
            event
                .formatted_output
                .contains("command timed out after 50 milliseconds")
        );
        assert!(!event.parsed_cmd.is_empty());
    }
}
