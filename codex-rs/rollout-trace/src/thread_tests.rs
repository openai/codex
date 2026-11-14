use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::FileChange;
use codex_protocol::protocol::PatchApplyBeginEvent;
use codex_protocol::protocol::PatchApplyEndEvent;
use codex_protocol::protocol::PatchApplyStatus;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::*;
use crate::AgentResultTracePayload;
use crate::CompactionCheckpointTracePayload;
use crate::ExecutionStatus;
use crate::RawTraceEventPayload;
use crate::RolloutStatus;
use crate::ToolCallKind;
use crate::ToolDispatchPayload;
use crate::ToolDispatchRequester;
use crate::ToolDispatchResult;
use crate::replay_bundle;

#[test]
fn create_in_root_writes_replayable_lifecycle_events() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let thread_id = ThreadId::new();
    let thread_trace = ThreadTraceContext::start_root_in_root_for_test(
        temp.path(),
        ThreadStartedTraceMetadata {
            thread_id: thread_id.to_string(),
            agent_path: "/root".to_string(),
            task_name: None,
            nickname: None,
            agent_role: None,
            session_source: SessionSource::Exec,
            cwd: PathBuf::from("/workspace"),
            rollout_path: Some(PathBuf::from("/tmp/rollout.jsonl")),
            model: "gpt-test".to_string(),
            provider_name: "test-provider".to_string(),
            approval_policy: "never".to_string(),
            permissions_profile: "danger-full-access".to_string(),
        },
    )?;

    thread_trace.record_ended(RolloutStatus::Completed);

    let bundle_dir = single_bundle_dir(temp.path())?;
    thread_trace.flush()?;
    let replayed = replay_bundle(&bundle_dir)?;

    assert_eq!(replayed.status, RolloutStatus::Completed);
    assert_eq!(replayed.root_thread_id, thread_id.to_string());
    assert_eq!(replayed.threads[&thread_id.to_string()].agent_path, "/root");
    assert_eq!(replayed.raw_payloads.len(), 1);
    assert_eq!(replayed.recording_status, crate::RecordingStatus::Unclosed);

    thread_trace.record_protocol_event(&EventMsg::ShutdownComplete);
    drop(thread_trace);
    let complete = replay_bundle(&bundle_dir)?;
    assert_eq!(complete.status, RolloutStatus::Completed);
    assert_eq!(complete.recording_status, crate::RecordingStatus::Complete);

    // A syntactically valid prefix is still an incomplete export.
    let path = bundle_dir.join("trace.jsonl");
    let events = fs::read_to_string(&path)?;
    fs::write(&path, events.strip_suffix('\n').unwrap())?;
    assert_eq!(
        replay_bundle(&bundle_dir)?.recording_status,
        crate::RecordingStatus::Unclosed
    );
    let (prefix, _) = events.trim_end().rsplit_once('\n').unwrap();
    fs::write(&path, prefix)?;
    assert_eq!(
        replay_bundle(&bundle_dir)?.recording_status,
        crate::RecordingStatus::Unclosed
    );
    fs::write(&path, &events)?;

    // Completion closes the log; even a correctly numbered later event is invalid.
    let mut trailing: serde_json::Value = serde_json::from_str(events.lines().last().unwrap())?;
    trailing["seq"] = (trailing["seq"].as_u64().unwrap() + 1).into();
    fs::write(&path, format!("{events}{trailing}\n"))?;
    assert!(replay_bundle(&bundle_dir).is_err());
    fs::write(&path, &events)?;

    // Protocol breadcrumbs are opaque to replay, but their payloads are still required.
    let opaque = complete
        .raw_payloads
        .values()
        .find(|payload| payload.kind == RawPayloadKind::ProtocolEvent)
        .unwrap();
    let shard = opaque.shard.unwrap();
    let shard_path = bundle_dir.join(format!("payloads/{shard:06}.zst"));
    let compressed = fs::read(&shard_path)?;
    // Even when all records can be decoded, a completed bundle requires the frame checksum.
    fs::write(&shard_path, &compressed[..compressed.len() - 1])?;
    assert!(replay_bundle(&bundle_dir).is_err());
    let mut corrupt_checksum = compressed.clone();
    *corrupt_checksum.last_mut().unwrap() ^= 1;
    fs::write(&shard_path, corrupt_checksum)?;
    assert!(replay_bundle(&bundle_dir).is_err());
    fs::write(&shard_path, &compressed)?;

    let mut reader = crate::PayloadReader::new(&bundle_dir);
    let original = reader.read(opaque)?;
    let mut decoded = zstd::decode_all(compressed.as_slice())?;
    let start = decoded
        .windows(original.len())
        .position(|body| body == original)
        .unwrap();
    let string_byte = start + original.iter().position(|&b| b == b'"').unwrap() + 1;
    for (index, bad_byte) in [(start, b'?'), (string_byte, 0xff)] {
        let old_byte = std::mem::replace(&mut decoded[index], bad_byte);
        fs::write(&shard_path, zstd::encode_all(decoded.as_slice(), 3)?)?;
        assert!(replay_bundle(&bundle_dir).is_err());
        decoded[index] = old_byte;
    }
    fs::remove_file(&shard_path)?;
    assert!(replay_bundle(&bundle_dir).is_err());

    Ok(())
}

