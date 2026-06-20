use super::session::Session;
use super::turn_context::TurnContext;
use crate::context::ContextualUserFragment;
use codex_features::Feature;

const TOKEN_BUDGET_USAGE_THRESHOLDS: [i64; 3] = [25, 50, 75];

pub(super) async fn maybe_record_token_budget_remaining_context(
    sess: &Session,
    turn_context: &TurnContext,
    tokens_before_sampling: i64,
    tokens_after_sampling: i64,
) {
    if !turn_context.config.features.enabled(Feature::TokenBudget) {
        return;
    }
    let Some(model_context_window) = turn_context.model_context_window() else {
        return;
    };
    if model_context_window <= 0 || tokens_after_sampling <= tokens_before_sampling {
        return;
    }

    let tokens_before_sampling = tokens_before_sampling.max(0);
    let tokens_after_sampling = tokens_after_sampling.max(0);
    let crossed_threshold = TOKEN_BUDGET_USAGE_THRESHOLDS.iter().any(|threshold| {
        tokens_before_sampling.saturating_mul(100) < model_context_window.saturating_mul(*threshold)
            && tokens_after_sampling.saturating_mul(100)
                >= model_context_window.saturating_mul(*threshold)
    });
    if !crossed_threshold {
        return;
    }

    let tokens_left = model_context_window
        .saturating_sub(tokens_after_sampling)
        .max(0);

    let response_item = ContextualUserFragment::into(
        crate::context::TokenBudgetRemainingContext::new(tokens_left),
    );
    sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
        .await;
}
