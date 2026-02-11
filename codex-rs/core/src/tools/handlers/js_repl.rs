use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::sync::Arc;

use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::js_repl::JS_REPL_PRAGMA_PREFIX;
use crate::tools::js_repl::JsReplArgs;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::FunctionCallOutputBody;

pub struct JsReplHandler;
pub struct JsReplResetHandler;

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
        let manager = turn.js_repl.manager().await?;
        let result = manager
            .execute(Arc::clone(&session), Arc::clone(&turn), tracker, args)
            .await?;

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(result.output),
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
    let directive = pragma.trim();
    if !directive.is_empty() {
        for token in directive.split_whitespace() {
            let (key, value) = token.split_once('=').ok_or_else(|| {
                FunctionCallError::RespondToModel(format!(
                    "js_repl pragma expects space-separated key=value pairs (supported keys: timeout_ms); got `{token}`"
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
                _ => {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "js_repl pragma only supports timeout_ms; got `{key}`"
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
    use super::parse_freeform_args;
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
}
