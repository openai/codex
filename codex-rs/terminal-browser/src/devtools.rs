use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use url::Host;
use url::Url;

use crate::cdp::CdpClient;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DevtoolsTarget {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    title: String,
    web_socket_debugger_url: Option<String>,
}

pub(crate) struct PageTarget {
    pub(crate) title: String,
    pub(crate) websocket_url: String,
}

pub(crate) fn prepare_devtools_active_port(profile: &Path) -> Result<()> {
    let active_port_path = profile.join("DevToolsActivePort");
    let metadata = match std::fs::symlink_metadata(&active_port_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).context("inspect Carbonyl DevToolsActivePort before startup");
        }
    };
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "refusing symbolic-link Carbonyl DevToolsActivePort"
    );
    anyhow::ensure!(
        metadata.is_file(),
        "refusing non-file Carbonyl DevToolsActivePort"
    );
    std::fs::remove_file(&active_port_path)
        .context("remove stale Carbonyl DevToolsActivePort before startup")
}

pub(crate) async fn discover_page_target(profile: &Path) -> Result<PageTarget> {
    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(/*secs*/ 2))
        .build()?;
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 12);
    let active_port_path = profile.join("DevToolsActivePort");
    let port = loop {
        match std::fs::read_to_string(&active_port_path) {
            Ok(contents) => {
                let port = contents
                    .lines()
                    .next()
                    .context("DevToolsActivePort did not contain a port")?
                    .parse::<u16>()
                    .context("DevToolsActivePort contained an invalid port")?;
                anyhow::ensure!(port != 0, "DevToolsActivePort contained port zero");
                break port;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).context("read Carbonyl DevToolsActivePort");
            }
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "timed out waiting for Carbonyl DevToolsActivePort"
        );
        tokio::time::sleep(Duration::from_millis(/*millis*/ 100)).await;
    };
    let endpoint = format!("http://127.0.0.1:{port}/json/list");
    loop {
        if let Ok(response) = client.get(&endpoint).send().await
            && let Ok(targets) = response.json::<Vec<DevtoolsTarget>>().await
        {
            for target in targets {
                if target.kind != "page" {
                    continue;
                }
                let Some(websocket_url) = target.web_socket_debugger_url else {
                    continue;
                };
                return Ok(PageTarget {
                    title: target.title,
                    websocket_url: validated_websocket_url(&websocket_url, port)?,
                });
            }
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "timed out waiting for Carbonyl DevTools on {endpoint}"
        );
        tokio::time::sleep(Duration::from_millis(/*millis*/ 100)).await;
    }
}

pub(crate) fn validated_websocket_url(websocket_url: &str, expected_port: u16) -> Result<String> {
    let parsed = Url::parse(websocket_url).context("parse Carbonyl DevTools WebSocket URL")?;
    anyhow::ensure!(parsed.scheme() == "ws", "Carbonyl DevTools URL must use ws");
    let loopback = match parsed.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(_)) | None => false,
    };
    anyhow::ensure!(loopback, "Carbonyl DevTools URL must use a loopback host");
    anyhow::ensure!(
        parsed.port() == Some(expected_port),
        "Carbonyl DevTools URL used an unexpected port"
    );
    Ok(parsed.to_string())
}

pub(crate) async fn deny_downloads(client: &CdpClient) -> Result<()> {
    if client
        .call(
            "Browser.setDownloadBehavior",
            serde_json::json!({ "behavior": "deny" }),
        )
        .await
        .is_ok()
    {
        return Ok(());
    }
    client
        .call(
            "Page.setDownloadBehavior",
            serde_json::json!({ "behavior": "deny" }),
        )
        .await
        .context("disable Carbonyl downloads")?;
    Ok(())
}
