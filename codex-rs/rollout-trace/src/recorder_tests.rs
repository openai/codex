use std::fs;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use tempfile::TempDir;

use super::*;
use crate::CompactionCheckpointTracePayload;
use crate::RolloutStatus;
use crate::replay_bundle;

#[test]
fn create_in_root_writes_replayable_lifecycle_events() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let thread_id = ThreadId::new();
    let recorder =
        RolloutTraceRecorder::create_in_root(temp.path(), thread_id).expect("trace recorder");
    recorder.record_thread_started(ThreadStartedTraceMetadata {
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
    });

    let bundle_dir = single_bundle_dir(temp.path())?;
    let replayed = replay_bundle(&bundle_dir)?;

    assert_eq!(replayed.status, RolloutStatus::Running);
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
    let recorder =
        RolloutTraceRecorder::create_in_root(temp.path(), root_thread_id).expect("trace recorder");
    recorder.record_thread_started(minimal_metadata(root_thread_id));

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
    let bundle_dir = single_bundle_dir(temp.path())?;
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
        crate::ExecutionStatus::Running
    );
    assert_eq!(replayed.raw_payloads.len(), 2);

    Ok(())
}

#[test]
fn disabled_recorder_accepts_trace_calls_without_writing() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let thread_id = ThreadId::new();
    let recorder = RolloutTraceRecorder::disabled();

    recorder.record_thread_started(minimal_metadata(thread_id));

    let inference_trace =
        recorder.inference_trace_context(thread_id, "turn-1", "gpt-test", "test-provider");
    let inference_attempt = inference_trace.start_attempt();
    inference_attempt.record_started(&serde_json::json!({ "kind": "inference" }));
    let token_usage: Option<codex_protocol::protocol::TokenUsage> = None;
    inference_attempt.record_completed("response-1", &token_usage, &[]);
    inference_attempt.record_failed("inference failed");

    let compaction_trace = recorder.compaction_trace_context(
        thread_id,
        "turn-1",
        "compaction-1",
        "gpt-test",
        "test-provider",
    );
    let compaction_attempt =
        compaction_trace.start_attempt(&serde_json::json!({ "kind": "compaction" }));
    compaction_attempt.record_completed(&[]);
    compaction_attempt.record_failed("compaction failed");
    compaction_trace.record_installed(&CompactionCheckpointTracePayload {
        input_history: &[],
        replacement_history: &[],
    });

    assert_eq!(fs::read_dir(temp.path())?.count(), 0);

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
