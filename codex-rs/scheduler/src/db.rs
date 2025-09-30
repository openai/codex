use crate::config::{ArangoConfig, SchedulerConfig};
use anyhow::{bail, Result};
use reqwest::{Client, ClientBuilder};
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
    // Tunables from SchedulerConfig
    max_retries: usize,
    backoff_base_ms: u64,
    backoff_jitter: f32,
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
        let client = ClientBuilder::new()
            .use_rustls_tls()
            .https_only(true)
            .user_agent(concat!("codex-scheduler/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            base_url: cfg.url.clone(),
            database: cfg.database.clone(),
            runs: cfg.runs_collection.clone(),
            events: cfg.events_collection.clone(),
            notes: cfg.notifications_collection.clone(),
            state: cfg.state_collection.clone(),
            client,
            auth: (cfg.username.clone(), cfg.password.clone()),
            max_retries: 5,
            backoff_base_ms: 200,
            backoff_jitter: 0.25,
        })
    }

    /// Construct Db with TLS + retry/backoff from SchedulerConfig and ArangoConfig.
    pub fn from_config(s: &SchedulerConfig, cfg: &ArangoConfig) -> Result<Self> {
        let https_only = !(cfg.allow_insecure && !s.strict_tls);
        let client = ClientBuilder::new()
            .use_rustls_tls()
            .https_only(https_only)
            .user_agent(concat!("codex-scheduler/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            base_url: cfg.url.clone(),
            database: cfg.database.clone(),
            runs: cfg.runs_collection.clone(),
            events: cfg.events_collection.clone(),
            notes: cfg.notifications_collection.clone(),
            state: cfg.state_collection.clone(),
            client,
            auth: (cfg.username.clone(), cfg.password.clone()),
            max_retries: s.max_write_retries,
            backoff_base_ms: s.backoff_base_ms,
            backoff_jitter: s.backoff_jitter,
        })
    }

    pub fn state_collection(&self) -> &str {
        &self.state
    }

    fn col_url(&self, col: &str) -> String {
        format!(
            "{}/_db/{}/_api/document/{}",
            self.base_url, self.database, col
        )
    }
    fn cursor_url(&self) -> String {
        format!("{}/_db/{}/_api/cursor", self.base_url, self.database)
    }

    async fn post_json(&self, url: &str, body: &Value) -> Result<Value> {
        let res = self
            .client
            .post(url)
            .basic_auth(self.auth.0.clone(), Some(self.auth.1.clone()))
            .json(body)
            .send()
            .await?;
        let status = res.status();
        let val: Value = res.json().await.unwrap_or(json!({"error":"invalid json"}));
        if !status.is_success() {
            bail!("arangodb POST {} failed: {} body={}", url, status, val);
        }
        Ok(val)
    }

    /// Retry wrapper with jittered exponential backoff for POSTs that insert documents.
    async fn post_json_with_retry(&self, url: &str, body: &Value) -> Result<()> {
        use rand::{thread_rng, Rng};
        use tokio::time::{sleep, Duration};

        let mut attempt = 0usize;
        loop {
            match self.post_json(url, body).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    tracing::warn!("Arango POST retryable error: {e}");
                }
            }
            if attempt >= self.max_retries {
                break;
            }
            // backoff base * 2^attempt with +/- jitter
            let base_ms = self.backoff_base_ms.saturating_mul(1u64 << attempt.min(8));
            let variance = (base_ms as f32 * self.backoff_jitter) as u64;
            let jitter = {
                let mut rng = thread_rng();
                let offset: i64 = rng.gen_range(-(variance as i64)..=(variance as i64));
                (base_ms as i64 + offset).max(50) as u64
            };
            sleep(Duration::from_millis(jitter)).await;
            attempt += 1;
        }
        bail!("arangodb POST exhausted retries for {}", url)
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
            let _ = self
                .client
                .post(&url)
                .basic_auth(self.auth.0.clone(), Some(self.auth.1.clone()))
                .json(&body)
                .send()
                .await;
        }

        // Indexes (best-effort)
        let _ = self
            .aql_public(
                "RETURN ENSURE_INDEX(@col, { type: 'persistent', fields: ['job_id'] })",
                json!({"col": self.runs}),
            )
            .await;
        let _ = self
            .aql_public(
                "RETURN ENSURE_INDEX(@col, { type: 'persistent', fields: ['created_at'] })",
                json!({"col": self.runs}),
            )
            .await;
        let _ = self
            .aql_public(
                "RETURN ENSURE_INDEX(@col, { type: 'persistent', fields: ['status'] })",
                json!({"col": self.runs}),
            )
            .await;
        let _ = self.aql_public(
            "RETURN ENSURE_INDEX(@col, { type: 'persistent', fields: ['run_id','seq'], unique: true })",
            json!({"col": self.events}),
        ).await;
        let _ = self
            .aql_public(
                "RETURN ENSURE_INDEX(@col, { type: 'persistent', fields: ['created_at'] })",
                json!({"col": self.notes}),
            )
            .await;
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

    pub async fn update_run_finished(
        &self,
        run_id: &str,
        status: &str,
        err: Option<&str>,
    ) -> Result<()> {
        let q = format!(
            "FOR r IN {} FILTER r.run_id == @run_id UPDATE r WITH {{ status: @status, finished_at: @finished_at, error_message: @error }} IN {}",
            self.runs, self.runs
        );
        let _ = self
            .aql_public(
                &q,
                json!({
                    "run_id": run_id,
                    "status": status,
                    "finished_at": now_iso(),
                    "error": err
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn insert_events_batch(
        &self,
        run_id: &str,
        batch: &[(i64, String, Value)],
    ) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let docs: Vec<Value> = batch.iter().map(|(seq, typ, payload)| {
            json!({"run_id": run_id, "seq": seq, "ts": now_iso(), "type": typ, "payload": payload})
        }).collect();
        let url = self.col_url(&self.events);
        let body = Value::from(docs);
        self.post_json_with_retry(&url, &body).await
    }

    pub async fn notify(
        &self,
        run_id: &str,
        job_id: &str,
        kind: &str,
        ttl_secs: u64,
    ) -> Result<()> {
        let created_at = now_iso();
        let expires_at = plus_seconds_iso(ttl_secs as i64);
        let body = json!({"run_id": run_id, "job_id": job_id, "kind": kind, "created_at": created_at, "expiresAt": expires_at});
        let url = self.col_url(&self.notes);
        self.post_json_with_retry(&url, &body).await
    }

    pub async fn fetch_notifications_since(&self, since_iso: &str) -> Result<Vec<NotificationDoc>> {
        let q = format!(
            "FOR n IN {} FILTER n.created_at > @since SORT n.created_at ASC LIMIT 1000 RETURN n",
            self.notes
        );
        let resp = self.aql_public(&q, json!({"since": since_iso})).await?;
        let arr = resp
            .get("result")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::with_capacity(arr.len());
        for v in arr {
            out.push(serde_json::from_value(v)?);
        }
        Ok(out)
    }

    pub async fn fetch_run(&self, run_id: &str) -> Result<RunDoc> {
        let q = format!(
            "FOR r IN {} FILTER r.run_id == @run_id LIMIT 1 RETURN r",
            self.runs
        );
        let resp = self.aql_public(&q, json!({"run_id": run_id})).await?;
        let arr = resp
            .get("result")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        if arr.is_empty() {
            bail!("run not found");
        }
        Ok(serde_json::from_value(arr[0].clone())?)
    }
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn plus_seconds_iso(secs: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::seconds(secs))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
