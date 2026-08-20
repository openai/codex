use codex_core::EnvironmentConfig;
use codex_core::EnvironmentMcpPolicy;
use codex_core::TurnInputRequest;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerConfig;
use codex_config::McpServerOAuthConfig;
use codex_config::McpServerTransportConfig;
use codex_exec_server::CreateDirectoryOptions;
use codex_features::Feature;
use codex_protocol::mcp_policy::McpServerIdentity;
use codex_protocol::mcp_policy::McpServerRequirement;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::process::Command;
use url::Url;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const PROXY_TEST_SUBPROCESS_ENV_VAR: &str = "CODEX_MCP_HTTP_PROXY_TEST_SUBPROCESS";
const SKILL_CALLBACK_PORT_ENV_VAR: &str = "CODEX_MCP_SKILL_CALLBACK_PORT";
const SKILL_GLOBAL_CALLBACK_PORT_ENV_VAR: &str = "CODEX_MCP_SKILL_GLOBAL_CALLBACK_PORT";
const TEST_NAME: &str = "suite::mcp_startup_refresh_http_proxy::local_mcp_startup_and_refresh_use_configured_http_client";
const SKILL_TEST_NAME: &str =
    "suite::mcp_startup_refresh_http_proxy::skill_mcp_dependency_oauth_uses_configured_http_client";
