use super::session::Session;
use super::turn_context::TurnContext;
use crate::context::ContextualUserFragment;
use codex_features::Feature;

const TOKEN_BUDGET_USAGE_THRESHOLDS: [i64; 3] = [25, 50, 75];

pub(super) async fn maybe_record_token_budget_remaining_context(
    sess: &Session,
    turn_context: &TurnContext,
    active_context_tokens_before_sampling: i64,
    active_context_tokens_after_sampling: i64,
) {
    if !turn_context.config.features.enabled(Feature::TokenBudget) {
        return;
    }

    let model_context_window = turn_context
        .model_context_window()
        .filter(|window| *window > 0);
    let crossed_remaining_threshold = model_context_window.is_some_and(|model_context_window| {
        active_context_tokens_after_sampling > active_context_tokens_before_sampling
            && TOKEN_BUDGET_USAGE_THRESHOLDS.iter().any(|threshold| {
                crossed_usage_threshold(
                    active_context_tokens_before_sampling.max(0),
                    active_context_tokens_after_sampling.max(0),
                    model_context_window,
                    *threshold,
                )
            })
    });
    if !crossed_remaining_threshold {
        return;
    }

    if let Some(model_context_window) = model_context_window {
        let tokens_left = model_context_window
            .saturating_sub(active_context_tokens_after_sampling)
            .max(0);
        let response_item = ContextualUserFragment::into(
            crate::context::TokenBudgetRemainingContext::new(tokens_left),
        );
        sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
            .await;
    }
}

pub(super) async fn maybe_record_token_budget_reminder(
    sess: &Session,
    turn_context: &TurnContext,
    tokens_until_compaction: i64,
    reminder_delivered: bool,
) {
    if !turn_context.config.features.enabled(Feature::TokenBudget) || reminder_delivered {
        return;
    }
    let Some(config) = turn_context.config.token_budget.as_ref() else {
        return;
    };
    let Some(threshold) = config.reminder_threshold_tokens else {
        return;
    };
    if tokens_until_compaction > threshold {
        return;
    }

    let response_item = ContextualUserFragment::into(crate::context::TokenBudgetReminder::new(
        &config.reminder_message_template,
        tokens_until_compaction,
    ));
    sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
        .await;
    sess.mark_token_budget_reminder_delivered().await;
}

fn crossed_usage_threshold(
    tokens_before_sampling: i64,
    tokens_after_sampling: i64,
    model_context_window: i64,
    threshold_percent: i64,
) -> bool {
    tokens_before_sampling.saturating_mul(100)
        < model_context_window.saturating_mul(threshold_percent)
        && tokens_after_sampling.saturating_mul(100)
            >= model_context_window.saturating_mul(threshold_percent)
}
