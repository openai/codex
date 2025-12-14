use crate::config::Config;
use crate::config::types::Graphiti;
use crate::config::types::GraphitiGroupIdStrategy;
use crate::config::types::GraphitiScope;
use crate::git_info::get_git_repo_root;
use crate::graphiti::client::AddMessagesRequest;
use crate::graphiti::client::GraphitiClient;
use crate::graphiti::client::GraphitiClientError;
use crate::graphiti::client::GraphitiMessage;
use crate::graphiti::client::GraphitiRoleType;
use crate::graphiti::client::SearchQuery;
use chrono::Utc;
use codex_protocol::ConversationId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use sha2::Digest;
use sha2::Sha256;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct GraphitiGroupIds {
    pub session: String,
    pub workspace: String,
    pub global: Option<String>,
}

pub struct GraphitiMemoryService {
    client: GraphitiClient,
    config: Graphiti,
    group_ids: GraphitiGroupIds,
    source_description: String,
    ingestion_queue: GraphitiIngestionQueue,
    ingested_turn_keys: Mutex<HashSet<String>>,
    recall_cache: Mutex<HashMap<String, Option<String>>>,
}

impl GraphitiMemoryService {
    pub async fn new_if_enabled(
        config: &Config,
        conversation_id: &ConversationId,
        cwd: &Path,
    ) -> Option<Self> {
        if !config.graphiti.enabled {
            return None;
        }

        if !config.active_project.is_trusted() {
            info!("Graphiti disabled: project is not trusted");
            return None;
        }

        if !config.graphiti.consent {
            warn!("Graphiti disabled: consent not granted (set [graphiti].consent = true)");
            return None;
        }

        let endpoint = config.graphiti.endpoint.as_deref()?;
        let bearer_token = config
            .graphiti
            .bearer_token_env_var
            .as_deref()
            .and_then(|key| std::env::var(key).ok());
        let client = match GraphitiClient::from_base_url_str(endpoint, bearer_token) {
            Ok(client) => client,
            Err(err) => {
                warn!(error = %safe_graphiti_client_error_summary(&err), "Graphiti disabled: invalid client configuration");
                return None;
            }
        };

        let session_key = conversation_id.to_string();
        let workspace_key = get_git_repo_root(cwd)
            .unwrap_or_else(|| cwd.to_path_buf())
            .to_string_lossy()
            .to_string();
        let group_id_strategy = config.graphiti.group_id_strategy.clone();

        let group_ids = GraphitiGroupIds {
            session: make_group_id("codex-session", &session_key, &group_id_strategy),
            workspace: make_group_id("codex-workspace", &workspace_key, &group_id_strategy),
            global: config
                .graphiti
                .global
                .enabled
                .then(|| config.graphiti.global.group_id.clone()),
        };

        let source_description = if config.graphiti.include_git_metadata {
            build_git_source_description(cwd)
                .await
                .unwrap_or_else(|| "codex".to_string())
        } else {
            "codex".to_string()
        };

        let ingestion_queue = GraphitiIngestionQueue::new(client.clone(), config.graphiti.clone());
        Some(Self {
            client,
            config: config.graphiti.clone(),
            group_ids,
            source_description,
            ingestion_queue,
            ingested_turn_keys: Mutex::new(HashSet::new()),
            recall_cache: Mutex::new(HashMap::new()),
        })
    }

    pub fn group_ids(&self) -> &GraphitiGroupIds {
        &self.group_ids
    }

    pub fn is_recall_enabled(&self) -> bool {
        self.config.recall.enabled
    }

    pub async fn recall_prompt_item(&self, turn_id: &str, query: &str) -> Option<ResponseItem> {
        if !self.config.recall.enabled {
            return None;
        }

        let query = query.trim();
        if query.is_empty() {
            return None;
        }

        if let Some(cached) = self.recall_cache.lock().await.get(turn_id).cloned() {
            return cached.map(memory_to_prompt_item);
        }

        let group_ids = self.recall_group_ids();
        if group_ids.is_empty() {
            return None;
        }

        let recall_timeout = Duration::from_millis(self.config.recall.timeout_ms);
        let request = SearchQuery {
            group_ids: Some(group_ids),
            query: truncate_chars(query, 2_048),
            max_facts: self.config.recall.max_facts,
        };

        let memory = match self.client.search(request, recall_timeout).await {
            Ok(results) => format_memory_section(
                &results
                    .facts
                    .into_iter()
                    .map(|f| f.fact)
                    .collect::<Vec<_>>(),
                self.config.recall.max_fact_chars,
                self.config.recall.max_total_chars,
            ),
            Err(err) => {
                warn!(
                    error = %safe_graphiti_client_error_summary(&err),
                    "Graphiti recall failed (continuing without memory)"
                );
                None
            }
        };

        self.recall_cache
            .lock()
            .await
            .insert(turn_id.to_string(), memory.clone());

        memory.map(memory_to_prompt_item)
    }

