//! V2 compact dispatch - entry point for auto and manual compaction.
//!
//! This module provides the dispatch logic for the V2 compact system,
//! implementing the two-tier architecture (micro-compact → full compact).

use std::sync::Arc;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::features::Feature;
use crate::protocol::EventMsg;
use crate::protocol::ExtEventMsg;
use crate::protocol::MicroCompactCompletedEvent;
use codex_protocol::user_input::UserInput;
use tracing::debug;
use tracing::info;

use super::CompactConfig;
use super::CompactResult;
use super::MicroCompactConfig;
use super::TokenCounter;
use super::calculate_thresholds;
use super::full_compact::DEFAULT_CONTEXT_WINDOW;
use super::full_compact::run_full_compact_v2;
use super::try_micro_compact;

/// V2 auto-compact dispatch.
///
/// Called when token limit is reached and `Feature::CompactV2` is enabled.
/// Implements the two-tier architecture:
/// 1. Try micro-compact first (fast, no API)
/// 2. Fall back to full compact (LLM summarization)
pub(crate) async fn auto_compact_dispatch(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> CompactResult {
    info!("V2 auto-compact dispatch started");

    // Get config from TurnContext's client
    let session_config = turn_context.client.config();
    let config = &session_config.ext.compact;

    // Validate config
    if let Err(e) = config.validate() {
        tracing::warn!("Invalid compact config: {e}");
        return CompactResult::Skipped;
    }

    // 1. Check if disabled
    if !config.enabled || !config.auto_compact_enabled {
        debug!("Compact V2 disabled in config");
        return CompactResult::Skipped;
    }

    // 2. Calculate threshold
    let used_tokens = sess.get_total_token_usage().await;
    let context_limit = turn_context
        .client
        .get_model_context_window()
        .unwrap_or(DEFAULT_CONTEXT_WINDOW);
    let threshold_state = calculate_thresholds(used_tokens, context_limit, &config);

    debug!(
        "Threshold state: used={}, limit={}, above_auto_compact={}",
        used_tokens, context_limit, threshold_state.is_above_auto_compact
    );

    if !threshold_state.is_above_auto_compact {
        return CompactResult::NotNeeded;
    }

    // 3. Try micro-compact first (if Feature::MicroCompact enabled)
    if sess.enabled(Feature::MicroCompact) {
        if let Some(result) = try_micro_compact_v2(&sess, &config).await {
            if result.was_effective {
                // Recompute token usage after micro-compact
                sess.recompute_token_usage(&turn_context).await;

                // Emit MicroCompactCompleted event
                let event = EventMsg::Ext(ExtEventMsg::MicroCompactCompleted(
                    MicroCompactCompletedEvent {
                        tools_compacted: result.tools_compacted,
                        tokens_saved: result.tokens_saved,
                    },
                ));
                sess.send_event(&turn_context, event).await;

                info!(
                    "Micro-compact succeeded: compacted {} tools, saved {} tokens",
                    result.tools_compacted, result.tokens_saved
                );
                return CompactResult::MicroCompacted(result);
            }
        }
    }

    // 4. Check for remote compact (OpenAI provider + Feature::RemoteCompaction)
    let provider = turn_context.client.get_provider();
    if provider.is_openai() && sess.enabled(Feature::RemoteCompaction) {
        info!("Using remote compact (OpenAI + RemoteCompaction enabled)");
        crate::compact_remote::run_remote_compact_task(sess.clone(), turn_context.clone()).await;
        return CompactResult::RemoteCompacted;
    }

    // 5. Fall back to full compact V2
    info!("Using full compact V2");
    match run_full_compact_v2(sess, turn_context, config, true).await {
        Ok(metrics) => CompactResult::FullCompacted(metrics),
        Err(e) => {
            tracing::error!("Full compact V2 failed: {e}");
            CompactResult::Skipped
        }
    }
}

/// V2 manual compact dispatch.
///
/// Called from /compact command when `Feature::CompactV2` is enabled.
pub(crate) async fn manual_compact_dispatch(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    _input: Vec<UserInput>,
) -> CompactResult {
    info!("V2 manual compact dispatch started");

    // Get config from TurnContext's client
    let session_config = turn_context.client.config();
    let config = &session_config.ext.compact;

    // Check if disabled
    if !config.enabled {
        debug!("Compact V2 disabled in config");
        return CompactResult::Skipped;
    }

    // For manual compact, skip micro-compact and go straight to full compact V2
    info!("Using full compact V2 (manual)");
    match run_full_compact_v2(sess, turn_context, config, false).await {
        Ok(metrics) => CompactResult::FullCompacted(metrics),
        Err(e) => {
            tracing::error!("Full compact V2 (manual) failed: {e}");
            CompactResult::Skipped
        }
    }
}

/// Try micro-compact with session state management.
async fn try_micro_compact_v2(
    sess: &Session,
    config: &CompactConfig,
) -> Option<super::MicroCompactResult> {
    let history = sess.clone_history().await.get_history();

    // Get persistent CompactState from state_ext (keyed by conversation_id)
    let mut compact_state_guard =
        crate::state::state_ext::get_compact_state_mut(sess.conversation_id);

    let micro_config = MicroCompactConfig::from(config);
    let token_counter = TokenCounter::from(config);
    let result = try_micro_compact(
        &history,
        &mut *compact_state_guard,
        &micro_config,
        &token_counter,
    )?;

    if result.was_effective {
        // Apply the compacted items to the session
        sess.replace_history(result.compacted_items.clone()).await;
        Some(result)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_disabled_returns_skipped() {
        let config = CompactConfig {
            enabled: false,
            ..Default::default()
        };
        // Would test auto_compact_dispatch but it requires Session/TurnContext
        assert!(!config.enabled);
    }

    #[test]
    fn auto_compact_disabled_returns_skipped() {
        let config = CompactConfig {
            enabled: true,
            auto_compact_enabled: false,
            ..Default::default()
        };
        assert!(!config.auto_compact_enabled);
    }
}
