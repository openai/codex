#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use codex_core::CodexAuth;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use dunce::canonicalize as normalize_path;
use tempfile::TempDir;
use wiremock::MockServer;

fn write_plugin_skill_plugin(home: &TempDir) -> std::path::PathBuf {
    let plugin_root = home.path().join("plugins/sample");
    let skill_dir = plugin_root.join("skills/sample-search");
    std::fs::create_dir_all(skill_dir.as_path()).expect("create plugin skill dir");
    std::fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create plugin manifest dir");
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )
    .expect("write plugin manifest");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: inspect sample data\n---\n\n# body\n",
    )
    .expect("write plugin skill");
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "[features]\nplugins = true\n\n[plugins.sample]\nenabled = true\npath = \"{}\"\n",
            plugin_root.display()
        ),
    )
    .expect("write config");
    skill_dir.join("SKILL.md")
}

fn write_plugin_mcp_plugin(home: &TempDir, command: &str) {
    let plugin_root = home.path().join("plugins/sample");
    std::fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create plugin manifest dir");
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )
    .expect("write plugin manifest");
    std::fs::write(
        plugin_root.join(".mcp.json"),
        format!(
            r#"{{
  "mcpServers": {{
    "sample": {{
      "command": "{command}"
    }}
  }}
}}"#
        ),
    )
    .expect("write plugin mcp config");
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "[features]\nplugins = true\n\n[plugins.sample]\nenabled = true\npath = \"{}\"\n",
            plugin_root.display()
        ),
    )
    .expect("write config");
}

async fn build_plugin_test_codex(
    server: &MockServer,
    codex_home: Arc<TempDir>,
) -> Result<Arc<codex_core::CodexThread>> {
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_auth(CodexAuth::from_api_key("Test API Key"));
    Ok(builder
        .build(server)
        .await
        .expect("create new conversation")
        .codex)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plugin_skills_append_to_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = MockServer::start().await;

    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let codex_home = Arc::new(TempDir::new()?);
    let skill_path = write_plugin_skill_plugin(codex_home.as_ref());
    let codex = build_plugin_test_codex(&server, Arc::clone(&codex_home)).await?;

    codex
        .submit(Op::UserInput {
            items: vec![codex_protocol::user_input::UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = resp_mock.single_request();
    let request_body = request.body_json();
    let instructions_text = request_body["input"][1]["content"][0]["text"]
        .as_str()
        .expect("instructions text");
    assert!(
        instructions_text.contains("## Skills"),
        "expected skills section present"
    );
    assert!(
        instructions_text.contains("sample:sample-search: inspect sample data"),
        "expected namespaced plugin skill summary"
    );
    let expected_path = normalize_path(skill_path)?;
    let expected_path_str = expected_path.to_string_lossy().replace('\\', "/");
    assert!(
        instructions_text.contains(&expected_path_str),
        "expected path {expected_path_str} in instructions"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn plugin_mcp_tools_are_listed() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let codex_home = Arc::new(TempDir::new()?);
    let rmcp_test_server_bin = stdio_server_bin()?;
    write_plugin_mcp_plugin(codex_home.as_ref(), &rmcp_test_server_bin);
    let codex = build_plugin_test_codex(&server, codex_home).await?;

    let tools_ready_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        codex.submit(Op::ListMcpTools).await?;
        let list_event = wait_for_event_with_timeout(
            &codex,
            |ev| matches!(ev, EventMsg::McpListToolsResponse(_)),
            Duration::from_secs(10),
        )
        .await;
        let EventMsg::McpListToolsResponse(tool_list) = list_event else {
            unreachable!("event guard guarantees McpListToolsResponse");
        };
        if tool_list.tools.contains_key("mcp__sample__echo")
            && tool_list.tools.contains_key("mcp__sample__image")
        {
            break;
        }

        let available_tools: Vec<&str> = tool_list.tools.keys().map(String::as_str).collect();
        if Instant::now() >= tools_ready_deadline {
            panic!("timed out waiting for plugin MCP tools; discovered tools: {available_tools:?}");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(())
}
