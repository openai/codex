#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use codex_core::TurnInputRequest;
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
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::HostSkillsLoadInput;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::install;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::skip_if_target_windows;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server;
use core_test_support::zsh_fork::zsh_fork_runtime;
use core_test_support::zsh_fork::zsh_fork_test_builder;
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
        max_context_tokens: config.skill_max_context_tokens,
        bundled_skills_enabled: config.bundled_skills_enabled(),
        orchestrator_skills_enabled: config.orchestrator_skills_enabled,
        shadow_selection_enabled: config.features.enabled(Feature::SkillSearch),
    });
    Arc::new(extensions.build())
}

fn sample_plugin_root(home: &TempDir) -> std::path::PathBuf {
    home.path().join("plugins/cache/test/sample/local")
}

pub(super) fn write_sample_plugin_manifest_and_config(home: &TempDir) -> std::path::PathBuf {
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

fn write_agent_plugin_skill_plugin(home: &TempDir) -> std::path::PathBuf {
    let plugin_root = home.path().join("plugins/cache/test/acme.tools/local");
    let direct_skill = plugin_root.join("skills/review");
    let nested_skill = plugin_root.join("skills/group/hidden");
    std::fs::create_dir_all(&direct_skill).expect("create direct skill");
    std::fs::create_dir_all(&nested_skill).expect("create nested skill");
    std::fs::write(
        plugin_root.join("plugin.json"),
        r#"{"$schema":"https://agent-plugins.org/schemas/1.0.0/plugin.schema.json","name":"acme.tools","extensions":{"com.openai":{"interface":{"displayName":"Acme Developer Tools"}}}}"#,
    )
    .expect("write Agent Plugin manifest");
    std::fs::write(
        direct_skill.join("SKILL.md"),
        format!(
            "---\nname: review\ndescription: Review code\n---\n\n{}\nAGENT_SKILL_TRUNCATED_TAIL\n",
            "x".repeat(9_000)
        ),
    )
    .expect("write direct skill");
    std::fs::write(
        nested_skill.join("SKILL.md"),
        "---\nname: hidden\ndescription: Hidden skill\n---\n\nHidden.\n",
    )
    .expect("write nested skill");
    std::fs::write(
        home.path().join("config.toml"),
        "[features]\nplugins = true\n\n[plugins.\"acme.tools@test\"]\nenabled = true\n",
    )
    .expect("write Agent Plugin config");
    direct_skill.join("SKILL.md")
}

fn write_plugin_mcp_plugin(home: &TempDir, command: &str) {
    let plugin_root = write_sample_plugin_manifest_and_config(home);
    std::fs::write(
        plugin_root.join(".mcp.json"),
        serde_json::to_vec(&serde_json::json!({
            "mcpServers": {
                "sample": {
                    "command": command,
                    "cwd": ".",
                    "startup_timeout_sec": 60.0,
                },
            },
        }))
        .expect("serialize plugin MCP configuration"),
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
    builder.build_with_auto_env(server).await
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
    builder.build_with_remote_and_local_env(server).await
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

#[test_case(false; "classic shell")]
#[test_case(true; "zsh-fork shell")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persisted_remote_plugin_command_attribution_flows_through_turn_context(
    zsh_fork: bool,
) -> Result<()> {
    skip_if_target_windows!(Ok(()), "executes a POSIX shell script");
    skip_if_no_network!(Ok(()));
    skip_if_remote!(
        Ok(()),
        "remote plugin attribution fixture uses a local Codex home cache"
    );

    let server = start_mock_server().await;
    let codex_home = Arc::new(TempDir::new()?);
    let script_path = write_remote_plugin_script_and_config(codex_home.as_ref());
    std::fs::write(
        &script_path,
        r#"printf '%s' '{"version":1,"measurements":[{"name":"files_scanned","value":7}]}' > "$CODEX_PLUGIN_METRICS_OUTPUT"
"#,
    )?;
    let plugin_root = script_path
        .parent()
        .and_then(std::path::Path::parent)
        .expect("plugin root");
    std::fs::write(
        plugin_root.join("analytics.yaml"),
        "version: 1\noperations: {scan: {path: ./scripts/run.sh, measurements: {files_scanned: {}}}}\n",
    )?;
    let builder = if zsh_fork {
        let Some(runtime) = zsh_fork_runtime("zsh-fork plugin measurement test")? else {
            return Ok(());
        };
        zsh_fork_test_builder(runtime, AskForApproval::Never)
    } else {
        test_codex()
    };
    let command = shlex::try_join(["/bin/sh", script_path.to_string_lossy().as_ref()])?;
    let call_id = "remote-plugin-command";
    let arguments = serde_json::to_string(&serde_json::json!({
        "cmd": command,
        "login": false,
    }))?;
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "exec_command", &arguments),
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

    let chatgpt_base_url = server.uri();
    let mut builder = builder
        .with_home(Arc::clone(&codex_home))
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5.2")
        .with_config(move |config| config.chatgpt_base_url = chatgpt_base_url);
    let test_codex = builder.build_with_auto_env(&server).await?;
    let codex = Arc::clone(&test_codex.codex);
    let cwd = test_codex.config.cwd.clone();
    let session_model = test_codex.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::read_only(), cwd.as_path());
    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![codex_protocol::user_input::UserInput::Text {
                text: "run the remote plugin script".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(cwd)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
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
    assert_eq!(
        end.exit_code, 0,
        "sandboxed plugin command failed: {}",
        end.aggregated_output
    );
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    for (plugin_id, script_path) in [
        (begin.plugin_id.as_deref(), begin.script_path.as_deref()),
        (end.plugin_id.as_deref(), end.script_path.as_deref()),
    ] {
        assert_eq!(plugin_id, Some(REMOTE_PLUGIN_CONFIG_NAME));
        assert_eq!(script_path, Some("scripts/run.sh"));
    }

    let measurement = wait_for_analytics_event(&server, "codex_plugin_measurement_event").await;
    assert_eq!(
        serde_json::json!({
            "plugin_id": measurement["event_params"]["plugin_id"],
            "operation": measurement["event_params"]["operation"],
            "measurement_name": measurement["event_params"]["measurement_name"],
            "number_value": measurement["event_params"]["number_value"],
        }),
        serde_json::json!({
            "plugin_id": REMOTE_PLUGIN_CONFIG_NAME,
            "operation": "scan",
            "measurement_name": "files_scanned",
            "number_value": 7.0,
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_plugin_skills_use_shared_catalog_and_direct_child_discovery() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    let skill_path = dunce::canonicalize(write_agent_plugin_skill_plugin(codex_home.as_ref()))?;
    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_extensions(skills_extensions());
    let test_codex = builder.build_with_auto_env(&server).await?;

    test_codex
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Skill {
            name: "acme.tools:review".into(),
            path: skill_path,
        }]))
        .await?;
    let warning = wait_for_event(&test_codex.codex, |ev| {
        matches!(
            ev,
            EventMsg::Warning(warning)
                if warning.message.contains("main prompt context limit")
        )
    })
    .await;
    wait_for_event(&test_codex.codex, |ev| {
        matches!(ev, EventMsg::TurnComplete(_))
    })
    .await;

    let developer_text = resp_mock
        .single_request()
        .message_input_texts("developer")
        .join("\n");
    assert!(developer_text.contains("acme.tools:review: Review code"));
    assert!(!developer_text.contains("acme.tools:hidden"));
    let user_text = resp_mock
        .single_request()
        .message_input_texts("user")
        .join("\n");
    assert!(user_text.contains("acme.tools:review"));
    assert!(!user_text.contains("AGENT_SKILL_TRUNCATED_TAIL"));
    let EventMsg::Warning(warning) = warning else {
        unreachable!("wait_for_event matched an Agent skill truncation warning")
    };
    assert!(warning.message.contains("acme.tools:review"));
    Ok(())
}

#[test_case("CHATGPT", false, None; "product restricted skill is unavailable")]
#[test_case("CODEX", true, Some("native review skill"); "native skill wins over migrated command")]
#[test_case("CHATGPT", true, Some("migrated review command"); "migrated command replaces filtered native skill")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plugin_skill_product_policy_and_migrated_command_precedence_reach_agent_turns(
    native_skill_product: &str,
    include_migrated_command: bool,
    expected_skill_description: Option<&str>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    let plugin_root = write_sample_plugin_manifest_and_config(codex_home.as_ref());
    let native_skill_dir = plugin_root.join("skills/review");
    std::fs::create_dir_all(native_skill_dir.join("agents"))?;
    std::fs::write(
        native_skill_dir.join("SKILL.md"),
        "---\nname: source-command-review\ndescription: native review skill\n---\n",
    )?;
    std::fs::write(
        native_skill_dir.join("agents/openai.yaml"),
        format!("policy:\n  products: [{native_skill_product}]\n"),
    )?;
    if include_migrated_command {
        let migrated_skill_dir =
            plugin_root.join(".codex-plugin/migrated-command-skills/source-command-review");
        std::fs::create_dir_all(&migrated_skill_dir)?;
        std::fs::write(
            migrated_skill_dir.join("SKILL.md"),
            "---\nname: source-command-review\ndescription: migrated review command\n---\n",
        )?;
    }

    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_extensions(skills_extensions());
    let test = builder.build_with_auto_env(&server).await?;
    let plugin_outcome = test
        .thread_manager
        .plugins_manager()
        .plugins_for_config(&test.config.plugins_config_input())
        .await;
    assert_eq!(
        plugin_outcome
            .plugins()
            .iter()
            .map(|plugin| (plugin.config_name.as_str(), plugin.has_enabled_skills))
            .collect::<Vec<_>>(),
        vec![(
            SAMPLE_PLUGIN_CONFIG_NAME,
            expected_skill_description.is_some()
        )]
    );
    assert_eq!(
        plugin_outcome
            .capability_summaries()
            .iter()
            .map(|plugin| (plugin.config_name.as_str(), plugin.has_skills))
            .collect::<Vec<_>>(),
        expected_skill_description
            .map(|_| (SAMPLE_PLUGIN_CONFIG_NAME, true))
            .into_iter()
            .collect::<Vec<_>>()
    );

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Inspect the available plugin skills.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let developer_text = response
        .single_request()
        .message_input_texts("developer")
        .join("\n");
    assert_eq!(
        (
            developer_text.contains("sample:source-command-review: native review skill"),
            developer_text.contains("sample:source-command-review: migrated review command"),
        ),
        (
            expected_skill_description == Some("native review skill"),
            expected_skill_description == Some("migrated review command"),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_plugin_skill_prompt_remains_complete() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    let skill_path = write_plugin_skill_plugin(codex_home.as_ref());
    let skill_contents = format!(
        "---\nname: sample-search\ndescription: inspect sample data\n---\n\n{}\nLEGACY_SKILL_FULL_TAIL\n",
        "x".repeat(9_000)
    );
    std::fs::write(&skill_path, &skill_contents)?;
    let skill_path = dunce::canonicalize(skill_path)?;
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_extensions(skills_extensions());
    let test_codex = builder.build_with_auto_env(&server).await?;

    test_codex
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Skill {
            name: "sample:sample-search".into(),
            path: skill_path,
        }]))
        .await?;
    wait_for_event(&test_codex.codex, |ev| {
        matches!(ev, EventMsg::TurnComplete(_))
    })
    .await;

    let user_text = resp_mock
        .single_request()
        .message_input_texts("user")
        .join("\n");
    assert!(user_text.contains(&skill_contents));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_plugin_root_mcp_stdio_tool_round_trip_expands_reserved_paths() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let search_call_id = "search-agent-echo";
    let tool_call_id = "call-agent-echo";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_tool_search_call(search_call_id, &serde_json::json!({"query": "echo"})),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call_with_namespace(
                    tool_call_id,
                    "mcp__agent",
                    "echo",
                    r#"{"message":"ping"}"#,
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    write_agent_plugin_skill_plugin(codex_home.as_ref());
    let plugin_root = codex_home
        .path()
        .join("plugins/cache/test/acme.tools/local");
    let stdio_server = match stdio_server_bin() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("test_stdio_server binary not available, skipping test: {err}");
            return Ok(());
        }
    };
    let stdio_server_name = format!("test_stdio_server{}", std::env::consts::EXE_SUFFIX);
    std::fs::copy(stdio_server, plugin_root.join(&stdio_server_name))?;
    let mcp_config = serde_json::json!({
        "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
        "mcpServers": {
            "agent": {
                "type": "stdio",
                "command": format!("./{stdio_server_name}"),
                "env": {"MCP_TEST_VALUE": "${PLUGIN_ROOT}|${PLUGIN_DATA}"}
            }
        }
    });
    std::fs::write(
        plugin_root.join("mcp.json"),
        serde_json::to_vec_pretty(&mcp_config)?,
    )?;
    let mut builder = test_codex().with_home(Arc::clone(&codex_home));
    let test_codex = builder.build_with_remote_and_local_env(&server).await?;
    wait_for_mcp_server(&test_codex.codex, "agent").await?;
    let data_root = dunce::canonicalize(
        std::fs::read_dir(codex_home.path().join("plugins/data/agent-plugins"))?
            .next()
            .expect("Agent Plugin data root")?
            .path(),
    )?;
    let expected_env = format!(
        "{}|{}",
        dunce::canonicalize(&plugin_root)?.display(),
        data_root.display()
    );

    test_codex
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "call the Agent Plugin echo tool".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let end = wait_for_event(&test_codex.codex, |event| {
        matches!(event, EventMsg::McpToolCallEnd(_))
    })
    .await;
    wait_for_event(&test_codex.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let EventMsg::McpToolCallEnd(end) = end else {
        unreachable!("wait_for_event matched an MCP tool end")
    };
    let result = end.result.as_ref().expect("Agent Plugin MCP tool result");
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .and_then(|content| content.get("env"))
            .and_then(serde_json::Value::as_str),
        Some(expected_env.as_str())
    );
    let requests = mock.requests();
    let search_output = requests[1].tool_search_output(search_call_id);
    assert!(namespace_child_tool(&search_output, "mcp__agent", "echo").is_some());
    assert!(requests[2].function_call_output(tool_call_id).is_object());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn curated_plugin_skills_follow_auth_switch() -> Result<()> {
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
        expected_target_loaded_plugin_skills: &'static [&'static str],
        expected_target_skill_description: &'static str,
    }

    const FIXTURES: &[Fixture] = &[
        Fixture {
            name: "ChatGPT",
            target_auth: TargetAuth::Chatgpt,
            target_model_provider_id: OPENAI_PROVIDER_ID,
            expected_target_loaded_plugin_skills: &[CHATGPT_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "chatgpt description",
        },
        Fixture {
            name: "ChatGPT with a custom provider",
            target_auth: TargetAuth::Chatgpt,
            target_model_provider_id: "ollama",
            expected_target_loaded_plugin_skills: &[CHATGPT_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "chatgpt description",
        },
        Fixture {
            name: "API key",
            target_auth: TargetAuth::ApiKey,
            target_model_provider_id: OPENAI_PROVIDER_ID,
            expected_target_loaded_plugin_skills: &[API_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "api description before",
        },
        Fixture {
            name: "Bedrock API key",
            target_auth: TargetAuth::BedrockApiKey,
            target_model_provider_id: AMAZON_BEDROCK_PROVIDER_ID,
            expected_target_loaded_plugin_skills: &[API_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "api description before",
        },
        Fixture {
            name: "ambient Bedrock",
            target_auth: TargetAuth::NoCodexAuth,
            target_model_provider_id: AMAZON_BEDROCK_PROVIDER_ID,
            expected_target_loaded_plugin_skills: &[API_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "api description before",
        },
        Fixture {
            name: "unauthenticated OpenAI",
            target_auth: TargetAuth::NoCodexAuth,
            target_model_provider_id: OPENAI_PROVIDER_ID,
            expected_target_loaded_plugin_skills: &[API_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "api description before",
        },
        Fixture {
            name: "unauthenticated custom provider",
            target_auth: TargetAuth::NoCodexAuth,
            target_model_provider_id: "ollama",
            expected_target_loaded_plugin_skills: &[API_CURATED_PLUGIN_SKILL],
            expected_target_skill_description: "api description before",
        },
    ];

    async fn loaded_plugin_skills_for_config(test_codex: &TestCodex, config: &Config) -> String {
        let plugins_input = config.plugins_config_input();
        let plugins_manager = test_codex.thread_manager.plugins_manager();
        let plugin_outcome = plugins_manager.plugins_for_config(&plugins_input).await;
        let skills_input = HostSkillsLoadInput::new(
            config.cwd.clone(),
            plugin_outcome.effective_plugin_skill_roots(),
            config.config_layer_stack.clone(),
        )
        .with_plugin_skill_snapshots(
            plugins_manager.plugin_skill_snapshots_for_config(&plugins_input),
        );
        let skills_snapshot = test_codex
            .thread_manager
            .skills_service()
            .snapshot_for_config(&skills_input, /*fs*/ None)
            .await;
        skills_snapshot
            .outcome()
            .skills
            .iter()
            .filter_map(|skill| {
                let plugin_id = skill.plugin_id.as_deref()?;
                let plugin_name = plugin_id
                    .split_once('@')
                    .map_or(plugin_id, |(plugin_name, _)| plugin_name);
                Some(format!(
                    "{plugin_name}:{}\n{}",
                    skill.name, skill.description
                ))
            })
            .collect::<Vec<_>>()
            .join("\n")
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
        let initial_skills = loaded_plugin_skills_for_config(&test_codex, &test_codex.config).await;
        assert_loaded_plugin_skills(
            fixture.name,
            "initial ChatGPT config",
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

        let expected_auth_mode = match fixture.target_auth {
            TargetAuth::Chatgpt => Some(AuthMode::Chatgpt),
            TargetAuth::ApiKey => {
                codex_login::login_with_api_key(
                    codex_home.path(),
                    "test-api-key",
                    codex_login::AuthCredentialsStoreMode::File,
                    codex_login::AuthKeyringBackendKind::default(),
                )?;
                test_codex.thread_manager.auth_manager().reload().await;
                Some(AuthMode::ApiKey)
            }
            TargetAuth::BedrockApiKey => {
                codex_login::login_with_bedrock_api_key(
                    codex_home.path(),
                    "test-bedrock-api-key",
                    "us-east-1",
                    codex_login::AuthCredentialsStoreMode::File,
                    codex_login::AuthKeyringBackendKind::default(),
                )?;
                test_codex.thread_manager.auth_manager().reload().await;
                Some(AuthMode::BedrockApiKey)
            }
            TargetAuth::NoCodexAuth => {
                test_codex.thread_manager.auth_manager().logout().await?;
                None
            }
        };
        assert_eq!(
            test_codex.thread_manager.auth_manager().get_api_auth_mode(),
            expected_auth_mode
        );
        test_codex.thread_manager.skills_service().clear_cache();
        let mut target_config = test_codex.config.clone();
        target_config.model_provider_id = fixture.target_model_provider_id.to_string();
        let target_skills = loaded_plugin_skills_for_config(&test_codex, &target_config).await;
        assert_loaded_plugin_skills(
            fixture.name,
            "target config",
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

#[test_case(true; "enabled app")]
#[test_case(false; "disabled app")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_plugin_mentions_use_apps_for_chatgpt_dual_surface_plugins(
    app_enabled: bool,
) -> Result<()> {
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
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        config_path,
        format!("{config}\n[apps.calendar]\nenabled = {app_enabled}\n"),
    )?;

    let test_codex =
        build_apps_enabled_plugin_test_codex(&server, codex_home, apps_server.chatgpt_base_url)
            .await?;
    let codex = Arc::clone(&test_codex.codex);
    wait_for_mcp_server(&codex, CODEX_APPS_MCP_SERVER_NAME).await?;

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            codex_protocol::user_input::UserInput::Mention {
                name: "sample".into(),
                path: format!("plugin://{SAMPLE_PLUGIN_CONFIG_NAME}"),
            },
        ]))
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
    assert_eq!(
        developer_messages
            .iter()
            .any(|text| text.contains("Apps from this plugin")),
        app_enabled,
        "plugin app guidance should match app enablement: {developer_messages:?}"
    );
    assert_eq!(
        developer_messages
            .iter()
            .any(|text| text.contains("if `tool_search` is available")),
        app_enabled,
        "plugin app search guidance should match app enablement: {developer_messages:?}"
    );
    assert!(
        request
            .tool_by_name(SAMPLE_PLUGIN_MCP_NAMESPACE, "echo")
            .is_none(),
        "plugin MCP tool should not leak into the request for ChatGPT auth"
    );
    let (calendar_tool, echo_tool) = searched_plugin_tools(&requests[1]);
    assert_eq!(
        calendar_tool.is_some(),
        app_enabled,
        "plugin app tool search should match app enablement"
    );
    if let Some(calendar_tool) = calendar_tool {
        assert_plugin_provenance(&calendar_tool);
    }
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
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            codex_protocol::user_input::UserInput::Mention {
                name: "sample".into(),
                path: format!("plugin://{SAMPLE_PLUGIN_CONFIG_NAME}"),
            },
        ]))
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
    let skill_path = dunce::canonicalize(write_plugin_skill_plugin(codex_home.as_ref()))?;
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
    let test_codex = builder.build_with_remote_and_local_env(&server).await?;
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
        .start_or_steer_turn(TurnInputRequest::user_input(vec![input]))
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
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            codex_protocol::user_input::UserInput::Mention {
                name: "sample".into(),
                path: format!("plugin://{SAMPLE_PLUGIN_CONFIG_NAME}"),
            },
        ]))
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
    let skill_path = dunce::canonicalize(write_remote_plugin_skill_plugin(codex_home.as_ref()))?;
    persist_sample_remote_plugin_id(codex_home.as_ref());
    let test_codex = build_analytics_plugin_test_codex(&server, codex_home).await?;
    let codex = Arc::clone(&test_codex.codex);

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Skill {
            name: "sample:sample-search".into(),
            path: skill_path,
        }]))
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

