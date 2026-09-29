use std::io;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_config::types::AuthKeyringBackendKind;
use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_keyring_store::CredentialStoreError;
use codex_keyring_store::tests::MockKeyringStore;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use pretty_assertions::assert_ne;
use serde_json::json;
use tokio::sync::oneshot;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;
use crate::EmaAuthFailure;
use crate::WrappedOAuthTokenResponse;
use crate::oauth::ResolvedOAuthCredentialStore;
use crate::oauth::test_support::TempCodexHome;

fn credentials(issuer: &str, subject: &str, expires_at: u64) -> StoredOAuthTokens {
    let assertion = format!(
        "{}.{}.signature",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "iss":issuer,"aud":"idp-client","sub":subject,"exp":expires_at,
            }))
            .expect("claims")
        )
    );
    StoredOAuthTokens {
        server_name: "ema-idp:enterprise-test".to_string(),
        url: issuer.to_string(),
        issuer: Some(issuer.to_string()),
        client_id: "idp-client".to_string(),
        token_response: WrappedOAuthTokenResponse(
            serde_json::from_value(json!({
                "access_token":"unused","token_type":"Bearer","id_token":assertion,
                "refresh_token":"stored-refresh","scope":"openid offline_access",
            }))
            .expect("credentials"),
        ),
        expires_at: None,
    }
}

fn request<'a>(
    issuer: &'a str,
    credentials: &'a StoredOAuthCredentialSnapshot,
) -> EmaIdpIdentityRequest<'a> {
    EmaIdpIdentityRequest {
        issuer,
        client_id: "idp-client",
        credentials,
        http_client: Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        redirect_mode: StreamableHttpRedirectMode::Legacy,
    }
}

async fn discovery() -> (MockServer, String) {
    let server = MockServer::start().await;
    let issuer = format!("{}/idp", server.uri());
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/idp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer":issuer,"authorization_endpoint":format!("{issuer}/authorize"),
            "token_endpoint":format!("{issuer}/token"),
            "identity_chaining_requested_token_types_supported":[ID_JAG_TOKEN_TYPE],
            "grant_types_supported":[TOKEN_EXCHANGE_GRANT_TYPE],
            "token_endpoint_auth_methods_supported":["none"],
        })))
        .mount(&server)
        .await;
    (server, issuer)
}

#[tokio::test]
async fn enterprise_discovery_shares_idp_checks_but_keeps_login_endpoint_policy() -> Result<()> {
    for case in [
        "valid",
        "missing-discovery",
        "issuer",
        "missing-issuer",
        "token-endpoint",
        "public-client",
        "authorization-endpoint",
    ] {
        let server = MockServer::start().await;
        let issuer = format!("{}/idp", server.uri());
        // Login need not advertise ID-JAG exchange capabilities.
        let mut metadata = json!({
            "issuer": issuer, "authorization_endpoint": format!("{issuer}/authorize"),
            "token_endpoint": format!("{issuer}/token"),
            "token_endpoint_auth_methods_supported": ["none"],
            "grant_types_supported": ["authorization_code"],
        });
        match case {
            "issuer" => metadata["issuer"] = json!("https://other.example"),
            "missing-issuer" => {
                metadata.as_object_mut().expect("metadata").remove("issuer");
            }
            "token-endpoint" => metadata["token_endpoint"] = json!("http://unsafe.example/token"),
            "public-client" => {
                metadata["token_endpoint_auth_methods_supported"] = json!(["client_secret_basic"])
            }
            "authorization-endpoint" => {
                metadata["authorization_endpoint"] = json!("http://unsafe.example/authorize")
            }
            "valid" | "missing-discovery" => {}
            _ => unreachable!(),
        }
        if case != "missing-discovery" {
            Mock::given(method("GET"))
                .and(path("/.well-known/oauth-authorization-server/idp"))
                .respond_with(ResponseTemplate::new(200).set_body_json(metadata))
                .mount(&server)
                .await;
        }
        let client = Arc::new(OAuthHttpClientAdapter::new_with_redirect_mode(
            Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
                OutboundProxyPolicy::ReqwestDefault,
            ))),
            build_default_headers(/*http_headers*/ None, /*env_http_headers*/ None)?,
            &issuer,
            /*has_configured_headers*/ false,
            StreamableHttpRedirectMode::Legacy,
        )?);
        let shared = resolve_ema_idp_authorization_manager(&issuer, client.clone()).await;
        let login = crate::enterprise_oauth_login::resolve_enterprise_authorization_manager(
            &issuer, client,
        )
        .await;
        assert_eq!(
            (shared.is_ok(), login.is_ok()),
            (
                matches!(case, "valid" | "authorization-endpoint"),
                case == "valid"
            ),
            "{case}",
        );
        if case == "missing-discovery" {
            assert_eq!(
                shared
                    .err()
                    .expect("reject synthesized metadata")
                    .to_string(),
                "enterprise IdP must publish authorization metadata",
            );
        }
    }
    Ok(())
}

