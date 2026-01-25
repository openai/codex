use anyhow::Context;
use anyhow::Result;
use codex_core::config::find_codex_home;
use std::fs;
use tracing::info;

pub fn run_init() -> Result<()> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let proxy_dir = codex_home.join("proxy");

    fs::create_dir_all(&proxy_dir)
        .with_context(|| format!("failed to create {}", proxy_dir.display()))?;

    info!("ensured {}", proxy_dir.display());
    Ok(())
}
