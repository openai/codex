use super::session::Session;
use super::turn_context::TurnContext;
use crate::context::ContextualUserFragment;
use codex_features::Feature;

const TOKEN_BUDGET_USAGE_THRESHOLDS: [i64; 3] = [25, 50, 75];

#[derive(Clone, Copy)]
pub(super) struct TokenBudgetSnapshot {
    pub(super) active_context_tokens: i64,
    pub(super) tokens_until_compaction: i64,
    pub(super) reminder_delivered: bool,
}

pub(super) async fn maybe_record(
    sess: &Session,
    turn_context: &TurnContext,
    before: TokenBudgetSnapshot,
    after: TokenBudgetSnapshot,
) {
    if !turn_context.config.features.enabled(Feature::TokenBudget) {
        return;
    }

    let mut response_items = Vec::with_capacity(2);
    let model_context_window = turn_context
        .model_context_window()
        .filter(|window| *window > 0);
    let crossed_remaining_threshold = model_context_window.is_some_and(|model_context_window| {
        after.active_context_tokens > before.active_context_tokens
            && TOKEN_BUDGET_USAGE_THRESHOLDS.iter().any(|threshold| {
                crossed_usage_threshold(
                    before.active_context_tokens.max(0),
                    after.active_context_tokens.max(0),
                    model_context_window,
                    *threshold,
                )
            })
    });
    if crossed_remaining_threshold && let Some(model_context_window) = model_context_window {
        let tokens_left = model_context_window
            .saturating_sub(after.active_context_tokens)
            .max(0);
        response_items.push(ContextualUserFragment::into(
            crate::context::TokenBudgetRemainingContext::new(tokens_left),
        ));
    }

    let reminder_config = if after.reminder_delivered {
        None
    } else {
        turn_context.config.token_budget.as_ref().filter(|config| {
            config
                .reminder_threshold_tokens
                .is_some_and(|threshold| after.tokens_until_compaction <= threshold)
        })
    };
    if let Some(config) = reminder_config {
        response_items.push(ContextualUserFragment::into(
            crate::context::TokenBudgetReminder::new(
                &config.reminder_message_template,
                after.tokens_until_compaction,
            ),
        ));
    }

    if !response_items.is_empty() {
        sess.record_conversation_items(turn_context, &response_items)
            .await;
    }
    if reminder_config.is_some() {
        sess.mark_token_budget_reminder_delivered().await;
    }
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
