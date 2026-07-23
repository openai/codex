use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use keyring::Error as KeyringError;
use oauth2::AccessToken;
use oauth2::TokenResponse;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationManager;
use rmcp::transport::auth::OAuthState;
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
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::MockKeyringStore;
use super::TempCodexHome;
use super::assert_tokens_match_without_expiry;
use super::sample_tokens;
use crate::oauth::OAuthPersistor;
use crate::oauth::ResolvedOAuthCredentialStore;
use crate::oauth::StoredOAuthTokens;
use crate::oauth::WrappedOAuthTokenResponse;
use crate::oauth::compute_store_key;
use crate::oauth::load_oauth_tokens_from_file;
use crate::oauth::refresh_lock::RefreshCredentialLock;
use crate::oauth::save_oauth_tokens_to_file;
use crate::oauth::stored_oauth_credentials;
use crate::startup_error::is_authentication_required_error;

const REFRESH_LOCK_CONTENTION_EVENT_TARGET: &str =
    "codex_rmcp_client::oauth::refresh_lock::contention";

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

    let first = persistor_for(&initial).await?;
    let second = persistor_for(&initial).await?;
    let initial_credentials = first.stored_credentials().await;
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
    first.persist_if_needed().await?;
    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
        .expect("refreshed credentials should be stored");
    let live_credentials = first.stored_credentials().await;
    let disk_credentials = stored_oauth_credentials(
        &initial.server_name,
        &initial.url,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::Direct,
    )?;
    assert_eq!(live_credentials, disk_credentials);
    assert_ne!(live_credentials, initial_credentials);
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
    let manager = authorization_manager_for(&initial).await?;
    let persistor = OAuthPersistor::new(
        initial.server_name.clone(),
        initial.url.clone(),
        Arc::clone(&manager),
        ResolvedOAuthCredentialStore::Keyring(AuthKeyringBackendKind::Direct),
        Some(initial.clone()),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn caller_cancellation_does_not_cancel_refresh_persistence() -> Result<()> {
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
    let persistor = persistor_for(&initial).await?;
    let caller = tokio::spawn({
        let persistor = persistor.clone();
        async move { persistor.refresh_if_needed().await }
    });

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

async fn persistor_for(tokens: &StoredOAuthTokens) -> Result<OAuthPersistor> {
    Ok(OAuthPersistor::new(
        tokens.server_name.clone(),
        tokens.url.clone(),
        authorization_manager_for(tokens).await?,
        ResolvedOAuthCredentialStore::File,
        Some(tokens.clone()),
    ))
}

async fn test_context() -> Result<(TempCodexHome, MockServer, StoredOAuthTokens)> {
    let env = TempCodexHome::new();
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    let tokens = expired_tokens(&format!("{}/mcp", server.uri()));
    Ok((env, server, tokens))
}

async fn authorization_manager_for(
    tokens: &StoredOAuthTokens,
) -> Result<Arc<TokioMutex<AuthorizationManager>>> {
    let mut state = OAuthState::new(tokens.url.clone(), Some(reqwest::Client::new())).await?;
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
    Ok(Arc::new(TokioMutex::new(manager)))
}

async fn mount_oauth_metadata(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
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
    tokens.expires_at = Some(0);
    tokens
        .token_response
        .0
        .set_expires_in(Some(&Duration::ZERO));
    tokens
}
