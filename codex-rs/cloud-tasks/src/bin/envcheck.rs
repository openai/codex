use base64::Engine;
use clap::Parser;
use codex_cloud_tasks::util::set_user_agent_suffix;
use codex_core::config::find_codex_home;
use codex_core::default_client::get_codex_user_agent;
use codex_login::AuthManager;
use reqwest::header::AUTHORIZATION;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;

#[derive(Debug, Parser)]
#[command(version, about = "Resolve Codex environment id (debug helper)")]
struct Args {
    /// Optional override for environment id; if present we just echo it.
    #[arg(long = "env-id")]
    environment_id: Option<String>,
    /// Optional label to select a matching environment (case-insensitive exact match).
    #[arg(long = "env-label")]
    environment_label: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Base URL (default to ChatGPT backend API) with normalization
    let mut base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL")
        .unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
    while base_url.ends_with('/') {
        base_url.pop();
    }
    if (base_url.starts_with("https://chatgpt.com")
        || base_url.starts_with("https://chat.openai.com"))
        && !base_url.contains("/backend-api")
    {
        base_url = format!("{base_url}/backend-api");
    }
    println!("base_url: {base_url}");
    println!(
        "path_style: {}",
        if base_url.contains("/backend-api") {
            "wham"
        } else {
            "codex-api"
        }
    );

    // Build headers: UA + ChatGPT auth if available
    set_user_agent_suffix("codex_cloud_tasks_envcheck");
    let ua = get_codex_user_agent();
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        HeaderValue::from_str(&ua).unwrap_or(HeaderValue::from_static("codex-cli")),
    );

    // Locate CODEX_HOME and try to load ChatGPT auth
    if let Ok(home) = find_codex_home() {
        println!("codex_home: {}", home.display());
        let authm = AuthManager::new(home);
        if let Some(auth) = authm.auth() {
            match auth.get_token().await {
                Ok(token) if !token.is_empty() => {
                    println!("auth: ChatGPT token present ({} chars)", token.len());
                    let value = format!("Bearer {token}");
                    if let Ok(hv) = HeaderValue::from_str(&value) {
                        headers.insert(AUTHORIZATION, hv);
                    }
                    if let Some(account_id) = auth
                        .get_account_id()
                        .or_else(|| extract_chatgpt_account_id(&token))
                    {
                        println!("auth: ChatGPT-Account-Id: {account_id}");
                        if let Ok(name) = HeaderName::from_bytes(b"ChatGPT-Account-Id")
                            && let Ok(hv) = HeaderValue::from_str(&account_id)
                        {
                            headers.insert(name, hv);
                        }
                    }
                }
                Ok(_) => println!("auth: ChatGPT token empty"),
                Err(e) => println!("auth: failed to load ChatGPT token: {e}"),
            }
        } else {
            println!("auth: no ChatGPT auth.json");
        }
    } else {
        println!("codex_home: <not found>");
    }

    // If user supplied an environment id, just echo it and exit.
    if let Some(id) = args.environment_id {
        println!("env: provided env-id={id}");
        return Ok(());
    }

    // Auto-detect environment id using shared env_detect
    match codex_cloud_tasks::env_detect::autodetect_environment_id(
        &base_url,
        &headers,
        args.environment_label,
    )
    .await
    {
        Ok(sel) => {
            println!(
                "env: selected environment_id={} label={}",
                sel.id,
                sel.label.unwrap_or_else(|| "<none>".to_string())
            );
            Ok(())
        }
        Err(e) => {
            println!("env: failed: {e}");
            std::process::exit(2)
        }
    }
}

fn extract_chatgpt_account_id(token: &str) -> Option<String> {
    // JWT: header.payload.signature
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
