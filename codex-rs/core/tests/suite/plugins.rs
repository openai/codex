#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use codex_core::config::Config;
use codex_core_plugins::store::PluginStore;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_plugin::PluginId;
use codex_protocol::auth::AuthMode;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::install;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;
use wiremock::MockServer;

const SAMPLE_PLUGIN_CONFIG_NAME: &str = "sample@test";
const SAMPLE_REMOTE_PLUGIN_CONFIG_NAME: &str = "sample@openai-curated-remote";
const SAMPLE_PLUGIN_DISPLAY_NAME: &str = "sample";
const SAMPLE_PLUGIN_DESCRIPTION: &str = "inspect sample data";
const SAMPLE_REMOTE_PLUGIN_ID: &str = "plugins~Plugin_sample";
const SAMPLE_PLUGIN_APP_NAMESPACE: &str = "mcp__codex_apps__google_calendar";
const SAMPLE_PLUGIN_MCP_NAMESPACE: &str = "mcp__sample";
const PLUGIN_APP_SEARCH_CALL_ID: &str = "plugin-app-search";
const PLUGIN_MCP_SEARCH_CALL_ID: &str = "plugin-mcp-search";
const REMOTE_PLUGIN_CONFIG_NAME: &str = "sample@openai-curated-remote";

fn skills_extensions() -> Arc<ExtensionRegistry<Config>> {
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install(&mut extensions, |config: &Config| SkillsExtensionConfig {
        include_instructions: config.include_skill_instructions,
        bundled_skills_enabled: config.bundled_skills_enabled(),
        orchestrator_skills_enabled: config.orchestrator_skills_enabled,
        shadow_selection_enabled: config.features.enabled(Feature::SkillSearch),
    });
    Arc::new(extensions.build())
}

fn sample_plugin_root(home: &TempDir) -> std::path::PathBuf {
    home.path().join("plugins/cache/test/sample/local")
}

fn write_sample_plugin_manifest_and_config(home: &TempDir) -> std::path::PathBuf {
    write_sample_plugin_manifest_and_config_at_root(
        home,
        sample_plugin_root(home),
        SAMPLE_PLUGIN_CONFIG_NAME,
    )
}

fn write_sample_plugin_manifest_and_config_at_root(
    home: &TempDir,
    plugin_root: std::path::PathBuf,
    plugin_config_name: &str,
) -> std::path::PathBuf {
    std::fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create plugin manifest dir");
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(
            r#"{{"name":"{SAMPLE_PLUGIN_DISPLAY_NAME}","description":"{SAMPLE_PLUGIN_DESCRIPTION}"}}"#
        ),
    )
    .expect("write plugin manifest");
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "[features]\nplugins = true\n\n[plugins.\"{plugin_config_name}\"]\nenabled = true\n"
        ),
    )
    .expect("write config");
    plugin_root
}

fn write_remote_plugin_script_and_config(home: &TempDir) -> std::path::PathBuf {
    let plugin_id = PluginId::parse(REMOTE_PLUGIN_CONFIG_NAME).expect("plugin id");
    let store = PluginStore::new(home.path().to_path_buf());
    let plugin_root = store.plugin_root(&plugin_id, "1.2.3");
    let script_path = plugin_root.join("scripts/run.sh");
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))
        .expect("create remote plugin manifest dir");
    std::fs::create_dir_all(script_path.parent().expect("script parent"))
        .expect("create remote plugin scripts dir");
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample","version":"1.2.3"}"#,
    )
    .expect("write remote plugin manifest");
    std::fs::write(&script_path, "echo remote attribution\n").expect("write remote plugin script");
    store
        .write_remote_plugin_id(&plugin_id, "plugins~Plugin_sample")
        .expect("persist remote plugin id");
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "[features]\nplugins = true\n\n[plugins.\"{REMOTE_PLUGIN_CONFIG_NAME}\"]\nenabled = true\n"
        ),
    )
    .expect("write remote plugin config");
    script_path.into_path_buf()
}

fn write_plugin_skill_plugin(home: &TempDir) -> std::path::PathBuf {
    write_sample_plugin_skill(write_sample_plugin_manifest_and_config(home))
}

