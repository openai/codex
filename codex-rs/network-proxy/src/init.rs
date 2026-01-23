use anyhow::Context;
use anyhow::Result;
use codex_core::config::find_codex_home;
use std::fs;
use tracing::info;

pub fn run_init() -> Result<()> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let root = codex_home.join("network_proxy");
    let mitm_dir = root.join("mitm");

    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    fs::create_dir_all(&mitm_dir)
        .with_context(|| format!("failed to create {}", mitm_dir.display()))?;

    info!("ensured {}", mitm_dir.display());
    Ok(())
}
