use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use crate::model::AgentOrigin;
use crate::model::ExecutionStatus;
use crate::model::InteractionEdgeKind;
use crate::model::RolloutStatus;
use crate::model::ToolCallKind;
use crate::model::ToolCallSummary;
use crate::model::TraceAnchor;
use crate::payload::RawPayloadKind;
use crate::raw_event::RawToolCallRequester;
use crate::raw_event::RawTraceEventPayload;
use crate::reducer::test_support::append_completed_inference;
use crate::reducer::test_support::append_inference_request;
use crate::reducer::test_support::create_started_agent_writer;
use crate::reducer::test_support::message;
use crate::reducer::test_support::start_agent_turn;
use crate::reducer::test_support::start_thread;
use crate::reducer::test_support::start_turn_for_thread;
use crate::reducer::test_support::trace_context_for_agent;
use crate::reducer::test_support::trace_context_for_thread;
use crate::replay_bundle;
use crate::writer::TraceWriter;

#[test]
fn child_thread_metadata_creates_spawn_origin_without_delivery_edge() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = TraceWriter::create(
        temp.path(),
        "trace-1".to_string(),
        "rollout-1".to_string(),
        "019d0000-0000-7000-8000-000000000002".to_string(),
    )?;
    let metadata = writer.write_json_payload(
        RawPayloadKind::SessionMetadata,
        &json!({
            "nickname": "James",
            "agent_role": "explorer",
            "task_name": "repo_file_counter",
            "model": "gpt-test",
            "session_source": {
                "subagent": {
                    "thread_spawn": {
                        "parent_thread_id": "019d0000-0000-7000-8000-000000000001",
                        "agent_path": "/root/repo_file_counter",
                        "agent_nickname": "James",
                        "agent_role": "explorer"
                    }
                }
            }
        }),
    )?;
    writer.append(RawTraceEventPayload::ThreadStarted {
        thread_id: "019d0000-0000-7000-8000-000000000002".to_string(),
        agent_path: "/root/repo_file_counter".to_string(),
        metadata_payload: Some(metadata),
    })?;

    let replayed = replay_bundle(temp.path())?;
    let thread = &replayed.threads["019d0000-0000-7000-8000-000000000002"];
    assert_eq!(thread.nickname, Some("James".to_string()));
    assert_eq!(thread.default_model, Some("gpt-test".to_string()));
    assert_eq!(
        thread.origin,
        AgentOrigin::Spawned {
            parent_thread_id: "019d0000-0000-7000-8000-000000000001".to_string(),
            spawn_edge_id: "edge:spawn:019d0000-0000-7000-8000-000000000001:019d0000-0000-7000-8000-000000000002".to_string(),
            task_name: "repo_file_counter".to_string(),
            agent_role: "explorer".to_string(),
        }
    );
    assert!(
        !replayed.interaction_edges.contains_key(
            "edge:spawn:019d0000-0000-7000-8000-000000000001:019d0000-0000-7000-8000-000000000002"
        ),
        "spawn metadata identifies the child, but the delivery edge waits for the recipient \
         conversation item"
    );

    Ok(())
}

