//! Cross-process credential changes invalidate staged logins and cached EMA leases.

use std::fs;
use std::path::PathBuf;

use codex_exec_server::ExecServerError;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use futures::FutureExt;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;
use tokio::sync::Notify;

use super::*;

#[tokio::test]
async fn failed_persistence_fences_old_snapshots_but_allows_fresh_pins() -> Result<()> {
    if isolated_process("enterprise_oauth_login::tests::logout::failed_persistence_fences_old_snapshots_but_allows_fresh_pins").await? {
        return Ok(());
    }
    let keyring = TestKeyring::default();
    keyring::set_default_credential_builder(Box::new(keyring.clone()));
    let issuer = "https://idp.example";
    let assertion = format!(
        "{}.{}.signature",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(
            json!({"iss":issuer,"sub":"user","aud":"enterprise-client","exp":4102444800_u64})
                .to_string()
        )
    );
    let tokens: StoredOAuthTokens = serde_json::from_value(json!({
        "server_name": CREDENTIAL_NAME, "url": issuer, "issuer": issuer,
        "client_id": "enterprise-client", "token_response": {
            "access_token": SECRET, "refresh_token": SECRET, "id_token": assertion, "token_type": "Bearer"
        }
    }))?;
    let mode = OAuthCredentialsStoreMode::Keyring;
    let backend = AuthKeyringBackendKind::Direct;
    crate::save_oauth_tokens(CREDENTIAL_NAME, &tokens, mode, backend).await?;
    for operation in ["save", "delete"] {
        let snapshot =
            crate::stored_oauth_credential_snapshot(CREDENTIAL_NAME, issuer, mode, backend)?
                .expect("stored grant")
                .pin_ema_credentials()
                .await?;
        drop(snapshot.acquire_current_ema_credentials().await?);
        // Fail the actual keyring mutation, after the public API advances the generation.
        keyring
            .values
            .lock()
            .unwrap()
            .values()
            .next()
            .expect("credential")
            .set_error(keyring::Error::PlatformFailure(Box::new(
                std::io::Error::other("injected persistence failure"),
            )));
        let result = match operation {
            "save" => crate::save_oauth_tokens(CREDENTIAL_NAME, &tokens, mode, backend).await,
            "delete" => crate::delete_oauth_tokens(CREDENTIAL_NAME, issuer, mode, backend)
                .await
                .map(|_| ()),
            _ => unreachable!(),
        };
        assert!(
            result.is_err(),
            "{operation} must report the storage failure"
        );
        assert!(
            snapshot.acquire_current_ema_credentials().await.is_err(),
            "{operation} must fence the old snapshot"
        );
        assert_eq!(stored(issuer)?, Some(tokens.clone()));
        // A generation is not a deletion tombstone: a fresh connection may use the surviving grant.
        let fresh =
            crate::stored_oauth_credential_snapshot(CREDENTIAL_NAME, issuer, mode, backend)?
                .expect("surviving grant")
                .pin_ema_credentials()
                .await?;
        drop(fresh.acquire_current_ema_credentials().await?);
    }
    Ok(())
}

const TEST: &str =
    "enterprise_oauth_login::tests::logout::logout_invalidates_pending_login_across_processes";
const LOGOUT_ISSUER: &str = "CODEX_ENTERPRISE_LOGOUT_TEST_ISSUER";
const REPLACE_CREDENTIAL: &str = "CODEX_ENTERPRISE_LOGOUT_TEST_REPLACE";