fn write_remote_plugin_skill_plugin(home: &TempDir) -> std::path::PathBuf {
    let plugin_root = home
        .path()
        .join("plugins/cache/openai-curated-remote/sample/local");
    write_sample_plugin_skill(write_sample_plugin_manifest_and_config_at_root(
        home,
        plugin_root,
        SAMPLE_REMOTE_PLUGIN_CONFIG_NAME,
    ))
}

fn write_sample_plugin_skill(plugin_root: std::path::PathBuf) -> std::path::PathBuf {
    let skill_dir = plugin_root.join("skills/sample-search");
    std::fs::create_dir_all(skill_dir.as_path()).expect("create plugin skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: inspect sample data\n---\n\n# body\n",
    )
    .expect("write plugin skill");
    skill_dir.join("SKILL.md")
}

fn write_plugin_mcp_plugin(home: &TempDir, command: &str) {
    let plugin_root = write_sample_plugin_manifest_and_config(home);
    std::fs::write(
        plugin_root.join(".mcp.json"),
        format!(
            r#"{{
  "mcpServers": {{
    "sample": {{
      "command": "{command}",
      "cwd": ".",
      "startup_timeout_sec": 60.0
    }}
  }}
}}"#
        ),
    )
    .expect("write plugin mcp config");
}

fn block_plugin_mcp_startup(home: &TempDir, command: &str) -> std::path::PathBuf {
    let barrier = home.path().join("allow-plugin-initialize");
    std::fs::write(
        sample_plugin_root(home).join(".mcp.json"),
        serde_json::to_vec(&serde_json::json!({
            "mcpServers": {
                "sample": {
                    "command": command,
                    "cwd": ".",
                    "env": {
                        "MCP_TEST_INITIALIZE_BARRIER_FILE": barrier,
                    },
                    "startup_timeout_sec": 10,
                },
            },
        }))
        .expect("serialize blocked plugin MCP configuration"),
    )
    .expect("write blocked plugin MCP configuration");
    barrier
}

fn write_plugin_app_plugin(home: &TempDir) {
    write_plugin_app_plugin_with_name(home, "sample");
}

fn write_plugin_app_plugin_with_name(home: &TempDir, app_name: &str) {
    let plugin_root = write_sample_plugin_manifest_and_config(home);
    std::fs::write(
        plugin_root.join(".app.json"),
        format!(
            r#"{{
  "apps": {{
    "{app_name}": {{
      "id": "calendar"
    }}
  }}
}}"#
        ),
    )
    .expect("write plugin app config");
}

async fn build_analytics_plugin_test_codex(
    server: &MockServer,
    codex_home: Arc<TempDir>,
) -> Result<TestCodex> {
    let chatgpt_base_url = server.uri();
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5.2")
        .with_config(move |config| {
            config.chatgpt_base_url = chatgpt_base_url;
        });
    Ok(builder
        .build(server)
        .await
        .expect("create new conversation"))
}

async fn build_apps_enabled_plugin_test_codex(
    server: &MockServer,
    codex_home: Arc<TempDir>,
    chatgpt_base_url: String,
) -> Result<TestCodex> {
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Apps)
                .expect("test config should allow feature update");
            config.chatgpt_base_url = chatgpt_base_url;
        });
    Ok(builder
        .build(server)
        .await
        .expect("create new conversation"))
}

