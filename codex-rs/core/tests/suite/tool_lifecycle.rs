use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ResponseItem;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_protocol::models::ContentItem;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

struct RecordedHistory {
    call_id: String,
    arguments: String,
    items: Vec<ResponseItem>,
}

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

    let recorder = Arc::new(ConversationHistoryRecorder {
        histories: Mutex::new(Vec::new()),
    });
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
