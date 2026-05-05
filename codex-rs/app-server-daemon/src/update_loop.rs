use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use tokio::process::Command;
use tokio::time::sleep;

use crate::Daemon;

const INITIAL_UPDATE_DELAY: Duration = Duration::from_secs(5 * 60);
const UPDATE_INTERVAL: Duration = Duration::from_secs(60 * 60);

pub(crate) async fn run() -> Result<()> {
    sleep(INITIAL_UPDATE_DELAY).await;
    loop {
        let _ = update_once().await;
        sleep(UPDATE_INTERVAL).await;
    }
}

async fn update_once() -> Result<()> {
    install_latest_standalone().await?;

    let daemon = Daemon::from_environment()?;
    daemon.restart_if_running().await?;
    Ok(())
}

async fn install_latest_standalone() -> Result<()> {
    let status = Command::new("/bin/sh")
        .args([
            "-c",
            "tmp=$(mktemp) && trap 'rm -f \"$tmp\"' EXIT && curl -fsSL https://chatgpt.com/codex/install.sh -o \"$tmp\" && sh \"$tmp\"",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .context("failed to invoke standalone Codex updater")?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("standalone Codex updater exited with status {status}")
    }
}
