use base64::Engine as _;
use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use reqwest::header::HeaderMap;
use std::sync::Arc;

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
    if (base_url.starts_with("https://chatgpt.com")
        || base_url.starts_with("https://chat.openai.com"))
        && !base_url.contains("/backend-api")
    {
        base_url = format!("{base_url}/backend-api");
    }
    base_url
}

/// Extract the ChatGPT account id from a JWT token, when present.
pub fn extract_chatgpt_account_id(token: &str) -> Option<String> {
    let mut parts = token.split('.');
    let (_h, payload_b64, _s) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => return None,
    };
    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&payload_bytes).ok()?;
    v.get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .and_then(|id| id.as_str())
        .map(str::to_string)
}

pub async fn load_auth_manager(chatgpt_base_url: Option<String>) -> Option<Arc<AuthManager>> {
    // TODO: pass in cli overrides once cloud tasks properly support them.
    let config = Config::load_with_cli_overrides(Vec::new()).await.ok()?;
    let auth_manager =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false);
    if let Some(chatgpt_base_url) = chatgpt_base_url {
        auth_manager.set_chatgpt_backend_base_url(Some(chatgpt_base_url));
    }
    Some(auth_manager)
}

/// Build headers for ChatGPT-backed requests.
pub async fn build_chatgpt_headers() -> HeaderMap {
    use reqwest::header::AUTHORIZATION;
    use reqwest::header::HeaderName;
    use reqwest::header::HeaderValue;
    use reqwest::header::USER_AGENT;

    set_user_agent_suffix("codex_cloud_tasks_tui");
    let ua = codex_login::default_client::get_codex_user_agent();
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&ua).unwrap_or(HeaderValue::from_static("codex-cli")),
    );
    let base_url = normalize_base_url(
        &std::env::var("CODEX_CLOUD_TASKS_BASE_URL")
            .unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string()),
    );
    if let Some(auth_manager) = load_auth_manager(Some(base_url)).await
        && let Some(auth) = auth_manager.auth().await
    {
        if let Some(authorization_header_value) = auth_manager
            .chatgpt_authorization_header_for_auth(&auth)
            .await
            && let Ok(hv) = HeaderValue::from_str(&authorization_header_value)
        {
            headers.insert(AUTHORIZATION, hv);
        }
        if let Some(acc) = auth.get_account_id().or_else(|| {
            auth.get_token()
                .ok()
                .and_then(|token| extract_chatgpt_account_id(&token))
        }) && let Ok(name) = HeaderName::from_bytes(b"ChatGPT-Account-Id")
            && let Ok(hv) = HeaderValue::from_str(&acc)
        {
            headers.insert(name, hv);
        }
        if auth.is_fedramp_account()
            && let Ok(name) = HeaderName::from_bytes(b"X-OpenAI-Fedramp")
        {
            headers.insert(name, HeaderValue::from_static("true"));
        }
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
