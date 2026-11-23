#![cfg(test)]

use super::*;
use crate::test_support::auth_headers;
use crate::test_support::fixture;
use crate::test_support::key_request;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::task::Context as TaskContext;
use std::task::Waker;
use tokio::sync::Notify;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
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
    for response in [
        json!({"keys": [], "cache_ttl_seconds": 600}),
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
