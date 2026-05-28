#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use std::sync::Arc;
use std::time::Duration;

async fn write_repo_skill(
    cwd: AbsolutePathBuf,
    fs: Arc<dyn ExecutorFileSystem>,
    name: &str,
    description: &str,
    body: &str,
) -> Result<()> {
    let skill_dir = cwd.join(".agents").join("skills").join(name);
    fs.create_directory(
        &skill_dir,
        CreateDirectoryOptions { recursive: true },
        /*sandbox*/ None,
    )
    .await?;
    let contents = format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n");
    let path = skill_dir.join("SKILL.md");
    fs.write_file(&path, contents.into_bytes(), /*sandbox*/ None)
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_includes_skill_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let skill_body = "skill body";
    let mut builder = test_codex().with_workspace_setup(move |cwd, fs| async move {
        write_repo_skill(cwd, fs, "demo", "demo skill", skill_body).await
    });
    let test = builder.build_with_remote_env(&server).await?;

    let skill_path = test
        .config
        .cwd
        .join(".agents/skills/demo/SKILL.md")
        .canonicalize()
        .unwrap_or_else(|_| test.config.cwd.join(".agents/skills/demo/SKILL.md"))
        .to_path_buf();

    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![
                UserInput::Text {
                    text: "please use $demo".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "demo".to_string(),
                    path: skill_path.clone(),
                },
            ],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                cwd: Some(test.config.cwd.to_path_buf()),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    core_test_support::wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;

    let request = mock.single_request();
    let user_texts = request.message_input_texts("user");
    let skill_path_str = skill_path.to_string_lossy();
    assert!(
        user_texts.iter().any(|text| {
            text.contains("<skill>\n<name>demo</name>")
                && text.contains("<path>")
                && text.contains(skill_body)
                && text.contains(skill_path_str.as_ref())
        }),
        "expected skill instructions in user input, got {user_texts:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_skill_waits_for_configured_mcp_dependency_startup() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    AppsTestServer::mount_with_connector_name_and_tools_list_delay(
        &server,
        "Dependency",
        Some(Duration::from_secs(/*secs*/ 2)),
    )
    .await?;
    let dependency_url = format!("{}/api/codex/apps", server.uri());
    let skill_dependency_url = dependency_url.clone();
    let mut builder = test_codex()
        .with_workspace_setup(move |cwd, fs| {
            let skill_dependency_url = skill_dependency_url;
            async move {
                write_repo_skill(cwd.clone(), Arc::clone(&fs), "demo", "demo skill", "body")
                    .await?;
                let agents_dir = cwd.join(".agents/skills/demo/agents");
                fs.create_directory(
                    &agents_dir,
                    CreateDirectoryOptions { recursive: true },
                    /*sandbox*/ None,
                )
                .await?;
                let metadata = format!(
                    "dependencies:\n  tools:\n    - type: \"mcp\"\n      value: \"dependency\"\n      transport: \"streamable_http\"\n      url: \"{skill_dependency_url}\"\n"
                );
                fs.write_file(
                    &agents_dir.join("openai.yaml"),
                    metadata.into_bytes(),
                    /*sandbox*/ None,
                )
                .await?;
                Ok(())
            }
        })
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "dependency".to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: dependency_url,
                        bearer_token_env_var: None,
                        http_headers: None,
                        env_http_headers: None,
                    },
                    environment_id: "local".to_string(),
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    disabled_reason: None,
                    startup_timeout_sec: Some(Duration::from_secs(/*secs*/ 10)),
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: None,
                    oauth_resource: None,
                    tools: Default::default(),
                },
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test mcp servers should accept any configuration");
        });
    let test = builder.build_with_remote_env(&server).await?;
    let skill_path = test
        .config
        .cwd
        .join(".agents/skills/demo/SKILL.md")
        .canonicalize()
        .unwrap_or_else(|_| test.config.cwd.join(".agents/skills/demo/SKILL.md"))
        .to_path_buf();
    let mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Skill {
                name: "demo".to_string(),
                path: skill_path,
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    core_test_support::wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, codex_protocol::protocol::EventMsg::TurnComplete(_))
    })
    .await;

    assert!(
        mock.single_request()
            .tool_by_name("mcp__dependency", "calendar_create_event")
            .is_some(),
        "expected selected skill MCP dependency tool on the first turn"
    );

    Ok(())
}
