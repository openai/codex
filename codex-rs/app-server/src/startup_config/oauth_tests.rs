//! Exercises the startup-only connection against an isolated managed enrollment.

use super::oauth::serve;
use crate::config_manager::ConfigManager;
use codex_config::LoaderOverrides;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::DuplexStream;
use tokio::io::Lines;
use tokio::io::ReadHalf;
use tokio::io::WriteHalf;
use url::Url;

struct Client {
    lines: Lines<BufReader<ReadHalf<DuplexStream>>>,
    input: WriteHalf<DuplexStream>,
}

impl Client {
    async fn receive(&mut self) -> anyhow::Result<Value> {
        let line = self
            .lines
            .next_line()
            .await?
            .ok_or_else(|| anyhow::anyhow!("startup pipe closed"))?;
        Ok(serde_json::from_str(&line)?)
    }

    async fn send(&mut self, request: Value) -> anyhow::Result<()> {
        self.input
            .write_all(format!("{request}\n").as_bytes())
            .await?;
        Ok(())
    }
}

#[tokio::test]
async fn startup_gateway_oauth_cancels_pending_browser_login() -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let managed = home.path().join("managed_config.toml");
    let requirements = home.path().join("requirements.toml");
    std::fs::write(
        &requirements,
        r#"
[config_server]
base_url = "https://config.example.test"
[config_server.oauth]
authorization_url = "https://login.example.test/authorize"
token_url = "https://login.example.test/token"
client_id = "desktop"
"#,
    )?;
    let (client, server) = tokio::io::duplex(8192);
    let (read, write) = tokio::io::split(server);
    let task = tokio::spawn(async move {
        let manager = ConfigManager::new_for_tests(
            home.path().into(),
            vec![],
            LoaderOverrides::with_managed_config_path_for_tests(managed),
            Default::default(),
        );
        serve(manager, read, write).await
    });
    let (read, input) = tokio::io::split(client);
    let mut client = Client {
        lines: BufReader::new(read).lines(),
        input,
    };
    client
        .send(json!({
            "id": 1,
            "method": "account/gatewayOAuth/login",
            "params": {"target": {"kind": "configServer"}},
        }))
        .await?;
    let started = client.receive().await?;
    assert_eq!(started["method"], "account/gatewayOAuth/changed");
    assert_eq!(started["params"]["status"], "started");
    assert_eq!(started["params"]["target"], json!({"kind": "configServer"}));
    let url = Url::parse(started["params"]["authUrl"].as_str().unwrap())?;
    assert_eq!(url.host_str(), Some("login.example.test"));

    client
        .send(json!({"id": 2, "method": "account/gatewayOAuth/cancel"}))
        .await?;
    let cancelled = client.receive().await?;
    assert_eq!(cancelled["id"], 1);
    assert_eq!(cancelled["error"]["data"]["errorCode"], "LoginFailed");
    assert_eq!(client.receive().await?, json!({"id": 2, "result": {}}));
    drop(client);
    task.await??;
    Ok(())
}
