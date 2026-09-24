use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_keyring_store::DefaultKeyringStore;
use http::HeaderMap;
use keyring::Error as KeyringError;
use oauth2::AccessToken;
use oauth2::RefreshToken;
use oauth2::TokenResponse;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::OAuthState;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use tokio::sync::Mutex as TokioMutex;
use tracing::Event;
use tracing::Id;
use tracing::Metadata;
use tracing::Subscriber;
use tracing::span::Attributes;
use tracing::span::Record;
use tracing::subscriber::Interest;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::MockKeyringStore;
use super::TempCodexHome;
use super::assert_tokens_match_without_expiry;
use super::sample_tokens;
use crate::http_client_adapter::StreamableHttpClientAdapter;
use crate::http_client_adapter::StreamableHttpRedirectMode;
use crate::oauth::OAuthCredentialStore;
use crate::oauth::OAuthPersistor;
use crate::oauth::OAuthRuntime;
use crate::oauth::ResolvedOAuthCredentialStore;
use crate::oauth::StoredOAuthTokens;
use crate::oauth::WrappedOAuthTokenResponse;
use crate::oauth::compute_expires_at_millis;
use crate::oauth::compute_store_key;
use crate::oauth::delete_oauth_tokens;
use crate::oauth::load_oauth_tokens_from_file;
use crate::oauth::refresh_lock::RefreshCredentialLock;
use crate::oauth::save_oauth_tokens;
use crate::oauth::save_oauth_tokens_to_file;
use crate::oauth::stored_oauth_credentials;
use crate::oauth_http_client::OAuthHttpClientAdapter;
use crate::oauth_http_client::PROACTIVE_REFRESH_TIMEOUT;
use crate::startup_error::is_authentication_required_error;

const REFRESH_LOCK_CONTENTION_EVENT_TARGET: &str =
    "codex_rmcp_client::oauth::refresh_lock::contention";

