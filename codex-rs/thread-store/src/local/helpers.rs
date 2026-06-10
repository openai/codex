use std::ffi::OsStr;
use std::fs::FileTimes;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use chrono::DateTime;
use chrono::Utc;
use codex_git_utils::GitSha;
use codex_protocol::ThreadId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GitInfo;
use codex_protocol::protocol::NetworkAccess;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_rollout::ARCHIVED_SESSIONS_SUBDIR;
use codex_rollout::ThreadItem;
use codex_state::ThreadMetadata;

use crate::StoredThread;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;

pub(super) fn scoped_rollout_path(
    root: PathBuf,
    rollout_path: &Path,
    root_name: &str,
) -> ThreadStoreResult<PathBuf> {
    let canonical_root =
        std::fs::canonicalize(&root).map_err(|err| ThreadStoreError::Internal {
            message: format!(
                "failed to resolve {root_name} directory `{}`: {err}",
                root.display()
            ),
        })?;
    let canonical_rollout_path =
        std::fs::canonicalize(rollout_path).map_err(|_| ThreadStoreError::InvalidRequest {
            message: format!(
                "rollout path `{}` must be in {root_name} directory",
                rollout_path.display()
            ),
        })?;
    if canonical_rollout_path.starts_with(&canonical_root) {
        Ok(canonical_rollout_path)
    } else {
        Err(ThreadStoreError::InvalidRequest {
            message: format!(
                "rollout path `{}` must be in {root_name} directory",
                rollout_path.display()
            ),
        })
    }
}

pub(super) fn rollout_path_is_archived(codex_home: &Path, path: &Path) -> bool {
    path.starts_with(codex_home.join(ARCHIVED_SESSIONS_SUBDIR))
        || path
            .components()
            .any(|component| component.as_os_str() == OsStr::new(ARCHIVED_SESSIONS_SUBDIR))
}

pub(super) fn matching_rollout_file_name(
    rollout_path: &Path,
    thread_id: ThreadId,
    display_path: &Path,
) -> ThreadStoreResult<std::ffi::OsString> {
    let Some(file_name) = rollout_path.file_name().map(OsStr::to_owned) else {
        return Err(ThreadStoreError::InvalidRequest {
            message: format!(
                "rollout path `{}` missing file name",
                display_path.display()
            ),
        });
    };
    let required_plain_suffix = format!("{thread_id}.jsonl");
    let required_compressed_suffix = format!("{required_plain_suffix}.zst");
    let file_name_str = file_name.to_string_lossy();
    if file_name_str.ends_with(required_plain_suffix.as_str())
        || file_name_str.ends_with(required_compressed_suffix.as_str())
    {
        Ok(file_name)
    } else {
        Err(ThreadStoreError::InvalidRequest {
            message: format!(
                "rollout path `{}` does not match thread id {thread_id}",
                display_path.display()
            ),
        })
    }
}

pub(super) fn touch_modified_time(path: &Path) -> std::io::Result<()> {
    let times = FileTimes::new().set_modified(SystemTime::now());
    OpenOptions::new().append(true).open(path)?.set_times(times)
}

pub(super) fn stored_thread_from_rollout_item(
    item: ThreadItem,
    archived: bool,
    default_provider: &str,
) -> Option<StoredThread> {
    let thread_id = item
        .thread_id
        .or_else(|| thread_id_from_rollout_path(item.path.as_path()))?;
    let created_at = parse_rfc3339(item.created_at.as_deref()).unwrap_or_else(Utc::now);
    let updated_at = parse_rfc3339(item.updated_at.as_deref()).unwrap_or(created_at);
    let archived_at = archived.then_some(updated_at);
    let git_info = git_info_from_parts(
        item.git_sha.clone(),
        item.git_branch.clone(),
        item.git_origin_url.clone(),
    );
    let source = item.source.unwrap_or(SessionSource::Unknown);
    let preview = item
        .preview
        .clone()
        .or_else(|| item.first_user_message.clone())
        .unwrap_or_default();
    let rollout_path = codex_rollout::plain_rollout_path(item.path.as_path());

    Some(StoredThread {
        thread_id,
        extra_config: None,
        rollout_path: Some(rollout_path),
        forked_from_id: None,
        parent_thread_id: item.parent_thread_id,
        preview,
        name: None,
        model_provider: item
            .model_provider
            .filter(|provider| !provider.is_empty())
            .unwrap_or_else(|| default_provider.to_string()),
        model: None,
        reasoning_effort: None,
        created_at,
        updated_at,
        archived_at,
        cwd: item.cwd.unwrap_or_default(),
        cli_version: item.cli_version.unwrap_or_default(),
        source,
        thread_source: None,
        agent_nickname: item.agent_nickname,
        agent_role: item.agent_role,
        agent_path: None,
        git_info,
        approval_mode: AskForApproval::OnRequest,
        permission_profile: PermissionProfile::read_only(),
        token_usage: None,
        first_user_message: item.first_user_message,
        history: None,
    })
}

