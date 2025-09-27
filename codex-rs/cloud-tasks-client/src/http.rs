use crate::ApplyOutcome;
use crate::ApplyStatus;
use crate::AttemptStatus;
use crate::CloudBackend;
use crate::CloudTaskError;
use crate::DiffSummary;
use crate::Result;
use crate::TaskId;
use crate::TaskStatus;
use crate::TaskSummary;
use crate::TurnAttempt;
use crate::api::TaskText;
use chrono::DateTime;
use chrono::Utc;

use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;

use codex_backend_client as backend;
use codex_backend_client::CodeTaskDetailsResponseExt;

#[derive(Clone)]
pub struct HttpClient {
    pub base_url: String,
    backend: backend::Client,
}

impl HttpClient {
    pub fn new(base_url: impl Into<String>) -> anyhow::Result<Self> {
        let base_url = base_url.into();
        let backend = backend::Client::new(base_url.clone())?;
        Ok(Self { base_url, backend })
    }

    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.backend = self.backend.clone().with_bearer_token(token);
        self
    }

    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.backend = self.backend.clone().with_user_agent(ua);
        self
    }

    pub fn with_chatgpt_account_id(mut self, account_id: impl Into<String>) -> Self {
        self.backend = self.backend.clone().with_chatgpt_account_id(account_id);
        self
    }
}

fn is_unified_diff(diff: &str) -> bool {
    let t = diff.trim_start();
    if t.starts_with("diff --git ") {
        return true;
    }
    let has_dash_headers = diff.contains("\n--- ") && diff.contains("\n+++ ");
    let has_hunk = diff.contains("\n@@ ") || diff.starts_with("@@ ");
    has_dash_headers && has_hunk
}

fn tail(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s[s.len() - max..].to_string()
    }
}

#[async_trait::async_trait]
impl CloudBackend for HttpClient {
    async fn list_tasks(&self, env: Option<&str>) -> Result<Vec<TaskSummary>> {
        let resp = self
            .backend
            .list_tasks(Some(20), Some("current"), env)
            .await
            .map_err(|e| CloudTaskError::Http(format!("list_tasks failed: {e}")))?;

        let tasks: Vec<TaskSummary> = resp
            .items
            .into_iter()
            .map(map_task_list_item_to_summary)
            .collect();
        // Debug log for env filtering visibility
        append_error_log(&format!(
            "http.list_tasks: env={} items={}",
            env.unwrap_or("<all>"),
            tasks.len()
        ));
        Ok(tasks)
    }

    async fn get_task_diff(&self, _id: TaskId) -> Result<Option<String>> {
        let id = _id.0;
        let (details, body, ct) = self
            .backend
            .get_task_details_with_body(&id)
            .await
            .map_err(|e| CloudTaskError::Http(format!("get_task_details failed: {e}")))?;
        if let Some(diff) = details.unified_diff() {
            return Ok(Some(diff));
        }
        // No diff yet (pending or non-diff task).
        // Keep variables bound for potential future logging.
        let _ = (body, ct);
        Ok(None)
    }

    async fn get_task_messages(&self, _id: TaskId) -> Result<Vec<String>> {
        let id = _id.0;
        let (details, body, ct) = self
            .backend
            .get_task_details_with_body(&id)
            .await
            .map_err(|e| CloudTaskError::Http(format!("get_task_details failed: {e}")))?;
        let mut msgs = details.assistant_text_messages();
        if msgs.is_empty() {
            msgs.extend(extract_assistant_messages_from_body(&body));
        }
        if !msgs.is_empty() {
            return Ok(msgs);
        }
        if let Some(err) = details.assistant_error_message() {
            return Ok(vec![format!("Task failed: {err}")]);
        }
        // No assistant messages found; return a debuggable error with context for logging.
        let url = if self.base_url.contains("/backend-api") {
            format!("{}/wham/tasks/{}", self.base_url, id)
        } else {
            format!("{}/api/codex/tasks/{}", self.base_url, id)
        };
        Err(CloudTaskError::Http(format!(
            "No assistant text messages in response. GET {url}; content-type={ct}; body={body}"
        )))
    }

