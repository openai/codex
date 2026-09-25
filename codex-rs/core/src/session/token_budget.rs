use super::session::Session;
use super::turn_context::TurnContext;
use crate::config::Config;
use crate::config::TokenBudgetConfig;
use crate::config::resolve_token_budget_config;
use crate::context::ContextualUserFragment;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::account::PlanType;
use codex_protocol::auth::AuthMode;
use codex_protocol::openai_models::ModelInfo;

fn experimental_context_is_eligible(auth_mode: AuthMode, plan_type: Option<PlanType>) -> bool {
    auth_mode == AuthMode::Chatgpt
        && matches!(
            plan_type,
            Some(PlanType::Plus | PlanType::Pro | PlanType::ProLite | PlanType::ProMax)
        )
}

pub(super) fn apply_experimental_context(
    config: &mut Config,
    auth: Option<&CodexAuth>,
    starting_model: &ModelInfo,
) -> std::io::Result<()> {
    let provider = &config.model_provider;
    if !config.features.enabled(Feature::ContextManagement)
        || !starting_model.supports_experimental_context
        || !provider.supports_codex_backend_routes()
        || !provider.requires_openai_auth
        || provider.env_key.is_some()
        || provider.experimental_bearer_token.is_some()
        || provider.auth.is_some()
        || provider.aws.is_some()
        || !auth.is_some_and(|auth| {
            experimental_context_is_eligible(auth.auth_mode(), auth.account_plan_type())
        })
        || config.features.enable(Feature::TokenBudget).is_err()
        || !config.features.enabled(Feature::TokenBudget)
    {
        return Ok(());
    }

    if config.token_budget.is_none() {
        let config_toml = config
            .config_layer_stack
            .effective_config()
            .try_into()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        config.token_budget = resolve_token_budget_config(&config_toml, &config.features)?;
    }

    config
        .token_budget
        .get_or_insert_default()
        .use_history_notes_extension = true;
    Ok(())
}

/// Detects explicit preferences before model defaults are applied to the turn config.
pub(super) fn has_explicit_settings(config: &Config) -> bool {
    config
        .config_layer_stack
        .effective_config()
        .get("features")
        .and_then(|features| features.get("token_budget"))
        .and_then(|token_budget| token_budget.as_table())
        .is_some_and(|settings| {
            settings
                .keys()
                .any(|key| !matches!(key.as_str(), "enabled" | "use_history_notes_extension"))
        })
        || config.token_budget.as_ref().is_some_and(|token_budget| {
            let mut settings = token_budget.clone();
            settings.use_history_notes_extension = false;
            settings != TokenBudgetConfig::default()
        })
}

/// Resolves user-configured token-budget preferences against the current model's defaults.
pub(super) fn resolve_token_budget(
    configured_token_budget: Option<&TokenBudgetConfig>,
    use_model_defaults: bool,
    model_info: &ModelInfo,
) -> Option<TokenBudgetConfig> {
    if !use_model_defaults {
        return configured_token_budget.cloned();
    }

    let Some(model_defaults) = model_info
        .model_messages
        .as_ref()
        .and_then(|messages| messages.token_budget.as_ref())
    else {
        return configured_token_budget.cloned();
    };

    let token_budget = TokenBudgetConfig {
        use_history_notes_extension: configured_token_budget
            .is_some_and(|token_budget| token_budget.use_history_notes_extension),
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
        return configured_token_budget.cloned();
    }

    Some(token_budget)
}

/// Applies model activation defaults before thread extensions are initialized.
pub(super) fn apply_model_defaults(config: &mut Config, model_info: &ModelInfo) {
    let Some(model_defaults) = model_info
        .model_messages
        .as_ref()
        .and_then(|messages| messages.token_budget.as_ref())
    else {
        return;
    };
    if !model_defaults.enabled {
        return;
    }

    let has_explicit_config = config.token_budget.is_some()
        || config
            .config_layer_stack
            .effective_config()
            .get("features")
            .and_then(|features| features.get("token_budget"))
            .is_some();
    if has_explicit_config {
        return;
    }

    if config.features.enable(Feature::TokenBudget).is_err() {
        return;
    }
    // Managed requirements can pin the feature off even when enable() succeeds.
    if !config.features.enabled(Feature::TokenBudget) {
        return;
    }

    // Keep prompts unresolved so later turns and model switches use their own defaults.
    config.token_budget = Some(TokenBudgetConfig {
        use_history_notes_extension: model_defaults.use_history_notes_extension,
        ..TokenBudgetConfig::default()
    });
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
            sess.record_conversation_items(
                turn_context,
                turn_context.model_info(),
                std::slice::from_ref(&response_item),
            )
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
    sess.record_conversation_items(
        turn_context,
        turn_context.model_info(),
        std::slice::from_ref(&response_item),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::experimental_context_is_eligible;
    use codex_protocol::account::PlanType;
    use codex_protocol::auth::AuthMode;

    #[test]
    fn experimental_context_requires_eligible_chatgpt_subscription() {
        for (auth_mode, plan_type, expected) in [
            (AuthMode::Chatgpt, PlanType::Plus, true),
            (AuthMode::Chatgpt, PlanType::Pro, true),
            (AuthMode::Chatgpt, PlanType::ProLite, true),
            (AuthMode::Chatgpt, PlanType::ProMax, true),
            (AuthMode::Chatgpt, PlanType::Free, false),
            (AuthMode::Chatgpt, PlanType::Enterprise, false),
            (AuthMode::ApiKey, PlanType::Pro, false),
        ] {
            assert_eq!(
                experimental_context_is_eligible(auth_mode, Some(plan_type)),
                expected
            );
        }
    }
}