#[test]
fn spawn_runtime_payload_targets_delivered_child_message() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_agent_writer(&temp)?;
    start_agent_turn(&writer, "turn-1")?;
    let invocation_payload = writer.write_json_payload(
        RawPayloadKind::ToolInvocation,
        &json!({
            "tool_name": "spawn_agent",
            "payload": {
                "type": "function",
                "arguments": "{\"task_name\":\"repo_file_counter\",\"message\":\"count\"}"
            }
        }),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: "call-spawn".to_string(),
            model_visible_call_id: Some("call-spawn".to_string()),
            code_mode_runtime_tool_id: None,
            requester: RawToolCallRequester::Model,
            kind: ToolCallKind::SpawnAgent,
            summary: ToolCallSummary::Generic {
                label: "spawn_agent".to_string(),
                input_preview: None,
                output_preview: None,
            },
            invocation_payload: Some(invocation_payload.clone()),
        },
    )?;
    let begin_payload = writer.write_json_payload(
        RawPayloadKind::ToolRuntimeEvent,
        &json!({
            "call_id": "call-spawn",
            "sender_thread_id": "019d0000-0000-7000-8000-000000000001",
            "prompt": "count"
        }),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallRuntimeStarted {
            tool_call_id: "call-spawn".to_string(),
            runtime_payload: begin_payload.clone(),
        },
    )?;
    let end_payload = writer.write_json_payload(
        RawPayloadKind::ToolRuntimeEvent,
        &json!({
            "call_id": "call-spawn",
            "sender_thread_id": "019d0000-0000-7000-8000-000000000001",
            "new_thread_id": "019d0000-0000-7000-8000-000000000002",
            "prompt": "count",
            "model": "gpt-test",
            "reasoning_effort": "medium",
            "status": "running"
        }),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallRuntimeEnded {
            tool_call_id: "call-spawn".to_string(),
            status: ExecutionStatus::Completed,
            runtime_payload: end_payload.clone(),
        },
    )?;
    let result_payload = writer.write_json_payload(
        RawPayloadKind::ToolResult,
        &json!({"task_name": "/root/repo_file_counter"}),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallEnded {
            tool_call_id: "call-spawn".to_string(),
            status: ExecutionStatus::Completed,
            result_payload: Some(result_payload.clone()),
        },
    )?;
    start_thread(
        &writer,
        "019d0000-0000-7000-8000-000000000002",
        "/root/repo_file_counter",
    )?;
    start_turn_for_thread(
        &writer,
        "019d0000-0000-7000-8000-000000000002",
        "turn-child-1",
    )?;
    let delivered = inter_agent_message(
        "/root",
        "/root/repo_file_counter",
        "count",
        /*trigger_turn*/ true,
    );
    append_inference_request(
        &writer,
        "019d0000-0000-7000-8000-000000000002",
        "turn-child-1",
        "inference-child-1",
        vec![message("assistant", &delivered)],
    )?;

    let replayed = replay_bundle(temp.path())?;
    let edge = &replayed.interaction_edges["edge:spawn:019d0000-0000-7000-8000-000000000001:019d0000-0000-7000-8000-000000000002"];
    assert_eq!(edge.kind, InteractionEdgeKind::SpawnAgent);
    assert_eq!(
        edge.source,
        TraceAnchor::ToolCall {
            tool_call_id: "call-spawn".to_string()
        }
    );
    let target_item_id = target_conversation_item_id(&edge.target);
    assert_eq!(edge.carried_item_ids, vec![target_item_id.clone()]);
    assert_eq!(
        replayed.conversation_items[target_item_id].thread_id,
        "019d0000-0000-7000-8000-000000000002"
    );
    assert_eq!(
        edge.carried_raw_payload_ids,
        vec![
            invocation_payload.raw_payload_id,
            begin_payload.raw_payload_id,
            end_payload.raw_payload_id,
            result_payload.raw_payload_id,
        ]
    );

    Ok(())
}

#[test]
fn send_message_runtime_payload_targets_delivered_child_message() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_agent_writer(&temp)?;
    start_agent_turn(&writer, "turn-1")?;
    let invocation_payload = writer.write_json_payload(
        RawPayloadKind::ToolInvocation,
        &json!({
            "tool_name": "send_message",
            "payload": {
                "type": "function",
                "arguments": "{\"target\":\"/root/child\",\"message\":\"hello\"}"
            }
        }),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: "call-send".to_string(),
            model_visible_call_id: Some("call-send".to_string()),
            code_mode_runtime_tool_id: None,
            requester: RawToolCallRequester::Model,
            kind: ToolCallKind::SendMessage,
            summary: ToolCallSummary::Generic {
                label: "send_message".to_string(),
                input_preview: None,
                output_preview: None,
            },
            invocation_payload: Some(invocation_payload),
        },
    )?;
    let begin_payload = writer.write_json_payload(
        RawPayloadKind::ToolRuntimeEvent,
        &json!({
            "call_id": "call-send",
            "sender_thread_id": "019d0000-0000-7000-8000-000000000001",
            "receiver_thread_id": "019d0000-0000-7000-8000-000000000002",
            "prompt": "hello",
            "status": "running"
        }),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallRuntimeStarted {
            tool_call_id: "call-send".to_string(),
            runtime_payload: begin_payload,
        },
    )?;
    let end_payload = writer.write_json_payload(
        RawPayloadKind::ToolRuntimeEvent,
        &json!({
            "call_id": "call-send",
            "sender_thread_id": "019d0000-0000-7000-8000-000000000001",
            "receiver_thread_id": "019d0000-0000-7000-8000-000000000002",
            "prompt": "hello",
            "status": "running"
        }),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallRuntimeEnded {
            tool_call_id: "call-send".to_string(),
            status: ExecutionStatus::Completed,
            runtime_payload: end_payload,
        },
    )?;
    start_thread(
        &writer,
        "019d0000-0000-7000-8000-000000000002",
        "/root/child",
    )?;
    start_turn_for_thread(
        &writer,
        "019d0000-0000-7000-8000-000000000002",
        "turn-child-1",
    )?;
    let delivered =
        inter_agent_message("/root", "/root/child", "hello", /*trigger_turn*/ false);
    append_inference_request(
        &writer,
        "019d0000-0000-7000-8000-000000000002",
        "turn-child-1",
        "inference-child-1",
        vec![message("assistant", &delivered)],
    )?;

    let replayed = replay_bundle(temp.path())?;
    let edge = &replayed.interaction_edges["edge:tool:call-send"];
    assert_eq!(edge.kind, InteractionEdgeKind::SendMessage);
    assert_eq!(
        edge.source,
        TraceAnchor::ToolCall {
            tool_call_id: "call-send".to_string()
        }
    );
    let target_item_id = target_conversation_item_id(&edge.target);
    assert_eq!(edge.carried_item_ids, vec![target_item_id.clone()]);
    assert_eq!(
        replayed.conversation_items[target_item_id].thread_id,
        "019d0000-0000-7000-8000-000000000002"
    );
    assert!(edge.ended_at_unix_ms.is_some());

    Ok(())
}

