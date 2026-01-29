use crate::config::Config;
use crate::default_client::create_client;
use crate::git_info::collect_git_info;
use crate::git_info::get_git_repo_root;
use codex_app_server_protocol::AuthMode;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SkillScope;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct TrackEventsContext {
    pub(crate) auth_mode: Option<AuthMode>,
    pub(crate) access_token: Option<String>,
    pub(crate) account_id: Option<String>,
    pub(crate) model_slug: String,
    pub(crate) conversation_id: String,
    pub(crate) session_source: SessionSource,
    pub(crate) product_client_id: String,
}

pub(crate) struct SkillInvocation {
    pub(crate) skill_name: String,
    pub(crate) skill_scope: SkillScope,
    pub(crate) skill_path: PathBuf,
}

#[derive(Serialize)]
struct TrackEventsRequest {
    events: Vec<TrackEvent>,
}

#[derive(Serialize)]
struct TrackEvent {
    event_type: &'static str,
    event_params: TrackEventParams,
}

#[derive(Serialize)]
struct TrackEventParams {
    skill_id: String,
    skill_scope: String,
    product_surface: String,
    product_client_id: String,
    model_slug: String,
    conversation_id: String,
    gizmo_id: Option<String>,
    gizmo_type: Option<String>,
    message_id: Option<String>,
}

pub(crate) async fn track_skill_invocations(
    config: &Config,
    tracking: Option<&TrackEventsContext>,
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
    if tracking.auth_mode != Some(AuthMode::ChatGPT) {
        return;
    }
    let (Some(access_token), Some(account_id)) =
        (tracking.access_token.as_ref(), tracking.account_id.as_ref())
    else {
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
            repo_root.as_ref().map(PathBuf::as_path),
            invocation.skill_path.as_path(),
            invocation.skill_name.as_str(),
        );
        let skill_id_path = skill_id_path_for_local_skill(
            repo_url.as_deref(),
            repo_root.as_ref().map(PathBuf::as_path),
            invocation.skill_path.as_path(),
        );
        events.push(TrackEvent {
            event_type: "skill_invocation",
            //TODO: add skill_name_encrypted
            event_params: TrackEventParams {
                skill_id,
                skill_scope: skill_scope.to_string(),
                product_surface: tracking.session_source.to_string(),
                product_client_id: tracking.product_client_id.clone(),
                model_slug: tracking.model_slug.clone(),
                conversation_id: tracking.conversation_id.clone(),
                gizmo_id: None,
                gizmo_type: None,
                message_id: None,
            },
        });
    }

    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = if base_url.ends_with("/backend-api/codex") {
        format!("{base_url}/analytics-events/track-events")
    } else if base_url.ends_with("/backend-api") {
        format!("{base_url}/codex/analytics-events/track-events")
    } else {
        format!("{base_url}/api/codex/analytics-events/track-events")
    };
    let payload = TrackEventsRequest { events };

    let response = create_client()
        .post(&url)
        .bearer_auth(access_token)
        .header("chatgpt-account-id", account_id)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;

    match response {
        Ok(response) if response.status().is_success() => {}
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("track-events failed with status {status}: {body}");
        }
        Err(err) => {
            tracing::warn!("failed to send track-events request: {err}");
        }
    }
}

fn skill_id_for_local_skill(
    repo_url: Option<&str>,
    repo_root: Option<&Path>,
    skill_path: &Path,
    skill_name: &str,
) -> String {
    let path = skill_id_path_for_local_skill(repo_url, repo_root, skill_path);
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

fn skill_id_path_for_local_skill(
    repo_url: Option<&str>,
    repo_root: Option<&Path>,
    skill_path: &Path,
) -> String {
    let resolved_path =
        std::fs::canonicalize(skill_path).unwrap_or_else(|_| skill_path.to_path_buf());
    match (repo_url, repo_root) {
        (Some(_), Some(root)) => resolved_path
            .strip_prefix(root)
            .unwrap_or(resolved_path.as_path())
            .to_string_lossy()
            .replace('\\', "/"),
        _ => resolved_path.to_string_lossy().replace('\\', "/"),
    }
}
