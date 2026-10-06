use super::HandleOutputCtx;
use super::TurnItemContributorPolicy;
use super::completed_item_defers_mailbox_delivery_to_next_turn;
use super::finalize_non_tool_response_item;
use super::handle_non_tool_response_item;
use super::handle_output_item_done;
use super::last_assistant_message_from_item;
use super::response_item_may_include_external_context;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::tests::make_session_and_context;
use crate::session::tests::make_session_and_context_with_auth_and_config_and_rx;
use crate::session::tests::tool_registry_for_test_step;
use crate::session::turn_context::TurnContext;
use crate::tools::ToolRouter;
use crate::tools::parallel::ToolCallRuntime;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_extension_api::ExtensionData;
use codex_extension_api::TurnItemContributor;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::ResponseItemId;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::TurnItem;
use codex_protocol::memory_citation::MemoryCitation;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::LocalShellExecAction;
use codex_protocol::models::LocalShellStatus;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

fn assistant_output_text(text: &str) -> ResponseItem {
    assistant_output_text_with_phase(text, /*phase*/ None)
}

fn assistant_output_text_with_phase(text: &str, phase: Option<MessagePhase>) -> ResponseItem {
    ResponseItem::Message {
        id: Some(ResponseItemId::with_suffix("msg", "1")),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase,
        internal_chat_message_metadata_passthrough: None,
    }
}

#[test]
fn external_context_pollution_items_include_web_search_and_tool_search() {
    let polluting_items = [
        ResponseItem::WebSearchCall {
            id: None,
            status: Some("completed".to_string()),
            action: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ToolSearchCall {
            id: None,
            call_id: Some("search-1".to_string()),
            status: None,
            execution: "client".to_string(),
            arguments: serde_json::json!({"query": "calendar"}),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ToolSearchOutput {
            id: None,
            call_id: Some("search-1".to_string()),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: Vec::new(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: None,
            name: Some("notifications".to_string()),
            namespace: Some("slack".to_string()),
            output: FunctionCallOutputPayload::from_text("new message".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    assert!(
        polluting_items
            .iter()
            .all(response_item_may_include_external_context)
    );
}

#[test]
fn external_context_pollution_items_exclude_local_tool_calls() {
    let non_polluting_items = [
        ResponseItem::LocalShellCall {
            id: None,
            call_id: Some("shell-1".to_string()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["cat".to_string(), "README.md".to_string()],
                timeout_ms: None,
                working_directory: None,
                env: None,
                user: None,
            }),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "shell".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
            encrypted_function_args: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-1".to_string()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "custom-1".to_string(),
            name: "apply_patch".to_string(),
            namespace: None,
            input: "*** Begin Patch\n*** End Patch\n".to_string(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::CustomToolCallOutput {
            id: None,
            call_id: "custom-1".to_string(),
            name: Some("apply_patch".to_string()),
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
        assistant_output_text("plain assistant text"),
    ];

    assert!(
        !non_polluting_items
            .iter()
            .any(response_item_may_include_external_context)
    );
}

#[tokio::test]
async fn handle_non_tool_response_item_strips_citations_from_assistant_message() {
    let (session, _) = make_session_and_context().await;
    let item = assistant_output_text(
        "hello<oai-mem-citation><citation_entries>\nMEMORY.md:1-2|note=[x]\n</citation_entries>\n<rollout_ids>\n019cc2ea-1dff-7902-8d40-c8f6e5d83cc4\n</rollout_ids></oai-mem-citation> world",
    );

    let turn_item = handle_non_tool_response_item(
        &session,
        TurnItemContributorPolicy::Skip,
        &item,
        /*plan_mode*/ false,
    )
    .await
    .expect("assistant message should parse");

    let TurnItem::AgentMessage(agent_message) = turn_item else {
        panic!("expected agent message");
    };
    let text = agent_message
        .content
        .iter()
        .map(|entry| match entry {
            codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
        })
        .collect::<String>();
    assert_eq!(text, "hello world");
    let memory_citation = agent_message
        .memory_citation
        .expect("memory citation should be parsed");
    assert_eq!(memory_citation.entries.len(), 1);
    assert_eq!(memory_citation.entries[0].path, "MEMORY.md");
    assert_eq!(
        memory_citation.rollout_ids,
        vec!["019cc2ea-1dff-7902-8d40-c8f6e5d83cc4".to_string()]
    );
}

struct TestTurnItemContributor;

#[derive(Debug)]
struct TurnItemContributorRan;

impl TurnItemContributor for TestTurnItemContributor {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        turn_store: &'a ExtensionData,
        item: &'a mut TurnItem,
    ) -> codex_extension_api::ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            turn_store.insert(TurnItemContributorRan);
            if let TurnItem::AgentMessage(agent_message) = item {
                agent_message.memory_citation = Some(MemoryCitation {
                    entries: Vec::new(),
                    rollout_ids: Vec::new(),
                });
            }
            Ok(())
        })
    }
}

struct RewriteAgentMessageContributor;

impl TurnItemContributor for RewriteAgentMessageContributor {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        item: &'a mut TurnItem,
    ) -> codex_extension_api::ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if let TurnItem::AgentMessage(agent_message) = item {
                agent_message.content = vec![AgentMessageContent::Text {
                    text: "contributed assistant text".to_string(),
                }];
            }
            Ok(())
        })
    }
}

