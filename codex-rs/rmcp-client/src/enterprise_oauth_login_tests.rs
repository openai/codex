//! Public enterprise login exercises the real callback and keyring adapter in an isolated process.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use keyring::credential::Credential;
use keyring::credential::CredentialApi;
use keyring::credential::CredentialBuilderApi;
use keyring::credential::CredentialPersistence;
use keyring::mock::MockCredential;
use oauth2::TokenResponse;
use pretty_assertions::assert_eq;
use serde_json::json;
use sha2::Digest;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tracing_test::traced_test;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;

const SECRET: &str = "enterprise-secret-sentinel";
const CREDENTIAL_NAME: &str = "ema-idp:enterprise-secret-sentinel";

#[path = "enterprise_oauth_logout_tests.rs"]
mod logout;

async fn isolated_process(test_name: &str) -> Result<bool> {
    const CHILD: &str = "CODEX_ENTERPRISE_LOGIN_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        return Ok(false);
    }
    let home = tempfile::tempdir()?;
    let output = tokio::process::Command::new(std::env::current_exe()?)
        .args(["--exact", test_name, "--nocapture"])
        .env(CHILD, "1")
        .env("CODEX_HOME", home.path())
        .current_dir(home.path())
        .output()
        .await?;
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(true)
}

async fn login(issuer: &str, callback_url: Option<&str>) -> Result<EnterpriseOAuthLoginHandle> {
    perform_enterprise_oauth_login_return_url(EnterpriseOAuthLoginRequest {
        credential_name: CREDENTIAL_NAME,
        issuer,
        client_id: "enterprise-client",
        keyring_backend_kind: AuthKeyringBackendKind::Direct,
        callback_port: None,
        callback_url,
        timeout_secs: Some(5),
        http_client: Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        redirect_mode: StreamableHttpRedirectMode::Legacy,
    })
    .await
}

async fn complete_login(issuer: &str) -> Result<EnterpriseOAuthCredentials> {
    let handle = login(issuer, /*callback_url*/ None).await?;
    callback(
        &handle.authorization_url(),
        issuer,
        /*provider_error*/ false,
    )
    .await?;
    handle.wait().await
}

async fn metadata(server: &MockServer, issuer: &str) {
    Mock::given(method("GET")).and(path("/.well-known/oauth-authorization-server/idp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": issuer, "authorization_endpoint": format!("{issuer}/authorize?prompt=none"),
            "token_endpoint": format!("{issuer}/token"), "token_endpoint_auth_methods_supported": ["none"],
        }))).mount(server).await;
}

