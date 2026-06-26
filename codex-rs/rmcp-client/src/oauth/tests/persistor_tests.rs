use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use keyring::Error as KeyringError;
use oauth2::AccessToken;
use oauth2::TokenResponse;
use pretty_assertions::assert_eq;
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
use super::sample_tokens;
use crate::oauth::OAuthPersistor;
use crate::oauth::ResolvedOAuthCredentialStore;
use crate::oauth::StoredOAuthTokens;
use crate::oauth::WrappedOAuthTokenResponse;
use crate::oauth::compute_store_key;
use crate::oauth::delete_oauth_tokens_locked;
use crate::oauth::fallback_file_path;
use crate::oauth::load_oauth_tokens_from_file;
use crate::oauth::load_oauth_tokens_from_keyring;
use crate::oauth::refresh_lock::RefreshCredentialLock;
use crate::oauth::refresh_transaction::RefreshReason;
use crate::oauth::save_oauth_tokens_to_file;
use crate::oauth::save_oauth_tokens_with_keyring;

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
    let _env = TempCodexHome::new();
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    let refresh_started = mount_delayed_refresh(&server, "refreshed-access-token").await;
    let initial = expired_tokens(&format!("{}/mcp", server.uri()));
    save_oauth_tokens_to_file(&initial)?;

    let first = persistor_for(&initial).await?;
    let second = persistor_for(&initial).await?;
    let first_task = tokio::spawn({
        let first = first.clone();
        async move { first.refresh_if_needed().await }
    });
    wait_for_signal(refresh_started).await?;
    let second_task = tokio::spawn({
        let second = second.clone();
        async move { second.refresh_if_needed().await }
    });

    first_task.await??;
    second_task.await??;
    server.verify().await;

    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
        .expect("refreshed credentials should be stored");
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

