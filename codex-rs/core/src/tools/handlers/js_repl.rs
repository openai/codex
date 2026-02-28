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
use crate::tools::js_repl::JS_REPL_POLL_TIMEOUT_ARG_ERROR_MESSAGE;
use crate::tools::js_repl::JS_REPL_PRAGMA_PREFIX;
use crate::tools::js_repl::JsExecPollResult;
use crate::tools::js_repl::JsReplArgs;
use crate::tools::js_repl::JsReplExecuteError;
use crate::tools::js_repl::emit_js_repl_exec_end;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;

pub struct JsReplHandler;
pub struct JsReplResetHandler;
pub struct JsReplPollHandler;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsReplPollArgs {
    exec_id: String,
    #[serde(default)]
    yield_time_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsReplResetArgs {
    #[serde(default)]
    session_id: Option<String>,
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
            ToolPayload::Custom { input } => {
                parse_freeform_args(&input, session.features().enabled(Feature::JsReplPolling))?
            }
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "js_repl expects custom or function payload".to_string(),
                ));
            }
        };
        if args.poll {
            if args
                .session_id
                .as_deref()
                .is_some_and(|session_id| session_id.trim().is_empty())
            {
                return Err(FunctionCallError::RespondToModel(
                    "js_repl session_id must not be empty".to_string(),
                ));
            }
            if args.timeout_ms.is_some() {
                return Err(FunctionCallError::RespondToModel(
                    JS_REPL_POLL_TIMEOUT_ARG_ERROR_MESSAGE.to_string(),
                ));
            }
            if !session.features().enabled(Feature::JsReplPolling) {
                return Err(FunctionCallError::RespondToModel(
                    "js_repl polling is disabled by feature flag".to_string(),
                ));
            }
        } else if args.session_id.is_some() {
            return Err(FunctionCallError::RespondToModel(
                "js_repl session_id is only supported when poll=true".to_string(),
            ));
        }
        let manager = turn.js_repl.manager().await?;
        let started_at = Instant::now();
        emit_js_repl_exec_begin(session.as_ref(), turn.as_ref(), &call_id).await;

        if args.poll {
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
                "session_id": submission.session_id,
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
        let result = manager
            .execute(Arc::clone(&session), Arc::clone(&turn), tracker, args)
            .await;
        let result = match result {
            Ok(result) => result,
            Err(err) => {
                let timed_out = matches!(err, JsReplExecuteError::TimedOut);
                let message = err.to_string();
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
                return Err(err.into());
            }
        };

        let content = result.output;
        let mut items = Vec::with_capacity(result.content_items.len() + 1);
        if !content.is_empty() {
            items.push(FunctionCallOutputContentItem::InputText {
                text: content.clone(),
            });
        }
        items.extend(result.content_items);

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
            body: if items.is_empty() {
                FunctionCallOutputBody::Text(content)
            } else {
                FunctionCallOutputBody::ContentItems(items)
            },
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
        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(
                "js_repl_reset expects function payload".to_string(),
            ));
        };
        let args: JsReplResetArgs = parse_arguments(&arguments)?;
        let manager = turn.js_repl.manager().await?;
        let content = if let Some(session_id) = args.session_id {
            if session_id.trim().is_empty() {
                return Err(FunctionCallError::RespondToModel(
                    "js_repl session_id must not be empty".to_string(),
                ));
            }
            manager.reset_session(&session_id).await?;
            serde_json::to_string(&serde_json::json!({
                "status": "reset",
                "session_id": session_id,
            }))
            .map_err(|err| {
                FunctionCallError::Fatal(format!("failed to serialize js_repl reset result: {err}"))
            })?
        } else {
            manager.reset().await?;
            serde_json::to_string(&serde_json::json!({
                "status": "reset_all",
            }))
            .map_err(|err| {
                FunctionCallError::Fatal(format!("failed to serialize js_repl reset result: {err}"))
            })?
        };
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
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
        let result = manager
            .poll(
                &args.exec_id,
                args.yield_time_ms.map(|yield_time_ms| {
                    yield_time_ms.max(crate::unified_exec::MIN_EMPTY_YIELD_TIME_MS)
                }),
            )
            .await?;
        let output = format_poll_output(&result)?;
        Ok(ToolOutput::Function {
            body: output.body,
            success: Some(true),
        })
    }
}

