use crate::config::ArangoConfig;
use anyhow::{bail, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone)]
pub struct Db {
    pub(crate) base_url: String,
    pub(crate) database: String,
    runs: String,
    events: String,
    notes: String,
    state: String,
    client: Client,
    auth: (String, String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDoc {
    pub run_id: String,
    pub job_id: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub status: String,
    pub conversation_id: Option<String>,
    pub submission_id: Option<String>,
    pub prompt: String,
    pub model: Option<String>,
    pub sandbox: Option<String>,
    pub approval_policy: Option<String>,
    pub cwd: Option<String>,
    pub error_message: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationDoc {
    pub _key: Option<String>,
    pub run_id: String,
    pub job_id: String,
    pub kind: String,
    pub created_at: String,
}

impl Db {
    pub async fn new(cfg: &ArangoConfig) -> Result<Self> {
        Ok(Self {
            base_url: cfg.url.clone(),
            database: cfg.database.clone(),
            runs: cfg.runs_collection.clone(),
            events: cfg.events_collection.clone(),
            notes: cfg.notifications_collection.clone(),
            state: cfg.state_collection.clone(),
            client: Client::builder().build()?,
            auth: (cfg.username.clone(), cfg.password.clone()),
        })
    }

    pub fn state_collection(&self) -> &str { &self.state }

    fn col_url(&self, col: &str) -> String {
        format!("{}/_db/{}/_api/document/{}", self.base_url, self.database, col)
    }
    fn cursor_url(&self) -> String {
        format!("{}/_db/{}/_api/cursor", self.base_url, self.database)
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value> {
        let res = self.client.post(url)
            .basic_auth(self.auth.0.clone(), Some(self.auth.1.clone()))
            .json(body)
            .send().await?;
        let status = res.status();
        let val: Value = res.json().await.unwrap_or(json!({"error":"invalid json"}));
        if !status.is_success() {
            bail!("arangodb POST {} failed: {} body={}", url, status, val);
        }
        Ok(val)
    }

    pub async fn aql_public(&self, query: &str, bind_vars: Value) -> Result<Value> {
        let body = json!({"query": query, "bindVars": bind_vars});
        self.post_json(&self.cursor_url(), &body).await
    }

    pub async fn ensure_collections_and_indexes(&self) -> Result<()> {
        // Create collections if not exist (best-effort)
        for col in [&self.runs, &self.events, &self.notes, &self.state] {
            let url = format!("{}/_db/{}/_api/collection", self.base_url, self.database);
            let body = json!({"name": col, "type": 2});
            let _ = self.client.post(&url)
                .basic_auth(self.auth.0.clone(), Some(self.auth.1.clone()))
                .json(&body)
                .send().await;
        }

        // Indexes (best-effort)
        let _ = self.aql_public(
            "RETURN ENSURE_INDEX(@col, { type: 'persistent', fields: ['job_id'] })",
            json!({"col": self.runs}),
        ).await;
        let _ = self.aql_public(
            "RETURN ENSURE_INDEX(@col, { type: 'persistent', fields: ['created_at'] })",
            json!({"col": self.runs}),
        ).await;
        let _ = self.aql_public(
            "RETURN ENSURE_INDEX(@col, { type: 'persistent', fields: ['status'] })",
            json!({"col": self.runs}),
        ).await;
        let _ = self.aql_public(
            "RETURN ENSURE_INDEX(@col, { type: 'persistent', fields: ['run_id','seq'], unique: true })",
            json!({"col": self.events}),
        ).await;
        let _ = self.aql_public(
            "RETURN ENSURE_INDEX(@col, { type: 'persistent', fields: ['created_at'] })",
            json!({"col": self.notes}),
        ).await;
        Ok(())
    }

    pub async fn insert_run_started(&self, doc: &RunDoc) -> Result<()> {
        let body = json!({
          "run_id": doc.run_id, "job_id": doc.job_id, "created_at": doc.created_at,
          "started_at": doc.started_at, "status": doc.status, "prompt": doc.prompt,
          "conversation_id": doc.conversation_id, "submission_id": doc.submission_id,
          "model": doc.model, "sandbox": doc.sandbox, "approval_policy": doc.approval_policy,
          "cwd": doc.cwd, "error_message": doc.error_message, "tags": doc.tags
        });
        let _ = self.post_json(&self.col_url(&self.runs), &body).await?;
        Ok(())
    }

    pub async fn update_run_finished(&self, run_id: &str, status: &str, err: Option<&str>) -> Result<()> {
        let q = format!(
            "FOR r IN {} FILTER r.run_id == @run_id UPDATE r WITH { status: @status, finished_at: @finished_at, error_message: @error } IN {}",
            self.runs, self.runs
        );
        let _ = self.aql_public(&q, json!({
            "run_id": run_id,
            "status": status,
            "finished_at": now_iso(),
            "error": err
        })).await?;
        Ok(())
    }

    pub async fn insert_events_batch(&self, run_id: &str, batch: &[(i64, String, Value)]) -> Result<()> {
        if batch.is_empty() { return Ok(()); }
        let docs: Vec<Value> = batch.iter().map(|(seq, typ, payload)| {
            json!({"run_id": run_id, "seq": seq, "ts": now_iso(), "type": typ, "payload": payload})
        }).collect();
        let q = format!("FOR d IN @docs INSERT d INTO {}", self.events);
        let _ = self.aql_public(&q, json!({"docs": docs})).await?;
        Ok(())
    }

    pub async fn notify(&self, run_id: &str, job_id: &str, kind: &str, ttl_secs: u64) -> Result<()> {
        let created_at = now_iso();
        let expires_at = plus_seconds_iso(ttl_secs as i64);
        let body = json!({"run_id": run_id, "job_id": job_id, "kind": kind, "created_at": created_at, "expiresAt": expires_at});
        let _ = self.post_json(&self.col_url(&self.notes), &body).await?;
        Ok(())
    }

    pub async fn fetch_notifications_since(&self, since_iso: &str) -> Result<Vec<NotificationDoc>> {
        let q = format!("FOR n IN {} FILTER n.created_at > @since SORT n.created_at ASC LIMIT 1000 RETURN n", self.notes);
        let resp = self.aql_public(&q, json!({"since": since_iso})).await?;
        let arr = resp.get("result").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let mut out = Vec::with_capacity(arr.len());
        for v in arr { out.push(serde_json::from_value(v)?); }
        Ok(out)
    }

    pub async fn fetch_run(&self, run_id: &str) -> Result<RunDoc> {
        let q = format!("FOR r IN {} FILTER r.run_id == @run_id LIMIT 1 RETURN r", self.runs);
        let resp = self.aql_public(&q, json!({"run_id": run_id})).await?;
        let arr = resp.get("result").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        if arr.is_empty() { bail!("run not found"); }
        Ok(serde_json::from_value(arr[0].clone())?)
    }
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn plus_seconds_iso(secs: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::seconds(secs)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

