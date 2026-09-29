//! Account API regressions for enterprise policy, persisted authority, and login attempts.

use super::*;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context;
use app_test_support::MockResponsesConfig;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_app_server::in_process;
use codex_app_server::in_process::InProcessClientHandle;
use codex_app_server::in_process::InProcessServerEvent;
use codex_app_server::in_process::InProcessStartArgs;
use codex_app_server_protocol::InitializeParams;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::McpEmaAuthScope;
use codex_config::McpServerIdpOAuthConfig;
use codex_config::NoopThreadConfigLoader;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_core::config::ConfigBuilder;
use codex_exec_server::EnvironmentManager;
use codex_features::Feature;
use codex_feedback::CodexFeedback;
use codex_login::CODEX_ACCESS_TOKEN_ENV_VAR;
use codex_protocol::protocol::SessionSource;
use codex_rmcp_client::stored_oauth_credentials;
use core_test_support::skip_if_no_network;
use keyring::credential::Credential;
use keyring::credential::CredentialApi;
use keyring::credential::CredentialBuilderApi;
use keyring::credential::CredentialPersistence;
use keyring::mock::MockCredential;
use pretty_assertions::assert_eq;
use serde_json::Value;
use test_case::test_case;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::header;

const WORKSPACE_ID_REFRESHED: &str = "123e4567-e89b-42d3-a456-426614174012";

