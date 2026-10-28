//! Finalizes a pending reviewer input after tools and turn context are resolved.
//! The thread attachment is a single in-flight request, consumed before history
//! recording. Budget failures preserve it for a compaction retry; successful
//! finalization consumes it. Checkpoint recovery skips startup compaction and
//! budgets the fresh full input against the preserved checkpoint.
//! Loss telemetry describes accepted inputs, excluding feasibility checks and failed retries.

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;

use codex_features::Feature;
use codex_guardian_context::ComposedContext;
use codex_guardian_context::HistoryTruncation;
use codex_guardian_context::RequestBudget;
use codex_guardian_context::effective_input_token_limit;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;

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

pub(super) use codex_guardian_reviewer::CheckpointRecovery;

/// Restart a reused reviewer before paying for a summary we would discard.
/// A fresh checkpoint stays intact while final input selection makes the review fit.
pub(crate) fn should_compact(session: &Session) -> CodexResult<bool> {
    let Some(recovery) = session
        .services
        .thread_extension_data
        .get::<CheckpointRecovery>()
    else {
        return Ok(true);
    };
    if recovery.fresh_parent_checkpoint {
        return Ok(false);
    }
    recovery.requested.store(/*val*/ true, Ordering::Release);
    Err(CodexErr::InvalidRequest(
        "Guardian needs compaction; restart from the parent checkpoint".to_owned(),
    ))
}

