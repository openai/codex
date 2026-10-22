#![cfg(test)]

//! Unit tests for the crate and its related test suites.

use super::BlockingLruCache;
use std::num::NonZeroUsize;

#[tokio::test(flavor = "multi_thread")]
async fn stores_and_retrieves_values() {
    let cache = BlockingLruCache::new(NonZeroUsize::new(2).expect("capacity"));

    assert!(cache.get(&"first").is_none());
    cache.insert("first", /*value*/ 1);
    assert_eq!(cache.get(&"first"), Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn evicts_least_recently_used() {
    let cache = BlockingLruCache::new(NonZeroUsize::new(2).expect("capacity"));
    cache.insert("a", /*value*/ 1);
    cache.insert("b", /*value*/ 2);
    assert_eq!(cache.get(&"a"), Some(1));

    cache.insert("c", /*value*/ 3);

    assert!(cache.get(&"b").is_none());
    assert_eq!(cache.get(&"a"), Some(1));
    assert_eq!(cache.get(&"c"), Some(3));
}

#[test]
fn disabled_without_runtime() {
    let cache = BlockingLruCache::new(NonZeroUsize::new(2).expect("capacity"));
    cache.insert("first", /*value*/ 1);
    assert!(cache.get(&"first").is_none());

    assert_eq!(cache.get_or_insert_with("first", || 2), 2);
    assert!(cache.get(&"first").is_none());

    assert!(cache.remove(&"first").is_none());
    cache.clear();

    let result = cache.with_mut(|inner| {
        inner.put("tmp", 3);
        inner.get(&"tmp").cloned()
    });
    assert_eq!(result, Some(3));
    assert!(cache.get(&"tmp").is_none());

    assert!(cache.blocking_lock().is_none());
}

#[path = "initialization_tests.rs"]
mod initialization_tests;
