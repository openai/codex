use base64::Engine;
use clap::Parser;
use codex_cloud_tasks::util::set_user_agent_suffix;
use codex_core::config::find_codex_home;
use codex_core::default_client::get_codex_user_agent;
use codex_login::AuthManager;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;

#[derive(Debug, Parser)]
#[command(version, about = "Create a new Codex cloud task (debug helper)")]
struct Args {
    /// Optional override for environment id; if absent we auto-detect.
    #[arg(long = "env-id")]
    environment_id: Option<String>,
    /// Optional label match for environment selection (case-insensitive, exact match).
    #[arg(long = "env-label")]
    environment_label: Option<String>,
    /// Branch or ref to use (e.g., main)
    #[arg(long = "ref", default_value = "main")]
    git_ref: String,
    /// Run environment in QA (ask) mode
    #[arg(long = "qa-mode", default_value_t = false)]
    qa_mode: bool,
    /// Task prompt text
    #[arg(required = true)]
    prompt: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let prompt = args.prompt.join(" ");

    // Base URL (default to ChatGPT backend API)
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
    let is_wham = base_url.contains("/backend-api");
    println!("path_style: {}", if is_wham { "wham" } else { "codex-api" });

    // Build headers: UA + ChatGPT auth if available
    set_user_agent_suffix("codex_cloud_tasks_newtask");
    let ua = get_codex_user_agent();
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        HeaderValue::from_str(&ua).unwrap_or(HeaderValue::from_static("codex-cli")),
    );
    let mut have_auth = false;
    // Locate CODEX_HOME and try to load ChatGPT auth
    if let Ok(home) = find_codex_home() {
        let authm = AuthManager::new(home);
        if let Some(auth) = authm.auth() {
            match auth.get_token().await {
                Ok(token) if !token.is_empty() => {
                    have_auth = true;
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
    }
    if !have_auth {
        println!("note: Online endpoints typically require ChatGPT sign-in. Run: `codex login`");
    }

    // Determine environment id: prefer flag, then by-repo lookup, then full list.
    let env_id = if let Some(id) = args.environment_id.clone() {
        println!("env: using provided env-id={id}");
        id
    } else {
        match codex_cloud_tasks::env_detect::autodetect_environment_id(
            &base_url,
            &headers,
            args.environment_label.clone(),
        )
        .await
        {
            Ok(sel) => sel.id,
            Err(e) => {
                println!("env: failed to auto-detect environment: {e}");
                std::process::exit(2);
            }
        }
    };
    println!("env: selected environment_id={env_id}");

    // Build request payload patterned after VSCode: POST /wham/tasks
    let url = if is_wham {
        format!("{base_url}/wham/tasks")
    } else {
        format!("{base_url}/api/codex/tasks")
    };
    println!(
        "request: POST {}",
        url.strip_prefix(&base_url).unwrap_or(&url)
    );

    // input_items
    let mut input_items: Vec<serde_json::Value> = Vec::new();
    input_items.push(serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [{ "content_type": "text", "text": prompt }]
    }));

    // Optional: starting diff via env var for quick testing
    if let Ok(diff) = std::env::var("CODEX_STARTING_DIFF")
        && !diff.is_empty()
    {
        input_items.push(serde_json::json!({
            "type": "pre_apply_patch",
            "output_diff": { "diff": diff }
        }));
    }

    let request_body = serde_json::json!({
        "new_task": {
            "environment_id": env_id,
            "branch": args.git_ref,
            "run_environment_in_qa_mode": args.qa_mode,
        },
        "input_items": input_items,
    });

    let http = reqwest::Client::builder().build()?;
    let res = http
        .post(&url)
        .headers(headers)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .json(&request_body)
        .send()
        .await?;

    let status = res.status();
    let ct = res
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = res.text().await.unwrap_or_default();
    println!("status: {status}");
    println!("content-type: {ct}");
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(v) => println!(
            "response (pretty JSON):\n{}",
            serde_json::to_string_pretty(&v).unwrap_or(body)
        ),
        Err(_) => println!("response (raw):\n{body}"),
    }

    if !status.is_success() {
        // Exit non-zero on failure
        std::process::exit(1);
    }
    Ok(())
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