    async fn get_task_text(&self, _id: TaskId) -> Result<TaskText> {
        let id = _id.0;
        let (details, body, _ct) = self
            .backend
            .get_task_details_with_body(&id)
            .await
            .map_err(|e| CloudTaskError::Http(format!("get_task_details failed: {e}")))?;
        let prompt = details.user_text_prompt();
        let mut messages = details.assistant_text_messages();
        if messages.is_empty() {
            messages.extend(extract_assistant_messages_from_body(&body));
        }
        let turn_map = details.current_assistant_turn.as_ref();
        let turn_id = turn_map
            .and_then(|m| m.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let sibling_turn_ids = turn_map
            .and_then(|m| m.get("sibling_turn_ids"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let attempt_placement = turn_map
            .and_then(|m| m.get("attempt_placement"))
            .and_then(Value::as_i64);
        let attempt_status = attempt_status_from_str(
            turn_map
                .and_then(|m| m.get("turn_status"))
                .and_then(Value::as_str),
        );
        Ok(TaskText {
            prompt,
            messages,
            turn_id,
            sibling_turn_ids,
            attempt_placement,
            attempt_status,
        })
    }

    async fn list_sibling_attempts(
        &self,
        task: TaskId,
        turn_id: String,
    ) -> Result<Vec<TurnAttempt>> {
        let resp = self
            .backend
            .list_sibling_turns(&task.0, &turn_id)
            .await
            .map_err(|e| CloudTaskError::Http(format!("list_sibling_turns failed: {e}")))?;

        let mut attempts: Vec<TurnAttempt> = resp
            .sibling_turns
            .iter()
            .filter_map(turn_attempt_from_map)
            .collect();
        attempts.sort_by(compare_attempts);
        Ok(attempts)
    }

    async fn apply_task(&self, _id: TaskId, diff_override: Option<String>) -> Result<ApplyOutcome> {
        let id = _id.0;
        self.apply_with_diff(id, diff_override, false).await
    }

    async fn apply_task_preflight(
        &self,
        _id: TaskId,
        diff_override: Option<String>,
    ) -> Result<ApplyOutcome> {
        let id = _id.0;
        self.apply_with_diff(id, diff_override, true).await
    }

    async fn create_task(
        &self,
        env_id: &str,
        prompt: &str,
        git_ref: &str,
        qa_mode: bool,
    ) -> Result<crate::CreatedTask> {
        // Build request payload patterned after VSCode/newtask.rs
        let mut input_items: Vec<serde_json::Value> = Vec::new();
        input_items.push(serde_json::json!({
            "type": "message",
            "role": "user",
            "content": [{ "content_type": "text", "text": prompt }]
        }));

        if let Ok(diff) = std::env::var("CODEX_STARTING_DIFF")
            && !diff.is_empty()
        {
            input_items.push(serde_json::json!({
                "type": "pre_apply_patch",
                "output_diff": { "diff": diff }
            }));
        }

        let request_body = serde_json::json!({
            "new_task": {
                "environment_id": env_id,
                "branch": git_ref,
                "run_environment_in_qa_mode": qa_mode,
            },
            "input_items": input_items,
        });

        // Use the underlying backend client to post with proper headers
        match self.backend.create_task(request_body).await {
            Ok(id) => {
                append_error_log(&format!(
                    "new_task: created id={id} env={} prompt_chars={}",
                    env_id,
                    prompt.chars().count()
                ));
                Ok(crate::CreatedTask { id: TaskId(id) })
            }
            Err(e) => {
                append_error_log(&format!(
                    "new_task: create failed env={} prompt_chars={}: {}",
                    env_id,
                    prompt.chars().count(),
                    e
                ));
                Err(CloudTaskError::Http(format!("create_task failed: {e}")))
            }
        }
    }
}

/// Best-effort extraction of assistant text messages from a raw `get_task_details` body.
/// Falls back to worklog messages when structured turns are not present.
impl HttpClient {
    async fn apply_with_diff(
        &self,
        id: String,
        diff_override: Option<String>,
        preflight: bool,
    ) -> Result<ApplyOutcome> {
        let diff = match diff_override {
            Some(diff) => diff,
            None => {
                let details =
                    self.backend.get_task_details(&id).await.map_err(|e| {
                        CloudTaskError::Http(format!("get_task_details failed: {e}"))
                    })?;
                details.unified_diff().ok_or_else(|| {
                    CloudTaskError::Msg(format!("No diff available for task {id}"))
                })?
            }
        };

        if !is_unified_diff(&diff) {
            let summary = summarize_patch_for_logging(&diff);
            let mode = if preflight { "preflight" } else { "apply" };
            append_error_log(&format!(
                "apply_error: id={id} mode={mode} format=non-unified; {summary}"
            ));
            return Ok(ApplyOutcome {
                applied: false,
                status: ApplyStatus::Error,
                message: "Expected unified git diff; backend returned an incompatible format."
                    .to_string(),
                skipped_paths: Vec::new(),
                conflict_paths: Vec::new(),
            });
        }

        let req = codex_git_apply::ApplyGitRequest {
            cwd: std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()),
            diff: diff.clone(),
            revert: false,
            preflight,
        };
        let r = codex_git_apply::apply_git_patch(&req)
            .map_err(|e| CloudTaskError::Io(format!("git apply failed to run: {e}")))?;

        let status = if r.exit_code == 0 {
            ApplyStatus::Success
        } else if !r.applied_paths.is_empty() || !r.conflicted_paths.is_empty() {
            ApplyStatus::Partial
        } else {
            ApplyStatus::Error
        };
        let applied = matches!(status, ApplyStatus::Success) && !preflight;

        let message = if preflight {
            match status {
                ApplyStatus::Success => format!("Preflight passed for task {id} (applies cleanly)"),
                ApplyStatus::Partial => format!(
                    "Preflight: patch does not fully apply for task {id} (applied={}, skipped={}, conflicts={})",
                    r.applied_paths.len(),
                    r.skipped_paths.len(),
                    r.conflicted_paths.len()
                ),
                ApplyStatus::Error => format!(
                    "Preflight failed for task {id} (applied={}, skipped={}, conflicts={})",
                    r.applied_paths.len(),
                    r.skipped_paths.len(),
                    r.conflicted_paths.len()
                ),
            }
        } else {
            match status {
                ApplyStatus::Success => format!(
                    "Applied task {id} locally ({} files)",
                    r.applied_paths.len()
                ),
                ApplyStatus::Partial => format!(
                    "Apply partially succeeded for task {id} (applied={}, skipped={}, conflicts={})",
                    r.applied_paths.len(),
                    r.skipped_paths.len(),
                    r.conflicted_paths.len()
                ),
                ApplyStatus::Error => format!(
                    "Apply failed for task {id} (applied={}, skipped={}, conflicts={})",
                    r.applied_paths.len(),
                    r.skipped_paths.len(),
                    r.conflicted_paths.len()
                ),
            }
        };

        if matches!(status, ApplyStatus::Partial | ApplyStatus::Error)
            || (preflight && !matches!(status, ApplyStatus::Success))
        {
            let mut log = String::new();
            let summary = summarize_patch_for_logging(&diff);
            let mode = if preflight { "preflight" } else { "apply" };
            use std::fmt::Write as _;
            let _ = writeln!(
                &mut log,
                "apply_result: mode={} id={} status={:?} applied={} skipped={} conflicts={} cmd={}",
                mode,
                id,
                status,
                r.applied_paths.len(),
                r.skipped_paths.len(),
                r.conflicted_paths.len(),
                r.cmd_for_log
            );
            let _ = writeln!(
                &mut log,
                "stdout_tail=
{}
stderr_tail=
{}",
                tail(&r.stdout, 2000),
                tail(&r.stderr, 2000)
            );
            let _ = writeln!(&mut log, "{summary}");
            let _ = writeln!(
                &mut log,
                "----- PATCH BEGIN -----
{diff}
----- PATCH END -----"
            );
            append_error_log(&log);
        }

        Ok(ApplyOutcome {
            applied,
            status,
            message,
            skipped_paths: r.skipped_paths,
            conflict_paths: r.conflicted_paths,
        })
    }
}

fn extract_assistant_messages_from_body(body: &str) -> Vec<String> {
    let mut msgs = Vec::new();
    if let Ok(full) = serde_json::from_str::<serde_json::Value>(body)
        && let Some(arr) = full
            .get("current_assistant_turn")
            .and_then(|v| v.get("worklog"))
            .and_then(|v| v.get("messages"))
            .and_then(|v| v.as_array())
    {
        for m in arr {
            let is_assistant = m
                .get("author")
                .and_then(|a| a.get("role"))
                .and_then(|r| r.as_str())
                == Some("assistant");
            if !is_assistant {
                continue;
            }
            if let Some(parts) = m
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
            {
                for p in parts {
                    if let Some(s) = p.as_str() {
                        if !s.is_empty() {
                            msgs.push(s.to_string());
                        }
                        continue;
                    }
                    if let Some(obj) = p.as_object()
                        && obj.get("content_type").and_then(|t| t.as_str()) == Some("text")
                        && let Some(txt) = obj.get("text").and_then(|t| t.as_str())
                    {
                        msgs.push(txt.to_string());
                    }
                }
            }
        }
    }
    msgs
}

fn turn_attempt_from_map(turn: &HashMap<String, Value>) -> Option<TurnAttempt> {
    let turn_id = turn.get("id").and_then(Value::as_str)?.to_string();
    let attempt_placement = turn.get("attempt_placement").and_then(Value::as_i64);
    let created_at = parse_timestamp_value(turn.get("created_at"));
    let status = attempt_status_from_str(turn.get("turn_status").and_then(Value::as_str));
    let diff = extract_diff_from_turn(turn);
    let messages = extract_assistant_messages_from_turn(turn);
    Some(TurnAttempt {
        turn_id,
        attempt_placement,
        created_at,
        status,
        diff,
        messages,
    })
}

fn compare_attempts(a: &TurnAttempt, b: &TurnAttempt) -> Ordering {
    match (a.attempt_placement, b.attempt_placement) {
        (Some(lhs), Some(rhs)) => lhs.cmp(&rhs),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => match (a.created_at, b.created_at) {
            (Some(lhs), Some(rhs)) => lhs.cmp(&rhs),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.turn_id.cmp(&b.turn_id),
        },
    }
}

fn extract_diff_from_turn(turn: &HashMap<String, Value>) -> Option<String> {
    let items = turn.get("output_items").and_then(Value::as_array)?;
    for item in items {
        match item.get("type").and_then(Value::as_str) {
            Some("output_diff") => {
                if let Some(diff) = item.get("diff").and_then(Value::as_str)
                    && !diff.is_empty()
                {
                    return Some(diff.to_string());
                }
            }
            Some("pr") => {
                if let Some(diff) = item
                    .get("output_diff")
                    .and_then(Value::as_object)
                    .and_then(|od| od.get("diff"))
                    .and_then(Value::as_str)
                    && !diff.is_empty()
                {
                    return Some(diff.to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_assistant_messages_from_turn(turn: &HashMap<String, Value>) -> Vec<String> {
    let mut msgs = Vec::new();
    if let Some(items) = turn.get("output_items").and_then(Value::as_array) {
        for item in items {
            if item.get("type").and_then(Value::as_str) != Some("message") {
                continue;
            }
            if let Some(content) = item.get("content").and_then(Value::as_array) {
                for part in content {
                    if part.get("content_type").and_then(Value::as_str) == Some("text")
                        && let Some(txt) = part.get("text").and_then(Value::as_str)
                        && !txt.is_empty()
                    {
                        msgs.push(txt.to_string());
                    }
                }
            }
        }
    }
    if msgs.is_empty()
        && let Some(err) = turn.get("error").and_then(Value::as_object)
    {
        let message = err.get("message").and_then(Value::as_str).unwrap_or("");
        let code = err.get("code").and_then(Value::as_str).unwrap_or("");
        if !message.is_empty() || !code.is_empty() {
            let text = if !code.is_empty() && !message.is_empty() {
                format!("{code}: {message}")
            } else if !code.is_empty() {
                code.to_string()
            } else {
                message.to_string()
            };
            msgs.push(format!("Task failed: {text}"));
        }
    }
    msgs
}

fn parse_timestamp_value(v: Option<&Value>) -> Option<DateTime<Utc>> {
    let raw = v?.as_f64()?;
    let secs = raw as i64;
    let nanos = ((raw - secs as f64) * 1_000_000_000.0) as u32;
    Some(DateTime::<Utc>::from(
        std::time::UNIX_EPOCH + std::time::Duration::new(secs.max(0) as u64, nanos),
    ))
}

fn attempt_status_from_str(s: Option<&str>) -> AttemptStatus {
    match s.unwrap_or("") {
        "pending" => AttemptStatus::Pending,
        "in_progress" => AttemptStatus::InProgress,
        "completed" => AttemptStatus::Completed,
        "failed" => AttemptStatus::Failed,
        "cancelled" => AttemptStatus::Cancelled,
        _ => AttemptStatus::Unknown,
    }
}

fn map_task_list_item_to_summary(src: backend::TaskListItem) -> TaskSummary {
    fn env_label_from_status_display(v: Option<&HashMap<String, Value>>) -> Option<String> {
        let obj = v?;
        let raw = obj.get("environment_label")?;
        if let Some(s) = raw.as_str() {
            if s.trim().is_empty() {
                return None;
            }
            return Some(s.to_string());
        }
        if let Some(o) = raw.as_object() {
            // Best-effort support for rich shapes: { text: "..." } or { plain_text: "..." }
            if let Some(s) = o.get("text").and_then(Value::as_str)
                && !s.trim().is_empty()
            {
                return Some(s.to_string());
            }
            if let Some(s) = o.get("plain_text").and_then(Value::as_str)
                && !s.trim().is_empty()
            {
                return Some(s.to_string());
            }
            // Fallback: compact JSON for debugging
            if let Ok(s) = serde_json::to_string(o)
                && !s.is_empty()
            {
                return Some(s);
            }
        }
        None
    }

    // Best-effort parse of diff_stats (when present in latest_turn_status_display)
    fn diff_summary_from_status_display(v: Option<&HashMap<String, Value>>) -> DiffSummary {
        let mut out = DiffSummary::default();
        let Some(map) = v else { return out };
        let latest = map
            .get("latest_turn_status_display")
            .and_then(Value::as_object);
        let Some(latest) = latest else { return out };
        if let Some(ds) = latest.get("diff_stats").and_then(Value::as_object) {
            if let Some(n) = ds.get("files_modified").and_then(Value::as_i64) {
                out.files_changed = n.max(0) as usize;
            }
            if let Some(n) = ds.get("lines_added").and_then(Value::as_i64) {
                out.lines_added = n.max(0) as usize;
            }
            if let Some(n) = ds.get("lines_removed").and_then(Value::as_i64) {
                out.lines_removed = n.max(0) as usize;
            }
        }
        out
    }

    fn attempt_total_from_status_display(v: Option<&HashMap<String, Value>>) -> Option<usize> {
        let map = v?;
        let latest = map
            .get("latest_turn_status_display")
            .and_then(Value::as_object)?;
        let siblings = latest.get("sibling_turn_ids").and_then(Value::as_array)?;
        Some(siblings.len().saturating_add(1))
    }

    TaskSummary {
        id: TaskId(src.id),
        title: src.title,
        status: map_status(src.task_status_display.as_ref()),
        updated_at: parse_updated_at(src.updated_at.as_ref()),
        environment_id: None,
        environment_label: env_label_from_status_display(src.task_status_display.as_ref()),
        summary: diff_summary_from_status_display(src.task_status_display.as_ref()),
        is_review: src
            .pull_requests
            .as_ref()
            .is_some_and(|prs| !prs.is_empty()),
        attempt_total: attempt_total_from_status_display(src.task_status_display.as_ref()),
    }
}

fn map_status(v: Option<&HashMap<String, Value>>) -> TaskStatus {
    if let Some(val) = v {
        // Prefer nested latest_turn_status_display.turn_status when present.
        if let Some(turn) = val
            .get("latest_turn_status_display")
            .and_then(Value::as_object)
            && let Some(s) = turn.get("turn_status").and_then(Value::as_str)
        {
            return match s {
                "failed" => TaskStatus::Error,
                "completed" => TaskStatus::Ready,
                "in_progress" => TaskStatus::Pending,
                "pending" => TaskStatus::Pending,
                "cancelled" => TaskStatus::Error,
                _ => TaskStatus::Pending,
            };
        }
        // Legacy or alternative flat state.
        if let Some(state) = val.get("state").and_then(Value::as_str) {
            return match state {
                "pending" => TaskStatus::Pending,
                "ready" => TaskStatus::Ready,
                "applied" => TaskStatus::Applied,
                "error" => TaskStatus::Error,
                _ => TaskStatus::Pending,
            };
        }
    }
    TaskStatus::Pending
}

fn parse_updated_at(ts: Option<&f64>) -> DateTime<Utc> {
    if let Some(v) = ts {
        // Value is seconds since epoch with fractional part.
        let secs = *v as i64;
        let nanos = ((*v - secs as f64) * 1_000_000_000.0) as u32;
        return DateTime::<Utc>::from(
            std::time::UNIX_EPOCH + std::time::Duration::new(secs.max(0) as u64, nanos),
        );
    }
    Utc::now()
}

/// Return a compact one-line classification of the patch plus a short head snippet
/// to aid debugging when apply fails.
fn summarize_patch_for_logging(patch: &str) -> String {
    let trimmed = patch.trim_start();
    let kind = if trimmed.starts_with("*** Begin Patch") {
        "codex-patch"
    } else if trimmed.starts_with("diff --git ") || trimmed.contains("\n*** End Patch\n") {
        // In some cases providers nest a codex patch inside another format; detect both.
        "git-diff"
    } else if trimmed.starts_with("@@ ") || trimmed.contains("\n@@ ") {
        "unified-diff"
    } else {
        "unknown"
    };
    let lines = patch.lines().count();
    let chars = patch.len();
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<unknown>".to_string());
    // Grab the first up-to-20 non-empty lines for context.
    let head: String = patch.lines().take(20).collect::<Vec<&str>>().join("\n");
    // Make sure we don't explode logs with huge content.
    let head_trunc = if head.len() > 800 {
        format!("{}…", &head[..800])
    } else {
        head
    };
    format!(
        "patch_summary: kind={kind} lines={lines} chars={chars} cwd={cwd} ; head=\n{head_trunc}"
    )
}

fn append_error_log(message: &str) {
    let ts = Utc::now().to_rfc3339();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("error.log")
    {
        use std::io::Write as _;
        let _ = writeln!(f, "[{ts}] {message}");
    }
}