#[test]
fn spawned_thread_start_appends_to_root_bundle() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let root_thread_id = ThreadId::new();
    let child_thread_id = ThreadId::new();
    let root_trace = ThreadTraceContext::start_root_in_root_for_test(
        temp.path(),
        minimal_metadata(root_thread_id),
    )?;

    let child_trace = root_trace.start_child_thread_trace(ThreadStartedTraceMetadata {
        thread_id: child_thread_id.to_string(),
        agent_path: "/root/repo_file_counter".to_string(),
        task_name: Some("repo_file_counter".to_string()),
        nickname: Some("Kepler".to_string()),
        agent_role: Some("worker".to_string()),
        session_source: SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id: root_thread_id,
            depth: 1,
            agent_path: Some(
                AgentPath::try_from("/root/repo_file_counter").map_err(anyhow::Error::msg)?,
            ),
            agent_nickname: Some("Kepler".to_string()),
            agent_role: Some("worker".to_string()),
        }),
        cwd: PathBuf::from("/workspace"),
        rollout_path: Some(PathBuf::from("/tmp/child-rollout.jsonl")),
        model: "gpt-test".to_string(),
        provider_name: "test-provider".to_string(),
        approval_policy: "never".to_string(),
        permissions_profile: "danger-full-access".to_string(),
    })?;
    child_trace.record_ended(RolloutStatus::Completed);
    let bundle_dir = single_bundle_dir(temp.path())?;
    root_trace.flush()?;
    let replayed = replay_bundle(&bundle_dir)?;

    assert_eq!(fs::read_dir(temp.path())?.count(), 1);
    assert_eq!(replayed.threads.len(), 2);
    assert_eq!(
        replayed.threads[&child_thread_id.to_string()].agent_path,
        "/root/repo_file_counter"
    );
    assert_eq!(replayed.status, RolloutStatus::Running);
    assert_eq!(
        replayed.threads[&child_thread_id.to_string()]
            .execution
            .status,
        ExecutionStatus::Completed
    );
    assert_eq!(replayed.raw_payloads.len(), 2);

    root_trace.record_ended(RolloutStatus::Completed);
    drop(root_trace);
    // Root shutdown does not close the recording while a child still owns it.
    assert_eq!(
        replay_bundle(&bundle_dir)?.recording_status,
        crate::RecordingStatus::Unclosed
    );
    drop(child_trace);
    assert_eq!(
        replay_bundle(&bundle_dir)?.recording_status,
        crate::RecordingStatus::Complete
    );

    Ok(())
}

#[test]
fn resumed_child_rejects_a_failed_shared_writer() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let root = ThreadTraceContext::start_root_in_root_for_test(
        temp.path(),
        minimal_metadata(ThreadId::new()),
    )?;
    let child_id = ThreadId::new();
    let child = root.start_child_thread_trace(minimal_metadata(child_id))?;
    child.record_ended(RolloutStatus::Completed);
    drop(child);

    // A parent payload write fails while the child is evicted.
    let bundle = single_bundle_dir(temp.path())?;
    // Occupy the next shard path so rotation fails without moving an open file.
    let next_shard = bundle.join("payloads/000002.zst");
    fs::write(&next_shard, [])?;
    let ThreadTraceContextState::Enabled(context) = &root.state else {
        unreachable!()
    };
    let oversized = "x".repeat(32 * 1024 * 1024);
    assert!(
        context
            .writer
            .write_json_payload(RawPayloadKind::ProtocolEvent, &oversized)
            .is_err()
    );
    fs::remove_file(next_shard)?;

    assert!(
        root.resume_child_thread_trace(child_id.to_string())
            .is_err()
    );
    drop(root);
    assert_eq!(
        replay_bundle(&bundle)?.recording_status,
        crate::RecordingStatus::Unclosed
    );
    Ok(())
}