#[test_case(false; "current_policy")]
#[test_case(true; "policy_load_failure")]
#[tokio::test]
async fn logout_reloads_persisted_workspace_and_its_enterprise_policy(
    policy_load_failure: bool,
) -> Result<()> {
    let server = MockServer::start().await;
    for (workspace, issuer) in [
        (WORKSPACE_ID_INITIAL, "https://initial-idp.example"),
        (WORKSPACE_ID_REFRESHED, "https://next-idp.example"),
    ] {
        let contents = if policy_load_failure && workspace == WORKSPACE_ID_REFRESHED {
            "invalid = [".to_string()
        } else {
            format!(
                r#"[mcp_enterprise_managed_auth.idp]
issuer = "{issuer}"
client_id = "enterprise-client""#
            )
        };
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/config/bundle"))
            .and(header("chatgpt-account-id", workspace))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    CloudConfigBundleFixture::enterprise_config(contents).into_bundle(),
                ),
            )
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/oauth/revoke"))
        .and(body_partial_json(json!({
            "token": "next-refresh-token",
            "token_type_hint": "refresh_token",
        })))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        CreateConfigTomlParams {
            chatgpt_base_url: Some(format!("{}/backend-api", server.uri())),
            ..Default::default()
        },
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("initial-token")
            .refresh_token("initial-refresh-token")
            .account_id(WORKSPACE_ID_INITIAL)
            .chatgpt_user_id("enterprise-user")
            .plan_type("business"),
        AuthCredentialsStoreMode::File,
    )?;
    // Keep the real platform keyring out of this API test. Enterprise cleanup
    // failure must still permit revocation and removal of the selected account.
    if !policy_load_failure {
        std::fs::write(codex_home.path().join("mcp-oauth-locks"), "not a directory")?;
    }
    let refresh_url = format!("{}/oauth/token", server.uri());
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[
            ("OPENAI_API_KEY", None),
            (REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR, Some(&refresh_url)),
        ])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    // Simulate a second process switching workspaces while this server caches A.
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("next-token")
            .refresh_token("next-refresh-token")
            .account_id(WORKSPACE_ID_REFRESHED)
            .chatgpt_user_id("enterprise-user")
            .plan_type("business"),
        AuthCredentialsStoreMode::File,
    )?;
    let id = app.send_logout_account_request().await?;
    let response: LogoutAccountResponse =
        timeout(DEFAULT_READ_TIMEOUT, app.read_response(id)).await??;
    assert_eq!(response, LogoutAccountResponse {});
    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_stream_until_notification_message("account/updated"),
    )
    .await??;
    let notification: ServerNotification = notification.try_into()?;
    let ServerNotification::AccountUpdated(account) = notification else {
        bail!("unexpected notification: {notification:?}");
    };
    assert_eq!(
        account,
        AccountUpdatedNotification {
            auth_mode: None,
            plan_type: None,
        }
    );
    assert!(!codex_home.path().join("auth.json").exists());
    if policy_load_failure {
        assert!(
            !codex_home.path().join("mcp-oauth-locks").exists(),
            "invalid current policy must not select any enterprise credential for deletion"
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enterprise_login_attempt_lifecycle() -> Result<()> {
    skip_if_no_network!(Ok(()));
    const CHILD: &str = "CODEX_ENTERPRISE_ATTEMPT_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // The app server and callback worker share only this child's mock keyring.
        let home = TempDir::new()?;
        let oauth_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/revoke"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&oauth_server)
            .await;
        let output = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "suite::v2::account::enterprise_tests::enterprise_login_attempt_lifecycle",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("CODEX_HOME", home.path())
            .env(
                REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
                format!("{}/oauth/token", oauth_server.uri()),
            )
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .env_remove(CODEX_ACCESS_TOKEN_ENV_VAR)
            .current_dir(home.path())
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed; 0 failed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }

    keyring::set_default_credential_builder(Box::<TestKeyring>::default());
    let server = MockServer::builder().start().await;
    let origin = server.uri();
    let issuer = format!("{origin}/idp");
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/bundle"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(CloudConfigBundleFixture::default().into_bundle()),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/idp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/authorize"),
            "token_endpoint": format!("{issuer}/token"),
            "token_endpoint_auth_methods_supported": ["none"],
        })))
        .mount(&server)
        .await;
    let assertion = format!(
        "{}.{}.signature",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(
            json!({"iss":issuer,"sub":"enterprise-user","aud":"idp-client","exp":4102444800_u64})
                .to_string()
        )
    );
    Mock::given(method("POST"))
        .and(path("/idp/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token":"enterprise-access", "refresh_token":"enterprise-refresh",
            "id_token":assertion, "token_type":"Bearer",
        })))
        .mount(&server)
        .await;

    let home = TempDir::new()?;
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let callback_port = listener.local_addr()?.port();
    MockResponsesConfig::new(&origin)
        .enable_feature(Feature::UseXaa)
        .disable_feature(Feature::SecretAuthStorage)
        .disable_feature(Feature::Apps)
        .with_root_config(&format!(
            r#"cli_auth_credentials_store = "file"
chatgpt_base_url = "{origin}/backend-api"
mcp_oauth_callback_port = {callback_port}
analytics = {{ enabled = false }}"#
        ))
        .with_provider_config("requires_openai_auth = true")
        .with_extra_config(&format!(
            r#"[mcp_enterprise_managed_auth.idp]
issuer = "{issuer}"
client_id = "idp-client"

[mcp_servers.enterprise]
url = "{origin}/mcp"
auth = "ema_auth"
scopes = ["files.read"]
oauth = {{ client_id = "mcp-client", authorization_server_issuer = "{origin}/as" }}

[mcp_servers.disabled-enterprise]
url = "{origin}/mcp"
enabled = false
auth = "ema_auth"
oauth = {{ client_id = "mcp-client", authorization_server_issuer = "{origin}/as" }}"#
        ))
        .write(home.path())?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("account-access")
            .account_id(WORKSPACE_ID_INITIAL)
            .chatgpt_user_id("enterprise-user")
            .plan_type("business"),
        AuthCredentialsStoreMode::File,
    )?;
    let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    let config = Arc::new(
        ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .fallback_cwd(Some(home.path().to_path_buf()))
            .loader_overrides(loader_overrides.clone())
            .build()
            .await?,
    );
    let mut client = in_process::start(InProcessStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config,
        cli_overrides: Vec::new(),
        loader_overrides,
        strict_config: false,
        cloud_config_bundle: CloudConfigBundleLoader::default(),
        thread_config_loader: Arc::new(NoopThreadConfigLoader),
        feedback: CodexFeedback::new(),
        log_db: None,
        state_db: None,
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        config_warnings: Vec::new(),
        embedded_network_policy: Default::default(),
        session_source: SessionSource::Cli,
        enable_codex_api_key_env: false,
        initialize: InitializeParams {
            client_info: ClientInfo {
                name: "codex-app-server-tests".into(),
                title: None,
                version: "0.1.0".into(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: true,
                ..Default::default()
            }),
        },
        channel_capacity: in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await?;
    drop(listener);
    let thread = enterprise_rpc(&client, "thread/start", json!({})).await?;
    let thread_id = thread["thread"]["id"].as_str().context("thread ID")?;

    assert_eq!(
        enterprise_rpc(
            &client,
            "mcpServer/oauth/login",
            json!({"name":"disabled-enterprise","threadId":thread_id}),
        )
        .await
        .expect_err("disabled enterprise MCP login must reject immediately")
        .to_string(),
        "mcpServer/oauth/login: MCP server 'disabled-enterprise' is disabled."
    );
    let first = enterprise_rpc(
        &client,
        "mcpServer/oauth/login",
        json!({"name":"enterprise","threadId":thread_id}),
    )
    .await?;
    enterprise_callback(&first, &issuer, callback_port)?;
    let replacement = enterprise_rpc(
        &client,
        "mcpServer/oauth/login",
        json!({"name":"enterprise","threadId":thread_id}),
    )
    .await?;
    assert_ne!(replacement["loginId"], first["loginId"]);
    enterprise_callback(&replacement, &issuer, callback_port)?;
    assert_eq!(
        enterprise_completion(&mut client).await?,
        json!({"name":"enterprise","threadId":thread_id,"loginId":first["loginId"],"success":false,"error":"Enterprise sign-in failed."})
    );
    assert_eq!(
        enterprise_rpc(
            &client,
            "account/login/cancel",
            json!({"loginId":replacement["loginId"]}),
        )
        .await?,
        json!({"status":"canceled"})
    );
    assert_eq!(
        enterprise_completion(&mut client).await?,
        json!({"name":"enterprise","threadId":thread_id,"loginId":replacement["loginId"],"success":false,"error":"Enterprise sign-in failed."})
    );
    let idp = McpServerIdpOAuthConfig {
        issuer: issuer.clone(),
        client_id: "idp-client".into(),
    };
    let initial_scope = McpEmaAuthScope::new("enterprise-user".into(), WORKSPACE_ID_INITIAL.into())
        .expect("initial scope");
    assert_eq!(
        stored_oauth_credentials(
            &idp.credential_name(&initial_scope),
            &issuer,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?,
        None
    );

    let previous = enterprise_rpc(
        &client,
        "mcpServer/oauth/login",
        json!({"name":"enterprise","threadId":thread_id}),
    )
    .await?;
    let previous_callback = enterprise_callback(&previous, &issuer, callback_port)?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("account-access")
            .account_id(WORKSPACE_ID_REFRESHED)
            .chatgpt_user_id("enterprise-user")
            .plan_type("business"),
        AuthCredentialsStoreMode::File,
    )?;
    let browser = HttpClientBuilder::new()
        .without_request_logging()
        .without_redirects()
        .build_direct()?;
    // Each attempt owns a one-shot listener on this port. Do not reuse an HTTP
    // connection from the previous callback after that listener has retired.
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        browser
            .get(previous_callback)
            .header(http::header::CONNECTION, "close")
            .send(),
    )
    .await??;
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        enterprise_completion(&mut client).await?,
        json!({"name":"enterprise","threadId":thread_id,"loginId":previous["loginId"],"success":false,"error":"Enterprise sign-in failed."})
    );

    let retry = enterprise_rpc(
        &client,
        "mcpServer/oauth/login",
        json!({"name":"enterprise","threadId":thread_id}),
    )
    .await?;
    let retry_callback = enterprise_callback(&retry, &issuer, callback_port)?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        browser
            .get(retry_callback)
            .header(http::header::CONNECTION, "close")
            .send(),
    )
    .await??;
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        enterprise_completion(&mut client).await?,
        json!({"name":"enterprise","threadId":thread_id,"loginId":retry["loginId"],"success":true})
    );
    let replacement_scope =
        McpEmaAuthScope::new("enterprise-user".into(), WORKSPACE_ID_REFRESHED.into())
            .expect("replacement scope");
    for (scope, present) in [(initial_scope, false), (replacement_scope, true)] {
        assert_eq!(
            stored_oauth_credentials(
                &idp.credential_name(&scope),
                &issuer,
                OAuthCredentialsStoreMode::Keyring,
                AuthKeyringBackendKind::Direct,
            )?
            .is_some(),
            present
        );
    }

    let stalled = enterprise_rpc(
        &client,
        "mcpServer/oauth/login",
        json!({"name":"enterprise","threadId":thread_id}),
    )
    .await?;
    let mut unfinished = tokio::net::TcpStream::connect(("127.0.0.1", callback_port)).await?;
    unfinished
        .write_all(b"POST /unfinished HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 2048\r\n\r\n")
        .await?;
    // Reading the response proves the callback worker has accepted this request.
    // It cannot finish draining the declared body while this socket stays open.
    let mut response = [0; 256];
    assert!(timeout(DEFAULT_READ_TIMEOUT, unfinished.read(&mut response)).await?? > 0);
    let logout = timeout(
        std::time::Duration::from_secs(15),
        enterprise_rpc(&client, "account/logout", json!({})),
    )
    .await;
    // Release the worker even when the regression fails, so child shutdown cannot hang.
    drop(unfinished);
    logout.context("an unfinished callback must not prevent primary logout")??;
    assert!(!home.path().join("auth.json").exists());
    assert!(
        stored_oauth_credentials(
            &idp.credential_name(
                &McpEmaAuthScope::new("enterprise-user".into(), WORKSPACE_ID_REFRESHED.into(),)
                    .expect("replacement scope")
            ),
            &issuer,
            OAuthCredentialsStoreMode::Keyring,
            AuthKeyringBackendKind::Direct,
        )?
        .is_none()
    );
    assert_eq!(
        enterprise_completion(&mut client).await?,
        json!({"name":"enterprise","threadId":thread_id,"loginId":stalled["loginId"],"success":false,"error":"Enterprise sign-in failed."})
    );
    client.shutdown().await?;
    Ok(())
}

