pub mod config;
pub mod cronloop;
pub mod db;
pub mod runner;

use anyhow::Result;
use config::load_scheduler_config_from_toml;
use db::Db;
use tokio::task::JoinHandle;
use tracing::info;

pub struct SchedulerHandles {
    pub cron_handle: JoinHandle<()>,
}

/// Start the scheduler if `[scheduler]` is enabled in `~/.codex/config.toml`.
/// Returns `Ok(None)` when disabled or config is missing.
pub async fn start_scheduler_if_configured() -> Result<Option<SchedulerHandles>> {
    let maybe_cfg = load_scheduler_config_from_toml(None).await?;
    let Some((sched_cfg, arango_cfg)) = maybe_cfg else {
        info!("scheduler: not configured/enabled; skipping start");
        return Ok(None);
    };

    let db = Db::from_config(&sched_cfg, &arango_cfg)?;
    db.ensure_collections_and_indexes().await?;

    let cron_handle = tokio::spawn(async move {
        if let Err(e) = cronloop::run(sched_cfg, db).await {
            tracing::error!("scheduler: cron loop exited with error: {e:#}");
        }
    });

    Ok(Some(SchedulerHandles { cron_handle }))
}
