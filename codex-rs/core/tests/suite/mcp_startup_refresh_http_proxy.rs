use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_exec_server::CreateDirectoryOptions;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
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
use tokio::process::Command;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const PROXY_TEST_SUBPROCESS_ENV_VAR: &str = "CODEX_MCP_HTTP_PROXY_TEST_SUBPROCESS";
const TEST_NAME: &str = "suite::mcp_startup_refresh_http_proxy::local_mcp_startup_and_refresh_use_configured_http_client";
const SKILL_TEST_NAME: &str =
    "suite::mcp_startup_refresh_http_proxy::skill_mcp_dependency_oauth_uses_configured_http_client";
const SERVER_NAME: &str = "proxied_mcp";
const SERVER_URL: &str = "http://mcp-proxy.invalid/api/codex/ps/mcp";

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
                    },
                    environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
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

        let mut command = Command::new(std::env::current_exe()?);
        command.arg("--exact").arg(SKILL_TEST_NAME);
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
            .expect("mock proxy should record MCP OAuth requests");
        assert!(
            output.status.success(),
            "subprocess test `{SKILL_TEST_NAME}` failed\nstdout:\n{}\nstderr:\n{}\nproxy requests:\n{requests:#?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(
            requests.iter().any(|request| {
                request.method == "POST" && request.url.path() == "/oauth/register"
            }),
            "skill dependency OAuth registration should use the configured proxy: {requests:#?}"
        );
        return Ok(());
    }

    let responses_server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::SkillMcpDependencyInstall)
                .expect("test config should allow skill MCP dependency installation");
            if cfg!(target_os = "linux") {
                config
                    .features
                    .enable(Feature::RespectSystemProxy)
                    .expect("test config should allow the system proxy feature");
                config.respect_system_proxy = true;
            }
        })
        .with_workspace_setup(|cwd, fs| async move {
            let skill_dir = cwd.join(".agents/skills/proxy-skill");
            let agents_dir = skill_dir.join("agents");
            fs.create_directory(
                &PathUri::from_host_native_path(&agents_dir)?,
                CreateDirectoryOptions { recursive: true },
                /*sandbox*/ None,
            )
            .await?;
            fs.write_file(
                &PathUri::from_host_native_path(skill_dir.join("SKILL.md"))?,
                b"---\nname: proxy-skill\ndescription: Uses a proxied MCP server.\n---\n".to_vec(),
                /*sandbox*/ None,
            )
            .await?;
            let metadata = format!(
                "dependencies:\n  tools:\n    - type: mcp\n      value: {SERVER_NAME}\n      transport: streamable_http\n      url: {SERVER_URL}\n"
            );
            fs.write_file(
                &PathUri::from_host_native_path(agents_dir.join("openai.yaml"))?,
                metadata.into_bytes(),
                /*sandbox*/ None,
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
    fixture
        .codex
        .submit(Op::UserInput {
            items: vec![
                UserInput::Text {
                    text: "please use $proxy-skill".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "proxy-skill".to_string(),
                    path: skill_path.to_path_buf(),
                },
            ],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(fixture.config.cwd.clone())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            },
        })
        .await?;
    core_test_support::wait_for_event(fixture.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let servers = codex_config::load_global_mcp_servers(&fixture.config.codex_home).await?;
    assert!(servers.contains_key(SERVER_NAME));
    Ok(())
}