#[test]
fn close_agent_runtime_payload_targets_thread() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_agent_writer(&temp)?;
    start_thread(
        &writer,
        "019d0000-0000-7000-8000-000000000002",
        "/root/child",
    )?;
    start_agent_turn(&writer, "turn-1")?;
    let invocation_payload = writer.write_json_payload(
        RawPayloadKind::ToolInvocation,
        &json!({
            "tool_name": "close_agent",
            "payload": {
                "type": "function",
                "arguments": r#"{"target":"/root/child"}"#
            }
        }),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallStarted {
            tool_call_id: "call-close".to_string(),
            model_visible_call_id: Some("call-close".to_string()),
            code_mode_runtime_tool_id: None,
            requester: RawToolCallRequester::Model,
            kind: ToolCallKind::CloseAgent,
            summary: ToolCallSummary::Generic {
                label: "close_agent".to_string(),
                input_preview: None,
                output_preview: None,
            },
            invocation_payload: Some(invocation_payload.clone()),
        },
    )?;
    let begin_payload = writer.write_json_payload(
        RawPayloadKind::ToolRuntimeEvent,
        &json!({
            "call_id": "call-close",
            "sender_thread_id": "019d0000-0000-7000-8000-000000000001",
            "receiver_thread_id": "019d0000-0000-7000-8000-000000000002"
        }),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallRuntimeStarted {
            tool_call_id: "call-close".to_string(),
            runtime_payload: begin_payload.clone(),
        },
    )?;
    let end_payload = writer.write_json_payload(
        RawPayloadKind::ToolRuntimeEvent,
        &json!({
            "call_id": "call-close",
            "sender_thread_id": "019d0000-0000-7000-8000-000000000001",
            "receiver_thread_id": "019d0000-0000-7000-8000-000000000002",
            "receiver_agent_nickname": "Scout",
            "receiver_agent_role": "explorer",
            "status": "running"
        }),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallRuntimeEnded {
            tool_call_id: "call-close".to_string(),
            status: ExecutionStatus::Completed,
            runtime_payload: end_payload.clone(),
        },
    )?;
    let result_payload = writer.write_json_payload(
        RawPayloadKind::ToolResult,
        &json!({"previous_status": "running"}),
    )?;
    writer.append_with_context(
        trace_context_for_agent("turn-1"),
        RawTraceEventPayload::ToolCallEnded {
            tool_call_id: "call-close".to_string(),
            status: ExecutionStatus::Completed,
            result_payload: Some(result_payload.clone()),
        },
    )?;
    writer.append(RawTraceEventPayload::ThreadEnded {
        thread_id: "019d0000-0000-7000-8000-000000000002".to_string(),
        status: RolloutStatus::Completed,
    })?;

    let replayed = replay_bundle(temp.path())?;
    let edge = &replayed.interaction_edges["edge:tool:call-close"];
    assert_eq!(edge.kind, InteractionEdgeKind::CloseAgent);
    assert_eq!(
        edge.source,
        TraceAnchor::ToolCall {
            tool_call_id: "call-close".to_string()
        }
    );
    assert_eq!(
        edge.target,
        TraceAnchor::Thread {
            thread_id: "019d0000-0000-7000-8000-000000000002".to_string()
        }
    );
    assert!(edge.carried_item_ids.is_empty());
    assert_eq!(
        edge.carried_raw_payload_ids,
        vec![
            invocation_payload.raw_payload_id,
            begin_payload.raw_payload_id,
            end_payload.raw_payload_id,
            result_payload.raw_payload_id,
        ]
    );
    assert_eq!(
        replayed.threads["019d0000-0000-7000-8000-000000000002"]
            .execution
            .status,
        ExecutionStatus::Completed
    );
    assert_eq!(replayed.status, RolloutStatus::Running);

    Ok(())
}

