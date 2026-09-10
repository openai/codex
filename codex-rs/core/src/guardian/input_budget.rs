//! Finalizes a pending reviewer input after tools and turn context are resolved.
//! The thread attachment is a single in-flight request, consumed before history
//! recording. Budget failures preserve it for a compaction retry; successful
//! finalization consumes it. It is not another retained history.

use codex_features::Feature;
use codex_guardian_context::ComposedContext;
use codex_guardian_context::HistoryTruncation;
use codex_guardian_context::RequestBudget;
use codex_guardian_context::effective_input_token_limit;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;

use crate::context::ContextualUserFragment;
use crate::context::GuardianBudgetOmission;
use crate::context_manager::estimate_item_token_count;
use crate::responses_metadata::CodexResponsesRequestKind;
use crate::session::TurnInput;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn::build_prompt;
use crate::session::turn_context::TurnContext;

#[derive(Clone)]
pub(crate) struct PendingReviewContext(pub ComposedContext);

/// Reject inputs that cannot fit even without history or tools before Core tries
/// pre-turn compaction. This is only a feasibility check; its selected copy is
/// discarded. Actual selection waits for the complete first-step overhead.
pub(crate) async fn check_pending(session: &Session, turn: &TurnContext) -> CodexResult<()> {
    let Some(pending) = session
        .services
        .thread_extension_data
        .get::<PendingReviewContext>()
    else {
        return Ok(());
    };
    let base = session.get_prompt_base_instructions().await;
    let minimum_prefix =
        codex_protocol::protocol::TruncationPolicy::Bytes(base.text.len()).token_budget();
    let maximum = effective_input_token_limit(turn.model_info(), turn.config.model_context_window)
        .saturating_sub(super::request_budget::INPUT_TOKEN_MARGIN);
    if pending.0.estimated_tokens().saturating_add(minimum_prefix) > maximum {
        pending
            .0
            .clone()
            .enforce_budget(
                RequestBudget {
                    max_input_tokens: maximum,
                    existing_context_tokens: minimum_prefix,
                },
                GuardianBudgetOmission.render(),
                HistoryTruncation::Allow,
            )
            .map_err(|_error| {
                session
                    .services
                    .thread_extension_data
                    .insert(super::request_budget::ExhaustedReviewBudget::Detected);
                CodexErr::ContextWindowExceeded
            })?;
    }
    Ok(())
}

pub(crate) async fn finalize(
    session: &Session,
    step: &StepContext,
    input: &mut [TurnInput],
    history_truncation: HistoryTruncation,
) -> CodexResult<()> {
    let Some(pending) = session
        .services
        .thread_extension_data
        .get::<PendingReviewContext>()
    else {
        return Ok(());
    };
    let [TurnInput::UserInput { content, .. }] = input else {
        return Err(CodexErr::InvalidRequest(
            "Guardian expects one review input".to_owned(),
        ));
    };
    let context = pending.0.clone();
    let model = &step.settings.model_info;
    let history = session.clone_history().await;
    let prompt = build_prompt(
        history.for_prompt(&model.input_modalities),
        step,
        session.get_prompt_base_instructions().await,
    );
    let request = session.services.model_client.build_responses_request(
        &prompt,
        model,
        /*effort*/ None,
        ReasoningSummary::None,
        /*service_tier*/ None,
        &session
            .responses_metadata(&step.turn, CodexResponsesRequestKind::Turn)
            .await,
    )?;
    let mut existing = super::request_budget::estimate_request_tokens(&request)
        .max(usize::try_from(session.get_total_token_usage().await).unwrap_or(usize::MAX));
    // These reminders are appended after the accepted input. Reserve their full
    // serialized cost before choosing optional evidence, even if the time
    // reminder's interval ultimately suppresses it.
    if step
        .turn
        .config
        .features
        .enabled(Feature::CurrentTimeReminder)
        && step.turn.config.current_time_reminder.is_some()
    {
        let reminder = ContextualUserFragment::into(crate::context::CurrentTimeReminder::new(
            chrono::Utc::now(),
        ));
        existing = existing.saturating_add(
            usize::try_from(estimate_item_token_count(&reminder)).unwrap_or(usize::MAX),
        );
    }
    if let Some(reminder) = session
        .services
        .agent_control
        .rollout_budget()
        .pending_reminder(session.thread_id(), &session.current_window_id().await)
    {
        let reminder = ContextualUserFragment::into(crate::context::RolloutBudgetContext {
            remaining_tokens: reminder.remaining_tokens,
        });
        existing = existing.saturating_add(
            usize::try_from(estimate_item_token_count(&reminder)).unwrap_or(usize::MAX),
        );
    }
    // Reserve the real input annotations as well as the shared content cost.
    // Removing optional items can only reduce this metadata. The remaining
    // margin covers IDs and timestamps assigned when recording the message.
    let mut framing = session.response_item_from_user_input(content.clone());
    if let ResponseItem::Message { content, .. } = &mut framing {
        content.clear();
    }
    let budget = RequestBudget {
        max_input_tokens: effective_input_token_limit(model, step.turn.config.model_context_window)
            .saturating_sub(super::request_budget::INPUT_TOKEN_MARGIN),
        existing_context_tokens: existing.saturating_add(
            usize::try_from(estimate_item_token_count(&framing)).unwrap_or(usize::MAX),
        ),
    };
    let context = context
        .enforce_budget(budget, GuardianBudgetOmission.render(), history_truncation)
        .map_err(|error| {
            session
                .services
                .thread_extension_data
                .insert(super::request_budget::ExhaustedReviewBudget::Detected);
            match error {
                codex_guardian_context::SectionError::EvidenceLimitExceeded { .. } => {
                    CodexErr::ContextWindowExceeded
                }
                error => CodexErr::InvalidRequest(error.to_string()),
            }
        })?;
    for (section, cost) in context.section_costs() {
        for (measurement, value) in cost.measurements() {
            session
                .services
                .session_telemetry
                .histogram_with_boundaries(
                    codex_guardian_context::SECTION_COST_METRIC,
                    i64::try_from(value).unwrap_or(i64::MAX),
                    codex_guardian_context::SECTION_COST_BOUNDARIES,
                    &[
                        ("target", "sync"),
                        ("section", section),
                        ("measurement", measurement),
                    ],
                );
        }
    }
    *content = context
        .into_user_inputs()
        .map_err(|error| CodexErr::InvalidRequest(error.to_string()))?;
    session
        .services
        .thread_extension_data
        .remove::<PendingReviewContext>();
    session
        .services
        .thread_extension_data
        .remove::<super::request_budget::ExhaustedReviewBudget>();
    Ok(())
}

#[cfg(test)]
#[path = "input_budget_tests.rs"]
mod tests;