async fn mount_plugin_tool_search_turn(server: &MockServer) -> ResponseMock {
    mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_tool_search_call(
                    PLUGIN_APP_SEARCH_CALL_ID,
                    &serde_json::json!({"query": "create calendar event"}),
                ),
                ev_tool_search_call(
                    PLUGIN_MCP_SEARCH_CALL_ID,
                    &serde_json::json!({"query": "echo"}),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await
}

fn assert_plugin_provenance(tool: &serde_json::Value) {
    let description = tool
        .get("description")
        .and_then(serde_json::Value::as_str)
        .expect("plugin tool description should be present");
    assert!(
        description.contains("This tool is part of plugin `sample`."),
        "expected plugin provenance in tool description: {description:?}"
    );
}

fn searched_plugin_tools(
    request: &ResponsesRequest,
) -> (Option<serde_json::Value>, Option<serde_json::Value>) {
    let app_output = request.tool_search_output(PLUGIN_APP_SEARCH_CALL_ID);
    let mcp_output = request.tool_search_output(PLUGIN_MCP_SEARCH_CALL_ID);
    (
        namespace_child_tool(
            &app_output,
            SAMPLE_PLUGIN_APP_NAMESPACE,
            SEARCH_CALENDAR_CREATE_TOOL,
        )
        .cloned(),
        namespace_child_tool(&mcp_output, SAMPLE_PLUGIN_MCP_NAMESPACE, "echo").cloned(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persisted_remote_plugin_command_attribution_flows_through_turn_context() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "remote plugin attribution fixture uses a local Codex home cache"
    );

    let server = start_mock_server().await;
    let codex_home = Arc::new(TempDir::new()?);
    let script_path = write_remote_plugin_script_and_config(codex_home.as_ref());
    let script_path = script_path.to_string_lossy();
    let command = shlex::try_join(["/bin/sh", script_path.as_ref()])?;
    let call_id = "remote-plugin-command";
    let arguments = serde_json::to_string(&serde_json::json!({
        "command": command,
        "login": false,
    }))?;
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "shell_command", &arguments),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5.2");
    let test_codex = builder.build_with_auto_env(&server).await?;
    let codex = Arc::clone(&test_codex.codex);
    let cwd = test_codex.config.cwd.clone();
    let session_model = test_codex.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd.as_path());
    codex
        .submit(Op::UserInput {
            items: vec![codex_protocol::user_input::UserInput::Text {
                text: "run the remote plugin script".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd)),
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

    let begin = wait_for_event_match(&codex, |event| match event {
        EventMsg::ExecCommandBegin(event) if event.call_id == call_id => Some(event.clone()),
        _ => None,
    })
    .await;
    let end = wait_for_event_match(&codex, |event| match event {
        EventMsg::ExecCommandEnd(event) if event.call_id == call_id => Some(event.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    for (plugin_id, script_path) in [
        (begin.plugin_id.as_deref(), begin.script_path.as_deref()),
        (end.plugin_id.as_deref(), end.script_path.as_deref()),
    ] {
        assert_eq!(plugin_id, Some(REMOTE_PLUGIN_CONFIG_NAME));
        assert_eq!(script_path, Some("scripts/run.sh"));
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capability_sections_render_in_developer_message_in_order() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_with_connector_name(&server, "Google Calendar").await?;

    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let codex_home = Arc::new(TempDir::new()?);
    write_plugin_skill_plugin(codex_home.as_ref());
    write_plugin_app_plugin(codex_home.as_ref());
    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_extensions(skills_extensions())
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Apps)
                .expect("test config should allow feature update");
            config.chatgpt_base_url = apps_server.chatgpt_base_url;
        });
    let test_codex = builder.build(&server).await?;
    let codex = Arc::clone(&test_codex.codex);

    codex
        .submit(Op::UserInput {
            items: vec![codex_protocol::user_input::UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = resp_mock.single_request();
    let developer_messages = request.message_input_texts("developer");
    let developer_text = developer_messages.join("\n\n");
    let apps_pos = developer_text
        .find("## Apps")
        .expect("expected apps section in developer message");
    let skills_pos = developer_text
        .find("## Skills")
        .expect("expected skills section in developer message");
    let plugins_pos = developer_text
        .find("## Plugins")
        .expect("expected plugins section in developer message");
    assert!(
        skills_pos < apps_pos && apps_pos < plugins_pos,
        "expected Skills -> Apps -> Plugins order: {developer_messages:?}"
    );
    assert!(
        !developer_text.contains("`sample`: inspect sample data"),
        "did not expect plugin description in developer message: {developer_messages:?}"
    );
    assert!(
        developer_text.contains("skill entries are prefixed with `plugin_name:`"),
        "expected plugin skill naming guidance in developer message: {developer_messages:?}"
    );
    assert!(
        developer_text.contains("sample:sample-search: inspect sample data"),
        "expected namespaced plugin skill summary in developer message: {developer_messages:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_turns_route_curated_plugin_skills_after_auth_switch() -> Result<()> {
    const CHATGPT_CURATED_PLUGIN_SKILL: &str = "chatgpt-plugin:chatgpt-skill";
    const API_CURATED_PLUGIN_SKILL: &str = "api-plugin:api-skill";
    const CURATED_PLUGIN_SKILLS: &[&str] =
        &[CHATGPT_CURATED_PLUGIN_SKILL, API_CURATED_PLUGIN_SKILL];

    #[derive(Clone, Copy)]
    enum TargetAuth {
        Chatgpt,
        ApiKey,
        BedrockApiKey,
        NoCodexAuth,
    }

    #[derive(Clone, Copy)]
    struct Fixture {
        name: &'static str,
        target_auth: TargetAuth,
        target_model_provider_id: &'static str,
        target_prompt: &'static str,
        expected_target_loaded_plugin_skills: &'static [&'static str],
        expected_target_skill_description: &'static str,
    }

    const FIXTURES: &[Fixture] = &[
        Fixture {
            name: "ChatGPT",
            target_auth: TargetAuth::Chatgpt,
            target_model_provider_id: OPENAI_PROVIDER_ID,
            target_prompt: "chatgpt target turn",
            expected_target_loaded_plugin_skills: &[CHATGPT_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "chatgpt description",
        },
        Fixture {
            name: "API key",
            target_auth: TargetAuth::ApiKey,
            target_model_provider_id: OPENAI_PROVIDER_ID,
            target_prompt: "api key target turn",
            expected_target_loaded_plugin_skills: &[API_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "api description before",
        },
        Fixture {
            name: "Bedrock API key",
            target_auth: TargetAuth::BedrockApiKey,
            target_model_provider_id: AMAZON_BEDROCK_PROVIDER_ID,
            target_prompt: "bedrock key target turn",
            expected_target_loaded_plugin_skills: &[API_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "api description before",
        },
        Fixture {
            name: "ambient Bedrock",
            target_auth: TargetAuth::NoCodexAuth,
            target_model_provider_id: AMAZON_BEDROCK_PROVIDER_ID,
            target_prompt: "ambient bedrock target turn",
            expected_target_loaded_plugin_skills: &[API_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "api description before",
        },
    ];

    async fn skills_for_agent_turn(
        test_codex: &TestCodex,
        response: &ResponseMock,
        model_provider_id: &str,
        prompt: &str,
        expected_request_count: usize,
    ) -> Result<String> {
        let mut config = test_codex.config.clone();
        config.model_provider_id = model_provider_id.to_string();
        let thread = test_codex
            .thread_manager
            .start_thread(codex_core::StartThreadOptions::new(config))
            .await?
            .thread;
        thread
            .submit(Op::UserInput {
                items: vec![codex_protocol::user_input::UserInput::Text {
                    text: prompt.to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
                additional_context: Default::default(),
                thread_settings: Default::default(),
            })
            .await?;
        wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;

        let requests = response.requests();
        assert_eq!(requests.len(), expected_request_count);
        Ok(requests
            .last()
            .expect("agent turn should send a request")
            .message_input_text_groups("developer")
            .into_iter()
            .rev()
            .find(|texts| texts.iter().any(|text| text.contains("## Skills")))
            .expect("agent turn should include a skills developer message")
            .join("\n"))
    }

    skip_if_no_network!(Ok(()));
    let assert_loaded_plugin_skills =
        |fixture_name: &str, phase: &str, skills: &str, expected: &[&str]| {
            let loaded_plugin_skills = CURATED_PLUGIN_SKILLS
                .iter()
                .copied()
                .filter(|plugin_skill| skills.contains(plugin_skill))
                .collect::<Vec<_>>();
            assert_eq!(
                loaded_plugin_skills.as_slice(),
                expected,
                "unexpected curated plugin skills for {fixture_name} during {phase}: {skills:?}"
            );
        };

    for fixture in FIXTURES {
        let server = start_mock_server().await;
        let response = mount_sse_sequence(
            &server,
            vec![
                sse(vec![
                    ev_response_created("resp-initial"),
                    ev_completed("resp-initial"),
                ]),
                sse(vec![
                    ev_response_created("resp-target"),
                    ev_completed("resp-target"),
                ]),
            ],
        )
        .await;

        let codex_home = Arc::new(TempDir::new()?);
        std::fs::write(
            codex_home.path().join("config.toml"),
            r#"[features]
plugins = true
remote_plugin = false

[plugins."chatgpt-plugin@openai-curated"]
enabled = true

[plugins."api-plugin@openai-api-curated"]
enabled = true
"#,
        )?;
        for (marketplace_name, plugin_name, skill_name, description) in [
            (
                "openai-curated",
                "chatgpt-plugin",
                "chatgpt-skill",
                "chatgpt description",
            ),
            (
                "openai-api-curated",
                "api-plugin",
                "api-skill",
                "api description before",
            ),
        ] {
            let plugin_root = codex_home
                .path()
                .join("plugins/cache")
                .join(marketplace_name)
                .join(plugin_name)
                .join("local");
            std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
            std::fs::write(
                plugin_root.join(".codex-plugin/plugin.json"),
                format!(r#"{{"name":"{plugin_name}","description":"{plugin_name}"}}"#),
            )?;
            let skill_dir = plugin_root.join("skills").join(skill_name);
            std::fs::create_dir_all(&skill_dir)?;
            std::fs::write(
                skill_dir.join("SKILL.md"),
                format!("---\ndescription: {description}\n---\n\n# body\n"),
            )?;
        }

        let mut builder = test_codex()
            .with_home(Arc::clone(&codex_home))
            .with_extensions(skills_extensions())
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        let test_codex = builder.build_with_auto_env(&server).await?;
        let plugins_manager = test_codex.thread_manager.plugins_manager();
        let skills_service = test_codex.thread_manager.skills_service();

        let initial_skills = skills_for_agent_turn(
            &test_codex,
            &response,
            OPENAI_PROVIDER_ID,
            "initial chatgpt turn",
            /*expected_request_count*/ 1,
        )
        .await?;
        assert_loaded_plugin_skills(
            fixture.name,
            "initial ChatGPT turn",
            &initial_skills,
            &[CHATGPT_CURATED_PLUGIN_SKILL],
        );
        assert!(initial_skills.contains("chatgpt description"));

        std::fs::write(
            codex_home.path().join(
                "plugins/cache/openai-api-curated/api-plugin/local/skills/api-skill/SKILL.md",
            ),
            "---\ndescription: api description after\n---\n\n# body\n",
        )?;

        match fixture.target_auth {
            TargetAuth::Chatgpt => {}
            TargetAuth::ApiKey => {
                plugins_manager.set_auth_mode(Some(AuthMode::ApiKey));
            }
            TargetAuth::BedrockApiKey => {
                plugins_manager.set_auth_mode(Some(AuthMode::BedrockApiKey));
            }
            TargetAuth::NoCodexAuth => {
                test_codex.thread_manager.auth_manager().logout().await?;
                assert_eq!(
                    test_codex.thread_manager.auth_manager().get_api_auth_mode(),
                    None
                );
                plugins_manager.set_auth_mode(/*auth_mode*/ None);
            }
        }
        skills_service.clear_cache();
        let target_skills = skills_for_agent_turn(
            &test_codex,
            &response,
            fixture.target_model_provider_id,
            fixture.target_prompt,
            /*expected_request_count*/ 2,
        )
        .await?;
        assert_loaded_plugin_skills(
            fixture.name,
            "target turn",
            &target_skills,
            fixture.expected_target_loaded_plugin_skills,
        );
        assert!(
            target_skills.contains(fixture.expected_target_skill_description),
            "expected {:?} in current skills: {skills:?}",
            fixture.expected_target_skill_description,
            skills = target_skills
        );
        assert!(!target_skills.contains("api description after"));
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_plugin_mentions_use_apps_for_chatgpt_dual_surface_plugins() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_with_connector_name(&server, "Google Calendar").await?;
    let mock = mount_plugin_tool_search_turn(&server).await;

    let codex_home = Arc::new(TempDir::new()?);
    let rmcp_test_server_bin = match stdio_server_bin() {
        Ok(bin) => bin,
        Err(err) => {
            eprintln!("test_stdio_server binary not available, skipping test: {err}");
            return Ok(());
        }
    };
    write_plugin_skill_plugin(codex_home.as_ref());
    write_plugin_mcp_plugin(codex_home.as_ref(), &rmcp_test_server_bin);
    write_plugin_app_plugin(codex_home.as_ref());

    let test_codex =
        build_apps_enabled_plugin_test_codex(&server, codex_home, apps_server.chatgpt_base_url)
            .await?;
    let codex = Arc::clone(&test_codex.codex);
    wait_for_mcp_server(&codex, CODEX_APPS_MCP_SERVER_NAME).await?;

    codex
        .submit(Op::UserInput {
            items: vec![codex_protocol::user_input::UserInput::Mention {
                name: "sample".into(),
                path: format!("plugin://{SAMPLE_PLUGIN_CONFIG_NAME}"),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = mock.requests();
    let request = &requests[0];
    let developer_messages = request.message_input_texts("developer");
    assert!(
        developer_messages
            .iter()
            .any(|text| text.contains("Skills from this plugin")),
        "expected plugin skills guidance: {developer_messages:?}"
    );
    assert!(
        !developer_messages
            .iter()
            .any(|text| text.contains("MCP servers from this plugin")),
        "expected plugin MCP guidance to be suppressed for ChatGPT auth: {developer_messages:?}"
    );
    assert!(
        developer_messages
            .iter()
            .any(|text| text.contains("Apps from this plugin")),
        "expected visible plugin app guidance: {developer_messages:?}"
    );
    assert!(
        request
            .tool_by_name(SAMPLE_PLUGIN_MCP_NAMESPACE, "echo")
            .is_none(),
        "plugin MCP tool should not leak into the request for ChatGPT auth"
    );
    let (calendar_tool, echo_tool) = searched_plugin_tools(&requests[1]);
    let calendar_tool = calendar_tool.expect("plugin app tool should be searchable");
    assert_plugin_provenance(&calendar_tool);
    assert!(
        echo_tool.is_none(),
        "plugin MCP tool should be suppressed for ChatGPT auth"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_plugin_mentions_keep_non_conflicting_mcp_for_chatgpt_auth() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_with_connector_name(&server, "Google Calendar").await?;
    let mock = mount_plugin_tool_search_turn(&server).await;

    let codex_home = Arc::new(TempDir::new()?);
    let rmcp_test_server_bin = match stdio_server_bin() {
        Ok(bin) => bin,
        Err(err) => {
            eprintln!("test_stdio_server binary not available, skipping test: {err}");
            return Ok(());
        }
    };
    write_plugin_skill_plugin(codex_home.as_ref());
    write_plugin_mcp_plugin(codex_home.as_ref(), &rmcp_test_server_bin);
    write_plugin_app_plugin_with_name(codex_home.as_ref(), "sample_app");

    let test_codex =
        build_apps_enabled_plugin_test_codex(&server, codex_home, apps_server.chatgpt_base_url)
            .await?;
    let codex = Arc::clone(&test_codex.codex);
    wait_for_mcp_server(&codex, "sample").await?;

    codex
        .submit(Op::UserInput {
            items: vec![codex_protocol::user_input::UserInput::Mention {
                name: "sample".into(),
                path: format!("plugin://{SAMPLE_PLUGIN_CONFIG_NAME}"),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = mock.requests();
    let request = &requests[0];
    let developer_messages = request.message_input_texts("developer");
    assert!(
        developer_messages
            .iter()
            .any(|text| text.contains("MCP servers from this plugin")),
        "expected plugin MCP guidance to remain visible for non-conflicting app declaration: {developer_messages:?}"
    );
    assert!(
        developer_messages
            .iter()
            .any(|text| text.contains("Apps from this plugin")),
        "expected plugin app guidance: {developer_messages:?}"
    );
    let (calendar_tool, echo_tool) = searched_plugin_tools(&requests[1]);
    assert!(
        calendar_tool.is_some(),
        "plugin app tool should be searchable"
    );
    let echo_tool = echo_tool.expect("plugin MCP tool should remain searchable");
    assert_plugin_provenance(&echo_tool);

    Ok(())
}

#[derive(Clone, Copy)]
enum ExplicitMcpRequest {
    Plugin,
    PluginSkill,
    ServerMention,
    LinkedServerMention,
}

#[test_case(ExplicitMcpRequest::Plugin; "plugin mention")]
#[test_case(ExplicitMcpRequest::PluginSkill; "plugin skill")]
#[test_case(ExplicitMcpRequest::ServerMention; "MCP server mention")]
#[test_case(ExplicitMcpRequest::LinkedServerMention; "linked MCP server mention")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicitly_requested_mcp_waits_for_startup(request: ExplicitMcpRequest) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mock = mount_plugin_tool_search_turn(&server).await;

    let codex_home = Arc::new(TempDir::new()?);
    let rmcp_test_server_bin = match stdio_server_bin() {
        Ok(bin) => bin,
        Err(err) => {
            eprintln!("test_stdio_server binary not available, skipping test: {err}");
            return Ok(());
        }
    };
    let skill_path = std::fs::canonicalize(write_plugin_skill_plugin(codex_home.as_ref()))?;
    write_plugin_mcp_plugin(codex_home.as_ref(), &rmcp_test_server_bin);
    write_plugin_app_plugin(codex_home.as_ref());
    let initialize_barrier = block_plugin_mcp_startup(codex_home.as_ref(), &rmcp_test_server_bin);

    let mut builder = test_codex()
        .with_home(codex_home)
        .with_auth(CodexAuth::from_api_key("Test API Key"))
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Apps)
                .expect("test config should allow feature update");
        });
    let test_codex = builder
        .build(&server)
        .await
        .expect("create new conversation");
    let codex = Arc::clone(&test_codex.codex);

    let input = match request {
        ExplicitMcpRequest::Plugin => UserInput::Mention {
            name: "sample".into(),
            path: format!("plugin://{SAMPLE_PLUGIN_CONFIG_NAME}"),
        },
        ExplicitMcpRequest::PluginSkill => UserInput::Skill {
            name: "sample:sample-search".into(),
            path: skill_path,
        },
        ExplicitMcpRequest::ServerMention => UserInput::Mention {
            name: "sample".into(),
            path: "mcp://sample".into(),
        },
        ExplicitMcpRequest::LinkedServerMention => UserInput::Text {
            text: "use [$sample](mcp://sample)".to_string(),
            text_elements: Vec::new(),
        },
    };
    codex
        .submit(Op::UserInput {
            items: vec![input],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(
        mock.requests().is_empty(),
        "an explicitly requested MCP should finish starting before inference"
    );
    std::fs::write(initialize_barrier, "ready")?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = mock.requests();
    let model_request = &requests[0];
    let developer_messages = model_request.message_input_texts("developer");
    if matches!(request, ExplicitMcpRequest::Plugin) {
        assert!(
            developer_messages
                .iter()
                .any(|text| text.contains("Skills from this plugin")),
            "expected plugin skills guidance: {developer_messages:?}"
        );
        assert!(
            developer_messages
                .iter()
                .any(|text| text.contains("MCP servers from this plugin")),
            "expected visible plugin MCP guidance: {developer_messages:?}"
        );
    }
    if matches!(request, ExplicitMcpRequest::PluginSkill) {
        let user_messages = model_request.message_input_texts("user");
        assert!(
            user_messages
                .iter()
                .any(|message| message.contains("sample:sample-search")),
            "expected explicitly requested skill instructions: {user_messages:?}"
        );
    }
    assert!(
        !developer_messages
            .iter()
            .any(|text| text.contains("Apps from this plugin")),
        "expected plugin app guidance to be suppressed for API-key auth: {developer_messages:?}"
    );
    assert!(
        model_request
            .tool_by_name(SAMPLE_PLUGIN_APP_NAMESPACE, SEARCH_CALENDAR_CREATE_TOOL)
            .is_none(),
        "plugin app tool should not leak into the request for API-key auth"
    );
    let (calendar_tool, echo_tool) = searched_plugin_tools(&requests[1]);
    assert!(
        calendar_tool.is_none(),
        "plugin app tool should be hidden for API-key auth"
    );
    let echo_tool = echo_tool.expect("plugin MCP tool should be searchable");
    assert_plugin_provenance(&echo_tool);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_plugin_mentions_track_plugin_used_analytics() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let _resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let codex_home = Arc::new(TempDir::new()?);
    write_plugin_skill_plugin(codex_home.as_ref());
    let test_codex = build_analytics_plugin_test_codex(&server, codex_home).await?;
    let codex = Arc::clone(&test_codex.codex);

    codex
        .submit(Op::UserInput {
            items: vec![codex_protocol::user_input::UserInput::Mention {
                name: "sample".into(),
                path: format!("plugin://{SAMPLE_PLUGIN_CONFIG_NAME}"),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let event = wait_for_analytics_event(&server, "codex_plugin_used").await;
    assert_eq!(event["event_params"]["plugin_id"], "sample@test");
    assert_eq!(event["event_params"]["plugin_name"], "sample");
    assert_eq!(event["event_params"]["marketplace_name"], "test");
    assert_eq!(event["event_params"]["has_skills"], true);
    assert_eq!(event["event_params"]["mcp_server_count"], 0);
    assert_eq!(
        event["event_params"]["mcp_server_names"],
        serde_json::json!([])
    );
    assert_eq!(
        event["event_params"]["connector_ids"],
        serde_json::json!([])
    );
    assert_eq!(
        event["event_params"]["product_client_id"],
        serde_json::json!(codex_login::default_client::originator().value)
    );
    assert_eq!(event["event_params"]["model_slug"], "gpt-5.2");
    assert!(event["event_params"]["thread_id"].as_str().is_some());
    assert!(event["event_params"]["turn_id"].as_str().is_some());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_plugin_skill_invocation_tracks_remote_plugin_id() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let _resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let codex_home = Arc::new(TempDir::new()?);
    let skill_path = std::fs::canonicalize(write_remote_plugin_skill_plugin(codex_home.as_ref()))?;
    persist_sample_remote_plugin_id(codex_home.as_ref());
    let test_codex = build_analytics_plugin_test_codex(&server, codex_home).await?;
    let codex = Arc::clone(&test_codex.codex);

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Skill {
                name: "sample:sample-search".into(),
                path: skill_path,
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let event = wait_for_analytics_event(&server, "skill_invocation").await;
    assert_eq!(
        event["event_params"]["plugin_id"],
        SAMPLE_REMOTE_PLUGIN_CONFIG_NAME
    );
    assert_eq!(
        event["event_params"]["remote_plugin_id"],
        SAMPLE_REMOTE_PLUGIN_ID
    );
    assert_eq!(event["event_params"]["invoke_type"], "explicit");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn implicit_plugin_skill_invocation_tracks_remote_plugin_id() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let codex_home = Arc::new(TempDir::new()?);
    let skill_path = write_remote_plugin_skill_plugin(codex_home.as_ref());
    persist_sample_remote_plugin_id(codex_home.as_ref());
    let command_args = serde_json::json!({
        "command": format!("cat {}", skill_path.display()),
        "login": false,
    })
    .to_string();
    let _resp_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call("call-1", "shell_command", &command_args),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let test_codex = build_analytics_plugin_test_codex(&server, codex_home).await?;
    let codex = Arc::clone(&test_codex.codex);

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "inspect the sample skill".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let event = wait_for_analytics_event(&server, "skill_invocation").await;
    assert_eq!(
        event["event_params"]["plugin_id"],
        SAMPLE_REMOTE_PLUGIN_CONFIG_NAME
    );
    assert_eq!(
        event["event_params"]["remote_plugin_id"],
        SAMPLE_REMOTE_PLUGIN_ID
    );
    assert_eq!(event["event_params"]["invoke_type"], "implicit");

    Ok(())
}

fn persist_sample_remote_plugin_id(home: &TempDir) {
    let plugin_id =
        PluginId::parse(SAMPLE_REMOTE_PLUGIN_CONFIG_NAME).expect("remote plugin id should parse");
    PluginStore::new(home.path().to_path_buf())
        .write_remote_plugin_id(&plugin_id, SAMPLE_REMOTE_PLUGIN_ID)
        .expect("persist remote plugin id");
}

async fn wait_for_analytics_event(server: &MockServer, event_type: &str) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let requests = server.received_requests().await.unwrap_or_default();
        if let Some(event) = requests
            .into_iter()
            .filter(|request| request.url.path() == "/codex/analytics-events/events")
            .find_map(|request| {
                let payload: serde_json::Value = serde_json::from_slice(&request.body).ok()?;
                payload["events"].as_array().and_then(|events| {
                    events
                        .iter()
                        .find(|event| event["event_type"] == event_type)
                        .cloned()
                })
            })
        {
            break event;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for {event_type} analytics request");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