#[tokio::test]
async fn handle_non_tool_response_item_runs_turn_item_contributors_only_when_requested() {
    let (mut session, turn_context) = make_session_and_context().await;
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_item_contributor(Arc::new(TestTurnItemContributor));
    session.services.extensions = Arc::new(builder.build());
    let turn_store = ExtensionData::new(turn_context.sub_id.clone());
    let item = assistant_output_text(
        "hello<oai-mem-citation>ignored by memory parser</oai-mem-citation> world",
    );

    let provisional_turn_item = handle_non_tool_response_item(
        &session,
        TurnItemContributorPolicy::Skip,
        &item,
        /*plan_mode*/ false,
    )
    .await
    .expect("assistant message should parse");

    assert!(turn_store.get::<TurnItemContributorRan>().is_none());
    let TurnItem::AgentMessage(provisional_agent_message) = provisional_turn_item else {
        panic!("expected agent message");
    };
    assert_eq!(provisional_agent_message.memory_citation, None);

    let turn_item = handle_non_tool_response_item(
        &session,
        TurnItemContributorPolicy::Run(&turn_store),
        &item,
        /*plan_mode*/ false,
    )
    .await
    .expect("assistant message should parse");

    assert!(turn_store.get::<TurnItemContributorRan>().is_some());
    let TurnItem::AgentMessage(agent_message) = turn_item else {
        panic!("expected agent message");
    };
    assert!(agent_message.memory_citation.is_some());
    let text = agent_message
        .content
        .iter()
        .map(|entry| match entry {
            codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
        })
        .collect::<String>();
    assert_eq!(text, "hello world");
}

fn output_context(session: Arc<Session>, turn_context: Arc<TurnContext>) -> HandleOutputCtx {
    let step_context = StepContext::for_test(Arc::clone(&turn_context));
    let (registry, hosted_specs) = tool_registry_for_test_step(step_context.as_ref());
    let router = Arc::new(ToolRouter::from_registry(
        step_context.turn.as_ref(),
        step_context.turn.model_info(),
        registry,
        hosted_specs,
        &Default::default(),
    ));
    let step_context = step_context.with_tool_router_for_test(router);
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
    let tool_runtime =
        ToolCallRuntime::new(Arc::clone(&session), Arc::clone(&step_context), tracker);
    HandleOutputCtx {
        sess: session,
        step_context,
        turn_store: Arc::new(ExtensionData::new(turn_context.sub_id.clone())),
        tool_runtime,
        cancellation_token: CancellationToken::new(),
    }
}

