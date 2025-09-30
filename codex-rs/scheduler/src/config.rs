use anyhow::{Context, Result};
use dirs::home_dir;
use serde::Deserialize;
use std::{path::PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerJob {
    pub id: String,
    pub cron: String,
    pub prompt: String,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub profile: Option<String>,
    pub sandbox: Option<String>,
    pub approval_policy: Option<String>,
    pub max_duration_seconds: Option<u64>,
    pub retry_limit: Option<u32>,
    pub retry_backoff_seconds: Option<u64>,
    pub resume_conversation_id: Option<String>,
    pub tags: Option<Vec<String>>, 
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerNotifications {
    pub enabled: Option<bool>,
    pub on: Option<Vec<String>>,               // kinds to inject on
    pub default_conversation_id: Option<String>,
    pub routes: Option<std::collections::HashMap<String, String>>, // job_id -> conv_id
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerConfig {
    pub enabled: bool,
    pub max_concurrency: Option<usize>,
    pub poll_interval_seconds: Option<u64>,
    pub lock_key: Option<String>,
    pub lock_ttl_seconds: Option<u64>,

    pub default_model: Option<String>,
    pub default_profile: Option<String>,
    pub default_sandbox: Option<String>,
    pub default_approval_policy: Option<String>,
    pub default_cwd: Option<String>,
    pub default_max_duration_seconds: Option<u64>,
    pub default_retry_limit: Option<u32>,
    pub default_retry_backoff_seconds: Option<u64>,

    pub notifications: Option<SchedulerNotifications>,
    pub jobs: Vec<SchedulerJob>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArangoConfig {
    pub url: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub runs_collection: String,
    pub events_collection: String,
    pub notifications_collection: String,
    pub state_collection: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RootToml {
    pub scheduler: Option<SchedulerConfig>,
    #[serde(rename = "database.arango")]
    pub database_arango: Option<ArangoConfig>,
}

pub async fn load_scheduler_config_from_toml(path: Option<PathBuf>) -> Result<Option<(SchedulerConfig, ArangoConfig)>> {
    let cfg_path = if let Some(p) = path { p } else {
        let mut p = home_dir().context("cannot locate home dir")?;
        p.push(".codex");
        p.push("config.toml");
        p
    };
    let data = tokio::fs::read(&cfg_path).await
        .with_context(|| format!("failed reading {:?}", cfg_path))?;
    let toml_str = String::from_utf8_lossy(&data);
    let root: RootToml = toml::from_str(&toml_str)
        .with_context(|| format!("failed parsing TOML {:?}", cfg_path))?;
    let Some(sched) = root.scheduler else { return Ok(None); };
    if !sched.enabled { return Ok(None); }
    let arango = root.database_arango.context("missing [database.arango] config while scheduler is enabled")?;
    Ok(Some((sched, arango)))
}

#[allow(dead_code)]
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
#[allow(dead_code)]
pub fn plus_seconds_iso(secs: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::seconds(secs)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

