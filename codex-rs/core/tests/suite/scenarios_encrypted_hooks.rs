//! Encrypted SessionStart context reaches sampling and survives a cold resume unchanged.

use super::super::rmcp_client::remote_aware_environment_id;
use super::super::rmcp_client::remote_aware_stdio_server_bin;
use super::*;
use core_test_support::hooks::trust_discovered_hooks;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn encrypted_session_start_context_survives_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("first", "Context loaded."),
                ev_completed("first-response"),
            ]),
            sse(vec![
                ev_assistant_message("resumed", "Context is still available."),
                ev_completed("resumed-response"),
            ]),
        ],
    )
    .await;
    // Larger than the normal 2,500-token plaintext spill threshold.
    let ciphertext = "sealed-memory-".repeat(1_000);
    let fixture_ciphertext = ciphertext.clone();
    let command = remote_aware_stdio_server_bin()?;
    let configure = move |config: &mut Config| {
        configure_scenario_catalog(config);
        trust_discovered_hooks(config);
        let mut servers = config.mcp_servers.get().clone();
        servers.insert(
            "memory".to_string(),
            serde_json::from_value::<McpServerConfig>(json!({
                "command": command,
                "env": { "MCP_TEST_ENCRYPTED_OUTPUT": fixture_ciphertext },
                "environment_id": remote_aware_environment_id(),
                "cwd": config.cwd.as_path(),
                "enabled_tools": ["encrypted_output"],
            }))
            .expect("valid memory MCP fixture"),
        );
        config.mcp_servers.set(servers).expect("test MCP servers");
    };
    let test = test_codex()
        .with_model("gpt-6-astra")
        .with_pre_build_hook(|home| {
            fs::write(
                home.join("hooks.json"),
                json!({"hooks": {"SessionStart": [{
                    "matcher": "startup",
                    "hooks": [{
                        "type": "mcp_tool", "server": "memory", "tool": "encrypted_output",
                    }],
                }]}})
                .to_string(),
            )
            .unwrap();
        })
        .with_config(configure.clone())
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Load the context.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let mut hook_entries = Vec::new();
    loop {
        let event = test.codex.next_event().await?;
        match event.msg {
            EventMsg::HookCompleted(completed) => hook_entries.extend(completed.run.entries),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    assert_eq!(
        hook_entries,
        vec![codex_protocol::protocol::HookOutputEntry {
            kind: codex_protocol::protocol::HookOutputEntryKind::Context,
            text: "Lookup completed".to_string(),
        }]
    );
    assert!(
        !std::env::temp_dir()
            .join("hook_outputs")
            .join(test.session_configured.thread_id.to_string())
            .exists()
    );

    let resumed = test_codex()
        .with_model("gpt-6-astra")
        .with_config(configure)
        .restart_with_auto_env(&server, &test)
        .await?;
    resumed.submit_text_turn("Use the context again.").await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        let items = request.input();
        let memory = items
            .iter()
            .position(|item| {
                item["type"] == "function_call_output" && item["name"] == "encrypted_output"
            })
            .expect("standalone encrypted MCP output");
        let user = items
            .iter()
            .position(|item| {
                item["role"] == "user" && item["content"][0]["text"] == "Load the context."
            })
            .expect("first user message");
        assert!(memory < user);
        assert_eq!(items[memory].get("call_id"), None);
        assert_eq!(items[memory]["namespace"], "mcp__memory");
        assert_eq!(
            items[memory]["output"],
            json!([{"type": "encrypted_content", "encrypted_content": ciphertext}]),
        );
        assert_eq!(
            items[memory]["internal_chat_message_metadata_passthrough"]["content_item_metadata"],
            json!([codex_protocol::models::ContentItemMetadata::tool(Some(
                "mcp__memory".to_string().into()
            ))])
        );
        assert_eq!(
            serde_json::to_string(&items)?.matches(&ciphertext).count(),
            1
        );
    }
    insta::assert_snapshot!(
        "encrypted_session_start_context",
        context_snapshot::format_request_history_snapshot(
            "A synchronous MCP SessionStart hook returns a public status and marked memory. The status appears in the hook preview, and the named encrypted tool output reaches sampling before the user message and survives a cold resume.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    resumed.codex.shutdown_and_wait().await?;
    Ok(())
}
