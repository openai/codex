#![cfg(test)]

use super::BoardSize;
use pretty_assertions::assert_eq;
use std::num::NonZeroU32;

#[test_case::test_case(None, 32 * 1024 * 1024; "default limit")]
#[test_case::test_case(NonZeroU32::new(1), 1024 * 1024; "configured limit")]
fn budget_counts_utf8_copies_for_distinct_destinations_only(limit: Option<NonZeroU32>, bytes: u64) {
    let size = BoardSize {
        channels: 1,
        posts: 1000,
        payload_bytes: bytes - 4,
        ..BoardSize::new(limit)
    };
    let routes = [vec!["old".into(), "new".into(), "new".into()]];
    let texts = ["é".into(), "deferred".into()];
    let full = size.with_batch(/*channels*/ 2, &routes, &texts).unwrap();
    assert_eq!(
        full,
        BoardSize {
            channels: 2,
            posts: 1002,
            payload_bytes: bytes,
            ..size
        }
    );
    assert!(full.with_batch(/*channels*/ 2, &routes, &texts).is_err());
}