#[test]
fn agent_result_edge_links_child_result_to_parent_notification() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let writer = create_started_agent_writer(&temp)?;
    start_thread(
        &writer,
        "019d0000-0000-7000-8000-000000000002",
        "/root/child",
    )?;
    start_turn_for_thread(
        &writer,
        "019d0000-0000-7000-8000-000000000002",
        "turn-child-1",
    )?;
    append_completed_inference(
        &writer,
        "019d0000-0000-7000-8000-000000000002",
        "turn-child-1",
        "inference-child-1",
        vec![message("assistant", "task")],
        vec![message("assistant", "done")],
    )?;

    let notification = "<subagent_notification>{\"agent_path\":\"/root/child\",\"status\":{\"completed\":\"done\"}}</subagent_notification>";
    let carried_payload = writer.write_json_payload(
        RawPayloadKind::AgentResult,
        &json!({
            "child_agent_path": "/root/child",
            "message": notification,
            "status": {"completed": "done"}
        }),
    )?;
    writer.append_with_context(
        trace_context_for_thread("019d0000-0000-7000-8000-000000000002", "turn-child-1"),
        RawTraceEventPayload::AgentResultObserved {
            edge_id: "edge:agent_result:thread-child:turn-child-1:thread-root".to_string(),
            child_thread_id: "019d0000-0000-7000-8000-000000000002".to_string(),
            child_codex_turn_id: "turn-child-1".to_string(),
            parent_thread_id: "019d0000-0000-7000-8000-000000000001".to_string(),
            message: notification.to_string(),
            carried_payload: Some(carried_payload.clone()),
        },
    )?;

    start_agent_turn(&writer, "turn-root-1")?;
    let delivered = inter_agent_message(
        "/root/child",
        "/root",
        notification,
        /*trigger_turn*/ false,
    );
    append_inference_request(
        &writer,
        "019d0000-0000-7000-8000-000000000001",
        "turn-root-1",
        "inference-root-1",
        vec![message("assistant", &delivered)],
    )?;

    let replayed = replay_bundle(temp.path())?;
    let edge =
        &replayed.interaction_edges["edge:agent_result:thread-child:turn-child-1:thread-root"];
    assert_eq!(edge.kind, InteractionEdgeKind::AgentResult);
    let TraceAnchor::ConversationItem {
        item_id: source_item_id,
    } = &edge.source
    else {
        panic!("expected child result conversation item source");
    };
    assert_eq!(
        text_body(&replayed.conversation_items[source_item_id]),
        "done"
    );
    let target_item_id = target_conversation_item_id(&edge.target);
    assert_eq!(
        replayed.conversation_items[target_item_id].thread_id,
        "019d0000-0000-7000-8000-000000000001"
    );
    assert_eq!(edge.carried_item_ids, vec![target_item_id.clone()]);
    assert_eq!(
        edge.carried_raw_payload_ids,
        vec![carried_payload.raw_payload_id]
    );

    Ok(())
}

fn inter_agent_message(author: &str, recipient: &str, content: &str, trigger_turn: bool) -> String {
    json!({
        "author": author,
        "recipient": recipient,
        "other_recipients": [],
        "content": content,
        "trigger_turn": trigger_turn,
    })
    .to_string()
}

fn target_conversation_item_id(anchor: &TraceAnchor) -> &String {
    let TraceAnchor::ConversationItem { item_id } = anchor else {
        panic!("expected conversation item target");
    };
    item_id
}

fn text_body(item: &crate::model::ConversationItem) -> &str {
    let [crate::model::ConversationPart::Text { text }] = item.body.parts.as_slice() else {
        panic!("expected single text part");
    };
    text
}
