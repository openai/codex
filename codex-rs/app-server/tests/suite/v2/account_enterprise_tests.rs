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
use codex_app_server_protocol::McpServerStartupState;
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
use core_test_support::responses;
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

    let keyring = TestKeyring::default();
    let account_read = Arc::clone(&keyring.account_read);
    keyring::set_default_credential_builder(Box::new(keyring));
    let server = MockServer::builder().start().await;
    let idp_server = MockServer::start().await;
    app_test_support::mount_workspace_routing(&server).await;
    let origin = server.uri();
    let issuer = format!("{}/idp", idp_server.uri());
    let resource = format!("{origin}/api/codex/ps/mcp");
    core_test_support::apps_test_server::AppsTestServer::mount(&server).await?;
    let catalog_meta = json!({"enterpriseCatalog": "calendar"});
    let response_meta = catalog_meta.clone();
    Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .and(wiremock::matchers::header_regex(
            "authorization",
            "^Bearer enterprise-resource-",
        ))
        .and(body_partial_json(json!({"method": "tools/list"})))
        .respond_with(move |request: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0", "id": body["id"],
                "result": {
                    "tools": [{"name": "calendar_list_events", "inputSchema": {"type": "object"}}],
                    "_meta": response_meta,
                },
            }))
        })
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/.well-known/oauth-protected-resource/api/codex/ps/mcp",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "resource": resource, "authorization_servers": [format!("{origin}/as")],
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/as"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": format!("{origin}/as"), "token_endpoint": format!("{origin}/as/token"),
            "grant_types_supported": ["urn:ietf:params:oauth:grant-type:jwt-bearer"],
            "authorization_grant_profiles_supported": ["urn:ietf:params:oauth:grant-profile:id-jag"],
            "token_endpoint_auth_methods_supported": ["none", "client_secret_basic", "client_secret_post"],
        })))
        .mount(&server)
        .await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let jag = format!(
        "{}.{}.c2lnbmF0dXJl",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256","typ":"oauth-id-jag+jwt"}"#),
        URL_SAFE_NO_PAD.encode(
            json!({
                "iss": issuer, "aud": format!("{origin}/as"), "sub": "enterprise-user",
                "client_id": "mcp-client", "jti": "enterprise-grant", "iat": now,
                "exp": now + 3600, "resource": resource, "scope": "files.read",
            })
            .to_string()
        ),
    );
    let expected_jag = jag.clone();
    let resource_exchanges = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let exchanges = Arc::clone(&resource_exchanges);
    let pending_account_refresh = Arc::new(Mutex::new(std::collections::VecDeque::<(
        std::path::PathBuf,
        Vec<u8>,
    )>::new()));
    let account_refresh = Arc::clone(&pending_account_refresh);
    Mock::given(method("POST")).and(path("/as/token"))
        .respond_with(move |request: &wiremock::Request| {
            let fields = url::form_urlencoded::parse(&request.body).into_owned().collect::<HashMap<_, _>>();
            // EMA section 5: the resource AS gets the ID-JAG, never the IdP credential.
            let expected = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode("mcp-client:resource-secret"));
            assert_eq!(request.headers.get("authorization").unwrap().to_str().unwrap(), expected);
            assert_eq!(serde_json::to_value(fields).unwrap(), json!({
                "grant_type": "urn:ietf:params:oauth:grant-type:jwt-bearer",
                "assertion": expected_jag,
            }));
            if let Some((path, auth)) = account_refresh.lock().unwrap().pop_front() {
                // Another process refreshes the account while the resource exchange runs.
                std::fs::write(path, auth).expect("publish account refresh");
            }
            ResponseTemplate::new(200).set_body_json(json!({
                "access_token": format!("enterprise-resource-{}", exchanges.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1),
                "token_type": "Bearer", "expires_in": 3600,
            }))
        }).mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .and(header("authorization", "Bearer enterprise-resource-1"))
        .and(body_partial_json(json!({"method": "tools/call"})))
        .respond_with(ResponseTemplate::new(401))
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;
    let token_resource = resource.clone();
    let token_audience = format!("{origin}/as");
    let pending_exchange = Arc::new(Mutex::new(
        None::<(
            tokio::sync::oneshot::Sender<()>,
            std::sync::mpsc::Receiver<()>,
        )>,
    ));
    let exchange_gate = Arc::clone(&pending_exchange);
    Mock::given(method("POST"))
        .and(path("/idp/token"))
        .and(wiremock::matchers::body_string_contains(
            "grant-type%3Atoken-exchange",
        ))
        .respond_with(move |request: &wiremock::Request| {
            let mut fields = url::form_urlencoded::parse(&request.body)
                .into_owned()
                .collect::<HashMap<_, _>>();
            fields.remove("client_secret");
            let subject = fields.get("subject_token").map(String::as_str);
            assert!(matches!(
                subject,
                Some(
                    "enterprise-refresh"
                        | "discovered-account-refresh"
                        | "rotated-account-refresh"
                        | "exchanged-account-refresh"
                        | "retried-account-refresh"
                )
            ));
            // EMA section 4: audience is the authorization server; resource is the MCP server.
            assert!(request.headers.get("authorization").is_none());
            assert_eq!(serde_json::to_value(&fields).unwrap(), json!({
                "grant_type": "urn:ietf:params:oauth:grant-type:token-exchange",
                "requested_token_type": "urn:ietf:params:oauth:token-type:id-jag",
                "subject_token_type": "urn:ietf:params:oauth:token-type:refresh_token",
                "subject_token": subject,
                "client_id": if subject == Some("enterprise-refresh") { "idp-client" } else { "account-client" },
                "audience": token_audience,
                "resource": token_resource,
                "scope": "files.read",
            }));
            if let Some((arrived, release)) = exchange_gate.lock().unwrap().take() {
                let _ = arrived.send(());
                let _ = release.recv();
            }
            ResponseTemplate::new(200).set_body_json(json!({
                "access_token": jag, "issued_token_type": "urn:ietf:params:oauth:token-type:id-jag",
                "token_type": "N_A", "resource": token_resource, "scope": "files.read",
            }))
        })
        .with_priority(2)
        .mount(&idp_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/bundle"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(CloudConfigBundleFixture::default().into_bundle()),
        )
        .mount(&server)
        .await;
    let pending_discovery = Arc::new(Mutex::new(
        None::<(
            tokio::sync::oneshot::Sender<()>,
            std::sync::mpsc::Receiver<()>,
        )>,
    ));
    let discovery_gate = Arc::clone(&pending_discovery);
    let idp_metadata = json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "token_endpoint_auth_methods_supported": ["none", "client_secret_basic", "client_secret_post"],
    });
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/idp"))
        .respond_with(move |_: &wiremock::Request| {
            if let Some((arrived, release)) = discovery_gate.lock().unwrap().take() {
                let _ = arrived.send(());
                let _ = release.recv();
            }
            ResponseTemplate::new(200).set_body_json(idp_metadata.clone())
        })
        .mount(&idp_server)
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
        .mount(&idp_server)
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
client_secret = "idp-secret"
token_endpoint_auth_method = "client_secret_post"

