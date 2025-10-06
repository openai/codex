use base64::Engine as _;
use chrono::Utc;
use reqwest::header::HeaderMap;

pub fn set_user_agent_suffix(suffix: &str) {
    if let Ok(mut guard) = codex_core::default_client::USER_AGENT_SUFFIX.lock() {
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

/// Build headers for ChatGPT-backed requests: `User-Agent`, optional `Authorization`,
/// and optional `ChatGPT-Account-Id`.
pub async fn build_chatgpt_headers() -> HeaderMap {
    use reqwest::header::AUTHORIZATION;
    use reqwest::header::HeaderName;
    use reqwest::header::HeaderValue;
    use reqwest::header::USER_AGENT;

    set_user_agent_suffix("codex_cloud_tasks_tui");
    let ua = codex_core::default_client::get_codex_user_agent();
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&ua).unwrap_or(HeaderValue::from_static("codex-cli")),
    );
    if let Ok(home) = codex_core::config::find_codex_home() {
        let am = codex_login::AuthManager::new(home, false);
        if let Some(auth) = am.auth()
            && let Ok(tok) = auth.get_token().await
            && !tok.is_empty()
        {
            let v = format!("Bearer {tok}");
            if let Ok(hv) = HeaderValue::from_str(&v) {
                headers.insert(AUTHORIZATION, hv);
            }
            if let Some(acc) = auth
                .get_account_id()
                .or_else(|| extract_chatgpt_account_id(&tok))
                && let Ok(name) = HeaderName::from_bytes(b"ChatGPT-Account-Id")
                && let Ok(hv) = HeaderValue::from_str(&acc)
            {
                headers.insert(name, hv);
            }
        }
    }
    headers
}
