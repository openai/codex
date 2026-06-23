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
    pub(crate) context_remaining_tokens: Option<i64>,
}

impl ContextWindowTokenStatus {
    pub(crate) fn tokens_until_compaction(&self) -> i64 {
        tokens_until_compaction(
            self.active_context_tokens,
            self.auto_compact_scope_tokens,
            self.auto_compact_scope_limit,
            self.full_context_window_limit,
        )
    }
}

pub(crate) async fn context_window_token_status(
    sess: &Session,
    turn_context: &TurnContext,
) -> ContextWindowTokenStatus {
    let active_context_tokens = sess.get_total_token_usage().await;
    let (
        auto_compact_scope_tokens,
        auto_compact_scope_limit,
        full_context_window_limit,
        auto_compact_window_prefill_tokens,
        has_context_remaining_limit,
    ) = match turn_context.config.model_auto_compact_token_limit_scope {
        AutoCompactTokenLimitScope::Total => (
            active_context_tokens,
            turn_context
                .model_info
                .auto_compact_token_limit()
                .unwrap_or(i64::MAX),
            None,
            None,
            false,
        ),
        AutoCompactTokenLimitScope::BodyAfterPrefix => {
            let window = sess.auto_compact_window_snapshot().await;
            let baseline = window.prefill_input_tokens.unwrap_or(active_context_tokens);
            let scope_limit = turn_context
                .config
                .model_auto_compact_token_limit
                .or_else(|| turn_context.model_info.auto_compact_token_limit());
            let full_context_window_limit = turn_context.model_context_window();
            (
                active_context_tokens.saturating_sub(baseline),
                scope_limit.unwrap_or(i64::MAX),
                full_context_window_limit,
                window.prefill_input_tokens,
                scope_limit.is_some() || full_context_window_limit.is_some(),
            )
        }
    };
    let full_context_window_limit_reached =
        full_context_window_limit.is_some_and(|full_context_window_limit| {
            active_context_tokens >= full_context_window_limit
        });
    let token_limit_reached =
        auto_compact_scope_tokens >= auto_compact_scope_limit || full_context_window_limit_reached;
    let context_remaining_limit_scope = turn_context.config.model_auto_compact_token_limit_scope;
    let context_remaining_tokens = match context_remaining_limit_scope {
        AutoCompactTokenLimitScope::Total => turn_context
            .model_context_window()
            .map(|limit| limit.saturating_sub(active_context_tokens).max(0)),
        AutoCompactTokenLimitScope::BodyAfterPrefix if has_context_remaining_limit => {
            Some(tokens_until_compaction(
                active_context_tokens,
                auto_compact_scope_tokens,
                auto_compact_scope_limit,
                full_context_window_limit,
            ))
        }
        AutoCompactTokenLimitScope::BodyAfterPrefix => None,
    };
    ContextWindowTokenStatus {
        active_context_tokens,
        auto_compact_scope_tokens,
        auto_compact_scope_limit,
        full_context_window_limit,
        auto_compact_window_prefill_tokens,
        full_context_window_limit_reached,
        token_limit_reached,
        context_remaining_tokens,
    }
}

fn tokens_until_compaction(
    active_context_tokens: i64,
    auto_compact_scope_tokens: i64,
    auto_compact_scope_limit: i64,
    full_context_window_limit: Option<i64>,
) -> i64 {
    let full_context_remaining = full_context_window_limit.map_or(i64::MAX, |limit| {
        limit.saturating_sub(active_context_tokens)
    });
    auto_compact_scope_limit
        .saturating_sub(auto_compact_scope_tokens)
        .min(full_context_remaining)
        .max(0)
}
