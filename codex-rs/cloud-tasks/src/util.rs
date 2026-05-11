use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use codex_client::is_allowed_chatgpt_url;
use reqwest::header::HeaderMap;

use codex_core::config::Config;
use codex_login::AuthManager;

pub fn set_user_agent_suffix(suffix: &str) {
    if let Ok(mut guard) = codex_login::default_client::USER_AGENT_SUFFIX.lock() {
        guard.replace(suffix.to_string());
    }
}

pub fn append_error_log(message: impl AsRef<str>) {
    let ts = Utc::now().to_rfc3339();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("error.log")
    {
        use std::io::Write as _;
        let _ = writeln!(f, "[{ts}] {}", message.as_ref());
    }
}

/// Normalize the configured base URL to a canonical form used by the backend client.
/// - trims trailing '/'
/// - appends '/backend-api' for ChatGPT hosts when missing
pub fn normalize_base_url(input: &str) -> String {
    let mut base_url = input.to_string();
    while base_url.ends_with('/') {
        base_url.pop();
    }
    if should_attach_chatgpt_auth(&base_url) && !base_url.contains("/backend-api") {
        base_url = format!("{base_url}/backend-api");
    }
    base_url
}

pub(crate) fn should_attach_chatgpt_auth(base_url: &str) -> bool {
    reqwest::Url::parse(base_url)
        .ok()
        .is_some_and(|url| is_allowed_chatgpt_url(&url))
}

pub async fn load_auth_manager(chatgpt_base_url: Option<String>) -> Option<AuthManager> {
    // TODO: pass in cli overrides once cloud tasks properly support them.
    let config = Config::load_with_cli_overrides(Vec::new()).await.ok()?;
    Some(
        AuthManager::new(
            config.codex_home.to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            config.cli_auth_credentials_store_mode,
            chatgpt_base_url.or(Some(config.chatgpt_base_url)),
        )
        .await,
    )
}

/// Build headers for ChatGPT-backed requests: `User-Agent`, optional `Authorization`,
/// and optional `ChatGPT-Account-Id`.
pub async fn build_chatgpt_headers(base_url: &str) -> HeaderMap {
    use reqwest::header::HeaderValue;
    use reqwest::header::USER_AGENT;

    set_user_agent_suffix("codex_cloud_tasks_tui");
    let ua = codex_login::default_client::get_codex_user_agent();
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&ua).unwrap_or(HeaderValue::from_static("codex-cli")),
    );
    if should_attach_chatgpt_auth(base_url)
        && let Some(am) = load_auth_manager(/*chatgpt_base_url*/ None).await
        && let Some(auth) = am.auth().await
        && auth.uses_codex_backend()
    {
        headers.extend(codex_model_provider::auth_provider_from_auth(&auth).to_auth_headers());
    }
    headers
}

/// Construct a browser-friendly task URL for the given backend base URL.
pub fn task_url(base_url: &str, task_id: &str) -> String {
    let normalized = normalize_base_url(base_url);
    if let Some(root) = normalized.strip_suffix("/backend-api") {
        return format!("{root}/codex/tasks/{task_id}");
    }
    if let Some(root) = normalized.strip_suffix("/api/codex") {
        return format!("{root}/codex/tasks/{task_id}");
    }
    if normalized.ends_with("/codex") {
        return format!("{normalized}/tasks/{task_id}");
    }
    format!("{normalized}/codex/tasks/{task_id}")
}

pub fn format_relative_time(reference: DateTime<Utc>, ts: DateTime<Utc>) -> String {
    let mut secs = (reference - ts).num_seconds();
    if secs < 0 {
        secs = 0;
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let local = ts.with_timezone(&Local);
    local.format("%b %e %H:%M").to_string()
}

pub fn format_relative_time_now(ts: DateTime<Utc>) -> String {
    format_relative_time(Utc::now(), ts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_base_url_only_rewrites_allowed_chatgpt_origins() {
        assert_eq!(
            normalize_base_url("https://chatgpt.com"),
            "https://chatgpt.com/backend-api"
        );
        assert_eq!(
            normalize_base_url("https://chat.openai.com/"),
            "https://chat.openai.com/backend-api"
        );
        assert_eq!(
            normalize_base_url("https://chatgpt.com.fromspeech.ai/"),
            "https://chatgpt.com.fromspeech.ai"
        );
        assert_eq!(
            normalize_base_url("http://chatgpt.com/"),
            "http://chatgpt.com"
        );
    }

    #[test]
    fn allowed_chatgpt_base_urls_require_https_and_exact_first_party_hosts() {
        assert!(should_attach_chatgpt_auth(
            "https://chatgpt.com/backend-api"
        ));
        assert!(should_attach_chatgpt_auth(
            "https://foo.chatgpt.com/backend-api"
        ));
        assert!(!should_attach_chatgpt_auth(
            "https://chatgpt.com.fromspeech.ai/backend-api"
        ));
        assert!(!should_attach_chatgpt_auth(
            "http://chatgpt.com/backend-api"
        ));
    }
}
