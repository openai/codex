use super::session::Session;
use super::turn_context::TurnContext;
use crate::context::ContextualUserFragment;
use codex_features::Feature;

const TOKEN_BUDGET_USAGE_THRESHOLDS: [i64; 3] = [25, 50, 75];

pub(super) async fn maybe_record(
    sess: &Session,
    turn_context: &TurnContext,
    tokens_before_sampling: i64,
    tokens_after_sampling: i64,
    tokens_until_compaction: i64,
) {
    if !turn_context.config.features.enabled(Feature::TokenBudget) {
        return;
    }

    let mut response_items = Vec::new();
    if let Some(model_context_window) = turn_context.model_context_window()
        && model_context_window > 0
        && tokens_after_sampling > tokens_before_sampling
    {
        let tokens_before_sampling = tokens_before_sampling.max(0);
        let tokens_after_sampling = tokens_after_sampling.max(0);
        let crossed_threshold = TOKEN_BUDGET_USAGE_THRESHOLDS.iter().any(|threshold| {
            tokens_before_sampling.saturating_mul(100)
                < model_context_window.saturating_mul(*threshold)
                && tokens_after_sampling.saturating_mul(100)
                    >= model_context_window.saturating_mul(*threshold)
        });
        if crossed_threshold {
            let tokens_left = model_context_window
                .saturating_sub(tokens_after_sampling)
                .max(0);
            response_items.push(ContextualUserFragment::into(
                crate::context::TokenBudgetRemainingContext::new(tokens_left),
            ));
        }
    }

    if let Some(config) = turn_context.config.token_budget.as_ref().filter(|config| {
        config
            .reminder_threshold_tokens
            .is_some_and(|threshold| tokens_until_compaction <= threshold)
    }) {
        let reminder_due = {
            let mut state = sess.state.lock().await;
            state.claim_token_budget_reminder()
        };
        if reminder_due {
            response_items.push(ContextualUserFragment::into(
                crate::context::TokenBudgetReminder::new(
                    &config.reminder_message_template,
                    tokens_until_compaction,
                ),
            ));
        }
    }

    if !response_items.is_empty() {
        sess.record_conversation_items(turn_context, &response_items)
            .await;
    }
}
