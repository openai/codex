use crate::db::{Db, RunDoc};
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::Value;
use std::process::Stdio;
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, time::{timeout, Duration}};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RunRequest {
    pub job_id: String,
    pub prompt: String,
    pub cwd: Option<String>,
    pub resume_conversation_id: Option<String>,
    pub max_duration_seconds: u64,
    pub tags: Option<Vec<String>>,
}

pub async fn execute_run(req: RunRequest, db: Db) -> Result<()> {
    let run_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    db.insert_run_started(&RunDoc {
        run_id: run_id.clone(),
        job_id: req.job_id.clone(),
        created_at: created_at.clone(),
        started_at: Some(created_at.clone()),
        finished_at: None,
        status: "running".into(),
        conversation_id: req.resume_conversation_id.clone(),
        submission_id: None,
        prompt: req.prompt.clone(),
        model: None,
        sandbox: None,
        approval_policy: None,
        cwd: req.cwd.clone(),
        error_message: None,
        tags: req.tags.clone(),
    }).await?;

    let _ = db.notify(&run_id, &req.job_id, "job_started", 120).await;

    let mut args: Vec<String> = vec!["exec".into(), "--experimental-json".into(), req.prompt.clone()];
    if let Some(thread) = &req.resume_conversation_id {
        args = vec!["exec".into(), "--experimental-json".into(), "resume".into(), thread.clone(), req.prompt.clone()];
    }

    let mut cmd = Command::new(which::which("codex").unwrap_or_else(|_| "codex".into()));
    cmd.args(&args);
    if let Some(cwd) = &req.cwd { cmd.current_dir(cwd); }
    cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());

    let mut child = cmd.spawn().context("spawn codex exec")?;
    let stdout = child.stdout.take().context("no stdout from codex exec")?;
    let mut reader = BufReader::new(stdout).lines();

    let run_future = async {
        let mut seq: i64 = 0;
        let mut batch: Vec<(i64, String, Value)> = Vec::with_capacity(64);
        while let Some(line) = reader.next_line().await? {
            let (typ, payload) = match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(v) => (v.get("type").and_then(|t| t.as_str()).unwrap_or("event").to_string(), v),
                Err(_) => ("raw_line".to_string(), serde_json::json!({"line": line})),
            };
            batch.push((seq, typ, payload));
            seq += 1;
            if batch.len() >= 50 { db.insert_events_batch(&run_id, &batch).await.ok(); batch.clear(); }
        }
        if !batch.is_empty() { db.insert_events_batch(&run_id, &batch).await.ok(); }
        Ok::<(), anyhow::Error>(())
    };

    let status = match timeout(Duration::from_secs(req.max_duration_seconds), run_future).await {
        Ok(Ok(())) => {
            let exit = child.wait().await?;
            if exit.success() { "succeeded" } else { "failed" }
        }
        Ok(Err(e)) => { db.update_run_finished(&run_id, "failed", Some(&e.to_string())).await.ok(); "failed" }
        Err(_) => { let _ = child.kill().await; "cancelled" }
    };

    db.update_run_finished(&run_id, status, None).await.ok();
    let _ = db.notify(&run_id, &req.job_id, &format!("job_{}", status), 300).await;
    info!("scheduler: job {} finished with status {}", req.job_id, status);
    Ok(())
}

