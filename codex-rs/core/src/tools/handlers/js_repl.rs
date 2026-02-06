use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crate::exec::ExecToolCallOutput;
use crate::exec::StreamOutput;
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
use crate::tools::js_repl::JsExecPollResult;
use crate::tools::js_repl::JsImageArtifact;
use crate::tools::js_repl::JsReplArgs;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;

const PER_IMAGE_RAW_LIMIT: usize = 32 * 1024 * 1024;
const TOTAL_RAW_LIMIT: usize = 36 * 1024 * 1024;

pub struct JsReplHandler;
pub struct JsReplResetHandler;
pub struct JsReplPollHandler;

#[derive(Clone, Debug, Deserialize)]
struct JsReplPollArgs {
    exec_id: String,
    #[serde(default)]
    yield_time_ms: Option<u64>,
}

fn join_outputs(stdout: &str, stderr: &str) -> String {
    if stdout.is_empty() {
        stderr.to_string()
    } else if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n{stderr}")
    }
}

fn build_js_repl_exec_output(
    output: &str,
    error: Option<&str>,
    duration: Duration,
) -> ExecToolCallOutput {
    let stdout = output.to_string();
    let stderr = error.unwrap_or("").to_string();
    let aggregated_output = join_outputs(&stdout, &stderr);
    ExecToolCallOutput {
        exit_code: if error.is_some() { 1 } else { 0 },
        stdout: StreamOutput::new(stdout),
        stderr: StreamOutput::new(stderr),
        aggregated_output: StreamOutput::new(aggregated_output),
        duration,
        timed_out: false,
    }
}

async fn emit_js_repl_exec_end(
    session: &crate::codex::Session,
    turn: &crate::codex::TurnContext,
    call_id: &str,
    output: &str,
    error: Option<&str>,
    duration: Duration,
) {
    let exec_output = build_js_repl_exec_output(output, error, duration);
    let emitter = ToolEmitter::shell(
        vec!["js_repl".to_string()],
        turn.cwd.clone(),
        ExecCommandSource::Agent,
        false,
    );
    let ctx = ToolEventCtx::new(session, turn, call_id, None);
    emitter
        .emit(ctx, ToolEventStage::Success(exec_output))
        .await;
}

fn encode_artifacts(
    artifacts: &[JsImageArtifact],
) -> (Vec<FunctionCallOutputContentItem>, Vec<String>) {
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    let mut total_base64: usize = 0;

    for (idx, artifact) in artifacts.iter().enumerate() {
        match encode_single_image(artifact, &mut total_base64) {
            Ok(Some(item)) => items.push(item),
            Ok(None) => {
                warnings.push(format!(
                    "image #{idx} omitted: exceeded total payload budget (~50MB base64)"
                ));
            }
            Err(err) => {
                warnings.push(format!("image #{idx} omitted: {err}"));
            }
        }
    }

    (items, warnings)
}

fn encode_single_image(
    artifact: &JsImageArtifact,
    total_base64: &mut usize,
) -> Result<Option<FunctionCallOutputContentItem>, String> {
    let bytes = &artifact.bytes;
    let mime = artifact
        .mime
        .clone()
        .unwrap_or_else(|| "image/png".to_string());

    if bytes.len() > PER_IMAGE_RAW_LIMIT {
        return Err(format!(
            "raw image payload ({size} bytes) exceeds per-image limit",
            size = bytes.len()
        ));
    }

    let base64_size = encoded_len(bytes.len());
    if *total_base64 + base64_size > TOTAL_RAW_LIMIT * 4 / 3 {
        return Ok(None);
    }

    *total_base64 += base64_size;
    let data_url = to_data_url(bytes, &mime);
    Ok(Some(FunctionCallOutputContentItem::InputImage {
        image_url: data_url,
    }))
}

fn to_data_url(bytes: &[u8], mime: &str) -> String {
    let encoded = BASE64_STANDARD.encode(bytes);
    format!("data:{mime};base64,{encoded}")
}

