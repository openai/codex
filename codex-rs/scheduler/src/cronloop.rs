use crate::config::SchedulerConfig;
use crate::config::SchedulerJob;
use crate::db::Db;
use crate::runner::execute_run;
use crate::runner::RunRequest;
use anyhow::Context;
use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use cron::Schedule;
use hostname;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::sleep;
use tracing::warn;
use uuid::Uuid;

pub async fn run(cfg: SchedulerConfig, db: Db) -> Result<()> {
    let owner = format!(
        "{}-{}",
        hostname::get().unwrap_or_default().to_string_lossy(),
        Uuid::new_v4()
    );
    let lock_key = cfg
        .lock_key
        .clone()
        .unwrap_or_else(|| "codex-scheduler".to_string());
    let lock_ttl = cfg.lock_ttl_seconds.unwrap_or(60);
    let poll_secs = cfg.poll_interval_seconds.unwrap_or(5);
    let due_window = std::time::Duration::from_millis(cfg.due_window_ms.max(1));

    let sem = Arc::new(Semaphore::new(cfg.max_concurrency.unwrap_or(2)));
    let mut compiled: Vec<(SchedulerJob, Schedule)> = vec![];
    for j in &cfg.jobs {
        let sched = Schedule::from_str(&j.cron)
            .with_context(|| format!("invalid cron for job {}", j.id))?;
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
                if is_due_within(next, now, due_window) {
                    if let Ok(permit) = sem.clone().try_acquire_owned() {
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

/// Returns true if `next_due` is non-negative and within `window` from `now`.
/// This isolates the proximity decision for unit testing and consistent behavior.
pub(crate) fn is_due_within(
    next_due: DateTime<Utc>,
    now: DateTime<Utc>,
    window: std::time::Duration,
) -> bool {
    let delta = next_due - now;
    if delta.num_milliseconds() < 0 {
        return false;
    }
    match delta.to_std() {
        Ok(d) => d <= window,
        Err(_) => false,
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
        .aql_public(
            q,
            serde_json::json!({
                "@state": db.state_collection(),
                "key": key,
                "owner": owner,
                "ttl_ms": ttl_ms
            }),
        )
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
        .aql_public(
            q,
            serde_json::json!({
                "@state": db.state_collection(),
                "key": key,
                "owner": owner,
                "ttl_ms": ttl_ms
            }),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn proximity_detection_basic() {
        let now = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap();
        let within = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 2).unwrap();
        let outside = Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 6).unwrap();
        assert!(is_due_within(
            within,
            now,
            std::time::Duration::from_secs(5)
        ));
        assert!(!is_due_within(
            outside,
            now,
            std::time::Duration::from_secs(5)
        ));
    }
}
