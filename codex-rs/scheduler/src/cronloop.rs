use crate::config::{SchedulerConfig, SchedulerJob};
use crate::db::Db;
use crate::runner::{execute_run, RunRequest};
use anyhow::{Context, Result};
use cron::Schedule;
use hostname;
use std::{str::FromStr, time::Duration};
use tokio::{sync::Semaphore, time::sleep};
use tracing::warn;
use uuid::Uuid;

pub async fn run(cfg: SchedulerConfig, db: Db) -> Result<()> {
    let owner = format!(
        "{}-{}",
        hostname::get().unwrap_or_default().to_string_lossy(),
        Uuid::new_v4()
    );
    let lock_key = cfg.lock_key.clone().unwrap_or_else(|| "codex-scheduler".to_string());
    let lock_ttl = cfg.lock_ttl_seconds.unwrap_or(60);
    let poll_secs = cfg.poll_interval_seconds.unwrap_or(5);

    let sem = Semaphore::new(cfg.max_concurrency.unwrap_or(2));
    let mut compiled: Vec<(SchedulerJob, Schedule)> = vec![];
    for j in &cfg.jobs {
        let sched = Schedule::from_str(&j.cron).with_context(|| format!("invalid cron for job {}", j.id))?;
        compiled.push((j.clone(), sched));
    }

    loop {
        if !try_acquire_lock(&db, &lock_key, &owner, lock_ttl).await? {
            sleep(Duration::from_secs(poll_secs)).await;
            continue;
        }
        let _ = renew_lock(&db, &lock_key, &owner, lock_ttl).await;

        let now = chrono::Utc::now();
        for (job, schedule) in &compiled {
            if let Some(next) = schedule.after(&now).next() {
                let delta = (now - next).num_seconds().abs();
                if delta <= poll_secs as i64 {
                    if let Ok(permit) = sem.try_acquire() {
                        let dbc = db.clone();
                        let jobc = job.clone();
                        tokio::spawn(async move {
                            let _g = permit;
                            let req = RunRequest {
                                job_id: jobc.id.clone(),
                                prompt: jobc.prompt.clone(),
                                cwd: jobc.cwd.clone(),
                                resume_conversation_id: jobc.resume_conversation_id.clone(),
                                max_duration_seconds: jobc.max_duration_seconds.unwrap_or(900),
                                tags: jobc.tags.clone(),
                            };
                            if let Err(e) = execute_run(req, dbc).await {
                                warn!("scheduler: job {} failed: {e:#}", jobc.id);
                            }
                        });
                    }
                }
            }
        }

        sleep(Duration::from_secs(poll_secs)).await;
    }
}

async fn try_acquire_lock(db: &Db, key: &str, owner: &str, ttl_secs: u64) -> Result<bool> {
    let q = r#"
LET now = DATE_NOW()
LET expiry = DATE_ADD(DATE_NOW(), @ttl_ms, 'millisecond')
LET doc = DOCUMENT(@@state, @key)
LET can_acquire = doc == null OR doc.expiresAt < DATE_ISO8601(now) OR doc.owner_id == @owner
IF can_acquire
  UPSERT { _key: @key }
  INSERT { _key: @key, owner_id: @owner, heartbeat: DATE_ISO8601(now), expiresAt: DATE_ISO8601(expiry) }
  UPDATE { owner_id: @owner, heartbeat: DATE_ISO8601(now), expiresAt: DATE_ISO8601(expiry) }
  IN @@state
  RETURN { acquired: true }
ELSE
  RETURN { acquired: false }
"#;
    let ttl_ms = (ttl_secs as i64) * 1000;
    let resp = db
        .aql_public(q, serde_json::json!({
            "@state": db.state_collection(),
            "key": key,
            "owner": owner,
            "ttl_ms": ttl_ms
        }))
        .await?;
    let acquired = resp
        .pointer("/result/0/acquired")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Ok(acquired)
}

async fn renew_lock(db: &Db, key: &str, owner: &str, ttl_secs: u64) -> Result<()> {
    let q = r#"
LET now = DATE_NOW()
LET expiry = DATE_ADD(DATE_NOW(), @ttl_ms, 'millisecond')
UPDATE { _key: @key } WITH { owner_id: @owner, heartbeat: DATE_ISO8601(now), expiresAt: DATE_ISO8601(expiry) } IN @@state
"#;
    let ttl_ms = (ttl_secs as i64) * 1000;
    let _ = db
        .aql_public(q, serde_json::json!({
            "@state": db.state_collection(),
            "key": key,
            "owner": owner,
            "ttl_ms": ttl_ms
        }))
        .await?;
    Ok(())
}

