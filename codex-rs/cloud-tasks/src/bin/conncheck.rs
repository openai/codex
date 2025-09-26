use codex_cloud_tasks::util::build_chatgpt_headers;
use codex_cloud_tasks::util::normalize_base_url;
use codex_cloud_tasks::util::set_user_agent_suffix;
use codex_cloud_tasks_client::CloudBackend;
use codex_cloud_tasks_client::HttpClient;
use codex_cloud_tasks_client::MockClient;
use reqwest::header::AUTHORIZATION;
use reqwest::header::HeaderName;
use reqwest::header::USER_AGENT;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    set_user_agent_suffix("codex_cloud_tasks_conncheck");

    let raw_base = std::env::var("CODEX_CLOUD_TASKS_BASE_URL")
        .unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
    let base_url = normalize_base_url(&raw_base);
    println!("base_url: {base_url}");
    let path_style = if base_url.contains("/backend-api") {
        "wham"
    } else {
        "codex-api"
    };
    println!("path_style: {path_style}");

    let use_mock = matches!(
        std::env::var("CODEX_CLOUD_TASKS_MODE").ok().as_deref(),
        Some(mode) if mode.eq_ignore_ascii_case("mock")
    );

    let backend: Arc<dyn CloudBackend> = if use_mock {
        println!("mode: mock (no network calls)");
        Arc::new(MockClient)
    } else {
        println!("mode: online");
        let mut client = HttpClient::new(base_url.clone())?;
        let headers = build_chatgpt_headers().await;

        if let Some(ua) = headers.get(USER_AGENT).and_then(|v| v.to_str().ok()) {
            client = client.with_user_agent(ua.to_string());
        }

        let mut authed = false;
        if let Some(auth) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) {
            let token = auth.strip_prefix("Bearer ").unwrap_or(auth);
            if !token.is_empty() {
                client = client.with_bearer_token(token.to_string());
                authed = true;
            }
        }

        let account_header = HeaderName::from_static("chatgpt-account-id");
        if let Some(acc) = headers.get(&account_header).and_then(|v| v.to_str().ok())
            && !acc.is_empty() {
                client = client.with_chatgpt_account_id(acc.to_string());
            }

        if authed {
            println!("auth: bearer token configured");
        } else {
            println!("auth: none (run `codex login` for online mode)");
        }

        Arc::new(client)
    };

    // Limit request time to keep diagnostics snappy in CI.
    let result = timeout(Duration::from_secs(30), backend.list_tasks(None)).await;
    match result {
        Err(_) => {
            println!("error: request timed out after 30s");
            std::process::exit(2);
        }
        Ok(Err(e)) => {
            println!("error: {e}");
            std::process::exit(1);
        }
        Ok(Ok(tasks)) => {
            println!("ok: received {} tasks", tasks.len());
            for task in tasks.iter().take(5) {
                println!("- {} — {}", task.id.0, task.title);
            }
        }
    }

    Ok(())
}