const REREAD_TEST_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 5);
const REREAD_PANIC_SENTINEL: &str = "credential-panic-payload-sentinel";

#[derive(Clone, Copy, Debug)]
enum ReadOutcome {
    Stored,
    BackendError,
    Panic,
}

#[derive(Clone, Debug)]
struct GatedKeyringStore {
    inner: MockKeyringStore,
    executor_thread: thread::ThreadId,
    entered: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    release: Arc<Mutex<mpsc::Receiver<ReadOutcome>>>,
}

impl KeyringStore for GatedKeyringStore {
    fn load(&self, service: &str, account: &str) -> Result<Option<String>, CredentialStoreError> {
        // Fail before blocking if a regression puts this read on the current-thread executor.
        assert_ne!(thread::current().id(), self.executor_thread);
        if let Some(entered) = self
            .entered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = entered.send(());
        }
        let outcome = match self
            .release
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv_timeout(REREAD_TEST_TIMEOUT)
        {
            Ok(outcome) => outcome,
            Err(mpsc::RecvTimeoutError::Disconnected) => ReadOutcome::Stored,
            Err(mpsc::RecvTimeoutError::Timeout) => panic!("credential reread gate timed out"),
        };
        match outcome {
            ReadOutcome::Stored => self.inner.load(service, account),
            ReadOutcome::BackendError => Err(CredentialStoreError::new(
                keyring::Error::PlatformFailure(Box::new(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "credential backend unavailable",
                ))),
            )),
            ReadOutcome::Panic => panic!("{REREAD_PANIC_SENTINEL}"),
        }
    }

    fn save(&self, service: &str, account: &str, value: &str) -> Result<(), CredentialStoreError> {
        self.inner.save(service, account, value)
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, CredentialStoreError> {
        self.inner.delete(service, account)
    }
}

#[tokio::test(flavor = "current_thread")]
async fn refresh_subject_reread_is_cancellable_and_releases_guard_on_failure() -> Result<()> {
    let _home = TempCodexHome::new();
    let (_server, issuer) = discovery().await;
    let store = ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct);
    let stored = credentials(&issuer, "user", /*expires_at*/ 0);
    let snapshot = StoredOAuthCredentialSnapshot::new(stored.clone(), store);
    for outcome in [
        ReadOutcome::Stored,
        ReadOutcome::BackendError,
        ReadOutcome::Panic,
    ] {
        let inner = MockKeyringStore::default();
        store.save(&inner, &stored.server_name, &stored)?;
        let (entered_tx, entered_rx) = oneshot::channel();
        // Dropping the sole sender releases the worker on any early return or panic.
        let (release_tx, release_rx) = mpsc::channel();
        let keyring = GatedKeyringStore {
            inner,
            executor_thread: thread::current().id(),
            entered: Arc::new(Mutex::new(Some(entered_tx))),
            release: Arc::new(Mutex::new(release_rx)),
        };
        let request = request(&issuer, &snapshot);
        crate::oauth::test_support::warm_http_client(request.http_client.as_ref()).await?;
        let mut identity = Box::pin(resolve_ema_idp_identity_in(request, &keyring));
        tokio::select! {
            result = &mut identity => {
                result?;
                bail!("credential reread completed before its gate was released");
            }
            entered = tokio::time::timeout(REREAD_TEST_TIMEOUT, entered_rx) => { entered??; }
        }

        if matches!(outcome, ReadOutcome::Stored) {
            // The caller times out while the already-started blocking read remains gated.
            assert!(
                tokio::time::timeout(Duration::ZERO, identity)
                    .await
                    .is_err()
            );
            assert!(
                RefreshCredentialLock::acquire_for_server(&stored.server_name, &issuer)
                    .now_or_never()
                    .is_none()
            );
            release_tx.send(outcome)?;
        } else {
            release_tx.send(outcome)?;
            let error = tokio::time::timeout(REREAD_TEST_TIMEOUT, identity)
                .await?
                .err()
                .context("credential reread should fail")?;
            if matches!(outcome, ReadOutcome::Panic) {
                assert_eq!(
                    format!("{error:#}"),
                    "enterprise IdP credential reread task failed"
                );
            } else {
                assert!(error.to_string().contains("refusing file fallback"));
                // The store's transparent wrapper exposes a platform error's source.
                assert!(error.chain().any(|cause| {
                    matches!(
                        cause.downcast_ref::<io::Error>(),
                        Some(error) if error.kind() == io::ErrorKind::PermissionDenied
                    )
                }));
            }
        }
        // Drain the detached read before TempCodexHome changes the process environment.
        let _released = tokio::time::timeout(
            REREAD_TEST_TIMEOUT,
            RefreshCredentialLock::acquire_for_server(&stored.server_name, &issuer),
        )
        .await??;
    }
    Ok(())
}