#[tokio::test]
async fn handle_output_item_done_returns_contributed_last_agent_message() {
    let (mut session, turn_context) = make_session_and_context().await;
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_item_contributor(Arc::new(RewriteAgentMessageContributor));
    session.services.extensions = Arc::new(builder.build());
    let mut ctx = output_context(Arc::new(session), Arc::new(turn_context));
    let item = assistant_output_text("original assistant text");

    let output = handle_output_item_done(&mut ctx, item, /*previously_active_item*/ None)
        .await
        .expect("assistant message should complete");

    assert_eq!(
        output.last_agent_message.as_deref(),
        Some("contributed assistant text")
    );
}

#[tokio::test]
async fn direct_results_keep_their_own_records_when_call_ids_repeat() {
    let (session, turn_context, _events) = make_session_and_context_with_auth_and_config_and_rx(
        CodexAuth::from_api_key("Test API Key"),
        Vec::new(),
        |config| {
            config
                .features
                .enable(Feature::ExecutedToolCallMetadata)
                .expect("enable metadata");
            config.update_plan_enabled = true;
        },
    )
    .await;
    let mut ctx = output_context(Arc::clone(&session), turn_context);
    let mut pending = Vec::new();
    for step in ["read", "write"] {
        let arguments = json!({"plan": [{"step": step, "status": "in_progress"}]});
        let call = serde_json::from_value(json!({
            "type": "function_call", "call_id": "reused", "name": "update_plan",
            "arguments": arguments.to_string(),
        }))
        .expect("direct input");
        let result = handle_output_item_done(&mut ctx, call, /*previously_active_item*/ None)
            .await
            .expect("direct call should be handled");
        assert!(result.needs_follow_up);
        pending.push((
            arguments,
            result.tool_future.expect("dispatched direct call"),
        ));
    }

    // Reapplying an enabled config keeps the same recording lifetime.
    let current_config = session.get_config().await;
    let config = current_config.as_ref().clone();
    assert_eq!(
        session.refresh_runtime_config(current_config, config).await,
        crate::ConfigRefreshOutcome::Published
    );
    // Await in reverse order: each result must already own its record before history attachment.
    let mut outputs = Vec::new();
    for (arguments, future) in pending.into_iter().rev() {
        let envelope = future.await.expect("plan result");
        let item = &envelope.item;
        let ResponseItem::FunctionCallOutput {
            call_id: output_call_id,
            output,
            ..
        } = item
        else {
            panic!("expected plan output");
        };
        assert_eq!(output_call_id.as_deref(), Some("reused"));
        assert_eq!(output.text_content(), Some("Plan updated"));
        let metadata = item
            .executed_tool_call_metadata()
            .expect("recorded direct call");
        assert_eq!(metadata.tool_calls_complete, Some(true));
        assert_eq!(
            serde_json::to_value(&metadata.executed_tool_calls).expect("call metadata"),
            json!([{
                "name": "update_plan", "arguments": arguments,
            }])
        );
        outputs.push(envelope.item);
    }
    let original = outputs.clone();
    session
        .services
        .executed_tool_calls
        .attach_to_prompt(&mut outputs, &mut Default::default());
    assert_eq!(outputs, original);

    let mut pending = Vec::new();
    for call_id in ["disabled", "re-enabled"] {
        let call = serde_json::from_value(json!({
            "type": "function_call", "call_id": call_id, "name": "update_plan",
            "arguments": "{\"plan\": []}",
        }))
        .expect("direct input");
        pending.push(
            handle_output_item_done(&mut ctx, call, /*previously_active_item*/ None)
                .await
                .expect("direct call should be handled")
                .tool_future
                .expect("dispatched direct call"),
        );
    }
    let current_config = session.get_config().await;
    let mut config = current_config.as_ref().clone();
    config
        .features
        .disable(Feature::ExecutedToolCallMetadata)
        .expect("disable metadata");
    let _ = session
        .refresh_runtime_config(current_config, config.clone())
        .await;
    let result = pending
        .remove(0)
        .await
        .expect("plan result after capture disabled");
    assert!(result.item.executed_tool_call_metadata().is_none());
    session
        .services
        .executed_tool_calls
        .attach_to_prompt(&mut outputs, &mut Default::default());
    let mut expected = original;
    for item in &mut expected {
        item.clear_executed_tool_calls();
    }
    assert_eq!(outputs, expected);

    config
        .features
        .enable(Feature::ExecutedToolCallMetadata)
        .expect("re-enable metadata");
    let current_config = session.get_config().await;
    let _ = session.refresh_runtime_config(current_config, config).await;
    let result = pending
        .pop()
        .expect("call prepared before disable")
        .await
        .expect("plan result after capture re-enabled");
    assert!(result.item.executed_tool_call_metadata().is_none());
}