#[tokio::test(flavor = "current_thread")]
async fn login_and_logout_follow_the_completed_refresh() -> Result<()> {
    let (_env, _server, initial) = test_context().await?;
    let mut replacement = initial.clone();
    replacement
        .token_response
        .0
        .set_access_token(AccessToken::new("replacement-access-token".into()));
    replacement
        .token_response
        .0
        .set_refresh_token(Some(RefreshToken::new("replacement-refresh-token".into())));
    let mut refreshed = initial.clone();
    refreshed
        .token_response
        .0
        .set_refresh_token(Some(RefreshToken::new("rotated-refresh-token".into())));

    for expected in [Some(replacement), None] {
        save_oauth_tokens_to_file(&initial)?;
        let held_lock =
            RefreshCredentialLock::acquire_for_server(&initial.server_name, &initial.url).await?;
        let (contended_tx, contended_rx) = mpsc::channel();
        let _subscriber_guard =
            tracing::subscriber::set_default(LockContentionSubscriber { contended_tx });
        let mutation = tokio::spawn({
            let initial = initial.clone();
            let expected = expected.clone();
            async move {
                match expected {
                    Some(tokens) => {
                        save_oauth_tokens(
                            &tokens.server_name,
                            &tokens,
                            OAuthCredentialsStoreMode::File,
                            AuthKeyringBackendKind::Direct,
                        )
                        .await
                    }
                    None => {
                        assert!(
                            delete_oauth_tokens(
                                &initial.server_name,
                                &initial.url,
                                OAuthCredentialsStoreMode::File,
                                AuthKeyringBackendKind::Direct,
                            )
                            .await?
                        );
                        Ok(())
                    }
                }
            }
        });
        wait_for_lock_contention(contended_rx, /*expected_count*/ 1).await?;
        assert!(!mutation.is_finished());
        assert_eq!(
            load_oauth_tokens_from_file(&initial.server_name, &initial.url)?,
            Some(initial.clone())
        );
        // Complete the in-flight refresh before allowing the login or logout to write.
        save_oauth_tokens_to_file(&refreshed)?;
        drop(held_lock);
        mutation.await??;
        assert_eq!(
            load_oauth_tokens_from_file(&initial.server_name, &initial.url)?,
            expected
        );
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn fresh_preparation_skips_storage_and_refresh_lock() -> Result<()> {
    let (env, server, mut initial) = test_context().await?;
    let fresh = sample_tokens();
    initial.token_response = fresh.token_response;
    initial.expires_at = fresh.expires_at;
    std::fs::create_dir(env.path().join(".credentials.json"))?;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    for mode in [
        crate::McpOAuthRefreshMode::Legacy,
        crate::McpOAuthRefreshMode::Coordinated,
    ] {
        let runtime = runtime_for(&initial, mode).await?;
        let held_lock =
            RefreshCredentialLock::acquire_for_server(&initial.server_name, &initial.url).await?;
        tokio::time::timeout(
            Duration::from_millis(/*millis*/ 100),
            runtime.refresh_if_needed(),
        )
        .await
        .context("fresh preparation must not wait for the refresh lock")??;
        drop(held_lock);
    }
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn coordinated_401_refresh_rereads_and_persists_before_retry() -> Result<()> {
    let (_env, server, mut initial) = test_context().await?;
    let fresh = sample_tokens();
    initial.token_response = fresh.token_response;
    initial.expires_at = fresh.expires_at;
    save_oauth_tokens_to_file(&initial)?;
    let mut manager = authorization_manager_for(&initial).await?;
    manager.set_credential_store(OAuthCredentialStore::new(
        initial.clone(),
        ResolvedOAuthCredentialStore::File,
        DefaultKeyringStore,
        /*oauth_config*/ None,
    ));
    let client = AuthClient::new(
        StreamableHttpClientAdapter::new(
            Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                OutboundProxyPolicy::ReqwestDefault,
            ))),
            HeaderMap::new(),
            /*auth_provider*/ None,
            /*has_configured_headers*/ false,
            StreamableHttpRedirectMode::Legacy,
            Arc::default(),
        ),
        manager,
    );
    let mut durable = initial.clone();
    durable
        .token_response
        .0
        .set_access_token(AccessToken::new("durable-access-token".into()));
    durable
        .token_response
        .0
        .set_refresh_token(Some(RefreshToken::new("durable-refresh-token".into())));
    save_oauth_tokens_to_file(&durable)?;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("authorization", "Bearer access-token"))
        .respond_with(ResponseTemplate::new(401).insert_header("www-authenticate", "Bearer"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("refresh_token=durable-refresh-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "refreshed-access-token",
            "refresh_token": "rotated-refresh-token",
            "token_type": "Bearer",
            "expires_in": 3600,
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(header("authorization", "Bearer refreshed-access-token"))
        .respond_with(move |_: &wiremock::Request| {
            let saved = load_oauth_tokens_from_file(&durable.server_name, &durable.url)
                .expect("read persisted refresh before retry")
                .expect("refresh must be persisted before retry");
            let mut expected = durable.token_response.0.clone();
            expected.set_access_token(AccessToken::new("refreshed-access-token".into()));
            expected.set_refresh_token(Some(RefreshToken::new("rotated-refresh-token".into())));
            expected.set_expires_in(saved.token_response.0.expires_in().as_ref());
            assert_eq!(saved.token_response, WrappedOAuthTokenResponse(expected));
            ResponseTemplate::new(202)
        })
        .expect(1)
        .mount(&server)
        .await;
    client
        .post_message(
            initial.url.into(),
            serde_json::from_value(
                serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}),
            )?,
            /*session_id*/ None,
            /*auth_token*/ None,
            Default::default(),
        )
        .await?;
    server.verify().await;
    Ok(())
}

struct LockContentionSubscriber {
    contended_tx: mpsc::Sender<()>,
}

