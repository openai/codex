use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context;
use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ResponseItem;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_protocol::models::ContentItem;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

struct RecordedHistory {
    call_id: String,
    arguments: String,
    items: Vec<ResponseItem>,
}

#[derive(Default)]
struct ConversationHistoryRecorder {
    histories: Mutex<Vec<RecordedHistory>>,
}

impl ToolLifecycleContributor for ConversationHistoryRecorder {
    fn on_tool_start<'a>(&'a self, input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            self.histories
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(RecordedHistory {
                    call_id: input.call_id.to_owned(),
                    arguments: input.payload.log_payload().into_owned(),
                    items: input.conversation_history.items().cloned().collect(),
                });
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_start_receives_conversation_history() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let first_call_id = "first-plan-call";
    let second_call_id = "second-plan-call";
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_reasoning_item("reasoning-1", &["inspect the workspace"], &[]),
                responses::ev_function_call(
                    first_call_id,
                    "update_plan",
                    &json!({
                        "plan": [{ "step": "Inspect workspace", "status": "in_progress" }]
                    })
                    .to_string(),
                ),
                responses::ev_completed("first-response"),
            ]),
            responses::sse(vec![
                responses::ev_function_call(
                    second_call_id,
                    "update_plan",
                    &json!({
                        "plan": [{ "step": "Inspect workspace", "status": "completed" }]
                    })
                    .to_string(),
                ),
                responses::ev_completed("second-response"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("assistant-1", "done"),
                responses::ev_completed("third-response"),
            ]),
        ],
    )
    .await;

    let recorder = Arc::new(ConversationHistoryRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(recorder.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;

    let user_prompt = "Inspect the workspace and update the plan.";
    test.submit_text_turn(user_prompt).await?;

    let histories = recorder
        .histories
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let call_ids = histories
        .iter()
        .map(|history| history.call_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(call_ids, vec![first_call_id, second_call_id]);
    let arguments = histories
        .iter()
        .map(|history| serde_json::from_str::<serde_json::Value>(&history.arguments))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        arguments,
        vec![
            json!({ "plan": [{ "step": "Inspect workspace", "status": "in_progress" }] }),
            json!({ "plan": [{ "step": "Inspect workspace", "status": "completed" }] }),
        ]
    );

    let first_history = &histories[0].items;
    assert!(first_history.iter().any(|item| matches!(
        item,
        ResponseItem::Message { role, content, .. }
            if role == "user"
                && content.iter().any(|content| matches!(
                    content,
                    ContentItem::InputText { text } if text == user_prompt
                ))
    )));
    assert!(
        first_history
            .iter()
            .any(|item| matches!(item, ResponseItem::Reasoning { .. }))
    );
    assert!(first_history.iter().any(|item| matches!(
        item,
        ResponseItem::FunctionCall { call_id, .. } if call_id == first_call_id
    )));

    let second_history = &histories[1].items;
    assert!(second_history.iter().any(|item| matches!(
        item,
        ResponseItem::FunctionCallOutput { call_id, .. } if call_id == first_call_id
    )));
    assert!(second_history.iter().any(|item| matches!(
        item,
        ResponseItem::FunctionCall { call_id, .. } if call_id == second_call_id
    )));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_start_receives_rewritten_payload_and_post_hook_history() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "command hooks require a host-native executor");

    let server = responses::start_mock_server().await;
    let call_id = "rewritten-plan-call";
    let original_input = json!({
        "plan": [{ "step": "Original step", "status": "in_progress" }]
    });
    let rewritten_input = json!({
        "plan": [{ "step": "Rewritten step", "status": "completed" }]
    });
    let additional_context = "Only available after the pre-tool hook.";
    responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call(call_id, "update_plan", &original_input.to_string()),
                responses::ev_completed("first-response"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("assistant-1", "done"),
                responses::ev_completed("second-response"),
            ]),
        ],
    )
    .await;

    let hook_output = json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": rewritten_input,
            "additionalContext": additional_context,
        }
    });
    let recorder = Arc::new(ConversationHistoryRecorder::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(recorder.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_pre_build_hook(move |home| {
            write_pre_tool_hook(home, "^update_plan$", &hook_output)
                .expect("write pre-tool hook fixture");
        })
        .with_config(trust_discovered_hooks)
        .build_with_auto_env(&server)
        .await?;

    test.submit_text_turn("Update the plan.").await?;

    let histories = recorder
        .histories
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let [history] = histories.as_slice() else {
        panic!("expected one tool start, got {}", histories.len());
    };
    assert_eq!(
        (
            history.call_id.as_str(),
            serde_json::from_str::<serde_json::Value>(&history.arguments)?,
        ),
        (call_id, rewritten_input)
    );
    assert!(history.items.iter().any(|item| matches!(
        item,
        ResponseItem::Message { role, content, .. }
            if role == "developer"
                && content.iter().any(|content| matches!(
                    content,
                    ContentItem::InputText { text } if text == additional_context
                ))
    )));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_start_is_not_called_when_pre_tool_hook_prevents_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "command hooks require a host-native executor");

    for (tool_name, matcher, arguments, hook_output) in [
        (
            "update_plan",
            "^update_plan$",
            json!({ "plan": [{ "step": "Blocked step", "status": "in_progress" }] }),
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": "blocked by lifecycle test",
                }
            }),
        ),
        (
            "shell_command",
            "^Bash$",
            json!({ "command": "echo original" }),
            json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "updatedInput": { "command": 123 },
                }
            }),
        ),
    ] {
        let server = responses::start_mock_server().await;
        let call_id = format!("prevented-{tool_name}-call");
        responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_function_call(&call_id, tool_name, &arguments.to_string()),
                    responses::ev_completed("first-response"),
                ]),
                responses::sse(vec![
                    responses::ev_assistant_message("assistant-1", "done"),
                    responses::ev_completed("second-response"),
                ]),
            ],
        )
        .await;

        let recorder = Arc::new(ConversationHistoryRecorder::default());
        let mut extensions = ExtensionRegistryBuilder::<Config>::new();
        extensions.tool_lifecycle_contributor(recorder.clone());
        let test = test_codex()
            .with_extensions(Arc::new(extensions.build()))
            .with_pre_build_hook(move |home| {
                write_pre_tool_hook(home, matcher, &hook_output)
                    .expect("write pre-tool hook fixture");
            })
            .with_config(trust_discovered_hooks)
            .build_with_auto_env(&server)
            .await?;

        test.submit_text_turn("Run the tool.").await?;

        assert!(
            recorder
                .histories
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty(),
            "tool start should not run for {tool_name}"
        );
    }

    Ok(())
}

fn write_pre_tool_hook(home: &Path, matcher: &str, output: &serde_json::Value) -> Result<()> {
    let script_path = home.join("tool_lifecycle_hook.py");
    let output_json = serde_json::to_string(output).context("serialize pre-tool hook output")?;
    fs::write(
        &script_path,
        format!("import json\nimport sys\njson.load(sys.stdin)\nprint({output_json:?})\n"),
    )
    .context("write pre-tool hook script")?;
    let hooks = json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": matcher,
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                }]
            }]
        }
    });
    fs::write(home.join("hooks.json"), hooks.to_string()).context("write pre-tool hooks.json")?;

    Ok(())
}
