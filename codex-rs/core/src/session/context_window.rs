use super::session::Session;
use super::turn_context::TurnContext;
use codex_protocol::config_types::AutoCompactTokenLimitScope;

#[derive(Debug)]
pub(crate) struct ContextWindowTokenStatus {
    // Full active context usage, independent of the configured auto-compact scope.
    pub(crate) active_context_tokens: i64,
    // Usage counted against `model_auto_compact_token_limit` for the current scope.
    pub(crate) auto_compact_scope_tokens: i64,
    pub(crate) auto_compact_scope_limit: Option<i64>,
    pub(crate) full_context_window_limit: Option<i64>,
    pub(crate) tokens_until_compaction: Option<i64>,
    pub(crate) auto_compact_window_prefill_tokens: Option<i64>,
    pub(crate) full_context_window_limit_reached: bool,
    pub(crate) token_limit_reached: bool,
}

struct BodyAfterPrefixWindowStatus {
    full_context_window_limit: Option<i64>,
    auto_compact_window_prefill_tokens: Option<i64>,
}

pub(crate) async fn context_window_token_status(
    sess: &Session,
    turn_context: &TurnContext,
) -> ContextWindowTokenStatus {
    let active_context_tokens = sess.get_total_token_usage().await;

    let (auto_compact_scope_tokens, auto_compact_scope_limit, body_window) =
        match turn_context.config.model_auto_compact_token_limit_scope {
            AutoCompactTokenLimitScope::Total => (
                active_context_tokens,
                turn_context.model_info.auto_compact_token_limit(),
                None,
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
                    scope_limit,
                    Some(BodyAfterPrefixWindowStatus {
                        full_context_window_limit,
                        auto_compact_window_prefill_tokens: window.prefill_input_tokens,
                    }),
                )
            }
        };

    let full_context_window_limit = body_window
        .as_ref()
        .and_then(|window| window.full_context_window_limit);
    let auto_compact_window_prefill_tokens = body_window
        .as_ref()
        .and_then(|window| window.auto_compact_window_prefill_tokens);

    let full_context_window_limit_reached =
        full_context_window_limit.is_some_and(|full_context_window_limit| {
            active_context_tokens >= full_context_window_limit
        });
    let token_limit_reached = auto_compact_scope_limit
        .is_some_and(|limit| auto_compact_scope_tokens >= limit)
        || full_context_window_limit_reached;

    let auto_compact_scope_remaining = auto_compact_scope_limit
        .map(|limit| limit.saturating_sub(auto_compact_scope_tokens).max(0));
    let full_context_remaining =
        full_context_window_limit.map(|limit| limit.saturating_sub(active_context_tokens).max(0));
    let tokens_until_compaction = match (auto_compact_scope_remaining, full_context_remaining) {
        (Some(scope_remaining), Some(full_remaining)) => Some(scope_remaining.min(full_remaining)),
        (scope_remaining, full_remaining) => scope_remaining.or(full_remaining),
    };

    ContextWindowTokenStatus {
        active_context_tokens,
        auto_compact_scope_tokens,
        auto_compact_scope_limit,
        full_context_window_limit,
        tokens_until_compaction,
        auto_compact_window_prefill_tokens,
        full_context_window_limit_reached,
        token_limit_reached,
    }
}
