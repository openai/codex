#![deny(clippy::unwrap_used, clippy::expect_used)]

use codex_backend_client::Client as BackendClient;
use codex_core::config::find_codex_home;
use codex_core::default_client::get_codex_user_agent;
use codex_login::AuthManager;
use codex_login::AuthMode;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL")
        .unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
    let ua = get_codex_user_agent(Some("codex_cloud_tasks_detailcheck"));
    let mut client = BackendClient::new(base_url)?.with_user_agent(ua);

    if let Ok(home) = find_codex_home() {
        let am = AuthManager::new(
            home,
            AuthMode::ChatGPT,
            "codex_cloud_tasks_detailcheck".to_string(),
        );
        if let Some(auth) = am.auth()
            && let Ok(tok) = auth.get_token().await
        {
            client = client.with_bearer_token(tok);
        }
    }

    let list = client.list_tasks(Some(5), Some("current"), None).await?;
    println!("items: {}", list.items.len());
    for item in list.items.iter().take(5) {
        println!("item: {} {}", item.id, item.title);
        let (details, body, ct) = client.get_task_details_with_body(&item.id).await?;
        let diff = codex_backend_client::CodeTaskDetailsResponseExt::unified_diff(&details);
        match diff {
            Some(d) => println!(
                "unified diff len={} sample=\n{}",
                d.len(),
                &d.lines().take(10).collect::<Vec<_>>().join("\n")
            ),
            None => {
                println!(
                    "no unified diff found; ct={ct}; body sample=\n{}",
                    &body.chars().take(5000).collect::<String>()
                );
            }
        }
    }
    Ok(())
}
