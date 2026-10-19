//! Checks that successful refreshes preserve newly pending work.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn successful_manual_claim_consumes_only_preexisting_work() {
    for initially_pending in [false, true] {
        for invalidated_during_refresh in [false, true] {
            let refresh = McpRefresh::new();
            if initially_pending {
                refresh.invalidate();
            }
            assert_eq!(refresh.claim(), initially_pending);
            let mut guard = McpRefreshInvalidationGuard {
                refresh: &refresh,
                published: false,
            };
            if invalidated_during_refresh {
                refresh.invalidate();
            }
            guard.published = true;
            drop(guard);
            assert_eq!(refresh.is_pending(), invalidated_during_refresh);
        }
    }
}