[mcp_servers.enterprise]
url = "{resource}"
auth = "ema_auth"
scopes = ["files.read"]
oauth = {{ client_id = "mcp-client", client_secret = "resource-secret", token_endpoint_auth_method = "client_secret_basic", authorization_server_issuer = "{origin}/as" }}

[mcp_servers.ordinary]
url = "{resource}"
http_headers = {{ Authorization = "Bearer ordinary-access" }}

[mcp_servers.disabled-enterprise]
url = "{origin}/mcp"
enabled = false
auth = "ema_auth"
oauth = {{ client_id = "mcp-client", client_secret = "resource-secret", token_endpoint_auth_method = "client_secret_basic", authorization_server_issuer = "{origin}/as" }}"#
        ))
        .write(home.path())?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("account-access")
            .login_owner_id("original-account-login")
            .account_id(WORKSPACE_ID_INITIAL)
            .chatgpt_user_id("enterprise-user")
            .plan_type("business"),
        AuthCredentialsStoreMode::File,
    )?;
    let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .fallback_cwd(Some(home.path().to_path_buf()))
        .loader_overrides(loader_overrides.clone())
        .build()
        .await?;
    // Development builds otherwise force file storage, bypassing the mock keyring.
    config.cli_auth_credentials_store_mode = AuthCredentialsStoreMode::Auto;
    let state_db = codex_rollout::state_db::try_init(&config).await?;
    let config = Arc::new(config);
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
        state_db: Some(state_db),
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
        client_secret: None,
        token_endpoint_auth_method: None,
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
            .login_owner_id("original-account-login")
            .account_id(WORKSPACE_ID_REFRESHED)
            .chatgpt_user_id("enterprise-user")
            .plan_type("business"),
        AuthCredentialsStoreMode::File,
    )?;
    // Same issuer, different client: the dedicated confidential grant must win.
    let auth_path = home.path().join("auth.json");
    let mut account_auth: Value = serde_json::from_slice(&std::fs::read(&auth_path)?)?;
    account_auth["tokens"]["oauth_provenance"] =
        json!({"issuer":issuer,"client_id":"account-client"});
    account_auth["tokens"]["id_token"] = json!(format!("{}.{}.signature",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(json!({"iss":issuer,"aud":"account-client","sub":"enterprise-user","exp":now + 3600,
            "https://api.openai.com/auth":{"chatgpt_user_id":"enterprise-user","chatgpt_account_id":WORKSPACE_ID_REFRESHED,"chatgpt_plan_type":"business"}}).to_string())));
    std::fs::write(&auth_path, serde_json::to_vec(&account_auth)?)?;
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

    // A fresh thread uses the saved enterprise grant and renews it for MCP requests.
    let connected =
        enterprise_rpc(&client, "thread/start", json!({"historyMode":"paginated"})).await?;
    let connected_id = connected["thread"]["id"]
        .as_str()
        .context("connected thread ID")?;
    let call = |server: &str| {
        json!({
            "threadId": connected_id, "server": server,
            "tool": "calendar_list_events", "arguments": {"query": "hello"},
        })
    };
    // A rejected operation is not replayed. The next explicit call renews its bearer.
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", call("enterprise")).await?["isError"],
        true
    );
    let enterprise_result =
        enterprise_rpc(&client, "mcpServer/tool/call", call("enterprise")).await?;
    let ordinary_result = enterprise_rpc(&client, "mcpServer/tool/call", call("ordinary")).await?;
    assert_eq!(enterprise_result, ordinary_result);

    // Revert replaces the runtime without revoking its replacement's EMA access.
    let _response =
        responses::mount_sse_once(&server, responses::sse_completed("revert-turn")).await;
    let turn = enterprise_rpc(
        &client,
        "turn/start",
        json!({"threadId":connected_id,"input":[{"type":"text","text":"Before revert"}]}),
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            if let InProcessServerEvent::ServerNotification(notification) =
                client.next_event().await.context("turn notification")?
                && let ServerNotification::TurnCompleted(completed) = *notification
                && completed.thread_id == connected_id
            {
                assert_eq!(
                    completed.turn.status,
                    codex_app_server_protocol::TurnStatus::Completed
                );
                return anyhow::Ok(());
            }
        }
    })
    .await??;
    enterprise_rpc(
        &client,
        "thread/revert",
        json!({"threadId":connected_id,"beforeTurnId":turn["turn"]["id"]}),
    )
    .await?;
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", call("enterprise"))
            .await
            .context("enterprise tools must survive revert")?,
        enterprise_result
    );
    let bulk_status = enterprise_rpc(
        &client,
        "mcpServerStatus/list",
        json!({"threadId": connected_id}),
    )
    .await?;
    let statuses = bulk_status["data"].as_array().context("server statuses")?;
    for name in ["enterprise", "ordinary"] {
        let selected_status = enterprise_rpc(
            &client,
            "mcpServerStatus/list",
            json!({"threadId": connected_id, "serverName": name}),
        )
        .await?;
        let status = statuses
            .iter()
            .find(|status| status["name"] == name)
            .context("connected server status")?;
        assert_eq!(status, &selected_status["data"][0]);
        assert!(!status["tools"]["calendar_list_events"].is_null());
        if name == "enterprise" {
            assert_eq!(status["authStatus"], "oAuth");
            assert_eq!(status["toolListMeta"], catalog_meta);
        }
    }
    // Bulk inventory must not attach the old connection's catalog to an edited registration.
    let inventory_request_count = |requests: &[wiremock::Request]| {
        requests
            .iter()
            .filter(|request| {
                request.headers.get("authorization").is_some_and(|value| {
                    value.as_bytes().starts_with(b"Bearer enterprise-resource-")
                }) && serde_json::from_slice::<Value>(&request.body).is_ok_and(|body| {
                    matches!(
                        body["method"].as_str(),
                        Some("resources/list" | "resources/templates/list")
                    )
                })
            })
            .count()
    };
    let inventory_requests_before =
        inventory_request_count(&server.received_requests().await.context("requests")?);
    assert!(inventory_requests_before > 0);
    let config_path = home.path().join("config.toml");
    let saved_config = std::fs::read_to_string(&config_path)?;
    let mut edited_config: toml::Value = toml::from_str(&saved_config)?;
    for (url, enabled) in [
        ("https://replacement.example/mcp", true),
        (resource.as_str(), false),
    ] {
        let enterprise = edited_config["mcp_servers"]["enterprise"]
            .as_table_mut()
            .context("enterprise config")?;
        enterprise.insert("url".into(), toml::Value::String(url.into()));
        enterprise.insert("enabled".into(), toml::Value::Boolean(enabled));
        std::fs::write(&config_path, toml::to_string(&edited_config)?)?;
        let changed = enterprise_rpc(
            &client,
            "mcpServerStatus/list",
            json!({"threadId": connected_id}),
        )
        .await?;
        assert_eq!(
            inventory_request_count(&server.received_requests().await.context("requests")?),
            inventory_requests_before,
            "bulk status must not query the previous enterprise registration",
        );
        let changed = changed["data"].as_array().context("changed statuses")?;
        let enterprise = changed
            .iter()
            .find(|status| status["name"] == "enterprise")
            .context("changed enterprise status")?;
        assert_eq!(
            enterprise["httpOrigin"],
            url::Url::parse(url)?.origin().ascii_serialization()
        );
        assert_eq!(enterprise["tools"], json!({}));
        assert_eq!(enterprise["resources"], json!([]));
        assert_eq!(enterprise["resourceTemplates"], json!([]));
        for field in ["serverInfo", "serverCapabilities", "toolListMeta"] {
            assert!(enterprise[field].is_null(), "stale {field}: {enterprise}");
        }
        assert_eq!(
            changed.iter().find(|status| status["name"] == "ordinary"),
            statuses.iter().find(|status| status["name"] == "ordinary"),
        );
    }
    std::fs::write(&config_path, &saved_config)?;
    let handshakes = server.received_requests().await.context("MCP handshakes")?;
    let enterprise_discovery = handshakes
        .iter()
        .find_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            (request.headers.get("authorization")?.to_str().ok()? == "Bearer enterprise-resource-1"
                && body["method"] == "server/discover")
                .then_some(body)
        })
        .context("authenticated EMA connection negotiates modern MCP")?;
    let metadata = &enterprise_discovery["params"]["_meta"];
    assert_eq!(
        metadata["io.modelcontextprotocol/protocolVersion"],
        "2026-07-28"
    );
    assert_eq!(
        metadata["io.modelcontextprotocol/clientCapabilities"]["extensions"]
            [codex_protocol::mcp::ENTERPRISE_MANAGED_AUTHORIZATION_EXTENSION_ID],
        json!({})
    );
    let ordinary_initialize = handshakes
        .iter()
        .find_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            (request.headers.get("authorization")?.to_str().ok()? == "Bearer ordinary-access"
                && body["method"] == "initialize")
                .then_some(body)
        })
        .context("ordinary MCP keeps its legacy handshake")?;
    assert_eq!(
        ordinary_initialize["params"]["protocolVersion"],
        "2025-06-18"
    );
    assert!(
        ordinary_initialize["params"]["capabilities"]["extensions"]
            .get(codex_protocol::mcp::ENTERPRISE_MANAGED_AUTHORIZATION_EXTENSION_ID)
            .is_none()
    );
    let authorization = server
        .received_requests()
        .await
        .context("requests")?
        .into_iter()
        .filter(|request| {
            serde_json::from_slice::<Value>(&request.body)
                .ok()
                .is_some_and(|body| body["method"] == "tools/call")
        })
        .map(|request| {
            request.headers["authorization"]
                .to_str()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        authorization,
        [
            "Bearer enterprise-resource-1",
            "Bearer enterprise-resource-2",
            "Bearer ordinary-access",
            "Bearer enterprise-resource-3"
        ]
    );
    assert_eq!(
        resource_exchanges.load(std::sync::atomic::Ordering::SeqCst),
        3
    );

    // Login and both renewals used the dedicated IdP registration, even though
    // the primary account belongs to a different client at the same issuer.
    assert!(
        idp_server
            .received_requests()
            .await
            .context("IdP requests")?
            .iter()
            .filter(|request| request.url.path() == "/idp/token")
            .all(|request| url::form_urlencoded::parse(&request.body)
                .any(|(name, value)| name == "client_secret" && value == "idp-secret"))
    );

    // Replacing the enterprise grant fences old threads, including cached bearers.
    let relogin = enterprise_rpc(
        &client,
        "mcpServer/oauth/login",
        json!({"name":"enterprise","threadId":connected_id}),
    )
    .await?;
    browser
        .get(enterprise_callback(&relogin, &issuer, callback_port)?)
        .header(http::header::CONNECTION, "close")
        .send()
        .await?;
    assert_eq!(
        enterprise_completion(&mut client).await?,
        json!({
            "name":"enterprise","threadId":connected_id,"loginId":relogin["loginId"],"success":true,
        })
    );
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", call("enterprise")).await?["isError"],
        true
    );
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", call("ordinary")).await?,
        ordinary_result
    );

    // A same-account login replacement refuses EMA without adopting the new owner.
    let auth_path = home.path().join("auth.json");
    let saved_auth = std::fs::read_to_string(&auth_path)?;
    let mut replaced_auth: Value = serde_json::from_str(&saved_auth)?;
    replaced_auth["tokens"]["login_owner_id"] = json!("replacement-account-login");
    std::fs::write(&auth_path, serde_json::to_string(&replaced_auth)?)?;
    let refused = enterprise_rpc(&client, "thread/start", json!({})).await?;
    let refused_call = |server: &str| {
        json!({
            "threadId": refused["thread"]["id"], "server": server,
            "tool": "calendar_list_events", "arguments": {"query": "hello"},
        })
    };
    assert!(
        enterprise_rpc(&client, "mcpServer/tool/call", refused_call("enterprise"))
            .await
            .is_err()
    );
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", refused_call("ordinary")).await?,
        ordinary_result
    );
    assert_eq!(
        resource_exchanges.load(std::sync::atomic::Ordering::SeqCst),
        3
    );
    std::fs::write(&auth_path, saved_auth)?;

    // An EMA account read stays inside that server's startup, leaving ordinary MCP usable.
    let (arrived, read_started) = tokio::sync::oneshot::channel();
    let (release, read_release) = std::sync::mpsc::channel();
    *account_read.lock().unwrap() = Some((arrived, read_release));
    let startup = async {
        let thread = enterprise_rpc(&client, "thread/start", json!({})).await?;
        read_started.await?;
        let result = enterprise_rpc(
            &client,
            "mcpServer/tool/call",
            json!({
                "threadId": thread["thread"]["id"], "server": "ordinary",
                "tool": "calendar_list_events", "arguments": {"query":"hello"},
            }),
        )
        .await?;
        enterprise_rpc(
            &client,
            "thread/archive",
            json!({"threadId": thread["thread"]["id"]}),
        )
        .await?;
        anyhow::Ok(result)
    };
    let result = timeout(DEFAULT_READ_TIMEOUT, startup).await;
    // Release even on failure so the blocking worker cannot hang child shutdown.
    let _ = release.send(());
    assert_eq!(
        result.context("account read blocked ordinary MCP")??,
        ordinary_result
    );

    // Rejected and missing grants need sign-in; administrator configuration errors do not.
    let reauthentication =
        Some(codex_app_server_protocol::McpServerStartupFailureReason::ReauthenticationRequired);
    for (error, reason) in [
        (Some("invalid_client"), None),
        (Some("insufficient_user_authentication"), reauthentication),
        (Some("invalid_grant"), reauthentication),
        (None, reauthentication),
    ] {
        if let Some(error) = error {
            Mock::given(method("POST"))
                .and(path("/idp/token"))
                .and(wiremock::matchers::body_string_contains(
                    "grant-type%3Atoken-exchange",
                ))
                .respond_with(ResponseTemplate::new(400).set_body_json(json!({"error": error})))
                .with_priority(1)
                .up_to_n_times(1)
                .expect(1)
                .mount(&idp_server)
                .await;
        }
        let rejected = enterprise_rpc(&client, "thread/start", json!({})).await?;
        let rejected_id = rejected["thread"]["id"].as_str().context("thread ID")?;
        let failure = timeout(DEFAULT_READ_TIMEOUT, async {
            loop {
                if let InProcessServerEvent::ServerNotification(notification) =
                    client.next_event().await.context("startup notification")?
                    && let ServerNotification::McpServerStatusUpdated(notification) = *notification
                    && notification.thread_id.as_deref() == Some(rejected_id)
                    && notification.name == "enterprise"
                    && notification.status
                        == codex_app_server_protocol::McpServerStartupState::Failed
                {
                    return anyhow::Ok(notification);
                }
            }
        })
        .await??;
        assert_eq!(failure.failure_reason, reason, "{error:?}: {failure:?}");
        if reason.is_some() {
            let status = enterprise_rpc(
                &client,
                "mcpServerStatus/list",
                json!({"threadId": rejected_id, "serverName": "enterprise"}),
            )
            .await?;
            assert_eq!(status["data"][0]["authStatus"], "notLoggedIn");
        }
    }

    let recovery = enterprise_rpc(
        &client,
        "mcpServer/oauth/login",
        json!({"name":"enterprise","threadId":thread_id}),
    )
    .await?;
    browser
        .get(enterprise_callback(&recovery, &issuer, callback_port)?)
        .header(http::header::CONNECTION, "close")
        .send()
        .await?;
    assert_eq!(
        enterprise_completion(&mut client).await?,
        json!({"name":"enterprise","threadId":thread_id,"loginId":recovery["loginId"],"success":true})
    );

    // Switch the IdP registration to public for the account-grant recovery checks.
    // The earlier dedicated login, startup, and renewal used separate IdP/AS secrets.
    let mut config: toml::Value = toml::from_str(&saved_config)?;
    let idp_config = config["mcp_enterprise_managed_auth"]["idp"]
        .as_table_mut()
        .context("IdP config")?;
    idp_config.remove("client_secret");
    idp_config.remove("token_endpoint_auth_method");
    std::fs::write(&config_path, toml::to_string(&config)?)?;

    // An eligible account login uses its own OAuth client, without a second sign-in.
    let auth_path = home.path().join("auth.json");
    let mut account_auth: Value = serde_json::from_slice(&std::fs::read(&auth_path)?)?;
    account_auth["tokens"]["oauth_provenance"] =
        json!({"issuer": issuer, "client_id": "account-client"});
    account_auth["tokens"]["login_owner_id"] = json!("completed-account-login");
    account_auth["tokens"]["refresh_token"] = json!("account-refresh");
    account_auth["tokens"]["id_token"] = json!(format!("{}.{}.c2lnbmF0dXJl",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(json!({
            "iss": issuer, "sub": "enterprise-user", "aud": "account-client", "exp": now + 3600,
            "https://api.openai.com/auth": {"chatgpt_user_id": "enterprise-user", "chatgpt_account_id": WORKSPACE_ID_REFRESHED, "chatgpt_plan_type": "business"}
        }).to_string())));
    std::fs::write(&auth_path, serde_json::to_vec(&account_auth)?)?;
    // Startup does not adopt a different login. Explicitly reload the account
    // before starting a thread under the new completed login.
    enterprise_rpc(&client, "account/read", json!({"refreshToken":true})).await?;
    let (discovery_arrived, discovery_wait) = tokio::sync::oneshot::channel();
    let (release_discovery, discovery_release) = std::sync::mpsc::channel();
    *pending_discovery.lock().unwrap() = Some((discovery_arrived, discovery_release));
    let account_thread = enterprise_rpc(&client, "thread/start", json!({})).await?;
    let account_id = account_thread["thread"]["id"]
        .as_str()
        .context("account thread ID")?;
    let account_call = json!({"threadId":account_id,"server":"enterprise","tool":"calendar_list_events","arguments":{"query":"hello"}});
    let mut initial_call = Box::pin(enterprise_rpc(
        &client,
        "mcpServer/tool/call",
        account_call.clone(),
    ));
    tokio::select! {
        arrived = discovery_wait => arrived?,
        result = &mut initial_call => anyhow::bail!(
            "fresh EMA startup failed before IdP discovery: {result:?}"
        ),
    }
    // Account refresh can publish a newer grant while IdP metadata is still pending.
    // EMA must acquire that newer credential before sending anything to the IdP.
    account_auth["tokens"]["access_token"] = json!("account-after-discovery");
    account_auth["tokens"]["refresh_token"] = json!("discovered-account-refresh");
    std::fs::write(&auth_path, serde_json::to_vec(&account_auth)?)?;
    let refreshed = enterprise_rpc(&client, "account/read", json!({"refreshToken":true})).await;
    account_auth["tokens"]["access_token"] = json!("account-after-resource-exchange");
    account_auth["tokens"]["id_token"] = json!(format!("{}.{}.c2lnbmF0dXJl",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(json!({
            "iss": issuer, "sub": "enterprise-user", "aud": "account-client", "iat": now, "exp": now + 7200,
            "https://api.openai.com/auth": {"chatgpt_user_id": "enterprise-user", "chatgpt_account_id": WORKSPACE_ID_REFRESHED, "chatgpt_plan_type": "business"}
        }).to_string())));
    pending_account_refresh
        .lock()
        .unwrap()
        .push_back((auth_path.clone(), serde_json::to_vec(&account_auth)?));
    release_discovery.send(()).expect("release IdP discovery");
    refreshed.context("IdP discovery must not block account refresh")?;
    assert_eq!(initial_call.await?, ordinary_result);
    assert_eq!(
        resource_exchanges.load(std::sync::atomic::Ordering::SeqCst),
        4
    );

    // A no-op refresh keeps the established connection and its cached bearer.
    enterprise_rpc(&client, "config/mcpServer/reload", json!({})).await?;
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", account_call.clone()).await?,
        ordinary_result
    );
    assert_eq!(
        resource_exchanges.load(std::sync::atomic::Ordering::SeqCst),
        4
    );

    let request_count = server
        .received_requests()
        .await
        .context("MCP requests")?
        .len();
    account_auth["tokens"]["refresh_token"] = json!("rotated-account-refresh");
    std::fs::write(&auth_path, serde_json::to_vec(&account_auth)?)?;
    Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .and(header("authorization", "Bearer enterprise-resource-4"))
        .and(body_partial_json(json!({"method":"tools/call"})))
        .respond_with(ResponseTemplate::new(401))
        .with_priority(1)
        .expect(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", account_call.clone()).await?["isError"],
        true
    );
    assert_eq!(
        resource_exchanges.load(std::sync::atomic::Ordering::SeqCst),
        4
    );
    // Rotate during both resource exchanges; discard stale bearers without revoking the owner.
    for refresh_token in ["exchanged-account-refresh", "retried-account-refresh"] {
        account_auth["tokens"]["refresh_token"] = json!(refresh_token);
        pending_account_refresh
            .lock()
            .unwrap()
            .push_back((auth_path.clone(), serde_json::to_vec(&account_auth)?));
    }
    assert!(
        enterprise_rpc(&client, "mcpServer/tool/call", account_call.clone())
            .await
            .is_err()
    );
    assert_eq!(
        resource_exchanges.load(std::sync::atomic::Ordering::SeqCst),
        6
    );
    // Retry exhaustion fails only this call. The next explicit call can still renew.
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", account_call.clone()).await?,
        ordinary_result
    );
    let account_subjects = idp_server
        .received_requests()
        .await
        .context("IdP requests")?
        .iter()
        .filter_map(|request| {
            url::form_urlencoded::parse(&request.body)
                .find(|(name, _)| name == "subject_token")
                .map(|(_, token)| token.into_owned())
        })
        .filter(|token| token != "enterprise-refresh")
        .collect::<Vec<_>>();
    assert_eq!(
        account_subjects,
        [
            "discovered-account-refresh",
            "rotated-account-refresh",
            "exchanged-account-refresh",
            "retried-account-refresh"
        ]
    );
    let renewed_calls = server
        .received_requests()
        .await
        .context("MCP requests")?
        .into_iter()
        .skip(request_count)
        .filter_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            (body["method"] == "tools/call").then(|| {
                request.headers["authorization"]
                    .to_str()
                    .expect("valid bearer header")
                    .to_owned()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        renewed_calls,
        [
            "Bearer enterprise-resource-4",
            "Bearer enterprise-resource-7"
        ],
        "failed calls are not replayed and stale exchange bearers never reach MCP"
    );

    // One-shot responses avoid scoped-mock IDs being reordered by wiremock priorities.
    // Resource-AS step-up is an administrator error, not an account sign-in failure.
    Mock::given(method("POST"))
        .and(path("/as/token"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"error":"insufficient_user_authentication"})),
        )
        .with_priority(1)
        .expect(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let rejected_thread = enterprise_rpc(&client, "thread/start", json!({})).await?;
    let rejected_id = &rejected_thread["thread"]["id"];
    let status = enterprise_rpc(
        &client,
        "mcpServerStatus/list",
        json!({"threadId":rejected_id,"serverName":"enterprise"}),
    )
    .await?;
    assert_eq!(status["data"][0]["authStatus"], "bearerToken");
    let error = enterprise_rpc(&client, "mcpServer/tool/call", json!({"threadId":rejected_id,"server":"enterprise","tool":"calendar_list_events","arguments":{}})).await.unwrap_err();
    assert!(
        error.to_string().contains("contact your administrator"),
        "{error}"
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(&auth_path)?)?,
        account_auth
    );

    // IdP step-up preserves the account grant and affects this registration across threads.
    Mock::given(method("POST"))
        .and(path("/idp/token"))
        .and(wiremock::matchers::body_string_contains(
            "subject_token=retried-account-refresh",
        ))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"error":"insufficient_user_authentication"})),
        )
        .with_priority(1)
        .expect(1)
        .up_to_n_times(1)
        .mount(&idp_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .and(header("authorization", "Bearer enterprise-resource-7"))
        .and(body_partial_json(json!({"method":"tools/call"})))
        .respond_with(ResponseTemplate::new(401))
        .with_priority(1)
        .expect(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", account_call.clone()).await?["isError"],
        true
    );
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", account_call).await?["isError"],
        true
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(&auth_path)?)?,
        account_auth
    );
    // Access-token rotation must not forget the unchanged grant's sign-in requirement.
    account_auth["tokens"]["access_token"] = json!("account-after-step-up");
    std::fs::write(&auth_path, serde_json::to_vec(&account_auth)?)?;
    enterprise_rpc(&client, "account/read", json!({"refreshToken":true})).await?;
    let recovered = enterprise_rpc(&client, "thread/start", json!({})).await?;
    let recovered_call = json!({"threadId":recovered["thread"]["id"],"server":"enterprise","tool":"calendar_list_events","arguments":{"query":"hello"}});
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", recovered_call).await?,
        ordinary_result
    );
    assert_eq!(
        resource_exchanges.load(std::sync::atomic::Ordering::SeqCst),
        8
    );
    let recovered_subject = idp_server
        .received_requests()
        .await
        .context("IdP requests")?
        .iter()
        .filter_map(|request| {
            url::form_urlencoded::parse(&request.body)
                .find(|(name, _)| name == "subject_token")
                .map(|(_, token)| token.into_owned())
        })
        .next_back();
    assert_eq!(recovered_subject.as_deref(), Some("enterprise-refresh"));

    // Both failure stages preserve a dedicated grant. Status must finish startup first.
    for (endpoint, identity_provider) in [("/idp/token", true), ("/as/token", false)] {
        let target = if identity_provider {
            &idp_server
        } else {
            &server
        };
        Mock::given(method("POST"))
            .and(path(endpoint))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(json!({"error":"insufficient_user_authentication"})),
            )
            .with_priority(1)
            .expect(1)
            .up_to_n_times(1)
            .mount(target)
            .await;
        let rejected = enterprise_rpc(&client, "thread/start", json!({})).await?;
        let rejected_id = &rejected["thread"]["id"];
        let status = enterprise_rpc(
            &client,
            "mcpServerStatus/list",
            json!({"threadId":rejected_id,"serverName":"enterprise"}),
        )
        .await?;
        assert_eq!(
            status["data"][0]["authStatus"],
            if identity_provider {
                "notLoggedIn"
            } else {
                "oAuth"
            }
        );
        let error = enterprise_rpc(&client, "mcpServer/tool/call", json!({"threadId":rejected_id,"server":"enterprise","tool":"calendar_list_events","arguments":{}})).await.unwrap_err();
        assert!(
            error.to_string().contains(if identity_provider {
                "Complete enterprise sign-in"
            } else {
                "contact your administrator"
            }),
            "{error}"
        );
    }

    // Use the thread admitted after switching the IdP registration to public.
    let stalled = enterprise_rpc(
        &client,
        "mcpServer/oauth/login",
        json!({"name":"enterprise","threadId":recovered["thread"]["id"]}),
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
    let (exchange_arrived, exchange_wait) = tokio::sync::oneshot::channel();
    let (release_exchange, exchange_release) = std::sync::mpsc::channel();
    *pending_exchange.lock().unwrap() = Some((exchange_arrived, exchange_release));
    let pending = enterprise_rpc(&client, "thread/start", json!({})).await?;
    timeout(DEFAULT_READ_TIMEOUT, exchange_wait).await??;
    let logout = timeout(
        std::time::Duration::from_secs(15),
        enterprise_rpc(&client, "account/logout", json!({})),
    )
    .await;
    // Release the worker even when the regression fails, so child shutdown cannot hang.
    drop(unfinished);
    let _ = release_exchange.send(());
    logout.context("an unfinished callback must not prevent primary logout")??;
    // Observe the original startup's completion before a tool call can refresh it.
    let mut startup_finished = false;
    let mut login_completion = None;
    timeout(DEFAULT_READ_TIMEOUT, async {
        while !startup_finished || login_completion.is_none() {
            if let InProcessServerEvent::ServerNotification(notification) =
                client.next_event().await.context("logout notification")?
            {
                match *notification {
                    ServerNotification::McpServerStatusUpdated(notification)
                        if notification.thread_id.as_deref()
                            == pending["thread"]["id"].as_str()
                            && notification.name == "enterprise"
                            && notification.status != McpServerStartupState::Starting =>
                    {
                        startup_finished = true;
                    }
                    ServerNotification::McpServerOauthLoginCompleted(notification) => {
                        login_completion = Some(serde_json::to_value(notification)?);
                    }
                    _ => {}
                }
            }
        }
        anyhow::Ok(())
    })
    .await??;
    assert_eq!(
        login_completion.context("enterprise login completion")?,
        json!({"name":"enterprise","threadId":recovered["thread"]["id"],"loginId":stalled["loginId"],"success":false,"error":"Enterprise sign-in failed."})
    );
    assert_eq!(
        resource_exchanges.load(std::sync::atomic::Ordering::SeqCst),
        8
    );
    assert!(
        enterprise_rpc(
            &client,
            "mcpServer/tool/call",
            json!({
                "threadId": pending["thread"]["id"], "server": "enterprise",
                "tool": "calendar_list_events", "arguments": {},
            })
        )
        .await
        .is_err()
    );
    assert!(!home.path().join("auth.json").exists());
    assert!(
        enterprise_rpc(&client, "mcpServer/tool/call", call("enterprise"))
            .await
            .is_err()
    );
    assert_eq!(
        enterprise_rpc(&client, "mcpServer/tool/call", call("ordinary")).await?,
        ordinary_result
    );
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

type AccountReadGate = Arc<
    Mutex<
        Option<(
            tokio::sync::oneshot::Sender<()>,
            std::sync::mpsc::Receiver<()>,
        )>,
    >,
>;

#[derive(Default)]
struct TestKeyring {
    credentials: Mutex<HashMap<(String, String), Arc<MockCredential>>>,
    account_read: AccountReadGate,
}

struct TestCredential(Arc<MockCredential>, Option<AccountReadGate>);

impl CredentialApi for TestCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
        self.0.set_secret(secret)
    }
    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        let pending = self
            .1
            .as_ref()
            .and_then(|gate| gate.lock().expect("account read gate").take());
        if let Some((arrived, release)) = pending {
            let _ = arrived.send(());
            let _ = release.recv();
        }
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
            .credentials
            .lock()
            .expect("mock keyring")
            .entry((service.into(), user.into()))
            .or_default()
            .clone();
        Ok(Box::new(TestCredential(
            credential,
            (service == "Codex Auth").then(|| Arc::clone(&self.account_read)),
        )))
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
                .login_owner_id("original-account-login")
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
            client_secret: None,
            token_endpoint_auth_method: None,
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
