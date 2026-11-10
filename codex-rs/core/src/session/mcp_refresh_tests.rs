//! Verifies that refreshes consume their own causes and preserve concurrent work.

use super::*;
use pretty_assertions::assert_eq;

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
