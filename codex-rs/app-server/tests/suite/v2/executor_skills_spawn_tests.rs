//! Executor skill selection survives subagent spawning independently of forked history.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::CapabilityRootLocation;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SelectedCapabilityRoot;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_exec_server::CreateDirectoryOptions;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;
use tokio::sync::Notify;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::matchers::method;
use wiremock::matchers::path;

/// Both capability roots and the parent's plugin choices apply regardless of copied history.
#[test_case("none", false; "no history enabled")]
#[test_case("all", false; "full history enabled")]
#[test_case("none", true; "no history disabled")]
#[test_case("all", true; "full history disabled")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawned_agent_inherits_selected_executor_skill(
    fork_turns: &str,
    disabled: bool,
) -> Result<()> {
    let server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_extra_config(
            "[features.multi_agent_v2]\nenabled = true\n\n[skills]\ninclude_instructions = true\n",
        )
        .write(codex_home.path())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    // Install only on the executor, and select the root through the same API as the bridge.
    let auto_env = app_server.auto_env()?;
    let environment_id = auto_env.selection().environment_id.clone();
    let plugin_dir = auto_env.selection().cwd.join("spawn-plugin")?;
    let manifest_dir = plugin_dir.join(".codex-plugin")?;
    let skill_dir = plugin_dir.join("skills/deploy")?;
    let file_system = auto_env.environment().get_filesystem();
    for directory in [&manifest_dir, &skill_dir] {
        file_system
            .create_directory(
                directory,
                CreateDirectoryOptions {
                    recursive: true,
                    follow_symlinks: true,
                },
                /*sandbox*/ None,
            )
            .await?;
    }
    file_system
        .write_file(
            &manifest_dir.join("plugin.json")?,
            br#"{"name":"spawn-plugin"}"#.to_vec(),
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    // A registered but unreachable server is still visible in mcpServerStatus/list.
    // Disabled plugins must not register it at all.
    file_system
        .write_file(
            &plugin_dir.join(".mcp.json")?,
            br#"{"mcpServers":{"spawn-probe":{"type":"http","url":"http://127.0.0.1:9/mcp"}}}"#
                .to_vec(),
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    let skill_contents = "---\nname: deploy\ndescription: Deploy through the inherited executor.\n---\n\nINHERITED_EXECUTOR_SKILL_BODY\n";
    file_system
        .write_file(
            &skill_dir.join("SKILL.md")?,
            skill_contents.as_bytes().to_vec(),
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    let request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            selected_capability_roots: Some(vec![SelectedCapabilityRoot {
                id: "spawn-plugin@1".to_string(),
                location: CapabilityRootLocation::Environment {
                    environment_id,
                    path: plugin_dir,
                },
            }]),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } = to_response(
        app_server
            .read_stream_until_response_message(RequestId::Integer(request_id))
            .await?,
    )?;
    let parent_id = thread.id.clone();
    let fork_turns = fork_turns.to_string();
    let child_read_completed = Arc::new(Notify::new());
    let notify_child_read = Arc::clone(&child_read_completed);
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(move |request: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&request.body).expect("responses request");
            let parent = request.headers["thread-id"] == parent_id;
            let agent = if parent { "parent" } else { "child" };
            let completed = |call_id: &str| {
                body["input"]
                    .as_array()
                    .expect("input array")
                    .iter()
                    .any(|item| {
                        item["type"] == "function_call_output" && item["call_id"] == call_id
                    })
            };
            let list_call = format!("{agent}-list");
            let read_call = format!("{agent}-read");
            let item = if !completed(&list_call) {
                responses::ev_function_call_with_namespace(
                    &list_call,
                    "skills",
                    "list",
                    r#"{"authority":{"kind":"executor"}}"#,
                )
            } else if !completed(&read_call) {
                responses::ev_function_call_with_namespace(
                    &read_call,
                    "skills",
                    "read",
                    r#"{"package":"e0/skills/deploy"}"#,
                )
            } else if parent && !completed("spawn-worker") {
                responses::ev_function_call_with_namespace(
                    "spawn-worker",
                    "collaboration",
                    "spawn_agent",
                    &json!({
                        "task_name": "worker",
                        "message": "List the executor skills and read the deploy skill.",
                        "fork_turns": fork_turns,
                    })
                    .to_string(),
                )
            } else {
                if !parent {
                    notify_child_read.notify_one();
                }
                responses::ev_assistant_message(&format!("{agent}-done"), "Done")
            };
            responses::sse_response(responses::sse(vec![
                responses::ev_response_created("response"),
                item,
                responses::ev_completed("response"),
            ]))
        })
        .mount(&server)
        .await;
    timeout(
        Duration::from_secs(/*secs*/ 30),
        app_server.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            disabled_plugin_ids: Some(if disabled {
                vec!["spawn-plugin@1".to_string()]
            } else {
                Vec::new()
            }),
            input: vec![UserInput::Text {
                text: "List and read the deploy skill, then delegate the same task.".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;
    timeout(
        Duration::from_secs(/*secs*/ 30),
        child_read_completed.notified(),
    )
    .await?;

    let requests = responses::received_responses_requests(&server).await;
    let mut catalogs = Vec::new();
    for agent in ["parent", "child"] {
        let read_call = format!("{agent}-read");
        let request = requests
            .iter()
            .find(|request| request.function_call_output_text(&read_call).is_some())
            .expect("agent should return its skill read result to the model");
        let output = request
            .function_call_output_text(&read_call)
            .expect("skill read result");
        let list: Value = serde_json::from_str(
            &request
                .function_call_output_text(&format!("{agent}-list"))
                .expect("skill list result"),
        )?;
        if disabled {
            assert_eq!(list["skills"], json!([]), "{agent} disabled plugin catalog");
            assert!(
                !output.contains("INHERITED_EXECUTOR_SKILL_BODY"),
                "{agent} read a disabled skill: {output}"
            );
        } else {
            let read: Value = serde_json::from_str(&output)
                .unwrap_or_else(|_| panic!("{agent} could not read the executor skill: {output}"));
            assert_eq!(read["contents"], skill_contents, "{agent} skill contents");
            assert_eq!(read["skill_root"], skill_dir.inferred_native_path_string());
            assert_eq!(list["skills"][0]["name"], "spawn-plugin:deploy");
        }
        let status_id = app_server
            .send_list_mcp_server_status_request(ListMcpServerStatusParams {
                thread_id: Some(request.header("thread-id").expect("agent thread ID")),
                server_name: None,
                cursor: None,
                limit: None,
                detail: None,
            })
            .await?;
        let status: ListMcpServerStatusResponse = app_server.read_response(status_id).await?;
        let has_plugin_server = status
            .data
            .iter()
            .any(|server| server.name.contains("spawn-probe"));
        assert_eq!(
            has_plugin_server, !disabled,
            "{agent} plugin MCP registration"
        );
        catalogs.push(list);
    }
    assert_eq!(
        catalogs[0], catalogs[1],
        "parent and child executor catalogs"
    );
    Ok(())
}
