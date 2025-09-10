#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use base64::Engine;
use codex_backend_client::Client as BackendClient;
use codex_core::config::find_codex_home;
use codex_core::default_client::get_codex_user_agent;
use codex_login::AuthManager;
use codex_login::AuthMode;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Base URL (default to ChatGPT backend API)
    let base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL")
        .unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
    println!("base_url: {base_url}");
    let path_style = if base_url.contains("/backend-api") {
        "wham"
    } else {
        "codex-api"
    };
    println!("path_style: {path_style}");

    // Locate CODEX_HOME and try to load ChatGPT auth
    let codex_home = match find_codex_home() {
        Ok(p) => {
            println!("codex_home: {}", p.display());
            Some(p)
        }
        Err(e) => {
            println!("codex_home: <not found> ({e})");
            None
        }
    };

    // Build backend client with UA
    let ua = get_codex_user_agent(Some("codex_cloud_tasks_conncheck"));
    let mut client = BackendClient::new(base_url.clone())?.with_user_agent(ua);

    // Attach bearer token if available from ChatGPT auth
    let mut have_auth = false;
    if let Some(home) = codex_home {
        let authm = AuthManager::new(
            home,
            AuthMode::ChatGPT,
            "codex_cloud_tasks_conncheck".to_string(),
        );
        if let Some(auth) = authm.auth() {
            match auth.get_token().await {
                Ok(token) if !token.is_empty() => {
                    have_auth = true;
                    println!("auth: ChatGPT token present ({} chars)", token.len());
                    // Add Authorization header
                    client = client.with_bearer_token(&token);

                    // Attempt to extract ChatGPT account id from the JWT and set header.
                    if let Some(account_id) = extract_chatgpt_account_id(&token) {
                        println!("auth: ChatGPT-Account-Id: {account_id}");
                        client = client.with_chatgpt_account_id(account_id);
                    } else if let Some(acc) = auth.get_account_id() {
                        // Fallback: some older auth.jsons persist account_id
                        println!("auth: ChatGPT-Account-Id (from auth.json): {acc}");
                        client = client.with_chatgpt_account_id(acc);
                    }
                }
                Ok(_) => {
                    println!("auth: ChatGPT token empty");
                }
                Err(e) => {
                    println!("auth: failed to load ChatGPT token: {e}");
                }
            }
        } else {
            println!("auth: no ChatGPT auth.json");
        }
    }

    if !have_auth {
        println!("note: Online endpoints typically require ChatGPT sign-in. Run: `codex login`");
    }

    // Attempt the /list call with a short timeout to avoid hanging
    match path_style {
        "wham" => println!("request: GET /wham/tasks/list?limit=5&task_filter=current"),
        _ => println!("request: GET /api/codex/tasks/list?limit=5&task_filter=current"),
    }
    let fut = client.list_tasks(Some(5), Some("current"), None);
    let res = tokio::time::timeout(Duration::from_secs(30), fut).await;
    match res {
        Err(_) => {
            println!("error: request timed out after 30s");
            std::process::exit(2);
        }
        Ok(Err(e)) => {
            // backend-client includes HTTP status and body in errors.
            println!("error: {e}");
            std::process::exit(1);
        }
        Ok(Ok(list)) => {
            println!("ok: received {} tasks", list.items.len());
            for item in list.items.iter().take(5) {
                println!("- {} — {}", item.id, item.title);
            }
            // Print the full response object for debugging/inspection.
            match serde_json::to_string_pretty(&list) {
                Ok(json) => {
                    println!("\nfull response object (pretty JSON):\n{json}");
                }
                Err(e) => {
                    println!("failed to serialize response to JSON: {e}");
                }
            }
        }
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
        .map(|s| s.to_string())
}
