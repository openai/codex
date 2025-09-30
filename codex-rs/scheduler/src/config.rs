use anyhow::{anyhow, Context, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchedulerNotifications {
    pub enabled: Option<bool>,
    pub on: Option<Vec<String>>, // kinds to inject on
    pub default_conversation_id: Option<String>,
    pub routes: Option<std::collections::HashMap<String, String>>, // job_id -> conv_id
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

    /// Enforce HTTPS and rustls; when true, http:// URLs will be rejected
    /// even if `allow_insecure=true` on the ArangoDB config.
    #[serde(default)]
    pub strict_tls: bool,

    /// Proximity window (ms) within which a cron is considered due on a poll tick.
    #[serde(default = "default_due_window_ms")]
    pub due_window_ms: u64,

    /// Max write retries for notify/events batch.
    #[serde(default = "default_max_retries")]
    pub max_write_retries: usize,
    /// Base backoff in ms (jittered exponential).
    #[serde(default = "default_backoff_base_ms")]
    pub backoff_base_ms: u64,
    /// Jitter ratio (0.0 - 1.0) applied to backoff (default 0.25).
    #[serde(default = "default_backoff_jitter")]
    pub backoff_jitter: f32,

    pub notifications: Option<SchedulerNotifications>,
    #[serde(default)]
    pub jobs: Vec<SchedulerJob>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ArangoConfig {
    pub url: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub runs_collection: String,
    pub events_collection: String,
    pub notifications_collection: String,
    pub state_collection: String,
    /// Allow http:// URLs only when explicitly set. Default: false
    #[serde(default)]
    pub allow_insecure: bool,
}

impl std::fmt::Debug for ArangoConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArangoConfig")
            .field("url", &self.url)
            .field("database", &self.database)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("runs_collection", &self.runs_collection)
            .field("events_collection", &self.events_collection)
            .field("notifications_collection", &self.notifications_collection)
            .field("state_collection", &self.state_collection)
            .field("allow_insecure", &self.allow_insecure)
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RootToml {
    pub scheduler: Option<SchedulerConfig>,
    #[serde(rename = "database.arango")]
    pub database_arango: Option<ArangoConfig>,
}

pub async fn load_scheduler_config_from_toml(
    path: Option<PathBuf>,
) -> Result<Option<(SchedulerConfig, ArangoConfig)>> {
    let cfg_path = if let Some(p) = path {
        p
    } else {
        let mut p = home_dir().context("cannot locate home dir")?;
        p.push(".codex");
        p.push("config.toml");
        p
    };
    let data = tokio::fs::read(&cfg_path)
        .await
        .with_context(|| format!("failed reading {:?}", cfg_path))?;
    let toml_str = String::from_utf8_lossy(&data);
    let root: RootToml =
        toml::from_str(&toml_str).with_context(|| format!("failed parsing TOML {:?}", cfg_path))?;
    let Some(sched) = root.scheduler else {
        return Ok(None);
    };
    if !sched.enabled {
        return Ok(None);
    }
    let arango = root
        .database_arango
        .context("missing [database.arango] config while scheduler is enabled")?;
    Ok(Some((sched, arango)))
}

#[allow(dead_code)]
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
#[allow(dead_code)]
pub fn plus_seconds_iso(secs: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::seconds(secs))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

impl SchedulerConfig {
    /// Parse from a TOML string and validate URL scheme according to
    /// `strict_tls` and `database.arango.allow_insecure`.
    pub fn from_toml(src: &str) -> Result<(Self, ArangoConfig)> {
        #[derive(Deserialize)]
        struct DbBlock {
            arango: Option<ArangoConfig>,
        }
        #[derive(Deserialize)]
        struct Root2 {
            scheduler: Option<SchedulerConfig>,
            database: Option<DbBlock>,
        }

        let root: Root2 = toml::from_str(src)?;
        let sched = root
            .scheduler
            .ok_or_else(|| anyhow!("missing [scheduler] section"))?;
        if !sched.enabled {
            return Err(anyhow!("scheduler disabled"));
        }
        let arango = root
            .database
            .and_then(|d| d.arango)
            .ok_or_else(|| anyhow!("missing [database.arango] section"))?;

        // URL scheme validation
        let strict = sched.strict_tls;
        if strict && !arango.url.starts_with("https://") {
            return Err(anyhow!(
                "strict_tls=true requires Arango URL to be https://"
            ));
        }
        if !(arango.url.starts_with("https://")
            || (arango.allow_insecure && arango.url.starts_with("http://")))
        {
            return Err(anyhow!(
                "Arango URL must be https:// (set allow_insecure=true to permit http:// for local dev)"
            ));
        }

        if !(0.0..=1.0).contains(&sched.backoff_jitter) {
            return Err(anyhow!("backoff_jitter must be between 0.0 and 1.0"));
        }

        Ok((sched, arango))
    }
}

fn default_due_window_ms() -> u64 {
    5_000
}
fn default_max_retries() -> usize {
    5
}
fn default_backoff_base_ms() -> u64 {
    200
}
fn default_backoff_jitter() -> f32 {
    0.25
}