#[test]
fn disabled_thread_context_accepts_trace_calls_without_writing() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let thread_trace = ThreadTraceContext::disabled();

    thread_trace.record_ended(RolloutStatus::Completed);
    thread_trace.record_protocol_event(&EventMsg::ShutdownComplete);
    thread_trace.record_codex_turn_event("turn-1", &EventMsg::ShutdownComplete);
    thread_trace.record_tool_call_event("turn-1", &EventMsg::ShutdownComplete);
    thread_trace.record_agent_result_interaction(
        "turn-1",
        ThreadId::new(),
        &AgentResultTracePayload {
            child_agent_path: "/root/child",
            message: "done",
            status: &AgentStatus::Completed(Some("done".to_string())),
        },
    );

    let inference_trace =
        thread_trace.inference_trace_context("turn-1", "gpt-test", "test-provider");
    let inference_attempt = inference_trace.start_attempt();
    let input = serde_json::json!({ "kind": "inference" });
    inference_attempt.record_started(
        serde_json::to_vec(&input)?,
        std::sync::Arc::new(input),
        || Ok(None),
    );
    let token_usage: Option<codex_protocol::protocol::TokenUsage> = None;
    inference_attempt.record_completed("response-1", Some("req-1"), &token_usage, &[]);
    inference_attempt.record_failed("inference failed", /*upstream_request_id*/ None, &[]);

    let compaction_trace = thread_trace.compaction_trace_context(
        "turn-1",
        "compaction-1",
        "gpt-test",
        "test-provider",
    );
    assert!(!compaction_trace.is_enabled());
    let compaction_attempt =
        compaction_trace.start_attempt::<()>(|| panic!("disabled tracing built the request"));
    compaction_attempt.record_completed(&[]);
    compaction_attempt.record_failed("compaction failed");
    compaction_trace.record_installed(&CompactionCheckpointTracePayload {
        input_history: &[],
        replacement_history: &[],
    });

    let built_dispatch_invocation = Cell::new(false);
    let dispatch_trace = thread_trace.start_tool_dispatch_trace(|| {
        built_dispatch_invocation.set(true);
        None
    });
    assert!(!built_dispatch_invocation.get());
    assert!(!dispatch_trace.is_enabled());

    assert_eq!(fs::read_dir(temp.path())?.count(), 0);

    Ok(())
}

#[test]
fn compaction_contexts_share_identity_across_models() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let thread_id = ThreadId::new();
    let thread_trace =
        ThreadTraceContext::start_root_in_root_for_test(temp.path(), minimal_metadata(thread_id))?;
    thread_trace.record_codex_turn_started("turn-1");

    for model in ["gpt-previous", "gpt-selected"] {
        let compaction_trace =
            thread_trace.compaction_trace_context("turn-1", "compaction-1", model, "test-provider");
        assert!(compaction_trace.is_enabled());
        let caller = std::thread::current().id();
        compaction_trace
            .start_attempt(move || {
                assert_ne!(std::thread::current().id(), caller);
                serde_json::json!({ "model": model })
            })
            .record_failed("test failure");
    }

    thread_trace.flush()?;
    let replayed = replay_bundle(&single_bundle_dir(temp.path())?)?;
    let mut attempts = replayed
        .compaction_requests
        .values()
        .map(|attempt| (attempt.model.clone(), attempt.compaction_id.clone()))
        .collect::<Vec<_>>();
    attempts.sort();
    assert_eq!(
        attempts,
        vec![
            ("gpt-previous".to_string(), "compaction-1".to_string()),
            ("gpt-selected".to_string(), "compaction-1".to_string()),
        ]
    );

    Ok(())
}

#[test]
fn protocol_wrapper_records_selected_events_as_raw_payloads() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let thread_id = ThreadId::new();
    let thread_trace =
        ThreadTraceContext::start_root_in_root_for_test(temp.path(), minimal_metadata(thread_id))?;

    thread_trace.record_protocol_event(&EventMsg::ShutdownComplete);

    thread_trace.flush()?;
    let event_log = fs::read_to_string(single_bundle_dir(temp.path())?.join("trace.jsonl"))?;
    let protocol_event_seen = event_log.lines().any(|line| {
        let event: crate::RawTraceEvent = serde_json::from_str(line).expect("raw trace event");
        matches!(
            event.payload,
            RawTraceEventPayload::ProtocolEventObserved {
                event_type,
                ..
            } if event_type == "shutdown_complete"
        )
    });

    assert!(protocol_event_seen);
    Ok(())
}

