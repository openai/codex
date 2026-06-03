use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use codex_api::SharedAuthProvider;
use codex_http_state::HttpStateContext;
use codex_http_state::HttpStateSurface;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::USER_AGENT;

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

#[derive(Clone)]
pub struct ChatGptRequestAuth {
    user_agent: HeaderValue,
    auth_provider: Option<SharedAuthProvider>,
}

impl ChatGptRequestAuth {
    pub fn headers_for_url(&self, request_url: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, self.user_agent.clone());
        if let Some(auth_provider) = &self.auth_provider {
            auth_provider.add_auth_headers_for_url(request_url, &mut headers);
        }
        headers
    }

    pub fn observe_response_headers(
        &self,
        request_url: &str,
        request_headers: &HeaderMap,
        response_headers: &HeaderMap,
    ) {
        if let Some(auth_provider) = &self.auth_provider {
            auth_provider.observe_response_headers(request_url, request_headers, response_headers);
        }
    }
}

/// Build request auth for ChatGPT-backed cloud-task requests.
pub async fn build_chatgpt_request_auth() -> ChatGptRequestAuth {
    set_user_agent_suffix("codex_cloud_tasks_tui");
    let ua = codex_login::default_client::get_codex_user_agent();
    let auth_manager = load_auth_manager(/*chatgpt_base_url*/ None).await;
    let auth_provider = if let Some(am) = auth_manager
        && let Some(auth) = am.auth_for_surface(HttpStateSurface::CodexCli).await
        && auth.uses_codex_backend()
    {
        Some(codex_model_provider::with_native_integrity_state(
            codex_model_provider::auth_provider_from_auth(&auth),
            Some(&auth),
            Some(HttpStateContext::new(
                am.codex_home().to_path_buf(),
                HttpStateSurface::CodexCli,
            )),
        ))
    } else {
        None
    };
    ChatGptRequestAuth {
        user_agent: HeaderValue::from_str(&ua).unwrap_or(HeaderValue::from_static("codex-cli")),
        auth_provider,
    }
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
