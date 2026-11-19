//! Sampling hooks observe attempts. Successful synchronous MCP post-hooks can
//! add a price to a completed response or to provider-reported incomplete usage.
use codex_protocol::BudgetUnitsSum;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_protocol::protocol::TokenUsage;
use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
use serde::Deserialize;
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
        #[schemars(required, schema_with = "nullable_schema::<TokenUsage>")]
        token_usage: Option<TokenUsage>,
        /// The provider's identifier for the completed or incomplete response, if available.
        #[schemars(with = "crate::schema::NullableString")]
        response_id: Option<String>,
    },
}

// Hook schemas otherwise omit null from Option fields, but usage is always present on post.
fn nullable_schema<T: JsonSchema>(
    generator: &mut schemars::r#gen::SchemaGenerator,
) -> schemars::schema::Schema {
    schemars::schema::SchemaObject {
        subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
            any_of: Some(vec![
                generator.subschema_for::<T>(),
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
    pub fn completed(response_id: &str, token_usage: Option<&TokenUsage>) -> Self {
        Self::PostModelRequest {
            status: ModelRequestStatus::Completed,
            error: None,
            token_usage: token_usage.cloned(),
            response_id: Some(response_id.to_owned()),
        }
    }

    pub fn failed(error: &codex_protocol::error::CodexErr) -> Self {
        use codex_protocol::error::CodexErrorDetails;
        let (response_id, token_usage) = match error.details() {
            CodexErrorDetails::TurnAborted | CodexErrorDetails::Interrupted => {
                return Self::cancelled();
            }
            CodexErrorDetails::ResponseIncomplete(incomplete) => (
                incomplete.response_id.clone(),
                incomplete.token_usage.clone(),
            ),
            _ => (None, None),
        };
        Self::PostModelRequest {
            status: ModelRequestStatus::Failed,
            error: Some(error.to_string()),
            token_usage,
            response_id,
        }
    }

    pub fn cancelled() -> Self {
        Self::PostModelRequest {
            status: ModelRequestStatus::Cancelled,
            error: None,
            token_usage: None,
            response_id: None,
        }
    }

    fn hook_event(&self) -> HookEventName {
        match self {
            Self::PreModelRequest => HookEventName::PreModelRequest,
            Self::PostModelRequest { .. } => HookEventName::PostModelRequest,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SamplingKind {
    Sampling,
    ForegroundCompaction,
    BackgroundCompaction,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[schemars(rename = "model-request.command.input")]
pub struct ModelRequestHookInput {
    /// Names the hook lifecycle for this attempt, including asynchronous completions.
    #[serde(skip)]
    pub lifecycle_id: uuid::Uuid,
    pub kind: SamplingKind,
    /// Identifies a generation across harness retries and model fallback.
    pub request_id: String,
    /// One-based attempt number within this generation.
    pub attempt_number: u32,
    pub session_id: String,
    /// Identifies the issuing thread within its shared session.
    pub thread_id: String,
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

fn identify_run(run: &mut HookRunSummary, lifecycle_id: uuid::Uuid) {
    run.id = format!("{}:{lifecycle_id}", run.id);
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
    .map(|handler| {
        let mut run = dispatcher::running_summary(handler);
        identify_run(&mut run, request.lifecycle_id);
        run
    })
    .collect()
}

/// Sum of successful hook prices. An overflowing sum is rejected before posting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HookCharge {
    Units(f64),
    Overflow,
}

impl Default for HookCharge {
    fn default() -> Self {
        Self::Units(0.0)
    }
}

/// A failed observer does not erase prices reported by successful siblings.
#[derive(Debug, Default)]
pub struct ModelRequestHookOutcome {
    pub charge: HookCharge,
    pub invalid_price: Option<String>,
}

/// Structured MCP output from a synchronous post-hook. Omitted units add zero
/// to the provider's charge.
#[derive(Deserialize)]
struct PostHookOutput {
    #[serde(default)]
    extra_codex_rollout_budget_units: f64,
}

type ReportedPrice = Result<f64, String>;

pub(crate) async fn run(
    engine: &ClaudeHooksEngine,
    request: ModelRequestHookInput,
    on_completed: impl FnMut(&HookCompletedEvent) + Send,
) -> (Vec<HookCompletedEvent>, ModelRequestHookOutcome) {
    let matched = dispatcher::select_handlers(
        &engine.handlers,
        request.event.hook_event(),
        /*matcher_input*/ None,
    );
    if matched.is_empty() {
        return Default::default();
    }
    let billable = matches!(
        request.event,
        ModelRequestHookEvent::PostModelRequest {
            status: ModelRequestStatus::Completed,
            ..
        } | ModelRequestHookEvent::PostModelRequest {
            status: ModelRequestStatus::Failed,
            token_usage: Some(_),
            ..
        }
    );
    let input = match serde_json::to_value(&request) {
        Ok(input) => input,
        Err(error) => {
            let mut events = common::serialization_failure_hook_events(
                matched,
                Some(request.turn_id),
                error.to_string(),
            );
            for event in &mut events {
                identify_run(&mut event.run, request.lifecycle_id);
            }
            return (events, Default::default());
        }
    };
    // The dispatcher returns synchronous results in configuration order. Prices
    // become visible together, only after the entire group finishes.
    let results = dispatcher::execute_handlers_with_progress(
        engine,
        matched,
        &input,
        request.cwd.as_path(),
        Some(request.turn_id),
        /*metadata*/ None,
        move |handler, result, turn_id| {
            let mut parsed = parse_completed(handler, result, turn_id);
            identify_run(&mut parsed.completed.run, request.lifecycle_id);
            parsed
        },
        on_completed,
    )
    .await;
    let mut outcome = ModelRequestHookOutcome::default();
    let mut reported_units = Some(BudgetUnitsSum::default());
    let mut events = Vec::with_capacity(results.len());
    for result in results {
        if billable && let Some(price) = result.data {
            match price {
                Ok(units) => {
                    reported_units = reported_units.and_then(|sum| sum.checked_add(units));
                }
                Err(error) => outcome.invalid_price = Some(error),
            }
        }
        events.push(result.completed);
    }
    outcome.charge = match reported_units {
        Some(sum) => HookCharge::Units(sum.total()),
        None => HookCharge::Overflow,
    };
    (events, outcome)
}

fn parse_completed(
    handler: &ConfiguredHandler,
    result: HandlerRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<Option<ReportedPrice>> {
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
    let price = if status == HookRunStatus::Completed {
        result
            .structured_content
            .as_ref()
            .filter(|body| !body.is_null())
            .map(|body| {
                if !body.is_object() {
                    return Err("post-hook structuredContent must be an object or null".into());
                }
                let output: PostHookOutput =
                    serde_path_to_error::deserialize(body).map_err(|error| {
                        // Serde's underlying error can contain the raw hook value.
                        format!("invalid post-hook structuredContent.{}", error.path())
                    })?;
                let units = output.extra_codex_rollout_budget_units;
                if units.is_finite() && units >= 0.0 {
                    Ok(units)
                } else {
                    Err(
                        "hook extra_codex_rollout_budget_units must be finite and nonnegative"
                            .into(),
                    )
                }
            })
    } else {
        None
    };
    dispatcher::ParsedHandler {
        completed: HookCompletedEvent {
            turn_id,
            run: dispatcher::completed_summary(handler, &result, status, entries),
        },
        data: price,
        completion_order: 0,
    }
}
