//! App omissions cannot bypass strict deferral; app access restrictions still apply.
use super::*;
use pretty_assertions::assert_eq;
use std::sync::Mutex;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[tracing_test::traced_test]
async fn strict_ignores_app_omissions_but_preserves_app_access() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let mut tools = vec![];
    for name in ["lookup", "disabled", "app_only"] {
        let mut tool = json!({
            "name": name,
            "description": format!("Calendar {name}"),
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": {"readOnlyHint": true},
            "_meta": {"connector_id": "calendar", "connector_name": "Calendar"}
        });
        if name == "app_only" {
            tool["_meta"]["ui"] = json!({"visibility": ["app"]});
        }
        tools.push(tool);
    }
    AppsTestServer::mount_with_tools(&server, Arc::new(Mutex::new(tools))).await?;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(Arc::new(AppsServer(vec![])));
    let script = r#"const names = ALL_TOOLS.filter(t => t.name.startsWith("mcp__codex_apps__calendar__")).map(t => t.name).sort();
const t = ALL_TOOLS.find(t => t.name === "mcp__codex_apps__calendar__lookup");
text(JSON.stringify({names, result: t ? (await tools[t.name]({})).content[0].text : null}));"#;
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_custom_tool_call("lookup", "exec", script),
                responses::ev_completed("first"),
            ]),
            responses::sse(vec![responses::ev_completed("second")]),
        ],
    )
    .await;
    let mut builder = search_capable_apps_builder(server.uri())
        .with_code_mode_host_program(codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?)
        .with_auth(codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_extensions(Arc::new(extensions.build()))
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                r#"
[apps.calendar]
omit_tools_from = ["deferred"]
[apps.calendar.tools.disabled]
enabled = false
"#,
            )
            .expect("write app policy");
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.tool_mode = Some(ToolMode::CodeModeOnly);
            model.supports_search_tool = false;
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodeModeHost)
                .expect("code mode host");
            config
                .features
                .enable(Feature::CodeModeOnlyStrictThirdPartyTools)
                .expect("strict third-party tools");
            config.analytics_enabled = Some(false);
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("Look up my calendar.").await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0]
            .tool_by_name("mcp__codex_apps__calendar", "lookup")
            .is_none()
    );
    let output =
        super::super::code_mode::custom_tool_output_last_non_empty_text(&requests[1], "lookup")
            .expect("exec reports actual discovery and dispatch");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output)?,
        json!({
            "names": ["mcp__codex_apps__calendar__lookup"],
            "result": "called lookup for  at  with "
        }),
        "app omissions are ignored; disabled and app-only tools never enter ALL_TOOLS"
    );
    // The spawned Core session does not inherit the test span.
    let logs = String::from_utf8(
        tracing_test::internal::global_buf()
            .lock()
            .expect("captured logs")
            .clone(),
    )?;
    let thread_id = test.session_configured.thread_id.to_string();
    assert!(logs.lines().any(|line| {
        line.contains(&thread_id) && line.contains("Ignoring MCP/app omit_tools_from")
    }));
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