async fn callback(authorization_url: &str, issuer: &str, provider_error: bool) -> Result<()> {
    let query = Url::parse(authorization_url)?
        .query_pairs()
        .into_owned()
        .collect::<HashMap<_, _>>();
    assert_eq!(query.get("prompt").map(String::as_str), Some("consent"));
    assert!(!query.contains_key("resource"));
    let mut callback = Url::parse(&query["redirect_uri"])?;
    let mut pairs = callback.query_pairs_mut();
    if provider_error {
        pairs
            .append_pair("error", SECRET)
            .append_pair("error_description", SECRET);
    } else {
        pairs
            .append_pair("code", SECRET)
            .append_pair("state", &query["state"])
            .append_pair("iss", issuer);
    }
    drop(pairs);
    let port = callback.port().expect("actual listener port");
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
    stream
        .write_all(
            format!(
                "GET {}?{} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n",
                callback.path(),
                callback.query().expect("query")
            )
            .as_bytes(),
        )
        .await?;
    stream.read_to_end(&mut Vec::new()).await?;
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn enterprise_callback_errors_and_sdk_logs_exclude_credentials() -> Result<()> {
    if isolated_process(
        "enterprise_oauth_login::tests::enterprise_callback_errors_and_sdk_logs_exclude_credentials",
    )
    .await?
    {
        return Ok(());
    }
    for (callback_error, token_success) in [(true, false), (false, false), (false, true)] {
        let server = MockServer::start().await;
        let issuer = format!("{}/idp", server.uri());
        metadata(&server, &issuer).await;
        let response = if token_success {
            ResponseTemplate::new(200).set_body_json(json!({
                "access_token": SECRET, "refresh_token": SECRET, "id_token": SECRET, "token_type": "Bearer",
            }))
        } else {
            ResponseTemplate::new(400)
                .set_body_json(json!({"error": "invalid_grant", "error_description": SECRET}))
        };
        Mock::given(method("POST"))
            .and(path("/idp/token"))
            .respond_with(response)
            .expect(u64::from(!callback_error))
            .mount(&server)
            .await;
        let login = login(&issuer, /*callback_url*/ None).await?;
        callback(&login.authorization_url(), &issuer, callback_error).await?;
        tracing::trace!(target: "rmcp::transport::auth", "SDK trace capture enabled");
        let error = login.wait().await.err().expect("reject provider response");
        assert!(!format!("{error} {error:?} {error:#}").contains(SECRET));
        assert!(logs_contain("SDK trace capture enabled"));
        assert!(!logs_contain(SECRET));
    }
    Ok(())
}

#[test]
fn enterprise_callback_requires_loopback() -> Result<()> {
    let issuer = "https://idp.example";
    let client_id = Some("enterprise-client");
    for (callback, port, expected_ip, expected_port) in [
        (None, None, "127.0.0.1", None),
        (None, Some(8080), "127.0.0.1", Some(8080)),
        (Some("http://localhost/callback"), None, "127.0.0.1", None),
        (Some("http://127.0.0.2/callback"), None, "127.0.0.2", None),
        (Some("http://[::1]/callback"), None, "::1", None),
        (
            Some("http://localhost:8080/callback"),
            None,
            "127.0.0.1",
            Some(8080),
        ),
        (
            Some("http://localhost:8080/callback"),
            Some(8080),
            "127.0.0.1",
            Some(8080),
        ),
    ] {
        assert_eq!(
            enterprise_callback_settings(issuer, client_id, callback, port)?,
            (expected_ip.parse::<IpAddr>()?, expected_port),
        );
    }
    for callback in [
        "http://0.0.0.0/callback",
        "http://[::]/callback",
        "https://127.0.0.1/callback",
        "http://remote.example/callback",
    ] {
        assert!(
            enterprise_callback_settings(
                issuer,
                client_id,
                Some(callback),
                /*callback_port*/ None
            )
            .is_err()
        );
    }
    assert!(
        enterprise_callback_settings(
            issuer,
            client_id,
            Some("http://localhost:8080/callback"),
            Some(9090),
        )
        .is_err()
    );
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn enterprise_public_api_storage_and_privacy() -> Result<()> {
    if isolated_process("enterprise_oauth_login::tests::enterprise_public_api_storage_and_privacy")
        .await?
    {
        return Ok(());
    }
    let keyring = TestKeyring::default();
    keyring::set_default_credential_builder(Box::new(keyring.clone()));
    let server = MockServer::start().await;
    let issuer = format!("{}/idp", server.uri());
    metadata(&server, &issuer).await;
    assert!(
        login(&issuer, Some(&format!("{}/callback", server.uri())))
            .await
            .is_err(),
        "an occupied registered callback port must not silently move"
    );
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

    // Callback, exchange and DefaultKeyringStore all run through public entrypoints.
    for (login_index, callback_url) in ["http://localhost/callback", "http://127.0.0.1/callback"]
        .into_iter()
        .enumerate()
    {
        let login = login(&issuer, Some(callback_url)).await?;
        let authorization_url = login.authorization_url();
        let authorization_query = Url::parse(&authorization_url)?
            .query_pairs()
            .into_owned()
            .collect::<HashMap<_, _>>();
        callback(&authorization_url, &issuer, /*provider_error*/ false).await?;
        let credentials = login.wait().await?;
        let token_requests = server
            .received_requests()
            .await
            .expect("recorded OAuth requests")
            .into_iter()
            .filter(|request| {
                request.method == http::Method::POST && request.url.path() == "/idp/token"
            })
            .collect::<Vec<_>>();
        assert_eq!(token_requests.len(), login_index + 1);
        let token_form = url::form_urlencoded::parse(&token_requests[login_index].body)
            .into_owned()
            .collect::<HashMap<_, _>>();
        assert_eq!(
            token_form.get("grant_type").map(String::as_str),
            Some("authorization_code")
        );
        assert!(!token_form.contains_key("resource"));
        assert_eq!(
            token_form.get("redirect_uri"),
            Some(
                authorization_query
                    .get("redirect_uri")
                    .expect("authorization redirect URI")
            )
        );
        assert_eq!(
            authorization_query
                .get("code_challenge_method")
                .map(String::as_str),
            Some("S256")
        );
        let verifier = token_form.get("code_verifier").expect("PKCE verifier");
        let challenge = URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
        assert_eq!(authorization_query.get("code_challenge"), Some(&challenge));
        assert!(
            keyring
                .values
                .lock()
                .unwrap()
                .values()
                .all(|value| value.get_secret().is_err())
        );
        let attempt = tokio::sync::Mutex::new(Some("active"));
        {
            let mut authority = credentials
                .commit_if(|| async { Some(attempt.lock().await) })
                .await?;
            assert!(
                attempt.try_lock().is_err(),
                "commit retains the attempt gate until the caller retires it"
            );
            *authority = None;
        }
        assert_eq!(*attempt.lock().await, None);
        let stored = tracing::subscriber::with_default(
            tracing::subscriber::NoSubscriber::default(),
            || {
                crate::stored_oauth_credentials(
                    CREDENTIAL_NAME,
                    &issuer,
                    OAuthCredentialsStoreMode::Keyring,
                    AuthKeyringBackendKind::Direct,
                )
            },
        )?
        .expect("stored grant");
        assert_eq!(
            stored
                .token_response
                .0
                .refresh_token()
                .map(|token| token.secret().as_str()),
            Some(SECRET)
        );
        assert!(
            delete_enterprise_oauth_tokens(
                CREDENTIAL_NAME,
                &issuer,
                AuthKeyringBackendKind::Direct
            )
            .await?
        );
    }

    // Cancellation while blocked on the actual credential lock cannot leave a
    // detached persistence worker that writes after the other process releases it.
    let canceled = complete_login(&issuer).await?;
    let guard = EnterpriseOAuthCredentialGuard::acquire(
        CREDENTIAL_NAME,
        &issuer,
        AuthKeyringBackendKind::Direct,
    )
    .await?;
    let mut commit = Box::pin(canceled.commit_if(|| async { Some(()) }));
    assert!(futures::poll!(&mut commit).is_pending());
    drop(commit);
    drop(guard);
    assert!(
        keyring
            .values
            .lock()
            .unwrap()
            .values()
            .all(|value| value.get_secret().is_err())
    );

    // Rejected old attempts neither write nor delete a newer grant.
    let old = complete_login(&issuer).await?;
    complete_login(&issuer)
        .await?
        .commit_if(|| async { Some(()) })
        .await?;
    assert!(old.commit_if(|| async { None::<()> }).await.is_err());
    assert!(
        keyring
            .values
            .lock()
            .unwrap()
            .values()
            .any(|value| value.get_secret().is_ok())
    );

    // Inject raw account identifiers into the actual keyring adapter's error chain.
    keyring.fail.store(true, Ordering::SeqCst);
    let save_error = complete_login(&issuer)
        .await?
        .commit_if(|| async { Some(()) })
        .await
        .unwrap_err();
    let delete_error =
        delete_enterprise_oauth_tokens(CREDENTIAL_NAME, &issuer, AuthKeyringBackendKind::Direct)
            .await
            .unwrap_err();
    for error in [save_error, delete_error] {
        assert!(!format!("{error} {error:?} {error:#}").contains(SECRET));
    }
    keyring.fail.store(false, Ordering::SeqCst);
    let home = std::path::PathBuf::from(std::env::var("CODEX_HOME")?);
    assert!(
        !home.join(".credentials.json").exists(),
        "enterprise storage never falls back to plaintext"
    );
    let locks = home.join("mcp-oauth-locks");
    std::fs::rename(&locks, home.join("held-locks"))?;
    std::fs::write(&locks, b"not a directory")?;
    let lock_error =
        delete_enterprise_oauth_tokens(CREDENTIAL_NAME, &issuer, AuthKeyringBackendKind::Direct)
            .await
            .unwrap_err();
    assert!(!format!("{lock_error} {lock_error:?} {lock_error:#}").contains(SECRET));
    assert!(!logs_contain(SECRET));
    Ok(())
}

#[derive(Clone, Default)]
struct TestKeyring {
    values: Arc<Mutex<HashMap<String, Arc<MockCredential>>>>,
    fail: Arc<AtomicBool>,
}

struct TestCredential(Arc<MockCredential>);

impl CredentialApi for TestCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
        self.0.set_secret(secret)
    }
    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        self.0.get_secret()
    }
    fn delete_credential(&self) -> keyring::Result<()> {
        self.0.delete_credential()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CredentialBuilderApi for TestKeyring {
    fn build(
        &self,
        _target: Option<&str>,
        _service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(keyring::Error::Invalid("account".into(), user.into()));
        }
        Ok(Box::new(TestCredential(
            self.values
                .lock()
                .unwrap()
                .entry(user.into())
                .or_default()
                .clone(),
        )))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::ProcessOnly
    }
}
