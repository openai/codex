use super::session::Session;
use super::turn_context::TurnContext;
use crate::context::ContextualUserFragment;
use codex_features::Feature;

pub(super) async fn maybe_record(
    sess: &Session,
    turn_context: &TurnContext,
    base_window_tokens_remaining: Option<i64>,
    allow_auto_compact_fallback: bool,
) {
    if !turn_context.config.features.enabled(Feature::TokenBudget) {
        return;
    }
    let Some(base_window_tokens_remaining) = base_window_tokens_remaining else {
        return;
    };

    let Some(config) = turn_context.config.token_budget.as_ref() else {
        return;
    };

    if config
        .reminder_threshold_tokens
        .is_some_and(|threshold| base_window_tokens_remaining <= threshold)
    {
        let reminder_due = {
            let mut state = sess.state.lock().await;
            state.claim_token_budget_reminder()
        };
        if reminder_due {
            let response_item =
                ContextualUserFragment::into(crate::context::TokenBudgetReminder::new(
                    &config.reminder_message_template,
                    base_window_tokens_remaining,
                ));
            sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
                .await;
        }
    }

    if !allow_auto_compact_fallback || base_window_tokens_remaining != 0 {
        return;
    }
    let Some(prompt) = config.auto_compact_fallback_prompt.as_deref() else {
        return;
    };

    let fallback_due = {
        let mut state = sess.state.lock().await;
        state.claim_auto_compact_fallback()
    };
    if !fallback_due {
        return;
    }

    let response_item =
        ContextualUserFragment::into(crate::context::AutoCompactFallbackPrompt::new(prompt));
    sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
        .await;
}