#[tokio::test]
async fn finalized_turn_item_defers_mailbox_for_contributed_visible_text() {
    let (mut session, turn_context) = make_session_and_context().await;
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_item_contributor(Arc::new(RewriteAgentMessageContributor));
    session.services.extensions = Arc::new(builder.build());
    let turn_store = ExtensionData::new(turn_context.sub_id.clone());
    let item = assistant_output_text("<oai-mem-citation>hidden only</oai-mem-citation>");

    let finalized = finalize_non_tool_response_item(
        &session,
        TurnItemContributorPolicy::Run(&turn_store),
        &item,
        /*plan_mode*/ false,
    )
    .await
    .expect("assistant message should parse");

    assert_eq!(
        finalized.facts.last_agent_message.as_deref(),
        Some("contributed assistant text")
    );
    assert!(finalized.facts.defers_mailbox_delivery_to_next_turn);
}

#[test_case::test_case(MessagePhase::Commentary; "commentary")]
#[test_case::test_case(MessagePhase::PartialAnswer; "partial_answer")]
#[tokio::test]
async fn finalized_turn_item_keeps_mailbox_open_for_nonterminal_text(phase: MessagePhase) {
    let (mut session, turn_context) = make_session_and_context().await;
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_item_contributor(Arc::new(RewriteAgentMessageContributor));
    session.services.extensions = Arc::new(builder.build());
    let turn_store = ExtensionData::new(turn_context.sub_id.clone());
    let item = assistant_output_text_with_phase("still working", Some(phase));

    let finalized = finalize_non_tool_response_item(
        &session,
        TurnItemContributorPolicy::Run(&turn_store),
        &item,
        /*plan_mode*/ false,
    )
    .await
    .expect("assistant message should parse");

    assert_eq!(
        finalized.facts.last_agent_message.as_deref(),
        Some("contributed assistant text")
    );
    assert!(!finalized.facts.defers_mailbox_delivery_to_next_turn);
}

#[test]
fn last_assistant_message_from_item_strips_citations_and_plan_blocks() {
    let item = assistant_output_text(
        "before<oai-mem-citation>doc1</oai-mem-citation>\n<proposed_plan>\n- x\n</proposed_plan>\nafter",
    );

    let message = last_assistant_message_from_item(&item, /*plan_mode*/ true)
        .expect("assistant text should remain after stripping");

    assert_eq!(message, "before\nafter");
}

#[test]
fn last_assistant_message_from_item_returns_none_for_citation_only_message() {
    let item = assistant_output_text("<oai-mem-citation>doc1</oai-mem-citation>");

    assert_eq!(
        last_assistant_message_from_item(&item, /*plan_mode*/ false),
        None
    );
}

#[test]
fn last_assistant_message_from_item_returns_none_for_plan_only_hidden_message() {
    let item = assistant_output_text("<proposed_plan>\n- x\n</proposed_plan>");

    assert_eq!(
        last_assistant_message_from_item(&item, /*plan_mode*/ true),
        None
    );
}

#[test]
fn completed_item_defers_mailbox_delivery_for_unknown_phase_messages() {
    let item = assistant_output_text("final answer");

    assert!(completed_item_defers_mailbox_delivery_to_next_turn(
        &item, /*plan_mode*/ false,
    ));
}

#[test_case::test_case(MessagePhase::Commentary; "commentary")]
#[test_case::test_case(MessagePhase::PartialAnswer; "partial_answer")]
fn completed_item_keeps_mailbox_delivery_open_for_nonterminal_messages(phase: MessagePhase) {
    let item = assistant_output_text_with_phase("still working", Some(phase));

    assert!(!completed_item_defers_mailbox_delivery_to_next_turn(
        &item, /*plan_mode*/ false,
    ));
}
