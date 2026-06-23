use super::session::Session;
use super::turn_context::TurnContext;
use codex_protocol::config_types::AutoCompactTokenLimitScope;

#[derive(Debug)]
pub(crate) struct ContextWindowTokenStatus {
    // Full active context usage, independent of the configured auto-compact scope.
    pub(crate) active_context_tokens: i64,
    // Usage counted against `model_auto_compact_token_limit` for the current scope.
    pub(crate) auto_compact_scope_tokens: i64,
    pub(crate) auto_compact_scope_limit: i64,
    pub(crate) full_context_window_limit: Option<i64>,
    pub(crate) auto_compact_window_prefill_tokens: Option<i64>,
    pub(crate) full_context_window_limit_reached: bool,
    pub(crate) token_limit_reached: bool,
}

impl ContextWindowTokenStatus {
    pub(crate) fn tokens_until_compaction(&self) -> i64 {
        let full_context_remaining = self.full_context_window_limit.map_or(i64::MAX, |limit| {
            limit.saturating_sub(self.active_context_tokens)
        });
        self.auto_compact_scope_limit
            .saturating_sub(self.auto_compact_scope_tokens)
            .min(full_context_remaining)
            .max(0)
    }
}

pub(crate) async fn context_window_token_status(
    sess: &Session,
    turn_context: &TurnContext,
) -> ContextWindowTokenStatus {
    let active_context_tokens = sess.get_total_token_usage().await;
    let mut auto_compact_window_prefill_tokens = None;
    let (auto_compact_scope_tokens, auto_compact_scope_limit, full_context_window_limit) =
        match turn_context.config.model_auto_compact_token_limit_scope {
            AutoCompactTokenLimitScope::Total => (
                active_context_tokens,
                turn_context
                    .model_info
                    .auto_compact_token_limit()
                    .unwrap_or(i64::MAX),
                None,
            ),
            AutoCompactTokenLimitScope::BodyAfterPrefix => {
                let window = sess.auto_compact_window_snapshot().await;
                auto_compact_window_prefill_tokens = window.prefill_input_tokens;
                let baseline = window.prefill_input_tokens.unwrap_or(active_context_tokens);
                (
                    active_context_tokens.saturating_sub(baseline),
                    turn_context
                        .config
                        .model_auto_compact_token_limit
                        .or_else(|| turn_context.model_info.auto_compact_token_limit())
                        .unwrap_or(i64::MAX),
                    turn_context.model_context_window(),
                )
            }
        };
    let full_context_window_limit_reached =
        full_context_window_limit.is_some_and(|full_context_window_limit| {
            active_context_tokens >= full_context_window_limit
        });
    let token_limit_reached =
        auto_compact_scope_tokens >= auto_compact_scope_limit || full_context_window_limit_reached;
    ContextWindowTokenStatus {
        active_context_tokens,
        auto_compact_scope_tokens,
        auto_compact_scope_limit,
        full_context_window_limit,
        auto_compact_window_prefill_tokens,
        full_context_window_limit_reached,
        token_limit_reached,
    }
}
