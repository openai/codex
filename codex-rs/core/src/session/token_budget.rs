use super::session::Session;
use super::turn_context::TurnContext;
use crate::config::Config;
use crate::config::TokenBudgetConfig;
use crate::context::ContextualUserFragment;
use codex_features::Feature;
use codex_protocol::openai_models::ModelInfo;

pub(super) fn has_explicit_settings(config: &Config) -> bool {
    config
        .config_layer_stack
        .effective_config()
        .get("features")
        .and_then(|features| features.get("token_budget"))
        .and_then(|token_budget| token_budget.as_table())
        .is_some_and(|settings| settings.keys().any(|key| key != "enabled" && key != "mode"))
        || config.token_budget.as_ref().is_some_and(|token_budget| {
            token_budget
                != &TokenBudgetConfig {
                    mode: token_budget.mode,
                    ..TokenBudgetConfig::default()
                }
        })
}

pub(super) fn apply_model_defaults(config: &mut Config, model_info: &ModelInfo) {
    if !config.features.enabled(Feature::TokenBudget) || has_explicit_settings(config) {
        return;
    }

    let Some(model_defaults) = model_info
        .model_messages
        .as_ref()
        .and_then(|messages| messages.token_budget.as_ref())
    else {
        return;
    };

    let token_budget = TokenBudgetConfig {
        mode: config
            .token_budget
            .as_ref()
            .map(|token_budget| token_budget.mode)
            .unwrap_or_default(),
        reminder_threshold_tokens: Some(model_defaults.reminder_threshold_tokens),
        reminder_message_template: model_defaults.reminder_message_template.clone(),
        guidance_message: Some(model_defaults.guidance_message.clone()),
        auto_compact_fallback_prompt: Some(model_defaults.auto_compact_fallback_prompt.clone()),
        auto_compact_fallback_buffer_tokens: Some(
            model_defaults.auto_compact_fallback_buffer_tokens,
        ),
    };

    if let Err(error) = token_budget.validate() {
        tracing::warn!(
            model = %model_info.slug,
            %error,
            "ignoring invalid model-owned token-budget defaults"
        );
        return;
    }

    config.token_budget = Some(token_budget);
}

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
