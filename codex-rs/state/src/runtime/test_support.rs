#[cfg(test)]
use chrono::DateTime;
#[cfg(test)]
use chrono::Utc;
#[cfg(test)]
use codex_protocol::ThreadId;
#[cfg(test)]
use codex_protocol::protocol::AskForApproval;
#[cfg(test)]
use codex_protocol::protocol::SandboxPolicy;
#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::time::SystemTime;
#[cfg(test)]
use std::time::UNIX_EPOCH;
#[cfg(test)]
use uuid::Uuid;

#[cfg(test)]
use crate::ThreadMetadata;

#[cfg(test)]
pub(super) fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "codex-state-runtime-test-{nanos}-{}",
        Uuid::new_v4()
    ))
}

#[cfg(test)]
pub(super) fn test_thread_metadata(
    codex_home: &Path,
    thread_id: ThreadId,
    cwd: PathBuf,
) -> ThreadMetadata {
    let now = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).expect("timestamp");
    ThreadMetadata {
        id: thread_id,
        rollout_path: codex_home.join(format!("rollout-{thread_id}.jsonl")),
        created_at: now,
        updated_at: now,
        source: "cli".to_string(),
        agent_nickname: None,
        agent_role: None,
        model_provider: "test-provider".to_string(),
        cwd,
        cli_version: "0.0.0".to_string(),
        title: String::new(),
        sandbox_policy: crate::extract::enum_to_string(&SandboxPolicy::new_read_only_policy()),
        approval_mode: crate::extract::enum_to_string(&AskForApproval::OnRequest),
        tokens_used: 0,
        first_user_message: Some("hello".to_string()),
        archived_at: None,
        git_sha: None,
        git_branch: None,
        git_origin_url: None,
    }
}