#[tokio::test]
async fn logout_invalidates_pending_login_across_processes() -> Result<()> {
    if isolated_process(TEST).await? {
        return Ok(());
    }
    let home = PathBuf::from(std::env::var("CODEX_HOME")?);
    let keyring = FileKeyring(home.join("test-keyring"));
    fs::create_dir_all(&keyring.0)?;
    keyring::set_default_credential_builder(Box::new(keyring));
    if let Ok(issuer) = std::env::var(LOGOUT_ISSUER) {
        if std::env::var_os(REPLACE_CREDENTIAL).is_some() {
            crate::save_oauth_tokens(
                CREDENTIAL_NAME,
                &stored(&issuer)?.expect("stored grant"),
                OAuthCredentialsStoreMode::Keyring,
                AuthKeyringBackendKind::Direct,
            )
            .await?;
        } else {
            delete_enterprise_oauth_tokens(
                CREDENTIAL_NAME,
                &issuer,
                AuthKeyringBackendKind::Direct,
            )
            .await?;
        }
        return Ok(());
    }
    let server = MockServer::start().await;
    let issuer = format!("{}/idp", server.uri());
    metadata(&server, &issuer).await;
    let assertion = format!(
        "{}.{}.signature",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(
            json!({"iss":issuer,"sub":"user","aud":"enterprise-client","exp":4102444800_u64})
                .to_string()
        )
    );
    Mock::given(method("POST"))
        .and(path("/idp/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token":SECRET,"refresh_token":SECRET,"id_token":assertion,"token_type":"Bearer",
        })))
        .mount(&server)
        .await;

    // A logout during provider setup must invalidate the in-flight start.
    let paused_client = Arc::new(PausedSetupHttpClient {
        inner: Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        should_pause: AtomicBool::new(true),
        entered: Notify::new(),
        release: Notify::new(),
    });
    let starting_client = Arc::clone(&paused_client);
    let starting_issuer = issuer.clone();
    let starting = tokio::spawn(async move {
        perform_enterprise_oauth_login_return_url(EnterpriseOAuthLoginRequest {
            credential_name: CREDENTIAL_NAME,
            issuer: &starting_issuer,
            client_id: "enterprise-client",
            keyring_backend_kind: AuthKeyringBackendKind::Direct,
            callback_port: None,
            callback_url: None,
            timeout_secs: Some(5),
            http_client: starting_client,
            redirect_mode: StreamableHttpRedirectMode::Legacy,
        })
        .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        paused_client.entered.notified(),
    )
    .await?;
    delete_enterprise_oauth_tokens(CREDENTIAL_NAME, &issuer, AuthKeyringBackendKind::Direct)
        .await?;
    paused_client.release.notify_one();
    let error = tokio::time::timeout(std::time::Duration::from_secs(5), starting)
        .await??
        .err()
        .expect("logout must invalidate a start during provider setup");
    assert_eq!(
        error.to_string(),
        "enterprise login changed during replacement setup"
    );

    // The first logout has no grant to delete. The second deletes the fresh grant
    // from the previous iteration. Both must invalidate pending callbacks and staged grants.
    for _ in 0..2 {
        let pending = login(&issuer, /*callback_url*/ None).await?;
        let staged = complete_login(&issuer).await?;
        let output = tokio::process::Command::new(std::env::current_exe()?)
            .args(["--exact", TEST, "--nocapture"])
            .env(LOGOUT_ISSUER, &issuer)
            .output()
            .await?;
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(stored(&issuer)?.is_none());
        // The logout process has exited; its invalidation must survive that exit.
        assert!(staged.commit_if(|| async { Some(()) }).await.is_err());
        assert!(stored(&issuer)?.is_none());

        // A fresh login after logout remains usable, including for the same account.
        complete_login(&issuer)
            .await?
            .commit_if(|| async { Some(()) })
            .await?;
        let fresh = stored(&issuer)?.expect("fresh grant");
        callback(
            &pending.authorization_url(),
            &issuer,
            /*provider_error*/ false,
        )
        .await?;
        assert!(
            pending
                .wait()
                .await?
                .commit_if(|| async { Some(()) })
                .await
                .is_err()
        );
        assert_eq!(stored(&issuer)?, Some(fresh));
    }

    // Even a byte-identical replacement in another process retires the cached
    // version. Its next lease must fail after that process exits, as must logout.
    for operation in ["replace", "delete"] {
        let before = stored(&issuer)?;
        let cached = crate::stored_oauth_credential_snapshot(
            CREDENTIAL_NAME,
            &issuer,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?
        .expect("keyring-backed snapshot")
        .pin_ema_credentials()
        .await?;
        drop(cached.acquire_current_ema_credentials().await?);

        let mut child = tokio::process::Command::new(std::env::current_exe()?);
        child
            .args(["--exact", TEST, "--nocapture"])
            .env(LOGOUT_ISSUER, &issuer);
        if operation == "replace" {
            child.env(REPLACE_CREDENTIAL, "1");
        }
        let output = child.output().await?;
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(cached.acquire_current_ema_credentials().await.is_err());
        assert_eq!(
            stored(&issuer)?,
            if operation == "replace" { before } else { None }
        );
    }

    let stale = complete_login(&issuer).await?;
    let generation_path = fs::read_dir(home.join("mcp-oauth-locks"))?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|ext| ext == "enterprise-generation")
        })
        .expect("persistent generation");
    fs::write(&generation_path, b"incomplete write")?;
    assert!(stale.commit_if(|| async { Some(()) }).await.is_err());
    assert!(login(&issuer, /*callback_url*/ None).await.is_err());

    // A lost marker never revives a pre-logout attempt or a pre-reset generation.
    fs::remove_file(&generation_path)?;
    let stale = complete_login(&issuer).await?;
    fs::remove_file(&generation_path)?;
    complete_login(&issuer)
        .await?
        .commit_if(|| async { Some(()) })
        .await?;
    assert!(stale.commit_if(|| async { Some(()) }).await.is_err());

    // Metadata failures are logout errors and must leave the stored grant intact.
    let before = stored(&issuer)?;
    let version_path = generation_path.with_extension("enterprise-credential-version");
    let version = fs::read(&version_path)?;
    fs::remove_file(&version_path)?;
    fs::create_dir(&version_path)?;
    let mode = OAuthCredentialsStoreMode::Keyring;
    let backend = AuthKeyringBackendKind::Direct;
    let mut replacement = before.clone().expect("stored grant");
    replacement.client_id.push_str("-replacement");
    let saved = crate::save_oauth_tokens(CREDENTIAL_NAME, &replacement, mode, backend).await;
    assert!(saved.is_err());
    assert_eq!(stored(&issuer)?, before);
    let deleted = crate::delete_oauth_tokens(CREDENTIAL_NAME, &issuer, mode, backend).await;
    assert!(deleted.is_err());
    assert_eq!(stored(&issuer)?, before);
    fs::remove_dir(&version_path)?;
    fs::write(&version_path, version)?;

    fs::remove_file(&generation_path)?;
    fs::create_dir(&generation_path)?;
    assert!(
        delete_enterprise_oauth_tokens(CREDENTIAL_NAME, &issuer, AuthKeyringBackendKind::Direct)
            .await
            .is_err()
    );
    assert_eq!(stored(&issuer)?, before);
    assert!(!home.join(".credentials.json").exists());
    Ok(())
}