    pub async fn ingest_turn(&self, turn_id: &str, user_text: &str, assistant_text: &str) {
        let user_text = user_text.trim();
        let assistant_text = assistant_text.trim();
        if user_text.is_empty() && assistant_text.is_empty() {
            return;
        }

        let messages = build_turn_messages(
            user_text,
            assistant_text,
            &self.source_description,
            self.config.ingest.max_content_chars,
        );
        if messages.is_empty() {
            return;
        }

        for group_id in self.ingest_group_ids() {
            let key = format!("{group_id}:{turn_id}");
            let should_enqueue = self.ingested_turn_keys.lock().await.insert(key);
            if !should_enqueue {
                continue;
            }
            self.ingestion_queue
                .enqueue(group_id, messages.clone())
                .await;
        }
    }

    fn ingest_group_ids(&self) -> Vec<String> {
        let mut out = Vec::new();
        for scope in &self.config.ingest_scopes {
            match scope {
                GraphitiScope::Session => out.push(self.group_ids.session.clone()),
                GraphitiScope::Workspace => out.push(self.group_ids.workspace.clone()),
                GraphitiScope::Global => {
                    if let Some(group_id) = self.group_ids.global.clone() {
                        out.push(group_id);
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn recall_group_ids(&self) -> Vec<String> {
        let mut out = Vec::new();
        for scope in &self.config.recall.scopes {
            match scope {
                GraphitiScope::Session => out.push(self.group_ids.session.clone()),
                GraphitiScope::Workspace => out.push(self.group_ids.workspace.clone()),
                GraphitiScope::Global => {
                    if let Some(group_id) = self.group_ids.global.clone() {
                        out.push(group_id);
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

fn memory_to_prompt_item(memory: String) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "system".to_string(),
        content: vec![ContentItem::InputText { text: memory }],
    }
}

fn build_turn_messages(
    user_text: &str,
    assistant_text: &str,
    source_description: &str,
    max_content_chars: usize,
) -> Vec<GraphitiMessage> {
    let now = Utc::now();
    let mut messages = Vec::new();

    if !user_text.is_empty() {
        messages.push(GraphitiMessage {
            content: truncate_with_marker(user_text, max_content_chars),
            uuid: None,
            name: String::new(),
            role_type: GraphitiRoleType::User,
            role: None,
            timestamp: now,
            source_description: source_description.to_string(),
        });
    }

    if !assistant_text.is_empty() {
        messages.push(GraphitiMessage {
            content: truncate_with_marker(assistant_text, max_content_chars),
            uuid: None,
            name: String::new(),
            role_type: GraphitiRoleType::Assistant,
            role: None,
            timestamp: now,
            source_description: source_description.to_string(),
        });
    }

    messages
}

fn format_memory_section(
    facts: &[String],
    max_fact_chars: usize,
    max_total_chars: usize,
) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for fact in facts {
        let trimmed = fact.trim();
        if trimmed.is_empty() {
            continue;
        }
        lines.push(format!(
            "- {}",
            truncate_with_marker(trimmed, max_fact_chars)
        ));
    }

    if lines.is_empty() {
        return None;
    }

    let mut out = String::from("<graphiti_memory>\n");
    for line in lines {
        if out.chars().count() >= max_total_chars {
            break;
        }

        let next = format!("{line}\n");
        out.push_str(&next);
        if out.chars().count() >= max_total_chars {
            break;
        }
    }

    out.push_str("</graphiti_memory>");

    let out = truncate_chars(&out, max_total_chars);
    (!out.trim().is_empty()).then_some(out)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn truncate_with_marker(text: &str, max_chars: usize) -> String {
    let marker = "\n…[truncated]";
    let marker_len = marker.chars().count();
    let len = text.chars().count();
    if len <= max_chars {
        return text.to_string();
    }
    if max_chars <= marker_len {
        return truncate_chars(text, max_chars);
    }
    let head = truncate_chars(text, max_chars - marker_len);
    format!("{head}{marker}")
}

fn make_group_id(prefix: &str, raw_key: &str, strategy: &GraphitiGroupIdStrategy) -> String {
    match strategy {
        GraphitiGroupIdStrategy::Hashed => {
            let mut hasher = Sha256::new();
            hasher.update(raw_key.as_bytes());
            let digest = hasher.finalize();
            let hex = hex_encode(digest.as_slice());
            format!("{prefix}-{}", &hex[..16])
        }
        GraphitiGroupIdStrategy::Raw => {
            let safe = sanitize_group_id(raw_key);
            let mut candidate = format!("{prefix}-{safe}");
            if candidate.len() <= 120 {
                return candidate;
            }

            let mut hasher = Sha256::new();
            hasher.update(raw_key.as_bytes());
            let digest = hasher.finalize();
            let hex = hex_encode(digest.as_slice());
            let suffix = format!("-{}", &hex[..16]);
            let max_prefix = 120usize.saturating_sub(suffix.len());
            candidate.truncate(max_prefix);
            candidate.push_str(&suffix);
            candidate
        }
    }
}

fn sanitize_group_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn safe_graphiti_client_error_summary(err: &GraphitiClientError) -> String {
    match err {
        GraphitiClientError::InvalidBaseUrl(_) => "invalid_base_url".to_string(),
        GraphitiClientError::UrlJoin(_) => "url_join".to_string(),
        GraphitiClientError::Request(_) => "request_failed".to_string(),
        GraphitiClientError::UnexpectedStatus { status, body } => {
            format!(
                "unexpected_status {} (body_len={})",
                status.as_u16(),
                body.len()
            )
        }
    }
}

async fn build_git_source_description(cwd: &Path) -> Option<String> {
    let repo_root = get_git_repo_root(cwd)?;
    let repo_name = repo_root.file_name()?.to_string_lossy();

    let timeout = Duration::from_millis(500);
    let (branch, commit, dirty) = tokio::join!(
        git_current_branch(&repo_root, timeout),
        git_current_commit(&repo_root, timeout),
        git_worktree_is_dirty(&repo_root, timeout),
    );

    let mut parts = vec![format!("codex repo={repo_name}")];
    if let Some(branch) = branch
        && branch != "HEAD"
    {
        parts.push(format!("branch={branch}"));
    }
    if let Some(commit) = commit {
        let short_commit = commit.trim().chars().take(12).collect::<String>();
        if !short_commit.is_empty() {
            parts.push(format!("commit={short_commit}"));
        }
    }

    match dirty {
        Some(dirty) => parts.push(format!("dirty={dirty}")),
        None => parts.push("dirty=unknown".to_string()),
    }

    Some(parts.join(" "))
}

async fn run_git_output_with_timeout(
    repo_root: &Path,
    args: &[&str],
    timeout: Duration,
) -> Option<std::process::Output> {
    let child = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .ok()?
        .ok()
}

async fn git_current_branch(repo_root: &Path, timeout: Duration) -> Option<String> {
    let out =
        run_git_output_with_timeout(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"], timeout)
            .await?;
    if !out.status.success() {
        return None;
    }
    let branch = String::from_utf8(out.stdout).ok()?;
    let branch = branch.trim();
    (!branch.is_empty()).then(|| branch.to_string())
}

async fn git_current_commit(repo_root: &Path, timeout: Duration) -> Option<String> {
    let out = run_git_output_with_timeout(repo_root, &["rev-parse", "HEAD"], timeout).await?;
    if !out.status.success() {
        return None;
    }
    let sha = String::from_utf8(out.stdout).ok()?;
    let sha = sha.trim();
    (!sha.is_empty()).then(|| sha.to_string())
}

async fn git_worktree_is_dirty(repo_root: &Path, timeout: Duration) -> Option<bool> {
    let out = run_git_output_with_timeout(repo_root, &["status", "--porcelain"], timeout).await?;
    if !out.status.success() {
        return None;
    }
    Some(!out.stdout.is_empty())
}

struct GraphitiIngestionQueue {
    inner: std::sync::Arc<GraphitiIngestionQueueInner>,
}

impl Drop for GraphitiIngestionQueue {
    fn drop(&mut self) {
        self.inner.shutdown.cancel();
    }
}

struct GraphitiIngestionQueueInner {
    config: Graphiti,
    client: GraphitiClient,
    state: Mutex<QueueState>,
    notify: Notify,
    shutdown: CancellationToken,
}

#[derive(Clone)]
struct IngestJob {
    group_id: String,
    messages: Vec<GraphitiMessage>,
    attempt: u32,
}

#[derive(Clone)]
struct ScheduledJob {
    run_at: Instant,
    seq: u64,
    job: IngestJob,
}

impl PartialEq for ScheduledJob {
    fn eq(&self, other: &Self) -> bool {
        self.run_at == other.run_at && self.seq == other.seq
    }
}

impl Eq for ScheduledJob {}

impl PartialOrd for ScheduledJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledJob {
    fn cmp(&self, other: &Self) -> Ordering {
        self.run_at
            .cmp(&other.run_at)
            .then_with(|| self.seq.cmp(&other.seq))
    }
}

#[derive(Default)]
struct QueueState {
    ready: VecDeque<IngestJob>,
    delayed: BinaryHeap<std::cmp::Reverse<ScheduledJob>>,
    next_seq: u64,
    dropped_jobs: u64,
}

impl GraphitiIngestionQueue {
    fn new(client: GraphitiClient, config: Graphiti) -> Self {
        let inner = std::sync::Arc::new(GraphitiIngestionQueueInner {
            config,
            client,
            state: Mutex::new(QueueState::default()),
            notify: Notify::new(),
            shutdown: CancellationToken::new(),
        });

        let worker = std::sync::Arc::clone(&inner);
        tokio::spawn(async move {
            worker.worker_loop().await;
        });

        Self { inner }
    }

    async fn enqueue(&self, group_id: String, messages: Vec<GraphitiMessage>) {
        self.inner.enqueue(group_id, messages).await;
    }
}

impl GraphitiIngestionQueueInner {
    async fn enqueue(&self, group_id: String, messages: Vec<GraphitiMessage>) {
        if messages.is_empty() {
            return;
        }

        let mut state = self.state.lock().await;
        let queued = state.ready.len() + state.delayed.len();
        if queued >= self.config.ingest.max_queue_size {
            if state.ready.pop_front().is_none() {
                let _ = state.delayed.pop();
            }
            state.dropped_jobs = state.dropped_jobs.saturating_add(1);
            warn!(
                dropped_jobs = state.dropped_jobs,
                "Graphiti ingestion queue full; dropping oldest job"
            );
        }

        state.ready.push_back(IngestJob {
            group_id,
            messages,
            attempt: 0,
        });

        drop(state);
        self.notify.notify_one();
    }

    async fn worker_loop(self: std::sync::Arc<Self>) {
        loop {
            if self.shutdown.is_cancelled() {
                break;
            }

            let maybe_job = self.pop_next_job().await;
            match maybe_job {
                NextJob::Ready(job) => {
                    self.process_job(job).await;
                }
                NextJob::Wait(duration) => {
                    tokio::select! {
                        _ = self.shutdown.cancelled() => break,
                        _ = self.notify.notified() => {}
                        _ = tokio::time::sleep(duration) => {}
                    }
                }
                NextJob::Idle => {
                    tokio::select! {
                        _ = self.shutdown.cancelled() => break,
                        _ = self.notify.notified() => {}
                    }
                }
            }
        }
    }

    async fn pop_next_job(&self) -> NextJob {
        let mut state = self.state.lock().await;

        if let Some(job) = state.ready.pop_front() {
            return NextJob::Ready(job);
        }

        let now = Instant::now();
        if let Some(std::cmp::Reverse(next)) = state.delayed.peek().cloned() {
            if next.run_at <= now {
                if let Some(std::cmp::Reverse(due)) = state.delayed.pop() {
                    return NextJob::Ready(due.job);
                }
                return NextJob::Idle;
            }

            let wait = next.run_at.saturating_duration_since(now);
            return NextJob::Wait(wait);
        }

        NextJob::Idle
    }

    async fn process_job(&self, job: IngestJob) {
        let timeout = Duration::from_millis(self.config.ingest.timeout_ms);
        let request = AddMessagesRequest {
            group_id: job.group_id.clone(),
            messages: job.messages.clone(),
        };

        let result = self
            .client
            .add_messages(request, timeout, self.config.ingest.max_batch_size)
            .await;

        let err = match result {
            Ok(_) => return,
            Err(err) => err,
        };
        let next_attempt = job.attempt.saturating_add(1);
        if next_attempt >= self.config.ingest.retry_max_attempts {
            warn!(
                group_id = %job.group_id,
                error = %safe_graphiti_client_error_summary(&err),
                "Graphiti ingestion failed; dropping job after max attempts"
            );
            return;
        }

        let backoff_ms = compute_backoff_ms(
            self.config.ingest.retry_initial_backoff_ms,
            self.config.ingest.retry_max_backoff_ms,
            job.attempt,
        );
        warn!(
            group_id = %job.group_id,
            attempt = next_attempt,
            backoff_ms,
            error = %safe_graphiti_client_error_summary(&err),
            "Graphiti ingestion failed; scheduling retry"
        );

        let run_at = Instant::now() + Duration::from_millis(backoff_ms);
        self.schedule_retry(
            IngestJob {
                attempt: next_attempt,
                ..job
            },
            run_at,
        )
        .await;
    }

    async fn schedule_retry(&self, job: IngestJob, run_at: Instant) {
        let mut state = self.state.lock().await;
        let queued = state.ready.len() + state.delayed.len();
        if queued >= self.config.ingest.max_queue_size {
            if state.ready.pop_front().is_none() {
                let _ = state.delayed.pop();
            }
            state.dropped_jobs = state.dropped_jobs.saturating_add(1);
            warn!(
                dropped_jobs = state.dropped_jobs,
                "Graphiti ingestion queue full; dropping oldest job"
            );
        }

        let seq = state.next_seq;
        state.next_seq = state.next_seq.saturating_add(1);
        state
            .delayed
            .push(std::cmp::Reverse(ScheduledJob { run_at, seq, job }));
        drop(state);
        self.notify.notify_one();
    }
}

enum NextJob {
    Ready(IngestJob),
    Wait(Duration),
    Idle,
}

fn compute_backoff_ms(initial_ms: u64, max_ms: u64, attempt: u32) -> u64 {
    let exponent = attempt.min(31);
    let factor = 2u64.saturating_pow(exponent);
    let ms = initial_ms.saturating_mul(factor);
    ms.min(max_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn group_id_hashed_is_stable_and_prefixed() {
        let id1 = make_group_id(
            "codex-workspace",
            "/tmp/some path/with spaces",
            &GraphitiGroupIdStrategy::Hashed,
        );
        let id2 = make_group_id(
            "codex-workspace",
            "/tmp/some path/with spaces",
            &GraphitiGroupIdStrategy::Hashed,
        );
        assert_eq!(id1, id2);
        assert!(id1.starts_with("codex-workspace-"));
        assert_eq!(id1.len(), "codex-workspace-".len() + 16);
    }

    #[test]
    fn group_id_raw_is_sanitized_and_capped() {
        let raw = "this has spaces and /slashes/ and 😀";
        let id = make_group_id("codex-session", raw, &GraphitiGroupIdStrategy::Raw);
        assert!(id.starts_with("codex-session-"));
        assert!(
            id.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        );
        assert!(id.len() <= 120);
    }

    #[tokio::test]
    async fn build_git_source_description_includes_branch_commit_and_dirty() {
        let repo = TempDir::new().expect("temp repo");
        let repo_path = repo.path();

        let status = std::process::Command::new("git")
            .arg("init")
            .current_dir(repo_path)
            .status()
            .expect("git init should run");
        assert!(status.success());

        let status = std::process::Command::new("git")
            .args(["config", "user.email", "codex@example.com"])
            .current_dir(repo_path)
            .status()
            .expect("git config user.email should run");
        assert!(status.success());

        let status = std::process::Command::new("git")
            .args(["config", "user.name", "Codex Test"])
            .current_dir(repo_path)
            .status()
            .expect("git config user.name should run");
        assert!(status.success());

        std::fs::write(repo_path.join("file.txt"), "hello\n").expect("write file");

        let status = std::process::Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(repo_path)
            .status()
            .expect("git add should run");
        assert!(status.success());

        let status = std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo_path)
            .status()
            .expect("git commit should run");
        assert!(status.success());

        std::fs::write(repo_path.join("file.txt"), "hello world\n").expect("write dirty file");

        let desc = build_git_source_description(repo_path)
            .await
            .expect("should produce description");

        assert!(desc.starts_with("codex repo="), "desc={desc:?}");
        assert!(desc.contains(" branch="), "desc={desc:?}");
        assert!(desc.contains(" commit="), "desc={desc:?}");
        assert!(desc.contains(" dirty=true"), "desc={desc:?}");
    }
}