#[derive(Clone, Copy)]
enum ImplicitPluginSkillInvocation {
    SkillDocumentRead,
    SkillScriptRun,
}

#[test_case(ImplicitPluginSkillInvocation::SkillDocumentRead; "skill document read")]
#[test_case(ImplicitPluginSkillInvocation::SkillScriptRun; "skill script run")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn implicit_plugin_skill_invocation_tracks_remote_plugin_id(
    invocation: ImplicitPluginSkillInvocation,
) -> Result<()> {
    skip_if_target_windows!(Ok(()), "executes POSIX cat and bash commands");
    skip_if_remote!(Ok(()), "shell commands use host plugin-cache paths");
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let codex_home = Arc::new(TempDir::new()?);
    let skill_path = write_remote_plugin_skill_plugin(codex_home.as_ref());
    persist_sample_remote_plugin_id(codex_home.as_ref());
    let command = match invocation {
        ImplicitPluginSkillInvocation::SkillDocumentRead => {
            format!("cat {}", skill_path.display())
        }
        ImplicitPluginSkillInvocation::SkillScriptRun => {
            let script_path = skill_path
                .parent()
                .expect("skill path should have a parent")
                .join("scripts/test.sh");
            std::fs::create_dir_all(
                script_path
                    .parent()
                    .expect("script path should have a parent"),
            )?;
            std::fs::write(&script_path, "echo skill script invoked\n")?;
            format!("bash {}", script_path.display())
        }
    };
    let command_args = serde_json::json!({
        "cmd": command,
        "login": false,
    })
    .to_string();
    let _resp_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call("call-1", "exec_command", &command_args),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let test_codex = build_analytics_plugin_test_codex(&server, codex_home).await?;
    let codex = Arc::clone(&test_codex.codex);

    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "inspect the sample skill".into(),
            text_elements: Vec::new(),
        }]))
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