const SERVER_NAME: &str = "proxied_mcp";
const SERVER_URL: &str = "http://mcp-proxy.invalid/api/codex/ps/mcp";
const DENIED_SERVER_NAME: &str = "denied_mcp";
const DENIED_SERVER_URL: &str = "http://mcp-proxy.invalid/denied-mcp";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_mcp_startup_and_refresh_use_configured_http_client() -> Result<()> {
    skip_if_no_network!(Ok(()));

    if std::env::var_os(PROXY_TEST_SUBPROCESS_ENV_VAR).is_none() {
        let proxy = MockServer::start().await;
        let _apps_server = AppsTestServer::mount(&proxy).await?;
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("--exact").arg(TEST_NAME);
        for &key in codex_network_proxy::PROXY_ENV_KEYS {
            command.env_remove(key);
        }
        command
            .env(PROXY_TEST_SUBPROCESS_ENV_VAR, "1")
            .env("HTTP_PROXY", proxy.uri())
            .env("http_proxy", proxy.uri())
            .env("NO_PROXY", codex_network_proxy::DEFAULT_NO_PROXY_VALUE)
            .env("no_proxy", codex_network_proxy::DEFAULT_NO_PROXY_VALUE);

        let output = command.output().await?;
        let requests = proxy
            .received_requests()
            .await
            .expect("mock proxy should record MCP requests");
        assert!(
            output.status.success(),
            "subprocess test `{TEST_NAME}` failed\nstdout:\n{}\nstderr:\n{}\nproxy requests:\n{requests:#?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let initialize_authorizations = requests
            .iter()
            .filter_map(|request| {
                let body = serde_json::from_slice::<Value>(&request.body).ok()?;
                (body.get("method").and_then(Value::as_str) == Some("initialize"))
                    .then(|| {
                        request
                            .headers
                            .get("authorization")
                            .and_then(|header| header.to_str().ok())
                            .map(str::to_string)
                    })
                    .flatten()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            initialize_authorizations,
            vec!["Bearer initial", "Bearer refreshed"]
        );
        return Ok(());
    }

    let responses_server = responses::start_mock_server().await;
    let fixture = test_codex()
        .with_config(|config| {
            if cfg!(target_os = "linux") {
                config
                    .features
                    .enable(Feature::RespectSystemProxy)
                    .expect("test config should allow the system proxy feature");
                config.respect_system_proxy = true;
            }
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                SERVER_NAME.to_string(),
                McpServerConfig {
                    auth: Default::default(),
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: SERVER_URL.to_string(),
                        bearer_token_env_var: None,
                        http_headers: Some(HashMap::from([(
                            "Authorization".to_string(),
                            "Bearer initial".to_string(),
                        )])),
                        env_http_headers: None,
                        http_headers_helper: None,
                    },
                    environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    omit_tools_from: None,
                    disabled_reason: None,
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: None,
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test MCP servers should accept any configuration");
        })
        .build_with_auto_env(&responses_server)
        .await?;
    wait_for_mcp_server(&fixture.codex, SERVER_NAME).await?;

    let mut refreshed_config = fixture.config.clone();
    let mut servers = refreshed_config.mcp_servers.get().clone();
    let server = servers
        .get_mut(SERVER_NAME)
        .expect("configured MCP server should exist");
    let McpServerTransportConfig::StreamableHttp { http_headers, .. } = &mut server.transport
    else {
        unreachable!("test MCP server should use streamable HTTP");
    };
    *http_headers = Some(HashMap::from([(
        "Authorization".to_string(),
        "Bearer refreshed".to_string(),
    )]));
    refreshed_config
        .mcp_servers
        .set(servers)
        .expect("test MCP servers should accept the refreshed configuration");
    fixture.codex.refresh_runtime_config(refreshed_config).await;
    let result = fixture
        .codex
        .call_mcp_tool(
            SERVER_NAME,
            "calendar_create_event",
            Some(json!({
                "title": "Proxy refresh",
                "starts_at": "2026-07-23T12:00:00Z",
            })),
            /*meta*/ None,
        )
        .await?;
    assert_eq!(result.is_error, Some(false));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_mcp_dependency_oauth_uses_configured_http_client() -> Result<()> {
    skip_if_target_windows!(Ok(()), "requires native cross-OS skill paths");
    skip_if_no_network!(Ok(()));

    if std::env::var_os(PROXY_TEST_SUBPROCESS_ENV_VAR).is_none() {
        let proxy = MockServer::start().await;
        let _apps_server = AppsTestServer::mount(&proxy).await?;
        let challenge = "Bearer resource_metadata=\"http://mcp-proxy.invalid/oauth-resource\"";
        Mock::given(method("GET"))
            .and(path("/api/codex/ps/mcp"))
            .respond_with(ResponseTemplate::new(401).insert_header("WWW-Authenticate", challenge))
            .mount(&proxy)
            .await;
        Mock::given(method("GET"))
            .and(path("/oauth-resource"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "resource": SERVER_URL,
                "authorization_servers": ["http://mcp-proxy.invalid"],
            })))
            .mount(&proxy)
            .await;
        Mock::given(method("GET"))
            .and(path("/.well-known/oauth-authorization-server"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "authorization_endpoint": "http://mcp-proxy.invalid/oauth/authorize",
                "token_endpoint": "http://mcp-proxy.invalid/oauth/token",
                "registration_endpoint": "http://mcp-proxy.invalid/oauth/register",
                "response_types_supported": ["code"],
                "code_challenge_methods_supported": ["S256"],
            })))
            .mount(&proxy)
            .await;
        Mock::given(method("POST"))
            .and(path("/oauth/register"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&proxy)
            .await;

        let skill_callback_listener = TcpListener::bind("127.0.0.1:0").await?;
        let skill_callback_port = skill_callback_listener.local_addr()?.port();
        let global_callback_listener = TcpListener::bind("127.0.0.1:0").await?;
        let global_callback_port = global_callback_listener.local_addr()?.port();
        drop(skill_callback_listener);

        let mut command = Command::new(std::env::current_exe()?);
        command.arg("--exact").arg(SKILL_TEST_NAME);
        for &key in codex_network_proxy::PROXY_ENV_KEYS {
            command.env_remove(key);
        }
        command
            .env(PROXY_TEST_SUBPROCESS_ENV_VAR, "1")
            .env(SKILL_CALLBACK_PORT_ENV_VAR, skill_callback_port.to_string())
            .env(
                SKILL_GLOBAL_CALLBACK_PORT_ENV_VAR,
                global_callback_port.to_string(),
            )
            .env("HTTP_PROXY", proxy.uri())
            .env("http_proxy", proxy.uri())
            .env("NO_PROXY", codex_network_proxy::DEFAULT_NO_PROXY_VALUE)
            .env("no_proxy", codex_network_proxy::DEFAULT_NO_PROXY_VALUE);

        let output = command.output().await?;
        let requests = proxy
            .received_requests()
            .await
            .expect("mock proxy should record MCP OAuth requests");
        assert!(
            output.status.success(),
            "subprocess test `{SKILL_TEST_NAME}` failed\nstdout:\n{}\nstderr:\n{}\nproxy requests:\n{requests:#?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let registration_request = requests
            .iter()
            .find(|request| {
                request.method == "POST" && request.url.path() == "/oauth/register"
            })
            .unwrap_or_else(|| {
                panic!(
                    "skill dependency OAuth registration should use the configured proxy: {requests:#?}"
                )
            });
        let registration: Value = serde_json::from_slice(&registration_request.body)?;
        let redirect_uri = Url::parse(
            registration["redirect_uris"][0]
                .as_str()
                .expect("OAuth client registration redirect URI"),
        )?;
        assert_eq!(redirect_uri.port(), Some(skill_callback_port));
        assert!(
            requests
                .iter()
                .all(|request| request.url.path() != "/denied-mcp"),
            "denied skill dependency must not perform OAuth discovery: {requests:#?}"
        );
        return Ok(());
    }

    let skill_callback_port = std::env::var(SKILL_CALLBACK_PORT_ENV_VAR)?.parse::<u16>()?;
    let global_callback_port = std::env::var(SKILL_GLOBAL_CALLBACK_PORT_ENV_VAR)?.parse::<u16>()?;
    let responses_server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_config(move |config| {
            config
                .features
                .enable(Feature::SkillMcpDependencyInstall)
                .expect("test config should allow skill MCP dependency installation");
            config.mcp_oauth_callback_port = Some(global_callback_port);
            if cfg!(target_os = "linux") {
                config
                    .features
                    .enable(Feature::RespectSystemProxy)
                    .expect("test config should allow the system proxy feature");
                config.respect_system_proxy = true;
            }
        })
        .with_workspace_setup(move |cwd, fs| async move {
            let skill_dir = cwd.join(".agents/skills/proxy-skill");
            let agents_dir = skill_dir.join("agents");
            fs.create_directory(
                &PathUri::from_host_native_path(&agents_dir)?,
                CreateDirectoryOptions { recursive: true, follow_symlinks: true },
                /*sandbox*/ None,
            )
            .await?;
            fs.write_file(
                &PathUri::from_host_native_path(skill_dir.join("SKILL.md"))?,
                b"---\nname: proxy-skill\ndescription: Uses a proxied MCP server.\n---\n".to_vec(),
                Default::default(), /*sandbox*/ None,
            )
            .await?;
            let metadata = format!(
                "dependencies:\n  tools:\n    - type: mcp\n      value: {SERVER_NAME}\n      transport: streamable_http\n      url: {SERVER_URL}\n      oauth:\n        callbackPort: {skill_callback_port}\n    - type: mcp\n      value: {DENIED_SERVER_NAME}\n      transport: streamable_http\n      url: {DENIED_SERVER_URL}\n"
            );
            fs.write_file(
                &PathUri::from_host_native_path(agents_dir.join("openai.yaml"))?,
                metadata.into_bytes(),
                Default::default(), /*sandbox*/ None,
            )
            .await?;
            Ok(())
        });
    let fixture = builder.build_with_auto_env(&responses_server).await?;
    responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let skill_path = fixture
        .config
        .cwd
        .join(".agents/skills/proxy-skill/SKILL.md")
        .canonicalize()
        .unwrap_or_else(|_| {
            fixture
                .config
                .cwd
                .join(".agents/skills/proxy-skill/SKILL.md")
        });
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, fixture.config.cwd.as_path());
    let mut environments = local_selections(fixture.config.cwd.clone());
    environments.environments[0].config = EnvironmentConfigState::Ready(EnvironmentConfig {
        allow_login_shell: fixture.config.permissions.allow_login_shell,
        permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::Disabled),
        shell_environment_policy: Default::default(),
        exec_policy: None,
        mcp_policy: Some(EnvironmentMcpPolicy {
            servers: Some(BTreeMap::from([(
                SERVER_NAME.to_string(),
                McpServerRequirement::Identity {
                    identity: McpServerIdentity::Url {
                        url: SERVER_URL.to_string(),
                    },
                },
            )])),
            plugins: None,
        }),
        network_policy: None,
        selected_capability_roots: Vec::new(),
    });
    fixture
        .codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![
                UserInput::Text {
                    text: "please use $proxy-skill".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "proxy-skill".to_string(),
                    path: skill_path.to_path_buf(),
                },
            ])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(environments),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            }),
        )
        .await?;
    core_test_support::wait_for_event(fixture.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let servers = codex_config::load_global_mcp_servers(&fixture.config.codex_home).await?;
    assert_eq!(
        servers
            .get(SERVER_NAME)
            .and_then(|server| server.oauth.as_ref()),
        Some(&McpServerOAuthConfig {
            client_id: None,
            callback_port: Some(skill_callback_port),
        })
    );
    assert!(!servers.contains_key(DENIED_SERVER_NAME));
    Ok(())
}