#[tokio::test(flavor = "current_thread")]
async fn delayed_unauthorized_retries_adopt_the_winning_token() -> Result<()> {
    let _env = TempCodexHome::new();
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    let _refresh_started = mount_delayed_refresh(&server, "refreshed-access-token").await;
    let mut initial = expired_tokens(&format!("{}/mcp", server.uri()));
    initial.expires_at = None;
    initial.token_response.0.set_expires_in(None);
    save_oauth_tokens_to_file(&initial)?;

    let first = persistor_for(&initial).await?;
    let second_manager = authorization_manager_for(&initial).await?;
    let second = OAuthPersistor::new(
        initial.server_name.clone(),
        initial.url.clone(),
        Arc::clone(&second_manager),
        ResolvedOAuthCredentialStore::File,
        Some(initial.clone()),
    );
    let rejected_access_token = initial.token_response.0.access_token().clone();

    first
        .refresh_after_unauthorized(rejected_access_token.clone())
        .await?;
    // Both calls model requests that left with A. Once the first 401 rotates A to B, later 401s
    // must adopt B and retry their requests instead of rotating B again.
    first
        .refresh_after_unauthorized(rejected_access_token.clone())
        .await?;
    second
        .refresh_after_unauthorized(rejected_access_token)
        .await?;

    server.verify().await;
    let stored = load_oauth_tokens_from_file(&initial.server_name, &initial.url)?
        .expect("the winning refresh should be persisted");
    assert_eq!(
        stored.token_response.0.access_token().secret(),
        "refreshed-access-token"
    );
    let adopted = tokens_from_manager(&second_manager).await?;
    assert_eq!(adopted.0.access_token().secret(), "refreshed-access-token");
    assert!(adopted.0.refresh_token().is_none());
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn oauth_callback_waits_for_refresh_and_then_becomes_authoritative() -> Result<()> {
    let _env = TempCodexHome::new();
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "login-access-token",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "login-refresh-token",
            "scope": "scope-a scope-b",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let server_name = "callback-lock-test";
    let server_url = format!("{}/mcp", server.uri());
    let held_lock = RefreshCredentialLock::acquire_for_server(server_name, &server_url).await?;
    let (contended_tx, contended_rx) = mpsc::channel();
    let _subscriber_guard =
        tracing::subscriber::set_default(LockContentionSubscriber { contended_tx });
    let handle = crate::perform_oauth_login_return_url_with_http_client(
        server_name,
        &server_url,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        &["scope-a".to_string(), "scope-b".to_string()],
        Some("test-client-id"),
        /*oauth_resource*/ None,
        Some(/*timeout_secs*/ 5),
        /*callback_port*/ None,
        /*callback_url*/ None,
        Environment::default_for_tests().get_http_client(),
    )
    .await?;
    let authorization_url = reqwest::Url::parse(handle.authorization_url())?;
    let query = authorization_url
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    let redirect_uri = query
        .get("redirect_uri")
        .context("authorization URL omitted redirect_uri")?;
    let state = query
        .get("state")
        .context("authorization URL omitted state")?;
    let mut callback_url = reqwest::Url::parse(redirect_uri)?;
    callback_url
        .query_pairs_mut()
        .append_pair("code", "authorization-code")
        .append_pair("state", state);
    let (_authorization_url, completion) = handle.into_parts();
    reqwest::Client::new()
        .get(callback_url)
        .send()
        .await?
        .error_for_status()?;

    // This event is emitted only after the callback's real persistence path observes WouldBlock.
    // Writing while the lock is held models a refresh that started first; login must overwrite it
    // after the transaction finishes.
    wait_for_signal(contended_rx).await?;
    let mut refresh_winner = sample_tokens();
    refresh_winner.server_name = server_name.to_string();
    refresh_winner.url.clone_from(&server_url);
    refresh_winner
        .token_response
        .0
        .set_access_token(AccessToken::new("refresh-winner".to_string()));
    save_oauth_tokens_to_file(&refresh_winner)?;
    drop(held_lock);
    completion
        .await
        .context("OAuth login task was cancelled")??;

    let stored = load_oauth_tokens_from_file(server_name, &server_url)?
        .expect("callback credentials should be persisted");
    assert_eq!(
        stored.token_response.0.access_token().secret(),
        "login-access-token"
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn locked_logout_waits_for_refresh_and_removes_its_result() -> Result<()> {
    let _env = TempCodexHome::new();
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    let refresh_started = mount_delayed_refresh(&server, "refreshed-before-logout").await;
    let initial = expired_tokens(&format!("{}/mcp", server.uri()));
    save_oauth_tokens_to_file(&initial)?;
    let persistor = persistor_for(&initial).await?;
    let refresh_task = tokio::spawn(async move { persistor.refresh_if_needed().await });
    wait_for_signal(refresh_started).await?;

    let (contended_tx, contended_rx) = mpsc::channel();
    let _subscriber_guard =
        tracing::subscriber::set_default(LockContentionSubscriber { contended_tx });
    let server_name = initial.server_name.clone();
    let url = initial.url.clone();
    let logout_task = tokio::spawn(async move {
        delete_oauth_tokens_locked(
            &server_name,
            &url,
            OAuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default(),
        )
        .await
    });

    wait_for_signal(contended_rx).await?;
    refresh_task.await??;
    assert!(logout_task.await??);
    assert!(load_oauth_tokens_from_file(&initial.server_name, &initial.url)?.is_none());
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn resolved_keyring_read_error_preserves_in_memory_credentials() -> Result<()> {
    let _env = TempCodexHome::new();
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    let initial = expired_tokens(&format!("{}/mcp", server.uri()));
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
        .refresh_in(
            keyring_store,
            RefreshReason::Expiry,
            Duration::from_secs(/*secs*/ 45),
        )
        .await
        .expect_err("the resolved keyring read error should abort refresh");
    assert!(
        error
            .to_string()
            .contains("failed to reread OAuth tokens from resolved keyring storage"),
        "unexpected error: {error:#}"
    );
    assert_eq!(tokens_from_manager(&manager).await?, initial.token_response);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn caller_cancellation_does_not_cancel_refresh_persistence() -> Result<()> {
    let _env = TempCodexHome::new();
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    let refresh_started = mount_delayed_refresh(&server, "cancel-safe-access-token").await;
    let initial = expired_tokens(&format!("{}/mcp", server.uri()));
    save_oauth_tokens_to_file(&initial)?;
    let persistor = persistor_for(&initial).await?;
    let caller = tokio::spawn({
        let persistor = persistor.clone();
        async move { persistor.refresh_if_needed().await }
    });

    wait_for_signal(refresh_started).await?;
    caller.abort();
    assert!(
        caller
            .await
            .expect_err("caller should be cancelled")
            .is_cancelled()
    );

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
    let _env = TempCodexHome::new();
    let server = MockServer::start().await;
    mount_oauth_metadata(&server).await;
    let _refresh_started = mount_delayed_refresh(&server, "late-access-token").await;
    let initial = expired_tokens(&format!("{}/mcp", server.uri()));
    save_oauth_tokens_to_file(&initial)?;
    let persistor = persistor_for(&initial).await?;

    let error = persistor
        .refresh_in(
            MockKeyringStore::default(),
            RefreshReason::Expiry,
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
    assert_eq!(
        stored.token_response.0.access_token().secret(),
        initial.token_response.0.access_token().secret()
    );
    assert_eq!(
        stored
            .token_response
            .0
            .refresh_token()
            .map(oauth2::RefreshToken::secret),
        initial
            .token_response
            .0
            .refresh_token()
            .map(oauth2::RefreshToken::secret)
    );
    server.verify().await;
    Ok(())
}

#[test]
fn secrets_exact_store_save_does_not_mutate_stale_fallback_file() -> Result<()> {
    let _env = TempCodexHome::new();
    let initial = sample_tokens();
    let keyring_store = MockKeyringStore::default();
    save_oauth_tokens_with_keyring(
        &keyring_store,
        AuthKeyringBackendKind::Secrets,
        &initial.server_name,
        &initial,
    )?;

    let mut stale_file = initial.clone();
    stale_file
        .token_response
        .0
        .set_access_token(AccessToken::new("stale-file-access".to_string()));
    save_oauth_tokens_to_file(&stale_file)?;
    let fallback_path = fallback_file_path()?;
    let fallback_before = std::fs::read(&fallback_path)?;

    let mut refreshed = initial.clone();
    refreshed
        .token_response
        .0
        .set_access_token(AccessToken::new("secrets-refreshed-access".to_string()));
    save_oauth_tokens_with_keyring(
        &keyring_store,
        AuthKeyringBackendKind::Secrets,
        &refreshed.server_name,
        &refreshed,
    )?;

    assert_eq!(std::fs::read(fallback_path)?, fallback_before);
    let stored = load_oauth_tokens_from_keyring(
        &keyring_store,
        AuthKeyringBackendKind::Secrets,
        &initial.server_name,
        &initial.url,
    )?
    .expect("refreshed credentials should remain in Secrets");
    assert_eq!(
        stored.token_response.0.access_token().secret(),
        "secrets-refreshed-access"
    );
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

#[expect(
    clippy::await_holding_invalid_type,
    reason = "AuthorizationManager async access must be serialized through its Tokio mutex"
)]
async fn tokens_from_manager(
    manager: &Arc<TokioMutex<AuthorizationManager>>,
) -> Result<WrappedOAuthTokenResponse> {
    let guard = manager.lock().await;
    let (_client_id, token_response) = guard.get_credentials().await?;
    Ok(WrappedOAuthTokenResponse(
        token_response.expect("manager should retain credentials"),
    ))
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

async fn mount_delayed_refresh(
    server: &MockServer,
    response_access_token: &str,
) -> mpsc::Receiver<()> {
    let (started_tx, started_rx) = mpsc::channel();
    let response_access_token = response_access_token.to_string();
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=refresh-token"))
        .respond_with(move |_request: &wiremock::Request| {
            let _ = started_tx.send(());
            let access_token = response_access_token.clone();
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(/*millis*/ 200))
                .set_body_json(serde_json::json!({
                    "access_token": access_token,
                    "token_type": "Bearer",
                    "expires_in": 3600,
                }))
        })
        .expect(1)
        .mount(server)
        .await;
    started_rx
}

async fn wait_for_signal(rx: mpsc::Receiver<()>) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        rx.recv_timeout(Duration::from_secs(/*secs*/ 5))
            .context("timed out waiting for refresh request")
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
