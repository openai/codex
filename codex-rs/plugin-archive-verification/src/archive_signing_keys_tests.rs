#![cfg(test)]

use super::*;
use crate::test_support::auth_headers;
use crate::test_support::fixture;
use crate::test_support::key_request;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::future::Future;
use std::task::Context as TaskContext;
use std::task::Waker;
use tokio::sync::Barrier;
use tokio::sync::Notify;
use tokio::task::JoinSet;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn key_response() -> Value {
    fixture()["signing_keys_response"].clone()
}

#[tokio::test]
async fn reuses_fresh_keys_and_refetches_after_server_ttl_expiry() {
    for (ttl, expected_requests) in [(600, 1), (0, 2)] {
        let server = MockServer::start().await;
        let mut response = key_response();
        response["cache_ttl_seconds"] = json!(ttl);
        let expected: SigningKey = serde_json::from_value(response["keys"][0].clone()).unwrap();
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/signing-keys"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .expect(expected_requests)
            .mount(&server)
            .await;
        let base_url = format!("{}/backend-api", server.uri());
        let auth = auth_headers("Access Token", "account_id");
        let cache = SigningKeyCache::default();
        for _ in 0..2 {
            assert_eq!(
                cache
                    .key(&base_url, &expected.key_id, key_request(auth.clone()))
                    .await,
                Ok(expected.clone())
            );
        }
    }
}