fn enterprise_callback(start: &Value, issuer: &str, callback_port: u16) -> Result<Url> {
    let query = Url::parse(
        start["authorizationUrl"]
            .as_str()
            .context("authorization URL")?,
    )?
    .query_pairs()
    .into_owned()
    .collect::<HashMap<_, _>>();
    let mut callback = Url::parse(query.get("redirect_uri").context("redirect_uri")?)?;
    assert_eq!(
        (callback.scheme(), callback.host_str(), callback.port()),
        ("http", Some("127.0.0.1"), Some(callback_port))
    );
    callback
        .query_pairs_mut()
        .append_pair("code", "enterprise-code")
        .append_pair("state", query.get("state").context("state")?)
        .append_pair("iss", issuer);
    Ok(callback)
}

async fn enterprise_rpc(
    client: &InProcessClientHandle,
    method: &str,
    params: Value,
) -> Result<Value> {
    let request = serde_json::from_value(
        json!({"id":uuid::Uuid::new_v4().to_string(),"method":method,"params":params}),
    )?;
    let response = timeout(DEFAULT_READ_TIMEOUT, client.request(request))
        .await
        .with_context(|| format!("timed out waiting for {method}"))??;
    response.map_err(|error| anyhow::anyhow!("{method}: {}", error.message))
}