struct PausedSetupHttpClient {
    inner: Arc<dyn HttpClient>,
    should_pause: AtomicBool,
    entered: Notify,
    release: Notify,
}

impl PausedSetupHttpClient {
    async fn pause_once(&self) {
        if self.should_pause.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
    }
}

impl HttpClient for PausedSetupHttpClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        async move {
            self.pause_once().await;
            self.inner.http_request(params).await
        }
        .boxed()
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        async move {
            self.pause_once().await;
            self.inner.http_request_stream(params).await
        }
        .boxed()
    }
}

fn stored(issuer: &str) -> Result<Option<StoredOAuthTokens>> {
    crate::stored_oauth_credentials(
        CREDENTIAL_NAME,
        issuer,
        OAuthCredentialsStoreMode::Keyring,
        AuthKeyringBackendKind::Direct,
    )
}

// Only synthetic fixture credentials are persisted here. Every process still uses
// DefaultKeyringStore and the production credential lock, serializer and logout API.
struct FileKeyring(PathBuf);
struct FileCredential(PathBuf);

impl CredentialBuilderApi for FileKeyring {
    fn build(
        &self,
        _target: Option<&str>,
        _service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        let name = format!("{:x}", Sha256::digest(user.as_bytes()));
        Ok(Box::new(FileCredential(self.0.join(name))))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CredentialApi for FileCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
        fs::write(&self.0, secret).map_err(keyring_error)
    }

    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        fs::read(&self.0).map_err(keyring_error)
    }

    fn delete_credential(&self) -> keyring::Result<()> {
        fs::remove_file(&self.0).map_err(keyring_error)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn keyring_error(error: std::io::Error) -> keyring::Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        keyring::Error::NoEntry
    } else {
        keyring::Error::PlatformFailure(Box::new(error))
    }
}
