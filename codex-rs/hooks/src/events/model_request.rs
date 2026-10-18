//! Observation-only lifecycle hooks. Output is logged, never interpreted as control flow.
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_protocol::protocol::TokenUsage;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Serialize;

use super::common;
use crate::engine::ClaudeHooksEngine;
use crate::engine::ConfiguredHandler;
use crate::engine::HandlerRunResult;
use crate::engine::dispatcher;

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(tag = "hook_event_name")]
pub enum ModelRequestHookEvent {
    PreModelRequest,
    PostModelRequest {
        status: ModelRequestStatus,
        #[schemars(with = "crate::schema::NullableString")]
        error: Option<String>,
        #[schemars(required, schema_with = "token_usage_schema")]
        token_usage: Option<TokenUsage>,
    },
}

// Hook schemas otherwise omit null from Option fields, but usage is always present on post.
fn token_usage_schema(
    generator: &mut schemars::r#gen::SchemaGenerator,
) -> schemars::schema::Schema {
    schemars::schema::SchemaObject {
        subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
            any_of: Some(vec![
                generator.subschema_for::<TokenUsage>(),
                generator.subschema_for::<()>(),
            ]),
            ..Default::default()
        })),
        ..Default::default()
    }
    .into()
}

#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelRequestStatus {
    Completed,
    Failed,
    Cancelled,
}

impl ModelRequestHookEvent {
    fn hook_event(&self) -> HookEventName {
        match self {
            Self::PreModelRequest => HookEventName::PreModelRequest,
            Self::PostModelRequest { .. } => HookEventName::PostModelRequest,
        }
    }
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[schemars(rename = "model-request.command.input")]
pub struct ModelRequestHookInput {
    pub session_id: String,
    pub turn_id: String,
    pub cwd: AbsolutePathBuf,
    #[schemars(with = "crate::schema::NullableString")]
    pub transcript_path: Option<std::path::PathBuf>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(flatten)]
    pub event: ModelRequestHookEvent,
}

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    request: &ModelRequestHookInput,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        request.event.hook_event(),
        /*matcher_input*/ None,
    )
    .iter()
    .map(dispatcher::running_summary)
    .collect()
}

pub(crate) async fn run(
    engine: &ClaudeHooksEngine,
    request: ModelRequestHookInput,
) -> Vec<HookCompletedEvent> {
    let matched = dispatcher::select_handlers(
        &engine.handlers,
        request.event.hook_event(),
        /*matcher_input*/ None,
    );
    if matched.is_empty() {
        return Vec::new();
    }
    let input = match serde_json::to_string(&request) {
        Ok(input) => input,
        Err(error) => {
            return common::serialization_failure_hook_events(
                matched,
                Some(request.turn_id),
                error.to_string(),
            );
        }
    };
    dispatcher::execute_handlers(
        engine,
        matched,
        input,
        request.cwd.as_path(),
        Some(request.turn_id),
        parse_completed,
    )
    .await
    .into_iter()
    .map(|result| result.completed)
    .collect()
}

fn parse_completed(
    handler: &ConfiguredHandler,
    result: HandlerRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<()> {
    let status = if result.error.is_none() && result.exit_code == Some(0) {
        HookRunStatus::Completed
    } else {
        HookRunStatus::Failed
    };
    let mut entries = Vec::new();
    // Observer stdout is diagnostic, including any JSON that looks like a control directive.
    if !result.stdout.trim().is_empty() {
        tracing::debug!(stdout = %result.stdout, "model request hook output");
    }
    if let Some(text) = common::trimmed_non_empty(&result.stderr) {
        entries.push(HookOutputEntry {
            kind: HookOutputEntryKind::Error,
            text,
        });
    }
    if let Some(error) = &result.error {
        entries.push(HookOutputEntry {
            kind: HookOutputEntryKind::Error,
            text: error.clone(),
        });
    }
    dispatcher::ParsedHandler {
        completed: HookCompletedEvent {
            turn_id,
            run: dispatcher::completed_summary(handler, &result, status, entries),
        },
        data: (),
        completion_order: 0,
    }
}
