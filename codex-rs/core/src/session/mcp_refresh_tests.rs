//! Verifies that refreshes consume their own causes and preserve concurrent work.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn next_turn_reprojection_is_not_claimed_by_prewarm() {
    let refresh = McpRefresh::new();
    let thread_id = ThreadId::new();
    refresh.request_for_next_turn();
    refresh.request_for_next_turn();
    assert!(refresh.claim(thread_id).is_none());
    refresh.invalidate(McpRefreshReason::McpConfigReloaded);
    let mut prewarm = refresh.claim(thread_id).expect("unrelated refresh");
    prewarm.published = true;
    drop(prewarm);
    assert!(refresh.claim_for_turn_start());
    assert!(!refresh.claim_for_turn_start());
}

#[test]
fn refresh_keeps_concurrent_invalidations_and_restores_unpublished_causes() {
    let thread_id = ThreadId::new();
    let refresh = McpRefresh::new();
    refresh.invalidate(McpRefreshReason::McpConfigReloaded);
    let claimed = refresh.claim(thread_id).expect("pending");
    refresh.invalidate(McpRefreshReason::RuntimeAuthChanged);
    drop(claimed);

    let mut retry = refresh.claim(thread_id).expect("retry");
    assert_eq!(
        retry.reasons,
        BTreeSet::from([
            McpRefreshReason::RuntimeAuthChanged,
            McpRefreshReason::McpConfigReloaded,
            McpRefreshReason::PreviousRefreshUnpublished
        ])
    );
    refresh.invalidate(McpRefreshReason::RuntimeAuthChanged);
    retry.published = true;
    drop(retry);

    let mut next = refresh.claim(thread_id).expect("new invalidation");
    assert_eq!(
        next.reasons,
        BTreeSet::from([McpRefreshReason::RuntimeAuthChanged])
    );
    next.published = true;
    drop(next);
    assert!(refresh.claim(thread_id).is_none());
}