impl Subscriber for LockContentionSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target() == REFRESH_LOCK_CONTENTION_EVENT_TARGET
    }

    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if self.enabled(metadata) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        Some(tracing::level_filters::LevelFilter::DEBUG)
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(/*u*/ 1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows_from: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if self.enabled(event.metadata()) {
            self.contended_tx
                .send(())
                .expect("signal actual OAuth credential-lock contention");
        }
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[tokio::test(flavor = "current_thread")]
async fn concurrent_refreshes_call_provider_once_and_carry_omitted_fields() -> Result<()> {
    assert_concurrent_refreshes(crate::McpOAuthRefreshMode::Legacy).await?;
    assert_concurrent_refreshes(crate::McpOAuthRefreshMode::Coordinated).await
}

async fn assert_concurrent_refreshes(mode: crate::McpOAuthRefreshMode) -> Result<()> {
    let (_env, server, initial) = test_context().await?;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=refresh-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "refreshed-access-token",
            "token_type": "Bearer",
            "expires_in": 3600,
        })))
        .expect(1)
        .mount(&server)
        .await;
    save_oauth_tokens_to_file(&initial)?;

    // Hold the real credential lock until both refresh transactions report WouldBlock. This makes
    // the lock assertion independent of task scheduling and ensures removing transaction locking
    // makes the test fail before either request can reach the provider.
    let held_lock =
        RefreshCredentialLock::acquire_for_server(&initial.server_name, &initial.url).await?;
    let (contended_tx, contended_rx) = mpsc::channel();
    let _subscriber_guard =
        tracing::subscriber::set_default(LockContentionSubscriber { contended_tx });

    let first = runtime_for(&initial, mode).await?;
    let second = runtime_for(&initial, mode).await?;
    let first_task = tokio::spawn({
        let first = first.clone();
        async move { first.refresh_if_needed().await }
    });
    let second_task = tokio::spawn({
        let second = second.clone();
        async move { second.refresh_if_needed().await }
    });

    wait_for_lock_contention(contended_rx, /*expected_count*/ 2).await?;
    drop(held_lock);
    first_task.await??;
    second_task.await??;
    server.verify().await;

    // Layer 2 still invokes the legacy RMCP persistence hook after operations. Exercise that hook
    // so a raw provider response that omitted refresh token/scopes cannot overwrite the merged
    // authoritative credential.
    if let OAuthRuntime::Legacy(persistor) = &first {
        persistor.persist_if_needed().await?;
    }
    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
        .expect("refreshed credentials should be stored");
    assert_eq!(stored.issuer, initial.issuer);
    let live_credentials = match &first {
        OAuthRuntime::Legacy(persistor) => persistor.stored_credentials().await,
        OAuthRuntime::Coordinated { store, .. } => store.stored_credentials().await,
    };
    let disk_credentials = stored_oauth_credentials(
        &initial.server_name,
        &initial.url,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::Direct,
    )?;
    assert_eq!(live_credentials, disk_credentials);
    let mut expected_response = initial.token_response.0.clone();
    expected_response.set_access_token(AccessToken::new("refreshed-access-token".to_string()));
    // File loads derive `expires_in` from stable `expires_at`, so it may tick down before this
    // assertion. Normalize only that derived field and compare the complete token response so
    // omitted refresh-token and scope carry-forward remain covered.
    expected_response.set_expires_in(stored.token_response.0.expires_in().as_ref());
    assert_eq!(
        stored.token_response,
        WrappedOAuthTokenResponse(expected_response)
    );
    Ok(())
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "AuthorizationManager async access must be serialized through its Tokio mutex"
)]
#[tokio::test(flavor = "current_thread")]
async fn resolved_keyring_read_error_preserves_in_memory_credentials() -> Result<()> {
    let (_env, _server, initial) = test_context().await?;
    let keyring_store = MockKeyringStore::default();
    let key = compute_store_key(&initial.server_name, &initial.url)?;
    keyring_store.set_error(&key, KeyringError::Invalid("error".into(), "load".into()));
    let manager = Arc::new(TokioMutex::new(authorization_manager_for(&initial).await?));
    let persistor = OAuthPersistor::new(
        initial.server_name.clone(),
        initial.url.clone(),
        Arc::clone(&manager),
        ResolvedOAuthCredentialStore::Keyring(AuthKeyringBackendKind::Direct),
        Some(initial.clone()),
        /*oauth_config*/ None,
    );

    let error = persistor
        .refresh_if_needed_in(&keyring_store, Duration::from_secs(/*secs*/ 45))
        .await
        .expect_err("the resolved keyring read error should abort refresh");
    assert!(
        error
            .to_string()
            .contains("failed to reread OAuth tokens from resolved keyring storage"),
        "unexpected error: {error:#}"
    );
    let guard = manager.lock().await;
    let (_client_id, token_response) = guard.get_credentials().await?;
    assert_eq!(
        WrappedOAuthTokenResponse(token_response.expect("manager should retain credentials")),
        initial.token_response
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn missing_authoritative_credentials_require_reauthorization() -> Result<()> {
    let (_env, _server, initial) = test_context().await?;
    let persistor = persistor_for(&initial).await?;

    let error = persistor
        .refresh_if_needed()
        .await
        .expect_err("a removed authoritative credential should abort refresh");
    assert!(error.chain().any(|source| matches!(
        source.downcast_ref::<AuthError>(),
        Some(AuthError::AuthorizationRequired)
    )));
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn rejected_refresh_token_requires_reauthorization() -> Result<()> {
    let (_env, server, initial) = test_context().await?;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=refresh-token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant",
            "error_description": "refresh token expired or revoked",
        })))
        .expect(1)
        .mount(&server)
        .await;
    save_oauth_tokens_to_file(&initial)?;
    let persistor = persistor_for(&initial).await?;

    let error = persistor
        .refresh_if_needed()
        .await
        .expect_err("a provider-rejected refresh token should require reauthorization");
    assert!(is_authentication_required_error(&error));
    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
        .expect("rejected refresh must preserve the durable credentials");
    assert_tokens_match_without_expiry(&stored, &initial);
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn changed_issuer_requires_reauthorization_before_refresh() -> Result<()> {
    let (_env, server, mut initial) = test_context().await?;
    initial.issuer = Some("https://original-issuer.example.test".to_string());
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    save_oauth_tokens_to_file(&initial)?;
    let persistor = persistor_for(&initial).await?;

    let error = persistor
        .refresh_if_needed()
        .await
        .expect_err("a changed issuer must abort refresh");
    assert!(is_authentication_required_error(&error));
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn missing_issuer_requires_reauthorization_before_refresh() -> Result<()> {
    let (_env, server, mut initial) = test_context().await?;
    initial.issuer = None;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    save_oauth_tokens_to_file(&initial)?;
    let persistor = persistor_for(&initial).await?;

    let error = persistor
        .refresh_if_needed()
        .await
        .expect_err("a missing issuer must abort refresh");
    assert!(is_authentication_required_error(&error));
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn issuerless_newer_credentials_are_not_adopted_before_refresh() -> Result<()> {
    let (_env, server, initial) = test_context().await?;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    save_oauth_tokens_to_file(&initial)?;
    let persistor = persistor_for(&initial).await?;

    let mut latest = initial.clone();
    latest.issuer = None;
    latest.expires_at = Some(u64::MAX);
    save_oauth_tokens_to_file(&latest)?;

    let error = persistor
        .refresh_if_needed()
        .await
        .expect_err("an issuer-less refresh token must not be adopted");
    assert!(is_authentication_required_error(&error));
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn proactive_refresh_failure_with_unexpired_token_does_not_require_reauthorization()
-> Result<()> {
    let (_env, server, mut initial) = test_context().await?;
    initial
        .token_response
        .0
        .set_expires_in(Some(&Duration::from_secs(/*secs*/ 30)));
    initial.expires_at = compute_expires_at_millis(&initial.token_response.0);
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=refresh-token"))
        .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({
            "error": "temporarily_unavailable",
            "error_description": "provider is temporarily unavailable",
        })))
        .expect(1)
        .mount(&server)
        .await;
    save_oauth_tokens_to_file(&initial)?;
    let persistor = persistor_for(&initial).await?;

    let error = persistor
        .refresh_if_needed()
        .await
        .expect_err("a transient provider failure should not erase valid credentials");
    assert!(!is_authentication_required_error(&error));
    assert!(error.chain().any(|source| matches!(
        source.downcast_ref::<AuthError>(),
        Some(AuthError::TokenRefreshFailed(_))
    )));
    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
        .expect("a transient refresh failure must preserve durable credentials");
    assert_tokens_match_without_expiry(&stored, &initial);
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn caller_cancellation_does_not_cancel_refresh_persistence() -> Result<()> {
    assert_caller_cancellation(crate::McpOAuthRefreshMode::Legacy).await?;
    assert_caller_cancellation(crate::McpOAuthRefreshMode::Coordinated).await
}

async fn assert_caller_cancellation(mode: crate::McpOAuthRefreshMode) -> Result<()> {
    let (_env, server, initial) = test_context().await?;
    let (request_received_tx, request_received_rx) = mpsc::channel();
    let (release_response_tx, release_response_rx) = mpsc::channel();
    let release_response_rx = Arc::new(std::sync::Mutex::new(release_response_rx));
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=refresh-token"))
        .respond_with(move |_request: &wiremock::Request| {
            request_received_tx
                .send(())
                .expect("signal OAuth refresh request");
            release_response_rx
                .lock()
                .expect("lock OAuth refresh response gate")
                .recv()
                .expect("wait to release OAuth refresh response");
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "cancel-safe-access-token",
                "token_type": "Bearer",
                "expires_in": 3600,
            }))
        })
        .expect(1)
        .mount(&server)
        .await;
    save_oauth_tokens_to_file(&initial)?;
    let runtime = runtime_for(&initial, mode).await?;
    let caller = tokio::spawn(async move { runtime.refresh_if_needed().await });

    tokio::task::spawn_blocking(move || {
        request_received_rx
            .recv_timeout(Duration::from_secs(/*secs*/ 5))
            .context("timed out waiting for OAuth refresh request")
    })
    .await??;
    caller.abort();
    assert!(
        caller
            .await
            .expect_err("caller should be cancelled")
            .is_cancelled()
    );

    release_response_tx
        .send(())
        .context("release OAuth refresh response")?;

    // Reacquiring the same credential lock waits for the detached refresh task to persist and
    // release it, avoiding a scheduler-sensitive sleep after cancellation.
    let _lock = tokio::time::timeout(
        Duration::from_secs(/*secs*/ 2),
        RefreshCredentialLock::acquire_for_server(&initial.server_name, &initial.url),
    )
    .await
    .context("detached refresh did not release its credential lock")??;
    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
        .expect("detached refresh should persist credentials");
    assert_eq!(
        stored.token_response.0.access_token().secret(),
        "cancel-safe-access-token"
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn failed_refresh_adopts_login_completed_during_request() -> Result<()> {
    for expires_in in [Some(Duration::from_secs(/*secs*/ 3600)), None] {
        let (_env, server, initial) = test_context().await?;
        let mut replacement = initial.clone();
        replacement.client_id = "new-login-client".to_string();
        replacement
            .token_response
            .0
            .set_access_token(AccessToken::new("new-login-access-token".to_string()));
        replacement
            .token_response
            .0
            .set_expires_in(expires_in.as_ref());
        replacement.expires_at = compute_expires_at_millis(&replacement.token_response.0);
        save_oauth_tokens_to_file(&initial)?;
        let persistor = persistor_for(&initial).await?;
        let provider_replacement = replacement.clone();
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(move |_request: &wiremock::Request| {
                // A browser login writes independently of the refresh transaction lock.
                save_oauth_tokens_to_file(&provider_replacement).expect("complete new login");
                ResponseTemplate::new(200).set_body_string("not an OAuth token response")
            })
            .expect(1)
            .mount(&server)
            .await;

        persistor.refresh_if_needed().await?;
        let live = persistor.stored_credentials().await.expect("adopted login");
        let mut expected_live = replacement.clone();
        expected_live.token_response.0.set_expires_in(None);
        assert_eq!(live, expected_live);
        persistor.persist_if_needed().await?;
        let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
            .expect("new login must remain stored");
        assert_tokens_match_without_expiry(&stored, &replacement);
        server.verify().await;
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn failed_refresh_does_not_adopt_unbound_replacement_credentials() -> Result<()> {
    for issuer in [None, Some("https://different-issuer.example.test")] {
        let (_env, server, initial) = test_context().await?;
        let mut replacement = initial.clone();
        replacement.issuer = issuer.map(str::to_string);
        replacement.expires_at = Some(u64::MAX);
        replacement
            .token_response
            .0
            .set_access_token(AccessToken::new("replacement-access-token".to_string()));
        save_oauth_tokens_to_file(&initial)?;
        let persistor = persistor_for(&initial).await?;
        let provider_replacement = replacement.clone();
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(move |_request: &wiremock::Request| {
                save_oauth_tokens_to_file(&provider_replacement).expect("complete new login");
                ResponseTemplate::new(200).set_body_string("invalid JSON")
            })
            .expect(1)
            .mount(&server)
            .await;

        let error = persistor
            .refresh_if_needed()
            .await
            .expect_err("an unbound replacement must not enter the authorization manager");
        assert!(is_authentication_required_error(&error));
        let live = persistor
            .stored_credentials()
            .await
            .expect("original credentials");
        let mut expected_live = initial.clone();
        expected_live.token_response.0.set_expires_in(None);
        assert_eq!(live, expected_live);
        let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
            .expect("the replacement must not be deleted or overwritten");
        assert_tokens_match_without_expiry(&stored, &replacement);
        server.verify().await;
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn expired_refresh_failure_preserves_credentials_for_a_later_retry() -> Result<()> {
    let (_env, server, initial) = test_context().await?;
    save_oauth_tokens_to_file(&initial)?;
    let persistor = persistor_for(&initial).await?;
    let failure = Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("invalid JSON"))
        .expect(1)
        .mount_as_scoped(&server)
        .await;

    let error = persistor
        .refresh_if_needed()
        .await
        .expect_err("refresh failed");
    assert!(is_authentication_required_error(&error));
    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
        .expect("failed refresh must preserve stored credentials");
    assert_tokens_match_without_expiry(&stored, &initial);
    drop(failure);
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("refresh_token=refresh-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "retry-access-token",
            "token_type": "Bearer",
            "expires_in": 3600,
        })))
        .expect(1)
        .mount(&server)
        .await;

    persistor.refresh_if_needed().await?;
    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
        .expect("successful retry must persist new credentials");
    assert_eq!(
        stored.token_response.0.access_token().secret(),
        "retry-access-token"
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn provider_timeout_releases_lock_and_preserves_durable_credentials() -> Result<()> {
    let (_env, server, initial) = test_context().await?;
    mount_delayed_refresh(&server, "late-access-token").await;
    save_oauth_tokens_to_file(&initial)?;
    let persistor = persistor_for(&initial).await?;

    let error = persistor
        .refresh_if_needed_in(
            &MockKeyringStore::default(),
            Duration::from_millis(/*millis*/ 50),
        )
        .await
        .expect_err("provider request should reach its explicit timeout");
    assert!(error.to_string().contains("timed out after 50ms"));
    assert!(is_authentication_required_error(&error));

    let _lock = tokio::time::timeout(
        Duration::from_millis(/*millis*/ 100),
        RefreshCredentialLock::acquire_for_server(&initial.server_name, &initial.url),
    )
    .await
    .context("provider timeout did not release the credential lock")??;
    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
        .expect("timed-out refresh must leave durable credentials present");
    assert_tokens_match_without_expiry(&stored, &initial);
    server.verify().await;
    Ok(())
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "AuthorizationManager async access must be serialized through its Tokio mutex"
)]
#[tokio::test(flavor = "current_thread")]
async fn coordinated_provider_timeout_excludes_lock_wait() -> Result<()> {
    let (_env, server, initial) = test_context().await?;
    mount_delayed_refresh(&server, "late-access-token").await;
    save_oauth_tokens_to_file(&initial)?;
    let (auth_manager, _) = coordinated_manager_for(&initial).await?;
    let held_lock =
        RefreshCredentialLock::acquire_for_server(&initial.server_name, &initial.url).await?;
    let (contended_tx, contended_rx) = mpsc::channel();
    let _subscriber_guard =
        tracing::subscriber::set_default(LockContentionSubscriber { contended_tx });
    let mut refresh = tokio::spawn(
        PROACTIVE_REFRESH_TIMEOUT.scope(Duration::from_millis(/*millis*/ 50), async move {
            auth_manager.lock().await.get_access_token().await
        }),
    );
    wait_for_lock_contention(contended_rx, /*expected_count*/ 1).await?;
    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut refresh)
            .await
            .is_err(),
        "lock wait must not consume the provider timeout"
    );
    drop(held_lock);
    let error = refresh
        .await?
        .expect_err("the scoped provider request must time out");
    assert!(matches!(error, AuthError::TokenRefreshFailed(_)));
    let _lock = tokio::time::timeout(
        Duration::from_millis(/*millis*/ 100),
        RefreshCredentialLock::acquire_for_server(&initial.server_name, &initial.url),
    )
    .await??;
    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?.unwrap();
    assert_tokens_match_without_expiry(&stored, &initial);
    server.verify().await;
    Ok(())
}