/// Frozen originals for this review, restored only if its live history is compacted.
pub(super) struct RetainedReviewContext {
    pub context: ComposedContext,
    pub history_version: u64,
}

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
    let mut context = pending.0.clone();
    let history = session
        .clone_history()
        .await
        .for_prompt_annotated(
            &turn.model_info().input_modalities,
            session.services.image_store.as_ref(),
        )
        .await;
    codex_guardian_context::prepare_sync_delivery(&mut context, &history)
        .map_err(|error| CodexErr::InvalidRequest(error.to_string()))?;
    if context.estimated_tokens().saturating_add(minimum_prefix) > maximum {
        context
            .enforce_budget(
                RequestBudget {
                    max_input_tokens: maximum,
                    existing_context_tokens: minimum_prefix,
                },
                GuardianBudgetOmission
                    .render()
                    .into_text()
                    .unwrap_or_default(),
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
    input: &mut Vec<TurnInput>,
    history_truncation: HistoryTruncation,
) -> CodexResult<()> {
    let Some(pending) = session
        .services
        .thread_extension_data
        .get::<PendingReviewContext>()
    else {
        return Ok(());
    };
    let [TurnInput::UserInput { content, .. }] = input.as_slice() else {
        return Err(CodexErr::InvalidRequest(
            "Guardian expects one review input".to_owned(),
        ));
    };
    let mut context = pending.0.clone();
    let model = &step.settings.model_info;
    let history = session.clone_history().await;
    let recovery = session
        .services
        .thread_extension_data
        .get::<CheckpointRecovery>();
    if let Some(recovery) = &recovery
        && history.history_version() != recovery.history_version
    {
        recovery.requested.store(/*val*/ true, Ordering::Release);
        return Err(CodexErr::InvalidRequest(
            "Guardian history changed; restart from the parent checkpoint".to_owned(),
        ));
    }
    let history_version = history.history_version();
    let history = history
        .for_prompt_annotated(
            &model.input_modalities,
            session.services.image_store.as_ref(),
        )
        .await;
    // Use the history that will actually reach the model. Recompute after every
    // compaction retry; evidence removed by compaction must be delivered again.
    codex_guardian_context::prepare_sync_delivery(&mut context, &history)
        .map_err(|error| CodexErr::InvalidRequest(error.to_string()))?;
    let prompt = build_prompt(
        history
            .into_iter()
            .map(codex_history::ResponseItemEnvelope::into_item)
            .collect(),
        step,
        session.get_prompt_base_instructions().await,
        session.current_window_uses_incremental_tools(step).await,
    );
    let request = session.services.model_client.build_responses_request(
        &prompt,
        model,
        /*effort*/ None,
        ReasoningSummary::None,
        /*service_tier*/ None,
        &session
            .responses_metadata(step, CodexResponsesRequestKind::Turn)
            .await,
        /*include_internal*/ true,
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
        .pending_budget_reminder(session.thread_id(), &session.current_window_id().await)
        .await
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
    // A fresh session starts with the full current parent window. Keep its checkpoint
    // and apply the existing final selection instead of compacting and restarting.
    let history_truncation = if recovery
        .as_ref()
        .is_some_and(|recovery| recovery.fresh_parent_checkpoint)
    {
        HistoryTruncation::Allow
    } else {
        history_truncation
    };
    let mut context = context
        .enforce_budget(
            budget,
            GuardianBudgetOmission
                .render()
                .into_text()
                .unwrap_or_default(),
            history_truncation,
        )
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
    let observations = std::mem::take(&mut context.truncations);
    match context.clone().into_annotated_user_inputs() {
        Ok((user_input, metadata)) => {
            if metadata.is_some() {
                input[0] = TurnInput::ResponseItem(codex_history::ResponseItemEnvelope {
                    item: session.response_item_from_user_input(user_input),
                    metadata,
                });
            } else if let TurnInput::UserInput { content, .. } = &mut input[0] {
                *content = user_input;
            }
        }
        Err(codex_guardian_context::SectionError::UnsupportedDelivery {
            section: "conversation_transcript" | "planned_action",
        }) => {
            *input = context
                .into_annotated_messages()
                .into_iter()
                .map(|mut envelope| {
                    // Preserve the ordinary user-content annotations on each text segment;
                    // native evidence retains its original identity and ciphertext.
                    if let ResponseItem::Message { role, content, .. } = &envelope.item
                        && role == "user"
                    {
                        let user_input = content
                            .iter()
                            .map(|item| match item {
                                ContentItem::InputText { text } => Ok(UserInput::Text {
                                    text: text.clone(),
                                    text_elements: Vec::new(),
                                }),
                                ContentItem::InputImage { image, detail } => Ok(UserInput::Image {
                                    image: image.clone(),
                                    detail: *detail,
                                }),
                                ContentItem::OutputText { .. } | ContentItem::InputAudio { .. } => {
                                    Err(CodexErr::InvalidRequest(
                                        "Unsupported Guardian input content".to_owned(),
                                    ))
                                }
                            })
                            .collect::<CodexResult<Vec<_>>>()?;
                        envelope.item = session.response_item_from_user_input(user_input);
                    }
                    Ok(TurnInput::ResponseItem(envelope))
                })
                .collect::<CodexResult<Vec<_>>>()?;
        }
        Err(error) => return Err(CodexErr::InvalidRequest(error.to_string())),
    }
    // Match async classification accounting: one count per component/disposition
    // for this accepted input, with byte totals across all reduction stages.
    let mut truncations: BTreeMap<(&str, &str), (usize, usize)> = BTreeMap::new();
    for observation in observations {
        let disposition = if observation.retained_bytes == 0 {
            "omitted"
        } else {
            "truncated"
        };
        let totals = truncations
            .entry((observation.component, disposition))
            .or_default();
        totals.0 = totals.0.saturating_add(observation.original_bytes);
        totals.1 = totals.1.saturating_add(observation.retained_bytes);
    }
    for ((component, disposition), (original_bytes, retained_bytes)) in truncations {
        session.services.session_telemetry.counter(
            codex_otel::GUARDIAN_REVIEW_TRUNCATION_METRIC,
            /*inc*/ 1,
            &[("component", component), ("disposition", disposition)],
        );
        for (measurement, bytes) in [
            ("original", original_bytes),
            ("retained", retained_bytes),
            ("omitted", original_bytes.saturating_sub(retained_bytes)),
        ] {
            session
                .services
                .session_telemetry
                .histogram_with_boundaries(
                    codex_otel::GUARDIAN_REVIEW_TRUNCATION_BYTES_METRIC,
                    i64::try_from(bytes).unwrap_or(i64::MAX),
                    codex_guardian_context::SECTION_COST_BOUNDARIES,
                    &[
                        ("component", component),
                        ("disposition", disposition),
                        ("measurement", measurement),
                    ],
                );
        }
    }
    step.turn.extension_data.insert(RetainedReviewContext {
        context: pending.0.retained_instructions(),
        history_version,
    });
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