async fn enterprise_completion(client: &mut InProcessClientHandle) -> Result<Value> {
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            match client
                .next_event()
                .await
                .context("app-server notification")?
            {
                InProcessServerEvent::ServerNotification(notification) => {
                    if let ServerNotification::McpServerOauthLoginCompleted(notification) =
                        *notification
                    {
                        return Ok(serde_json::to_value(notification)?);
                    }
                }
                event => bail!("unexpected app-server event: {event:?}"),
            }
        }
    })
    .await
    .context("timed out waiting for enterprise login completion")?
}

#[derive(Default)]
struct TestKeyring(Mutex<HashMap<(String, String), Arc<MockCredential>>>);

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
        service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        let credential = self
            .0
            .lock()
            .expect("mock keyring")
            .entry((service.into(), user.into()))
            .or_default()
            .clone();
        Ok(Box::new(TestCredential(credential)))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::ProcessOnly
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enterprise_login_setup_is_cancelled_by_account_changes() -> Result<()> {
    skip_if_no_network!(Ok(()));
    const CHILD: &str = "CODEX_ENTERPRISE_SETUP_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // The app server and callback worker share only this child's mock keyring.
        let home = TempDir::new()?;
        let oauth_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/revoke"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0..=1)
            .mount(&oauth_server)
            .await;
        let output = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "suite::v2::account::enterprise_tests::enterprise_login_setup_is_cancelled_by_account_changes",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("CODEX_HOME", home.path())
            .env(
                REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
                format!("{}/oauth/token", oauth_server.uri()),
            )
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .env_remove(CODEX_ACCESS_TOKEN_ENV_VAR)
            .current_dir(home.path())
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed; 0 failed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }

    keyring::set_default_credential_builder(Box::<TestKeyring>::default());
    for (account_method, account_params) in [
        ("account/logout", json!({})),
        (
            "account/login/start",
            json!({"type":"apiKey","apiKey":"replacement-api-key"}),
        ),
    ] {
        let server = MockServer::builder().start().await;
        let origin = server.uri();
        let issuer = format!("{origin}/idp");
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/config/bundle"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(CloudConfigBundleFixture::default().into_bundle()),
            )
            .mount(&server)
            .await;
        let discovery_started = Arc::new(tokio::sync::Notify::new());
        let entered = Arc::clone(&discovery_started);
        Mock::given(method("GET"))
            .and(path("/.well-known/oauth-authorization-server/idp"))
            .respond_with(move |_: &wiremock::Request| {
                entered.notify_one();
                ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(60))
            })
            .mount(&server)
            .await;
        let assertion = format!(
        "{}.{}.signature",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(
            json!({"iss":issuer,"sub":"enterprise-user","aud":"idp-client","exp":4102444800_u64})
                .to_string()
        )
    );
        Mock::given(method("POST"))
            .and(path("/idp/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token":"enterprise-access", "refresh_token":"enterprise-refresh",
                "id_token":assertion, "token_type":"Bearer",
            })))
            .mount(&server)
            .await;

        let home = TempDir::new()?;
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let callback_port = listener.local_addr()?.port();
        MockResponsesConfig::new(&origin)
            .enable_feature(Feature::UseXaa)
            .disable_feature(Feature::SecretAuthStorage)
            .disable_feature(Feature::Apps)
            .with_root_config(&format!(
                r#"cli_auth_credentials_store = "file"
chatgpt_base_url = "{origin}/backend-api"
mcp_oauth_callback_port = {callback_port}
analytics = {{ enabled = false }}"#
            ))
            .with_provider_config("requires_openai_auth = true")
            .with_extra_config(&format!(
                r#"[mcp_enterprise_managed_auth.idp]
issuer = "{issuer}"
client_id = "idp-client"

[mcp_servers.enterprise]
url = "{origin}/mcp"
auth = "ema_auth"
scopes = ["files.read"]
oauth = {{ client_id = "mcp-client", authorization_server_issuer = "{origin}/as" }}

[mcp_servers.disabled-enterprise]
url = "{origin}/mcp"
enabled = false
auth = "ema_auth"
oauth = {{ client_id = "mcp-client", authorization_server_issuer = "{origin}/as" }}"#
            ))
            .write(home.path())?;
        write_chatgpt_auth(
            home.path(),
            ChatGptAuthFixture::new("account-access")
                .account_id(WORKSPACE_ID_INITIAL)
                .chatgpt_user_id("enterprise-user")
                .plan_type("business"),
            AuthCredentialsStoreMode::File,
        )?;
        let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
        let config = Arc::new(
            ConfigBuilder::default()
                .codex_home(home.path().to_path_buf())
                .fallback_cwd(Some(home.path().to_path_buf()))
                .loader_overrides(loader_overrides.clone())
                .build()
                .await?,
        );
        let client = in_process::start(InProcessStartArgs {
            arg0_paths: Arg0DispatchPaths::default(),
            config,
            cli_overrides: Vec::new(),
            loader_overrides,
            strict_config: false,
            cloud_config_bundle: CloudConfigBundleLoader::default(),
            thread_config_loader: Arc::new(NoopThreadConfigLoader),
            feedback: CodexFeedback::new(),
            log_db: None,
            state_db: None,
            environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
            config_warnings: Vec::new(),
            embedded_network_policy: Default::default(),
            session_source: SessionSource::Cli,
            enable_codex_api_key_env: false,
            initialize: InitializeParams {
                client_info: ClientInfo {
                    name: "codex-app-server-tests".into(),
                    title: None,
                    version: "0.1.0".into(),
                },
                capabilities: Some(InitializeCapabilities {
                    experimental_api: true,
                    ..Default::default()
                }),
            },
            channel_capacity: in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
        })
        .await?;
        drop(listener);
        let thread = enterprise_rpc(&client, "thread/start", json!({})).await?;
        let thread_id = thread["thread"]["id"].as_str().context("thread ID")?;
        let (setup_result, account_result) = tokio::join!(
            enterprise_rpc(
                &client,
                "mcpServer/oauth/login",
                json!({"name":"enterprise","threadId":thread_id}),
            ),
            async {
                timeout(DEFAULT_READ_TIMEOUT, discovery_started.notified()).await?;
                timeout(
                    std::time::Duration::from_secs(15),
                    enterprise_rpc(&client, account_method, account_params),
                )
                .await
                .context("account changes must not wait for IdP discovery")?
            },
        );
        account_result.context("stalled enterprise setup must not block account changes")?;
        assert_eq!(
            setup_result
                .expect_err("account changes cancel pending enterprise setup")
                .to_string(),
            "mcpServer/oauth/login: enterprise sign-in was cancelled",
        );
        let idp = McpServerIdpOAuthConfig {
            issuer: issuer.clone(),
            client_id: "idp-client".into(),
        };
        let scope =
            McpEmaAuthScope::new("enterprise-user".into(), WORKSPACE_ID_INITIAL.into()).unwrap();
        assert_eq!(
            stored_oauth_credentials(
                &idp.credential_name(&scope),
                &issuer,
                OAuthCredentialsStoreMode::Keyring,
                AuthKeyringBackendKind::Direct,
            )?,
            None
        );
        client.shutdown().await?;
    }
    Ok(())
}