async fn runtime_for(
    tokens: &StoredOAuthTokens,
    mode: crate::McpOAuthRefreshMode,
) -> Result<OAuthRuntime> {
    Ok(match mode {
        crate::McpOAuthRefreshMode::Legacy => OAuthRuntime::Legacy(persistor_for(tokens).await?),
        crate::McpOAuthRefreshMode::Coordinated => {
            let (auth_manager, store) = coordinated_manager_for(tokens).await?;
            OAuthRuntime::Coordinated {
                auth_manager,
                store,
            }
        }
    })
}

async fn coordinated_manager_for(
    tokens: &StoredOAuthTokens,
) -> Result<(Arc<TokioMutex<AuthorizationManager>>, OAuthCredentialStore)> {
    let mut manager = authorization_manager_for(tokens).await?;
    let store = OAuthCredentialStore::new(
        tokens.clone(),
        ResolvedOAuthCredentialStore::File,
        DefaultKeyringStore,
        /*oauth_config*/ None,
    );
    manager.set_credential_store(store.clone());
    Ok((Arc::new(TokioMutex::new(manager)), store))
}

async fn persistor_for(tokens: &StoredOAuthTokens) -> Result<OAuthPersistor> {
    Ok(OAuthPersistor::new(
        tokens.server_name.clone(),
        tokens.url.clone(),
        Arc::new(TokioMutex::new(authorization_manager_for(tokens).await?)),
        ResolvedOAuthCredentialStore::File,
        Some(tokens.clone()),
        /*oauth_config*/ None,
    ))
}

