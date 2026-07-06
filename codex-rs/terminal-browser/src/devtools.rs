use anyhow::Context;
use anyhow::Result;

use crate::cdp::CdpClient;

pub(crate) async fn deny_downloads(client: &CdpClient) -> Result<()> {
    if client
        .call_browser(
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
