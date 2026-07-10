mod streamable_http_test_support;

use std::any::Any;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthTokens;
use codex_rmcp_client::WrappedOAuthTokenResponse;
use codex_rmcp_client::save_oauth_tokens;
use futures::future::BoxFuture;
use keyring::credential::Credential;
use keyring::credential::CredentialApi;
use keyring::credential::CredentialBuilderApi;
use keyring::credential::CredentialPersistence;
use oauth2::AccessToken;
use oauth2::basic::BasicTokenType;
use pretty_assertions::assert_eq;
use rmcp::transport::auth::OAuthTokenResponse;
use rmcp::transport::auth::VendorExtraTokenFields;
use tempfile::TempDir;
use tokio::process::Command;

use streamable_http_test_support::arm_session_post_failure;
use streamable_http_test_support::call_echo_tool;
use streamable_http_test_support::expected_echo_result;
use streamable_http_test_support::initialize_client;
use streamable_http_test_support::spawn_streamable_http_server;

const SERVER_NAME: &str = "test-streamable-http-oauth-store-pinning";
const CHILD_SERVER_URL_ENV: &str = "MCP_TEST_OAUTH_PINNED_STORE_SERVER_URL";
const KEYRING_ACCESS_TOKEN: &str = "keyring-access-token";
const FILE_ACCESS_TOKEN: &str = "stale-file-access-token";

#[derive(Clone)]
struct RecordingHttpClient {
    inner: Arc<dyn HttpClient>,
    bearer_tokens: Arc<Mutex<Vec<String>>>,
}

impl RecordingHttpClient {
    fn new(inner: Arc<dyn HttpClient>) -> Self {
        Self {
            inner,
            bearer_tokens: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record_bearer_token(&self, params: &HttpRequestParams) {
        let Some(header) = params
            .headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case("authorization"))
        else {
            return;
        };
        self.bearer_tokens
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(header.value.clone());
    }

    fn bearer_tokens(&self) -> Vec<String> {
        self.bearer_tokens
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl HttpClient for RecordingHttpClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        self.record_bearer_token(&params);
        self.inner.http_request(params)
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        self.record_bearer_token(&params);
        self.inner.http_request_stream(params)
    }
}

#[derive(Debug, Default)]
struct TestKeyringState {
    secret: Mutex<Option<Vec<u8>>>,
    fail_reads: AtomicBool,
}

#[derive(Clone, Debug)]
struct TestCredential {
    state: Arc<TestKeyringState>,
}

impl CredentialApi for TestCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
        *self
            .state
            .secret
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(secret.to_vec());
        Ok(())
    }

    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        if self.state.fail_reads.load(Ordering::SeqCst) {
            return Err(keyring::Error::Invalid(
                "simulated keyring read failure".to_string(),
                "load".to_string(),
            ));
        }

        self.state
            .secret
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or(keyring::Error::NoEntry)
    }

    fn delete_credential(&self) -> keyring::Result<()> {
        self.state
            .secret
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .map(|_| ())
            .ok_or(keyring::Error::NoEntry)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct TestCredentialBuilder {
    state: Arc<TestKeyringState>,
}

impl CredentialBuilderApi for TestCredentialBuilder {
    fn build(
        &self,
        _target: Option<&str>,
        _service: &str,
        _user: &str,
    ) -> keyring::Result<Box<Credential>> {
        Ok(Box::new(TestCredential {
            state: Arc::clone(&self.state),
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::ProcessOnly
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn auto_store_remains_pinned_across_session_recovery() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let codex_home = TempDir::new()?;

    let status = Command::new(std::env::current_exe()?)
        .args([
            "auto_store_remains_pinned_across_session_recovery_child",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .env("CODEX_HOME", codex_home.path())
        .env(CHILD_SERVER_URL_ENV, &base_url)
        .status()
        .await?;

    assert!(
        status.success(),
        "OAuth store-pinning child failed: {status}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore = "spawned by auto_store_remains_pinned_across_session_recovery"]
async fn auto_store_remains_pinned_across_session_recovery_child() -> anyhow::Result<()> {
    let state = Arc::new(TestKeyringState::default());
    keyring::set_default_credential_builder(Box::new(TestCredentialBuilder {
        state: Arc::clone(&state),
    }));

    let base_url = std::env::var(CHILD_SERVER_URL_ENV)?;
    let server_url = format!("{base_url}/mcp");
    let keyring_tokens = stored_tokens(&server_url, KEYRING_ACCESS_TOKEN);
    save_oauth_tokens(
        SERVER_NAME,
        &keyring_tokens,
        OAuthCredentialsStoreMode::Keyring,
        AuthKeyringBackendKind::Direct,
    )?;
    let file_tokens = stored_tokens(&server_url, FILE_ACCESS_TOKEN);
    save_oauth_tokens(
        SERVER_NAME,
        &file_tokens,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::Direct,
    )?;
    let http_client = RecordingHttpClient::new(Environment::default_for_tests().get_http_client());

    let client = RmcpClient::new_streamable_http_client(
        SERVER_NAME,
        &server_url,
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::Auto,
        AuthKeyringBackendKind::Direct,
        Arc::new(http_client.clone()),
        /*auth_provider*/ None,
    )
    .await?;
    initialize_client(&client).await?;
    assert_eq!(
        call_echo_tool(&client, "warmup").await?,
        expected_echo_result("warmup")
    );

    arm_session_post_failure(
        &base_url,
        /*status*/ 404,
        /*remaining*/ 1,
        /*www_authenticate_headers*/ &[],
    )
    .await?;
    // The selected keyring becomes unavailable only after initial construction. If recovery
    // reevaluates Auto, it adopts the stale File token and this operation incorrectly succeeds.
    state.fail_reads.store(true, Ordering::SeqCst);

    match call_echo_tool(&client, "recovery-must-not-fallback").await {
        Ok(result) => assert_eq!(result, expected_echo_result("recovery-must-not-fallback")),
        Err(error) => {
            let error_chain = format!("{error:#}");
            assert!(
                error_chain.contains("failed to reread OAuth tokens from resolved keyring storage"),
                "unexpected recovery error: {error_chain}"
            );
        }
    }

    let bearer_tokens = http_client.bearer_tokens();
    assert!(
        bearer_tokens
            .iter()
            .any(|token| token == &format!("Bearer {KEYRING_ACCESS_TOKEN}")),
        "expected requests authenticated by the keyring token: {bearer_tokens:?}"
    );
    assert!(
        bearer_tokens
            .iter()
            .all(|token| token != &format!("Bearer {FILE_ACCESS_TOKEN}")),
        "stale File token must never be sent during recovery: {bearer_tokens:?}"
    );
    Ok(())
}

fn stored_tokens(server_url: &str, access_token: &str) -> StoredOAuthTokens {
    StoredOAuthTokens {
        server_name: SERVER_NAME.to_string(),
        url: server_url.to_string(),
        client_id: "test-client-id".to_string(),
        token_response: WrappedOAuthTokenResponse(OAuthTokenResponse::new(
            AccessToken::new(access_token.to_string()),
            BasicTokenType::Bearer,
            VendorExtraTokenFields::default(),
        )),
        expires_at: None,
    }
}
