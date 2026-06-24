use super::session::Session;
use super::turn_context::TurnContext;
use crate::context::ContextualUserFragment;
use codex_features::Feature;

pub(super) async fn maybe_record(
    sess: &Session,
    turn_context: &TurnContext,
    tokens_until_compaction: Option<i64>,
) {
    if !turn_context.config.features.enabled(Feature::TokenBudget) {
        return;
    }
    let Some(tokens_until_compaction) = tokens_until_compaction else {
        return;
    };

    let Some(config) = turn_context.config.token_budget.as_ref().filter(|config| {
        config
            .reminder_threshold_tokens
            .is_some_and(|threshold| tokens_until_compaction <= threshold)
    }) else {
        return;
    };

    let reminder_due = {
        let mut state = sess.state.lock().await;
        state.claim_token_budget_reminder()
    };
    if !reminder_due {
        return;
    }

    let response_item = ContextualUserFragment::into(crate::context::TokenBudgetReminder::new(
        &config.reminder_message_template,
        tokens_until_compaction,
    ));
    sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
        .await;
}
