#![cfg(test)]

use super::DiscoveryCacheKey;
use super::DiscoveryV2CacheEntry;
use super::ExecutorCapabilityDiscoveryV2Cache;
use crate::DiscoverV2CapabilitiesRequest;
use crate::DiscoverV2CapabilitiesResponse;
use crate::Environment;
use crate::ExecServerError;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::cell::Cell;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::time::Duration;
use tokio::sync::OnceCell;
use tokio::sync::oneshot;
use tokio::time::advance;

fn empty_cache_entry() -> DiscoveryV2CacheEntry {
    DiscoveryV2CacheEntry {
        key: DiscoveryCacheKey {
            environment: Weak::new(),
            session_id: None,
            cwd: PathUri::parse("file:///project").expect("test cwd"),
            sandbox: None,
        },
        force_reload: false,
        response: OnceCell::new(),
        failure: Mutex::new(None),
    }
}

#[tokio::test(start_paused = true)]
async fn repeated_failures_back_off_without_extending_cooldown_on_reads() {
    let entry = empty_cache_entry();
    let calls = Cell::new(0);
    let fail = || async {
        calls.set(calls.get() + 1);
        Err(ExecServerError::Protocol("discovery failed".to_string()))
    };
    for (attempt, delay) in [1, 2, 4, 8, 16, 30, 30].into_iter().enumerate() {
        let error = entry.get_or_try_init(fail).await.expect_err("failed load");
        assert_eq!(calls.get(), attempt + 1);
        advance(Duration::from_secs(delay) - Duration::from_millis(1)).await;
        let suppressed = entry.get_or_try_init(fail).await.expect_err("cooldown");
        assert!(Arc::ptr_eq(&error, &suppressed));
        assert_eq!(calls.get(), attempt + 1);
        // Suppressed lookups must not move the deadline or advance the failure count.
        advance(Duration::from_millis(1)).await;
    }

    let response = entry
        .get_or_try_init(|| async { Ok(DiscoverV2CapabilitiesResponse::default()) })
        .await
        .expect("recovery after cooldown");
    advance(Duration::from_secs(300)).await;
    let reused = entry.get_or_try_init(fail).await.expect("cached success");
    assert!(Arc::ptr_eq(&response, &reused));
    assert_eq!(calls.get(), 7);
}

#[tokio::test(start_paused = true)]
async fn queued_callers_share_failure_and_the_next_successful_attempt() {
    let entry = empty_cache_entry();
    let (release, wait) = oneshot::channel();
    let first = entry.get_or_try_init(|| async {
        wait.await.expect("release failed load");
        Err(ExecServerError::Protocol("discovery failed".to_string()))
    });
    tokio::pin!(first);
    assert!(futures::poll!(first.as_mut()).is_pending());
    let calls = Cell::new(0);
    let succeed = || async {
        calls.set(calls.get() + 1);
        Ok(DiscoverV2CapabilitiesResponse::default())
    };
    let queued = entry.get_or_try_init(succeed);
    tokio::pin!(queued);
    assert!(futures::poll!(queued.as_mut()).is_pending());
    release.send(()).expect("release failed load");
    let (first, queued) = tokio::join!(first, queued);
    assert!(Arc::ptr_eq(
        &first.expect_err("failed loader"),
        &queued.expect_err("queued caller observes cooldown"),
    ));
    assert_eq!(calls.get(), 0);

    advance(Duration::from_secs(1)).await;
    let (first, concurrent) = tokio::join!(
        entry.get_or_try_init(succeed),
        entry.get_or_try_init(succeed),
    );
    assert!(Arc::ptr_eq(
        &first.expect("recovered loader"),
        &concurrent.expect("shared success"),
    ));
    assert_eq!(calls.get(), 1);
}

#[tokio::test(start_paused = true)]
async fn cancellation_does_not_add_a_failure_or_cooldown() {
    let entry = empty_cache_entry();
    // Exercise cancellation both before any failure and after an expired failure.
    for delay in [1, 2] {
        let mut cancelled = Box::pin(entry.get_or_try_init(std::future::pending));
        assert!(futures::poll!(cancelled.as_mut()).is_pending());
        drop(cancelled);
        let attempted = Cell::new(false);
        entry
            .get_or_try_init(|| async {
                attempted.set(true);
                Err(ExecServerError::Protocol("discovery failed".to_string()))
            })
            .await
            .expect_err("fresh attempt after cancellation");
        assert!(attempted.get(), "cancellation must not start a cooldown");
        advance(Duration::from_secs(delay)).await;
    }
    entry
        .get_or_try_init(|| async { Ok(DiscoverV2CapabilitiesResponse::default()) })
        .await
        .expect("cancellation did not advance the backoff");
}

#[tokio::test]
async fn failed_contexts_are_isolated_and_invalidation_bypasses_cooldown() -> anyhow::Result<()> {
    // Local environments reject V2; their errors exercise the cache without a network fixture.
    let environment = Arc::new(Environment::create_for_tests(
        /*exec_server_url*/ None,
    )?);
    let replacement = Arc::new(Environment::create_for_tests(
        /*exec_server_url*/ None,
    )?);
    let cache = ExecutorCapabilityDiscoveryV2Cache::default();
    let request = DiscoverV2CapabilitiesRequest {
        force_reload: false,
        cwd: PathUri::parse("file:///project")?,
        sandbox: None,
    };
    let error = cache
        .discover("executor", &environment, request.clone())
        .await
        .expect_err("local environment rejects V2");
    let other_request = DiscoverV2CapabilitiesRequest {
        cwd: request.cwd.join("other")?,
        ..request.clone()
    };
    let other_error = cache
        .discover("executor", &environment, other_request.clone())
        .await
        .expect_err("independent cwd");
    let replacement_error = cache
        .discover("executor", &replacement, request.clone())
        .await
        .expect_err("independent environment");
    assert!(!Arc::ptr_eq(&error, &other_error));
    assert!(!Arc::ptr_eq(&error, &replacement_error));

    cache.invalidate_environment(&replacement);
    let suppressed = cache
        .discover("executor", &environment, request.clone())
        .await
        .expect_err("unrelated invalidation preserves cooldown");
    assert!(Arc::ptr_eq(&error, &suppressed));
    cache.invalidate_environment(&environment);
    let refreshed = cache
        .discover("executor", &environment, request)
        .await
        .expect_err("explicit refresh retries immediately");
    assert!(!Arc::ptr_eq(&error, &refreshed));
    cache.invalidate();
    let refreshed_other = cache
        .discover("executor", &environment, other_request)
        .await
        .expect_err("global invalidation retries immediately");
    assert!(!Arc::ptr_eq(&other_error, &refreshed_other));
    Ok(())
}