async fn test_context() -> Result<(TempCodexHome, MockServer, StoredOAuthTokens)> {
    let env = TempCodexHome::new();
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    let tokens = expired_tokens(&format!("{}/mcp", server.uri()));
    Ok((env, server, tokens))
}

async fn authorization_manager_for(tokens: &StoredOAuthTokens) -> Result<AuthorizationManager> {
    let oauth_http_client = Arc::new(OAuthHttpClientAdapter::new(
        Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        HeaderMap::new(),
        &tokens.url,
    ));
    let mut state =
        OAuthState::new_with_oauth_http_client(tokens.url.clone(), oauth_http_client).await?;
    state
        .set_credentials(&tokens.client_id, tokens.token_response.0.clone())
        .await?;
    let manager = match state {
        OAuthState::Authorized(manager) | OAuthState::Unauthorized(manager) => manager,
        OAuthState::Session(_) | OAuthState::AuthorizedHttpClient(_) => {
            anyhow::bail!("unexpected OAuth state")
        }
        _ => anyhow::bail!("unexpected OAuth state"),
    };
    Ok(manager)
}

async fn mount_oauth_metadata(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issuer": format!("{}/mcp", server.uri()),
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": ["scope-a", "scope-b"],
        })))
        .mount(server)
        .await;
}

async fn mount_delayed_refresh(server: &MockServer, response_access_token: &str) {
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=refresh-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(/*millis*/ 200))
                .set_body_json(serde_json::json!({
                    "access_token": response_access_token,
                    "token_type": "Bearer",
                    "expires_in": 3600,
                })),
        )
        .expect(1)
        .mount(server)
        .await;
}

async fn wait_for_lock_contention(rx: mpsc::Receiver<()>, expected_count: usize) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        for _ in 0..expected_count {
            rx.recv_timeout(Duration::from_secs(/*secs*/ 5))
                .context("timed out waiting for lock contention")?;
        }
        Ok(())
    })
    .await?
}

fn expired_tokens(url: &str) -> StoredOAuthTokens {
    let mut tokens = sample_tokens();
    tokens.url = url.to_string();
    tokens.issuer = Some(url.to_string());
    tokens.expires_at = Some(0);
    tokens
        .token_response
        .0
        .set_expires_in(Some(&Duration::ZERO));
    tokens
}