#[test]
fn dropping_tool_dispatch_closes_the_execution_window() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let thread_id = ThreadId::new();
    let trace =
        ThreadTraceContext::start_root_in_root_for_test(temp.path(), minimal_metadata(thread_id))?;
    trace.record_codex_turn_started("turn-1");
    let dispatch = trace.start_tool_dispatch_trace(|| {
        Some(ToolDispatchInvocation {
            thread_id: thread_id.to_string(),
            codex_turn_id: "turn-1".into(),
            tool_call_id: "call".into(),
            tool_name: "test_tool".into(),
            tool_namespace: None,
            requester: ToolDispatchRequester::Model {
                model_visible_call_id: "call".into(),
            },
            payload: ToolDispatchPayload::Function {
                arguments: "{}".into(),
            },
        })
    });
    trace.flush()?;
    let bundle = single_bundle_dir(temp.path())?;
    let before = replay_bundle(&bundle)?.tool_calls["call"].execution.clone();
    assert_eq!(before.status, ExecutionStatus::Running);
    assert_eq!(before.ended_at_unix_ms, None);

    drop(dispatch);
    trace.flush()?;
    let rollout = replay_bundle(&bundle)?;
    let call = &rollout.tool_calls["call"];
    assert_eq!(call.execution.status, ExecutionStatus::Aborted);
    assert_eq!(call.execution.started_at_unix_ms, before.started_at_unix_ms);
    assert!(call.execution.ended_at_unix_ms >= Some(before.started_at_unix_ms));
    assert!(call.execution.ended_seq > Some(before.started_seq));
    assert_eq!(call.raw_result_payload_id, None);
    Ok(())
}

#[test]
fn intercepted_exec_records_patch_without_a_terminal() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let thread_id = ThreadId::new();
    let trace =
        ThreadTraceContext::start_root_in_root_for_test(temp.path(), minimal_metadata(thread_id))?;
    trace.record_codex_turn_started("turn-1");
    let dispatch = trace.start_tool_dispatch_trace(|| Some(ToolDispatchInvocation {
        thread_id: thread_id.to_string(),
        codex_turn_id: "turn-1".to_string(),
        tool_call_id: "patch-call".to_string(),
        tool_name: "exec_command".to_string(),
        tool_namespace: None,
        requester: ToolDispatchRequester::Model {
            model_visible_call_id: "patch-call".to_string(),
        },
        payload: ToolDispatchPayload::Function {
            arguments: serde_json::json!({"cmd": "apply_patch <<'EOF'\n*** Begin Patch\n*** Add File: hello.txt\n+hello\n*** End Patch\nEOF"}).to_string(),
        },
    }));
    let bundle = single_bundle_dir(temp.path())?;
    trace.flush()?;
    let before = replay_bundle(&bundle)?.tool_calls["patch-call"].clone();
    let changes = HashMap::from([(
        PathBuf::from("hello.txt"),
        FileChange::Add {
            content: "hello\n".to_string(),
        },
    )]);
    trace.record_tool_call_event(
        "turn-1",
        &EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
            call_id: "patch-call".to_string(),
            turn_id: "turn-1".to_string(),
            auto_approved: true,
            changes: changes.clone(),
        }),
    );
    trace.record_tool_call_event(
        "turn-1",
        &EventMsg::PatchApplyEnd(PatchApplyEndEvent {
            call_id: "patch-call".to_string(),
            turn_id: "turn-1".to_string(),
            stdout: "Success".to_string(),
            stderr: String::new(),
            success: true,
            changes,
            status: PatchApplyStatus::Completed,
        }),
    );
    dispatch.record_completed(
        ExecutionStatus::Completed,
        ToolDispatchResult::DirectResponse {
            response_item: ResponseInputItem::FunctionCallOutput {
                call_id: "patch-call".to_string(),
                output: FunctionCallOutputPayload::from_text("Success".to_string()),
            },
        },
    );

    trace.flush()?;
    let rollout = replay_bundle(&bundle)?;
    let call = &rollout.tool_calls["patch-call"];
    assert_eq!(call.kind, ToolCallKind::ApplyPatch);
    assert_eq!(call.execution.status, ExecutionStatus::Completed);
    assert_eq!(call.summary, before.summary);
    assert_eq!(
        call.raw_invocation_payload_id,
        before.raw_invocation_payload_id
    );
    assert_eq!(call.raw_runtime_payload_ids.len(), 2);
    assert!(call.raw_result_payload_id.is_some());
    assert!(rollout.terminal_operations.is_empty());
    assert!(rollout.terminal_sessions.is_empty());
    Ok(())
}

fn minimal_metadata(thread_id: ThreadId) -> ThreadStartedTraceMetadata {
    ThreadStartedTraceMetadata {
        thread_id: thread_id.to_string(),
        agent_path: "/root".to_string(),
        task_name: None,
        nickname: None,
        agent_role: None,
        session_source: SessionSource::Exec,
        cwd: PathBuf::from("/workspace"),
        rollout_path: None,
        model: "gpt-test".to_string(),
        provider_name: "test-provider".to_string(),
        approval_policy: "never".to_string(),
        permissions_profile: "danger-full-access".to_string(),
    }
}

fn single_bundle_dir(root: &Path) -> anyhow::Result<PathBuf> {
    let mut entries = fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    assert_eq!(entries.len(), 1);
    Ok(entries.remove(0))
}