#[tokio::test]
async fn refresh_subject_rereads_pinned_credentials_after_id_token_expiry() -> Result<()> {
    let _home = TempCodexHome::new();
    let (_server, issuer) = discovery().await;
    let store = ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct);
    let stored = credentials(&issuer, "user", /*expires_at*/ 0);
    let snapshot = StoredOAuthCredentialSnapshot::new(stored.clone(), store);
    let keyring = MockKeyringStore::default();
    store.save(&keyring, &stored.server_name, &stored)?;

    let identity = resolve_ema_idp_identity_in(request(&issuer, &snapshot), &keyring).await?;
    assert_eq!(
        (&identity.token_endpoint, identity.refresh_token.as_str()),
        (&format!("{issuer}/token"), "stored-refresh")
    );
    assert!(
        tokio::time::timeout(
            Duration::from_millis(/*millis*/ 50),
            RefreshCredentialLock::acquire_for_server(&stored.server_name, &issuer),
        )
        .await
        .is_err()
    );
    drop(identity);
    let _released = RefreshCredentialLock::acquire_for_server(&stored.server_name, &issuer).await?;
    Ok(())
}

#[tokio::test]
async fn refresh_subject_rejects_removed_replaced_or_missing_credentials() -> Result<()> {
    let _home = TempCodexHome::new();
    let (server, issuer) = discovery().await;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let store = ResolvedOAuthCredentialStore::keyring(AuthKeyringBackendKind::Direct);
    let original = credentials(&issuer, "user", /*expires_at*/ 0);
    for change in [
        "deleted",
        "subject",
        "issuer",
        "client",
        "refresh",
        "login",
        "missing-refresh",
        "file",
    ] {
        let keyring = MockKeyringStore::default();
        let snapshot = StoredOAuthCredentialSnapshot::new(
            original.clone(),
            if change == "file" {
                ResolvedOAuthCredentialStore::file()
            } else {
                store
            },
        );
        let mut latest = original.clone();
        match change {
            "subject" => latest = credentials(&issuer, "other-user", /*expires_at*/ 0),
            "issuer" => latest.issuer = Some("https://other.example".to_string()),
            "client" => latest.client_id = "other-client".to_string(),
            "refresh" => {
                latest
                    .token_response
                    .0
                    .set_refresh_token(Some(oauth2::RefreshToken::new(
                        "other-users-refresh".to_string(),
                    )))
            }
            "login" => latest = credentials(&issuer, "user", now + 3600),
            "missing-refresh" => latest.token_response.0.set_refresh_token(None),
            "deleted" | "file" => {}
            _ => panic!("unexpected change"),
        }
        if change != "deleted" {
            store.save(&keyring, &original.server_name, &latest)?;
        }
        let error = resolve_ema_idp_identity_in(request(&issuer, &snapshot), &keyring)
            .await
            .err()
            .expect("must not reuse the stale snapshot or fall back to its ID token");
        if change == "file" {
            assert!(error.to_string().contains("require keyring storage"));
        } else {
            assert_eq!(
                error.downcast_ref::<EmaAuthFailure>(),
                Some(&EmaAuthFailure::ReauthenticationRequired),
                "{change}"
            );
        }
    }
    assert!(
        server
            .received_requests()
            .await
            .expect("requests")
            .iter()
            .all(|request| request.method.as_str() == "GET")
    );
    Ok(())
}

#[test]
fn stored_identity_usability_requires_a_bound_refresh_token_not_a_current_id_token() {
    let issuer = "https://idp.example";
    let expired = credentials(issuer, "user", /*expires_at*/ 0);
    assert!(stored_ema_identity_is_usable(
        &expired,
        issuer,
        "idp-client"
    ));
    for change in ["url", "issuer", "client", "refresh"] {
        let mut changed = expired.clone();
        match change {
            "url" => changed.url = "https://other.example".to_string(),
            "issuer" => changed.issuer = Some("https://other.example".to_string()),
            "client" => changed.client_id = "other-client".to_string(),
            "refresh" => changed.token_response.0.set_refresh_token(None),
            _ => panic!("unexpected change"),
        }
        assert!(!stored_ema_identity_is_usable(
            &changed,
            issuer,
            "idp-client"
        ));
    }
}
