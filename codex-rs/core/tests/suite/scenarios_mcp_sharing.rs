//! Shared MCP transports preserve each chat's model-visible tools and input responses.

use super::*;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::responses;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_mcp_keeps_tools_and_model_requested_input_per_chat() -> Result<()> {
    skip_if_no_network!(Ok(()));
    core_test_support::skip_if_remote!(Ok(()), "the shared stdio fixture runs on the host");
    skip_if_wine_exec!(Ok(()), "requires a Windows test_stdio_server binary");

    let first_server = start_mock_server().await;
    let second_server = start_mock_server().await;
    let command = stdio_server_bin()?;
    let test = test_codex()
        .with_config(move |config| {
            super::configure_scenario_catalog(config);
            config.approvals_reviewer = ApprovalsReviewer::User;
            config
                .features
                .enable(Feature::McpConnectionSharing)
                .expect("enable sharing");
            config
                .features
                .disable(Feature::Mcp20260728)
                .expect("let sharing select the protocol");
            config
                .mcp_servers
                .set(
                    serde_json::from_value(json!({
                        "shared": {
                            "command": command,
                            "enabled_tools": ["echo"],
                            "default_tools_approval_mode": "approve",
                            "env": {
                                "MCP_TEST_ECHO_INPUT": "1",
                            },
                        },
                    }))
                    .expect("shared MCP config"),
                )
                .expect("set MCP config");
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.tool_mode = Some(ToolMode::Direct);
            model.supports_search_tool = false;
            model.use_responses_lite = false;
        })
        .build_with_auto_env(&first_server)
        .await?;
    wait_for_mcp_server(&test.codex, "shared").await?;

    let mut options = test.start_thread_options().await;
    options.config.model_provider.base_url = Some(format!("{}/v1", second_server.uri()));
    let mut servers = options.config.mcp_servers.get().clone();
    servers
        .get_mut("shared")
        .expect("shared server")
        .enabled_tools = Some(vec!["echo-tool".to_string()]);
    options.config.mcp_servers.set(servers)?;
    let second = test.thread_manager.start_thread(options).await?.thread;
    wait_for_mcp_server(&second, "shared").await?;

    let chats = [
        (
            &test.codex,
            &first_server,
            "echo",
            "First task owner?",
            "Mira",
        ),
        (
            &second,
            &second_server,
            "echo_tool",
            "Second task owner?",
            "Sora",
        ),
    ];
    let mut mocks = Vec::new();
    for (thread, server, tool, question, _) in chats {
        mocks.push(
            mount_sse_sequence(
                server,
                vec![
                    sse(vec![
                        ev_response_created("ask-owner"),
                        ev_function_call_with_namespace(
                            "ask-owner",
                            "mcp__shared",
                            tool,
                            &json!({ "message": question }).to_string(),
                        ),
                        ev_completed("ask-owner"),
                    ]),
                    responses::sse_completed("done"),
                ],
            )
            .await,
        );
        thread
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: format!("Ask the user: {question}"),
                    text_elements: Vec::new(),
                }])
                .with_thread_settings(ThreadSettingsOverrides {
                    approval_policy: Some(AskForApproval::OnRequest),
                    permission_profile: Some(PermissionProfile::read_only()),
                    ..Default::default()
                }),
            )
            .await?;
    }

    let mut inputs = Vec::new();
    let mut processes = Vec::new();
    for (thread, _, _, question, _) in chats {
        let input = wait_for_event_match(thread, |event| match event {
            EventMsg::ElicitationRequest(input) => Some(input.clone()),
            _ => None,
        })
        .await;
        let ElicitationRequest::Form { meta, message, .. } = &input.request else {
            panic!("expected a form input");
        };
        assert_eq!(message, question);
        processes.push(meta.as_ref().expect("fixture metadata")["fixture/processId"].clone());
        inputs.push(input);
    }
    assert!(processes[0].is_u64());
    assert_eq!(
        processes[0], processes[1],
        "the chats must share one process"
    );

    // Both calls use the same server input key; resolve the joiner's input first.
    for ((thread, _, _, _, answer), input) in chats.into_iter().zip(inputs).rev() {
        thread
            .submit(Op::ResolveElicitation {
                server_name: input.server_name,
                request_id: input.id,
                decision: ElicitationAction::Accept,
                content: Some(json!({ "answer": answer })),
                meta: None,
            })
            .await?;
    }

    let mut histories = Vec::new();
    for ((thread, ..), mock) in chats.into_iter().zip(mocks) {
        wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        histories.push(mock.requests());
    }
    let entries = histories
        .iter()
        .zip(["creator", "joiner"])
        .flat_map(|(requests, label)| {
            requests
                .iter()
                .map(move |request| SnapshotEntry::captured(request).labeled(label))
        })
        .collect::<Vec<_>>();
    insta::assert_snapshot!(context_snapshot::format_context_snapshot(
        "Two chats share one MCP process, expose different tools, and answer simultaneous model-requested questions in reverse order.",
        &entries,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    ));
    Ok(())
}
