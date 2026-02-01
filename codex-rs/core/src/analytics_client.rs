use crate::AuthManager;
use crate::config::Config;
use crate::default_client::create_client;
use crate::git_info::collect_git_info;
use crate::git_info::get_git_repo_root;
use codex_protocol::protocol::SkillScope;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Clone)]
pub(crate) struct TrackEventsContext {
    pub(crate) model_slug: String,
    pub(crate) thread_id: String,
}

pub(crate) fn build_track_events_context(
    model_slug: String,
    thread_id: String,
) -> TrackEventsContext {
    TrackEventsContext {
        model_slug,
        thread_id,
    }
}

pub(crate) struct SkillInvocation {
    pub(crate) skill_name: String,
    pub(crate) skill_scope: SkillScope,
    pub(crate) skill_path: PathBuf,
}

#[derive(Clone)]
pub(crate) struct AnalyticsEventsQueue {
    sender: mpsc::Sender<TrackEventsJob>,
}

pub(crate) struct AnalyticsEventsClient {
    queue: AnalyticsEventsQueue,
    config: Arc<Config>,
}

impl AnalyticsEventsQueue {
    pub(crate) fn new(auth_manager: Arc<AuthManager>) -> Self {
        let (sender, mut receiver) = mpsc::channel(ANALYTICS_EVENTS_QUEUE_SIZE);
        tokio::spawn(async move {
            while let Some(job) = receiver.recv().await {
                send_track_skill_invocations(&auth_manager, job).await;
            }
        });
        Self { sender }
    }

    fn try_send(&self, job: TrackEventsJob) {
        if self.sender.try_send(job).is_err() {
            //TODO: add a metric for this
            tracing::warn!("dropping skill analytics events: queue is full");
        }
    }
}

impl AnalyticsEventsClient {
    pub(crate) fn new(config: Arc<Config>, auth_manager: Arc<AuthManager>) -> Self {
        Self {
            queue: AnalyticsEventsQueue::new(Arc::clone(&auth_manager)),
            config,
        }
    }

    pub(crate) fn track_skill_invocations(
        &self,
        tracking: TrackEventsContext,
        invocations: Vec<SkillInvocation>,
    ) {
        track_skill_invocations(
            &self.queue,
            Arc::clone(&self.config),
            Some(tracking),
            invocations,
        );
    }
}

struct TrackEventsJob {
    config: Arc<Config>,
    tracking: TrackEventsContext,
    invocations: Vec<SkillInvocation>,
}

const ANALYTICS_EVENTS_QUEUE_SIZE: usize = 256;
const ANALYTICS_EVENTS_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Serialize)]
struct TrackEventsRequest {
    events: Vec<TrackEvent>,
}

#[derive(Serialize)]
struct TrackEvent {
    event_type: &'static str,
    skill_id: String,
    skill_name: String,
    event_params: TrackEventParams,
}

#[derive(Serialize)]
struct TrackEventParams {
    product_client_id: Option<String>,
    skill_scope: Option<String>,
    repo_url: Option<String>,
    thread_id: Option<String>,
    invoke_type: Option<String>,
    model_slug: Option<String>,
}

pub(crate) fn track_skill_invocations(
    queue: &AnalyticsEventsQueue,
    config: Arc<Config>,
    tracking: Option<TrackEventsContext>,
    invocations: Vec<SkillInvocation>,
) {
    if config.analytics_enabled == Some(false) {
        return;
    }
    let Some(tracking) = tracking else {
        return;
    };
    if invocations.is_empty() {
        return;
    }
    let job = TrackEventsJob {
        config,
        tracking,
        invocations,
    };
    queue.try_send(job);
}