struct JsReplPollOutput {
    body: FunctionCallOutputBody,
}

#[derive(Default)]
struct ParsedJsReplPragma {
    timeout_ms: Option<u64>,
    poll: Option<bool>,
    session_id: Option<String>,
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
    let output = result.output.as_deref().filter(|output| !output.is_empty());
    let payload = serde_json::json!({
        "exec_id": result.exec_id,
        "session_id": result.session_id,
        "status": status,
        "logs": logs,
        "output": output,
        "error": result.error,
    });
    let content = serde_json::to_string(&payload).map_err(|err| {
        FunctionCallError::Fatal(format!("failed to serialize js_repl poll result: {err}"))
    })?;

    let body = if result.content_items.is_empty() {
        FunctionCallOutputBody::Text(content)
    } else {
        let mut items = Vec::with_capacity(result.content_items.len() + 1);
        items.push(FunctionCallOutputContentItem::InputText { text: content });
        items.extend(result.content_items.clone());
        FunctionCallOutputBody::ContentItems(items)
    };

    Ok(JsReplPollOutput { body })
}

fn parse_freeform_args(
    input: &str,
    polling_enabled: bool,
) -> Result<JsReplArgs, FunctionCallError> {
    if input.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "js_repl expects raw JavaScript tool input (non-empty). Provide JS source text, optionally with first-line `// codex-js-repl: ...`."
                .to_string(),
        ));
    }

    let mut lines = input.splitn(2, '\n');
    let first_line = lines.next().unwrap_or_default();
    let rest = lines.next().unwrap_or_default();
    let trimmed = first_line.trim_start();
    let Some(pragma) = trimmed.strip_prefix(JS_REPL_PRAGMA_PREFIX) else {
        reject_json_or_quoted_source(input)?;
        return Ok(JsReplArgs {
            code: input.to_string(),
            timeout_ms: None,
            poll: false,
            session_id: None,
        });
    };

    if rest.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "js_repl pragma must be followed by JavaScript source on subsequent lines".to_string(),
        ));
    }

    let pragma_args = parse_js_repl_pragma(pragma.trim(), polling_enabled)?;
    reject_json_or_quoted_source(rest)?;
    let args = JsReplArgs {
        code: rest.to_string(),
        timeout_ms: pragma_args.timeout_ms,
        poll: pragma_args.poll.unwrap_or(false),
        session_id: pragma_args.session_id,
    };
    if args.session_id.is_some() && !args.poll {
        return Err(FunctionCallError::RespondToModel(
            "js_repl session_id is only supported when poll=true".to_string(),
        ));
    }
    if args.poll && args.timeout_ms.is_some() {
        return Err(FunctionCallError::RespondToModel(
            JS_REPL_POLL_TIMEOUT_ARG_ERROR_MESSAGE.to_string(),
        ));
    }
    Ok(args)
}

