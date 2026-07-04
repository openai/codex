//! Installs reconstructed rollout history and compaction-window metadata.

use crate::context::ContextualUserFragment;
use crate::context::TokenBudgetReminder;
use crate::session::PreviousTurnSettings;
use crate::session::rollout_reconstruction;
use crate::session::session::Session;
use crate::state::AutoCompactWindowIds;
use crate::state::SessionState;
use codex_protocol::config_types::AutoCompactTokenLimitScope;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TokenUsageInfo;
use uuid::Uuid;

pub(in crate::session) enum RolloutHistoryInstallMode {
    KeepExisting,
    Replace,
    ReplacePreservingAutoCompactPrefill,
}

pub(in crate::session) enum AutoCompactWindowInstallMode {
    Reconcile,
    Restore,
}

pub(in crate::session) struct RolloutReconstructionInstallOptions {
    pub(in crate::session) token_info: Option<TokenUsageInfo>,
    pub(in crate::session) history: RolloutHistoryInstallMode,
    pub(in crate::session) auto_compact_window: AutoCompactWindowInstallMode,
    pub(in crate::session) token_budget_reminder_delivered: bool,
}

pub(in crate::session) fn history_contains_token_budget_reminder(history: &[ResponseItem]) -> bool {
    history.iter().any(|item| {
        let ResponseItem::Message { role, content, .. } = item else {
            return false;
        };
        role == "developer"
            && content.iter().any(|content| {
                matches!(
                    content,
                    ContentItem::InputText { text }
                        if TokenBudgetReminder::matches_text(text)
                )
            })
    })
}

impl Session {
    pub(in crate::session) fn install_rollout_reconstruction(
        state: &mut SessionState,
        auto_compact_token_limit_scope: AutoCompactTokenLimitScope,
        reconstruction: rollout_reconstruction::RolloutReconstruction,
        options: RolloutReconstructionInstallOptions,
    ) -> Option<PreviousTurnSettings> {
        let RolloutReconstructionInstallOptions {
            token_info,
            history: history_install,
            auto_compact_window,
            token_budget_reminder_delivered,
        } = options;
        let rollout_reconstruction::RolloutReconstruction {
            history,
            previous_turn_settings,
            reference_context_item,
            world_state_baseline,
            window_number,
            first_window_id,
            previous_window_id,
            window_id,
        } = reconstruction;
        match history_install {
            RolloutHistoryInstallMode::ReplacePreservingAutoCompactPrefill => {
                state.replace_history_preserving_auto_compact_prefill(
                    history,
                    reference_context_item,
                );
            }
            RolloutHistoryInstallMode::Replace => {
                state.replace_history(history, reference_context_item);
            }
            RolloutHistoryInstallMode::KeepExisting => {
                state.set_reference_context_item(reference_context_item);
            }
        }
        state
            .history
            .replace_world_state_baseline(world_state_baseline);
        let ids = Self::rollout_reconstruction_window_ids(
            first_window_id,
            previous_window_id,
            window_id,
            state.auto_compact_window_ids(),
        );
        match auto_compact_window {
            AutoCompactWindowInstallMode::Reconcile => {
                state.reconcile_auto_compact_window(window_number, ids);
            }
            AutoCompactWindowInstallMode::Restore => {
                state.restore_auto_compact_window(window_number, ids);
            }
        }
        state.set_token_budget_reminder_delivered(token_budget_reminder_delivered);
        state.set_previous_turn_settings(previous_turn_settings.clone());
        if let Some(token_info) = token_info {
            state.set_token_info(Some(token_info));
        }
        if matches!(
            auto_compact_token_limit_scope,
            AutoCompactTokenLimitScope::BodyAfterPrefix
        ) {
            let base_instructions = BaseInstructions {
                text: state.session_configuration.base_instructions.clone(),
            };
            if let Some(prefix_tokens) = state
                .history
                .estimate_token_count_with_base_instructions(&base_instructions)
            {
                state.set_auto_compact_window_estimated_prefill(prefix_tokens);
            }
        }
        previous_turn_settings
    }

    pub(super) fn rollout_reconstruction_window_ids(
        first_window_id: Option<Uuid>,
        previous_window_id: Option<Uuid>,
        window_id: Option<Uuid>,
        fallback_ids: AutoCompactWindowIds,
    ) -> AutoCompactWindowIds {
        let window_id = window_id.unwrap_or(fallback_ids.window_id);
        AutoCompactWindowIds {
            first_window_id: first_window_id.unwrap_or(window_id),
            previous_window_id,
            window_id,
        }
    }
}
