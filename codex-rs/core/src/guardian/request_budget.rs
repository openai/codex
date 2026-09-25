//! Restores compacted retained evidence and checks complete synchronous requests.
//! Restored records are budgeted and persisted before sampling, including continuations.

use codex_api::ResponsesApiRequest;
use codex_context_fragments::set_annotated_content;
use codex_guardian_context::HistoryTruncation;
use codex_guardian_context::REQUEST_TOKENS_BOUNDARIES;
use codex_guardian_context::REQUEST_TOKENS_METRIC;
use codex_guardian_context::RequestBudget;
use codex_guardian_context::effective_input_token_limit;
use codex_otel::SessionTelemetry;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;

use super::input_budget::RetainedReviewContext;
use crate::context::ContextualUserFragment;
use crate::context::GuardianBudgetOmission;
use crate::context::GuardianRetainedInstructions;
use crate::context_manager::estimate_item_token_count;
use crate::session::session::Session;
use crate::session::step_context::StepContext;

pub(super) const INPUT_TOKEN_MARGIN: usize = 256;

// Keep a failed reviewer out of reuse unless compaction and a fresh budget
// check succeed. Also distinguishes local budget failures from backend errors.
pub(crate) enum ExhaustedReviewBudget {
    Detected,
    // A compaction service failure must not be mistaken for local exhaustion.
    Compacting,
}

pub(crate) fn observe(telemetry: &SessionTelemetry, request: &ResponsesApiRequest) -> usize {
    let total = estimate_request_tokens(request);
    // The assembled input already includes inherited history and the current
    // review. Do not report a guessed old/new split after context injection.
    telemetry.histogram_with_boundaries(
        REQUEST_TOKENS_METRIC,
        i64::try_from(total).unwrap_or(i64::MAX),
        REQUEST_TOKENS_BOUNDARIES,
        &[("target", "sync"), ("component", "total")],
    );
    total
}

pub(super) fn estimate_request_tokens(request: &ResponsesApiRequest) -> usize {
    let input = request
        .input
        .iter()
        .map(estimate_item_token_count)
        .fold(0i64, i64::saturating_add);
    let instructions = TruncationPolicy::Bytes(request.instructions.len()).token_budget();
    let metadata = serde_json::to_vec(&(&request.tools, &request.text))
        .map(|bytes| TruncationPolicy::Bytes(bytes.len()).token_budget())
        .unwrap_or(usize::MAX);
    usize::try_from(input)
        .unwrap_or(usize::MAX)
        .saturating_add(instructions)
        .saturating_add(metadata)
}

/// Restores originals lost during this review, then checks the complete request.
/// Only budget-admitted additions enter history, preserving subsequent request prefixes.
pub(crate) async fn prepare_prompt(
    session: &Session,
    prompt: &mut crate::client_common::Prompt,
    step: &StepContext,
    metadata: &crate::responses_metadata::CodexResponsesMetadata,
) -> CodexResult<()> {
    let model = &step.settings.model_info;
    let maximum = effective_input_token_limit(model, step.turn.config.model_context_window)
        .saturating_sub(INPUT_TOKEN_MARGIN);
    let history = session.clone_history().await;
    let history_version = history.history_version();
    let retained = step.turn.extension_data.get::<RetainedReviewContext>();
    let mut restored = Vec::new();
    if let Some(retained) = retained
        .as_ref()
        .filter(|retained| retained.history_version != history_version)
    {
        let baseline = session.services.model_client.build_responses_request(
            prompt,
            model,
            /*effort*/ None,
            codex_protocol::config_types::ReasoningSummary::None,
            /*service_tier*/ None,
            metadata,
            /*include_internal*/ true,
        )?;
        let mut context = retained.context.clone();
        context.retain_new_instructions(&history.for_prompt_annotated(&model.input_modalities));
        let context = context
            .enforce_budget(
                RequestBudget {
                    max_input_tokens: maximum,
                    existing_context_tokens: estimate_request_tokens(&baseline),
                },
                GuardianBudgetOmission.render(),
                HistoryTruncation::Preserve,
            )
            .map_err(|error| {
                session
                    .services
                    .thread_extension_data
                    .insert(ExhaustedReviewBudget::Detected);
                match error {
                    codex_guardian_context::SectionError::EvidenceLimitExceeded { .. } => {
                        CodexErr::ContextWindowExceeded
                    }
                    error => CodexErr::InvalidRequest(error.to_string()),
                }
            })?;
        let mut messages = context.into_annotated_messages();
        for envelope in &mut messages {
            let message = &mut envelope.item;
            let ResponseItem::Message { content, .. } = message else {
                return Err(CodexErr::InvalidRequest(
                    "expected restored Guardian text".to_owned(),
                ));
            };
            let annotated = std::mem::take(content)
                .into_iter()
                .map(|item| {
                    let ContentItem::InputText { text } = item else {
                        return Err(CodexErr::InvalidRequest(
                            "expected restored Guardian text".to_owned(),
                        ));
                    };
                    Ok(GuardianRetainedInstructions::try_from(text)?
                        .render_fragment()
                        .into_parts()
                        .1)
                })
                .collect::<CodexResult<Vec<_>>>()?;
            set_annotated_content(message, annotated).ok_or_else(|| {
                CodexErr::InvalidRequest("expected restored Guardian message".to_owned())
            })?;
        }
        restored = session
            .prepare_annotated_conversation_items_for_history(step.turn.as_ref(), model, messages)
            .await
            .0;
        prompt
            .input
            .extend(restored.iter().map(|envelope| envelope.item.clone()));
    }
    let request = session.services.model_client.build_responses_request(
        prompt,
        model,
        /*effort*/ None,
        codex_protocol::config_types::ReasoningSummary::None,
        /*service_tier*/ None,
        metadata,
        /*include_internal*/ true,
    )?;
    if estimate_request_tokens(&request) > maximum {
        session
            .services
            .thread_extension_data
            .insert(ExhaustedReviewBudget::Detected);
        return Err(CodexErr::ContextWindowExceeded);
    }
    if !restored.is_empty() {
        session
            .record_annotated_conversation_items(step.turn.as_ref(), model, restored)
            .await;
    }
    if let Some(retained) = retained
        .as_ref()
        .filter(|retained| retained.history_version != history_version)
    {
        step.turn.extension_data.insert(RetainedReviewContext {
            context: retained.context.clone(),
            history_version,
        });
    }
    session
        .services
        .thread_extension_data
        .remove::<ExhaustedReviewBudget>();
    Ok(())
}