pub(super) fn permission_profile_from_metadata_value(value: &str, cwd: &Path) -> PermissionProfile {
    serde_json::from_str::<PermissionProfile>(value)
        .or_else(|_| {
            parse_legacy_sandbox_policy(value)
                .map(|policy| PermissionProfile::from_legacy_sandbox_policy_for_cwd(&policy, cwd))
        })
        .unwrap_or_else(|_| PermissionProfile::read_only())
}

pub(super) fn permission_profile_to_metadata_value(
    permission_profile: &PermissionProfile,
) -> String {
    match serde_json::to_string(permission_profile) {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!("failed to serialize permission profile metadata: {err}");
            String::new()
        }
    }
}

pub(super) fn distinct_thread_metadata_title(metadata: &ThreadMetadata) -> Option<String> {
    let title = metadata.title.trim();
    if title.is_empty() || metadata.first_user_message.as_deref().map(str::trim) == Some(title) {
        None
    } else {
        Some(title.to_string())
    }
}

pub(super) fn set_thread_name_from_title(thread: &mut StoredThread, title: String) {
    if title.trim().is_empty() || thread.preview.trim() == title.trim() {
        return;
    }
    thread.name = Some(title);
}

fn parse_rfc3339(value: Option<&str>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value?)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_legacy_sandbox_policy(value: &str) -> serde_json::Result<SandboxPolicy> {
    serde_json::from_str(value)
        .or_else(|_| serde_json::from_value(serde_json::Value::String(value.to_string())))
        .or_else(|_| match value {
            "danger-full-access" => Ok(SandboxPolicy::DangerFullAccess),
            "read-only" => Ok(SandboxPolicy::new_read_only_policy()),
            "workspace-write" => Ok(SandboxPolicy::new_workspace_write_policy()),
            "external-sandbox" => Ok(SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            }),
            _ => serde_json::from_value(serde_json::Value::String(value.to_string())),
        })
}

pub(super) fn git_info_from_parts(
    sha: Option<String>,
    branch: Option<String>,
    origin_url: Option<String>,
) -> Option<GitInfo> {
    if sha.is_none() && branch.is_none() && origin_url.is_none() {
        return None;
    }
    Some(GitInfo {
        commit_hash: sha.as_deref().map(GitSha::new),
        branch,
        repository_url: origin_url,
    })
}

fn thread_id_from_rollout_path(path: &Path) -> Option<ThreadId> {
    let file_name = path.file_name()?.to_str()?;
    let file_name = file_name.strip_suffix(".zst").unwrap_or(file_name);
    let stem = file_name.strip_suffix(".jsonl")?;
    if stem.len() < 37 {
        return None;
    }
    let uuid_start = stem.len().saturating_sub(36);
    if !stem[..uuid_start].ends_with('-') {
        return None;
    }
    ThreadId::from_string(&stem[uuid_start..]).ok()
}

#[cfg(test)]
mod tests {
    use codex_rollout::ThreadItem;
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn stored_thread_from_rollout_item_returns_logical_rollout_path() {
        let uuid = Uuid::from_u128(1);
        let compressed_path = PathBuf::from(format!(
            "/tmp/sessions/2025/01/03/rollout-2025-01-03T12-00-00-{uuid}.jsonl.zst"
        ));
        let thread = stored_thread_from_rollout_item(
            ThreadItem {
                path: compressed_path.clone(),
                ..Default::default()
            },
            /*archived*/ false,
            "test-provider",
        )
        .expect("stored thread");

        assert_eq!(
            thread.rollout_path,
            Some(
                compressed_path.with_file_name(format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"))
            )
        );
    }
}
