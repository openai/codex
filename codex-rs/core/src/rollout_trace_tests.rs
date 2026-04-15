use std::fs;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_rollout_trace::ExecutionStatus;
use codex_rollout_trace::RawTraceEventPayload;
use codex_rollout_trace::RolloutStatus;
use tempfile::TempDir;

use super::*;

#[test]
fn create_in_root_writes_replayable_lifecycle_events() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let thread_id = ThreadId::new();
    let recorder = RolloutTraceRecorder::create_in_root(
        temp.path(),
        thread_id,
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
            sandbox_policy: format!("{:?}", SandboxPolicy::DangerFullAccess),
        },
    )
    .expect("trace recorder");

    recorder.record_thread_ended(thread_id.to_string(), RolloutStatus::Completed);

    let bundle_dir = single_bundle_dir(temp.path())?;
    let replayed = codex_rollout_trace::replay_bundle(&bundle_dir)?;

    assert_eq!(replayed.status, RolloutStatus::Completed);
    assert_eq!(replayed.root_thread_id, thread_id.to_string());
    assert_eq!(replayed.threads[&thread_id.to_string()].agent_path, "/root");
    assert_eq!(replayed.raw_payloads.len(), 1);

    Ok(())
}

#[test]
fn spawned_thread_start_appends_to_root_bundle() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let root_thread_id = ThreadId::new();
    let child_thread_id = ThreadId::new();
    let recorder = RolloutTraceRecorder::create_in_root(
        temp.path(),
        root_thread_id,
        minimal_metadata(root_thread_id),
    )
    .expect("trace recorder");

    recorder.record_thread_started(ThreadStartedTraceMetadata {
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
        sandbox_policy: format!("{:?}", SandboxPolicy::DangerFullAccess),
    });
    recorder.record_thread_ended(child_thread_id.to_string(), RolloutStatus::Completed);

    let bundle_dir = single_bundle_dir(temp.path())?;
    let replayed = codex_rollout_trace::replay_bundle(&bundle_dir)?;

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

    Ok(())
}

#[test]
fn protocol_wrapper_records_selected_events_as_raw_payloads() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let thread_id = ThreadId::new();
    let recorder =
        RolloutTraceRecorder::create_in_root(temp.path(), thread_id, minimal_metadata(thread_id))
            .expect("trace recorder");

    recorder.record_protocol_event(&EventMsg::ShutdownComplete);

    let event_log = fs::read_to_string(single_bundle_dir(temp.path())?.join("trace.jsonl"))?;
    let protocol_event_seen = event_log.lines().any(|line| {
        let event: codex_rollout_trace::RawTraceEvent =
            serde_json::from_str(line).expect("raw trace event");
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
        sandbox_policy: "danger-full-access".to_string(),
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