async fn send_track_skill_invocations(auth_manager: &AuthManager, job: TrackEventsJob) {
    let TrackEventsJob {
        config,
        tracking,
        invocations,
    } = job;
    let Some(auth) = auth_manager.auth().await else {
        return;
    };
    if !auth.is_chatgpt_auth() {
        return;
    }
    let access_token = match auth.get_token() {
        Ok(token) => token,
        Err(_) => return,
    };
    let Some(account_id) = auth.get_account_id() else {
        return;
    };

    let mut events = Vec::with_capacity(invocations.len());
    for invocation in invocations {
        let skill_scope = match invocation.skill_scope {
            SkillScope::User => "user",
            SkillScope::Repo => "repo",
            SkillScope::System => "system",
            SkillScope::Admin => "admin",
        };
        let repo_root = get_git_repo_root(invocation.skill_path.as_path());
        let repo_url = if let Some(root) = repo_root.as_ref() {
            collect_git_info(root)
                .await
                .and_then(|info| info.repository_url)
        } else {
            None
        };
        let skill_id = skill_id_for_local_skill(
            repo_url.as_deref(),
            repo_root.as_deref(),
            invocation.skill_path.as_path(),
            invocation.skill_name.as_str(),
        );
        events.push(TrackEvent {
            event_type: "skill_invocation",
            skill_id,
            skill_name: invocation.skill_name.clone(),
            event_params: TrackEventParams {
                thread_id: Some(tracking.thread_id.clone()),
                invoke_type: Some("explicit".to_string()),
                model_slug: Some(tracking.model_slug.clone()),
                product_client_id: Some(crate::default_client::originator().value),
                repo_url,
                skill_scope: Some(skill_scope.to_string()),
            },
        });
    }

    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/codex/analytics-events/events");
    let payload = TrackEventsRequest { events };

    let response = create_client()
        .post(&url)
        .timeout(ANALYTICS_EVENTS_TIMEOUT)
        .bearer_auth(&access_token)
        .header("chatgpt-account-id", &account_id)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(response) if response.status().is_success() => {}
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("events failed with status {status}: {body}");
        }
        Err(err) => {
            tracing::warn!("failed to send events request: {err}");
        }
    }
}

fn skill_id_for_local_skill(
    repo_url: Option<&str>,
    repo_root: Option<&Path>,
    skill_path: &Path,
    skill_name: &str,
) -> String {
    let path = normalize_path_for_skill_id(repo_url, repo_root, skill_path);
    let prefix = if let Some(url) = repo_url {
        format!("repo_{url}")
    } else {
        "personal".to_string()
    };
    let raw_id = format!("{prefix}_{path}_{skill_name}");
    let mut hasher = Sha1::new();
    hasher.update(raw_id.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Returns a normalized path for skill ID construction.
///
/// - Repo-scoped skills use a path relative to the repo root.
/// - User/admin/system skills use an absolute path.
fn normalize_path_for_skill_id(
    repo_url: Option<&str>,
    repo_root: Option<&Path>,
    skill_path: &Path,
) -> String {
    let resolved_path =
        std::fs::canonicalize(skill_path).unwrap_or_else(|_| skill_path.to_path_buf());
    match (repo_url, repo_root) {
        (Some(_), Some(root)) => {
            let resolved_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
            resolved_path
                .strip_prefix(&resolved_root)
                .unwrap_or(resolved_path.as_path())
                .to_string_lossy()
                .replace('\\', "/")
        }
        _ => resolved_path.to_string_lossy().replace('\\', "/"),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_path_for_skill_id;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn expected_absolute_path(path: &PathBuf) -> String {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/")
    }

    #[test]
    fn normalize_path_for_skill_id_repo_scoped_uses_relative_path() {
        let repo_root = PathBuf::from("/repo/root");
        let skill_path = PathBuf::from("/repo/root/.codex/skills/doc/SKILL.md");

        let path = normalize_path_for_skill_id(
            Some("https://example.com/repo.git"),
            Some(repo_root.as_path()),
            skill_path.as_path(),
        );

        assert_eq!(path, ".codex/skills/doc/SKILL.md");
    }

    #[test]
    fn normalize_path_for_skill_id_user_scoped_uses_absolute_path() {
        let skill_path = PathBuf::from("/Users/abc/.codex/skills/doc/SKILL.md");

        let path = normalize_path_for_skill_id(None, None, skill_path.as_path());
        let expected = expected_absolute_path(&skill_path);

        assert_eq!(path, expected);
    }

    #[test]
    fn normalize_path_for_skill_id_admin_scoped_uses_absolute_path() {
        let skill_path = PathBuf::from("/etc/codex/skills/doc/SKILL.md");

        let path = normalize_path_for_skill_id(None, None, skill_path.as_path());
        let expected = expected_absolute_path(&skill_path);

        assert_eq!(path, expected);
    }

    #[test]
    fn normalize_path_for_skill_id_repo_root_not_in_skill_path_uses_absolute_path() {
        let repo_root = PathBuf::from("/repo/root");
        let skill_path = PathBuf::from("/other/path/.codex/skills/doc/SKILL.md");

        let path = normalize_path_for_skill_id(
            Some("https://example.com/repo.git"),
            Some(repo_root.as_path()),
            skill_path.as_path(),
        );
        let expected = expected_absolute_path(&skill_path);

        assert_eq!(path, expected);
    }
}