fn parse_js_repl_pragma(
    directive: &str,
    polling_enabled: bool,
) -> Result<ParsedJsReplPragma, FunctionCallError> {
    let mut args = ParsedJsReplPragma::default();
    let supported_keys = if polling_enabled {
        "timeout_ms, poll, session_id"
    } else {
        "timeout_ms"
    };

    for token in directive
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|token| !token.is_empty())
    {
        let (key, value) = token.split_once('=').ok_or_else(|| {
            FunctionCallError::RespondToModel(format!(
                "js_repl pragma expects space-separated key=value pairs (supported keys: {supported_keys}); got `{token}`"
            ))
        })?;
        match key {
            "timeout_ms" => {
                if args.timeout_ms.is_some() {
                    return Err(FunctionCallError::RespondToModel(
                        "js_repl pragma specifies timeout_ms more than once".to_string(),
                    ));
                }
                let parsed = value.parse::<u64>().map_err(|_| {
                    FunctionCallError::RespondToModel(format!(
                        "js_repl pragma timeout_ms must be an integer; got `{value}`"
                    ))
                })?;
                args.timeout_ms = Some(parsed);
            }
            "poll" => {
                if args.poll.is_some() {
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
                args.poll = Some(parsed);
            }
            "session_id" => {
                if args.session_id.is_some() {
                    return Err(FunctionCallError::RespondToModel(
                        "js_repl pragma specifies session_id more than once".to_string(),
                    ));
                }
                if value.trim().is_empty() {
                    return Err(FunctionCallError::RespondToModel(
                        "js_repl session_id must not be empty".to_string(),
                    ));
                }
                args.session_id = Some(value.to_string());
            }
            _ => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "js_repl pragma only supports {supported_keys}; got `{key}`"
                )));
            }
        }
    }

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
    use super::parse_freeform_args;
    use crate::codex::make_session_and_context_with_rx;
    use crate::protocol::EventMsg;
    use crate::protocol::ExecCommandSource;
    use crate::tools::js_repl::JS_REPL_POLL_TIMEOUT_ARG_ERROR_MESSAGE;
    use crate::tools::js_repl::JS_REPL_TIMEOUT_ERROR_MESSAGE;
    use crate::tools::js_repl::JsExecPollResult;
    use codex_protocol::models::FunctionCallOutputBody;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn parse_freeform_args_without_pragma() {
        let args = parse_freeform_args("console.log('ok');", true).expect("parse args");
        assert_eq!(args.code, "console.log('ok');");
        assert_eq!(args.timeout_ms, None);
        assert!(!args.poll);
        assert_eq!(args.session_id, None);
    }

    #[test]
    fn parse_freeform_args_with_pragma() {
        let input = "// codex-js-repl: timeout_ms=15000\nconsole.log('ok');";
        let args = parse_freeform_args(input, true).expect("parse args");
        assert_eq!(args.code, "console.log('ok');");
        assert_eq!(args.timeout_ms, Some(15_000));
        assert!(!args.poll);
        assert_eq!(args.session_id, None);
    }

    #[test]
    fn parse_freeform_args_with_poll() {
        let input = "// codex-js-repl: poll=true\nconsole.log('ok');";
        let args = parse_freeform_args(input, true).expect("parse args");
        assert_eq!(args.code, "console.log('ok');");
        assert_eq!(args.timeout_ms, None);
        assert!(args.poll);
        assert_eq!(args.session_id, None);
    }

    #[test]
    fn parse_freeform_args_rejects_timeout_ms_when_poll_true() {
        let input = "// codex-js-repl: poll=true timeout_ms=15000\nconsole.log('ok');";
        let err = parse_freeform_args(input, true).expect_err("expected error");
        assert_eq!(err.to_string(), JS_REPL_POLL_TIMEOUT_ARG_ERROR_MESSAGE);
    }

    #[test]
    fn parse_freeform_args_with_poll_and_session_id() {
        let input = "// codex-js-repl: poll=true session_id=my-session\nconsole.log('ok');";
        let args = parse_freeform_args(input, true).expect("parse args");
        assert_eq!(args.code, "console.log('ok');");
        assert_eq!(args.timeout_ms, None);
        assert!(args.poll);
        assert_eq!(args.session_id.as_deref(), Some("my-session"));
    }

    #[test]
    fn parse_freeform_args_with_comma_separated_poll_and_session_id() {
        let input = "// codex-js-repl: poll=true, session_id=my-session\nconsole.log('ok');";
        let args = parse_freeform_args(input, true).expect("parse args");
        assert_eq!(args.code, "console.log('ok');");
        assert_eq!(args.timeout_ms, None);
        assert!(args.poll);
        assert_eq!(args.session_id.as_deref(), Some("my-session"));
    }

    #[test]
    fn parse_freeform_args_with_comma_separated_pragma_without_spaces() {
        let input = "// codex-js-repl: timeout_ms=15000,poll=false\nconsole.log('ok');";
        let args = parse_freeform_args(input, true).expect("parse args");
        assert_eq!(args.code, "console.log('ok');");
        assert_eq!(args.timeout_ms, Some(15_000));
        assert!(!args.poll);
        assert_eq!(args.session_id, None);
    }

    #[test]
    fn parse_freeform_args_rejects_session_id_without_poll() {
        let input = "// codex-js-repl: session_id=my-session\nconsole.log('ok');";
        let err = parse_freeform_args(input, true).expect_err("expected error");
        assert_eq!(
            err.to_string(),
            "js_repl session_id is only supported when poll=true"
        );
    }

    #[test]
    fn parse_freeform_args_rejects_unknown_key() {
        let err = parse_freeform_args("// codex-js-repl: nope=1\nconsole.log('ok');", true)
            .expect_err("expected error");
        assert_eq!(
            err.to_string(),
            "js_repl pragma only supports timeout_ms, poll, session_id; got `nope`"
        );
    }

    #[test]
    fn parse_freeform_args_rejects_duplicate_poll() {
        let err = parse_freeform_args(
            "// codex-js-repl: poll=true poll=false\nconsole.log('ok');",
            true,
        )
        .expect_err("expected error");
        assert_eq!(
            err.to_string(),
            "js_repl pragma specifies poll more than once"
        );
    }

    #[test]
    fn parse_freeform_args_rejects_json_wrapped_code() {
        let err =
            parse_freeform_args(r#"{"code":"await doThing()"}"#, true).expect_err("expected error");
        assert_eq!(
            err.to_string(),
            "js_repl is a freeform tool and expects raw JavaScript source. Resend plain JS only (optional first line `// codex-js-repl: ...`); do not send JSON (`{\"code\":...}`), quoted code, or markdown fences."
        );
    }

    #[test]
    fn parse_freeform_args_hides_polling_keys_when_polling_disabled() {
        let err = parse_freeform_args("// codex-js-repl: nope=1\nconsole.log('ok');", false)
            .expect_err("expected error");
        assert_eq!(
            err.to_string(),
            "js_repl pragma only supports timeout_ms; got `nope`"
        );
    }

    #[test]
    fn format_poll_output_serializes_logs_in_json_payload() {
        let result = JsExecPollResult {
            exec_id: "exec-1".to_string(),
            session_id: "session-1".to_string(),
            logs: vec!["line 1".to_string(), "line 2".to_string()],
            output: None,
            content_items: Vec::new(),
            error: None,
            done: false,
        };
        let output = format_poll_output(&result).expect("format poll output");
        let FunctionCallOutputBody::Text(content) = output.body else {
            panic!("expected text poll output");
        };
        let payload: serde_json::Value =
            serde_json::from_str(&content).expect("valid json payload");
        assert_eq!(
            payload,
            json!({
                "exec_id": "exec-1",
                "session_id": "session-1",
                "status": "running",
                "logs": "line 1\nline 2",
                "output": null,
                "error": null,
            })
        );
    }

    #[test]
    fn format_poll_output_serializes_empty_output_as_null() {
        let result = JsExecPollResult {
            exec_id: "exec-1".to_string(),
            session_id: "session-1".to_string(),
            logs: Vec::new(),
            output: Some(String::new()),
            content_items: Vec::new(),
            error: None,
            done: true,
        };
        let output = format_poll_output(&result).expect("format poll output");
        let FunctionCallOutputBody::Text(content) = output.body else {
            panic!("expected text poll output");
        };
        let payload: serde_json::Value =
            serde_json::from_str(&content).expect("valid json payload");
        assert_eq!(
            payload,
            json!({
                "exec_id": "exec-1",
                "session_id": "session-1",
                "status": "completed",
                "logs": null,
                "output": null,
                "error": null,
            })
        );
    }

    #[test]
    fn format_poll_output_serializes_multimodal_content_items() {
        let result = JsExecPollResult {
            exec_id: "exec-1".to_string(),
            session_id: "session-1".to_string(),
            logs: Vec::new(),
            output: Some("stdout".to_string()),
            content_items: vec![FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,abc".to_string(),
            }],
            error: None,
            done: true,
        };
        let output = format_poll_output(&result).expect("format poll output");
        let FunctionCallOutputBody::ContentItems(items) = output.body else {
            panic!("expected content item poll output");
        };
        assert_eq!(
            items,
            vec![
                FunctionCallOutputContentItem::InputText {
                    text: json!({
                        "exec_id": "exec-1",
                        "session_id": "session-1",
                        "status": "completed",
                        "logs": null,
                        "output": "stdout",
                        "error": null,
                    })
                    .to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,abc".to_string(),
                },
            ]
        );
    }

    #[test]
    fn js_repl_poll_args_reject_unknown_fields() {
        let err = serde_json::from_str::<super::JsReplPollArgs>(
            r#"{"exec_id":"exec-1","unknown":"value"}"#,
        )
        .expect_err("expected unknown-field deserialization error");
        assert!(
            err.to_string().contains("unknown field `unknown`"),
            "unexpected deserialization error: {err}"
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