#[tokio::test]
async fn fetches_once_per_lookup_for_an_unknown_key() {
    let server = MockServer::start().await;
    let response = key_response();
    let known_key: SigningKey = serde_json::from_value(response["keys"][0].clone()).unwrap();
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/signing-keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .mount(&server)
        .await;
    let base_url = format!("{}/backend-api", server.uri());
    let auth = auth_headers("Access Token", "account_id");
    let cache = SigningKeyCache::default();
    // The first lookup has no cache; the second has a fresh cache without the key.
    for expected_requests in 1..=2 {
        assert_eq!(
            cache
                .key(&base_url, "unknown-key", key_request(auth.clone()))
                .await,
            Err(ArchiveVerificationFailure::UnknownSigningKey)
        );
        assert_eq!(
            server.received_requests().await.unwrap().len(),
            expected_requests
        );
    }
    assert_eq!(
        cache
            .key(&base_url, &known_key.key_id, key_request(auth.clone()))
            .await,
        Ok(known_key.clone())
    );
    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/signing-keys"))
        .respond_with(ResponseTemplate::new(403))
        .expect(2)
        .mount(&server)
        .await;
    // The missing key forces a refresh; its denial also invalidates the previously known key.
    for key_id in ["unknown-key", &known_key.key_id] {
        assert_eq!(
            cache
                .key(&base_url, key_id, key_request(auth.clone()))
                .await,
            Err(ArchiveVerificationFailure::SigningKeysUnavailable)
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn consolidates_concurrent_fetches_only_for_the_same_credentials() {
    enum Credentials {
        Same,
        Different,
    }

    let valid = key_response();
    let key: SigningKey = serde_json::from_value(valid["keys"][0].clone()).unwrap();
    for (status, ttl, key_id, expected, credentials) in [
        (
            200,
            0,
            key.key_id.as_str(),
            Ok(key.clone()),
            Credentials::Same,
        ),
        (
            200,
            600,
            "unknown-key",
            Err(ArchiveVerificationFailure::UnknownSigningKey),
            Credentials::Same,
        ),
        (
            503,
            600,
            key.key_id.as_str(),
            Err(ArchiveVerificationFailure::SigningKeysUnavailable),
            Credentials::Same,
        ),
        (
            200,
            0,
            key.key_id.as_str(),
            Ok(key.clone()),
            Credentials::Different,
        ),
        (
            401,
            600,
            key.key_id.as_str(),
            Err(ArchiveVerificationFailure::SigningKeysUnavailable),
            Credentials::Different,
        ),
    ] {
        let server = MockServer::start().await;
        let mut response = valid.clone();
        response["cache_ttl_seconds"] = json!(ttl);
        let auth = auth_headers("Access Token", "account_id");
        let different_credentials = matches!(credentials, Credentials::Different);
        let first_auth = if different_credentials {
            auth_headers("e30.e30.b2xk", "account_id")
        } else {
            auth.clone()
        };
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/signing-keys"))
            .and(header(
                "authorization",
                first_auth[AUTHORIZATION].to_str().unwrap(),
            ))
            .respond_with(ResponseTemplate::new(status).set_body_json(response.clone()))
            .expect(1)
            .mount(&server)
            .await;
        if different_credentials {
            Mock::given(method("GET"))
                .and(path("/backend-api/ps/plugins/signing-keys"))
                .and(header("authorization", "Bearer Access Token"))
                .respond_with(ResponseTemplate::new(200).set_body_json(response))
                .expect(1)
                .mount(&server)
                .await;
        }
        let base_url = format!("{}/backend-api", server.uri());
        let cache = SigningKeyCache::default();
        // Hold each credential scope until all its callers have observed the same fetch generation.
        let first_scope = cache.scope(&base_url, &first_auth);
        let other_scope = cache.scope(&base_url, &auth);
        assert_eq!(
            Arc::ptr_eq(&first_scope, &other_scope),
            !different_credentials
        );
        let first_permit = first_scope
            .refresh
            .try_acquire()
            .expect("fresh credential scope");
        let other_permit = different_credentials.then(|| {
            other_scope
                .refresh
                .try_acquire()
                .expect("fresh credential scope")
        });
        let auths = std::iter::once(&first_auth).chain(std::iter::repeat_n(&auth, 7));
        let mut lookups: Vec<_> = auths
            .map(|auth| Box::pin(cache.key(&base_url, key_id, key_request(auth.clone()))))
            .collect();
        for lookup in &mut lookups {
            assert!(
                lookup
                    .as_mut()
                    .poll(&mut TaskContext::from_waker(Waker::noop()))
                    .is_pending()
            );
        }
        drop(other_permit);
        drop(first_permit);
        let mut expected_results = vec![expected; 8];
        if different_credentials {
            // The second caller gets its own successful response even if the first is rejected.
            expected_results[1..].fill(Ok(key.clone()));
        }
        for (lookup, expected) in lookups.into_iter().zip(expected_results) {
            assert_eq!(lookup.await, expected);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawned_waiters_share_completion_or_retry_a_canceled_fetch() {
    for (cancel_owner, expected_requests) in [(false, 1), (true, 2)] {
        tokio::time::timeout(Duration::from_secs(10), async {
            let server = MockServer::start().await;
            let mut response = key_response();
            response["cache_ttl_seconds"] = json!(0);
            let expected: SigningKey = serde_json::from_value(response["keys"][0].clone()).unwrap();
            let response = ResponseTemplate::new(200).set_body_json(response);
            let request_started = Arc::new(Notify::new());
            let started = Arc::clone(&request_started);
            let (release_response, released) = std::sync::mpsc::channel();
            let released = Mutex::new(released);
            let held_response = response.clone();
            Mock::given(method("GET"))
                .and(path("/backend-api/ps/plugins/signing-keys"))
                .respond_with(move |_: &wiremock::Request| {
                    started.notify_one();
                    // Wiremock runs its own server thread; dropping the sender also releases it.
                    let _ = released.lock().unwrap().recv();
                    held_response.clone()
                })
                .up_to_n_times(1)
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/backend-api/ps/plugins/signing-keys"))
                .respond_with(response)
                .expect(expected_requests - 1)
                .mount(&server)
                .await;

            let base_url = format!("{}/backend-api", server.uri());
            let auth = auth_headers("Access Token", "account_id");
            let cache = Arc::new(SigningKeyCache::default());
            let owner = tokio::spawn({
                let cache = Arc::clone(&cache);
                let base_url = base_url.clone();
                let auth = auth.clone();
                let key_id = expected.key_id.clone();
                async move {
                    cache
                        .key(&base_url, &key_id, key_request(auth.clone()))
                        .await
                }
            });
            request_started.notified().await;
            let queued = Arc::new(Barrier::new(9));
            let mut waiters = JoinSet::new();
            for _ in 0..8 {
                let cache = Arc::clone(&cache);
                let base_url = base_url.clone();
                let auth = auth.clone();
                let key_id = expected.key_id.clone();
                let queued = Arc::clone(&queued);
                waiters.spawn(async move {
                    let mut lookup =
                        std::pin::pin!(cache.key(&base_url, &key_id, key_request(auth.clone())));
                    // All waiters must observe the in-flight fetch before it completes or is canceled.
                    assert!(
                        lookup
                            .as_mut()
                            .poll(&mut TaskContext::from_waker(Waker::noop()))
                            .is_pending()
                    );
                    queued.wait().await;
                    lookup.await
                });
            }
            queued.wait().await;
            if cancel_owner {
                owner.abort();
                assert!(owner.await.unwrap_err().is_cancelled());
                release_response
                    .send(())
                    .expect("release canceled response");
            } else {
                release_response.send(()).expect("release first response");
                assert_eq!(owner.await.unwrap(), Ok(expected.clone()));
            }
            while let Some(result) = waiters.join_next().await {
                assert_eq!(result.unwrap(), Ok(expected.clone()));
            }
            server.verify().await;
        })
        .await
        .expect("spawned signing-key fetches should finish");
    }
}

#[tokio::test]
async fn canceled_refresh_leaves_waiter_free_to_fetch_new_keys() {
    let server = MockServer::start().await;
    let response = key_response();
    let expected: SigningKey = serde_json::from_value(response["keys"][0].clone()).unwrap();
    let base_url = format!("{}/backend-api", server.uri());
    let keys_path = "/backend-api/ps/plugins/signing-keys";
    let auth = auth_headers("Access Token", "account_id");
    let cache = SigningKeyCache::default();
    Mock::given(method("GET"))
        .and(path(keys_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        cache
            .key(&base_url, &expected.key_id, key_request(auth.clone()))
            .await,
        Ok(expected.clone())
    );
    server.verify().await;
    server.reset().await;

    let started = Arc::new(Notify::new());
    let request_started = Arc::clone(&started);
    Mock::given(method("GET"))
        .and(path(keys_path))
        .respond_with(move |_: &wiremock::Request| {
            request_started.notify_one();
            ResponseTemplate::new(403).set_delay(Duration::from_secs(60))
        })
        .with_priority(1)
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(keys_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .expect(1)
        .mount(&server)
        .await;

    let mut refresh = Box::pin(cache.key(&base_url, "unknown-key", key_request(auth.clone())));
    tokio::select! {
        () = started.notified() => {},
        result = refresh.as_mut() => panic!("refresh completed before cancellation: {result:?}"),
    }

    let mut waiter = Box::pin(cache.key(&base_url, &expected.key_id, key_request(auth)));
    assert!(
        waiter
            .as_mut()
            .poll(&mut TaskContext::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(refresh);
    assert_eq!(waiter.await, Ok(expected));
}

#[tokio::test]
async fn rejects_invalid_key_responses() {
    let valid = key_response();
    let mut eval_key = valid["keys"][0].clone();
    eval_key["purpose"] = json!(EVAL_ARCHIVE_PURPOSE);
    for response in [
        json!({"keys": [], "cache_ttl_seconds": 600}),
        json!({"keys": valid["keys"], "eval_keys": [eval_key], "cache_ttl_seconds": 600}),
        json!({"keys": valid["keys"], "cache_ttl_seconds": -1}),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/signing-keys"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .expect(1)
            .mount(&server)
            .await;
        let base_url = format!("{}/backend-api", server.uri());
        let auth = auth_headers("Access Token", "account_id");
        assert_eq!(
            SigningKeyCache::default()
                .key(
                    &base_url,
                    "codex-test-rsa-2048-v1",
                    key_request(auth.clone())
                )
                .await,
            Err(ArchiveVerificationFailure::InvalidSigningKeysResponse)
        );
    }
}

#[tokio::test]
async fn eval_key_cache_requires_the_same_endpoint_account_and_token() {
    let server = MockServer::start().await;
    let cache = SigningKeyCache::default();
    let mut keys = key_response();
    let mut eval_key = keys["keys"][0].clone();
    eval_key["key_id"] = json!("eval-key");
    eval_key["purpose"] = json!(EVAL_ARCHIVE_PURPOSE);
    keys["eval_keys"] = json!([eval_key]);
    Mock::given(method("GET"))
        .and(path("/first/ps/plugins/signing-keys"))
        .and(header("authorization", "Bearer e30.e30.b2xk"))
        .and(header("chatgpt-account-id", "employee"))
        .respond_with(ResponseTemplate::new(200).set_body_json(keys))
        .with_priority(/*p*/ 1)
        .expect(1)
        .mount(&server)
        .await;
    // Other callers and endpoints publish only the marketplace key.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(key_response()))
        .expect(3)
        .mount(&server)
        .await;
    let eval_key: SigningKey = serde_json::from_value(eval_key).unwrap();
    let unknown = ArchiveVerificationFailure::UnknownSigningKey;
    for (endpoint, account, token, expected) in [
        ("first", "employee", "e30.e30.b2xk", Ok(eval_key)),
        ("second", "employee", "e30.e30.b2xk", Err(unknown)),
        ("first", "other", "e30.e30.b2xk", Err(unknown)),
        ("first", "employee", "e30.e30.bmV3", Err(unknown)),
    ] {
        let base_url = format!("{}/{endpoint}", server.uri());
        let auth = auth_headers(token, account);
        assert_eq!(
            cache
                .key(&base_url, "eval-key", key_request(auth.clone()))
                .await,
            expected
        );
        if expected.is_ok() {
            assert_eq!(
                cache
                    .key(&base_url, "eval-key", |_| async {
                        Err(ArchiveVerificationFailure::AuthenticationRequired)
                    })
                    .await,
                Err(ArchiveVerificationFailure::AuthenticationRequired)
            );
        }
    }
}