fn encoded_len(raw: usize) -> usize {
    raw.div_ceil(3) * 4
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
            let submission = Arc::clone(&manager)
                .submit(Arc::clone(&session), Arc::clone(&turn), tracker, args)
                .await?;
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
        let result = manager
            .execute(Arc::clone(&session), Arc::clone(&turn), tracker, args)
            .await?;

        let (mut items, warnings) = encode_artifacts(&result.artifacts);
        let mut content = result.output;
        if !warnings.is_empty() {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(&warnings.join("\n"));
        }
        items.insert(
            0,
            FunctionCallOutputContentItem::InputText {
                text: content.clone(),
            },
        );

        emit_js_repl_exec_end(
            session.as_ref(),
            turn.as_ref(),
            &call_id,
            &content,
            None,
            started_at.elapsed(),
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
        if result.done {
            let warnings = if result.artifacts.is_empty() {
                None
            } else {
                let (_, warnings) = encode_artifacts(&result.artifacts);
                if warnings.is_empty() {
                    None
                } else {
                    Some(warnings.join("\n"))
                }
            };
            let display_output = build_poll_display_output(&result, warnings.as_deref());
            let duration = result.duration.unwrap_or(Duration::ZERO);
            emit_js_repl_exec_end(
                session.as_ref(),
                turn.as_ref(),
                &result.exec_id,
                &display_output,
                result.error.as_deref(),
                duration,
            )
            .await;
        }
        let body = output
            .content_items
            .map(FunctionCallOutputBody::ContentItems)
            .unwrap_or_else(|| FunctionCallOutputBody::Text(output.content));
        Ok(ToolOutput::Function {
            body,
            success: Some(true),
        })
    }
}

struct JsReplPollOutput {
    content: String,
    content_items: Option<Vec<FunctionCallOutputContentItem>>,
}

fn build_poll_display_output(result: &JsExecPollResult, warnings: Option<&str>) -> String {
    let mut output = String::new();
    let mut push_section = |text: &str| {
        if text.is_empty() {
            return;
        }
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(text);
    };
    if !result.all_logs.is_empty() {
        push_section(&result.all_logs.join("\n"));
    }
    if let Some(text) = result.output.as_deref() {
        push_section(text);
    }
    if let Some(text) = warnings {
        push_section(text);
    }
    output
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

    let mut items = Vec::new();
    if !result.logs.is_empty() {
        items.extend(
            result
                .logs
                .iter()
                .map(|line| FunctionCallOutputContentItem::InputText { text: line.clone() }),
        );
    }
    items.push(FunctionCallOutputContentItem::InputText {
        text: content.clone(),
    });
    if result.done && !result.artifacts.is_empty() {
        let (mut artifacts, warnings) = encode_artifacts(&result.artifacts);
        items.append(&mut artifacts);
        if !warnings.is_empty() {
            items.push(FunctionCallOutputContentItem::InputText {
                text: warnings.join("\n"),
            });
        }
    }

    Ok(JsReplPollOutput {
        content,
        content_items: Some(items),
    })
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
    use super::parse_freeform_args;
    use crate::codex::make_session_and_context_with_rx;
    use crate::protocol::EventMsg;
    use crate::protocol::ExecCommandSource;
    use crate::tools::js_repl::JsExecPollResult;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use pretty_assertions::assert_eq;

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
    fn format_poll_output_includes_logs_as_output_items() {
        let result = JsExecPollResult {
            exec_id: "exec-1".to_string(),
            logs: vec!["line 1".to_string(), "line 2".to_string()],
            all_logs: Vec::new(),
            output: None,
            artifacts: Vec::new(),
            error: None,
            done: false,
            duration: None,
        };
        let output = format_poll_output(&result).expect("format poll output");
        let items = output.content_items.expect("content items");
        assert_eq!(
            items[0],
            FunctionCallOutputContentItem::InputText {
                text: "line 1".to_string()
            }
        );
        assert_eq!(
            items[1],
            FunctionCallOutputContentItem::InputText {
                text: "line 2".to_string()
            }
        );
        assert!(matches!(
            items[2],
            FunctionCallOutputContentItem::InputText { .. }
        ));
    }

    #[tokio::test]
    async fn emit_js_repl_exec_end_sends_event() {
        let (session, turn, rx) = make_session_and_context_with_rx().await;
        super::emit_js_repl_exec_end(
            session.as_ref(),
            turn.as_ref(),
            "call-1",
            "hello",
            None,
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
        assert_eq!(event.cwd, turn.cwd);
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
}
