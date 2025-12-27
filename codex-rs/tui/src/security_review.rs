#![allow(dead_code)]

use crate::app_event::AppEvent;
use crate::app_event::SecurityReviewAutoScopeSelection;
use crate::app_event::SecurityReviewCommandState;
use crate::app_event_sender::AppEventSender;
use crate::diff_render::display_path_for;
use crate::history_cell;
use crate::mermaid::fix_mermaid_blocks;
use crate::security_prompts::*;
use crate::security_report_viewer::build_report_html;
use crate::status_indicator_widget::fmt_elapsed_compact;
use crate::text_formatting::truncate_text;
use base64::Engine;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::WireApi;
use codex_core::config::Config;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config::load_global_mcp_servers;
use codex_core::config::types::McpServerConfig;
use codex_core::config::types::McpServerTransportConfig;
use codex_core::default_client::CodexHttpClient;
use codex_core::default_client::CodexRequestBuilder;
use codex_core::default_client::create_client;
use codex_core::default_retry_backoff;
use codex_core::features::Feature;
use codex_core::git_info::collect_git_info;
use codex_core::git_info::get_git_repo_root;
use codex_core::git_info::recent_commits;
use codex_core::mcp::auth::compute_auth_statuses;
use codex_core::protocol::EventMsg;
use codex_core::protocol::FinalOutput;
use codex_core::protocol::McpAuthStatus;
use codex_core::protocol::Op;
use codex_core::protocol::SessionSource;
use codex_core::protocol::SubAgentSource;
use codex_core::protocol::TokenUsage;
use codex_protocol::plan_tool::PlanItemArg;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;
use codex_protocol::user_input::UserInput;
use codex_rmcp_client::perform_oauth_login;
use codex_rmcp_client::supports_oauth_login;
use dirs::home_dir;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use pathdiff::diff_paths;
use regex::Regex;
use reqwest::header::ACCEPT;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;
use std::cmp::Ordering as CmpOrdering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fmt::Write;
use std::fs::OpenOptions;
use std::fs::{self};
use std::future::Future;
use std::io::Read;
use std::io::Write as IoWrite;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use tokio::fs as tokio_fs;
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio::task::spawn_blocking;
use tokio::time::sleep;
use url::Url;

const VALIDATION_SUMMARY_GRAPHEMES: usize = 96;
const VALIDATION_OUTPUT_GRAPHEMES: usize = 480;

//

// Heuristic limits inspired by the AppSec review agent to keep prompts manageable.
const DEFAULT_MAX_FILES: usize = usize::MAX;
const DEFAULT_MAX_BYTES_PER_FILE: usize = 500_000; // ~488 KiB
const DEFAULT_MAX_TOTAL_BYTES: usize = 7 * 1024 * 1024; // ~7 MiB
const MAX_PROMPT_BYTES: usize = 9_000_000; // ~8.6 MiB safety margin under API cap
const MAX_CONCURRENT_FILE_ANALYSIS: usize = 32;
const FILE_TRIAGE_CHUNK_SIZE: usize = 50;
const FILE_TRIAGE_CONCURRENCY: usize = 8;
const MAX_SEARCH_REQUESTS_PER_FILE: usize = 3;
const MAX_SEARCH_OUTPUT_CHARS: usize = 4_000;
const MAX_COMMAND_ERROR_RETRIES: usize = 10;
const MAX_SEARCH_PATTERN_LEN: usize = 256;
const MAX_FILE_SEARCH_RESULTS: usize = 40;
// Number of full passes over the triaged files during bug finding.
// Not related to per-file search/tool attempts. Defaults to 3.
const BUG_FINDING_PASSES: usize = 1;
const BUG_POLISH_CONCURRENCY: usize = 8;
const COMMAND_PREVIEW_MAX_LINES: usize = 2;
const COMMAND_PREVIEW_MAX_GRAPHEMES: usize = 96;
const MODEL_REASONING_LOG_MAX_GRAPHEMES: usize = 240;
const BUG_SCOPE_PROMPT_MAX_GRAPHEMES: usize = 600;
const ANALYSIS_CONTEXT_MAX_CHARS: usize = 6_000;
const AUTO_SCOPE_MODEL: &str = "gpt-5-codex";
const FILE_TRIAGE_MODEL: &str = "gpt-5-codex-mini";
const SPEC_GENERATION_MODEL: &str = "gpt-5-codex";
const THREAT_MODEL_MODEL: &str = "gpt-5-codex";
const CLASSIFICATION_PROMPT_SPEC_LIMIT: usize = 16_000;
// prompts moved to `security_prompts.rs`
const BUG_RERANK_CHUNK_SIZE: usize = 1;
const BUG_RERANK_MAX_CONCURRENCY: usize = 32;
const BUG_RERANK_CONTEXT_MAX_CHARS: usize = 2000;
const BUG_RERANK_MAX_TOOL_ROUNDS: usize = 4;
const BUG_RERANK_MAX_COMMAND_ERRORS: usize = 5;
const SPEC_COMBINE_MAX_TOOL_ROUNDS: usize = 6;
const SPEC_COMBINE_MAX_COMMAND_ERRORS: usize = 5;
// see BUG_RERANK_PROMPT_TEMPLATE in security_prompts
const SPEC_DIR_FILTER_TARGET: usize = 8;
// see SPEC_DIR_FILTER_SYSTEM_PROMPT in security_prompts
// see AUTO_SCOPE_* in security_prompts
const AUTO_SCOPE_MAX_PATHS: usize = 20;
const AUTO_SCOPE_MAX_KEYWORDS: usize = 6;
const AUTO_SCOPE_MAX_AGENT_STEPS: usize = 10;
const AUTO_SCOPE_INITIAL_KEYWORD_PROBES: usize = 4;
const AUTO_SCOPE_DEFAULT_READ_WINDOW: usize = 120;
const AUTO_SCOPE_DIRECTORY_LIST_LIMIT: usize = 200;
const AUTO_SCOPE_KEYWORD_STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "when", "where", "which", "while",
    "using", "use", "need", "please", "should", "scope", "scoped", "bug", "bugs", "review",
    "security", "analysis", "related", "request",
];
// see AUTO_SCOPE_KEYWORD_* in security_prompts
const AUTO_SCOPE_MARKER_FILES: [&str; 25] = [
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "yarn.lock",
    "requirements.txt",
    "pyproject.toml",
    "setup.py",
    "Pipfile",
    "Pipfile.lock",
    "Dockerfile",
    "docker-compose.yml",
    "Makefile",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "pom.xml",
    "go.mod",
    "go.sum",
    "Gemfile",
    "composer.json",
    "Procfile",
    "CMakeLists.txt",
];
pub(crate) const SECURITY_REVIEW_FOLLOW_UP_MARKER: &str = "[codex-security-review-follow-up]";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";

static EXCLUDED_DIR_NAMES: [&str; 13] = [
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "vendor",
    ".venv",
    "__pycache__",
    "dist",
    "build",
    ".idea",
    ".vscode",
    ".cache",
    "target",
];

pub fn sanitize_repo_slug(repo_path: &Path) -> String {
    let raw = repo_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| repo_path.to_string_lossy().into_owned());
    let mut slug = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };
        if mapped == '-' {
            if !slug.ends_with('-') {
                slug.push(mapped);
            }
        } else {
            slug.push(mapped);
        }
    }
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "repository".to_string()
    } else {
        trimmed
    }
}

fn encode_workspace_hint(repo_path: &Path) -> Option<String> {
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    let hostname = env::var("HOSTNAME").unwrap_or_else(|_| "unknown-host".to_string());
    let payload = json!({
        "hostname": hostname,
        "repo_root": canonical.to_string_lossy(),
    })
    .to_string();
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.as_bytes());
    Some(format!("[codex-workspace:{encoded}]"))
}

fn write_scope_file(
    output_root: &Path,
    repo_root: &Path,
    scope_display_paths: &[String],
    linear_issue: Option<&str>,
) -> std::io::Result<PathBuf> {
    let path = output_root.join("scope_paths.txt");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut content = String::new();
    content.push_str("Paths analyzed:\n");
    if scope_display_paths.is_empty() {
        content.push_str("- entire repository\n");
    } else {
        for path in scope_display_paths {
            content.push_str("- ");
            content.push_str(path);
            content.push('\n');
        }
    }
    content.push_str("\nRepo root: ");
    content.push_str(&repo_root.display().to_string());
    if let Some(issue) = linear_issue {
        content.push_str("\nLinear issue: ");
        content.push_str(issue);
    }

    fs::write(&path, content)?;
    Ok(path)
}

pub(crate) fn extract_linear_issue_ref(input: &str) -> Option<String> {
    for token in input.split_whitespace() {
        if let Some(rest) = token.strip_prefix("linear:") {
            let trimmed = rest.trim_matches(|ch| ch == ',' || ch == ';');
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)linear\.app/[^\s]+/issue/([A-Z0-9]+-[0-9]+)|\b([A-Z][A-Z0-9]+-[0-9]+)\b")
            .expect("linear regex compiles")
    });
    re.captures_iter(input)
        .find_map(|caps| caps.get(1).or_else(|| caps.get(2)))
        .map(|m| m.as_str().to_string())
}

fn build_step_title(step: &SecurityReviewPlanItem) -> String {
    if matches!(step.status, StepStatus::Completed)
        && let Some(duration) = step.duration()
    {
        let formatted = fmt_elapsed_compact(duration.as_secs());
        return format!("{} ({formatted})", step.title);
    }
    step.title.clone()
}

pub fn security_review_storage_root(repo_path: &Path) -> PathBuf {
    let base = env::var_os("CODEXHOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|dir| dir.join(".codex")))
        .unwrap_or_else(|| repo_path.to_path_buf());
    base.join("appsec_review")
        .join(sanitize_repo_slug(repo_path))
}

pub fn prepare_security_review_output_root(repo_path: &Path) -> std::io::Result<PathBuf> {
    let storage_root = security_review_storage_root(repo_path);
    fs::create_dir_all(&storage_root)?;
    let timestamp = OffsetDateTime::now_utc()
        .format(&format_description!(
            "[year][month][day]-[hour][minute][second]"
        ))
        .unwrap_or_else(|_| "run".to_string());
    let mut output_root = storage_root.join(&timestamp);
    let mut collision_counter: i64 = 1;
    while output_root.exists() {
        output_root = storage_root.join(format!("{timestamp}-{collision_counter:02}"));
        collision_counter = collision_counter.saturating_add(1);
    }
    fs::create_dir_all(&output_root)?;
    Ok(output_root)
}

fn resume_state_path(output_root: &Path) -> PathBuf {
    output_root.join("resume_state.json")
}

pub(crate) fn load_checkpoint(output_root: &Path) -> Option<SecurityReviewCheckpoint> {
    let path = resume_state_path(output_root);
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_checkpoint(
    output_root: &Path,
    checkpoint: &SecurityReviewCheckpoint,
) -> std::io::Result<()> {
    let path = resume_state_path(output_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(checkpoint)?;
    fs::write(path, bytes)
}

#[derive(Clone, Debug)]
pub struct RunningSecurityReviewCandidate {
    pub output_root: PathBuf,
    pub checkpoint: SecurityReviewCheckpoint,
}

pub fn latest_running_review_candidate(repo_path: &Path) -> Option<RunningSecurityReviewCandidate> {
    let storage_root = security_review_storage_root(repo_path);
    let entries = fs::read_dir(storage_root).ok()?;
    let mut candidates: Vec<(String, PathBuf, SecurityReviewCheckpoint)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(checkpoint) = load_checkpoint(&path) else {
            continue;
        };
        if checkpoint.status != SecurityReviewCheckpointStatus::Running {
            continue;
        }
        let name = entry
            .file_name()
            .to_str()
            .map(str::to_string)
            .unwrap_or_else(|| {
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            });
        candidates.push((name, path, checkpoint));
    }

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let (_, path, checkpoint) = candidates.into_iter().next()?;
    Some(RunningSecurityReviewCandidate {
        output_root: path,
        checkpoint,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SecurityReviewMode {
    #[default]
    Full,
    Bugs,
}

fn normalize_reasoning(reasoning: String) -> Option<String> {
    let trimmed = reasoning.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

impl SecurityReviewMode {
    pub fn as_str(self) -> &'static str {
        match self {
            SecurityReviewMode::Full => "full",
            SecurityReviewMode::Bugs => "bugs",
        }
    }
}

#[derive(Clone)]
pub struct SecurityReviewRequest {
    pub repo_path: PathBuf,
    pub include_paths: Vec<PathBuf>,
    pub scope_display_paths: Vec<String>,
    pub output_root: PathBuf,
    pub mode: SecurityReviewMode,
    pub include_spec_in_bug_analysis: bool,
    pub triage_model: String,
    pub model: String,
    pub provider: ModelProviderInfo,
    pub auth: Option<CodexAuth>,
    pub config: Config,
    pub auth_manager: Arc<AuthManager>,
    pub progress_sender: Option<AppEventSender>,
    pub log_sink: Option<Arc<SecurityReviewLogSink>>,
    pub progress_callback: Option<Arc<dyn Fn(String) + Send + Sync>>,
    // When true, accept auto-scoped directories without a confirmation dialog.
    pub skip_auto_scope_confirmation: bool,
    pub auto_scope_prompt: Option<String>,
    pub resume_checkpoint: Option<SecurityReviewCheckpoint>,
    // Optional Linear issue reference to sync status and create child tickets.
    pub linear_issue: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SecurityReviewSetupResult {
    pub logs: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SecurityReviewResult {
    pub findings_summary: String,
    pub bug_summary_table: Option<String>,
    pub bugs: Vec<SecurityReviewBug>,
    pub bugs_path: PathBuf,
    pub report_path: Option<PathBuf>,
    pub report_html_path: Option<PathBuf>,
    pub snapshot_path: PathBuf,
    pub metadata_path: PathBuf,
    pub api_overview_path: Option<PathBuf>,
    pub classification_json_path: Option<PathBuf>,
    pub classification_table_path: Option<PathBuf>,
    pub logs: Vec<String>,
    pub token_usage: TokenUsage,
    pub estimated_cost_usd: Option<f64>,
    pub rate_limit_wait: Duration,
}

#[derive(Clone, Debug)]
pub struct SecurityReviewFailure {
    pub message: String,
    pub logs: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SecurityReviewCheckpointStatus {
    Running,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityReviewCheckpoint {
    pub(crate) status: SecurityReviewCheckpointStatus,
    pub(crate) mode: SecurityReviewMode,
    pub(crate) include_paths: Vec<String>,
    pub(crate) scope_display_paths: Vec<String>,
    pub(crate) scope_file_path: Option<String>,
    pub(crate) auto_scope_prompt: Option<String>,
    pub(crate) triage_model: String,
    pub(crate) model: String,
    pub(crate) provider_name: String,
    pub(crate) repo_slug: String,
    pub(crate) repo_root: PathBuf,
    #[serde(with = "time::serde::rfc3339")]
    pub(crate) started_at: OffsetDateTime,
    pub(crate) plan_statuses: HashMap<String, StepStatus>,
    pub(crate) selected_snippets: Option<Vec<FileSnippet>>,
    pub(crate) spec: Option<StoredSpecOutcome>,
    pub(crate) threat_model: Option<StoredThreatModelOutcome>,
    pub(crate) bug_snapshot_path: Option<PathBuf>,
    pub(crate) bugs_path: Option<PathBuf>,
    pub(crate) report_path: Option<PathBuf>,
    pub(crate) report_html_path: Option<PathBuf>,
    pub(crate) api_overview_path: Option<PathBuf>,
    pub(crate) classification_json_path: Option<PathBuf>,
    pub(crate) classification_table_path: Option<PathBuf>,
    pub(crate) last_log: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StoredSpecOutcome {
    combined_markdown: String,
    locations: Vec<String>,
    logs: Vec<String>,
    api_entries: Vec<ApiEntry>,
    classification_rows: Vec<DataClassificationRow>,
    classification_table: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StoredThreatModelOutcome {
    markdown: String,
    logs: Vec<String>,
}

impl From<&SpecGenerationOutcome> for StoredSpecOutcome {
    fn from(spec: &SpecGenerationOutcome) -> Self {
        Self {
            combined_markdown: spec.combined_markdown.clone(),
            locations: spec.locations.clone(),
            logs: spec.logs.clone(),
            api_entries: spec.api_entries.clone(),
            classification_rows: spec.classification_rows.clone(),
            classification_table: spec.classification_table.clone(),
        }
    }
}

impl StoredSpecOutcome {
    fn into_outcome(self) -> SpecGenerationOutcome {
        SpecGenerationOutcome {
            combined_markdown: self.combined_markdown,
            locations: self.locations,
            logs: self.logs,
            api_entries: self.api_entries,
            classification_rows: self.classification_rows,
            classification_table: self.classification_table,
        }
    }
}

impl From<&ThreatModelOutcome> for StoredThreatModelOutcome {
    fn from(threat: &ThreatModelOutcome) -> Self {
        Self {
            markdown: threat.markdown.clone(),
            logs: threat.logs.clone(),
        }
    }
}

impl StoredThreatModelOutcome {
    fn into_outcome(self) -> ThreatModelOutcome {
        ThreatModelOutcome {
            markdown: self.markdown,
            logs: self.logs,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityReviewMetadata {
    pub mode: SecurityReviewMode,
    #[serde(default)]
    pub scope_paths: Vec<String>,
    #[serde(default)]
    pub git_commit: Option<String>,
    #[serde(default)]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub git_commit_timestamp: Option<i64>,
    #[serde(default)]
    pub linear_issue: Option<String>,
}

fn summarize_scope(scope_paths: &[PathBuf], repo_root: &Path) -> String {
    if scope_paths.is_empty() {
        return "entire repository".to_string();
    }

    let mut display_paths: Vec<String> = scope_paths
        .iter()
        .map(|path| display_path_for(path, repo_root))
        .collect();
    display_paths.sort();
    display_paths.dedup();
    let joined = display_paths.join(", ");
    truncate_text(joined.as_str(), 120)
}

fn is_open_source_review(prompt: &Option<String>) -> bool {
    let Some(text) = prompt.as_ref() else {
        return false;
    };
    let lower = text.to_ascii_lowercase();
    const KEYWORDS: &[&str] = &[
        "open source",
        "open-source",
        "open sourcing",
        "open-sourcing",
        "oss review",
        "oss drop",
        "open source this repo",
        "open source this project",
        "make this repo public",
    ];
    KEYWORDS.iter().any(|keyword| lower.contains(keyword))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SecurityReviewPlanStep {
    GenerateSpecs,
    ThreatModel,
    AnalyzeBugs,
    PolishFindings,
    AssembleReport,
}

#[derive(Clone)]
struct SecurityReviewPlanItem {
    kind: SecurityReviewPlanStep,
    title: String,
    status: StepStatus,
    started_at: Option<Instant>,
    completed_at: Option<Instant>,
}

impl SecurityReviewPlanItem {
    fn new(kind: SecurityReviewPlanStep, title: &str) -> Self {
        Self {
            kind,
            title: title.to_string(),
            status: StepStatus::Pending,
            started_at: None,
            completed_at: None,
        }
    }

    fn duration(&self) -> Option<Duration> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some(end.saturating_duration_since(start)),
            _ => None,
        }
    }
}

struct SecurityReviewPlanTracker {
    sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    explanation: String,
    steps: Vec<SecurityReviewPlanItem>,
}

impl SecurityReviewPlanTracker {
    fn new(
        mode: SecurityReviewMode,
        scope_paths: &[PathBuf],
        repo_root: &Path,
        sender: Option<AppEventSender>,
        log_sink: Option<Arc<SecurityReviewLogSink>>,
    ) -> Self {
        let steps = plan_steps_for_mode(mode);
        let scope_summary = summarize_scope(scope_paths, repo_root);
        let mode_label = mode.as_str();
        let explanation = format!("Security review plan ({mode_label}; scope: {scope_summary})");
        let tracker = Self {
            sender,
            log_sink,
            explanation,
            steps,
        };
        tracker.emit_update();
        tracker
    }

    fn complete_and_start_next(
        &mut self,
        finished: SecurityReviewPlanStep,
        next: Option<SecurityReviewPlanStep>,
    ) {
        let mut changed = self.set_status_if_present(finished, StepStatus::Completed);
        if let Some(next_step) = next {
            changed |= self.set_status_if_present(next_step, StepStatus::InProgress);
        }
        if changed {
            self.emit_update();
        }
    }

    fn start_step(&mut self, step: SecurityReviewPlanStep) {
        if self.set_status_if_present(step, StepStatus::InProgress) {
            self.emit_update();
        }
    }

    fn mark_complete(&mut self, step: SecurityReviewPlanStep) {
        if self.set_status_if_present(step, StepStatus::Completed) {
            self.emit_update();
        }
    }

    fn set_status_if_present(&mut self, step: SecurityReviewPlanStep, status: StepStatus) -> bool {
        let Some(entry) = self.steps.iter_mut().find(|item| item.kind == step) else {
            return false;
        };
        if matches!(status, StepStatus::InProgress) && entry.started_at.is_none() {
            entry.started_at = Some(Instant::now());
        }
        if matches!(status, StepStatus::Completed) {
            entry.started_at = entry.started_at.or_else(|| Some(Instant::now()));
            entry.completed_at = entry.completed_at.or_else(|| Some(Instant::now()));
        }
        if std::mem::discriminant(&entry.status) == std::mem::discriminant(&status) {
            return false;
        }
        entry.status = status;
        true
    }

    fn emit_update(&self) {
        let summary = self.build_log_summary();
        if let Some(sender) = self.sender.as_ref() {
            let plan_items: Vec<PlanItemArg> = self
                .steps
                .iter()
                .map(|step| PlanItemArg {
                    step: build_step_title(step),
                    status: step.status.clone(),
                })
                .collect();
            sender.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_plan_update(UpdatePlanArgs {
                    explanation: Some(self.explanation.clone()),
                    plan: plan_items,
                }),
            )));
            sender.send(AppEvent::SecurityReviewLog(summary.clone()));
        }
        write_log_sink(&self.log_sink, summary.as_str());
    }

    fn build_log_summary(&self) -> String {
        let mut parts: Vec<String> = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            let status = match step.status {
                StepStatus::Completed => "[done]",
                StepStatus::InProgress => "[doing]",
                StepStatus::Pending => "[todo]",
            };
            parts.push(format!("{status} {}", step.title));
        }
        format!("Plan update: {}", parts.join("; "))
    }

    fn restore_statuses(&mut self, statuses: &HashMap<String, StepStatus>) {
        let mut changed = false;
        for (slug, status) in statuses {
            if let Some(step) = plan_step_from_slug(slug) {
                changed |= self.set_status_if_present(step, status.clone());
            }
        }
        if changed {
            self.emit_update();
        }
    }

    fn snapshot_statuses(&self) -> HashMap<String, StepStatus> {
        let mut map = HashMap::new();
        for step in &self.steps {
            map.insert(plan_step_slug(step.kind).to_string(), step.status.clone());
        }
        map
    }

    fn status_for(&self, step: SecurityReviewPlanStep) -> Option<StepStatus> {
        self.steps
            .iter()
            .find(|entry| entry.kind == step)
            .map(|entry| entry.status.clone())
    }
}

fn plan_steps_for_mode(mode: SecurityReviewMode) -> Vec<SecurityReviewPlanItem> {
    let mut steps = Vec::new();

    if matches!(mode, SecurityReviewMode::Full) {
        steps.push(SecurityReviewPlanItem::new(
            SecurityReviewPlanStep::GenerateSpecs,
            "Generate system specifications",
        ));
        steps.push(SecurityReviewPlanItem::new(
            SecurityReviewPlanStep::ThreatModel,
            "Draft threat model",
        ));
    }

    steps.push(SecurityReviewPlanItem::new(
        SecurityReviewPlanStep::AnalyzeBugs,
        "Analyze code for bugs",
    ));
    steps.push(SecurityReviewPlanItem::new(
        SecurityReviewPlanStep::PolishFindings,
        "Polish, dedupe, and rerank findings",
    ));
    steps.push(SecurityReviewPlanItem::new(
        SecurityReviewPlanStep::AssembleReport,
        "Assemble report and artifacts",
    ));
    steps
}

fn plan_step_slug(step: SecurityReviewPlanStep) -> &'static str {
    match step {
        SecurityReviewPlanStep::GenerateSpecs => "generate_specs",
        SecurityReviewPlanStep::ThreatModel => "threat_model",
        SecurityReviewPlanStep::AnalyzeBugs => "analyze_bugs",
        SecurityReviewPlanStep::PolishFindings => "polish_findings",
        SecurityReviewPlanStep::AssembleReport => "assemble_report",
    }
}

fn plan_step_from_slug(slug: &str) -> Option<SecurityReviewPlanStep> {
    match slug {
        "generate_specs" => Some(SecurityReviewPlanStep::GenerateSpecs),
        "threat_model" => Some(SecurityReviewPlanStep::ThreatModel),
        "analyze_bugs" => Some(SecurityReviewPlanStep::AnalyzeBugs),
        "polish_findings" => Some(SecurityReviewPlanStep::PolishFindings),
        "assemble_report" => Some(SecurityReviewPlanStep::AssembleReport),
        _ => None,
    }
}

fn default_plan_statuses(mode: SecurityReviewMode) -> HashMap<String, StepStatus> {
    let mut statuses = HashMap::new();
    for step in plan_steps_for_mode(mode) {
        statuses.insert(plan_step_slug(step.kind).to_string(), StepStatus::Pending);
    }
    statuses
}

pub(crate) fn read_security_review_metadata(path: &Path) -> Result<SecurityReviewMetadata, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_json::from_slice::<SecurityReviewMetadata>(&bytes)
        .map_err(|e| format!("Failed to parse {}: {e}", path.display()))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct FileSnippet {
    relative_path: PathBuf,
    language: String,
    content: String,
    bytes: usize,
}

struct FileCollectionResult {
    snippets: Vec<FileSnippet>,
    logs: Vec<String>,
}

#[derive(Clone)]
pub struct SecurityReviewLogSink {
    path: Option<PathBuf>,
    callback: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

impl SecurityReviewLogSink {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(Self {
            path: Some(path.to_path_buf()),
            callback: None,
        })
    }

    pub fn with_callback(callback: Arc<dyn Fn(String) + Send + Sync>) -> Self {
        Self {
            path: None,
            callback: Some(callback),
        }
    }

    pub fn with_path_and_callback(
        path: &Path,
        callback: Arc<dyn Fn(String) + Send + Sync>,
    ) -> std::io::Result<Self> {
        let mut sink = Self::new(path)?;
        sink.callback = Some(callback);
        Ok(sink)
    }

    fn write(&self, message: &str) {
        if let Some(callback) = self.callback.as_ref() {
            callback(message.to_string());
        }

        if let Some(path) = self.path.as_ref() {
            let timestamp = OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .unwrap_or_else(|_| "unknown-time".to_string());
            let mut line = String::new();
            let _ = writeln!(&mut line, "{timestamp} {message}");
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }
}

struct BugAnalysisOutcome {
    bug_markdown: String,
    bug_summary_table: Option<String>,
    findings_count: usize,
    bug_summaries: Vec<BugSummary>,
    bug_details: Vec<BugDetail>,
    files_with_findings: Vec<FileSnippet>,
    logs: Vec<String>,
}

#[derive(Default)]
struct ReviewMetrics {
    model_calls: AtomicUsize,
    search_calls: AtomicUsize,
    grep_files_calls: AtomicUsize,
    read_calls: AtomicUsize,
    exec_calls: AtomicUsize,
    git_blame_calls: AtomicUsize,
    command_seq: AtomicU64,
    rate_limit_wait_ns: AtomicU64,

    // Aggregated token usage across all model calls
    input_tokens: AtomicI64,
    cached_input_tokens: AtomicI64,
    output_tokens: AtomicI64,
    reasoning_output_tokens: AtomicI64,
    total_tokens: AtomicI64,
}

#[derive(Clone, Copy)]
enum ToolCallKind {
    Search,
    GrepFiles,
    ReadFile,
    Exec,
    GitBlame,
}

struct MetricsSnapshot {
    model_calls: usize,
    search_calls: usize,
    grep_files_calls: usize,
    read_calls: usize,
    exec_calls: usize,
    git_blame_calls: usize,
}

impl MetricsSnapshot {
    fn tool_call_summary(&self) -> String {
        let mut parts = Vec::new();
        if self.search_calls > 0 {
            parts.push(format!("search {count}", count = self.search_calls));
        }
        if self.grep_files_calls > 0 {
            parts.push(format!("grep files {count}", count = self.grep_files_calls));
        }
        if self.read_calls > 0 {
            parts.push(format!("read {count}", count = self.read_calls));
        }
        if self.exec_calls > 0 {
            parts.push(format!("exec {count}", count = self.exec_calls));
        }
        if self.git_blame_calls > 0 {
            parts.push(format!("git blame {count}", count = self.git_blame_calls));
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(", ")
        }
    }
}

impl ReviewMetrics {
    fn record_model_call(&self) {
        self.model_calls.fetch_add(1, Ordering::Relaxed);
    }

    fn record_tool_call(&self, kind: ToolCallKind) {
        match kind {
            ToolCallKind::Search => {
                self.search_calls.fetch_add(1, Ordering::Relaxed);
            }
            ToolCallKind::GrepFiles => {
                self.grep_files_calls.fetch_add(1, Ordering::Relaxed);
            }
            ToolCallKind::ReadFile => {
                self.read_calls.fetch_add(1, Ordering::Relaxed);
            }
            ToolCallKind::Exec => {
                self.exec_calls.fetch_add(1, Ordering::Relaxed);
            }
            ToolCallKind::GitBlame => {
                self.git_blame_calls.fetch_add(1, Ordering::Relaxed);
            }
        };
    }

    fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            model_calls: self.model_calls.load(Ordering::Relaxed),
            search_calls: self.search_calls.load(Ordering::Relaxed),
            grep_files_calls: self.grep_files_calls.load(Ordering::Relaxed),
            read_calls: self.read_calls.load(Ordering::Relaxed),
            exec_calls: self.exec_calls.load(Ordering::Relaxed),
            git_blame_calls: self.git_blame_calls.load(Ordering::Relaxed),
        }
    }

    fn next_command_id(&self) -> u64 {
        self.command_seq
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1)
    }

    fn record_usage(&self, usage: &TokenUsage) {
        use std::sync::atomic::Ordering::Relaxed;
        self.input_tokens.fetch_add(usage.input_tokens, Relaxed);
        self.cached_input_tokens
            .fetch_add(usage.cached_input_tokens, Relaxed);
        self.output_tokens.fetch_add(usage.output_tokens, Relaxed);
        self.reasoning_output_tokens
            .fetch_add(usage.reasoning_output_tokens, Relaxed);
        self.total_tokens.fetch_add(usage.total_tokens, Relaxed);
    }

    fn record_usage_raw(
        &self,
        input_tokens: i64,
        cached_input_tokens: i64,
        output_tokens: i64,
        reasoning_output_tokens: i64,
        total_tokens: i64,
    ) {
        let usage = TokenUsage {
            input_tokens,
            cached_input_tokens,
            output_tokens,
            reasoning_output_tokens,
            total_tokens,
        };
        self.record_usage(&usage);
    }

    fn snapshot_usage(&self) -> TokenUsage {
        use std::sync::atomic::Ordering::Relaxed;
        TokenUsage {
            input_tokens: self.input_tokens.load(Relaxed),
            cached_input_tokens: self.cached_input_tokens.load(Relaxed),
            output_tokens: self.output_tokens.load(Relaxed),
            reasoning_output_tokens: self.reasoning_output_tokens.load(Relaxed),
            total_tokens: self.total_tokens.load(Relaxed),
        }
    }

    fn record_rate_limit_wait(&self, duration: Duration) {
        let nanos = duration.as_nanos().min(u64::MAX as u128) as u64;
        self.rate_limit_wait_ns.fetch_add(nanos, Ordering::Relaxed);
    }

    fn rate_limit_wait(&self) -> Duration {
        Duration::from_nanos(self.rate_limit_wait_ns.load(Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
struct ModelPricing {
    prompt_rate: f64,
    completion_rate: f64,
    cache_read_rate: Option<f64>,
}

fn record_tool_call_from_event(metrics: &ReviewMetrics, event: &EventMsg) {
    match event {
        EventMsg::WebSearchBegin(_) => metrics.record_tool_call(ToolCallKind::Search),
        EventMsg::McpToolCallBegin(begin) => {
            let tool = begin.invocation.tool.as_str();
            if tool.eq_ignore_ascii_case("web_search") || tool.eq_ignore_ascii_case("search") {
                metrics.record_tool_call(ToolCallKind::Search);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModelEntry>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelEntry {
    id: String,
    #[serde(default)]
    canonical_slug: Option<String>,
    #[serde(default)]
    pricing: Option<OpenRouterPricing>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterPricing {
    prompt: Option<PriceValue>,
    completion: Option<PriceValue>,
    #[serde(default)]
    input_cache_read: Option<PriceValue>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PriceValue {
    String(String),
    Number(f64),
}

impl PriceValue {
    fn as_rate(&self) -> Option<f64> {
        let raw = match self {
            PriceValue::String(value) => value.parse::<f64>().ok()?,
            PriceValue::Number(value) => *value,
        };
        if raw < 0.0 { None } else { Some(raw) }
    }
}

#[derive(Debug, Clone)]
struct CostBreakdown {
    total: f64,
    prompt_tokens: i64,
    prompt_rate: f64,
    prompt_cost: f64,
    completion_tokens: i64,
    completion_rate: f64,
    completion_cost: f64,
    cache_tokens: i64,
    cache_rate: Option<f64>,
    cache_cost: f64,
}

async fn fetch_openrouter_pricing(
    client: &CodexHttpClient,
    model: &str,
) -> Result<Option<ModelPricing>, String> {
    let response = client
        .get(OPENROUTER_MODELS_URL)
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|err| format!("request error: {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "HTTP {} from OpenRouter models API",
            response.status()
        ));
    }

    let body = response
        .bytes()
        .await
        .map_err(|err| format!("failed to read response body: {err}"))?;

    let payload: OpenRouterModelsResponse =
        serde_json::from_slice(&body).map_err(|err| format!("parse error: {err}"))?;

    let normalized = model.trim().to_ascii_lowercase();
    for entry in payload.data {
        if !matches_model_entry(&entry, &normalized) {
            continue;
        }

        if let Some(pricing) = entry.pricing {
            let OpenRouterPricing {
                prompt,
                completion,
                input_cache_read,
            } = pricing;
            let prompt_rate = prompt.and_then(|value| value.as_rate());
            let completion_rate = completion.and_then(|value| value.as_rate());
            if let (Some(prompt_rate), Some(completion_rate)) = (prompt_rate, completion_rate) {
                let cache_read_rate = input_cache_read.and_then(|value| value.as_rate());
                return Ok(Some(ModelPricing {
                    prompt_rate,
                    completion_rate,
                    cache_read_rate,
                }));
            }
        }
        break;
    }

    Ok(None)
}

fn matches_model_entry(entry: &OpenRouterModelEntry, normalized_model: &str) -> bool {
    let id = entry.id.to_ascii_lowercase();
    if matches_model_value(&id, normalized_model) {
        return true;
    }

    if let Some(slug) = entry.canonical_slug.as_ref() {
        let slug_lc = slug.to_ascii_lowercase();
        if matches_model_value(&slug_lc, normalized_model) {
            return true;
        }
    }

    false
}

fn matches_model_value(value: &str, model: &str) -> bool {
    if value == model {
        return true;
    }

    if let Some(segment) = value.rsplit('/').next() {
        if segment == model {
            return true;
        }
        if let Some(remainder) = segment.strip_prefix(model) {
            if remainder.is_empty() {
                return true;
            }
            if remainder.starts_with(['-', ':']) {
                return true;
            }
        }
    }

    false
}

fn compute_cost_breakdown(token_usage: &TokenUsage, pricing: &ModelPricing) -> CostBreakdown {
    let prompt_tokens = token_usage.non_cached_input();
    let cache_tokens = token_usage.cached_input();
    let completion_tokens = token_usage.output_tokens.max(0);

    let prompt_cost = pricing.prompt_rate * (prompt_tokens as f64);
    let cache_cost = pricing
        .cache_read_rate
        .map(|rate| rate * (cache_tokens as f64))
        .unwrap_or(0.0);
    let completion_cost = pricing.completion_rate * (completion_tokens as f64);

    CostBreakdown {
        total: prompt_cost + cache_cost + completion_cost,
        prompt_tokens,
        prompt_rate: pricing.prompt_rate,
        prompt_cost,
        completion_tokens,
        completion_rate: pricing.completion_rate,
        completion_cost,
        cache_tokens,
        cache_rate: pricing.cache_read_rate,
        cache_cost,
    }
}

struct FileBugResult {
    index: usize,
    path_display: String,
    duration: Duration,
    logs: Vec<String>,
    bug_section: Option<String>,
    snippet: Option<FileSnippet>,
    findings_count: usize,
}

struct FileTriageResult {
    included: Vec<FileSnippet>,
    logs: Vec<String>,
}

#[derive(Clone)]
struct FileTriageDescriptor {
    id: usize,
    path: String,
    listing_json: String,
}

#[derive(Clone)]
struct FileTriageChunkRequest {
    start_idx: usize,
    end_idx: usize,
    descriptors: Vec<FileTriageDescriptor>,
}

struct FileTriageChunkResult {
    include_ids: Vec<usize>,
    logs: Vec<String>,
    processed: usize,
}

#[derive(Clone)]
struct SpecEntry {
    location_label: String,
    markdown: String,
    raw_path: PathBuf,
    api_markdown: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct ApiEntry {
    location_label: String,
    markdown: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DataClassificationRow {
    data_type: String,
    sensitivity: String,
    storage_location: String,
    retention: String,
    encryption_at_rest: String,
    in_transit: String,
    accessed_by: String,
}

#[derive(Clone)]
struct SpecGenerationOutcome {
    combined_markdown: String,
    locations: Vec<String>,
    logs: Vec<String>,
    api_entries: Vec<ApiEntry>,
    classification_rows: Vec<DataClassificationRow>,
    classification_table: Option<String>,
}

struct SpecThreatOutcome {
    spec: Option<SpecGenerationOutcome>,
    threat: Option<ThreatModelOutcome>,
}

struct AutoScopeSelection {
    abs_path: PathBuf,
    display_path: String,
    reason: Option<String>,
    is_dir: bool,
}

fn truncate_auto_scope_selections(
    selections: &mut Vec<AutoScopeSelection>,
    logs: &mut Vec<String>,
) {
    if selections.len() > AUTO_SCOPE_MAX_PATHS {
        selections.truncate(AUTO_SCOPE_MAX_PATHS);
        logs.push(format!(
            "Auto scope limited to the first {AUTO_SCOPE_MAX_PATHS} paths returned by the model."
        ));
    }
}

fn prune_auto_scope_parent_child_overlaps(
    selections: &mut Vec<AutoScopeSelection>,
    logs: &mut Vec<String>,
) {
    if selections.len() <= 1 {
        return;
    }

    // Only prune parent directories when a child directory is already included.
    let mut directory_indices: Vec<usize> = selections
        .iter()
        .enumerate()
        .filter_map(|(idx, sel)| sel.is_dir.then_some(idx))
        .collect();
    if directory_indices.len() <= 1 {
        return;
    }

    directory_indices.sort_by(|&a, &b| {
        let da = selections[a].abs_path.components().count();
        let db = selections[b].abs_path.components().count();
        db.cmp(&da)
    });

    let mut kept_dirs: Vec<PathBuf> = Vec::new();
    let mut pruned_indices: HashSet<usize> = HashSet::new();

    for idx in directory_indices {
        let current = &selections[idx];
        if kept_dirs
            .iter()
            .any(|kept| kept.starts_with(current.abs_path.as_path()))
        {
            pruned_indices.insert(idx);
        } else {
            kept_dirs.push(current.abs_path.clone());
        }
    }

    if pruned_indices.is_empty() {
        return;
    }

    let pruned = pruned_indices.len();
    let mut filtered: Vec<AutoScopeSelection> = Vec::with_capacity(selections.len() - pruned);
    for (idx, sel) in selections.drain(..).enumerate() {
        if pruned_indices.contains(&idx) {
            continue;
        }
        filtered.push(sel);
    }
    *selections = filtered;
    logs.push(format!(
        "Auto scope pruned {pruned} parent directories due to overlap."
    ));
}

fn summarize_top_level(repo_root: &Path) -> String {
    let mut directories: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();

    if let Ok(entries) = fs::read_dir(repo_root) {
        for entry_result in entries.flatten().take(64) {
            let name = entry_result.file_name().to_string_lossy().into_owned();
            match entry_result.file_type() {
                Ok(ft) if ft.is_dir() => {
                    if is_auto_scope_excluded_dir(&name) {
                        continue;
                    }
                    directories.push(format!("{name}/"));
                }
                Ok(ft) if ft.is_file() => files.push(name),
                _ => {}
            }
        }
    }

    directories.sort();
    files.sort();

    let mut summary = Vec::new();
    if directories.is_empty() && files.is_empty() {
        summary.push("No top-level entries detected.".to_string());
    } else {
        if !directories.is_empty() {
            summary.push(format!("Directories: {}", directories.join(", ")));
        }
        if !files.is_empty() {
            summary.push(format!("Files: {}", files.join(", ")));
        }
    }

    summary.join("\n")
}

#[derive(Debug, Clone)]
struct GrepFilesArgs {
    pattern: String,
    include: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
}

enum AutoScopeToolCommand {
    SearchContent {
        pattern: String,
        mode: SearchMode,
    },
    GrepFiles(GrepFilesArgs),
    ReadFile {
        path: PathBuf,
        start: Option<usize>,
        end: Option<usize>,
    },
}

fn extract_auto_scope_commands(response: &str) -> Vec<AutoScopeToolCommand> {
    let mut commands = Vec::new();
    for line in response.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("SEARCH_FILES:") {
            let (mode, term) = parse_search_term(rest.trim_matches('`'));
            if !term.is_empty() {
                // Deprecated: map SEARCH_FILES to content search.
                commands.push(AutoScopeToolCommand::SearchContent {
                    pattern: term.to_string(),
                    mode,
                });
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("GREP_FILES:") {
            let spec = rest.trim();
            if !spec.is_empty()
                && let Ok(args) =
                    serde_json::from_str::<serde_json::Value>(spec).map(|v| GrepFilesArgs {
                        pattern: v
                            .get("pattern")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim()
                            .to_string(),
                        include: v
                            .get("include")
                            .and_then(Value::as_str)
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty()),
                        path: v
                            .get("path")
                            .and_then(Value::as_str)
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty()),
                        limit: v.get("limit").and_then(Value::as_u64).map(|n| n as usize),
                    })
                && !args.pattern.is_empty()
            {
                commands.push(AutoScopeToolCommand::GrepFiles(args));
                continue;
            }
        }
        if let Some(rest) = trimmed.strip_prefix("SEARCH:") {
            let (mode, term) = parse_search_term(rest.trim_matches('`'));
            if !term.is_empty() {
                commands.push(AutoScopeToolCommand::SearchContent {
                    pattern: term.to_string(),
                    mode,
                });
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("READ:") {
            let spec = rest.trim();
            if spec.is_empty() {
                continue;
            }
            let (path_part, range_part) = spec.split_once('#').unwrap_or((spec, ""));
            let relative = Path::new(path_part.trim()).to_path_buf();
            if relative.as_os_str().is_empty() || relative.is_absolute() {
                continue;
            }

            let mut start = None;
            let mut end = None;
            if let Some(range) = range_part.strip_prefix('L') {
                let mut parts = range.split('-');
                if let Some(start_str) = parts.next()
                    && let Ok(value) = start_str.trim().parse::<usize>()
                    && value > 0
                {
                    start = Some(value);
                }
                if let Some(end_str) = parts.next() {
                    let clean_end = end_str.trim().trim_start_matches('L');
                    if let Ok(value) = clean_end.parse::<usize>()
                        && value > 0
                    {
                        end = Some(value);
                    }
                }
            }

            commands.push(AutoScopeToolCommand::ReadFile {
                path: relative,
                start,
                end,
            });
        }
    }
    commands
}

async fn execute_auto_scope_search_content(
    repo_root: &Path,
    pattern: &str,
    mode: SearchMode,
    metrics: &Arc<ReviewMetrics>,
) -> (String, String) {
    match run_content_search(repo_root, pattern, mode, metrics).await {
        SearchResult::Matches(output) => (
            format!("Auto scope content search `{pattern}` returned results."),
            output,
        ),
        SearchResult::NoMatches => (
            format!("Auto scope content search `{pattern}` returned no matches."),
            "No matches found.".to_string(),
        ),
        SearchResult::Error(err) => (
            format!("Auto scope content search `{pattern}` failed: {err}"),
            format!("Search error: {err}"),
        ),
    }
}

async fn run_grep_files(
    repo_root: &Path,
    args: &GrepFilesArgs,
    metrics: &Arc<ReviewMetrics>,
) -> SearchResult {
    let pattern = args.pattern.trim();
    if pattern.is_empty() {
        return SearchResult::NoMatches;
    }
    let limit = args.limit.unwrap_or(100).min(2000);

    metrics.record_tool_call(ToolCallKind::GrepFiles);
    metrics.record_tool_call(ToolCallKind::Exec);
    let mut command = Command::new("rg");
    command
        .arg("--files-with-matches")
        .arg("--sortr=modified")
        .arg("--regexp")
        .arg(pattern)
        .arg("--no-messages")
        .current_dir(repo_root);

    if let Some(glob) = args.include.as_deref()
        && !glob.is_empty()
    {
        command.arg("--glob").arg(glob);
    }

    let search_path = if let Some(path) = args.path.as_deref() {
        if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            repo_root.join(path)
        }
    } else {
        repo_root.to_path_buf()
    };
    command.arg("--").arg(&search_path);

    let output = match command.output().await {
        Ok(o) => o,
        Err(err) => return SearchResult::Error(format!("failed to run rg: {err}")),
    };

    match output.status.code() {
        Some(0) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut lines = Vec::new();
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                lines.push(format!("- {trimmed}"));
                if lines.len() == limit {
                    break;
                }
            }
            if lines.is_empty() {
                SearchResult::NoMatches
            } else {
                let mut text = lines.join("\n");
                if text.len() > MAX_SEARCH_OUTPUT_CHARS {
                    text.truncate(MAX_SEARCH_OUTPUT_CHARS);
                    text.push_str("\n... (truncated)");
                }
                SearchResult::Matches(text)
            }
        }
        Some(1) => SearchResult::NoMatches,
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            // Retry with fixed-strings if the regex failed to parse.
            if stderr.contains("regex parse error")
                || stderr.contains("error parsing regex")
                || stderr.contains("unclosed group")
            {
                let mut fixed = Command::new("rg");
                fixed
                    .arg("--files-with-matches")
                    .arg("--sortr=modified")
                    .arg("--fixed-strings")
                    .arg(pattern)
                    .arg("--no-messages")
                    .current_dir(repo_root);
                if let Some(glob) = args.include.as_deref()
                    && !glob.is_empty()
                {
                    fixed.arg("--glob").arg(glob);
                }
                fixed.arg("--").arg(&search_path);
                let second = match fixed.output().await {
                    Ok(o) => o,
                    Err(err) => return SearchResult::Error(format!("failed to run rg: {err}")),
                };
                return match second.status.code() {
                    Some(0) => {
                        let stdout = String::from_utf8_lossy(&second.stdout);
                        let mut lines = Vec::new();
                        for line in stdout.lines() {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            lines.push(format!("- {trimmed}"));
                            if lines.len() == limit {
                                break;
                            }
                        }
                        if lines.is_empty() {
                            SearchResult::NoMatches
                        } else {
                            let mut text = lines.join("\n");
                            if text.len() > MAX_SEARCH_OUTPUT_CHARS {
                                text.truncate(MAX_SEARCH_OUTPUT_CHARS);
                                text.push_str("\n... (truncated)");
                            }
                            SearchResult::Matches(text)
                        }
                    }
                    Some(1) => SearchResult::NoMatches,
                    _ => {
                        let err2 = String::from_utf8_lossy(&second.stderr).trim().to_string();
                        if err2.is_empty() {
                            SearchResult::Error("rg returned an error".to_string())
                        } else {
                            SearchResult::Error(format!("rg error: {err2}"))
                        }
                    }
                };
            }
            if stderr.is_empty() {
                SearchResult::Error("rg returned an error".to_string())
            } else {
                SearchResult::Error(format!("rg error: {stderr}"))
            }
        }
    }
}

async fn execute_auto_scope_read(
    repo_root: &Path,
    command_path: &Path,
    command: ReadCommand,
    start: Option<usize>,
    end: Option<usize>,
    metrics: &ReviewMetrics,
) -> Result<String, String> {
    metrics.record_tool_call(ToolCallKind::ReadFile);
    metrics.record_tool_call(ToolCallKind::Exec);
    let absolute = repo_root.join(command_path);
    let canonical = absolute
        .canonicalize()
        .map_err(|err| format!("Failed to resolve path {}: {err}", command_path.display()))?;
    if !canonical.starts_with(repo_root) {
        return Err(format!(
            "Path {} escapes the repository root.",
            command_path.display()
        ));
    }
    let metadata = tokio_fs::metadata(&canonical)
        .await
        .map_err(|err| format!("Failed to inspect {}: {err}", command_path.display()))?;

    if command == ReadCommand::ListDir {
        if !metadata.is_dir() {
            return Err(format!(
                "Path {} is not a directory; LIST_DIR only supports directories.",
                command_path.display()
            ));
        }
        return build_directory_listing(repo_root, &canonical, command_path).await;
    }

    if metadata.is_dir() {
        return build_directory_listing(repo_root, &canonical, command_path).await;
    }

    if !metadata.is_file() {
        return Err(format!(
            "Path {} is not a regular file.",
            command_path.display()
        ));
    }

    let content = tokio_fs::read_to_string(&canonical)
        .await
        .map_err(|err| format!("Failed to read {}: {err}", command_path.display()))?;

    let relative = display_path_for(&canonical, repo_root);
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Ok(format!("{relative} is empty."));
    }

    let total_lines = lines.len();
    let start_line = start.unwrap_or(1).max(1).min(total_lines);
    let end_line = end
        .unwrap_or(start_line.saturating_add(AUTO_SCOPE_DEFAULT_READ_WINDOW))
        .max(start_line)
        .min(total_lines);

    let slice = &lines[start_line - 1..end_line];
    let mut formatted = format!("{relative} (L{start_line}-L{end_line}):\n");
    for (idx, line) in slice.iter().enumerate() {
        let line_number = start_line + idx;
        formatted.push_str(&format!("{line_number:>6}: {line}\n"));
        if formatted.len() > 8000 {
            formatted.push_str("... (truncated)\n");
            break;
        }
    }
    Ok(formatted.trim_end().to_string())
}

async fn build_directory_listing(
    repo_root: &Path,
    canonical: &Path,
    command_path: &Path,
) -> Result<String, String> {
    let mut reader = tokio_fs::read_dir(canonical)
        .await
        .map_err(|err| format!("Failed to read directory {}: {err}", command_path.display()))?;
    let mut entries: Vec<(bool, String)> = Vec::new();

    while let Some(entry) = reader
        .next_entry()
        .await
        .map_err(|err| format!("Failed to read directory {}: {err}", command_path.display()))?
    {
        let file_type = entry
            .file_type()
            .await
            .map_err(|err| format!("Failed to inspect {}: {err}", entry.path().display()))?;
        let mut label = display_path_for(&entry.path(), repo_root);
        if file_type.is_dir() {
            label.push('/');
        } else if file_type.is_symlink() {
            label.push('@');
        }
        entries.push((file_type.is_dir(), label));
    }

    entries.sort_by(|a, b| match b.0.cmp(&a.0) {
        CmpOrdering::Equal => a.1.cmp(&b.1),
        other => other,
    });

    let directory_label = display_path_for(canonical, repo_root);
    if entries.is_empty() {
        return Ok(format!(
            "{directory_label} is a directory with no entries. Use READ on specific files or run `ls {directory_label}` as needed."
        ));
    }

    let mut limited = entries
        .into_iter()
        .map(|(_, label)| label)
        .collect::<Vec<String>>();
    let omitted = limited
        .len()
        .saturating_sub(AUTO_SCOPE_DIRECTORY_LIST_LIMIT);
    if omitted > 0 {
        limited.truncate(AUTO_SCOPE_DIRECTORY_LIST_LIMIT);
    }

    let mut message = format!(
        "{directory_label} is a directory. Use READ on specific files or run `ls {directory_label}` to explore. Entries:\n"
    );
    for entry in &limited {
        let _ = writeln!(message, "- {entry}");
    }
    if omitted > 0 {
        let _ = writeln!(
            message,
            "… {omitted} more entr{} not shown.",
            if omitted == 1 { "y" } else { "ies" }
        );
    }

    Ok(message.trim_end().to_string())
}

fn is_path_lookup_error(err: &str) -> bool {
    err.starts_with("Failed to resolve path")
        || err.starts_with("Failed to inspect")
        || err.starts_with("Failed to read")
        || err.contains("escapes the repository root")
}

fn build_auto_scope_prompt(
    repo_overview: &str,
    user_query: &str,
    keywords: &[String],
    conversation: &str,
) -> String {
    let keywords_section = if keywords.is_empty() {
        "None".to_string()
    } else {
        keywords
            .iter()
            .map(|keyword| format!("- {keyword}"))
            .collect::<Vec<String>>()
            .join("\n")
    };
    let conversation_section = if conversation.trim().is_empty() {
        "No prior exchanges.".to_string()
    } else {
        conversation.to_string()
    };
    let base = AUTO_SCOPE_PROMPT_TEMPLATE
        .replace("{repo_overview}", repo_overview)
        .replace("{user_query}", user_query.trim())
        .replace("{keywords}", &keywords_section)
        .replace("{conversation}", &conversation_section)
        .replace("{read_window}", &AUTO_SCOPE_DEFAULT_READ_WINDOW.to_string());
    format!("{base}\n{AUTO_SCOPE_JSON_GUARD}")
}

struct ThreatModelOutcome {
    markdown: String,
    logs: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum BugValidationStatus {
    #[default]
    Pending,
    Passed,
    Failed,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BugValidationState {
    pub status: BugValidationStatus,
    pub tool: Option<String>,
    pub target: Option<String>,
    pub summary: Option<String>,
    pub output_snippet: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub run_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug)]
struct BugSummary {
    id: usize,
    title: String,
    file: String,
    severity: String,
    impact: String,
    likelihood: String,
    recommendation: String,
    blame: Option<String>,
    risk_score: Option<f32>,
    risk_rank: Option<usize>,
    risk_reason: Option<String>,
    verification_types: Vec<String>,
    vulnerability_tag: Option<String>,
    validation: BugValidationState,
    source_path: PathBuf,
    markdown: String,
    author_github: Option<String>,
}

#[derive(Clone, Debug)]
struct BugDetail {
    summary_id: usize,
    original_markdown: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityReviewBug {
    pub summary_id: usize,
    pub risk_rank: Option<usize>,
    pub risk_score: Option<f32>,
    pub title: String,
    pub severity: String,
    pub impact: String,
    pub likelihood: String,
    pub recommendation: String,
    pub file: String,
    pub blame: Option<String>,
    pub risk_reason: Option<String>,
    pub verification_types: Vec<String>,
    pub vulnerability_tag: Option<String>,
    pub validation: BugValidationState,
    pub assignee_github: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BugSnapshot {
    #[serde(flatten)]
    bug: SecurityReviewBug,
    original_markdown: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SecurityReviewSnapshot {
    #[serde(with = "time::serde::rfc3339")]
    generated_at: OffsetDateTime,
    findings_summary: String,
    report_sections_prefix: Vec<String>,
    bugs: Vec<BugSnapshot>,
}

struct PersistedArtifacts {
    bugs_path: PathBuf,
    snapshot_path: PathBuf,
    report_path: Option<PathBuf>,
    report_html_path: Option<PathBuf>,
    metadata_path: PathBuf,
    api_overview_path: Option<PathBuf>,
    classification_json_path: Option<PathBuf>,
    classification_table_path: Option<PathBuf>,
}

struct BugCommandPlan {
    index: usize,
    summary_id: usize,
    request: BugVerificationRequest,
    title: String,
    risk_rank: Option<usize>,
}

struct BugCommandResult {
    index: usize,
    validation: BugValidationState,
    logs: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BugIdentifier {
    RiskRank(usize),
    SummaryId(usize),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BugVerificationTool {
    Curl,
    Python,
    Playwright,
}

impl BugVerificationTool {
    fn as_str(self) -> &'static str {
        match self {
            BugVerificationTool::Curl => "curl",
            BugVerificationTool::Python => "python",
            BugVerificationTool::Playwright => "playwright",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BugVerificationRequest {
    pub id: BugIdentifier,
    pub tool: BugVerificationTool,
    pub target: Option<String>,
    pub script_path: Option<PathBuf>,
    pub script_inline: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct BugVerificationBatchRequest {
    pub snapshot_path: PathBuf,
    pub bugs_path: PathBuf,
    pub report_path: Option<PathBuf>,
    pub report_html_path: Option<PathBuf>,
    pub repo_path: PathBuf,
    pub work_dir: PathBuf,
    pub requests: Vec<BugVerificationRequest>,
}

#[derive(Clone, Debug)]
pub(crate) struct BugVerificationOutcome {
    pub bugs: Vec<SecurityReviewBug>,
    pub logs: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct BugVerificationFailure {
    pub message: String,
    pub logs: Vec<String>,
}

fn build_bug_records(
    summaries: Vec<BugSummary>,
    details: Vec<BugDetail>,
) -> (Vec<SecurityReviewBug>, Vec<BugSnapshot>) {
    let mut detail_lookup: HashMap<usize, String> = HashMap::new();
    for detail in details {
        detail_lookup.insert(detail.summary_id, detail.original_markdown);
    }

    let mut bugs: Vec<SecurityReviewBug> = Vec::new();
    let mut snapshots: Vec<BugSnapshot> = Vec::new();

    for summary in summaries {
        let BugSummary {
            id,
            title,
            file,
            severity,
            impact,
            likelihood,
            recommendation,
            blame,
            risk_score,
            risk_rank,
            risk_reason,
            verification_types,
            vulnerability_tag,
            validation,
            source_path: _,
            markdown,
            author_github,
        } = summary;

        let bug = SecurityReviewBug {
            summary_id: id,
            risk_rank,
            risk_score,
            title,
            severity,
            impact,
            likelihood,
            recommendation,
            file,
            blame,
            risk_reason,
            verification_types,
            vulnerability_tag,
            validation,
            assignee_github: author_github,
        };
        let original_markdown = detail_lookup.remove(&bug.summary_id).unwrap_or(markdown);
        snapshots.push(BugSnapshot {
            bug: bug.clone(),
            original_markdown,
        });
        bugs.push(bug);
    }

    (bugs, snapshots)
}

fn render_bug_sections(snapshots: &[BugSnapshot], git_link_info: Option<&GitLinkInfo>) -> String {
    let mut sections: Vec<String> = Vec::new();
    for snapshot in snapshots {
        let base = snapshot.original_markdown.trim();
        if base.is_empty() {
            continue;
        }
        let mut composed = String::new();
        let anchor_snippet = format!("<a id=\"bug-{}\"", snapshot.bug.summary_id);
        if base.contains(&anchor_snippet) {
            composed.push_str(&linkify_file_lines(base, git_link_info));
        } else {
            composed.push_str(&format!("<a id=\"bug-{}\"></a>\n", snapshot.bug.summary_id));
            composed.push_str(&linkify_file_lines(base, git_link_info));
        }
        if let Some(handle) = snapshot.bug.assignee_github.as_deref() {
            let mut replaced = false;
            let mut adjusted: Vec<String> = Vec::new();
            for line in composed.lines() {
                let trimmed = line.trim_start();
                let lower = trimmed.to_ascii_lowercase();
                if lower.starts_with("assignee:")
                    || lower.starts_with("author:")
                    || lower.starts_with("owner:")
                    || lower.starts_with("suggested owner:")
                {
                    let indent_len = line.len().saturating_sub(trimmed.len());
                    let indent = &line[..indent_len];
                    adjusted.push(format!("{indent}Suggested owner: {handle}"));
                    replaced = true;
                } else {
                    adjusted.push(line.to_string());
                }
            }
            composed = adjusted.join("\n");
            if !replaced {
                if !composed.trim_end().is_empty() {
                    composed.push_str("\n\n");
                }
                composed.push_str(&format!("Suggested owner: {handle}\n"));
            }
        }
        if !matches!(snapshot.bug.validation.status, BugValidationStatus::Pending) {
            composed.push_str("\n\n#### Validation\n");
            let status_label = validation_status_label(&snapshot.bug.validation);
            composed.push_str(&format!("- **Status:** {status_label}\n"));
            if let Some(target) = snapshot
                .bug
                .validation
                .target
                .as_ref()
                .filter(|target| !target.is_empty())
            {
                composed.push_str(&format!("- **Target:** `{target}`\n"));
            }
            if let Some(run_at) = snapshot.bug.validation.run_at
                && let Ok(formatted) = run_at.format(&Rfc3339)
            {
                composed.push_str(&format!("- **Checked:** {formatted}\n"));
            }
            if let Some(summary) = snapshot
                .bug
                .validation
                .summary
                .as_ref()
                .filter(|summary| !summary.is_empty())
            {
                composed.push_str(&format!("- **Summary:** {}\n", summary.trim()));
            }
            if let Some(snippet) = snapshot
                .bug
                .validation
                .output_snippet
                .as_ref()
                .filter(|snippet| !snippet.is_empty())
            {
                composed.push_str("- **Output:**\n```\n");
                composed.push_str(snippet.trim());
                composed.push_str("\n```\n");
            }
        }
        sections.push(composed);
    }
    sections.join("\n\n")
}

fn linkify_file_lines(markdown: &str, git_link_info: Option<&GitLinkInfo>) -> String {
    let Some(info) = git_link_info else {
        return markdown.to_string();
    };
    let mut out_lines: Vec<String> = Vec::new();
    for raw in markdown.lines() {
        let trimmed = raw.trim_start();
        if let Some(rest) = trimmed.strip_prefix("- **File & Lines:**") {
            let value = rest.trim().trim_matches('`');
            if value.is_empty() {
                out_lines.push(raw.to_string());
                continue;
            }
            let pairs = parse_location_item(value, info);
            if pairs.is_empty() {
                out_lines.push(raw.to_string());
                continue;
            }
            // If ranges exist for a path, drop the bare link for that path
            let filtered = filter_location_pairs(pairs);
            let mut links: Vec<String> = Vec::new();
            for (rel, frag) in filtered {
                let mut url = format!("{}{}", info.github_prefix, rel);
                let mut text = rel;
                if let Some(f) = frag.as_ref() {
                    url.push('#');
                    url.push_str(f);
                    text.push('#');
                    text.push_str(f);
                }
                links.push(format!("[{text}]({url})"));
            }
            let rebuilt = format!("- **File & Lines:** {}", links.join(", "));
            // Preserve original indentation
            let indent_len = raw.len().saturating_sub(trimmed.len());
            let indent = &raw[..indent_len];
            out_lines.push(format!("{indent}{rebuilt}"));
        } else {
            out_lines.push(raw.to_string());
        }
    }
    out_lines.join("\n")
}

#[allow(clippy::needless_collect)]
async fn polish_bug_markdowns(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    summaries: &mut [BugSummary],
    details: &mut [BugDetail],
    metrics: Arc<ReviewMetrics>,
) -> Result<Vec<String>, String> {
    if summaries.is_empty() {
        return Ok(Vec::new());
    }

    let mut detail_index: HashMap<usize, usize> = HashMap::new();
    for (idx, detail) in details.iter().enumerate() {
        detail_index.insert(detail.summary_id, idx);
    }

    struct BugPolishUpdate {
        id: usize,
        markdown: String,
        logs: Vec<String>,
    }

    let mut updates: HashMap<usize, BugPolishUpdate> = HashMap::new();
    let mut combined_logs: Vec<String> = Vec::new();

    let work_items: Vec<(usize, String)> = summaries
        .iter()
        .map(|summary| (summary.id, summary.markdown.clone()))
        .collect();

    let mut stream = futures::stream::iter(work_items.into_iter().map(|(bug_id, content)| {
        let metrics = metrics.clone();
        async move {
            if content.trim().is_empty() {
                return Ok(BugPolishUpdate {
                    id: bug_id,
                    markdown: content,
                    logs: Vec::new(),
                });
            }
            let outcome = polish_markdown_block(client, provider, auth, metrics, &content, None)
                .await
                .map_err(|err| format!("Bug {bug_id}: {err}"))?;
            let polished = fix_mermaid_blocks(&outcome.text);
            let logs = outcome
                .reasoning_logs
                .into_iter()
                .map(|line| format!("Bug {bug_id}: {line}"))
                .collect();
            Ok(BugPolishUpdate {
                id: bug_id,
                markdown: polished,
                logs,
            })
        }
    }))
    .buffer_unordered(BUG_POLISH_CONCURRENCY);

    while let Some(result) = stream.next().await {
        match result {
            Ok(update) => {
                combined_logs.extend(update.logs.iter().cloned());
                updates.insert(update.id, update);
            }
            Err(err) => return Err(err),
        }
    }

    drop(stream);

    for summary in summaries.iter_mut() {
        if let Some(update) = updates.get(&summary.id) {
            summary.markdown = update.markdown.clone();
            if let Some(&idx) = detail_index.get(&summary.id) {
                details[idx].original_markdown = update.markdown.clone();
            }
        }
    }

    Ok(combined_logs)
}

fn snapshot_bugs(snapshot: &SecurityReviewSnapshot) -> Vec<SecurityReviewBug> {
    snapshot
        .bugs
        .iter()
        .map(|entry| entry.bug.clone())
        .collect()
}

pub(crate) fn resume_completed_review_from_checkpoint(
    checkpoint: SecurityReviewCheckpoint,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
) -> Result<SecurityReviewResult, SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();
    let record = |logs: &mut Vec<String>, line: String| {
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(line.clone()));
        }
        write_log_sink(&log_sink, line.as_str());
        logs.push(line);
    };

    let snapshot_path =
        checkpoint
            .bug_snapshot_path
            .clone()
            .ok_or_else(|| SecurityReviewFailure {
                message: "Cannot resume completed review: missing bug snapshot path.".to_string(),
                logs: Vec::new(),
            })?;
    let bugs_path = checkpoint
        .bugs_path
        .clone()
        .ok_or_else(|| SecurityReviewFailure {
            message: "Cannot resume completed review: missing bugs path.".to_string(),
            logs: Vec::new(),
        })?;
    let output_root = snapshot_path
        .parent()
        .and_then(|ctx| ctx.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| checkpoint.repo_root.clone());
    let metadata_path = output_root.join("metadata.json");

    record(
        &mut logs,
        format!(
            "Resuming completed security review from {}.",
            snapshot_path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| snapshot_path.display().to_string())
        ),
    );

    let snapshot_bytes = fs::read(&snapshot_path).map_err(|err| SecurityReviewFailure {
        message: format!(
            "Cannot resume completed review: failed to read snapshot {}: {err}",
            snapshot_path.display()
        ),
        logs: logs.clone(),
    })?;
    let snapshot: SecurityReviewSnapshot =
        serde_json::from_slice(&snapshot_bytes).map_err(|err| SecurityReviewFailure {
            message: format!(
                "Cannot resume completed review: failed to parse snapshot {}: {err}",
                snapshot_path.display()
            ),
            logs: logs.clone(),
        })?;

    let findings_summary = snapshot.findings_summary.clone();
    let bugs = snapshot_bugs(&snapshot);

    let report_path = checkpoint.report_path.clone();
    let report_html_path = checkpoint.report_html_path.clone();
    let api_overview_path = checkpoint.api_overview_path.clone();
    let classification_json_path = checkpoint.classification_json_path.clone();
    let classification_table_path = checkpoint.classification_table_path;

    record(
        &mut logs,
        "All steps completed in prior run; showing saved report.".to_string(),
    );

    Ok(SecurityReviewResult {
        findings_summary,
        bug_summary_table: None,
        bugs,
        bugs_path,
        report_path,
        report_html_path,
        snapshot_path,
        metadata_path,
        api_overview_path,
        classification_json_path,
        classification_table_path,
        logs,
        token_usage: TokenUsage::default(),
        estimated_cost_usd: None,
        rate_limit_wait: Duration::ZERO,
    })
}

#[derive(Clone)]
struct GitLinkInfo {
    repo_root: PathBuf,
    github_prefix: String,
}

struct BugPromptData {
    prompt: String,
    logs: Vec<String>,
}

fn is_spec_dir_likely_low_signal(path: &Path) -> bool {
    let mut components: Vec<String> = Vec::new();
    for comp in path.components() {
        if let std::path::Component::Normal(part) = comp
            && let Some(s) = part.to_str()
        {
            components.push(s.to_ascii_lowercase());
        }
    }
    let skip_markers = [
        "test",
        "tests",
        "testing",
        "spec",
        "specs",
        "example",
        "examples",
        "fixture",
        "fixtures",
        "docs",
        "doc",
        "script",
        "scripts",
        "util",
        "utils",
        "tools",
        "tooling",
        "playground",
        "migration",
        "migrations",
        "seed",
        "seeds",
        "sample",
        "samples",
    ];
    components
        .iter()
        .any(|segment| skip_markers.iter().any(|marker| segment.contains(marker)))
}

fn prune_low_signal_spec_dirs(dirs: &[(PathBuf, String)]) -> (Vec<(PathBuf, String)>, Vec<String>) {
    let mut kept: Vec<(PathBuf, String)> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();
    for (path, label) in dirs {
        if is_spec_dir_likely_low_signal(path) {
            dropped.push(label.clone());
        } else {
            kept.push((path.clone(), label.clone()));
        }
    }
    if kept.is_empty() {
        (dirs.to_vec(), Vec::new())
    } else {
        (kept, dropped)
    }
}

fn filter_spec_targets(
    targets: &[PathBuf],
    repo_root: &Path,
    log: &mut dyn FnMut(String),
) -> Vec<PathBuf> {
    let mut kept: Vec<PathBuf> = Vec::new();
    for target in targets {
        if is_spec_dir_likely_low_signal(target) {
            let display = display_path_for(target, repo_root);
            log(format!(
                "Skipping specification for {display} (looks like tests/utils/scripts)."
            ));
            continue;
        }
        kept.push(target.clone());
    }
    if kept.is_empty() {
        targets.to_vec()
    } else {
        kept
    }
}

fn is_ignored_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    let lower_str = lower.as_str();

    const IMAGE_EXTS: &[&str] = &[".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico"];
    const ARCHIVE_EXTS: &[&str] = &[".zip", ".tar", ".gz", ".tgz", ".bz2", ".7z"];
    const LOCK_EXTS: &[&str] = &[".lock", ".log"];

    if IMAGE_EXTS.iter().any(|ext| lower_str.ends_with(ext)) {
        return true;
    }
    if ARCHIVE_EXTS.iter().any(|ext| lower_str.ends_with(ext)) {
        return true;
    }
    if LOCK_EXTS.iter().any(|ext| lower_str.ends_with(ext)) {
        return true;
    }

    // Match AppSec agent heuristic: skip files whose names contain test/spec.
    if lower_str.contains("test") || lower_str.contains("spec") {
        return true;
    }

    false
}

fn linear_mcp_server() -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: "https://mcp.linear.app/mcp".to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        enabled: true,
        startup_timeout_sec: None,
        tool_timeout_sec: Some(Duration::from_secs(300)),
        enabled_tools: None,
        disabled_tools: None,
    }
}

fn notion_mcp_server() -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: "https://mcp.notion.com/mcp".to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        enabled: true,
        startup_timeout_sec: None,
        tool_timeout_sec: Some(Duration::from_secs(300)),
        enabled_tools: None,
        disabled_tools: None,
    }
}

fn secbot_mcp_server() -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: "http://localhost:8082/mcp".to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        enabled: true,
        startup_timeout_sec: None,
        tool_timeout_sec: Some(Duration::from_secs(300)),
        enabled_tools: None,
        disabled_tools: None,
    }
}

fn google_workspace_mcp_server(bin_path: PathBuf) -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::Stdio {
            command: "node".to_string(),
            args: vec![bin_path.display().to_string()],
            env: None,
            env_vars: Vec::new(),
            cwd: None,
        },
        enabled: true,
        startup_timeout_sec: None,
        tool_timeout_sec: Some(Duration::from_secs(300)),
        enabled_tools: None,
        disabled_tools: None,
    }
}

async fn ensure_google_workspace_plugin(
    codex_home: &Path,
    existing: Option<&McpServerConfig>,
    logs: &mut Vec<String>,
) -> Result<PathBuf, SecurityReviewFailure> {
    let dest = codex_home.join("plugins/google-workspace-mcp/bin/mcp-server.js");
    if dest.exists() {
        logs.push(format!(
            "Found google-workspace-mcp binary at {}.",
            dest.display()
        ));
        return Ok(dest);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(cfg) = existing
        && let McpServerTransportConfig::Stdio { args, .. } = &cfg.transport
        && let Some(first) = args.first()
    {
        let candidate = PathBuf::from(first);
        if candidate.exists() {
            candidates.push(candidate);
        }
    }

    let repo_candidate =
        PathBuf::from("/Users/kh.ai/code/codex/google-workspace-mcp/bin/mcp-server.js");
    if repo_candidate.exists() {
        candidates.push(repo_candidate);
    }

    for candidate in candidates {
        if candidate.exists() {
            if let Some(parent) = dest.parent() {
                tokio_fs::create_dir_all(parent)
                    .await
                    .map_err(|err| SecurityReviewFailure {
                        message: format!(
                            "Failed to prepare google-workspace-mcp plugin directory {}: {err}",
                            parent.display()
                        ),
                        logs: logs.clone(),
                    })?;
            }
            tokio_fs::copy(&candidate, &dest)
                .await
                .map_err(|err| SecurityReviewFailure {
                    message: format!(
                        "Failed to copy google-workspace-mcp binary from {} to {}: {err}",
                        candidate.display(),
                        dest.display()
                    ),
                    logs: logs.clone(),
                })?;
            logs.push(format!(
                "Copied google-workspace-mcp binary from {} to {}.",
                candidate.display(),
                dest.display()
            ));
            return Ok(dest);
        }
    }

    Err(SecurityReviewFailure {
        message: format!(
            "google-workspace-mcp binary not found; expected at {}.",
            dest.display()
        ),
        logs: logs.clone(),
    })
}

pub async fn run_security_review_setup(
    config: &Config,
) -> Result<SecurityReviewSetupResult, SecurityReviewFailure> {
    let codex_home = config.codex_home.clone();
    let mut logs = Vec::new();

    if let Err(mut err) = check_executable_available("node") {
        logs.append(&mut err.logs);
        return Err(SecurityReviewFailure {
            message: err.message,
            logs,
        });
    }

    if let Err(mut err) = check_executable_available("gh") {
        logs.append(&mut err.logs);
        return Err(SecurityReviewFailure {
            message: err.message,
            logs,
        });
    }

    let mut servers =
        load_global_mcp_servers(&codex_home)
            .await
            .map_err(|err| SecurityReviewFailure {
                message: format!("Failed to load MCP servers: {err}"),
                logs: Vec::new(),
            })?;

    let google_bin =
        ensure_google_workspace_plugin(&codex_home, servers.get("google-workspace-mcp"), &mut logs)
            .await?;

    let defaults: [(&str, McpServerConfig); 4] = [
        ("linear", linear_mcp_server()),
        ("notion", notion_mcp_server()),
        (
            "google-workspace-mcp",
            google_workspace_mcp_server(google_bin.clone()),
        ),
        ("secbot", secbot_mcp_server()),
    ];

    for (name, desired) in defaults {
        match servers.get_mut(name) {
            Some(existing) => {
                let desired_transport = desired.transport.clone();
                if !existing.enabled {
                    existing.enabled = true;
                    logs.push(format!("Enabled MCP server `{name}`."));
                }
                if existing.tool_timeout_sec.is_none() && desired.tool_timeout_sec.is_some() {
                    existing.tool_timeout_sec = desired.tool_timeout_sec;
                }
                if name != "google-workspace-mcp" && existing.transport != desired_transport {
                    existing.transport = desired_transport.clone();
                    logs.push(format!(
                        "Updated MCP server `{name}` to use the default transport."
                    ));
                }
                if name == "google-workspace-mcp" {
                    match &mut existing.transport {
                        McpServerTransportConfig::Stdio { args, .. } => {
                            if args.is_empty() {
                                args.push(google_bin.display().to_string());
                            } else if let Some(arg) = args.first_mut() {
                                *arg = google_bin.display().to_string();
                            }
                        }
                        other => {
                            if matches!(desired_transport, McpServerTransportConfig::Stdio { .. }) {
                                *other = desired_transport;
                            }
                        }
                    };
                }
            }
            None => {
                servers.insert(name.to_string(), desired);
                logs.push(format!("Added MCP server `{name}`."));
            }
        }
    }

    ConfigEditsBuilder::new(&codex_home)
        .replace_mcp_servers(&servers)
        .apply()
        .await
        .map_err(|err| SecurityReviewFailure {
            message: format!("Failed to write MCP servers: {err}"),
            logs: logs.clone(),
        })?;

    logs.push(format!(
        "Saved MCP connector entries to {}.",
        codex_home.join("config.toml").display()
    ));

    let auth_statuses =
        compute_auth_statuses(servers.iter(), config.mcp_oauth_credentials_store_mode).await;

    for target in ["linear", "notion", "secbot"] {
        if let Some(entry) = auth_statuses.get(target) {
            match entry.auth_status {
                McpAuthStatus::NotLoggedIn => {
                    if target == "linear" {
                        logs.push(
                            "Linear is not logged in. Running `codex mcp login linear` for first-time setup..."
                                .to_string(),
                        );
                        match Command::new("codex")
                            .args(["mcp", "login", "linear"])
                            .output()
                            .await
                        {
                            Ok(output) => {
                                if output.status.success() {
                                    let stdout = String::from_utf8_lossy(&output.stdout);
                                    let stderr = String::from_utf8_lossy(&output.stderr);
                                    let combined = format!("{stdout}{stderr}");
                                    let trimmed = combined.trim();
                                    if !trimmed.is_empty() {
                                        logs.push(format!(
                                            "`codex mcp login linear` output:\n{trimmed}"
                                        ));
                                    }
                                    logs.push("Linear MCP login completed.".to_string());
                                    continue;
                                }
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                logs.push(format!(
                                    "`codex mcp login linear` failed with status {}.\nstdout:\n{}\nstderr:\n{}",
                                    output.status, stdout.trim(), stderr.trim()
                                ));
                                logs.push(
                                    "Run `codex mcp login linear` manually and re-run /secreview setup."
                                        .to_string(),
                                );
                                continue;
                            }
                            Err(err) => {
                                logs.push(format!("Failed to run `codex mcp login linear`: {err}"));
                                logs.push(
                                    "Run `codex mcp login linear` manually and re-run /secreview setup."
                                        .to_string(),
                                );
                                continue;
                            }
                        }
                    }
                    if let McpServerTransportConfig::StreamableHttp {
                        url,
                        http_headers,
                        env_http_headers,
                        ..
                    } = &entry.config.transport
                    {
                        match supports_oauth_login(url).await {
                            Ok(true) => {
                                logs.push(format!(
                                    "Launching OAuth login for `{target}` in your browser..."
                                ));
                                perform_oauth_login(
                                    target,
                                    url,
                                    config.mcp_oauth_credentials_store_mode,
                                    http_headers.clone(),
                                    env_http_headers.clone(),
                                    &Vec::new(),
                                )
                                .await
                                .map_err(|err| {
                                    SecurityReviewFailure {
                                        message: format!(
                                            "OAuth login failed for `{target}`: {err}"
                                        ),
                                        logs: logs.clone(),
                                    }
                                })?;
                                logs.push(format!("OAuth login completed for `{target}`."));
                            }
                            Ok(false) => logs.push(format!(
                                "`{target}` does not advertise OAuth; skipping login."
                            )),
                            Err(err) => logs.push(format!(
                                "Could not check OAuth support for `{target}`: {err}"
                            )),
                        }
                    } else {
                        logs.push(format!(
                            "`{target}` is not a streamable HTTP server; skipping OAuth login."
                        ));
                    }
                }
                McpAuthStatus::BearerToken | McpAuthStatus::OAuth => logs.push(format!(
                    "`{target}` already authenticated ({}).",
                    entry.auth_status
                )),
                McpAuthStatus::Unsupported => logs.push(format!(
                    "`{target}` does not support OAuth detection; skipping login."
                )),
            }
        } else {
            logs.push(format!(
                "MCP server `{target}` missing after write; skipping OAuth check."
            ));
        }
    }

    Ok(SecurityReviewSetupResult { logs })
}

fn check_executable_available(name: &str) -> Result<(), SecurityReviewFailure> {
    let result = StdCommand::new(name).arg("--version").output();
    match result {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(SecurityReviewFailure {
            message: format!("{name} is required for security review setup but was not found."),
            logs: vec![
                format!("Install `{name}` and ensure it is on PATH, then rerun /secreview setup."),
                format!("Try: {}", install_hint_for(name)),
            ],
        }),
    }
}

fn install_hint_for(name: &str) -> String {
    match name {
        "node" => {
            if cfg!(target_os = "macos") {
                "brew install node".to_string()
            } else if cfg!(target_os = "windows") {
                "winget install OpenJS.NodeJS".to_string()
            } else {
                "sudo apt-get update && sudo apt-get install -y nodejs npm".to_string()
            }
        }
        "gh" => {
            if cfg!(target_os = "macos") {
                "brew install gh".to_string()
            } else if cfg!(target_os = "windows") {
                "winget install GitHub.cli".to_string()
            } else {
                "type -p curl >/dev/null || sudo apt-get install -y curl && \\\n  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg && \\\n  sudo chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg && \\\n  echo \"deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main\" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null && \\\n  sudo apt-get update && sudo apt-get install -y gh".to_string()
            }
        }
        other => format!("Please install `{other}` and ensure it is on PATH."),
    }
}

pub async fn run_security_review(
    request: SecurityReviewRequest,
) -> Result<SecurityReviewResult, SecurityReviewFailure> {
    let mut progress_sender = request.progress_sender.clone();
    let log_sink = request.log_sink.clone();
    let mut logs = Vec::new();
    let metrics = Arc::new(ReviewMetrics::default());
    let model_client = create_client();
    let overall_start = Instant::now();

    if progress_sender.is_none() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let log_sink_for_task = log_sink.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    AppEvent::SecurityReviewLog(message) => {
                        eprintln!("{message}");
                        write_log_sink(&log_sink_for_task, message.as_str());
                    }
                    AppEvent::SecurityReviewCommandStatus {
                        summary,
                        state,
                        preview,
                        ..
                    } => {
                        let state_label = match state {
                            SecurityReviewCommandState::Running => "running",
                            SecurityReviewCommandState::Matches => "matches",
                            SecurityReviewCommandState::NoMatches => "no matches",
                            SecurityReviewCommandState::Error => "error",
                        };
                        eprintln!("Command [{state_label}]: {summary}");
                        write_log_sink(
                            &log_sink_for_task,
                            format!("Command [{state_label}]: {summary}").as_str(),
                        );
                        for line in preview {
                            if !line.trim().is_empty() {
                                eprintln!("{line}");
                                write_log_sink(&log_sink_for_task, line.as_str());
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
        progress_sender = Some(AppEventSender::new(tx));
    }

    let repo_path = request.repo_path.clone();
    let repo_slug = sanitize_repo_slug(&repo_path);
    let git_revision = collect_git_revision(&repo_path).await;
    let mut include_paths = request.include_paths.clone();
    let mut scope_display_paths = request.scope_display_paths.clone();
    let mut auto_scope_prompt = request.auto_scope_prompt.clone();
    let mut linear_issue = request.linear_issue.clone();
    let mut mode = request.mode;

    let mut checkpoint = request
        .resume_checkpoint
        .clone()
        .or_else(|| load_checkpoint(&request.output_root));
    if let Some(cp) = checkpoint.clone() {
        checkpoint = match cp.status {
            SecurityReviewCheckpointStatus::Running | SecurityReviewCheckpointStatus::Complete => {
                Some(cp)
            }
        };
    }
    let resuming = checkpoint.is_some();
    if let Some(checkpoint) = checkpoint.as_ref()
        && checkpoint.status == SecurityReviewCheckpointStatus::Complete
    {
        return resume_completed_review_from_checkpoint(
            checkpoint.clone(),
            request.progress_sender.clone(),
            request.log_sink.clone(),
        );
    }

    let mut checkpoint = checkpoint.unwrap_or_else(|| SecurityReviewCheckpoint {
        status: SecurityReviewCheckpointStatus::Running,
        mode,
        include_paths: include_paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
        scope_display_paths: scope_display_paths.clone(),
        scope_file_path: None,
        auto_scope_prompt: auto_scope_prompt.clone(),
        triage_model: request.triage_model.clone(),
        model: request.model.clone(),
        provider_name: request.provider.name.clone(),
        repo_slug: repo_slug.clone(),
        repo_root: repo_path.clone(),
        started_at: OffsetDateTime::now_utc(),
        plan_statuses: default_plan_statuses(mode),
        selected_snippets: None,
        spec: None,
        threat_model: None,
        bug_snapshot_path: None,
        bugs_path: None,
        report_path: None,
        report_html_path: None,
        api_overview_path: None,
        classification_json_path: None,
        classification_table_path: None,
        last_log: None,
    });
    let mut scope_file_path: Option<PathBuf> =
        checkpoint.scope_file_path.as_ref().map(PathBuf::from);

    if resuming {
        include_paths = checkpoint.include_paths.iter().map(PathBuf::from).collect();
        scope_display_paths = checkpoint.scope_display_paths.clone();
        auto_scope_prompt = checkpoint.auto_scope_prompt.clone();
        mode = checkpoint.mode;
    } else {
        checkpoint.plan_statuses = default_plan_statuses(mode);
    }

    let is_open_source = is_open_source_review(&auto_scope_prompt);

    if linear_issue.is_none() {
        linear_issue = auto_scope_prompt
            .as_ref()
            .and_then(|prompt| extract_linear_issue_ref(prompt.as_str()));
    }

    let previous_model = checkpoint.model.clone();
    let previous_provider = checkpoint.provider_name.clone();
    checkpoint.status = SecurityReviewCheckpointStatus::Running;
    checkpoint.repo_root = repo_path.clone();
    checkpoint.repo_slug = repo_slug.clone();
    checkpoint.mode = mode;
    checkpoint.triage_model = request.triage_model.clone();
    checkpoint.model = request.model.clone();
    checkpoint.provider_name = request.provider.name.clone();
    checkpoint.include_paths = include_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    checkpoint.scope_display_paths = scope_display_paths.clone();
    checkpoint.auto_scope_prompt = auto_scope_prompt.clone();
    if checkpoint.plan_statuses.is_empty() {
        checkpoint.plan_statuses = default_plan_statuses(mode);
    }

    let record = {
        let progress_sender = progress_sender.clone();
        let log_sink = log_sink.clone();
        move |logs: &mut Vec<String>, line: String| {
            if let Some(callback) = request.progress_callback.as_ref() {
                callback(line.clone());
            }
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(line.clone()));
            }
            write_log_sink(&log_sink, line.as_str());
            logs.push(line);
        }
    };

    if resuming {
        if previous_model != request.model {
            record(
                &mut logs,
                format!(
                    "Checkpoint recorded under model {}; continuing with {}.",
                    previous_model, request.model
                ),
            );
        }
        if previous_provider != request.provider.name {
            record(
                &mut logs,
                format!(
                    "Checkpoint recorded under provider {}; continuing with {}.",
                    previous_provider, request.provider.name
                ),
            );
        }
    }

    let persist_checkpoint = |checkpoint: &mut SecurityReviewCheckpoint, logs: &mut Vec<String>| {
        checkpoint.last_log = logs.last().cloned();
        if let Err(err) = write_checkpoint(&request.output_root, checkpoint) {
            let message = format!("Failed to persist checkpoint: {err}");
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(message.clone()));
            }
            write_log_sink(&log_sink, message.as_str());
            logs.push(message);
        }
    };

    if resuming {
        record(
            &mut logs,
            format!(
                "Resuming security review from {} (mode: {}, model: {})",
                request.output_root.display(),
                mode.as_str(),
                request.model
            ),
        );
    } else {
        record(
            &mut logs,
            format!(
                "Starting security review in {} (mode: {}, model: {})",
                repo_path.display(),
                mode.as_str(),
                request.model
            ),
        );
    }

    let git_link_info = build_git_link_info(&repo_path).await;
    persist_checkpoint(&mut checkpoint, &mut logs);

    // Initialize Linear status (classification, context gathering, checklist) when requested.
    if let Some(linear_issue) = linear_issue.as_ref() {
        let init_prompt = build_linear_init_prompt(
            linear_issue,
            &request.model,
            &repo_path,
            &request.output_root,
            mode,
            &include_paths,
            &scope_display_paths,
            scope_file_path.as_deref(),
            &checkpoint.plan_statuses,
        );
        {
            let config = request.config.clone();
            let provider = request.provider.clone();
            let auth_manager = request.auth_manager.clone();
            let repo_for_task = repo_path.clone();
            let progress_for_task = progress_sender.clone();
            let log_sink_for_task = log_sink.clone();
            let metrics_for_task = metrics.clone();
            tokio::spawn(async move {
                let _ = run_linear_status_agent(
                    &config,
                    &provider,
                    auth_manager,
                    &repo_for_task,
                    progress_for_task,
                    log_sink_for_task,
                    init_prompt,
                    metrics_for_task,
                )
                .await;
            });
        }
    }

    let mut auto_scope_conversation: Option<String> = None;
    if include_paths.is_empty()
        && let Some(linear_issue) = linear_issue.as_ref()
    {
        record(
            &mut logs,
            format!("Fetching Linear context for auto scope (issue: {linear_issue})..."),
        );
        match fetch_linear_context_for_auto_scope(
            &request.config,
            &request.provider,
            request.auth_manager.clone(),
            &repo_path,
            linear_issue,
            progress_sender.clone(),
            log_sink.clone(),
            metrics.clone(),
        )
        .await
        {
            Ok((context, context_logs)) => {
                for line in context_logs {
                    record(&mut logs, line);
                }
                let trimmed = truncate_text(&context, ANALYSIS_CONTEXT_MAX_CHARS);
                if trimmed.len() < context.len() {
                    record(
                        &mut logs,
                        format!(
                            "Linear context trimmed to {} characters for auto scope.",
                            trimmed.len()
                        ),
                    );
                }
                auto_scope_conversation = Some(trimmed);
            }
            Err(err) => {
                for line in err.logs {
                    record(&mut logs, line);
                }
                record(
                    &mut logs,
                    format!(
                        "Proceeding without Linear context for auto scope: {message}",
                        message = err.message
                    ),
                );
            }
        }
    }

    if include_paths.is_empty()
        && auto_scope_prompt.is_none()
        && let Some(context) = auto_scope_conversation.clone()
    {
        record(
            &mut logs,
            "No explicit scope provided; using Linear issue context to auto-detect scope."
                .to_string(),
        );
        auto_scope_prompt = Some(format!("Linear issue context:\n{context}"));
        checkpoint.auto_scope_prompt = auto_scope_prompt.clone();
    }

    if include_paths.is_empty()
        && let Some(prompt) = auto_scope_prompt.as_ref().and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    {
        // Always use the fast model for auto-scope regardless of triage_model.
        let auto_scope_model = AUTO_SCOPE_MODEL;

        record(
            &mut logs,
            format!("Auto-detecting review scope from user prompt: {prompt}"),
        );
        match auto_detect_scope(
            &model_client,
            &request.provider,
            &request.auth,
            auto_scope_model,
            &repo_path,
            prompt,
            auto_scope_conversation
                .as_deref()
                .unwrap_or("No prior exchanges."),
            metrics.clone(),
            &request.config,
            request.auth_manager.clone(),
            progress_sender.clone(),
            log_sink.clone(),
        )
        .await
        {
            Ok((selections, scope_logs)) => {
                for line in scope_logs {
                    record(&mut logs, line);
                }
                if selections.is_empty() {
                    record(
                        &mut logs,
                        "Auto scope returned no directories; reviewing entire repository."
                            .to_string(),
                    );
                } else {
                    let mut resolved_paths: Vec<PathBuf> = Vec::with_capacity(selections.len());
                    let mut selection_summaries: Vec<(String, Option<String>)> =
                        Vec::with_capacity(selections.len());
                    for selection in selections {
                        let AutoScopeSelection {
                            abs_path,
                            display_path,
                            reason,
                            is_dir,
                        } = selection;
                        let kind = if is_dir { "directory" } else { "file" };
                        let message = if let Some(reason) = reason.as_ref() {
                            format!("Auto scope included {kind} {display_path} — {reason}")
                        } else {
                            format!("Auto scope included {kind} {display_path}")
                        };
                        record(&mut logs, message);
                        resolved_paths.push(abs_path);
                        selection_summaries.push((display_path, reason));
                    }

                    if let Some(tx) = progress_sender.as_ref() {
                        let display_paths: Vec<String> = selection_summaries
                            .iter()
                            .map(|(path, _)| path.clone())
                            .collect();

                        if request.skip_auto_scope_confirmation {
                            // Option 2 (Quick bug sweep): auto-accept detected scope and continue.
                            include_paths = resolved_paths;
                            scope_display_paths = display_paths.clone();
                            record(&mut logs, "Auto scope selections accepted.".to_string());
                            tx.send(AppEvent::SecurityReviewScopeResolved {
                                paths: display_paths,
                            });
                            checkpoint.include_paths = include_paths
                                .iter()
                                .map(|path| path.to_string_lossy().to_string())
                                .collect();
                            checkpoint.scope_display_paths = scope_display_paths.clone();
                            persist_checkpoint(&mut checkpoint, &mut logs);
                        } else {
                            // Show confirmation dialog when not explicitly skipping.
                            let (confirm_tx, confirm_rx) = oneshot::channel();
                            let selections_for_ui: Vec<SecurityReviewAutoScopeSelection> =
                                selection_summaries
                                    .iter()
                                    .map(|(path, reason)| SecurityReviewAutoScopeSelection {
                                        display_path: path.clone(),
                                        reason: reason.clone(),
                                    })
                                    .collect();
                            tx.send(AppEvent::SecurityReviewAutoScopeConfirm {
                                mode,
                                prompt: prompt.to_string(),
                                selections: selections_for_ui,
                                responder: confirm_tx,
                            });

                            record(
                                &mut logs,
                                "Waiting for user confirmation of auto-detected scope..."
                                    .to_string(),
                            );

                            match confirm_rx.await {
                                Ok(true) => {
                                    record(&mut logs, "Auto scope confirmed by user.".to_string());
                                    include_paths = resolved_paths;
                                    scope_display_paths = display_paths.clone();
                                    tx.send(AppEvent::SecurityReviewScopeResolved {
                                        paths: display_paths,
                                    });
                                    checkpoint.include_paths = include_paths
                                        .iter()
                                        .map(|path| path.to_string_lossy().to_string())
                                        .collect();
                                    checkpoint.scope_display_paths = scope_display_paths.clone();
                                    persist_checkpoint(&mut checkpoint, &mut logs);
                                }
                                Ok(false) => {
                                    record(
                                        &mut logs,
                                        "Auto scope selection rejected by user; cancelling review."
                                            .to_string(),
                                    );
                                    tx.send(AppEvent::OpenSecurityReviewPathPrompt(mode));
                                    return Err(SecurityReviewFailure {
                                        message:
                                            "Security review cancelled after auto scope rejection."
                                                .to_string(),
                                        logs,
                                    });
                                }
                                Err(_) => {
                                    record(
                                        &mut logs,
                                        "Auto scope confirmation interrupted; cancelling review."
                                            .to_string(),
                                    );
                                    return Err(SecurityReviewFailure {
                                        message:
                                            "Auto scope confirmation interrupted; review cancelled."
                                                .to_string(),
                                        logs,
                                    });
                                }
                            }
                        }
                    } else {
                        include_paths = resolved_paths;
                    }
                }
            }
            Err(failure) => {
                record(
                    &mut logs,
                    format!("Auto scope detection failed: {}", failure.message),
                );
                for line in failure.logs {
                    record(&mut logs, line);
                }
            }
        }
    }

    checkpoint.include_paths = include_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    checkpoint.scope_display_paths = scope_display_paths.clone();
    persist_checkpoint(&mut checkpoint, &mut logs);

    let scope_file_exists = scope_file_path
        .as_ref()
        .map(|path| path.exists())
        .unwrap_or(false);
    if !scope_file_exists {
        match write_scope_file(
            &request.output_root,
            &repo_path,
            &scope_display_paths,
            linear_issue.as_deref(),
        ) {
            Ok(path) => {
                scope_file_path = Some(path);
            }
            Err(err) => {
                record(&mut logs, format!("Failed to write scope file: {err}"));
            }
        }
    }
    if let Some(path) = scope_file_path.as_ref() {
        checkpoint.scope_file_path = Some(path.display().to_string());
        persist_checkpoint(&mut checkpoint, &mut logs);
    }

    // Pull related docs via a helper sub-agent and update Linear to unblock analysis.
    if let Some(linear_issue) = linear_issue.as_ref() {
        let prompt = build_linear_related_docs_prompt(
            linear_issue,
            &checkpoint.model,
            &checkpoint,
            &request.output_root,
        );
        let config = request.config.clone();
        let provider = request.provider.clone();
        let auth_manager = request.auth_manager.clone();
        let repo_for_task = repo_path.clone();
        let progress_for_task = progress_sender.clone();
        let log_sink_for_task = log_sink.clone();
        let metrics_for_task = metrics.clone();
        tokio::spawn(async move {
            let _ = run_linear_status_agent(
                &config,
                &provider,
                auth_manager,
                &repo_for_task,
                progress_for_task,
                log_sink_for_task,
                prompt,
                metrics_for_task,
            )
            .await;
        });
    }

    let mut plan_tracker = SecurityReviewPlanTracker::new(
        mode,
        &include_paths,
        &repo_path,
        progress_sender.clone(),
        log_sink.clone(),
    );
    plan_tracker.restore_statuses(&checkpoint.plan_statuses);
    checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
    persist_checkpoint(&mut checkpoint, &mut logs);

    let mut selected_snippets = checkpoint.selected_snippets.clone();
    if selected_snippets.is_none() {
        record(&mut logs, "Collecting candidate files...".to_string());

        let progress_sender_for_collection = progress_sender.clone();
        let collection_paths = include_paths.clone();
        let repo_path_for_collection = repo_path.clone();
        let collection = match spawn_blocking(move || {
            collect_snippets_blocking(
                repo_path_for_collection,
                collection_paths,
                DEFAULT_MAX_FILES,
                DEFAULT_MAX_BYTES_PER_FILE,
                DEFAULT_MAX_TOTAL_BYTES,
                progress_sender_for_collection,
            )
        })
        .await
        {
            Ok(Ok(collection)) => collection,
            Ok(Err(failure)) => {
                let mut combined_logs = logs.clone();
                if let Some(tx) = progress_sender.as_ref() {
                    for line in &failure.logs {
                        tx.send(AppEvent::SecurityReviewLog(line.clone()));
                    }
                }
                combined_logs.extend(failure.logs);
                return Err(SecurityReviewFailure {
                    message: failure.message,
                    logs: combined_logs,
                });
            }
            Err(e) => {
                record(&mut logs, format!("File collection task failed: {e}"));
                return Err(SecurityReviewFailure {
                    message: format!("File collection task failed: {e}"),
                    logs,
                });
            }
        };

        for line in collection.logs {
            record(&mut logs, line);
        }

        if collection.snippets.is_empty() {
            record(
                &mut logs,
                "No candidate files found for review.".to_string(),
            );
            return Err(SecurityReviewFailure {
                message: "No candidate files found for review.".to_string(),
                logs,
            });
        }

        let triage_model = if request.triage_model.trim().is_empty() {
            FILE_TRIAGE_MODEL
        } else {
            request.triage_model.as_str()
        };

        // First prune at the directory level to keep triage manageable.
        let mut directories: HashMap<PathBuf, Vec<FileSnippet>> = HashMap::new();
        for snippet in collection.snippets {
            let parent = snippet
                .relative_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            directories.entry(parent).or_default().push(snippet);
        }
        let mut ranked_dirs: Vec<(PathBuf, Vec<FileSnippet>, usize)> = directories
            .into_iter()
            .map(|(dir, snippets)| {
                let bytes = snippets.iter().map(|s| s.bytes).sum::<usize>();
                (dir, snippets, bytes)
            })
            .collect();
        ranked_dirs.sort_by(|a, b| b.2.cmp(&a.2));
        let mut pruned_snippets: Vec<FileSnippet> = Vec::new();
        for (dir, snippets, bytes) in ranked_dirs {
            record(
                &mut logs,
                format!(
                    "Inspecting directory {} ({} files, {}).",
                    display_path_for(&dir, &repo_path),
                    snippets.len(),
                    human_readable_bytes(bytes),
                ),
            );
            pruned_snippets.extend(snippets);
        }
        record(
            &mut logs,
            format!(
                "Running LLM file triage to prioritize analysis across {} files ({} directories).",
                pruned_snippets.len(),
                pruned_snippets
                    .iter()
                    .filter_map(|s| s.relative_path.parent())
                    .collect::<HashSet<_>>()
                    .len()
            ),
        );
        let triage = match triage_files_for_bug_analysis(
            &model_client,
            &request.provider,
            &request.auth,
            triage_model,
            auto_scope_prompt.clone(),
            pruned_snippets,
            progress_sender.clone(),
            log_sink.clone(),
            metrics.clone(),
        )
        .await
        {
            Ok(result) => result,
            Err(err) => {
                record(&mut logs, err.message.clone());
                let mut combined_logs = logs.clone();
                combined_logs.extend(err.logs);
                return Err(SecurityReviewFailure {
                    message: err.message,
                    logs: combined_logs,
                });
            }
        };

        for line in &triage.logs {
            record(&mut logs, line.clone());
        }

        selected_snippets = Some(triage.included);
        checkpoint.selected_snippets = selected_snippets.clone();
        persist_checkpoint(&mut checkpoint, &mut logs);

        // Merge triaged file paths into the displayed scope paths for downstream prompts.
        if let Some(snippets) = selected_snippets.as_ref() {
            let mut combined: Vec<String> = scope_display_paths.clone();
            for snippet in snippets {
                let abs = repo_path.join(&snippet.relative_path);
                combined.push(display_path_for(&abs, &repo_path));
            }
            combined.sort();
            combined.dedup();
            if combined != scope_display_paths {
                scope_display_paths = combined;
                checkpoint.scope_display_paths = scope_display_paths.clone();
                persist_checkpoint(&mut checkpoint, &mut logs);
                if let Err(err) = write_scope_file(
                    &request.output_root,
                    &repo_path,
                    &scope_display_paths,
                    linear_issue.as_deref(),
                ) {
                    record(&mut logs, format!("Failed to update scope file: {err}"));
                }
            }
        }
    } else {
        let count = selected_snippets
            .as_ref()
            .map(std::vec::Vec::len)
            .unwrap_or(0);
        record(
            &mut logs,
            format!("Using {count} triaged file(s) from checkpoint resume."),
        );
    }

    let Some(selected_snippets) = selected_snippets else {
        return Err(SecurityReviewFailure {
            message: "No files selected for bug analysis after checkpoint resume.".to_string(),
            logs,
        });
    };

    if matches!(mode, SecurityReviewMode::Full)
        && !matches!(
            plan_tracker.status_for(SecurityReviewPlanStep::GenerateSpecs),
            Some(StepStatus::Completed | StepStatus::InProgress)
        )
    {
        plan_tracker.start_step(SecurityReviewPlanStep::GenerateSpecs);
        if let Some(linear_issue) = linear_issue.as_ref() {
            let prompt = build_linear_progress_prompt(
                linear_issue,
                &checkpoint.model,
                "Generate system specifications",
                &plan_tracker.snapshot_statuses(),
                &checkpoint,
                &request.output_root,
            );
            let config = request.config.clone();
            let provider = request.provider.clone();
            let auth_manager = request.auth_manager.clone();
            let repo_for_task = repo_path.clone();
            let progress_for_task = progress_sender.clone();
            let log_sink_for_task = log_sink.clone();
            let metrics_for_task = metrics.clone();
            tokio::spawn(async move {
                let _ = run_linear_status_agent(
                    &config,
                    &provider,
                    auth_manager,
                    &repo_for_task,
                    progress_for_task,
                    log_sink_for_task,
                    prompt,
                    metrics_for_task,
                )
                .await;
            });
        }
    }
    if !matches!(
        plan_tracker.status_for(SecurityReviewPlanStep::AnalyzeBugs),
        Some(StepStatus::Completed | StepStatus::InProgress)
    ) {
        plan_tracker.start_step(SecurityReviewPlanStep::AnalyzeBugs);
        if let Some(linear_issue) = linear_issue.as_ref() {
            let prompt = build_linear_progress_prompt(
                linear_issue,
                &checkpoint.model,
                "Analyze code for bugs",
                &plan_tracker.snapshot_statuses(),
                &checkpoint,
                &request.output_root,
            );
            let config = request.config.clone();
            let provider = request.provider.clone();
            let auth_manager = request.auth_manager.clone();
            let repo_for_task = repo_path.clone();
            let progress_for_task = progress_sender.clone();
            let log_sink_for_task = log_sink.clone();
            let metrics_for_task = metrics.clone();
            tokio::spawn(async move {
                let _ = run_linear_status_agent(
                    &config,
                    &provider,
                    auth_manager,
                    &repo_for_task,
                    progress_for_task,
                    log_sink_for_task,
                    prompt,
                    metrics_for_task,
                )
                .await;
            });
        }
    }
    checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
    persist_checkpoint(&mut checkpoint, &mut logs);
    let total_bytes = selected_snippets.iter().map(|s| s.bytes).sum::<usize>();
    let total_size = human_readable_bytes(total_bytes);
    record(
        &mut logs,
        format!(
            "Preparing bug analysis for {} files ({} total).",
            selected_snippets.len(),
            total_size
        ),
    );

    let repository_summary = build_repository_summary(&selected_snippets);
    let bug_scope_prompt = auto_scope_prompt
        .as_ref()
        .map(|prompt| prompt.trim().to_string())
        .filter(|prompt| !prompt.is_empty());
    let mut spec_targets: Vec<PathBuf> = if !include_paths.is_empty() {
        include_paths.clone()
    } else {
        let mut unique_dirs: HashSet<PathBuf> = HashSet::new();
        for snippet in &selected_snippets {
            let absolute = repo_path.join(&snippet.relative_path);
            let dir = absolute.parent().unwrap_or(&repo_path);
            unique_dirs.insert(dir.to_path_buf());
        }
        if unique_dirs.is_empty() {
            vec![repo_path.clone()]
        } else {
            unique_dirs.into_iter().collect()
        }
    };

    spec_targets.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    if matches!(mode, SecurityReviewMode::Full) {
        let mut directory_candidates: Vec<(PathBuf, String)> = spec_targets
            .iter()
            .map(|path| {
                let label = display_path_for(path, &repo_path);
                (path.clone(), label)
            })
            .collect();
        directory_candidates.sort_by(|a, b| a.1.cmp(&b.1));

        match filter_spec_directories(
            &model_client,
            &request.provider,
            &request.auth,
            &repo_path,
            &directory_candidates,
            metrics.clone(),
        )
        .await
        {
            Ok(filtered) => {
                let (preferred_dirs, dropped) = prune_low_signal_spec_dirs(&filtered);
                for label in &dropped {
                    record(
                        &mut logs,
                        format!(
                            "Skipping specification for {label} (low-signal helper/migration dir)."
                        ),
                    );
                }
                let selected_dirs = if preferred_dirs.is_empty() {
                    filtered
                } else {
                    preferred_dirs
                };
                let filtered_paths: Vec<PathBuf> =
                    selected_dirs.iter().map(|(path, _)| path.clone()).collect();
                if filtered_paths.len() < spec_targets.len() {
                    record(
                        &mut logs,
                        format!(
                            "Spec directory triage kept {}/{} directories for specification.",
                            filtered_paths.len(),
                            spec_targets.len()
                        ),
                    );
                }
                spec_targets = filtered_paths;
            }
            Err(err) => {
                for line in &err.logs {
                    record(&mut logs, line.clone());
                }
                record(
                    &mut logs,
                    format!(
                        "Spec directory triage failed; using all directories. {}",
                        err.message
                    ),
                );
            }
        }
    }

    let mut spec_generation: Option<SpecGenerationOutcome> = checkpoint
        .spec
        .as_ref()
        .map(|stored| stored.clone().into_outcome());
    let mut threat_model: Option<ThreatModelOutcome> = checkpoint
        .threat_model
        .as_ref()
        .map(|stored| stored.clone().into_outcome());
    let spec_threat_task = if matches!(mode, SecurityReviewMode::Full) && spec_generation.is_none()
    {
        record(
            &mut logs,
            format!(
                "Generating system specifications for {} scope path(s) (running in parallel with bug analysis).",
                spec_targets.len()
            ),
        );
        let model_client = model_client.clone();
        let provider = request.provider.clone();
        let auth = request.auth.clone();
        let repo_path = request.repo_path.clone();
        let spec_targets = spec_targets.clone();
        let output_root = request.output_root.clone();
        let progress_sender = progress_sender.clone();
        let metrics = metrics.clone();
        let log_sink = log_sink.clone();
        let config = request.config.clone();
        let auth_manager = request.auth_manager.clone();
        let repository_summary = repository_summary.clone();
        Some(tokio::spawn(async move {
            let spec_generation = match generate_specs(
                &model_client,
                &provider,
                &auth,
                &repo_path,
                &spec_targets,
                &output_root,
                progress_sender.clone(),
                metrics.clone(),
                log_sink.clone(),
                &config,
                auth_manager.clone(),
            )
            .await
            {
                Ok(spec) => spec,
                Err(err) => return Err(err),
            };

            let threat_model = if let Some(spec) = spec_generation.as_ref() {
                match generate_threat_model(
                    &model_client,
                    &provider,
                    &auth,
                    THREAT_MODEL_MODEL,
                    &repository_summary,
                    &repo_path,
                    spec,
                    &output_root,
                    progress_sender,
                    metrics,
                )
                .await
                {
                    Ok(threat) => threat,
                    Err(err) => return Err(err),
                }
            } else {
                None
            };

            Ok(SpecThreatOutcome {
                spec: spec_generation,
                threat: threat_model,
            })
        }))
    } else {
        None
    };
    if spec_threat_task.is_some() && request.include_spec_in_bug_analysis {
        record(
            &mut logs,
            "Running bug analysis while specifications and threat model are generated; will apply that context during risk rerank."
                .to_string(),
        );
    } else if spec_threat_task.is_some() && !request.include_spec_in_bug_analysis {
        record(
            &mut logs,
            "Bug analysis running in parallel with specification generation (context disabled by config)."
                .to_string(),
        );
    } else if matches!(mode, SecurityReviewMode::Full) {
        if spec_generation.is_some() {
            record(
                &mut logs,
                "Specification already available from checkpoint; skipping regeneration."
                    .to_string(),
            );
        }
        if threat_model.is_some() {
            record(
                &mut logs,
                "Threat model already available from checkpoint; skipping regeneration."
                    .to_string(),
            );
        }
    }
    let spec_for_bug_analysis: Option<&str> = None;

    let mut bug_summary_table: Option<String> = None;
    let mut bugs_for_result: Vec<SecurityReviewBug> = Vec::new();
    let mut report_sections_prefix: Vec<String> = Vec::new();
    let mut snapshot: Option<SecurityReviewSnapshot> = None;
    let mut bugs_markdown: String = String::new();
    let mut findings_summary: String = String::new();

    let mut skip_bug_analysis = false;
    if resuming
        && let Some(path) = checkpoint.bug_snapshot_path.as_ref()
        && path.exists()
    {
        match tokio_fs::read(path).await {
            Ok(bytes) => match serde_json::from_slice::<SecurityReviewSnapshot>(&bytes) {
                Ok(loaded) => {
                    record(
                        &mut logs,
                        format!(
                            "Loaded prior bug snapshot from {}; skipping bug re-analysis.",
                            path.display()
                        ),
                    );

                    plan_tracker.mark_complete(SecurityReviewPlanStep::AnalyzeBugs);
                    plan_tracker.mark_complete(SecurityReviewPlanStep::PolishFindings);
                    if !matches!(
                        plan_tracker.status_for(SecurityReviewPlanStep::AssembleReport),
                        Some(StepStatus::Completed | StepStatus::InProgress)
                    ) {
                        plan_tracker.start_step(SecurityReviewPlanStep::AssembleReport);
                    }
                    checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
                    persist_checkpoint(&mut checkpoint, &mut logs);

                    findings_summary = loaded.findings_summary.clone();
                    bugs_for_result = snapshot_bugs(&loaded);
                    bug_summary_table = make_bug_summary_table_from_bugs(&bugs_for_result);
                    report_sections_prefix = loaded.report_sections_prefix.clone();
                    bugs_markdown = build_bugs_markdown(&loaded, git_link_info.as_ref());
                    snapshot = Some(loaded);
                    skip_bug_analysis = true;
                }
                Err(err) => {
                    record(
                        &mut logs,
                        format!(
                            "Failed to parse bug snapshot {}; rerunning bug analysis. {err}",
                            path.display()
                        ),
                    );
                }
            },
            Err(err) => {
                record(
                    &mut logs,
                    format!(
                        "Failed to read bug snapshot {}; rerunning bug analysis. {err}",
                        path.display()
                    ),
                );
            }
        }
    }

    if !skip_bug_analysis {
        // Run bug analysis in N full passes across all selected files.
        let total_passes = BUG_FINDING_PASSES.max(1);
        record(
            &mut logs,
            format!("Running bug analysis in {total_passes} pass(es)."),
        );

        let mut aggregated_logs: Vec<String> = Vec::new();
        let mut all_summaries: Vec<BugSummary> = Vec::new();
        let mut all_details: Vec<BugDetail> = Vec::new();
        use std::collections::HashMap as StdHashMap;
        let mut files_map: StdHashMap<PathBuf, FileSnippet> = StdHashMap::new();

        for pass in 1..=total_passes {
            record(
                &mut logs,
                format!(
                    "Starting bug analysis pass {}/{} over {} files.",
                    pass,
                    total_passes,
                    selected_snippets.len()
                ),
            );

            let pass_outcome = match analyze_files_individually(
                &model_client,
                &request.provider,
                &request.auth,
                &request.model,
                &request.config,
                request.auth_manager.clone(),
                &repository_summary,
                spec_for_bug_analysis,
                bug_scope_prompt.as_deref(),
                &request.output_root,
                pass,
                &request.repo_path,
                &selected_snippets,
                git_link_info.clone(),
                progress_sender.clone(),
                log_sink.clone(),
                metrics.clone(),
            )
            .await
            {
                Ok(outcome) => outcome,
                Err(err) => {
                    record(&mut logs, err.message.clone());
                    let mut combined_logs = logs.clone();
                    combined_logs.extend(err.logs);
                    return Err(SecurityReviewFailure {
                        message: err.message,
                        logs: combined_logs,
                    });
                }
            };

            for line in &pass_outcome.logs {
                record(&mut logs, line.clone());
            }
            aggregated_logs.extend(pass_outcome.logs.clone());

            // Offset IDs from this pass to keep them unique when aggregating.
            let id_offset = all_summaries.iter().map(|s| s.id).max().unwrap_or(0);
            let mut pass_summaries = pass_outcome.bug_summaries;
            let mut pass_details = pass_outcome.bug_details;
            for s in pass_summaries.iter_mut() {
                s.id = s.id.saturating_add(id_offset);
            }
            for d in pass_details.iter_mut() {
                d.summary_id = d.summary_id.saturating_add(id_offset);
            }
            all_summaries.extend(pass_summaries);
            all_details.extend(pass_details);

            for snippet in pass_outcome.files_with_findings {
                files_map
                    .entry(snippet.relative_path.clone())
                    .or_insert(snippet);
            }

            record(
                &mut logs,
                format!("Completed bug analysis pass {pass}/{total_passes}."),
            );
        }

        plan_tracker.complete_and_start_next(
            SecurityReviewPlanStep::AnalyzeBugs,
            Some(SecurityReviewPlanStep::PolishFindings),
        );
        checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
        persist_checkpoint(&mut checkpoint, &mut logs);
        let mut analysis_context: Option<String> = None;
        let mut spec_for_rerank: Option<&str> = None;
        if let Some(task) = spec_threat_task {
            match task.await {
                Ok(Ok(outcome)) => {
                    spec_generation = outcome.spec;
                    threat_model = outcome.threat;
                    if let Some(spec) = spec_generation.as_ref() {
                        for line in &spec.logs {
                            record(&mut logs, line.clone());
                        }
                    } else {
                        record(
                            &mut logs,
                            "Specification step skipped (no targets).".to_string(),
                        );
                    }
                    plan_tracker.complete_and_start_next(
                        SecurityReviewPlanStep::GenerateSpecs,
                        Some(SecurityReviewPlanStep::ThreatModel),
                    );
                    checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
                    persist_checkpoint(&mut checkpoint, &mut logs);
                    if let Some(threat) = threat_model.as_ref() {
                        for line in &threat.logs {
                            record(&mut logs, line.clone());
                        }
                    }
                    plan_tracker.mark_complete(SecurityReviewPlanStep::ThreatModel);
                    checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
                    persist_checkpoint(&mut checkpoint, &mut logs);
                }
                Ok(Err(err)) => {
                    record(&mut logs, err.message.clone());
                    let mut combined_logs = logs.clone();
                    combined_logs.extend(err.logs);
                    return Err(SecurityReviewFailure {
                        message: err.message,
                        logs: combined_logs,
                    });
                }
                Err(join_err) => {
                    let message = format!("Specification/threat tasks failed: {join_err}");
                    record(&mut logs, message.clone());
                    let mut combined_logs = logs.clone();
                    combined_logs.push(message.clone());
                    return Err(SecurityReviewFailure {
                        message,
                        logs: combined_logs,
                    });
                }
            }
        } else if matches!(mode, SecurityReviewMode::Full) {
            if spec_generation.is_some() {
                plan_tracker.mark_complete(SecurityReviewPlanStep::GenerateSpecs);
            }
            if threat_model.is_some() {
                plan_tracker.mark_complete(SecurityReviewPlanStep::ThreatModel);
            }
            checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
            persist_checkpoint(&mut checkpoint, &mut logs);
        }

        checkpoint.spec = spec_generation.as_ref().map(StoredSpecOutcome::from);
        checkpoint.threat_model = threat_model.as_ref().map(StoredThreatModelOutcome::from);
        persist_checkpoint(&mut checkpoint, &mut logs);

        if request.include_spec_in_bug_analysis {
            let spec_text = spec_generation
                .as_ref()
                .map(|spec| spec.combined_markdown.as_str());
            let threat_text = threat_model.as_ref().map(|threat| threat.markdown.as_str());
            if spec_text.is_some() || threat_text.is_some() {
                match compact_analysis_context(
                    &model_client,
                    &request.provider,
                    &request.auth,
                    spec_text,
                    threat_text,
                    metrics.clone(),
                    progress_sender.clone(),
                    log_sink.clone(),
                    request.model.as_str(),
                )
                .await
                {
                    Ok(Some(compacted)) => {
                        let message = format!(
                            "Using compacted specification/threat model context ({} chars) for risk rerank.",
                            compacted.len()
                        );
                        record(&mut logs, message);
                        analysis_context = Some(compacted);
                    }
                    Ok(None) => {
                        record(
                            &mut logs,
                            "Specification context unavailable; risk rerank will omit it."
                                .to_string(),
                        );
                    }
                    Err(err) => {
                        record(&mut logs, err.message);
                    }
                }
            }
            spec_for_rerank = analysis_context.as_deref().or(spec_text);
        } else if spec_generation.is_some() {
            record(
                &mut logs,
                "Skipping specification context in risk rerank (disabled by config).".to_string(),
            );
        }
        // Post-process aggregated findings: normalize, filter, dedupe, then risk rerank.
        for summary in all_summaries.iter_mut() {
            if let Some(normalized) = normalize_severity_label(&summary.severity) {
                summary.severity = normalized;
            } else {
                summary.severity = summary.severity.trim().to_string();
            }
        }

        if !all_summaries.is_empty() {
            let mut replacements: HashMap<usize, String> = HashMap::new();
            for summary in all_summaries.iter_mut() {
                if let Some(updated) = rewrite_bug_markdown_severity(
                    summary.markdown.as_str(),
                    summary.severity.as_str(),
                ) {
                    summary.markdown = updated.clone();
                    replacements.insert(summary.id, updated);
                }
                if let Some(updated) =
                    rewrite_bug_markdown_heading_id(summary.markdown.as_str(), summary.id)
                {
                    summary.markdown = updated.clone();
                    replacements.insert(summary.id, updated);
                }
            }
            if !replacements.is_empty() {
                for detail in all_details.iter_mut() {
                    if let Some(markdown) = replacements.get(&detail.summary_id) {
                        detail.original_markdown = markdown.clone();
                    }
                }
            }
        }

        let original_summary_count = all_summaries.len();
        let mut retained_ids: HashSet<usize> = HashSet::new();
        all_summaries.retain(|summary| {
            let keep = matches!(
                summary.severity.trim().to_ascii_lowercase().as_str(),
                "high" | "medium" | "low"
            );
            if keep {
                retained_ids.insert(summary.id);
            }
            keep
        });
        all_details.retain(|detail| retained_ids.contains(&detail.summary_id));
        if all_summaries.len() < original_summary_count {
            let filtered = original_summary_count - all_summaries.len();
            let msg = format!(
                "Filtered out {filtered} informational finding{}.",
                if filtered == 1 { "" } else { "s" }
            );
            record(&mut logs, msg.clone());
            aggregated_logs.push(msg);
        }
        if all_summaries.is_empty() {
            let msg =
                "No high, medium, or low severity findings remain after filtering.".to_string();
            record(&mut logs, msg.clone());
            aggregated_logs.push(msg);
        }

        if !all_summaries.is_empty() {
            let (deduped_summaries, deduped_details, removed) =
                dedupe_bug_summaries(all_summaries, all_details);
            all_summaries = deduped_summaries;
            all_details = deduped_details;
            if removed > 0 {
                let msg = format!(
                    "Deduplicated {removed} duplicated finding{} by grouping titles/tags.",
                    if removed == 1 { "" } else { "s" }
                );
                record(&mut logs, msg.clone());
                aggregated_logs.push(msg);
            }
        }

        // Run risk rerank after deduplication to avoid redundant work.
        if !all_summaries.is_empty() {
            let risk_logs = rerank_bugs_by_risk(
                &model_client,
                &request.provider,
                &request.auth,
                &request.model,
                &mut all_summaries,
                &request.repo_path,
                &repository_summary,
                spec_for_rerank,
                metrics.clone(),
            )
            .await;
            aggregated_logs.extend(risk_logs.clone());
            for line in risk_logs {
                record(&mut logs, line);
            }
        }

        // Normalize severities again after rerank and update markdown + details.
        if !all_summaries.is_empty() {
            for summary in all_summaries.iter_mut() {
                if let Some(normalized) = normalize_severity_label(&summary.severity) {
                    summary.severity = normalized;
                } else {
                    summary.severity = summary.severity.trim().to_string();
                }
            }
            let mut replacements: HashMap<usize, String> = HashMap::new();
            for summary in all_summaries.iter_mut() {
                if let Some(updated) = rewrite_bug_markdown_severity(
                    summary.markdown.as_str(),
                    summary.severity.as_str(),
                ) {
                    summary.markdown = updated.clone();
                    replacements.insert(summary.id, updated);
                }
            }
            if !replacements.is_empty() {
                for detail in all_details.iter_mut() {
                    if let Some(markdown) = replacements.get(&detail.summary_id) {
                        detail.original_markdown = markdown.clone();
                    }
                }
            }
            // Final filter in case rerank reduced severity to informational
            let before = all_summaries.len();
            let mut retained: HashSet<usize> = HashSet::new();
            all_summaries.retain(|summary| {
                let keep = matches!(
                    summary.severity.trim().to_ascii_lowercase().as_str(),
                    "high" | "medium" | "low"
                );
                if keep {
                    retained.insert(summary.id);
                }
                keep
            });
            all_details.retain(|detail| retained.contains(&detail.summary_id));
            let after = all_summaries.len();
            if after < before {
                let filtered = before - after;
                let msg = format!(
                    "Filtered out {filtered} informational finding{} after rerank.",
                    if filtered == 1 { "" } else { "s" }
                );
                record(&mut logs, msg.clone());
                aggregated_logs.push(msg);
            }

            normalize_bug_identifiers(&mut all_summaries, &mut all_details);
        }

        if !all_summaries.is_empty() {
            let polish_message = format!(
                "Polishing markdown for {} bug finding(s).",
                all_summaries.len()
            );
            record(&mut logs, polish_message.clone());
            aggregated_logs.push(polish_message);
            let polish_logs = match polish_bug_markdowns(
                &model_client,
                &request.provider,
                &request.auth,
                &mut all_summaries,
                &mut all_details,
                metrics.clone(),
            )
            .await
            {
                Ok(logs) => logs,
                Err(err) => {
                    return Err(SecurityReviewFailure {
                        message: format!("Failed to polish bug markdown: {err}"),
                        logs: logs.clone(),
                    });
                }
            };
            for line in polish_logs {
                record(&mut logs, line.clone());
                aggregated_logs.push(line);
            }
        }

        let allowed_paths: HashSet<PathBuf> = all_summaries
            .iter()
            .map(|summary| summary.source_path.clone())
            .collect();
        let mut files_with_findings: Vec<FileSnippet> = files_map
            .into_values()
            .filter(|snippet| allowed_paths.contains(&snippet.relative_path))
            .collect();
        files_with_findings.sort_by_key(|s| s.relative_path.clone());

        let findings_count = all_summaries.len();

        record(
            &mut logs,
            format!(
                "Aggregated bug findings across {} file(s).",
                files_with_findings.len()
            ),
        );
        aggregated_logs.push(format!(
            "Aggregated bug findings across {} file(s).",
            files_with_findings.len()
        ));

        bug_summary_table = make_bug_summary_table(&all_summaries);
        if let Some(table) = bug_summary_table.as_ref() {
            record(&mut logs, "Findings summary table:".to_string());
            record(&mut logs, table.clone());
        }
        let bug_summaries = all_summaries;
        let bug_details = all_details;

        findings_summary = format_findings_summary(findings_count, files_with_findings.len());
        record(
            &mut logs,
            format!("Bug analysis summary: {}", findings_summary.as_str()),
        );
        record(&mut logs, "Bug analysis complete.".to_string());
        plan_tracker.complete_and_start_next(
            SecurityReviewPlanStep::PolishFindings,
            Some(SecurityReviewPlanStep::AssembleReport),
        );
        checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
        persist_checkpoint(&mut checkpoint, &mut logs);
        if let Some(linear_issue) = linear_issue.as_ref() {
            let prompt = build_linear_progress_prompt(
                linear_issue,
                &checkpoint.model,
                "Assemble report and artifacts",
                &checkpoint.plan_statuses,
                &checkpoint,
                &request.output_root,
            );
            let config = request.config.clone();
            let provider = request.provider.clone();
            let auth_manager = request.auth_manager.clone();
            let repo_for_task = repo_path.clone();
            let progress_for_task = progress_sender.clone();
            let log_sink_for_task = log_sink.clone();
            let metrics_for_task = metrics.clone();
            tokio::spawn(async move {
                let _ = run_linear_status_agent(
                    &config,
                    &provider,
                    auth_manager,
                    &repo_for_task,
                    progress_for_task,
                    log_sink_for_task,
                    prompt,
                    metrics_for_task,
                )
                .await;
            });
        }

        let (next_bugs_for_result, bug_snapshots) = build_bug_records(bug_summaries, bug_details);
        bugs_for_result = next_bugs_for_result;
        report_sections_prefix = Vec::new();
        if matches!(mode, SecurityReviewMode::Full) {
            if let Some(spec) = spec_generation.as_ref() {
                record(
                    &mut logs,
                    "Including combined specification in final report.".to_string(),
                );
                let trimmed = spec.combined_markdown.trim();
                if !trimmed.is_empty() {
                    report_sections_prefix.push(trimmed.to_string());
                }
            }
            if let Some(threat) = threat_model.as_ref() {
                record(
                    &mut logs,
                    "Including threat model in final report.".to_string(),
                );
                let trimmed = threat.markdown.trim();
                if !trimmed.is_empty() {
                    report_sections_prefix.push(trimmed.to_string());
                }
            }
        }

        let built_snapshot = SecurityReviewSnapshot {
            generated_at: OffsetDateTime::now_utc(),
            findings_summary: findings_summary.clone(),
            report_sections_prefix: report_sections_prefix.clone(),
            bugs: bug_snapshots,
        };

        bugs_markdown = build_bugs_markdown(&built_snapshot, git_link_info.as_ref());
        snapshot = Some(built_snapshot);
    }

    let snapshot = match snapshot {
        Some(snapshot) => snapshot,
        None => {
            return Err(SecurityReviewFailure {
                message: "Bug snapshot was not available after analysis.".to_string(),
                logs,
            });
        }
    };

    let findings_section = if bugs_markdown.trim().is_empty() {
        None
    } else {
        Some(format!("# Security Findings\n\n{}", bugs_markdown.trim()))
    };
    let report_markdown = match mode {
        SecurityReviewMode::Full => {
            let mut sections = report_sections_prefix.clone();
            if let Some(section) = findings_section.clone() {
                sections.push(section);
            }
            if sections.is_empty() {
                record(
                    &mut logs,
                    "No content available for final report.".to_string(),
                );
                None
            } else {
                record(
                    &mut logs,
                    "Final report assembled from specification, threat model, and findings."
                        .to_string(),
                );
                let combined = sections.join("\n\n");
                let cleaned = strip_operational_considerations_section(&combined);
                Some(fix_mermaid_blocks(&cleaned))
            }
        }
        SecurityReviewMode::Bugs => {
            if let Some(section) = findings_section {
                record(
                    &mut logs,
                    "Generated findings-only report for bug sweep.".to_string(),
                );
                let cleaned = strip_operational_considerations_section(&section);
                Some(fix_mermaid_blocks(&cleaned))
            } else {
                record(
                    &mut logs,
                    "No findings available for bug sweep report.".to_string(),
                );
                None
            }
        }
    };

    if is_open_source {
        let _ = run_trufflehog_scan(
            &repo_path,
            &request.output_root,
            progress_sender.clone(),
            log_sink.clone(),
            &mut logs,
        )
        .await;
    }

    // Intentionally avoid logging the output path pre-write to keep logs concise.
    let (git_commit, git_branch, git_commit_timestamp) = match git_revision.as_ref() {
        Some((commit, branch, ts)) => (Some(commit.clone()), branch.clone(), *ts),
        None => (None, None, None),
    };
    let metadata = SecurityReviewMetadata {
        mode,
        scope_paths: scope_display_paths.clone(),
        git_commit,
        git_branch,
        git_commit_timestamp,
        linear_issue: linear_issue.clone(),
    };
    let api_entries_for_persist = spec_generation
        .as_ref()
        .map(|spec| spec.api_entries.clone())
        .unwrap_or_default();
    let classification_rows_for_persist = spec_generation
        .as_ref()
        .map(|spec| spec.classification_rows.clone())
        .unwrap_or_default();
    let classification_table_for_persist = spec_generation
        .as_ref()
        .and_then(|spec| spec.classification_table.clone());
    let artifacts = match persist_artifacts(
        &request.output_root,
        &repo_path,
        &metadata,
        &bugs_markdown,
        &api_entries_for_persist,
        &classification_rows_for_persist,
        classification_table_for_persist.as_deref(),
        report_markdown.as_deref(),
        &snapshot,
    )
    .await
    {
        Ok(paths) => {
            record(
                &mut logs,
                format!("Artifacts written to {}", request.output_root.display()),
            );
            checkpoint.bug_snapshot_path = Some(paths.snapshot_path.clone());
            checkpoint.bugs_path = Some(paths.bugs_path.clone());
            checkpoint.report_path = paths.report_path.clone();
            checkpoint.report_html_path = paths.report_html_path.clone();
            checkpoint.api_overview_path = paths.api_overview_path.clone();
            checkpoint.classification_json_path = paths.classification_json_path.clone();
            checkpoint.classification_table_path = paths.classification_table_path.clone();
            checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
            persist_checkpoint(&mut checkpoint, &mut logs);
            paths
        }
        Err(err) => {
            record(&mut logs, format!("Failed to write artifacts: {err}"));
            return Err(SecurityReviewFailure {
                message: format!("Failed to write artifacts: {err}"),
                logs,
            });
        }
    };
    plan_tracker.mark_complete(SecurityReviewPlanStep::AssembleReport);
    checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
    checkpoint.status = SecurityReviewCheckpointStatus::Complete;
    persist_checkpoint(&mut checkpoint, &mut logs);

    let elapsed = overall_start.elapsed();
    let elapsed_display = fmt_elapsed_compact(elapsed.as_secs());
    let metrics_snapshot = metrics.snapshot();
    let token_usage = metrics.snapshot_usage();
    let rate_limit_wait = metrics.rate_limit_wait();
    let mut estimated_cost = None;
    let tool_summary = metrics_snapshot.tool_call_summary();
    let wait_display = fmt_elapsed_compact(rate_limit_wait.as_secs());
    record(
        &mut logs,
        format!(
            "Security review duration: {elapsed_display} (rate-limit backoff: {wait_display}; model calls: {model_calls}; tool calls: {tool_summary}).",
            model_calls = metrics_snapshot.model_calls,
        ),
    );

    if !token_usage.is_zero() {
        record(
            &mut logs,
            FinalOutput::from(token_usage.clone()).to_string(),
        );
        match fetch_openrouter_pricing(&model_client, &request.model).await {
            Ok(Some(pricing)) => {
                let cost = compute_cost_breakdown(&token_usage, &pricing);
                record(
                    &mut logs,
                    format!("Estimated model cost: ${:.4}.", cost.total),
                );
                estimated_cost = Some(cost.total);
            }
            Ok(None) => {
                record(
                    &mut logs,
                    format!(
                        "Pricing data for model {} not found via OpenRouter; skipping cost estimate.",
                        request.model
                    ),
                );
            }
            Err(err) => {
                record(
                    &mut logs,
                    format!("Failed to fetch pricing for model {}: {err}", request.model),
                );
            }
        }
    }

    let cost_label = match estimated_cost {
        Some(cost) => format!("{cost:.4}"),
        None => "unknown".to_string(),
    };
    let runtime_summary = format!(
        "security analysis completed in {elapsed_display} (rate-limit backoff {wait_display}; model calls {model_calls}; tool calls {tool_summary}; total tokens {total_tokens} = input {input_tokens} + cached_input {cached_input_tokens} + output {output_tokens}; estimated cost {cost_label}).",
        model_calls = metrics_snapshot.model_calls,
        tool_summary = tool_summary,
        total_tokens = token_usage.total_tokens,
        input_tokens = token_usage.input_tokens,
        cached_input_tokens = token_usage.cached_input_tokens,
        output_tokens = token_usage.output_tokens + token_usage.reasoning_output_tokens,
        cost_label = cost_label,
    );
    let revision_summary = git_revision
        .as_ref()
        .map(|(commit, branch, ts)| format_revision_label(commit.as_str(), branch.as_ref(), *ts))
        .unwrap_or_else(|| "unknown".to_string());
    let trufflehog_path = if is_open_source {
        let candidate = request.output_root.join("trufflehog.jsonl");
        candidate.exists().then_some(candidate)
    } else {
        None
    };

    if let Some(report_path) = artifacts.report_path.as_ref() {
        record(
            &mut logs,
            format!("Report markdown: {}", report_path.display()),
        );
    }
    if let Some(report_html_path) = artifacts.report_html_path.as_ref() {
        record(
            &mut logs,
            format!("Report HTML: {}", report_html_path.display()),
        );
    }

    // Final Linear sync and per-bug ticket creation.
    if let Some(linear_issue) = linear_issue.as_ref() {
        // Create child tickets (unassigned) for each bug and link back to the review issue.
        let create_prompt = build_linear_create_tickets_prompt(linear_issue, &bugs_markdown);
        {
            let config = request.config.clone();
            let provider = request.provider.clone();
            let auth_manager = request.auth_manager.clone();
            let repo_for_task = repo_path.clone();
            let progress_for_task = progress_sender.clone();
            let log_sink_for_task = log_sink.clone();
            let metrics_for_task = metrics.clone();
            tokio::spawn(async move {
                let _ = run_linear_status_agent(
                    &config,
                    &provider,
                    auth_manager,
                    &repo_for_task,
                    progress_for_task,
                    log_sink_for_task,
                    create_prompt,
                    metrics_for_task,
                )
                .await;
            });
        }

        // Then post a single final summary comment and finalize the status block.
        {
            let config = request.config.clone();
            let provider = request.provider.clone();
            let auth_manager = request.auth_manager.clone();
            let repo_for_task = repo_path.clone();
            let progress_for_task = progress_sender.clone();
            let log_sink_for_task = log_sink.clone();
            let metrics_for_task = metrics.clone();
            let issue_for_task = linear_issue.clone();
            let checkpoint_for_task = checkpoint.clone();
            let output_root_for_task = request.output_root.clone();
            let trufflehog_for_task = trufflehog_path;
            let runtime_for_task = runtime_summary;
            let revision_for_task = revision_summary;
            tokio::spawn(async move {
                let final_prompt = build_linear_finalize_prompt(
                    &issue_for_task,
                    &checkpoint_for_task.model,
                    &checkpoint_for_task.plan_statuses,
                    &output_root_for_task,
                    &checkpoint_for_task,
                    &runtime_for_task,
                    &revision_for_task,
                    trufflehog_for_task.as_deref(),
                );
                let _ = run_linear_status_agent(
                    &config,
                    &provider,
                    auth_manager,
                    &repo_for_task,
                    progress_for_task,
                    log_sink_for_task,
                    final_prompt,
                    metrics_for_task,
                )
                .await;
            });
        }
    }

    // Omit redundant completion log; the UI presents a follow-up line.

    Ok(SecurityReviewResult {
        findings_summary,
        bug_summary_table,
        bugs: bugs_for_result,
        bugs_path: artifacts.bugs_path,
        report_path: artifacts.report_path,
        report_html_path: artifacts.report_html_path,
        snapshot_path: artifacts.snapshot_path,
        metadata_path: artifacts.metadata_path,
        api_overview_path: artifacts.api_overview_path,
        classification_json_path: artifacts.classification_json_path,
        classification_table_path: artifacts.classification_table_path,
        logs,
        token_usage,
        estimated_cost_usd: estimated_cost,
        rate_limit_wait,
    })
}

fn render_checklist_markdown(statuses: &HashMap<String, StepStatus>) -> String {
    let mut lines: Vec<String> = Vec::new();
    let order = [
        ("generate_specs", "Generate system specifications"),
        ("threat_model", "Draft threat model"),
        ("analyze_bugs", "Analyze code for bugs"),
        ("polish_findings", "Polish, dedupe, and rerank findings"),
        ("assemble_report", "Assemble report and artifacts"),
    ];
    for (slug, title) in order {
        let status = statuses.get(slug);
        let mark = match status {
            Some(StepStatus::Completed) => "[x]",
            Some(StepStatus::InProgress) => "[~]",
            _ => "[ ]",
        };
        lines.push(format!("- {mark} {title}"));
    }
    lines.join("\n")
}

const LINEAR_SCOPE_CONTEXT_SYSTEM_PROMPT: &str = "Gather the full Linear issue context for a security review without modifying the ticket. Use Linear MCP tools only. Do not plan or reason before acting. Your FIRST action must be a Linear MCP call to fetch the issue by key; your SECOND action must fetch all comments/activity for that same issue. If attachments exist, fetch/describe them. Respond with plaintext only (no code fences). Retry failed Linear calls once, otherwise surface the failure succinctly.";

fn build_linear_scope_context_prompt(issue_ref: &str) -> String {
    format!(
        "Collect Linear issue context for `{issue_ref}` to inform auto-scoping.\n\nStrict tool order:\n1) Call Linear MCP to fetch this exact issue immediately (no planning).\n2) Call Linear MCP to fetch ALL comments/activity for the same issue.\n3) If attachments exist, fetch/describe them via Linear MCP.\n\nRequirements:\n- Read the full issue description and attachments.\n- Read ALL activity entries and comments, including authors and timestamps.\n- Capture the current issue status/state and assignee.\n- Do not edit the issue.\n\nOutput (plaintext):\n- One-line summary of the issue intent.\n- Current status/state and assignee.\n- Activity timeline (newest first) with key events.\n- Comments section listing each comment with author, timestamp, and body. Do not skip comments.\n- Attachments (if any) with brief notes."
    )
}

#[allow(clippy::too_many_arguments)]
async fn fetch_linear_context_for_auto_scope(
    config: &Config,
    provider: &ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    repo_root: &Path,
    issue_ref: &str,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
) -> Result<(String, Vec<String>), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();
    let mut lin_config = config.clone();
    lin_config.model = "gpt-5.1".to_string();
    lin_config.model_provider = provider.clone();
    lin_config.base_instructions = Some(LINEAR_SCOPE_CONTEXT_SYSTEM_PROMPT.to_string());
    lin_config.user_instructions = None;
    lin_config.developer_instructions = None;
    lin_config.compact_prompt = None;
    lin_config.cwd = repo_root.to_path_buf();
    lin_config
        .features
        .disable(Feature::ApplyPatchFreeform)
        .disable(Feature::WebSearchRequest)
        .disable(Feature::ViewImageTool)
        .disable(Feature::RmcpClient);
    lin_config.use_experimental_use_rmcp_client = false;

    let manager = ConversationManager::new(
        auth_manager,
        SessionSource::SubAgent(SubAgentSource::Other(
            "security_review_linear_context".to_string(),
        )),
    );

    let conversation = match manager.new_conversation(lin_config).await {
        Ok(new_conversation) => new_conversation.conversation,
        Err(err) => {
            let message = format!("Failed to start Linear context agent: {err}");
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };

    let prompt = build_linear_scope_context_prompt(issue_ref);
    if let Err(err) = conversation
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: prompt }],
        })
        .await
    {
        let message = format!("Failed to submit Linear context prompt: {err}");
        push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
        return Err(SecurityReviewFailure { message, logs });
    }

    let mut last_agent_message: Option<String> = None;
    let mut last_tool_log: Option<String> = None;

    loop {
        let event = match conversation.next_event().await {
            Ok(event) => event,
            Err(err) => {
                let message = format!("Linear context agent terminated unexpectedly: {err}");
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
        };

        record_tool_call_from_event(metrics.as_ref(), &event.msg);

        match event.msg {
            EventMsg::TaskComplete(done) => {
                if let Some(msg) = done.last_agent_message {
                    last_agent_message = Some(msg);
                }
                break;
            }
            EventMsg::AgentMessage(msg) => last_agent_message = Some(msg.message.clone()),
            EventMsg::McpToolCallBegin(begin) => {
                let tool = begin.invocation.tool.clone();
                let message = format!("Linear context: tool → {tool}");
                if last_tool_log.as_deref() != Some(message.as_str()) {
                    push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                    last_tool_log = Some(message);
                }
            }
            EventMsg::Warning(warn) => {
                push_progress_log(&progress_sender, &log_sink, &mut logs, warn.message);
            }
            EventMsg::Error(err) => {
                let message = format!(
                    "Linear context agent error: {message}",
                    message = err.message
                );
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
            EventMsg::TurnAborted(aborted) => {
                let message = format!("Linear context agent aborted: {:?}", aborted.reason);
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
            EventMsg::TokenCount(count) => {
                if let Some(info) = count.info {
                    metrics.record_model_call();
                    metrics.record_usage(&info.last_token_usage);
                }
            }
            _ => {}
        }
    }

    let response = match last_agent_message.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }) {
        Some(text) => text,
        None => {
            let message = "Linear context agent produced an empty response.".to_string();
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };

    let _ = conversation.submit(Op::Shutdown).await;

    Ok((response, logs))
}

fn build_linear_init_prompt(
    issue_ref: &str,
    model_name: &str,
    repo_root: &Path,
    output_root: &Path,
    mode: SecurityReviewMode,
    include_paths: &[PathBuf],
    scope_display_paths: &[String],
    scope_file_path: Option<&Path>,
    statuses: &HashMap<String, StepStatus>,
) -> String {
    let scope = if scope_display_paths.is_empty() {
        "entire repository".to_string()
    } else {
        scope_display_paths.join(", ")
    };
    let checklist = render_checklist_markdown(statuses);
    let scope_file_text = scope_file_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(not generated)".to_string());
    let include_text = if include_paths.is_empty() {
        "(auto-scoped)".to_string()
    } else {
        include_paths
            .iter()
            .map(|p| display_path_for(p, repo_root))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let workspace_marker = encode_workspace_hint(repo_root).unwrap_or_default();
    format!(
        "You are a workflow assistant that manages a security review using the configured MCP servers (Linear, Notion, Google Workspace, Secbot).\n\nTask: Using the Linear MCP server, open the issue `{issue}` directly (call `get_issue` first; avoid team or project enumeration unless the key fails).\n\nRules:\n- No long planning steps; fetch the issue immediately with `get_issue`, then proceed.\n- Do not post Linear comments until the full security review is finished; keep all interim updates in the description’s “Security Agent ({model}) - Review Status” block.\n- Prepend a header `# Security Agent ({model}) - Review Status` when you add the status section; do not add any extra automation tagline.\n- Preserve existing status content, key insights, and doc summaries—append or merge updates instead of overwriting.\n- Do not paste full findings or reproduction details into the description; use concise bullets and reference attachments/artifacts instead.\n\nSteps:\n1) Classify the issue. If this is NOT a security_review request, prepend a short note in the status block that automation only runs for security review tickets, then stop.\n2) If it IS a security_review: gather all context from the issue description, attachments, and comments. Open every linked doc (including links inside comments) with the appropriate MCP (Notion or Google Workspace) and read them fully; use Google Workspace MCP to open Google Docs links found in comments. Use Secbot MCP to search for prior contexts, tickets, policies, and recommendations relevant to this issue; reason from the issue context, select only relevant hits, and explain why each matters instead of dumping raw results.\n3) From the collected context, check for missing essentials (e.g., code locations/links, critical docs/flows, required policies). If critical items are missing (e.g., no code links), pause the review and capture the requests in the “Follow-ups / Missing Inputs” subsection of the status block; do not continue until inputs arrive. If most information is present, continue.\n4) Update the Linear issue description by PREPENDING a section titled “Security Agent ({model}) - Review Status” (remove any existing section with that heading first) with:\n   - A header line: `# Security Agent ({model}) - Review Status`\n   - Key points summary (3–7 concise bullets highlighting security-relevant conclusions and recommended next steps; integrate insights from docs and Secbot here when they materially change risk, but avoid restating raw observations).\n   - Scope summary: a short description of what was reviewed (for example, key services, directories, or APIs) rather than a full path dump. Do NOT inline every path or add a long `scope: ...` line; instead, ensure the \"Scope paths\" artifact ({scope_file}) is uploaded and reference it once in this bullet.\n   - Design / architecture docs: only include genuine design, architecture, requirements, threat-model, or runbook documents. Summarize each in 1–2 lines with inline hyperlinks and highlight why it matters for security. Skip generic README/env/source files that merely restate code.\n   - Related Secbot / precedent tickets: only when Secbot MCP surfaces clearly related policies or tickets grounded in the same repo, service, or code paths (or explicitly requested in the issue). Summarize why each is relevant and include the assignee and last-updated date. If no strong matches exist, omit this subsection entirely rather than forcing weak content.\n   - A checklist mirroring the AppSec pipeline steps.\n   - A “Design/Implementation follow-ups” subsection listing concrete security requirements that must be validated during build (derive from the issue/docs when present—e.g., user content isolation via a separate domain, role- or resource-based access control checks). When details are missing, phrase items as questions for the product/eng team so they can answer before implementation.\n   - A “Follow-ups / Missing Inputs” subsection listing any missing docs, flows, or code locations that block analysis; phrase items as actionable questions for the ticket owner.\n   - Artifacts: reference uploaded attachments only (no local filesystem paths, no gists). For scope details and triage outputs, reference the uploaded scope file or \"Scope paths\" artifact ({scope_file}) instead of pasting lists of directories or files.\n\nChecklist:\n{checklist}\n\nRepository: {repo}\nMode: {mode}\nScope: {scope}\nIncluded paths: {include_text}\nArtifacts directory (local): {art_dir}\nScope file (upload): {scope_file}\nWorkspace marker (embed this exact marker on its own line inside the status block so Codex can resume with the same local workspace later): {workspace_marker}\n\nLocal workspace convention:\n- Keep all repositories under `~/code`.\n- If a required repository is missing under `~/code`, use GitHub CLI to clone a shallow copy (depth=1). Example:\n  `gh repo clone owner/repo ~/code/repo -- --depth=1`\n\nNotes:\n- Preserve the existing description content below the new status section.\n- Do not remove any existing details; prepend the status section.\n- Keep local filesystem paths out of Linear updates; reference uploaded attachments (no gists or external pastes).\n- When resuming, download any relevant attachments locally into the artifacts directory before continuing.",
        issue = issue_ref,
        model = model_name,
        repo = repo_root.display(),
        mode = mode.as_str(),
        scope = scope,
        include_text = include_text,
        art_dir = output_root.display(),
        scope_file = scope_file_text,
        workspace_marker = workspace_marker,
    )
}

fn build_linear_progress_prompt(
    issue_ref: &str,
    model_name: &str,
    step_title: &str,
    statuses: &HashMap<String, StepStatus>,
    checkpoint: &SecurityReviewCheckpoint,
    output_root: &Path,
) -> String {
    let checklist = render_checklist_markdown(statuses);
    let mut artifacts: Vec<String> = Vec::new();
    if let Some(path) = checkpoint.classification_table_path.as_ref() {
        artifacts.push(format!("classification_table: {}", path.display()));
    }
    if let Some(path) = checkpoint.api_overview_path.as_ref() {
        artifacts.push(format!("api_overview: {}", path.display()));
    }
    if let Some(sn) = checkpoint.bug_snapshot_path.as_ref() {
        artifacts.push(format!("bug_snapshot: {}", sn.display()));
    }
    if let Some(bp) = checkpoint.bugs_path.as_ref() {
        artifacts.push(format!("bugs_markdown: {}", bp.display()));
    }
    if let Some(rp) = checkpoint.report_path.as_ref() {
        artifacts.push(format!("report_markdown: {}", rp.display()));
    }
    if let Some(rh) = checkpoint.report_html_path.as_ref() {
        artifacts.push(format!("report_html: {}", rh.display()));
    }
    let scope_paths_text = if checkpoint.scope_display_paths.is_empty() {
        "entire repository".to_string()
    } else {
        checkpoint.scope_display_paths.join(", ")
    };
    let scope_file_text = checkpoint
        .scope_file_path
        .as_deref()
        .unwrap_or("(not generated)");
    let artifacts_section = if artifacts.is_empty() {
        String::new()
    } else {
        let joined = artifacts.join("\n");
        format!(
            "\nKnown local artifacts:\n{joined}\nLocal artifacts root: {root}",
            joined = joined,
            root = output_root.display(),
        )
    };
    format!(
        "Use the Linear MCP to update issue `{issue}` progress.\n\nRules:\n- Open the issue directly by key `{issue}` (call `get_issue` first; avoid team/resource enumeration unless lookup fails).\n- No Linear comments until all security review steps are complete; keep updates in the description’s “Security Agent ({model}) - Review Status” block.\n- Prepend a header `# Security Agent ({model}) - Review Status` when you add the status section; do not add any extra automation tagline. Preserve existing status content, key insights, and doc summaries—append/merge instead of overwriting.\n- Do not paste full findings; reference uploaded artifacts instead.\n- Keep artifact references limited to uploaded attachments; do not expose local filesystem paths or create gists. Upload files directly from the local artifacts directory ({art_root}).\n- Before updating, read any newly linked docs in the description or comments via Notion/Google Workspace MCP (use Google Workspace MCP for Google Docs) and summarize; include inline hyperlinks to the docs in your summary.\n- Check Secbot MCP for any new relevant policies/recommendations; reason from the issue context, explain why results matter, and when Secbot surfaces similar or relevant tickets, capture assignee and date in the summary.\n- Upload/attach the scope file if not already present ({scope_file}) so that readers can inspect the full “Scope paths” artifact; do NOT paste long `scope: ...` lines or raw path dumps into the description.\n\nActions:\n- Update the checklist status for the step: {step}.\n- Maintain a single “Scope summary” bullet in the status block that briefly describes what is in scope and points to the uploaded \"Scope paths\" artifact instead of listing every path.\n- Add or update a “Design/Implementation follow-ups” subsection with concrete items to check during build (derive from the issue/docs—e.g., user content isolation via a separate domain, role- or resource-based access control checks). When details are missing, phrase the items as questions for the product/eng team.\n- If new artifacts exist, upload/attach them from the local artifacts directory (no gists or external shares) and reference them in the status block without including local paths.\n- Capture any new gaps (docs, flows, code links) in the “Follow-ups / Missing Inputs” subsection instead of posting comments.\n- If critical inputs (e.g., code links) are still missing, pause and keep them in “Follow-ups / Missing Inputs” until they are provided.\n\nChecklist now:\n{checklist}{artifacts_section}\n\nScope file (upload): {scope_file}\nScope paths: {scope_paths}",
        issue = issue_ref,
        model = model_name,
        step = step_title,
        checklist = checklist,
        artifacts_section = artifacts_section,
        scope_file = scope_file_text,
        scope_paths = scope_paths_text,
        art_root = output_root.display(),
    )
}

fn build_linear_finalize_prompt(
    issue_ref: &str,
    model_name: &str,
    statuses: &HashMap<String, StepStatus>,
    output_root: &Path,
    checkpoint: &SecurityReviewCheckpoint,
    runtime_summary: &str,
    revision_summary: &str,
    trufflehog_path: Option<&Path>,
) -> String {
    let checklist = render_checklist_markdown(statuses);
    let scope_paths_text = if checkpoint.scope_display_paths.is_empty() {
        "entire repository".to_string()
    } else {
        checkpoint.scope_display_paths.join(", ")
    };
    let scope_file_text = checkpoint.scope_file_path.as_deref().unwrap_or("(missing)");
    let html_path = checkpoint
        .report_html_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(missing)".to_string());
    let md_path = checkpoint
        .report_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(missing)".to_string());
    let trufflehog_text = trufflehog_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(not run)".to_string());

    format!(
        "Finalize Linear sync for `{issue}`.\n\nRules:\n- Open the issue directly by key `{issue}` (call `get_issue` first; avoid team/resource enumeration unless lookup fails).\n- Keep the description’s “Security Agent ({model}) - Review Status” block intact with the header `# Security Agent ({model}) - Review Status`; remove any prior instance of this section before writing the new one; do not add any extra automation tagline. Preserve earlier summaries and doc context—append final updates instead of overwriting.\n- Do not paste full findings; reference artifacts and child tickets instead.\n- Keep artifact references limited to uploaded attachments; do not expose local filesystem paths or create gists. Upload from the local artifacts directory ({root}) first, then reference.\n- Ensure the triaged scope file is attached and clearly labeled as the \"Scope paths\" artifact ({scope_file}); do **not** inline long `scope: ...` lines or raw path dumps into the description.\n- Before writing the final comment, use Linear MCP to list child/sub-issues and related issues for `{issue}`; treat tickets created for this review as the ground truth for findings.\n\nFinal status block:\n- Update the “Security Agent ({model}) - Review Status” section in the description so it reflects the final checklist below and the final scope summary while keeping previous insights and doc summaries.\n- Keep the scope section short (1–2 lines) and refer to the attached \"Scope paths\" artifact rather than listing every path.\n- Maintain a “Design/Implementation follow-ups” subsection with concrete build-time checks based on the issue/docs (e.g., user content isolation via a separate domain, role- or resource-based access control). When context is missing, phrase the items as questions for the product/eng team so future runs can confirm them.\n- Fold key insights from design docs, Secbot, and prior comments into the key points and follow-ups instead of listing irrelevant docs.\n\nFinal comment (single comment only):\n- Post exactly one final comment on the issue, prefixed with `Security Agent ({model}) - automated security review` so humans can recognize automation.\n- Structure the comment into three plain-language sections (no angle-bracket tags):\n  - Conclusion — a short paragraph summarizing whether the security analysis is complete and whether any HIGH or MEDIUM risk findings remain. If there are no HIGH or MEDIUM findings, state clearly that no blocking issues were found and that the surface appears ready from an AppSec perspective (subject to other launch gates), and that the review can likely be closed.\n  - Findings — a bullet list focusing on HIGH-severity findings only. For each HIGH finding, name the issue, mention its risk briefly, and link to its child ticket key. Summarize MEDIUM/LOW findings only as counts or short notes without listing each one.\n  - Runtime summary — a single line summarizing the runtime metrics for this review using: {runtime}. Include the analyzed revision line `{revision}` when it is available.\n- Do not include the full HTML report inline in the comment; instead, link to the uploaded markdown/HTML reports and the scope file.\n\nArtifacts to rely on:\n- report_markdown: {md}\n- report_html: {html}\n- scope_file (\"Scope paths\" artifact): {scope_file}\n- analyzed_scope_label: {scope_paths}\n- trufflehog_json (open-source reviews only, if present): {trufflehog}\n- artifacts_root (local, for uploads only): {root}\n\nChecklist:\n{checklist}",
        issue = issue_ref,
        model = model_name,
        checklist = checklist,
        scope_paths = scope_paths_text,
        scope_file = scope_file_text,
        html = html_path,
        md = md_path,
        root = output_root.display(),
        runtime = runtime_summary,
        revision = revision_summary,
        trufflehog = trufflehog_text,
    )
}

fn build_linear_create_tickets_prompt(issue_ref: &str, bugs_markdown: &str) -> String {
    let bugs_preview = bugs_markdown.trim();
    let bugs_block = if bugs_preview.is_empty() {
        "No structured findings were produced; do not create any child tickets.".to_string()
    } else {
        format!("Source findings markdown (full content):\n```markdown\n{bugs_preview}\n```")
    };
    format!(
        "Create Linear child tickets for validated security findings and link them to the review ticket `{issue_ref}`.\n\n{bugs_block}\n\nInstructions:\n- Work only from the findings markdown above plus the main review ticket context; do NOT invent findings.\n- For each HIGH-severity finding, first search for existing sub-issues or clearly related tickets (matching severity + component/file + summary) using Linear MCP. If a closely matching ticket already exists, link that ticket to `{issue_ref}` and **do not** create a duplicate.\n- Only create new tickets for findings that have no similar existing ticket.\n- When creating a new ticket, keep it unassigned and in the initial TODO/backlog state; do **not** auto-assign owners or move tickets to triaged/in-progress/done.\n- Parse the bugs markdown to create concise titles that include severity and the most important component/file.\n- Include reproduction/verification details and recommendations in each ticket body.\n- If the finding includes blame information or a suggested owner (for example, a `Suggested owner:` line derived from git blame), copy that line into the ticket body but do **not** set an assignee.\n- Add links back to the review ticket `{issue_ref}` so engineers can reach the full report and artifacts.\n- Do **not** post comments on the review ticket; this step is only for creating/linking child tickets. The final summary comment is handled separately.\n- Keep everything within the Linear MCP; do not use external network calls.",
    )
}

fn build_linear_related_docs_prompt(
    issue_ref: &str,
    model_name: &str,
    checkpoint: &SecurityReviewCheckpoint,
    output_root: &Path,
) -> String {
    let scope_paths_text = if checkpoint.scope_display_paths.is_empty() {
        "entire repository".to_string()
    } else {
        checkpoint.scope_display_paths.join(", ")
    };
    let scope_file_text = checkpoint
        .scope_file_path
        .as_deref()
        .unwrap_or("(not generated)");
    format!(
        "Unblock analysis by gathering related docs and updating the Linear status block for `{issue}`.\n\nSteps:\n- Use Linear MCP to open the issue immediately.\n- Collect all doc links (description + comments), then read them via Notion/Google Workspace MCP; include inline hyperlinks.\n- Use Secbot MCP to find prior contexts/policies related to this issue; reason from the issue context, select only relevant hits, and explain why each matters.\n- Summarize briefly in the description’s “Security Agent ({model}) - Review Status” block under a \"Related Docs / Secbot Results\" section; keep bullets concise.\n- Do not post comments; update only the status block. Keep artifact references to uploaded attachments only (local artifacts root: {art_root}).\n\nReminders:\n- Scope paths: {scope_paths}\n- Scope file: {scope_file}\n- Keep local filesystem paths out of Linear; link uploaded files or remote docs only.",
        issue = issue_ref,
        model = model_name,
        scope_paths = scope_paths_text,
        scope_file = scope_file_text,
        art_root = output_root.display(),
    )
}

#[allow(clippy::too_many_arguments)]
async fn run_linear_status_agent(
    config: &Config,
    provider: &ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    repo_root: &Path,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    prompt: String,
    metrics: Arc<ReviewMetrics>,
) -> Result<(), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();
    let mut lin_config = config.clone();
    lin_config.model = "gpt-5.1".to_string();
    lin_config.model_provider = provider.clone();
    lin_config.base_instructions = Some(
        "Use Linear, Notion, and Google Workspace via MCP tools. Prefer MCP tools for reading/updating issues and documents. When an issue key is provided, open it directly with Linear MCP `get_issue` before any searches; avoid enumerating teams/projects/resources unless direct lookup fails. Keep repositories under ~/code; if a repository is missing under ~/code, use GitHub CLI to clone a shallow copy (depth=1), e.g., `gh repo clone owner/repo ~/code/repo -- --depth=1`."
            .to_string(),
    );
    lin_config.user_instructions = None;
    lin_config.developer_instructions = None;
    lin_config.compact_prompt = None;
    lin_config.cwd = repo_root.to_path_buf();
    // Keep MCP servers as configured by the user. Avoid risky tools here.
    lin_config
        .features
        .disable(Feature::ApplyPatchFreeform)
        .disable(Feature::WebSearchRequest)
        .disable(Feature::ViewImageTool)
        .disable(Feature::RmcpClient);
    lin_config.use_experimental_use_rmcp_client = false;

    let manager = ConversationManager::new(
        auth_manager,
        SessionSource::SubAgent(SubAgentSource::Other("security_review_linear".to_string())),
    );

    let conversation = match manager.new_conversation(lin_config).await {
        Ok(new_conversation) => new_conversation.conversation,
        Err(err) => {
            let message = format!("Failed to start Linear status agent: {err}");
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };

    if let Err(err) = conversation
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: prompt }],
        })
        .await
    {
        let message = format!("Failed to submit Linear status prompt: {err}");
        push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
        return Err(SecurityReviewFailure { message, logs });
    }

    let mut last_agent_message: Option<String> = None;
    let mut last_tool_log: Option<String> = None;

    loop {
        let event = match conversation.next_event().await {
            Ok(event) => event,
            Err(err) => {
                let message = format!("Linear status agent terminated unexpectedly: {err}");
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
        };

        record_tool_call_from_event(metrics.as_ref(), &event.msg);

        match event.msg {
            EventMsg::TaskComplete(done) => {
                if let Some(msg) = done.last_agent_message {
                    last_agent_message = Some(msg);
                }
                break;
            }
            EventMsg::AgentMessage(msg) => {
                last_agent_message = Some(msg.message.clone());
            }
            EventMsg::AgentReasoning(reason) => {
                log_model_reasoning(&reason.text, &progress_sender, &log_sink, &mut logs);
            }
            EventMsg::McpToolCallBegin(begin) => {
                let tool = begin.invocation.tool.clone();
                let message = format!("Linear status: tool → {tool}");
                if last_tool_log.as_deref() != Some(message.as_str()) {
                    push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                    last_tool_log = Some(message);
                }
            }
            EventMsg::TokenCount(count) => {
                if let Some(info) = count.info {
                    metrics.record_model_call();
                    metrics.record_usage(&info.last_token_usage);
                }
            }
            EventMsg::Warning(warn) => {
                push_progress_log(&progress_sender, &log_sink, &mut logs, warn.message);
            }
            EventMsg::Error(err) => {
                let message = format!("Linear status agent error: {}", err.message);
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
            EventMsg::TurnAborted(aborted) => {
                let message = format!("Linear status agent aborted: {:?}", aborted.reason);
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
            _ => {}
        }
    }

    if let Some(msg) = last_agent_message {
        let trimmed = msg.trim();
        if !trimmed.is_empty() {
            push_progress_log(
                &progress_sender,
                &log_sink,
                &mut logs,
                format!("Linear status response: {trimmed}"),
            );
        }
    }

    let _ = conversation.submit(Op::Shutdown).await;
    Ok(())
}

async fn await_with_heartbeat<F, T, E>(
    progress_sender: Option<AppEventSender>,
    stage: &str,
    detail: Option<&str>,
    fut: F,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
{
    tokio::pin!(fut);

    if progress_sender.is_none() {
        return fut.await;
    }

    let start = Instant::now();
    let mut ticker = tokio::time::interval(Duration::from_secs(5));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            res = &mut fut => break res,
            _ = ticker.tick() => {
                if let Some(tx) = progress_sender.as_ref() {
                    let _elapsed = start.elapsed().as_secs();
                    let extra = detail
                        .map(|d| format!(" - {d}"))
                        .unwrap_or_default();
                    tx.send(AppEvent::SecurityReviewLog(format!(
                        "Still {stage}{extra}."
                    )));
                }
            }
        }
    }
}

fn write_log_sink(log_sink: &Option<Arc<SecurityReviewLogSink>>, message: &str) {
    if let Some(sink) = log_sink.as_ref() {
        sink.write(message);
    }
}

fn push_progress_log(
    progress_sender: &Option<AppEventSender>,
    log_sink: &Option<Arc<SecurityReviewLogSink>>,
    logs: &mut Vec<String>,
    message: String,
) {
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(message.clone()));
    }
    write_log_sink(log_sink, message.as_str());
    logs.push(message);
}

fn append_log(
    log_sink: &Option<Arc<SecurityReviewLogSink>>,
    logs: &mut Vec<String>,
    message: String,
) {
    write_log_sink(log_sink, message.as_str());
    logs.push(message);
}

fn sanitize_reasoning_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for segment in trimmed
        .split(" - ")
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if parts
            .last()
            .map(|last| last.eq_ignore_ascii_case(segment))
            .unwrap_or(false)
        {
            continue;
        }
        parts.push(segment);
    }
    if parts.is_empty() {
        Some(trimmed.to_string())
    } else {
        Some(parts.join(" - "))
    }
}

fn log_model_reasoning(
    reasoning: &str,
    progress_sender: &Option<AppEventSender>,
    log_sink: &Option<Arc<SecurityReviewLogSink>>,
    logs: &mut Vec<String>,
) {
    for line in reasoning
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(sanitized) = sanitize_reasoning_line(line) {
            if sanitized.is_empty() {
                continue;
            }
            let truncated = truncate_text(&sanitized, MODEL_REASONING_LOG_MAX_GRAPHEMES);
            let message = format!("Model reasoning: {truncated}");
            push_progress_log(progress_sender, log_sink, logs, message);
        }
    }
}

fn collect_snippets_blocking(
    repo_path: PathBuf,
    include_paths: Vec<PathBuf>,
    max_files: usize,
    max_bytes_per_file: usize,
    max_total_bytes: usize,
    progress_sender: Option<AppEventSender>,
) -> Result<FileCollectionResult, SecurityReviewFailure> {
    let mut state = CollectionState::new(
        repo_path.clone(),
        max_files,
        max_bytes_per_file,
        max_total_bytes,
        progress_sender,
    );
    let mut logs = Vec::new();

    let targets = if include_paths.is_empty() {
        vec![repo_path]
    } else {
        include_paths
    };

    for target in targets {
        if state.limit_reached() {
            break;
        }
        state.emit_progress_message(format!("Scanning {}...", target.display()));
        if let Err(err) = state.visit_path(&target) {
            logs.push(err.clone());
            return Err(SecurityReviewFailure { message: err, logs });
        }
    }

    if state.snippets.is_empty() {
        logs.push("No eligible files found during collection.".to_string());
        return Err(SecurityReviewFailure {
            message: "No eligible files found during collection.".to_string(),
            logs,
        });
    }

    if state.limit_hit {
        let reason = state.limit_reason.clone().unwrap_or_else(|| {
            "Reached file collection limits before scanning entire scope.".to_string()
        });
        logs.push(reason);
        logs.push(
            "Proceeding with the collected subset; rerun with `/secreview --path ...` to refine scope."
                .to_string(),
        );
    }

    logs.push(format!(
        "Collected {} files for analysis ({} total).",
        state.snippets.len(),
        human_readable_bytes(state.total_bytes)
    ));

    Ok(FileCollectionResult {
        snippets: state.snippets,
        logs,
    })
}

#[allow(clippy::needless_collect, clippy::too_many_arguments)]
async fn triage_files_for_bug_analysis(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    triage_model: &str,
    scope_prompt: Option<String>,
    snippets: Vec<FileSnippet>,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
) -> Result<FileTriageResult, SecurityReviewFailure> {
    let total = snippets.len();
    let mut logs: Vec<String> = Vec::new();
    let triage_model = if triage_model.trim().is_empty() {
        FILE_TRIAGE_MODEL
    } else {
        triage_model
    };

    if total == 0 {
        return Ok(FileTriageResult {
            included: Vec::new(),
            logs,
        });
    }

    let start_message = format!("Running LLM triage over {total} file(s) to prioritize analysis.");
    push_progress_log(
        &progress_sender,
        &log_sink,
        &mut logs,
        start_message.clone(),
    );

    let scope_prompt = scope_prompt
        .map(|prompt| prompt.trim().to_string())
        .filter(|prompt| !prompt.is_empty())
        .map(Arc::<str>::from);
    if let Some(prompt) = scope_prompt.as_deref() {
        let summarized = truncate_text(prompt, MODEL_REASONING_LOG_MAX_GRAPHEMES);
        push_progress_log(
            &progress_sender,
            &log_sink,
            &mut logs,
            format!("Guiding file triage with user prompt: {summarized}"),
        );
    }

    let chunk_requests: Vec<FileTriageChunkRequest> = snippets
        .iter()
        .enumerate()
        .collect::<Vec<_>>()
        .chunks(FILE_TRIAGE_CHUNK_SIZE)
        .map(|chunk| {
            let start_idx = chunk.first().map(|(idx, _)| *idx).unwrap_or(0);
            let end_idx = chunk.last().map(|(idx, _)| *idx).unwrap_or(start_idx);
            let descriptors = chunk
                .iter()
                .map(|(idx, snippet)| {
                    let preview = snippet
                        .content
                        .chars()
                        .filter(|c| *c == '\n' || *c == '\r' || *c == '\t' || !c.is_control())
                        .take(400)
                        .collect::<String>();
                    let descriptor = json!({
                        "id": idx,
                        "path": snippet.relative_path.display().to_string(),
                        "language": snippet.language,
                        "bytes": snippet.bytes,
                        "preview": preview,
                    });
                    FileTriageDescriptor {
                        id: *idx,
                        path: snippet.relative_path.display().to_string(),
                        listing_json: descriptor.to_string(),
                    }
                })
                .collect();
            FileTriageChunkRequest {
                start_idx,
                end_idx,
                descriptors,
            }
        })
        .collect();

    let mut include_ids: HashSet<usize> = HashSet::new();
    let mut aggregated_logs: Vec<String> = Vec::new();
    let mut processed_files: usize = 0;

    let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
    let mut remaining = chunk_requests.into_iter();
    let total_chunks = total.div_ceil(FILE_TRIAGE_CHUNK_SIZE.max(1));
    let concurrency = FILE_TRIAGE_CONCURRENCY.min(total_chunks.max(1));

    // Emit a brief, on-screen preview of the parallel task and sample tool calls.
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(format!(
            "  └ Launching parallel file triage ({concurrency} workers)"
        )));
    }

    for _ in 0..concurrency {
        if let Some(request) = remaining.next() {
            in_flight.push(triage_chunk(
                client,
                provider.clone(),
                auth.clone(),
                triage_model.to_string(),
                scope_prompt.clone(),
                request,
                progress_sender.clone(),
                log_sink.clone(),
                total,
                metrics.clone(),
            ));
        }
    }

    while let Some(result) = in_flight.next().await {
        match result {
            Ok(chunk_result) => {
                aggregated_logs.extend(chunk_result.logs);
                include_ids.extend(chunk_result.include_ids);
                processed_files = processed_files.saturating_add(chunk_result.processed);
                if let Some(tx) = progress_sender.as_ref() {
                    let percent = if total == 0 {
                        0
                    } else {
                        (processed_files * 100) / total
                    };
                    tx.send(AppEvent::SecurityReviewLog(format!(
                        "File triage progress: {}/{} - {percent}%.",
                        processed_files.min(total),
                        total
                    )));
                }
                if let Some(next_request) = remaining.next() {
                    in_flight.push(triage_chunk(
                        client,
                        provider.clone(),
                        auth.clone(),
                        triage_model.to_string(),
                        scope_prompt.clone(),
                        next_request,
                        progress_sender.clone(),
                        log_sink.clone(),
                        total,
                        metrics.clone(),
                    ));
                }
            }
            Err(mut failure) => {
                logs.append(&mut failure.logs);
                return Err(SecurityReviewFailure {
                    message: failure.message,
                    logs,
                });
            }
        }
    }

    logs.extend(aggregated_logs);

    if include_ids.is_empty() {
        append_log(
            &log_sink,
            &mut logs,
            "LLM triage excluded all files.".to_string(),
        );
        return Ok(FileTriageResult {
            included: Vec::new(),
            logs,
        });
    }

    let mut included = Vec::with_capacity(include_ids.len());
    for (idx, snippet) in snippets.into_iter().enumerate() {
        if include_ids.contains(&idx) {
            included.push(snippet);
        }
    }

    append_log(
        &log_sink,
        &mut logs,
        format!(
            "File triage selected {} of {} files (excluded {}).",
            included.len(),
            total,
            total.saturating_sub(included.len())
        ),
    );

    Ok(FileTriageResult { included, logs })
}

fn build_file_triage_prompt(listing: &str, scope_prompt: Option<&str>) -> String {
    let base = FILE_TRIAGE_PROMPT_TEMPLATE.replace("{files}", listing);
    if let Some(prompt) = scope_prompt {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return base;
        }
        return format!(
            "{base}\n\nUser scope prompt:\n{trimmed}\nUse this to keep files that align with the request and skip those that do not."
        );
    }
    base
}

#[allow(clippy::too_many_arguments)]
async fn triage_chunk(
    client: &CodexHttpClient,
    provider: ModelProviderInfo,
    auth: Option<CodexAuth>,
    triage_model: String,
    scope_prompt: Option<Arc<str>>,
    request: FileTriageChunkRequest,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    total_files: usize,
    metrics: Arc<ReviewMetrics>,
) -> Result<FileTriageChunkResult, SecurityReviewFailure> {
    let listing = request
        .descriptors
        .iter()
        .map(|desc| desc.listing_json.as_str())
        .collect::<Vec<_>>()
        .join(
            "
",
        );

    // Show the file range being triaged; overall % is reported by the parent loop.
    let detail = format!(
        "files {}-{} of {}",
        request.start_idx + 1,
        request.end_idx + 1,
        total_files
    );

    let prompt = build_file_triage_prompt(&listing, scope_prompt.as_deref());
    let response = await_with_heartbeat(
        progress_sender.clone(),
        "running file triage",
        Some(detail.as_str()),
        call_model(
            client,
            &provider,
            &auth,
            &triage_model,
            FILE_TRIAGE_SYSTEM_PROMPT,
            &prompt,
            metrics.clone(),
            0.0,
        ),
    )
    .await;

    let response_output = match response {
        Ok(output) => output,
        Err(err) => {
            let message = format!("File triage failed: {err}");
            let mut failure_logs = Vec::new();
            push_progress_log(
                &progress_sender,
                &log_sink,
                &mut failure_logs,
                message.clone(),
            );
            return Err(SecurityReviewFailure {
                message,
                logs: failure_logs,
            });
        }
    };
    let mut chunk_logs = Vec::new();
    if let Some(reasoning) = response_output.reasoning.as_ref() {
        log_model_reasoning(reasoning, &progress_sender, &log_sink, &mut chunk_logs);
    }
    let text = response_output.text;
    let mut include_ids: Vec<usize> = Vec::new();
    let mut parsed_any = false;
    let path_by_id: HashMap<usize, &str> = request
        .descriptors
        .iter()
        .map(|d| (d.id, d.path.as_str()))
        .collect();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(id) = parsed
            .get("id")
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as usize)
        else {
            continue;
        };
        let Some(path) = path_by_id.get(&id) else {
            continue;
        };
        parsed_any = true;
        let include = parsed
            .get("include")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let reason = parsed
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if include {
            include_ids.push(id);
            if !reason.is_empty() {
                let message = format!("Triage kept {path} — {reason}");
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(message.clone()));
                }
                chunk_logs.push(message);
            }
        } else if !reason.is_empty() {
            let message = format!("Triage skipped {path} — {reason}");
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(message.clone()));
            }
            chunk_logs.push(message);
        }
    }

    if !parsed_any {
        let message = format!(
            "Triage returned no structured output for files {}-{}; including all by default.",
            request.start_idx + 1,
            request.end_idx + 1
        );
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(message.clone()));
        }
        chunk_logs.push(message);
        include_ids.extend(request.descriptors.iter().map(|d| d.id));
    }

    chunk_logs.push(format!(
        "Triage kept {}/{} files for indices {}-{}.",
        include_ids.len(),
        request.descriptors.len(),
        request.start_idx + 1,
        request.end_idx + 1
    ));

    Ok(FileTriageChunkResult {
        include_ids,
        logs: chunk_logs,
        processed: request.descriptors.len(),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SpecGenerationProgress {
    version: i32,
    repo_root: String,
    targets: Vec<SpecGenerationProgressTarget>,
    completed: Vec<SpecGenerationProgressCompleted>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SpecGenerationProgressTarget {
    target_path: String,
    location_label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SpecGenerationProgressCompleted {
    target_path: String,
    location_label: String,
    raw_path: String,
}

impl SpecGenerationProgress {
    fn new(repo_root: &Path) -> Self {
        Self {
            version: 1,
            repo_root: repo_root.display().to_string(),
            targets: Vec::new(),
            completed: Vec::new(),
        }
    }

    fn upsert_completed(&mut self, entry: SpecGenerationProgressCompleted) {
        if let Some(existing) = self
            .completed
            .iter_mut()
            .find(|existing| existing.target_path == entry.target_path)
        {
            *existing = entry;
            return;
        }
        self.completed.push(entry);
    }
}

fn encode_progress_path(path: &Path, repo_root: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn decode_progress_path(encoded: &str, repo_root: &Path) -> PathBuf {
    let path = PathBuf::from(encoded);
    if path.is_absolute() {
        path
    } else {
        repo_root.join(path)
    }
}

async fn read_spec_generation_progress(
    progress_path: &Path,
    repo_root: &Path,
) -> Result<Option<SpecGenerationProgress>, SecurityReviewFailure> {
    if !progress_path.exists() {
        return Ok(None);
    }
    let bytes = tokio_fs::read(progress_path)
        .await
        .map_err(|err| SecurityReviewFailure {
            message: format!(
                "Failed to read spec generation progress {}: {err}",
                progress_path.display()
            ),
            logs: vec![format!(
                "Failed to read spec generation progress {}: {err}",
                progress_path.display()
            )],
        })?;
    let progress = serde_json::from_slice::<SpecGenerationProgress>(&bytes).map_err(|err| {
        SecurityReviewFailure {
            message: format!(
                "Failed to parse spec generation progress {}: {err}",
                progress_path.display()
            ),
            logs: vec![format!(
                "Failed to parse spec generation progress {}: {err}",
                progress_path.display()
            )],
        }
    })?;
    if progress.version != 1 || progress.repo_root != repo_root.display().to_string() {
        return Ok(None);
    }
    Ok(Some(progress))
}

async fn write_spec_generation_progress(
    progress_path: &Path,
    progress: &SpecGenerationProgress,
) -> Result<(), SecurityReviewFailure> {
    if let Some(parent) = progress_path.parent() {
        tokio_fs::create_dir_all(parent)
            .await
            .map_err(|err| SecurityReviewFailure {
                message: format!(
                    "Failed to create directory for spec progress {}: {err}",
                    parent.display()
                ),
                logs: vec![format!(
                    "Failed to create directory for spec progress {}: {err}",
                    parent.display()
                )],
            })?;
    }

    let bytes = serde_json::to_vec_pretty(progress).map_err(|err| SecurityReviewFailure {
        message: format!(
            "Failed to serialize spec generation progress {}: {err}",
            progress_path.display()
        ),
        logs: vec![format!(
            "Failed to serialize spec generation progress {}: {err}",
            progress_path.display()
        )],
    })?;

    let file_name = progress_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("spec_generation_progress.json");
    let tmp_path = progress_path.with_file_name(format!("{file_name}.tmp"));
    let _ = tokio_fs::remove_file(&tmp_path).await;
    tokio_fs::write(&tmp_path, bytes)
        .await
        .map_err(|err| SecurityReviewFailure {
            message: format!(
                "Failed to write spec generation progress {}: {err}",
                tmp_path.display()
            ),
            logs: vec![format!(
                "Failed to write spec generation progress {}: {err}",
                tmp_path.display()
            )],
        })?;

    match tokio_fs::rename(&tmp_path, progress_path).await {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = tokio_fs::remove_file(progress_path).await;
            tokio_fs::rename(&tmp_path, progress_path)
                .await
                .map_err(|second_err| SecurityReviewFailure {
                    message: format!(
                        "Failed to replace spec generation progress {}: {err}; retry failed: {second_err}",
                        progress_path.display()
                    ),
                    logs: vec![format!(
                        "Failed to replace spec generation progress {}: {err}; retry failed: {second_err}",
                        progress_path.display()
                    )],
                })?;
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn generate_specs(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    repo_root: &Path,
    include_paths: &[PathBuf],
    output_root: &Path,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    config: &Config,
    auth_manager: Arc<AuthManager>,
) -> Result<Option<SpecGenerationOutcome>, SecurityReviewFailure> {
    let spec_progress_path = output_root
        .join("context")
        .join("spec_generation_progress.json");
    let mut targets: Vec<PathBuf> = if include_paths.is_empty() {
        vec![repo_root.to_path_buf()]
    } else {
        include_paths.to_vec()
    };

    if targets.is_empty() {
        return Ok(None);
    }

    let mut logs: Vec<String> = Vec::new();

    let (normalized, spec_progress) =
        match read_spec_generation_progress(&spec_progress_path, repo_root).await {
            Ok(Some(progress)) => {
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(format!(
                        "Loaded spec generation progress from {}; resuming.",
                        spec_progress_path.display()
                    )));
                }
                logs.push(format!(
                    "Loaded spec generation progress from {}; resuming.",
                    spec_progress_path.display()
                ));
                let targets = progress
                    .targets
                    .iter()
                    .map(|target| decode_progress_path(&target.target_path, repo_root))
                    .collect::<Vec<_>>();
                (targets, progress)
            }
            Ok(None) => {
                let mut seen: HashSet<String> = HashSet::new();
                let mut normalized: Vec<PathBuf> = Vec::new();
                for target in targets.drain(..) {
                    let mut path = target.clone();
                    if path.is_file()
                        && let Some(parent) = path.parent()
                    {
                        path = parent.to_path_buf();
                    }
                    if !path.exists() {
                        continue;
                    }
                    let key = path.to_string_lossy().to_string();
                    if seen.insert(key) {
                        normalized.push(path);
                    }
                }

                if normalized.is_empty() {
                    return Ok(None);
                }

                (normalized, SpecGenerationProgress::new(repo_root))
            }
            Err(err) => {
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(err.message.clone()));
                }
                logs.push(err.message.clone());
                logs.extend(err.logs);

                let mut seen: HashSet<String> = HashSet::new();
                let mut normalized: Vec<PathBuf> = Vec::new();
                for target in targets.drain(..) {
                    let mut path = target.clone();
                    if path.is_file()
                        && let Some(parent) = path.parent()
                    {
                        path = parent.to_path_buf();
                    }
                    if !path.exists() {
                        continue;
                    }
                    let key = path.to_string_lossy().to_string();
                    if seen.insert(key) {
                        normalized.push(path);
                    }
                }

                if normalized.is_empty() {
                    return Ok(None);
                }

                (normalized, SpecGenerationProgress::new(repo_root))
            }
        };

    let mut spec_progress_state = spec_progress;

    let mut directory_candidates: Vec<(PathBuf, String)> = if spec_progress_state.targets.is_empty()
    {
        normalized
            .into_iter()
            .map(|path| {
                let label = display_path_for(&path, repo_root);
                (path, label)
            })
            .collect()
    } else {
        spec_progress_state
            .targets
            .iter()
            .map(|target| {
                (
                    decode_progress_path(&target.target_path, repo_root),
                    target.location_label.clone(),
                )
            })
            .collect()
    };
    directory_candidates.sort_by(|a, b| a.1.cmp(&b.1));

    let heuristically_filtered: Vec<(PathBuf, String)> =
        if spec_progress_state.targets.is_empty() {
            let mut filtered: Vec<(PathBuf, String)> = Vec::new();
            for (path, label) in &directory_candidates {
                if is_spec_dir_likely_low_signal(path) {
                    if let Some(tx) = progress_sender.as_ref() {
                        tx.send(AppEvent::SecurityReviewLog(format!(
                            "Heuristic skip for spec dir {label} (tests/utils/scripts)."
                        )));
                    }
                } else {
                    filtered.push((path.clone(), label.clone()));
                }
            }
            if filtered.is_empty() {
                directory_candidates.clone()
            } else {
                filtered
            }
        } else {
            directory_candidates.clone()
        };

    let mut filtered_dirs = if spec_progress_state.targets.is_empty() {
        match filter_spec_directories(
            client,
            provider,
            auth,
            repo_root,
            &heuristically_filtered,
            metrics.clone(),
        )
        .await
        {
            Ok(result) => result,
            Err(err) => {
                if let Some(tx) = progress_sender.as_ref() {
                    for line in &err.logs {
                        tx.send(AppEvent::SecurityReviewLog(line.clone()));
                    }
                }
                logs.extend(err.logs);
                let message = format!(
                    "Directory filter failed; using all directories. {}",
                    err.message
                );
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(message.clone()));
                }
                logs.push(message);
                directory_candidates.clone()
            }
        }
    } else {
        directory_candidates.clone()
    };

    if spec_progress_state.targets.is_empty() {
        let (preferred_dirs, dropped) = prune_low_signal_spec_dirs(&filtered_dirs);
        if !preferred_dirs.is_empty() {
            if let Some(tx) = progress_sender.as_ref() {
                for label in &dropped {
                    tx.send(AppEvent::SecurityReviewLog(format!(
                        "Skipping specification for {label} (low-signal helper/migration dir)."
                    )));
                }
            }
            for label in &dropped {
                logs.push(format!(
                    "Skipping specification for {label} (low-signal helper/migration dir)."
                ));
            }
            filtered_dirs = preferred_dirs;
        }
    }

    let mut spec_targets: Vec<(PathBuf, String)> = if filtered_dirs.is_empty() {
        directory_candidates.clone()
    } else {
        filtered_dirs.clone()
    };
    spec_targets.sort_by(|a, b| a.1.cmp(&b.1));

    if let Some(tx) = progress_sender.as_ref() {
        let kept = spec_targets.len();
        let total = directory_candidates.len();
        let message = if spec_progress_state.targets.is_empty() {
            format!(
                "Spec directory filter kept {kept}/{total} directories using {SPEC_GENERATION_MODEL}."
            )
        } else {
            format!("Resuming specification generation for {kept} directory(s).")
        };
        tx.send(AppEvent::SecurityReviewLog(message.clone()));
        logs.push(message);
    }

    let display_locations: Vec<String> = spec_targets
        .iter()
        .map(|(_, label)| label.clone())
        .collect();

    if spec_progress_state.targets.is_empty() {
        spec_progress_state.targets = spec_targets
            .iter()
            .map(|(path, label)| SpecGenerationProgressTarget {
                target_path: encode_progress_path(path, repo_root),
                location_label: label.clone(),
            })
            .collect();
        write_spec_generation_progress(&spec_progress_path, &spec_progress_state).await?;
    }

    let specs_root = output_root.join("specs");
    let raw_dir = specs_root.join("raw");
    let combined_dir = specs_root.join("combined");
    let apis_dir = specs_root.join("apis");

    tokio_fs::create_dir_all(&raw_dir)
        .await
        .map_err(|e| SecurityReviewFailure {
            message: format!("Failed to create {}: {e}", raw_dir.display()),
            logs: Vec::new(),
        })?;
    tokio_fs::create_dir_all(&combined_dir)
        .await
        .map_err(|e| SecurityReviewFailure {
            message: format!("Failed to create {}: {e}", combined_dir.display()),
            logs: Vec::new(),
        })?;
    tokio_fs::create_dir_all(&apis_dir)
        .await
        .map_err(|e| SecurityReviewFailure {
            message: format!("Failed to create {}: {e}", apis_dir.display()),
            logs: Vec::new(),
        })?;

    let mut completed_targets: HashSet<String> = HashSet::new();
    let mut spec_entries: Vec<SpecEntry> = Vec::new();
    for completed in &spec_progress_state.completed {
        let raw_path = PathBuf::from(&completed.raw_path);
        match tokio_fs::read_to_string(&raw_path).await {
            Ok(markdown) => {
                let api_markdown = extract_api_markdown(&markdown);
                spec_entries.push(SpecEntry {
                    location_label: completed.location_label.clone(),
                    markdown,
                    raw_path: raw_path.clone(),
                    api_markdown,
                });
                completed_targets.insert(completed.target_path.clone());
            }
            Err(err) => {
                logs.push(format!(
                    "Failed to read saved specification {} for {}: {err}",
                    raw_path.display(),
                    completed.location_label
                ));
            }
        }
    }

    let mut pending_targets: Vec<PathBuf> = Vec::new();
    for (path, _) in &spec_targets {
        let key = encode_progress_path(path, repo_root);
        if !completed_targets.contains(&key) {
            pending_targets.push(path.clone());
        }
    }

    let (mut new_entries, mut generation_logs) = match generate_specs_parallel_workers(
        client,
        provider,
        auth,
        repo_root,
        &pending_targets,
        &display_locations,
        &raw_dir,
        progress_sender.clone(),
        metrics.clone(),
        &spec_progress_path,
        &mut spec_progress_state,
        config,
        auth_manager.clone(),
        log_sink.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(mut failure) => {
            logs.append(&mut failure.logs);
            return Err(SecurityReviewFailure {
                message: failure.message,
                logs,
            });
        }
    };
    logs.append(&mut generation_logs);
    spec_entries.append(&mut new_entries);

    if spec_entries.is_empty() {
        logs.push("No specifications were generated.".to_string());
        return Ok(None);
    }

    let mut api_entries: Vec<ApiEntry> = Vec::new();
    for entry in spec_entries.iter_mut() {
        let location_label = entry.location_label.clone();
        if let Some(markdown) = entry
            .api_markdown
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
        {
            let slug = slugify_label(&location_label);
            let api_path = apis_dir.join(format!("{slug}_apis.md"));
            match tokio_fs::write(&api_path, markdown.as_bytes()).await {
                Ok(()) => {
                    let msg = format!(
                        "API entry points for {location_label} saved to {}.",
                        api_path.display()
                    );
                    if let Some(tx) = progress_sender.as_ref() {
                        tx.send(AppEvent::SecurityReviewLog(msg.clone()));
                    }
                    logs.push(msg);
                    api_entries.push(ApiEntry {
                        location_label,
                        markdown,
                    });
                }
                Err(err) => {
                    let msg = format!(
                        "Failed to write API entry points for {location_label} to {}: {err}",
                        api_path.display()
                    );
                    if let Some(tx) = progress_sender.as_ref() {
                        tx.send(AppEvent::SecurityReviewLog(msg.clone()));
                    }
                    logs.push(msg);
                }
            }
        } else {
            let msg =
                format!("Specification for {location_label} did not include API entry points.");
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(msg.clone()));
            }
            logs.push(msg);
        }
    }

    let combined_path = combined_dir.join("combined_specification.md");
    let (mut combined_markdown, mut combine_logs) = combine_spec_markdown(
        client,
        provider,
        auth,
        &display_locations,
        &spec_entries,
        &combined_path,
        repo_root,
        progress_sender.clone(),
        log_sink.clone(),
        metrics.clone(),
    )
    .await?;
    logs.append(&mut combine_logs);

    let mut classification_rows: Vec<DataClassificationRow> = Vec::new();
    let mut classification_table: Option<String> = None;
    match extract_data_classification(client, provider, auth, &combined_markdown, metrics.clone())
        .await
    {
        Ok(Some(extraction)) => {
            for line in extraction.reasoning_logs {
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(line.clone()));
                }
                logs.push(line);
            }
            classification_rows = extraction.rows.clone();
            classification_table = Some(extraction.table_markdown.clone());
            let injected =
                inject_data_classification_section(&combined_markdown, &extraction.table_markdown);
            combined_markdown = fix_mermaid_blocks(&injected);
            let msg = format!(
                "Injected data classification table with {} entr{} into combined specification.",
                extraction.rows.len(),
                if extraction.rows.len() == 1 {
                    "y"
                } else {
                    "ies"
                }
            );
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(msg.clone()));
            }
            logs.push(msg);
            if let Err(err) = tokio_fs::write(&combined_path, combined_markdown.as_bytes()).await {
                let warn = format!(
                    "Failed to update combined specification with data classification table: {err}"
                );
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(warn.clone()));
                }
                logs.push(warn);
            }
        }
        Ok(None) => {
            let msg = "Data classification extraction produced no entries.".to_string();
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(msg.clone()));
            }
            logs.push(msg);
        }
        Err(err) => {
            let msg = format!("Failed to extract data classification: {err}");
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(msg.clone()));
            }
            logs.push(msg);
        }
    }

    write_testing_instructions(
        repo_root,
        &specs_root,
        progress_sender.clone(),
        log_sink.clone(),
    )
    .await;

    Ok(Some(SpecGenerationOutcome {
        combined_markdown,
        locations: display_locations,
        logs,
        api_entries,
        classification_rows,
        classification_table,
    }))
}

fn is_auto_scope_excluded_dir(name: &str) -> bool {
    EXCLUDED_DIR_NAMES
        .iter()
        .any(|excluded| excluded.eq_ignore_ascii_case(name))
}

fn is_auto_scope_marker(name: &str) -> bool {
    AUTO_SCOPE_MARKER_FILES
        .iter()
        .any(|marker| marker.eq_ignore_ascii_case(name))
}

fn normalize_keyword_candidate(candidate: &str) -> Option<(String, String)> {
    let trimmed = candidate
        .trim()
        .trim_matches(|c: char| c == '"' || c == '\'')
        .trim();
    if trimmed.is_empty() {
        return None;
    }
    let cleaned = trimmed
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<&str>>()
        .join(" ");
    if cleaned.is_empty() {
        return None;
    }
    let lowercase = cleaned.to_ascii_lowercase();
    if lowercase.len() <= 1 {
        return None;
    }
    if AUTO_SCOPE_KEYWORD_STOPWORDS
        .iter()
        .any(|stop| lowercase == *stop)
    {
        return None;
    }
    Some((cleaned, lowercase))
}

fn extract_keywords_from_value(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => output.push(text.to_string()),
        Value::Array(items) => {
            for item in items {
                extract_keywords_from_value(item, output);
            }
        }
        Value::Object(map) => {
            for key in ["keyword", "keywords", "term", "value", "name"] {
                if let Some(val) = map.get(key) {
                    extract_keywords_from_value(val, output);
                }
            }
        }
        _ => {}
    }
}

fn parse_keyword_response(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        let mut collected = Vec::new();
        extract_keywords_from_value(&value, &mut collected);
        if !collected.is_empty() {
            return collected;
        }
    }

    let mut collected: Vec<String> = Vec::new();
    for line in trimmed.lines() {
        let stripped = line.trim().trim_start_matches(['-', '*', '•']).trim();
        if stripped.is_empty() || stripped.eq_ignore_ascii_case("none") {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(stripped) {
            extract_keywords_from_value(&value, &mut collected);
            continue;
        }
        for fragment in stripped.split([',', ';', '/']) {
            let fragment_trimmed = fragment.trim();
            if !fragment_trimmed.is_empty() {
                collected.push(fragment_trimmed.to_string());
            }
        }
    }
    collected
}

fn fallback_keywords_from_prompt(user_query: &str) -> Vec<String> {
    let mut keywords = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for token in user_query.split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-') {
        if token.is_empty() {
            continue;
        }
        let normalized = token.to_ascii_lowercase();
        if normalized.len() <= 2 {
            continue;
        }
        if AUTO_SCOPE_KEYWORD_STOPWORDS
            .iter()
            .any(|stop| normalized == *stop)
        {
            continue;
        }
        if seen.insert(normalized) {
            keywords.push(token.to_string());
            if keywords.len() >= AUTO_SCOPE_MAX_KEYWORDS {
                break;
            }
        }
    }
    keywords
}

async fn expand_auto_scope_keywords(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    user_query: &str,
    metrics: Arc<ReviewMetrics>,
) -> Result<Vec<String>, String> {
    let trimmed_query = truncate_text(user_query, 600);
    if trimmed_query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let fallback_keyword = fallback_keywords_from_prompt(&trimmed_query)
        .into_iter()
        .next()
        .unwrap_or_else(|| trimmed_query.clone());

    let prompt = AUTO_SCOPE_KEYWORD_PROMPT_TEMPLATE
        .replace("{user_query}", &trimmed_query)
        .replace("{max_keywords}", &AUTO_SCOPE_MAX_KEYWORDS.to_string())
        .replace("{fallback_keyword}", &fallback_keyword);

    let response = call_model(
        client,
        provider,
        auth,
        AUTO_SCOPE_MODEL,
        AUTO_SCOPE_KEYWORD_SYSTEM_PROMPT,
        &prompt,
        metrics.clone(),
        0.0,
    )
    .await
    .map_err(|err| format!("keyword expansion model call failed: {err}"))?;

    let raw_candidates = parse_keyword_response(&response.text);
    let mut keywords: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for candidate in raw_candidates {
        if let Some((display, key)) = normalize_keyword_candidate(&candidate)
            && seen.insert(key)
        {
            keywords.push(display);
            if keywords.len() >= AUTO_SCOPE_MAX_KEYWORDS {
                break;
            }
        }
    }
    Ok(keywords)
}

#[derive(Debug, Clone)]
struct RawAutoScopeSelection {
    path: String,
    reason: Option<String>,
}

enum AutoScopeParseResult {
    All,
    Selections(Vec<RawAutoScopeSelection>),
}

fn parse_include_flag(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "yes" | "y" | "include" | "1" => Some(true),
                "false" | "no" | "n" | "exclude" | "0" => Some(false),
                _ => None,
            }
        }
        Value::Number(number) => {
            if let Some(as_int) = number.as_i64() {
                return Some(as_int != 0);
            }
            number.as_f64().map(|value| value != 0.0)
        }
        _ => None,
    }
}

fn parse_raw_auto_scope_selection(map: &Map<String, Value>) -> Option<RawAutoScopeSelection> {
    // Be tolerant: default to include=true when the model omits the flag.
    // Older prompts mandated an explicit include flag, but newer responses
    // often omit it when the intent is clearly to include the path.
    let include = map
        .get("include")
        .and_then(parse_include_flag)
        .unwrap_or(true);
    if !include {
        return None;
    }

    let raw_path = map
        .get("path")
        .or_else(|| map.get("dir"))
        .or_else(|| map.get("directory"))
        .and_then(|value| value.as_str().map(str::trim))
        .filter(|value| !value.is_empty())?;

    let reason = map.get("reason").and_then(|value| match value {
        Value::Null => None,
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        other => {
            let rendered = other.to_string();
            (!rendered.is_empty()).then_some(rendered)
        }
    });

    Some(RawAutoScopeSelection {
        path: raw_path.to_string(),
        reason,
    })
}

fn collect_auto_scope_values(value: &Value, output: &mut Vec<RawAutoScopeSelection>) -> bool {
    match value {
        Value::String(text) => text.trim().eq_ignore_ascii_case("all"),
        Value::Array(items) => {
            let mut include_all = false;
            for item in items {
                if collect_auto_scope_values(item, output) {
                    include_all = true;
                }
            }
            include_all
        }
        Value::Object(map) => {
            if let Some(selection) = parse_raw_auto_scope_selection(map) {
                output.push(selection);
            }
            let mut include_all = false;
            for (key, item) in map {
                if matches!(
                    key.as_str(),
                    "path" | "dir" | "directory" | "reason" | "include"
                ) {
                    continue;
                }
                if collect_auto_scope_values(item, output) {
                    include_all = true;
                }
            }
            include_all
        }
        _ => false,
    }
}

fn extract_json_objects(raw: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut start: Option<usize> = None;
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escape = false;

    for (index, ch) in raw.char_indices() {
        if let Some(begin) = start {
            if in_string {
                if escape {
                    escape = false;
                } else if ch == '\\' {
                    escape = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    if depth == 0 {
                        let end = index + ch.len_utf8();
                        result.push(raw[begin..end].to_string());
                        start = None;
                    } else {
                        depth -= 1;
                    }
                }
                _ => {}
            }
        } else if ch == '{' {
            start = Some(index);
            depth = 0;
            in_string = false;
            escape = false;
        }
    }

    result
}

fn parse_auto_scope_response(raw: &str) -> AutoScopeParseResult {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return AutoScopeParseResult::Selections(Vec::new());
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        let mut selections: Vec<RawAutoScopeSelection> = Vec::new();
        let include_all = collect_auto_scope_values(&value, &mut selections);
        if include_all && selections.is_empty() {
            return AutoScopeParseResult::All;
        }
        return AutoScopeParseResult::Selections(selections);
    }

    let mut selections: Vec<RawAutoScopeSelection> = Vec::new();
    let mut include_all = false;
    for snippet in extract_json_objects(trimmed) {
        if let Ok(value) = serde_json::from_str::<Value>(&snippet)
            && collect_auto_scope_values(&value, &mut selections)
        {
            include_all = true;
        }
    }

    if selections.is_empty()
        && (include_all
            || trimmed
                .lines()
                .any(|line| line.trim().eq_ignore_ascii_case("all")))
    {
        AutoScopeParseResult::All
    } else {
        AutoScopeParseResult::Selections(selections)
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_auto_scope_agent(
    config: &Config,
    provider: &ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    repo_root: &Path,
    prompt: String,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
    model: &str,
) -> Result<(String, Vec<String>), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();

    let mut auto_config = config.clone();
    auto_config.model = model.to_string();
    auto_config.model_provider = provider.clone();
    auto_config.base_instructions = Some(AUTO_SCOPE_SYSTEM_PROMPT.to_string());
    auto_config.user_instructions = None;
    auto_config.developer_instructions = None;
    auto_config.compact_prompt = None;
    auto_config.cwd = repo_root.to_path_buf();
    auto_config
        .features
        .disable(Feature::ApplyPatchFreeform)
        .disable(Feature::WebSearchRequest)
        .disable(Feature::ViewImageTool)
        .disable(Feature::RmcpClient);
    auto_config.mcp_servers.clear();
    auto_config.use_experimental_use_rmcp_client = false;

    let manager = ConversationManager::new(
        auth_manager,
        SessionSource::SubAgent(SubAgentSource::Other(
            "security_review_auto_scope".to_string(),
        )),
    );

    let conversation = match manager.new_conversation(auto_config).await {
        Ok(new_conversation) => new_conversation.conversation,
        Err(err) => {
            let message = format!("Failed to start auto-scope agent: {err}");
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };

    if let Err(err) = conversation
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: prompt }],
        })
        .await
    {
        let message = format!("Failed to submit auto-scope prompt: {err}");
        push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
        return Err(SecurityReviewFailure { message, logs });
    }

    let mut last_agent_message: Option<String> = None;

    loop {
        let event = match conversation.next_event().await {
            Ok(event) => event,
            Err(err) => {
                let message = format!("Auto-scope agent terminated unexpectedly: {err}");
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
        };

        record_tool_call_from_event(metrics.as_ref(), &event.msg);

        match event.msg {
            EventMsg::TaskComplete(done) => {
                if let Some(msg) = done.last_agent_message {
                    last_agent_message = Some(msg);
                }
                break;
            }
            EventMsg::AgentMessage(msg) => {
                last_agent_message = Some(msg.message.clone());
            }
            EventMsg::AgentReasoning(reason) => {
                log_model_reasoning(&reason.text, &progress_sender, &log_sink, &mut logs);
            }
            EventMsg::Warning(warn) => {
                push_progress_log(&progress_sender, &log_sink, &mut logs, warn.message);
            }
            EventMsg::Error(err) => {
                let message = format!("Auto-scope agent error: {}", err.message);
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
            EventMsg::TurnAborted(aborted) => {
                let message = format!("Auto-scope agent aborted: {:?}", aborted.reason);
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
            EventMsg::TokenCount(count) => {
                if let Some(info) = count.info {
                    metrics.record_model_call();
                    metrics.record_usage(&info.last_token_usage);
                }
            }
            _ => {}
        }
    }

    let response = match last_agent_message.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }) {
        Some(text) => text,
        None => {
            let message = "Auto-scope agent produced an empty response.".to_string();
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };

    let _ = conversation.submit(Op::Shutdown).await;

    Ok((response, logs))
}

#[allow(clippy::too_many_arguments)]
async fn auto_detect_scope(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    repo_root: &Path,
    user_query: &str,
    conversation: &str,
    metrics: Arc<ReviewMetrics>,
    config: &Config,
    auth_manager: Arc<AuthManager>,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
) -> Result<(Vec<AutoScopeSelection>, Vec<String>), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();

    let mut keywords =
        match expand_auto_scope_keywords(client, provider, auth, user_query, metrics.clone()).await
        {
            Ok(values) => {
                if values.is_empty() {
                    logs.push(
                        "Auto scope keyword expansion returned no keywords; using fallback terms."
                            .to_string(),
                    );
                } else {
                    logs.push(format!(
                        "Auto scope keywords suggested by model: {}",
                        values.join(", ")
                    ));
                }
                values
            }
            Err(err) => {
                logs.push(format!("Auto scope keyword expansion failed: {err}"));
                Vec::new()
            }
        };

    if keywords.is_empty() {
        let fallback = fallback_keywords_from_prompt(user_query);
        if fallback.is_empty() {
            logs.push(
                "Auto scope keyword fallback produced no usable tokens; continuing with raw prompt."
                    .to_string(),
            );
        } else {
            logs.push(format!(
                "Auto scope fallback keywords derived from prompt: {}",
                fallback.join(", ")
            ));
            keywords = fallback;
        }
    }

    let repo_overview = summarize_top_level(repo_root);
    let prompt = build_auto_scope_prompt(&repo_overview, user_query, &keywords, conversation);

    let (assistant_reply, mut agent_logs) = run_auto_scope_agent(
        config,
        provider,
        auth_manager,
        repo_root,
        prompt,
        progress_sender,
        log_sink,
        metrics,
        model,
    )
    .await?;
    logs.append(&mut agent_logs);

    let assistant_reply = assistant_reply.trim();
    let parse_result = parse_auto_scope_response(assistant_reply);
    match parse_result {
        AutoScopeParseResult::All => {
            let canonical = repo_root
                .canonicalize()
                .unwrap_or_else(|_| repo_root.to_path_buf());
            logs.push("Auto scope model requested the entire repository.".to_string());
            Ok((
                vec![AutoScopeSelection {
                    display_path: display_path_for(&canonical, repo_root),
                    abs_path: canonical,
                    reason: Some("LLM requested full repository".to_string()),
                    is_dir: true,
                }],
                logs,
            ))
        }
        AutoScopeParseResult::Selections(raw_selections) => {
            if raw_selections.is_empty() {
                logs.push(
                    "Auto scope model returned no included directories in the final response."
                        .to_string(),
                );
                return Err(SecurityReviewFailure {
                    message: "Auto scope returned no directories.".to_string(),
                    logs,
                });
            }

            let mut seen: HashSet<PathBuf> = HashSet::new();
            let mut selections: Vec<AutoScopeSelection> = Vec::new();

            for raw in raw_selections {
                let mut candidate = PathBuf::from(&raw.path);
                if !candidate.is_absolute() {
                    candidate = repo_root.join(&candidate);
                }
                let canonical = match candidate.canonicalize() {
                    Ok(path) => path,
                    Err(_) => continue,
                };
                if !canonical.starts_with(repo_root) {
                    continue;
                }
                let metadata = match fs::metadata(&canonical) {
                    Ok(metadata) => metadata,
                    Err(_) => continue,
                };
                if !(metadata.is_dir() || metadata.is_file()) {
                    continue;
                }
                let is_dir = metadata.is_dir();
                if !seen.insert(canonical.clone()) {
                    continue;
                }
                selections.push(AutoScopeSelection {
                    display_path: display_path_for(&canonical, repo_root),
                    abs_path: canonical,
                    reason: raw.reason,
                    is_dir,
                });
            }

            if selections.is_empty() {
                return Err(SecurityReviewFailure {
                    message: "Auto scope returned no directories.".to_string(),
                    logs,
                });
            }

            prune_auto_scope_parent_child_overlaps(&mut selections, &mut logs);
            truncate_auto_scope_selections(&mut selections, &mut logs);

            Ok((selections, logs))
        }
    }
}

#[cfg(test)]
mod data_classification_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn build_table_sorts_by_sensitivity_then_name() {
        let rows = vec![
            DataClassificationRow {
                data_type: "Session Tokens".to_string(),
                sensitivity: "high".to_string(),
                storage_location: "redis".to_string(),
                retention: "7 days".to_string(),
                encryption_at_rest: "aes-256".to_string(),
                in_transit: "tls 1.3".to_string(),
                accessed_by: "web app".to_string(),
            },
            DataClassificationRow {
                data_type: "API Keys".to_string(),
                sensitivity: "high".to_string(),
                storage_location: "secrets manager".to_string(),
                retention: "rotate quarterly".to_string(),
                encryption_at_rest: "kms".to_string(),
                in_transit: "tls 1.3".to_string(),
                accessed_by: "deployment pipeline".to_string(),
            },
            DataClassificationRow {
                data_type: "Audit Logs".to_string(),
                sensitivity: "medium".to_string(),
                storage_location: "s3".to_string(),
                retention: "13 months".to_string(),
                encryption_at_rest: "aes-256".to_string(),
                in_transit: "tls".to_string(),
                accessed_by: "security team".to_string(),
            },
        ];

        let table = build_data_classification_table(&rows).expect("expected table output");
        let expected = ["## Data Classification",
            "",
            "| Data Type | Sensitivity | Storage Location | Retention | Encryption At Rest | In Transit | Accessed By |",
            "|---|---|---|---|---|---|---|",
            "| API Keys | high | secrets manager | rotate quarterly | kms | tls 1.3 | deployment pipeline |",
            "| Session Tokens | high | redis | 7 days | aes-256 | tls 1.3 | web app |",
            "| Audit Logs | medium | s3 | 13 months | aes-256 | tls | security team |",
            ""]
        .join("\n");
        assert_eq!(table, expected);

        assert_eq!(build_data_classification_table(&[]), None);
    }

    #[test]
    fn inject_replaces_existing_section() {
        let spec = "\
# Project Specification

## Data Classification
Legacy content to be replaced.

## Authentication
Existing auth details.
";
        let table_markdown = "\
## Data Classification

| Data Type | Sensitivity | Storage Location | Retention | Encryption At Rest | In Transit | Accessed By |
|---|---|---|---|---|---|---|
| Customer PII | high | postgres | 90 days | aes-256 | tls 1.2+ | support portal |

";

        let updated = inject_data_classification_section(spec, table_markdown);
        let expected = "\
# Project Specification

## Data Classification

| Data Type | Sensitivity | Storage Location | Retention | Encryption At Rest | In Transit | Accessed By |
|---|---|---|---|---|---|---|
| Customer PII | high | postgres | 90 days | aes-256 | tls 1.2+ | support portal |


## Authentication
Existing auth details.";
        assert_eq!(updated, expected);
    }

    #[test]
    fn inject_appends_section_when_missing() {
        let spec = "\
# Project Specification

## Overview
System overview text.
";
        let table_markdown = "\
## Data Classification

| Data Type | Sensitivity | Storage Location | Retention | Encryption At Rest | In Transit | Accessed By |
|---|---|---|---|---|---|---|
| Billing Data | high | stripe | 7 years | provider-managed | tls 1.2+ | finance team |

";
        let updated = inject_data_classification_section(spec, table_markdown);
        let expected = "\
# Project Specification

## Overview
System overview text.

## Data Classification

| Data Type | Sensitivity | Storage Location | Retention | Encryption At Rest | In Transit | Accessed By |
|---|---|---|---|---|---|---|
| Billing Data | high | stripe | 7 years | provider-managed | tls 1.2+ | finance team |

";
        assert_eq!(updated, expected);
    }
}

#[cfg(test)]
mod auto_scope_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn parse_paths(input: &str) -> Option<Vec<(String, Option<String>)>> {
        match parse_auto_scope_response(input) {
            AutoScopeParseResult::All => None,
            AutoScopeParseResult::Selections(selections) => Some(
                selections
                    .into_iter()
                    .map(|selection| (selection.path, selection.reason))
                    .collect(),
            ),
        }
    }

    #[test]
    fn parses_simple_json_lines() {
        let input = r#"
{"path": "api", "include": true, "reason": "handles requests"}
{"path": "cli", "include": false}
{"path": "auth", "include": true}
"#;

        let result = parse_paths(input).expect("expected selections");
        assert_eq!(
            result,
            vec![
                ("api".to_string(), Some("handles requests".to_string())),
                ("auth".to_string(), None),
            ]
        );
    }

    #[test]
    fn parses_wrapped_json_objects() {
        let input = r#"
LLM summary:
- relevant dirs below
{"path": "services/gateway", "include": "yes", "reason": "external entrypoint"}
{"path": "docs", "include": "no"}
Trailing note"#;

        let result = parse_paths(input).expect("expected selections");
        assert_eq!(
            result,
            vec![(
                "services/gateway".to_string(),
                Some("external entrypoint".to_string())
            )]
        );
    }

    #[test]
    fn detects_all_request() {
        let input = r#"
Some explanation first
ALL
"#;

        assert!(parse_paths(input).is_none());
    }

    #[test]
    fn parses_nested_json_array() {
        let input = r#"{"selections":[{"dir":"backend","include":1},{"dir":"tests","include":0}]}"#;

        let result = parse_paths(input).expect("expected selections");
        assert_eq!(result, vec![("backend".to_string(), None)],);
    }

    #[test]
    fn defaults_to_include_when_flag_missing() {
        // Model may omit `include`; treat as include=true.
        let input = r#"
{"path": "api"}
{"path": "docs", "include": false}
"#;

        let result = parse_paths(input).expect("expected selections");
        assert_eq!(result, vec![("api".to_string(), None)],);
    }
}

#[cfg(test)]
mod risk_rerank_tool_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn extracts_read_requests_with_range() {
        let input = "READ: src/lib.rs#L10-L12\n{\"id\": 1}\n";
        let (cleaned, requests) = extract_read_requests(input);
        assert_eq!(cleaned.trim(), "{\"id\": 1}");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.path, PathBuf::from("src/lib.rs"));
        assert_eq!(request.start, Some(10));
        assert_eq!(request.end, Some(12));
    }

    #[test]
    fn ignores_invalid_read_requests() {
        let input = "READ: /etc/passwd\npayload";
        let (cleaned, requests) = extract_read_requests(input);
        assert_eq!(requests.len(), 0);
        assert!(cleaned.contains("/etc/passwd"));
    }
}

#[cfg(test)]
mod bug_analysis_progress_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn bug_analysis_progress_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bug_analysis_pass_1.json");
        let mut progress = BugAnalysisProgress::new(1, 2);
        progress.upsert_file(BugAnalysisProgressFile {
            index: 0,
            path_display: "src/lib.rs".to_string(),
            relative_path: "src/lib.rs".to_string(),
            findings_count: 1,
            duration_ms: 123,
            bug_section: Some("### [1] Example\n\nDetails".to_string()),
        });

        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            write_bug_analysis_progress(&path, &progress)
                .await
                .expect("write progress");
            let loaded = read_bug_analysis_progress(&path)
                .await
                .expect("read progress")
                .expect("some progress");
            assert_eq!(loaded, progress);
        });
    }
}

async fn filter_spec_directories(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    repo_root: &Path,
    candidates: &[(PathBuf, String)],
    metrics: Arc<ReviewMetrics>,
) -> Result<Vec<(PathBuf, String)>, SecurityReviewFailure> {
    let repository_label = display_path_for(repo_root, repo_root);
    let mut prompt = String::new();
    prompt.push_str(&format!("Repository root: {repository_label}\n\n"));
    prompt.push_str("Candidate directories:\n");
    for (idx, (_, label)) in candidates.iter().enumerate() {
        let _ = writeln!(&mut prompt, "{:>2}. {}", idx + 1, label);
    }
    prompt.push_str(
        "\nSelect the most security-relevant directories (ideally 3-8). \
Return a newline-separated list using either directory indices or paths. \
Return ALL to keep every directory.",
    );

    let response = call_model(
        client,
        provider,
        auth,
        SPEC_GENERATION_MODEL,
        SPEC_DIR_FILTER_SYSTEM_PROMPT,
        &prompt,
        metrics,
        0.0,
    )
    .await
    .map_err(|err| SecurityReviewFailure {
        message: format!("Directory filter model request failed: {err}"),
        logs: vec![format!("Directory filter model request failed: {err}")],
    })?;

    let mut selected_indices: Vec<usize> = Vec::new();
    for raw_line in response.text.lines() {
        let trimmed = raw_line.trim().trim_matches('`');
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("all") {
            return Ok(candidates.to_vec());
        }

        let mut parsed: Option<usize> = None;

        if let Ok(idx) = trimmed.parse::<usize>() {
            if (1..=candidates.len()).contains(&idx) {
                parsed = Some(idx - 1);
            }
        } else {
            let digits: String = trimmed.chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty()
                && trimmed[digits.len()..].starts_with('.')
                && let Ok(idx) = digits.parse::<usize>()
                && (1..=candidates.len()).contains(&idx)
            {
                parsed = Some(idx - 1);
            }
            if parsed.is_none() {
                if let Some((index, _)) = candidates
                    .iter()
                    .enumerate()
                    .find(|(_, (_, label))| label.eq_ignore_ascii_case(trimmed))
                {
                    parsed = Some(index);
                } else if let Some((index, _)) =
                    candidates.iter().enumerate().find(|(_, (path, _))| {
                        path.file_name()
                            .and_then(|s| s.to_str())
                            .map(|name| name.eq_ignore_ascii_case(trimmed))
                            .unwrap_or(false)
                    })
                {
                    parsed = Some(index);
                }
            }
        }

        if let Some(index) = parsed
            && !selected_indices.contains(&index)
        {
            selected_indices.push(index);
        }
    }

    if selected_indices.is_empty() {
        return Ok(candidates.to_vec());
    }

    selected_indices.sort_unstable();
    if selected_indices.len() > SPEC_DIR_FILTER_TARGET {
        selected_indices.truncate(SPEC_DIR_FILTER_TARGET);
    }

    Ok(selected_indices
        .into_iter()
        .map(|idx| candidates[idx].clone())
        .collect())
}

#[allow(clippy::too_many_arguments)]
async fn run_spec_agent(
    config: &Config,
    provider: &ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    repo_root: &Path,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    prompt: String,
    metrics: Arc<ReviewMetrics>,
) -> Result<(String, Vec<String>), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();

    let mut spec_config = config.clone();
    spec_config.model = SPEC_GENERATION_MODEL.to_string();
    spec_config.model_provider = provider.clone();
    spec_config.base_instructions = Some(SPEC_SYSTEM_PROMPT.to_string());
    spec_config.user_instructions = None;
    spec_config.developer_instructions = None;
    spec_config.compact_prompt = None;
    spec_config.cwd = repo_root.to_path_buf();
    spec_config
        .features
        .disable(Feature::ApplyPatchFreeform)
        .disable(Feature::WebSearchRequest)
        .disable(Feature::ViewImageTool)
        .disable(Feature::RmcpClient);
    spec_config.mcp_servers.clear();
    spec_config.use_experimental_use_rmcp_client = false;

    let manager = ConversationManager::new(
        auth_manager,
        SessionSource::SubAgent(SubAgentSource::Other("security_review_spec".to_string())),
    );

    let conversation = match manager.new_conversation(spec_config).await {
        Ok(new_conversation) => new_conversation.conversation,
        Err(err) => {
            let message = format!("Failed to start specification agent: {err}");
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };

    if let Err(err) = conversation
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: prompt }],
        })
        .await
    {
        let message = format!("Failed to submit specification prompt: {err}");
        push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
        return Err(SecurityReviewFailure { message, logs });
    }

    let mut last_agent_message: Option<String> = None;

    loop {
        let event = match conversation.next_event().await {
            Ok(event) => event,
            Err(err) => {
                let message = format!("Specification agent terminated unexpectedly: {err}");
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
        };

        record_tool_call_from_event(metrics.as_ref(), &event.msg);

        match event.msg {
            EventMsg::TaskComplete(done) => {
                if let Some(msg) = done.last_agent_message {
                    last_agent_message = Some(msg);
                }
                break;
            }
            EventMsg::AgentMessage(msg) => {
                last_agent_message = Some(msg.message.clone());
            }
            EventMsg::AgentReasoning(reason) => {
                log_model_reasoning(&reason.text, &progress_sender, &log_sink, &mut logs);
            }
            EventMsg::Warning(warn) => {
                push_progress_log(&progress_sender, &log_sink, &mut logs, warn.message);
            }
            EventMsg::Error(err) => {
                let message = format!("Specification agent error: {}", err.message);
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
            EventMsg::TurnAborted(aborted) => {
                let message = format!("Specification agent aborted: {:?}", aborted.reason);
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
            EventMsg::TokenCount(count) => {
                if let Some(info) = count.info {
                    metrics.record_model_call();
                    metrics.record_usage(&info.last_token_usage);
                }
            }
            _ => {}
        }
    }

    let raw_spec = match last_agent_message.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }) {
        Some(text) => text,
        None => {
            let message = "Specification agent produced an empty response.".to_string();
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };

    let _ = conversation.submit(Op::Shutdown).await;

    Ok((raw_spec, logs))
}

struct BugAgentOutcome {
    section: String,
    logs: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
async fn run_bug_agent(
    config: &Config,
    provider: &ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    repo_root: &Path,
    prompt: String,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
    model: &str,
) -> Result<BugAgentOutcome, SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();

    let mut bug_config = config.clone();
    bug_config.model = model.to_string();
    bug_config.model_provider = provider.clone();
    bug_config.base_instructions = Some(BUGS_SYSTEM_PROMPT.to_string());
    bug_config.user_instructions = None;
    bug_config.developer_instructions = None;
    bug_config.compact_prompt = None;
    bug_config.cwd = repo_root.to_path_buf();
    bug_config
        .features
        .disable(Feature::ApplyPatchFreeform)
        .disable(Feature::WebSearchRequest)
        .disable(Feature::ViewImageTool)
        .disable(Feature::RmcpClient);
    bug_config.mcp_servers.clear();
    bug_config.use_experimental_use_rmcp_client = false;

    let manager = ConversationManager::new(
        auth_manager,
        SessionSource::SubAgent(SubAgentSource::Other("security_review_bug".to_string())),
    );

    let conversation = match manager.new_conversation(bug_config).await {
        Ok(new_conversation) => new_conversation.conversation,
        Err(err) => {
            let message = format!("Failed to start bug agent: {err}");
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };

    if let Err(err) = conversation
        .submit(Op::UserInput {
            items: vec![UserInput::Text { text: prompt }],
        })
        .await
    {
        let message = format!("Failed to submit bug analysis prompt: {err}");
        push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
        return Err(SecurityReviewFailure { message, logs });
    }

    let mut last_agent_message: Option<String> = None;

    loop {
        let event = match conversation.next_event().await {
            Ok(event) => event,
            Err(err) => {
                let message = format!("Bug agent terminated unexpectedly: {err}");
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
        };

        record_tool_call_from_event(metrics.as_ref(), &event.msg);

        match event.msg {
            EventMsg::TaskComplete(done) => {
                if let Some(msg) = done.last_agent_message {
                    last_agent_message = Some(msg);
                }
                break;
            }
            EventMsg::AgentMessage(msg) => {
                last_agent_message = Some(msg.message.clone());
            }
            EventMsg::AgentReasoning(reason) => {
                log_model_reasoning(&reason.text, &progress_sender, &log_sink, &mut logs);
            }
            EventMsg::Warning(warn) => {
                push_progress_log(&progress_sender, &log_sink, &mut logs, warn.message);
            }
            EventMsg::Error(err) => {
                let message = format!("Bug agent error: {}", err.message);
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
            EventMsg::TurnAborted(aborted) => {
                let message = format!("Bug agent aborted: {:?}", aborted.reason);
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
            EventMsg::TokenCount(count) => {
                if let Some(info) = count.info {
                    metrics.record_model_call();
                    metrics.record_usage(&info.last_token_usage);
                }
            }
            _ => {}
        }
    }

    let section = match last_agent_message.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }) {
        Some(text) => text,
        None => {
            let message = "Bug agent produced an empty response.".to_string();
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };

    let _ = conversation.submit(Op::Shutdown).await;

    Ok(BugAgentOutcome { section, logs })
}

#[allow(clippy::too_many_arguments)]
async fn generate_specs_single_worker(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    repo_root: &Path,
    project_locations: &[String],
    raw_dir: &Path,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
    config: &Config,
    auth_manager: Arc<AuthManager>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
) -> Result<(Vec<SpecEntry>, Vec<String>), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();
    let scope_label = build_spec_scope_label(project_locations, repo_root);
    let start_message = format!(
        "Generating specification with a single agent across {} location(s).",
        project_locations.len()
    );
    push_progress_log(
        &progress_sender,
        &log_sink,
        &mut logs,
        start_message.clone(),
    );

    let date = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown-date".to_string());
    let prompt = build_spec_prompt_text(
        project_locations,
        &scope_label,
        SPEC_GENERATION_MODEL,
        &date,
        repo_root,
    );

    let (raw_spec, agent_logs) = match run_spec_agent(
        config,
        provider,
        auth_manager.clone(),
        repo_root,
        progress_sender.clone(),
        log_sink.clone(),
        prompt,
        metrics.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            push_progress_log(&progress_sender, &log_sink, &mut logs, err.message.clone());
            logs.extend(err.logs);
            return Err(SecurityReviewFailure {
                message: err.message,
                logs,
            });
        }
    };
    logs.extend(agent_logs);

    let mut sanitized = fix_mermaid_blocks(&raw_spec);

    if !sanitized.trim().is_empty() {
        let polish_message = format!("Polishing specification markdown for {scope_label}.");
        push_progress_log(
            &progress_sender,
            &log_sink,
            &mut logs,
            polish_message.clone(),
        );
        let outcome =
            polish_markdown_block(client, provider, auth, metrics.clone(), &sanitized, None)
                .await
                .map_err(|err| SecurityReviewFailure {
                    message: format!("Failed to polish specification for {scope_label}: {err}"),
                    logs: Vec::new(),
                })?;
        if let Some(tx) = progress_sender.as_ref() {
            for line in &outcome.reasoning_logs {
                tx.send(AppEvent::SecurityReviewLog(line.clone()));
            }
        }
        logs.extend(outcome.reasoning_logs.clone());
        sanitized = fix_mermaid_blocks(&outcome.text);
    }

    let slug = slugify_label(&scope_label);
    let file_path = raw_dir.join(format!("{slug}.md"));
    tokio_fs::write(&file_path, sanitized.as_bytes())
        .await
        .map_err(|e| SecurityReviewFailure {
            message: format!(
                "Failed to write specification for {scope_label} to {}: {e}",
                file_path.display()
            ),
            logs: Vec::new(),
        })?;

    let display_path = display_path_for(&file_path, repo_root);
    let done_message = format!("Specification for {scope_label} saved to {display_path}.");
    push_progress_log(&progress_sender, &log_sink, &mut logs, done_message);

    let api_markdown = extract_api_markdown(&sanitized);

    Ok((
        vec![SpecEntry {
            location_label: scope_label,
            markdown: sanitized,
            raw_path: file_path,
            api_markdown,
        }],
        logs,
    ))
}

#[allow(clippy::too_many_arguments)]
async fn generate_specs_parallel_workers(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    repo_root: &Path,
    normalized: &[PathBuf],
    display_locations: &[String],
    raw_dir: &Path,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
    spec_progress_path: &Path,
    spec_progress: &mut SpecGenerationProgress,
    config: &Config,
    auth_manager: Arc<AuthManager>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
) -> Result<(Vec<SpecEntry>, Vec<String>), SecurityReviewFailure> {
    // Legacy parallel spec workers retained for potential reuse.
    let mut logs: Vec<String> = Vec::new();
    let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
    for path in normalized {
        let target = path.clone();
        let provider = provider.clone();
        let auth = auth.clone();
        let repo_root = repo_root.to_path_buf();
        let project_locations = display_locations.to_vec();
        let raw_dir = raw_dir.to_path_buf();
        let progress_sender = progress_sender.clone();
        let auth_manager = auth_manager.clone();
        let log_sink = log_sink.clone();
        let metrics = metrics.clone();
        in_flight.push(async move {
            generate_spec_for_location(
                client,
                provider,
                auth,
                repo_root,
                target.clone(),
                project_locations,
                raw_dir,
                progress_sender,
                metrics,
                config,
                auth_manager,
                log_sink,
            )
            .await
            .map(|(entry, logs)| (target, entry, logs))
        });
    }

    let mut spec_entries: Vec<SpecEntry> = Vec::new();

    while let Some(result) = in_flight.next().await {
        match result {
            Ok((target, entry, mut entry_logs)) => {
                spec_progress.upsert_completed(SpecGenerationProgressCompleted {
                    target_path: encode_progress_path(&target, repo_root),
                    location_label: entry.location_label.clone(),
                    raw_path: entry.raw_path.display().to_string(),
                });
                write_spec_generation_progress(spec_progress_path, spec_progress).await?;
                logs.append(&mut entry_logs);
                spec_entries.push(entry);
            }
            Err(mut failure) => {
                logs.append(&mut failure.logs);
                return Err(SecurityReviewFailure {
                    message: failure.message,
                    logs,
                });
            }
        }
    }

    Ok((spec_entries, logs))
}

#[allow(clippy::too_many_arguments)]
async fn generate_spec_for_location(
    client: &CodexHttpClient,
    provider: ModelProviderInfo,
    auth: Option<CodexAuth>,
    repo_root: PathBuf,
    target: PathBuf,
    project_locations: Vec<String>,
    raw_dir: PathBuf,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
    config: &Config,
    auth_manager: Arc<AuthManager>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
) -> Result<(SpecEntry, Vec<String>), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();
    let location_label = display_path_for(&target, &repo_root);
    let start_message = format!("Generating specification for {location_label}...");
    push_progress_log(
        &progress_sender,
        &log_sink,
        &mut logs,
        start_message.clone(),
    );

    let date = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown-date".to_string());
    let prompt = build_spec_prompt_text(
        &project_locations,
        &location_label,
        SPEC_GENERATION_MODEL,
        &date,
        &repo_root,
    );

    let (raw_spec, agent_logs) = match run_spec_agent(
        config,
        &provider,
        auth_manager.clone(),
        &repo_root,
        progress_sender.clone(),
        log_sink.clone(),
        prompt,
        metrics.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            push_progress_log(&progress_sender, &log_sink, &mut logs, err.message.clone());
            logs.extend(err.logs);
            return Err(SecurityReviewFailure {
                message: err.message,
                logs,
            });
        }
    };
    logs.extend(agent_logs);

    let mut sanitized = fix_mermaid_blocks(&raw_spec);

    if !sanitized.trim().is_empty() {
        let polish_message = format!("Polishing specification markdown for {location_label}.");
        push_progress_log(
            &progress_sender,
            &log_sink,
            &mut logs,
            polish_message.clone(),
        );
        let outcome =
            polish_markdown_block(client, &provider, &auth, metrics.clone(), &sanitized, None)
                .await
                .map_err(|err| SecurityReviewFailure {
                    message: format!("Failed to polish specification for {location_label}: {err}"),
                    logs: Vec::new(),
                })?;
        if let Some(tx) = progress_sender.as_ref() {
            for line in &outcome.reasoning_logs {
                tx.send(AppEvent::SecurityReviewLog(line.clone()));
            }
        }
        logs.extend(outcome.reasoning_logs.clone());
        sanitized = fix_mermaid_blocks(&outcome.text);
    }

    let slug = slugify_label(&location_label);
    let file_path = raw_dir.join(format!("{slug}.md"));
    tokio_fs::write(&file_path, sanitized.as_bytes())
        .await
        .map_err(|e| SecurityReviewFailure {
            message: format!(
                "Failed to write specification for {location_label} to {}: {e}",
                file_path.display()
            ),
            logs: Vec::new(),
        })?;

    let display_path = display_path_for(&file_path, &repo_root);
    let done_message = format!("Specification for {location_label} saved to {display_path}.");
    push_progress_log(&progress_sender, &log_sink, &mut logs, done_message);

    let api_markdown = extract_api_markdown(&sanitized);

    Ok((
        SpecEntry {
            location_label,
            markdown: sanitized,
            raw_path: file_path,
            api_markdown,
        },
        logs,
    ))
}

#[allow(clippy::too_many_arguments)]
async fn generate_threat_model(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    repository_summary: &str,
    repo_root: &Path,
    spec: &SpecGenerationOutcome,
    output_root: &Path,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
) -> Result<Option<ThreatModelOutcome>, SecurityReviewFailure> {
    if spec.combined_markdown.trim().is_empty() {
        return Ok(None);
    }

    let threats_dir = output_root.join("threats");
    tokio_fs::create_dir_all(&threats_dir)
        .await
        .map_err(|e| SecurityReviewFailure {
            message: format!("Failed to create {}: {e}", threats_dir.display()),
            logs: Vec::new(),
        })?;

    let mut logs: Vec<String> = Vec::new();
    let start_message = format!(
        "Generating threat model from {} specification section(s).",
        spec.locations.len().max(1)
    );
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(start_message.clone()));
    }
    logs.push(start_message);

    let prompt = build_threat_model_prompt(repository_summary, spec);
    let response_output = call_model(
        client,
        provider,
        auth,
        model,
        THREAT_MODEL_SYSTEM_PROMPT,
        &prompt,
        metrics.clone(),
        0.1,
    )
    .await
    .map_err(|err| {
        let failure_logs = vec![
            "Threat model provider returned a response that could not be parsed.".to_string(),
            format!("Model error: {err}"),
            "Double-check API credentials and network availability for the security review process.".to_string(),
        ];
        if let Some(tx) = progress_sender.as_ref() {
            for line in &failure_logs {
                tx.send(AppEvent::SecurityReviewLog(line.clone()));
            }
        }
        SecurityReviewFailure {
            message: format!("Threat model generation failed: {err}"),
            logs: failure_logs,
        }
    })?;
    if let Some(reasoning) = response_output.reasoning.as_ref() {
        log_model_reasoning(reasoning, &progress_sender, &None, &mut logs);
    }
    let mut response_text = response_output.text;
    let mut sanitized_response = fix_mermaid_blocks(&response_text);
    sanitized_response = sort_threat_table(&sanitized_response).unwrap_or(sanitized_response);

    if !threat_table_has_rows(&sanitized_response) {
        let warn = "Threat model is missing table rows; requesting correction.";
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(warn.to_string()));
        }
        logs.push(warn.to_string());

        let retry_prompt = build_threat_model_retry_prompt(&prompt, &sanitized_response);
        let response_output = call_model(
            client,
            provider,
            auth,
            model,
            THREAT_MODEL_SYSTEM_PROMPT,
            &retry_prompt,
            metrics.clone(),
            0.1,
        )
        .await
        .map_err(|err| {
            let failure_logs = vec![
                "Threat model retry still failed to decode the provider response.".to_string(),
                format!("Model error: {err}"),
                "Verify the provider is returning JSON (no HTML/proxy pages) and that credentials are correct.".to_string(),
            ];
            if let Some(tx) = progress_sender.as_ref() {
                for line in &failure_logs {
                    tx.send(AppEvent::SecurityReviewLog(line.clone()));
                }
            }
            SecurityReviewFailure {
                message: format!("Threat model regeneration failed: {err}"),
                logs: failure_logs,
            }
        })?;
        if let Some(reasoning) = response_output.reasoning.as_ref() {
            log_model_reasoning(reasoning, &progress_sender, &None, &mut logs);
        }
        response_text = response_output.text;
        sanitized_response = fix_mermaid_blocks(&response_text);
        sanitized_response = sort_threat_table(&sanitized_response).unwrap_or(sanitized_response);

        if !threat_table_has_rows(&sanitized_response) {
            let retry_warn =
                "Threat model retry still missing populated table rows; leaving placeholder.";
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(retry_warn.to_string()));
            }
            logs.push(retry_warn.to_string());
            sanitized_response.push_str(
                "\n\n> ⚠️ Threat table generation failed after retry; please review manually.\n",
            );
        }
    }

    if !sanitized_response.trim().is_empty() {
        let polish_message = "Polishing threat model markdown formatting.".to_string();
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(polish_message.clone()));
        }
        logs.push(polish_message);
        let outcome = match polish_markdown_block(
            client,
            provider,
            auth,
            metrics.clone(),
            &sanitized_response,
            None,
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(err) => {
                return Err(SecurityReviewFailure {
                    message: format!("Failed to polish threat model: {err}"),
                    logs: logs.clone(),
                });
            }
        };
        if let Some(tx) = progress_sender.as_ref() {
            for line in &outcome.reasoning_logs {
                tx.send(AppEvent::SecurityReviewLog(line.clone()));
            }
        }
        logs.extend(outcome.reasoning_logs.clone());
        sanitized_response = fix_mermaid_blocks(&outcome.text);
    }

    sanitized_response = ensure_threat_model_heading(sanitized_response);

    let threat_file = threats_dir.join("threat_model.md");
    tokio_fs::write(&threat_file, sanitized_response.as_bytes())
        .await
        .map_err(|e| SecurityReviewFailure {
            message: format!(
                "Failed to write threat model to {}: {e}",
                threat_file.display()
            ),
            logs: Vec::new(),
        })?;

    let done_message = format!(
        "Threat model saved to {}.",
        display_path_for(&threat_file, repo_root)
    );
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(done_message.clone()));
    }
    logs.push(done_message);

    Ok(Some(ThreatModelOutcome {
        markdown: sanitized_response,
        logs,
    }))
}

#[allow(clippy::too_many_arguments)]
async fn compact_analysis_context(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    spec_markdown: Option<&str>,
    threat_markdown: Option<&str>,
    metrics: Arc<ReviewMetrics>,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    model: &str,
) -> Result<Option<String>, SecurityReviewFailure> {
    let mut sections: Vec<String> = Vec::new();
    if let Some(spec) = spec_markdown {
        let trimmed = spec.trim();
        if !trimmed.is_empty() {
            sections.push(format!("## Specification\n{trimmed}"));
        }
    }
    if let Some(threat) = threat_markdown {
        let trimmed = threat.trim();
        if !trimmed.is_empty() {
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("# threat model") || lower.starts_with("## threat model") {
                sections.push(trimmed.to_string());
            } else {
                sections.push(format!("## Threat Model\n{trimmed}"));
            }
        }
    }

    if sections.is_empty() {
        return Ok(None);
    }

    let combined = sections.join("\n\n");
    let mut logs: Vec<String> = Vec::new();
    push_progress_log(
        &progress_sender,
        &log_sink,
        &mut logs,
        "Compacting specification and threat model context for bug analysis.".to_string(),
    );

    let prompt = format!(
        "Condense the following specification and threat model into a concise context for finding security bugs. Keep architecture, data flows, authn/z, controls, data sensitivity, and notable threats. Aim for at most {ANALYSIS_CONTEXT_MAX_CHARS} characters. Return short paragraphs or bullets; no tables or headings longer than a few words.\n\n{combined}"
    );

    let response = call_model(
        client,
        provider,
        auth,
        model,
        BUGS_SYSTEM_PROMPT,
        &prompt,
        metrics,
        0.0,
    )
    .await
    .map_err(|err| SecurityReviewFailure {
        message: format!("Context compaction failed: {err}"),
        logs: logs.clone(),
    })?;

    if let Some(reasoning) = response.reasoning.as_ref() {
        log_model_reasoning(reasoning, &progress_sender, &log_sink, &mut logs);
    }

    let compacted = trim_prompt_context(response.text.trim(), ANALYSIS_CONTEXT_MAX_CHARS);
    if compacted.is_empty() {
        let message = "Context compaction returned no content.".to_string();
        push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
        return Err(SecurityReviewFailure { message, logs });
    }

    let message = format!(
        "Compacted specification/threat context to {} characters.",
        compacted.len()
    );
    push_progress_log(&progress_sender, &log_sink, &mut logs, message);
    Ok(Some(compacted))
}

fn guess_testing_defaults(repo_root: &Path) -> (bool, bool, bool, bool, bool, u16, String) {
    let has_cargo = repo_root.join("Cargo.toml").exists();
    let has_package_json = repo_root.join("package.json").exists();
    let has_dockerfile = repo_root.join("Dockerfile").exists();
    let has_compose =
        repo_root.join("docker-compose.yml").exists() || repo_root.join("compose.yml").exists();
    let has_playwright = repo_root.join("playwright.config.ts").exists()
        || repo_root.join("playwright.config.js").exists()
        || repo_root.join("playwright.config.mjs").exists();
    let repo_label = repo_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("app")
        .to_string();
    let default_port = if has_package_json { 3000 } else { 8000 };
    (
        has_cargo,
        has_package_json,
        has_dockerfile,
        has_compose,
        has_playwright,
        default_port,
        repo_label,
    )
}

fn build_testing_instructions(repo_root: &Path) -> String {
    let (
        has_cargo,
        has_package_json,
        has_dockerfile,
        has_compose,
        has_playwright,
        default_port,
        repo_label,
    ) = guess_testing_defaults(repo_root);

    let mut sections: Vec<String> = Vec::new();

    let mut quickstart: Vec<String> = Vec::new();
    if has_cargo {
        quickstart.push("- cargo build --locked".to_string());
        quickstart.push("- cargo run --release".to_string());
    }
    if has_package_json {
        quickstart.push("- npm install".to_string());
        quickstart.push(format!(
            "- npm run build && npm run dev -- --port {default_port}"
        ));
    }
    if quickstart.is_empty() {
        quickstart.push(
            "- Identify the primary service entrypoint, install deps, and start it with the appropriate runner (npm/cargo/docker)."
                .to_string(),
        );
    }
    sections.push(format!("## Quickstart\n{}\n", quickstart.join("\n")));

    if has_cargo {
        sections.push(
            "## Native build\n- cargo build --locked\n- Artifacts: `target/debug` (or `target/release` after `cargo build --release`).\n- Run: `cargo run --release`\n"
                .to_string(),
        );
    }

    if has_package_json {
        sections.push(format!(
            "## Web/Node\n- npm install\n- npm run build\n- npm run dev -- --port {default_port}\n- Verify: `curl -i http://localhost:{default_port}` (adjust if your app uses a different port).\n"
        ));
    }

    if has_dockerfile || has_compose {
        let tag = format!("{repo_label}-local");
        let compose_cmd = if has_compose {
            "docker compose up --build".to_string()
        } else {
            "docker run -p <host_port>:<container_port> <image>".to_string()
        };
        sections.push(format!(
            "## Docker\n- docker build -t {tag} .\n- {compose_cmd}\n- Verify: `curl -i http://localhost:{default_port}` (update to the container's exposed port).\n"
        ));
    }

    if has_playwright {
        sections.push(
            "## Headless checks\n- npm install (if not already)\n- npx playwright install\n- npx playwright test\n"
                .to_string(),
        );
    } else {
        sections.push(
            "## Headless checks\n- If the project has a UI, install Playwright and add a smoke test harness (e.g., `npx playwright install` then `npx playwright test`).\n"
                .to_string(),
        );
    }

    sections.push(format!(
        "## Manual verification\n- Once the service is running (default port: {default_port}; override with `PORT` env if applicable), confirm a 200/OK from a health or root endpoint:\n  - `curl -I http://localhost:{default_port}`\n- For authenticated flows, add seed users in test config or fixtures before hitting secured endpoints.\n"
    ));

    format!("# Local build and smoke test\n\n{}\n", sections.join("\n"))
}

async fn write_testing_instructions(
    repo_root: &Path,
    specs_root: &Path,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
) {
    let testing_path = specs_root.join("TESTING.md");
    let contents = build_testing_instructions(repo_root);
    let log_message = format!(
        "Writing testing instructions to {}.",
        display_path_for(&testing_path, repo_root)
    );
    push_progress_log(&progress_sender, &log_sink, &mut Vec::new(), log_message);
    if let Some(parent) = testing_path.parent() {
        let _ = tokio_fs::create_dir_all(parent).await;
    }
    if let Err(err) = tokio_fs::write(&testing_path, contents.as_bytes()).await {
        let warn = format!(
            "Failed to write testing instructions to {}: {err}",
            testing_path.display()
        );
        push_progress_log(&progress_sender, &log_sink, &mut Vec::new(), warn);
    } else {
        let message = format!(
            "Testing instructions saved to {} (includes build/run, Docker, and headless checks).",
            display_path_for(&testing_path, repo_root)
        );
        push_progress_log(&progress_sender, &log_sink, &mut Vec::new(), message);
    }
}

#[allow(clippy::too_many_arguments)]
async fn combine_spec_markdown(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    project_locations: &[String],
    specs: &[SpecEntry],
    combined_path: &Path,
    repo_root: &Path,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
) -> Result<(String, Vec<String>), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();
    let message = format!(
        "Merging {} specification draft(s) into a single report.",
        specs.len()
    );
    push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());

    let base_prompt = build_combine_specs_prompt(project_locations, specs);
    let mut conversation: Vec<String> = Vec::new();
    let mut seen_search_requests: HashSet<String> = HashSet::new();
    let mut seen_read_requests: HashSet<String> = HashSet::new();
    let mut tool_rounds = 0usize;
    let mut command_error_count = 0usize;

    let combined_raw = loop {
        if tool_rounds > SPEC_COMBINE_MAX_TOOL_ROUNDS {
            return Err(SecurityReviewFailure {
                message: format!("Spec merge exceeded {SPEC_COMBINE_MAX_TOOL_ROUNDS} tool rounds."),
                logs,
            });
        }

        let mut prompt = base_prompt.clone();
        if !conversation.is_empty() {
            prompt.push_str("\n\n# Conversation history\n");
            prompt.push_str(&conversation.join("\n\n"));
        }

        let response = match call_model(
            client,
            provider,
            auth,
            SPEC_GENERATION_MODEL,
            SPEC_COMBINE_SYSTEM_PROMPT,
            &prompt,
            metrics.clone(),
            0.0,
        )
        .await
        {
            Ok(output) => output,
            Err(err) => {
                let message = format!("Failed to combine specifications: {err}");
                push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                return Err(SecurityReviewFailure { message, logs });
            }
        };

        if let Some(reasoning) = response.reasoning.as_ref() {
            for line in reasoning
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
                let msg = format!("Spec merge reasoning: {truncated}");
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(msg.clone()));
                }
                logs.push(msg);
            }
        }

        let assistant_reply = response.text.trim().to_string();
        if assistant_reply.is_empty() {
            conversation.push("Assistant:".to_string());
        } else {
            conversation.push(format!("Assistant:\n{assistant_reply}"));
        }

        let (after_read, read_requests) = extract_read_requests(&response.text);
        let (cleaned_text, search_requests) = parse_search_requests(&after_read);

        let mut executed_command = false;

        for request in read_requests {
            let cmd_label = request.command.label();
            let key = request.dedupe_key();
            if !seen_read_requests.insert(key) {
                let msg = format!(
                    "Spec merge {cmd_label} `{}` skipped (already provided).",
                    request.path.display(),
                );
                logs.push(msg.clone());
                conversation.push(format!(
                    "Tool {cmd_label} `{}` already provided earlier.",
                    request.path.display()
                ));
                executed_command = true;
                continue;
            }

            executed_command = true;
            match execute_auto_scope_read(
                repo_root,
                &request.path,
                request.command,
                request.start,
                request.end,
                metrics.as_ref(),
            )
            .await
            {
                Ok(output) => {
                    logs.push(format!(
                        "Spec merge {cmd_label} `{}` returned content.",
                        request.path.display(),
                    ));
                    conversation.push(format!(
                        "Tool {cmd_label} `{}`:\n{}",
                        request.path.display(),
                        output
                    ));
                }
                Err(err) => {
                    let status = format!(
                        "Spec merge {cmd_label} `{}` failed: {err}",
                        request.path.display()
                    );
                    logs.push(status);
                    let hint = format!(
                        "Tool {cmd_label} `{}` error: {err}. Paths must be relative to the repository root ({}). SEARCH is disabled for this step; retry READ with the correct path or use GREP_FILES to locate the right file.",
                        request.path.display(),
                        repo_root.display()
                    );
                    let mut guidance = hint;
                    if is_path_lookup_error(&err) {
                        let _ = write!(
                            guidance,
                            " This still counts toward the {SPEC_COMBINE_MAX_COMMAND_ERRORS}-error limit; fix the path and retry."
                        );
                    }
                    conversation.push(guidance);
                    command_error_count += 1;
                }
            }
        }

        for request in search_requests {
            let key = request.dedupe_key();
            if !seen_search_requests.insert(key) {
                match &request {
                    ToolRequest::Content { term, mode, .. } => {
                        let display_term = summarize_search_term(term, 80);
                        let msg = format!(
                            "Spec merge SEARCH `{display_term}` ({}) skipped (already provided).",
                            mode.as_str()
                        );
                        logs.push(msg.clone());
                        conversation.push(format!(
                            "Tool SEARCH `{display_term}` ({}) already provided earlier.",
                            mode.as_str()
                        ));
                    }
                    ToolRequest::GrepFiles { args, .. } => {
                        let mut shown = serde_json::json!({ "pattern": args.pattern });
                        if let Some(ref inc) = args.include {
                            shown["include"] = serde_json::Value::String(inc.clone());
                        }
                        if let Some(ref path) = args.path {
                            shown["path"] = serde_json::Value::String(path.clone());
                        }
                        if let Some(limit) = args.limit {
                            shown["limit"] =
                                serde_json::Value::Number(serde_json::Number::from(limit as u64));
                        }
                        let msg =
                            format!("Spec merge GREP_FILES {shown} skipped (already provided).");
                        logs.push(msg.clone());
                        conversation
                            .push(format!("Tool GREP_FILES {shown} already provided earlier."));
                    }
                }
                executed_command = true;
                continue;
            }

            executed_command = true;
            match request {
                ToolRequest::Content { term, mode, .. } => {
                    let display_term = summarize_search_term(&term, 80);
                    let msg = format!(
                        "Spec merge SEARCH `{display_term}` ({}) skipped; SEARCH is disabled for this step.",
                        mode.as_str()
                    );
                    logs.push(msg);
                    conversation.push(format!(
                        "Tool SEARCH `{display_term}` ({}) error: SEARCH is disabled during spec merge. Use READ (and optionally GREP_FILES) to gather context.",
                        mode.as_str()
                    ));
                }
                ToolRequest::GrepFiles { args, .. } => {
                    let mut shown = serde_json::json!({ "pattern": args.pattern });
                    if let Some(ref inc) = args.include {
                        shown["include"] = serde_json::Value::String(inc.clone());
                    }
                    if let Some(ref path) = args.path {
                        shown["path"] = serde_json::Value::String(path.clone());
                    }
                    if let Some(limit) = args.limit {
                        shown["limit"] =
                            serde_json::Value::Number(serde_json::Number::from(limit as u64));
                    }
                    logs.push(format!("Spec merge GREP_FILES {shown} executing."));
                    match run_grep_files(repo_root, &args, &metrics).await {
                        SearchResult::Matches(output) => {
                            conversation.push(format!("Tool GREP_FILES {shown}:\n{output}"));
                        }
                        SearchResult::NoMatches => {
                            let message = "No matches found.".to_string();
                            logs.push(format!(
                                "Spec merge GREP_FILES {shown} returned no matches."
                            ));
                            conversation.push(format!("Tool GREP_FILES {shown}:\n{message}"));
                        }
                        SearchResult::Error(err) => {
                            logs.push(format!("Spec merge GREP_FILES {shown} failed: {err}"));
                            conversation.push(format!("Tool GREP_FILES {shown} error: {err}"));
                            command_error_count += 1;
                        }
                    }
                }
            }
        }

        if command_error_count >= SPEC_COMBINE_MAX_COMMAND_ERRORS {
            return Err(SecurityReviewFailure {
                message: format!("Spec merge hit {SPEC_COMBINE_MAX_COMMAND_ERRORS} tool errors."),
                logs,
            });
        }

        if executed_command {
            tool_rounds = tool_rounds.saturating_add(1);
            continue;
        }

        let final_text = cleaned_text.trim();
        if final_text.is_empty() {
            return Err(SecurityReviewFailure {
                message: "Spec merge produced an empty response.".to_string(),
                logs,
            });
        }

        break final_text.to_string();
    };

    let sanitized = fix_mermaid_blocks(&combined_raw);

    let polish_message = "Polishing combined specification markdown formatting.".to_string();
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(polish_message.clone()));
    }
    logs.push(polish_message);

    let fix_prompt = build_fix_markdown_prompt(&sanitized, Some(SPEC_COMBINED_MARKDOWN_TEMPLATE));
    let polished_response = match call_model(
        client,
        provider,
        auth,
        MARKDOWN_FIX_MODEL,
        MARKDOWN_FIX_SYSTEM_PROMPT,
        &fix_prompt,
        metrics.clone(),
        0.0,
    )
    .await
    {
        Ok(output) => {
            if let Some(reasoning) = output.reasoning.as_ref() {
                for line in reasoning
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                {
                    let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
                    let msg = format!("Spec merge polish reasoning: {truncated}");
                    if let Some(tx) = progress_sender.as_ref() {
                        tx.send(AppEvent::SecurityReviewLog(msg.clone()));
                    }
                    logs.push(msg);
                }
            }
            output.text
        }
        Err(err) => {
            let message = format!("Failed to polish combined specification markdown: {err}");
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };
    let polished = fix_mermaid_blocks(&polished_response);

    if let Err(e) = tokio_fs::write(combined_path, polished.as_bytes()).await {
        return Err(SecurityReviewFailure {
            message: format!(
                "Failed to write combined specification to {}: {e}",
                combined_path.display()
            ),
            logs,
        });
    }

    let done_message = format!(
        "Combined specification saved to {}.",
        combined_path.display()
    );
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(done_message.clone()));
    }
    logs.push(done_message);

    Ok((polished, logs))
}

fn build_spec_scope_label(project_locations: &[String], repo_root: &Path) -> String {
    if let Some((first, rest)) = project_locations.split_first() {
        if rest.is_empty() {
            first.clone()
        } else {
            format!("{first} (+{} more)", rest.len())
        }
    } else {
        repo_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| repo_root.display().to_string())
    }
}

fn build_spec_prompt_text(
    project_locations: &[String],
    target_label: &str,
    model_name: &str,
    date: &str,
    repo_root: &Path,
) -> String {
    let locations_block = if project_locations.is_empty() {
        target_label.to_string()
    } else {
        project_locations.join("\n")
    };
    let repo_root_display = repo_root.to_string_lossy();

    let template_body = SPEC_MARKDOWN_TEMPLATE
        .replace("{project_locations}", &locations_block)
        .replace("{target_label}", target_label)
        .replace("{model_name}", model_name)
        .replace("{date}", date);

    SPEC_PROMPT_TEMPLATE
        .replace("{project_locations}", &locations_block)
        .replace("{target_label}", target_label)
        .replace("{repo_root}", repo_root_display.as_ref())
        .replace("{spec_template}", &template_body)
}

fn extract_api_markdown(spec_markdown: &str) -> Option<String> {
    let heading = "## API Entry Points";
    let start = spec_markdown.find(heading)?;
    let after_heading = &spec_markdown[start + heading.len()..];
    let after_trimmed = after_heading.trim_start_matches(['\n', '\r']);
    if after_trimmed.is_empty() {
        return None;
    }
    let next_heading_offset = after_trimmed.find("\n## ");
    let content = if let Some(idx) = next_heading_offset {
        &after_trimmed[..idx]
    } else {
        after_trimmed
    };
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn build_combine_specs_prompt(project_locations: &[String], specs: &[SpecEntry]) -> String {
    let locations_block = if project_locations.is_empty() {
        "repository root".to_string()
    } else {
        project_locations.join("\n")
    };

    let mut spec_block = String::new();
    for entry in specs {
        spec_block.push_str(&format!("## {}\n\n", entry.location_label));
        spec_block.push_str(entry.markdown.trim());
        spec_block.push_str("\n\n---\n\n");
    }

    SPEC_COMBINE_PROMPT_TEMPLATE
        .replace("{project_locations}", &locations_block)
        .replace("{spec_drafts}", spec_block.trim())
        .replace("{combined_template}", SPEC_COMBINED_MARKDOWN_TEMPLATE)
}

fn slugify_label(input: &str) -> String {
    let mut slug = String::new();
    let mut needs_separator = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if needs_separator && !slug.is_empty() {
                slug.push('_');
            }
            slug.push(ch.to_ascii_lowercase());
            needs_separator = false;
        } else if matches!(ch, '/' | '\\' | '-' | '_') || ch.is_whitespace() {
            needs_separator = !slug.is_empty();
        }
    }
    if slug.is_empty() {
        "spec".to_string()
    } else {
        slug.trim_matches('_').to_string()
    }
}

fn build_fix_markdown_prompt(original_content: &str, template_hint: Option<&str>) -> String {
    let mut prompt = String::from(
        "Read the report below and fix the formatting issues. Write the corrected version as the output.\n\
Make sure it looks professional and polished, but still concise and to the point.\n\n\
Some common issues to fix:\n\
- Unicode bullet points: •\n\
- Extra backticks around code blocks (``` markers)\n\
- Mermaid diagrams: nodes with unescaped characters like () or []\n\
- Incorrect number continuation (e.g. 1. 1. 1.)\n",
    );
    if let Some(template) = template_hint {
        prompt
            .push_str("\nWhen fixing, ensure the output conforms to this template:\n<template>\n");
        prompt.push_str(template);
        prompt.push_str("\n</template>\n");
    }
    prompt.push_str("\nOriginal Report:\n<original_report>\n");
    prompt.push_str(original_content);
    prompt.push_str(
        "\n</original_report>\n\n# Output\n- A valid markdown report\n\n# Important:\n- Do not add emojis, or any filler text in the output.\n- Do not add AI summary or thinking process in the output (usually at the beginning or end of the response)\n- Do not remove, rewrite, or replace any image/GIF/video embeds. If the input contains media embeds (e.g., ![alt](path) or <img> or <video>), preserve them exactly as-is, including their paths and alt text.\n- Do not insert any placeholder or disclaimer text about media not being included or omitted. If the media path looks local or absolute, keep it; do not change or comment on it.\n- Do not remove any existing formatting, like bold/italic/underline/code/etc.\n",
    );
    prompt.push_str(MARKDOWN_OUTPUT_GUARD);
    prompt
}

fn clamp_prompt_text(input: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(input.len().min(max_chars) + 32);
    let mut count = 0usize;
    for ch in input.chars() {
        if count >= max_chars {
            out.push_str("\n… (truncated)");
            break;
        }
        out.push(ch);
        count += 1;
    }
    if count < input.chars().count() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[derive(Clone)]
struct ClassificationExtraction {
    rows: Vec<DataClassificationRow>,
    table_markdown: String,
    reasoning_logs: Vec<String>,
}

fn build_data_classification_prompt(spec_markdown: &str) -> String {
    CONVERT_CLASSIFICATION_TO_JSON_PROMPT_TEMPLATE.replace("{spec_markdown}", spec_markdown)
}

fn sensitivity_rank(value: &str) -> usize {
    match value.trim().to_ascii_lowercase().as_str() {
        "high" => 0,
        "medium" => 1,
        "low" => 2,
        _ => 3,
    }
}

fn build_data_classification_table(rows: &[DataClassificationRow]) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    let mut sorted = rows.to_vec();
    sorted.sort_by(|a, b| {
        let rank_a = sensitivity_rank(&a.sensitivity);
        let rank_b = sensitivity_rank(&b.sensitivity);
        rank_a.cmp(&rank_b).then_with(|| {
            a.data_type
                .to_ascii_lowercase()
                .cmp(&b.data_type.to_ascii_lowercase())
        })
    });

    let mut lines: Vec<String> = vec![
        "## Data Classification".to_string(),
        String::new(),
        "| Data Type | Sensitivity | Storage Location | Retention | Encryption At Rest | In Transit | Accessed By |".to_string(),
        "|---|---|---|---|---|---|---|".to_string(),
    ];
    for row in &sorted {
        lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} |",
            row.data_type,
            row.sensitivity,
            row.storage_location,
            row.retention,
            row.encryption_at_rest,
            row.in_transit,
            row.accessed_by
        ));
    }
    lines.push(String::new());
    Some(lines.join("\n"))
}

fn inject_data_classification_section(spec_markdown: &str, table_markdown: &str) -> String {
    let lines: Vec<&str> = spec_markdown.lines().collect();
    let mut output: Vec<String> = Vec::new();
    let mut i = 0usize;
    let mut replaced = false;
    while i < lines.len() {
        let line = lines[i];
        if line
            .trim()
            .to_ascii_lowercase()
            .starts_with("## data classification")
        {
            replaced = true;
            output.push(table_markdown.to_string());
            i += 1;
            while i < lines.len() {
                let candidate = lines[i];
                if candidate.starts_with("## ")
                    && !candidate
                        .trim()
                        .eq_ignore_ascii_case("## Data Classification")
                {
                    break;
                }
                if candidate.starts_with("# ")
                    && !candidate
                        .trim()
                        .eq_ignore_ascii_case("# Project Specification")
                {
                    break;
                }
                i += 1;
            }
            continue;
        }
        output.push(line.to_string());
        i += 1;
    }

    if !replaced {
        if let Some(last) = output.last()
            && !last.trim().is_empty()
        {
            output.push(String::new());
        }
        output.push(table_markdown.to_string());
    }

    output.join("\n")
}

async fn extract_data_classification(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    spec_markdown: &str,
    metrics: Arc<ReviewMetrics>,
) -> Result<Option<ClassificationExtraction>, String> {
    let trimmed = spec_markdown.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let truncated_spec = clamp_prompt_text(trimmed, CLASSIFICATION_PROMPT_SPEC_LIMIT);
    let prompt = build_data_classification_prompt(&truncated_spec);

    let output = call_model(
        client,
        provider,
        auth,
        SPEC_GENERATION_MODEL,
        SPEC_SYSTEM_PROMPT,
        &prompt,
        metrics,
        0.0,
    )
    .await?;

    let mut reasoning_logs: Vec<String> = Vec::new();
    if let Some(reasoning) = output.reasoning.as_ref() {
        log_model_reasoning(reasoning, &None, &None, &mut reasoning_logs);
    }

    let mut rows: Vec<DataClassificationRow> = Vec::new();
    for raw in output.text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<DataClassificationRow>(trimmed) {
            Ok(mut row) => {
                row.sensitivity = row.sensitivity.trim().to_ascii_lowercase();
                if row.sensitivity != "high"
                    && row.sensitivity != "medium"
                    && row.sensitivity != "low"
                {
                    row.sensitivity = "unknown".to_string();
                }
                rows.push(row);
            }
            Err(err) => {
                reasoning_logs.push(format!(
                    "Skipping invalid classification line: {trimmed} ({err})"
                ));
            }
        }
    }

    if rows.is_empty() {
        return Ok(None);
    }

    let table_markdown = match build_data_classification_table(&rows) {
        Some(table) => table,
        None => return Ok(None),
    };

    Ok(Some(ClassificationExtraction {
        rows,
        table_markdown,
        reasoning_logs,
    }))
}

struct MarkdownPolishOutcome {
    text: String,
    reasoning_logs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct BugAnalysisProgress {
    version: i32,
    pass: i32,
    total_files: i32,
    files: Vec<BugAnalysisProgressFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct BugAnalysisProgressFile {
    index: i32,
    path_display: String,
    relative_path: String,
    findings_count: i32,
    duration_ms: i32,
    bug_section: Option<String>,
}

impl BugAnalysisProgress {
    fn new(pass: i32, total_files: usize) -> Self {
        Self {
            version: 1,
            pass,
            total_files: total_files.try_into().unwrap_or(i32::MAX),
            files: Vec::new(),
        }
    }

    fn upsert_file(&mut self, file: BugAnalysisProgressFile) {
        if let Some(existing) = self
            .files
            .iter_mut()
            .find(|existing| existing.index == file.index)
        {
            *existing = file;
            return;
        }
        self.files.push(file);
    }
}

async fn read_bug_analysis_progress(
    path: &Path,
) -> Result<Option<BugAnalysisProgress>, SecurityReviewFailure> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = tokio_fs::read(path)
        .await
        .map_err(|err| SecurityReviewFailure {
            message: format!(
                "Failed to read bug analysis progress {}: {err}",
                path.display()
            ),
            logs: vec![format!(
                "Failed to read bug analysis progress {}: {err}",
                path.display()
            )],
        })?;
    serde_json::from_slice::<BugAnalysisProgress>(&bytes)
        .map(Some)
        .map_err(|err| SecurityReviewFailure {
            message: format!(
                "Failed to parse bug analysis progress {}: {err}",
                path.display()
            ),
            logs: vec![format!(
                "Failed to parse bug analysis progress {}: {err}",
                path.display()
            )],
        })
}

async fn write_bug_analysis_progress(
    path: &Path,
    progress: &BugAnalysisProgress,
) -> Result<(), SecurityReviewFailure> {
    if let Some(parent) = path.parent() {
        tokio_fs::create_dir_all(parent)
            .await
            .map_err(|err| SecurityReviewFailure {
                message: format!(
                    "Failed to create bug analysis progress directory {}: {err}",
                    parent.display()
                ),
                logs: vec![format!(
                    "Failed to create bug analysis progress directory {}: {err}",
                    parent.display()
                )],
            })?;
    }

    let bytes = serde_json::to_vec_pretty(progress).map_err(|err| SecurityReviewFailure {
        message: format!(
            "Failed to serialize bug analysis progress {}: {err}",
            path.display()
        ),
        logs: vec![format!(
            "Failed to serialize bug analysis progress {}: {err}",
            path.display()
        )],
    })?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("bug_analysis_progress.json");
    let tmp_path = path.with_file_name(format!("{file_name}.tmp"));
    let _ = tokio_fs::remove_file(&tmp_path).await;
    tokio_fs::write(&tmp_path, bytes)
        .await
        .map_err(|err| SecurityReviewFailure {
            message: format!(
                "Failed to write bug analysis progress {}: {err}",
                tmp_path.display()
            ),
            logs: vec![format!(
                "Failed to write bug analysis progress {}: {err}",
                tmp_path.display()
            )],
        })?;

    match tokio_fs::rename(&tmp_path, path).await {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = tokio_fs::remove_file(path).await;
            tokio_fs::rename(&tmp_path, path)
                .await
                .map_err(|second_err| SecurityReviewFailure {
                    message: format!(
                        "Failed to replace bug analysis progress {}: {err}; retry failed: {second_err}",
                        path.display()
                    ),
                    logs: vec![format!(
                        "Failed to replace bug analysis progress {}: {err}; retry failed: {second_err}",
                        path.display()
                    )],
                })?;
            Ok(())
        }
    }
}

async fn polish_markdown_block(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    metrics: Arc<ReviewMetrics>,
    original_content: &str,
    template_hint: Option<&str>,
) -> Result<MarkdownPolishOutcome, String> {
    if original_content.trim().is_empty() {
        return Ok(MarkdownPolishOutcome {
            text: original_content.to_string(),
            reasoning_logs: Vec::new(),
        });
    }

    let fix_prompt = build_fix_markdown_prompt(original_content, template_hint);
    let output = call_model(
        client,
        provider,
        auth,
        MARKDOWN_FIX_MODEL,
        MARKDOWN_FIX_SYSTEM_PROMPT,
        &fix_prompt,
        metrics,
        0.0,
    )
    .await?;

    let mut reasoning_logs: Vec<String> = Vec::new();
    if let Some(reasoning) = output.reasoning.as_ref() {
        log_model_reasoning(reasoning, &None, &None, &mut reasoning_logs);
    }

    Ok(MarkdownPolishOutcome {
        text: output.text,
        reasoning_logs,
    })
}

#[allow(clippy::too_many_arguments)]
async fn analyze_files_individually(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    config: &Config,
    auth_manager: Arc<AuthManager>,
    repository_summary: &str,
    spec_markdown: Option<&str>,
    scope_prompt: Option<&str>,
    output_root: &Path,
    pass: usize,
    repo_root: &Path,
    snippets: &[FileSnippet],
    git_link_info: Option<GitLinkInfo>,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
) -> Result<BugAnalysisOutcome, SecurityReviewFailure> {
    let mut aggregated_logs: Vec<String> = Vec::new();
    let mut sections: Vec<(usize, String)> = Vec::new();
    let mut snippets_with_findings: Vec<(usize, FileSnippet)> = Vec::new();
    let mut findings_count = 0usize;
    let mut per_file_durations: Vec<(String, Duration, usize)> = Vec::new();
    let mut bug_details: Vec<BugDetail> = Vec::new();
    let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
    let total_files = snippets.len();
    let pass_label: i32 = pass.try_into().unwrap_or(1);
    let progress_path = output_root
        .join("context")
        .join(format!("bug_analysis_pass_{pass_label}.json"));

    let mut progress = match read_bug_analysis_progress(&progress_path).await {
        Ok(Some(existing))
            if existing.version == 1
                && existing.pass == pass_label
                && existing.total_files == total_files.try_into().unwrap_or(i32::MAX) =>
        {
            existing
        }
        Ok(Some(_)) => BugAnalysisProgress::new(pass_label, total_files),
        Ok(None) => BugAnalysisProgress::new(pass_label, total_files),
        Err(err) => {
            aggregated_logs.push(err.message.clone());
            aggregated_logs.extend(err.logs);
            BugAnalysisProgress::new(pass_label, total_files)
        }
    };

    let mut completed_indices: HashSet<usize> = HashSet::new();
    for entry in &progress.files {
        let Ok(index) = usize::try_from(entry.index) else {
            continue;
        };
        if index >= total_files {
            continue;
        }
        if !completed_indices.insert(index) {
            continue;
        }

        let findings_for_file = usize::try_from(entry.findings_count.max(0)).unwrap_or(0);
        findings_count = findings_count.saturating_add(findings_for_file);

        if let Some(section) = entry.bug_section.as_ref() {
            sections.push((index, section.clone()));
            snippets_with_findings.push((index, snippets[index].clone()));
        }

        let duration = Duration::from_millis(entry.duration_ms.max(0) as u64);
        per_file_durations.push((entry.path_display.clone(), duration, findings_for_file));
    }

    let mut completed_files: usize = completed_indices.len();
    if completed_files > 0 {
        aggregated_logs.push(format!(
            "Resuming bug analysis pass {pass_label}: skipping {completed_files}/{total_files} previously analyzed file(s)."
        ));
    }

    // Ensure progress is on disk even if the run crashes before the first file completes.
    write_bug_analysis_progress(&progress_path, &progress).await?;

    let mut remaining = snippets
        .iter()
        .enumerate()
        .filter(|(index, _)| !completed_indices.contains(index));

    let pending_files = total_files.saturating_sub(completed_files);
    let concurrency = MAX_CONCURRENT_FILE_ANALYSIS.min(pending_files);
    if concurrency > 0 {
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(format!(
                "  └ Launching parallel bug analysis ({concurrency} workers)"
            )));
        }
        for _ in 0..concurrency {
            if let Some((index, snippet)) = remaining.next() {
                in_flight.push(analyze_single_file(
                    client,
                    provider,
                    auth,
                    model,
                    config,
                    auth_manager.clone(),
                    repository_summary,
                    spec_markdown,
                    scope_prompt,
                    repo_root,
                    snippet.clone(),
                    index,
                    snippets.len(),
                    progress_sender.clone(),
                    log_sink.clone(),
                    metrics.clone(),
                ));
            }
        }
    }

    while let Some(result) = in_flight.next().await {
        match result {
            Ok(file_result) => {
                let FileBugResult {
                    index,
                    path_display,
                    duration,
                    logs,
                    bug_section,
                    snippet,
                    findings_count: file_findings_count,
                } = file_result;

                aggregated_logs.extend(logs);
                findings_count = findings_count.saturating_add(file_findings_count);
                if let Some(section) = bug_section.as_ref() {
                    sections.push((index, section.clone()));
                }
                if let Some(snippet) = snippet {
                    snippets_with_findings.push((index, snippet));
                }
                per_file_durations.push((path_display.clone(), duration, file_findings_count));
                completed_files = completed_files.saturating_add(1);

                let duration_ms_u128 = duration.as_millis().min(i32::MAX as u128);
                let progress_file = BugAnalysisProgressFile {
                    index: index.try_into().unwrap_or(i32::MAX),
                    path_display: path_display.clone(),
                    relative_path: snippets[index].relative_path.display().to_string(),
                    findings_count: file_findings_count.try_into().unwrap_or(i32::MAX),
                    duration_ms: duration_ms_u128.try_into().unwrap_or(i32::MAX),
                    bug_section,
                };
                progress.upsert_file(progress_file);
                write_bug_analysis_progress(&progress_path, &progress).await?;

                if let Some(tx) = progress_sender.as_ref() {
                    let percent = if total_files == 0 {
                        0
                    } else {
                        (completed_files * 100) / total_files
                    };
                    tx.send(AppEvent::SecurityReviewLog(format!(
                        "Bug analysis progress: {}/{} - {percent}%.",
                        completed_files.min(total_files),
                        total_files
                    )));
                }
                if let Some((index, snippet)) = remaining.next() {
                    in_flight.push(analyze_single_file(
                        client,
                        provider,
                        auth,
                        model,
                        config,
                        auth_manager.clone(),
                        repository_summary,
                        spec_markdown,
                        scope_prompt,
                        repo_root,
                        snippet.clone(),
                        index,
                        snippets.len(),
                        progress_sender.clone(),
                        log_sink.clone(),
                        metrics.clone(),
                    ));
                }
            }
            Err(failure) => {
                if let Some(tx) = progress_sender.as_ref() {
                    for line in &failure.logs {
                        tx.send(AppEvent::SecurityReviewLog(line.clone()));
                    }
                }
                let mut combined_logs = aggregated_logs;
                combined_logs.extend(failure.logs);
                return Err(SecurityReviewFailure {
                    message: failure.message,
                    logs: combined_logs,
                });
            }
        }
    }

    if sections.is_empty() {
        aggregated_logs.push("All analyzed files reported no bugs.".to_string());
        if !per_file_durations.is_empty() {
            per_file_durations.sort_by_key(|(_, duration, _)| *duration);
            let slowest = per_file_durations
                .iter()
                .rev()
                .take(3)
                .map(|(path, duration, _)| format!("{path}: {:.1}s", duration.as_secs_f32()))
                .collect::<Vec<_>>()
                .join(", ");
            aggregated_logs.push(format!(
                "Bug analysis timing (slowest {}): {}",
                per_file_durations.len().min(3),
                slowest
            ));
        }
        return Ok(BugAnalysisOutcome {
            bug_markdown: "no bugs found".to_string(),
            bug_summary_table: None,
            findings_count: 0,
            bug_summaries: Vec::new(),
            bug_details: Vec::new(),
            files_with_findings: Vec::new(),
            logs: aggregated_logs,
        });
    }

    if !per_file_durations.is_empty() {
        per_file_durations.sort_by_key(|(_, duration, _)| *duration);
        let slowest = per_file_durations
            .iter()
            .rev()
            .take(5)
            .map(|(path, duration, count)| {
                format!(
                    "{path}: {:.1}s ({} finding{})",
                    duration.as_secs_f32(),
                    count,
                    if *count == 1 { "" } else { "s" }
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        aggregated_logs.push(format!(
            "Bug analysis timing (slowest {}): {}",
            per_file_durations.len().min(5),
            slowest
        ));
        let avg_secs: f32 = per_file_durations
            .iter()
            .map(|(_, duration, _)| duration.as_secs_f32())
            .sum::<f32>()
            / per_file_durations.len().max(1) as f32;
        aggregated_logs.push(format!(
            "Bug analysis timing: avg {:.1}s over {} file(s).",
            avg_secs,
            per_file_durations.len()
        ));
    }

    sections.sort_by_key(|(index, _)| *index);
    let mut bug_summaries: Vec<BugSummary> = Vec::new();
    let mut next_summary_id: usize = 1;
    for (idx, section) in sections.into_iter() {
        let file_path = snippets[idx].relative_path.display().to_string();
        let (mut summaries, mut details) = extract_bug_summaries(
            &section,
            &file_path,
            snippets[idx].relative_path.as_path(),
            &mut next_summary_id,
        );
        bug_details.append(&mut details);
        bug_summaries.append(&mut summaries);
    }

    if let Some(info) = git_link_info.as_ref()
        && !bug_summaries.is_empty()
    {
        let blame_logs =
            enrich_bug_summaries_with_blame(&mut bug_summaries, info, metrics.clone()).await;
        if let Some(tx) = progress_sender.as_ref() {
            for line in &blame_logs {
                tx.send(AppEvent::SecurityReviewLog(line.clone()));
            }
        }
        aggregated_logs.extend(blame_logs);
    }

    // Normalize severities before filtering/dedup so ranking is consistent
    for summary in bug_summaries.iter_mut() {
        if let Some(normalized) = normalize_severity_label(&summary.severity) {
            summary.severity = normalized;
        } else {
            summary.severity = summary.severity.trim().to_string();
        }
    }

    if !bug_summaries.is_empty() {
        let mut replacements: HashMap<usize, String> = HashMap::new();
        for summary in bug_summaries.iter_mut() {
            if let Some(updated) =
                rewrite_bug_markdown_severity(summary.markdown.as_str(), summary.severity.as_str())
            {
                summary.markdown = updated.clone();
                replacements.insert(summary.id, updated);
            }
            if let Some(updated) =
                rewrite_bug_markdown_heading_id(summary.markdown.as_str(), summary.id)
            {
                summary.markdown = updated.clone();
                replacements.insert(summary.id, updated);
            }
        }
        if !replacements.is_empty() {
            for detail in bug_details.iter_mut() {
                if let Some(markdown) = replacements.get(&detail.summary_id) {
                    detail.original_markdown = markdown.clone();
                }
            }
        }
    }

    let original_summary_count = bug_summaries.len();
    let mut retained_ids: HashSet<usize> = HashSet::new();
    bug_summaries.retain(|summary| {
        let keep = matches!(
            summary.severity.trim().to_ascii_lowercase().as_str(),
            "high" | "medium" | "low"
        );
        if keep {
            retained_ids.insert(summary.id);
        }
        keep
    });
    bug_details.retain(|detail| retained_ids.contains(&detail.summary_id));
    if bug_summaries.len() < original_summary_count {
        let filtered = original_summary_count - bug_summaries.len();
        aggregated_logs.push(format!(
            "Filtered out {filtered} informational finding{}.",
            if filtered == 1 { "" } else { "s" }
        ));
    }
    if bug_summaries.is_empty() {
        aggregated_logs
            .push("No high, medium, or low severity findings remain after filtering.".to_string());
    }

    // Deduplicate/group similar findings (e.g., duplicate issues across files)
    if !bug_summaries.is_empty() {
        let (deduped_summaries, deduped_details, removed) =
            dedupe_bug_summaries(bug_summaries, bug_details);
        bug_summaries = deduped_summaries;
        bug_details = deduped_details;
        if removed > 0 {
            aggregated_logs.push(format!(
                "Deduplicated {removed} duplicated finding{} by grouping titles/tags.",
                if removed == 1 { "" } else { "s" }
            ));
        }
    }

    // Now run risk rerank on the deduplicated set
    if !bug_summaries.is_empty() {
        let risk_logs = rerank_bugs_by_risk(
            client,
            provider,
            auth,
            model,
            &mut bug_summaries,
            repo_root,
            repository_summary,
            spec_markdown,
            metrics.clone(),
        )
        .await;
        aggregated_logs.extend(risk_logs);
    }

    // Normalize again and rewrite markdown severities post-rerank,
    // then filter once more in case severities changed to informational.
    if !bug_summaries.is_empty() {
        for summary in bug_summaries.iter_mut() {
            if let Some(normalized) = normalize_severity_label(&summary.severity) {
                summary.severity = normalized;
            } else {
                summary.severity = summary.severity.trim().to_string();
            }
        }
        let mut replacements: HashMap<usize, String> = HashMap::new();
        for summary in bug_summaries.iter_mut() {
            if let Some(updated) =
                rewrite_bug_markdown_severity(summary.markdown.as_str(), summary.severity.as_str())
            {
                summary.markdown = updated.clone();
                replacements.insert(summary.id, updated);
            }
            if let Some(updated) =
                rewrite_bug_markdown_heading_id(summary.markdown.as_str(), summary.id)
            {
                summary.markdown = updated.clone();
                replacements.insert(summary.id, updated);
            }
        }
        if !replacements.is_empty() {
            for detail in bug_details.iter_mut() {
                if let Some(markdown) = replacements.get(&detail.summary_id) {
                    detail.original_markdown = markdown.clone();
                }
            }
        }

        let before = bug_summaries.len();
        let mut retained: HashSet<usize> = HashSet::new();
        bug_summaries.retain(|summary| {
            let keep = matches!(
                summary.severity.trim().to_ascii_lowercase().as_str(),
                "high" | "medium" | "low"
            );
            if keep {
                retained.insert(summary.id);
            }
            keep
        });
        bug_details.retain(|detail| retained.contains(&detail.summary_id));
        let after = bug_summaries.len();
        if after < before {
            aggregated_logs.push(format!(
                "Filtered out {} informational finding{} after rerank.",
                before - after,
                if (before - after) == 1 { "" } else { "s" }
            ));
        }

        normalize_bug_identifiers(&mut bug_summaries, &mut bug_details);
    }

    snippets_with_findings.sort_by_key(|(index, _)| *index);
    let allowed_paths: HashSet<PathBuf> = bug_summaries
        .iter()
        .map(|summary| summary.source_path.clone())
        .collect();
    let files_with_findings = snippets_with_findings
        .into_iter()
        .map(|(_, snippet)| snippet)
        .filter(|snippet| allowed_paths.contains(&snippet.relative_path))
        .collect::<Vec<_>>();

    let findings_count = bug_summaries.len();

    let bug_markdown = if bug_summaries.is_empty() {
        "No high, medium, or low severity findings.".to_string()
    } else {
        bug_summaries
            .iter()
            .map(|summary| summary.markdown.clone())
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    aggregated_logs.push(format!(
        "Aggregated bug findings across {} file(s).",
        files_with_findings.len()
    ));

    Ok(BugAnalysisOutcome {
        bug_markdown,
        bug_summary_table: make_bug_summary_table(&bug_summaries),
        findings_count,
        bug_summaries,
        bug_details,
        files_with_findings,
        logs: aggregated_logs,
    })
}

#[allow(clippy::too_many_arguments)]
async fn analyze_single_file(
    _client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    _auth: &Option<CodexAuth>,
    model: &str,
    config: &Config,
    auth_manager: Arc<AuthManager>,
    repository_summary: &str,
    spec_markdown: Option<&str>,
    scope_prompt: Option<&str>,
    repo_root: &Path,
    snippet: FileSnippet,
    index: usize,
    total_files: usize,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
) -> Result<FileBugResult, SecurityReviewFailure> {
    let started_at = Instant::now();
    let mut logs = Vec::new();
    let path_display = snippet.relative_path.display().to_string();
    let file_size = human_readable_bytes(snippet.bytes);
    let prefix = format!("{}/{}", index + 1, total_files);
    let start_message = format!("Analyzing file {prefix}: {path_display} ({file_size}).");
    push_progress_log(&progress_sender, &log_sink, &mut logs, start_message);

    let base_context = build_single_file_context(&snippet);
    let prompt_data = build_bugs_user_prompt(
        repository_summary,
        spec_markdown,
        &base_context,
        scope_prompt,
    );
    for line in &prompt_data.logs {
        push_progress_log(&progress_sender, &log_sink, &mut logs, line.clone());
    }

    let outcome = match run_bug_agent(
        config,
        provider,
        auth_manager,
        repo_root,
        prompt_data.prompt.clone(),
        progress_sender.clone(),
        log_sink.clone(),
        metrics,
        model,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(agent_failure) => {
            logs.extend(agent_failure.logs);
            let message = format!(
                "Bug agent loop failed for {path_display}: {}",
                agent_failure.message
            );
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(SecurityReviewFailure { message, logs });
        }
    };

    logs.extend(outcome.logs);
    let trimmed = outcome.section.trim();
    if trimmed.is_empty() {
        let warn = format!(
            "Bug agent returned an empty response for {path_display}; treating as no findings."
        );
        push_progress_log(&progress_sender, &log_sink, &mut logs, warn.clone());
        logs.push(warn);
        return Ok(FileBugResult {
            index,
            path_display,
            duration: started_at.elapsed(),
            logs,
            bug_section: None,
            snippet: None,
            findings_count: 0,
        });
    }
    if trimmed.eq_ignore_ascii_case("no bugs found") {
        let message = format!("No bugs found in {path_display}.");
        push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
        logs.push(message);
        return Ok(FileBugResult {
            index,
            path_display,
            duration: started_at.elapsed(),
            logs,
            bug_section: None,
            snippet: None,
            findings_count: 0,
        });
    }

    let file_findings = trimmed
        .lines()
        .filter(|line| line.trim_start().starts_with("### "))
        .count();
    let message = if file_findings == 0 {
        format!("Recorded findings for {path_display}.")
    } else {
        let plural = if file_findings == 1 { "" } else { "s" };
        format!("Recorded {file_findings} finding{plural} for {path_display}.")
    };
    push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
    logs.push(message);
    Ok(FileBugResult {
        index,
        path_display,
        duration: started_at.elapsed(),
        logs,
        bug_section: Some(outcome.section),
        snippet: Some(snippet),
        findings_count: file_findings,
    })
}

async fn run_content_search(
    repo_root: &Path,
    pattern: &str,
    mode: SearchMode,
    metrics: &Arc<ReviewMetrics>,
) -> SearchResult {
    if pattern.is_empty() || pattern.len() > MAX_SEARCH_PATTERN_LEN {
        return SearchResult::NoMatches;
    }

    let mut current_mode = mode;
    let mut allow_regex_fallback = true;

    loop {
        metrics.record_tool_call(ToolCallKind::Search);
        metrics.record_tool_call(ToolCallKind::Exec);

        let mut command = Command::new("rg");
        command
            .arg("--max-count")
            .arg("20")
            .arg("--with-filename")
            .arg("--color")
            .arg("never")
            .arg("--line-number");

        if matches!(current_mode, SearchMode::Literal) {
            command.arg("--fixed-strings");
        }

        if pattern.contains('\n') {
            command.arg("--multiline");
            command.arg("--multiline-dotall");
        }

        command.arg(pattern).current_dir(repo_root);

        let output = match command.output().await {
            Ok(output) => output,
            Err(err) => {
                return SearchResult::Error(format!("failed to run rg: {err}"));
            }
        };

        match output.status.code() {
            Some(0) => {
                let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if text.is_empty() {
                    return SearchResult::NoMatches;
                }
                if text.len() > MAX_SEARCH_OUTPUT_CHARS {
                    let mut boundary = MAX_SEARCH_OUTPUT_CHARS;
                    while boundary > 0 && !text.is_char_boundary(boundary) {
                        boundary -= 1;
                    }
                    text.truncate(boundary);
                    text.push_str("\n... (truncated)");
                }
                return SearchResult::Matches(text);
            }
            Some(1) => return SearchResult::NoMatches,
            _ => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    return SearchResult::Error("rg returned an error".to_string());
                }
                if allow_regex_fallback
                    && matches!(current_mode, SearchMode::Regex)
                    && is_regex_parse_error(&stderr)
                {
                    current_mode = SearchMode::Literal;
                    allow_regex_fallback = false;
                    continue;
                }
                return SearchResult::Error(format!("rg error: {stderr}"));
            }
        }
    }
}

fn is_regex_parse_error(stderr: &str) -> bool {
    let lowered = stderr.to_ascii_lowercase();
    lowered.contains("regex parse error") || lowered.contains("error parsing regex")
}

fn extract_bug_summaries(
    markdown: &str,
    default_path: &str,
    source_path: &Path,
    next_id: &mut usize,
) -> (Vec<BugSummary>, Vec<BugDetail>) {
    let mut summaries: Vec<BugSummary> = Vec::new();
    let mut details: Vec<BugDetail> = Vec::new();
    let mut current: Option<BugSummary> = None;
    let mut current_lines: Vec<String> = Vec::new();

    let finalize_current = |current: &mut Option<BugSummary>,
                            lines: &mut Vec<String>,
                            summaries: &mut Vec<BugSummary>,
                            details: &mut Vec<BugDetail>| {
        if let Some(mut summary) = current.take() {
            let section = lines.join("\n");
            let trimmed = section.trim().to_string();
            summary.markdown = trimmed.clone();
            details.push(BugDetail {
                summary_id: summary.id,
                original_markdown: trimmed,
            });
            summaries.push(summary);
        }
        lines.clear();
    };

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("### ") {
            finalize_current(
                &mut current,
                &mut current_lines,
                &mut summaries,
                &mut details,
            );
            let id = *next_id;
            *next_id = next_id.saturating_add(1);
            current = Some(BugSummary {
                id,
                title: trimmed.trim_start_matches("### ").trim().to_string(),
                file: default_path.to_string(),
                severity: String::new(),
                impact: String::new(),
                likelihood: String::new(),
                recommendation: String::new(),
                blame: None,
                risk_score: None,
                risk_rank: None,
                risk_reason: None,
                verification_types: Vec::new(),
                vulnerability_tag: None,
                validation: BugValidationState::default(),
                source_path: source_path.to_path_buf(),
                markdown: String::new(),
                author_github: None,
            });
            current_lines.push(line.to_string());
            continue;
        }

        if current.is_none() {
            continue;
        }

        current_lines.push(line.to_string());

        if let Some(summary) = current.as_mut() {
            if let Some(rest) = trimmed.strip_prefix("- **File & Lines:**") {
                let value = rest.trim().trim_matches('`').to_string();
                if !value.is_empty() {
                    summary.file = value;
                }
            } else if let Some(rest) = trimmed.strip_prefix("- **Severity:**") {
                summary.severity = rest.trim().to_string();
            } else if let Some(rest) = trimmed.strip_prefix("- **Impact:**") {
                summary.impact = rest.trim().to_string();
            } else if let Some(rest) = trimmed.strip_prefix("- **Likelihood:**") {
                summary.likelihood = rest.trim().to_string();
            } else if let Some(rest) = trimmed.strip_prefix("- **Recommendation:**") {
                summary.recommendation = rest.trim().to_string();
            } else if let Some(rest) = trimmed.strip_prefix("- **Verification Type:**") {
                let value = rest.trim();
                if !value.is_empty()
                    && let Ok(vec) = serde_json::from_str::<Vec<String>>(value)
                {
                    summary.verification_types = vec
                        .into_iter()
                        .map(|entry| entry.trim().to_string())
                        .filter(|entry| !entry.is_empty())
                        .collect();
                }
            } else if let Some(rest) = trimmed.strip_prefix("- TAXONOMY:") {
                let value = rest.trim();
                if !value.is_empty()
                    && let Ok(taxonomy) = serde_json::from_str::<Value>(value)
                    && let Some(tag) = taxonomy
                        .get("vuln_tag")
                        .and_then(Value::as_str)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                {
                    summary.vulnerability_tag = Some(tag);
                }
            }
        }
    }

    finalize_current(
        &mut current,
        &mut current_lines,
        &mut summaries,
        &mut details,
    );

    (summaries, details)
}

fn normalize_title_key(title: &str) -> String {
    let mut s = title.trim().to_ascii_lowercase();
    if let Some((head, _)) = s.rsplit_once(" in ") {
        let tail = s.split(" in ").last().unwrap_or("");
        if tail.contains('.') || tail.contains('/') {
            s = head.trim().to_string();
        }
    }
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn rewrite_bug_markdown_location(markdown: &str, new_location: &str) -> Option<String> {
    if markdown.trim().is_empty() {
        return None;
    }
    let mut lines: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in markdown.lines() {
        let trimmed = line.trim();
        if !replaced && trimmed.starts_with("- **File & Lines:**") {
            lines.push(format!("- **File & Lines:** `{new_location}`"));
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !replaced {
        let mut out: Vec<String> = Vec::new();
        let mut inserted = false;
        for line in markdown.lines() {
            out.push(line.to_string());
            if !inserted && line.trim_start().starts_with("### ") {
                out.push(format!("- **File & Lines:** `{new_location}`"));
                inserted = true;
            }
        }
        return Some(out.join("\n"));
    }
    Some(lines.join("\n"))
}

fn dedupe_bug_summaries(
    mut summaries: Vec<BugSummary>,
    details: Vec<BugDetail>,
) -> (Vec<BugSummary>, Vec<BugDetail>, usize) {
    if summaries.is_empty() {
        return (summaries, details, 0);
    }

    let mut detail_by_id: HashMap<usize, String> = HashMap::new();
    for d in &details {
        detail_by_id.insert(d.summary_id, d.original_markdown.clone());
    }

    #[derive(Clone)]
    struct GroupAgg {
        rep_index: usize,
        file_set: Vec<String>,
        members: Vec<usize>,
    }

    let mut key_to_group: HashMap<String, GroupAgg> = HashMap::new();
    for (idx, s) in summaries.iter().enumerate() {
        let key = if let Some(tag) = s.vulnerability_tag.as_ref() {
            format!("tag::{}", tag.trim().to_ascii_lowercase())
        } else {
            format!("title::{}", normalize_title_key(&s.title))
        };
        let entry = key_to_group.entry(key).or_insert_with(|| GroupAgg {
            rep_index: idx,
            file_set: Vec::new(),
            members: Vec::new(),
        });

        let rep = &summaries[entry.rep_index];
        let rep_rank = severity_rank(&rep.severity);
        let cur_rank = severity_rank(&s.severity);
        if cur_rank < rep_rank || (cur_rank == rep_rank && s.id < rep.id) {
            entry.rep_index = idx;
        }

        let loc = s.file.trim().to_string();
        if !loc.is_empty() && !entry.file_set.iter().any(|e| e == &loc) {
            entry.file_set.push(loc);
        }
        entry.members.push(s.id);
    }

    if key_to_group.len() == summaries.len() {
        return (summaries, details, 0);
    }

    let mut keep_ids: HashSet<usize> = HashSet::new();
    let id_to_index: HashMap<usize, usize> = summaries
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id, i))
        .collect();
    for agg in key_to_group.values() {
        let rep_id = summaries[agg.rep_index].id;
        let location_joined = if agg.file_set.is_empty() {
            summaries[agg.rep_index].file.clone()
        } else {
            agg.file_set.join(", ")
        };

        // Build merged verification types without borrowing rep mutably
        let mut types: Vec<String> = Vec::new();
        for m_id in &agg.members {
            if let Some(&i) = id_to_index.get(m_id) {
                for t in &summaries[i].verification_types {
                    if !types.iter().any(|e| e.eq_ignore_ascii_case(t)) {
                        types.push(t.clone());
                    }
                }
            }
        }

        // Pick highest severity across members
        let mut best_severity = summaries[agg.rep_index].severity.clone();
        for m_id in &agg.members {
            if let Some(&i) = id_to_index.get(m_id)
                && severity_rank(&summaries[i].severity) < severity_rank(&best_severity)
            {
                best_severity = summaries[i].severity.clone();
            }
        }

        // Now apply updates to the representative
        let rep_mut = &mut summaries[agg.rep_index];
        rep_mut.file = location_joined.clone();
        rep_mut.severity = best_severity;
        rep_mut.verification_types = types;
        if let Some(updated) = rewrite_bug_markdown_location(&rep_mut.markdown, &location_joined) {
            rep_mut.markdown = updated.clone();
            detail_by_id.insert(rep_id, updated);
        }

        keep_ids.insert(rep_id);
    }

    summaries.retain(|s| keep_ids.contains(&s.id));

    let mut new_details: Vec<BugDetail> = Vec::new();
    for id in &keep_ids {
        if let Some(markdown) = detail_by_id.get(id) {
            new_details.push(BugDetail {
                summary_id: *id,
                original_markdown: markdown.clone(),
            });
        }
    }

    let removed = details.len().saturating_sub(new_details.len());
    (summaries, new_details, removed)
}

fn bug_summary_cmp(a: &BugSummary, b: &BugSummary) -> CmpOrdering {
    match (a.risk_rank, b.risk_rank) {
        (Some(ra), Some(rb)) => ra.cmp(&rb),
        (Some(_), None) => CmpOrdering::Less,
        (None, Some(_)) => CmpOrdering::Greater,
        _ => severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.id.cmp(&b.id)),
    }
}

fn normalize_bug_identifiers(summaries: &mut Vec<BugSummary>, details: &mut Vec<BugDetail>) {
    if summaries.is_empty() {
        details.clear();
        return;
    }

    let mut sorted: Vec<BugSummary> = std::mem::take(summaries);
    sorted.sort_by(bug_summary_cmp);

    let mut detail_lookup: HashMap<usize, String> = details
        .iter()
        .map(|detail| (detail.summary_id, detail.original_markdown.clone()))
        .collect();
    let mut new_details: Vec<BugDetail> = Vec::with_capacity(sorted.len());

    for (index, summary) in sorted.iter_mut().enumerate() {
        let new_id = index + 1;
        let old_id = summary.id;
        summary.id = new_id;
        if let Some(updated) =
            rewrite_bug_markdown_heading_id(summary.markdown.as_str(), summary.id)
        {
            summary.markdown = updated;
        }

        let base_markdown = detail_lookup
            .remove(&old_id)
            .unwrap_or_else(|| summary.markdown.clone());
        let normalized_detail = rewrite_bug_markdown_heading_id(base_markdown.as_str(), summary.id)
            .unwrap_or(base_markdown);
        new_details.push(BugDetail {
            summary_id: summary.id,
            original_markdown: normalized_detail,
        });
    }

    *summaries = sorted;
    *details = new_details;
}

fn format_findings_summary(findings: usize, files_with_findings: usize) -> String {
    if findings == 0 {
        return "No findings identified.".to_string();
    }

    let finding_word = if findings == 1 { "finding" } else { "findings" };
    let file_word = if files_with_findings == 1 {
        "file"
    } else {
        "files"
    };
    format!("Identified {findings} {finding_word} across {files_with_findings} {file_word}.")
}

fn rewrite_bug_markdown_severity(markdown: &str, severity: &str) -> Option<String> {
    let mut changed = false;
    let mut lines: Vec<String> = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("- **Severity:**") {
            let indent_len = line.len().saturating_sub(trimmed.len());
            let indent = &line[..indent_len];
            lines.push(format!("{indent}- **Severity:** {severity}"));
            changed = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !changed {
        None
    } else {
        Some(lines.join("\n").trim().to_string())
    }
}

// Ensure bug detail heading includes the canonical summary ID and not a model-provided index
fn rewrite_bug_markdown_heading_id(markdown: &str, summary_id: usize) -> Option<String> {
    if markdown.trim().is_empty() {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    let mut changed = false;
    let mut updated_first_heading = false;
    for line in markdown.lines() {
        if !updated_first_heading {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("### ") {
                // Drop any leading bracketed id like "[12] " from the heading text
                let clean = rest
                    .trim_start()
                    .trim_start_matches('[')
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .trim_start_matches(']')
                    .trim_start();
                // Prepend an explicit anchor for stable linking
                out.push(format!("<a id=\"bug-{summary_id}\"></a>"));
                out.push(format!("### [{summary_id}] {clean}"));
                changed = true;
                updated_first_heading = true;
                continue;
            }
        }
        out.push(line.to_string());
    }
    if changed { Some(out.join("\n")) } else { None }
}

fn make_bug_summary_table(bugs: &[BugSummary]) -> Option<String> {
    if bugs.is_empty() {
        return None;
    }
    let mut ordered: Vec<&BugSummary> = bugs.iter().collect();
    ordered.sort_by(|a, b| match (a.risk_rank, b.risk_rank) {
        (Some(ra), Some(rb)) => ra.cmp(&rb),
        (Some(_), None) => CmpOrdering::Less,
        (None, Some(_)) => CmpOrdering::Greater,
        _ => severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.id.cmp(&b.id)),
    });

    let mut table = String::new();
    table.push_str("| # | Severity | Title | Validation | Impact |\n");
    table.push_str("| --- | --- | --- | --- | --- |\n");
    for (display_idx, bug) in ordered.iter().enumerate() {
        let id = display_idx + 1;
        let anchor_id = bug.id;
        let mut raw_title = sanitize_table_field(&bug.title);
        // Strip any leading bracketed numeric id from titles (e.g., "[5] Title")
        if let Some(stripped) = raw_title
            .trim_start()
            .strip_prefix('[')
            .and_then(|s| s.split_once(']'))
            .map(|(_, rest)| rest.trim_start())
        {
            raw_title = stripped.to_string();
        }
        let link_label = if raw_title == "-" {
            format!("Bug {anchor_id}")
        } else {
            raw_title.replace('[', r"\[").replace(']', r"\]")
        };
        let mut title_cell = format!("[{link_label}](#bug-{anchor_id})");
        if let Some(reason) = bug.risk_reason.as_ref() {
            let trimmed_reason = reason.trim();
            if !trimmed_reason.is_empty() {
                title_cell.push_str(" — ");
                let reason_display = sanitize_table_field(trimmed_reason);
                title_cell.push_str(&reason_display);
            }
        }
        let validation = validation_display(&bug.validation);
        table.push_str(&format!(
            "| {id} | {} | {} | {} | {} |\n",
            sanitize_table_field(&bug.severity),
            title_cell,
            sanitize_table_field(&validation),
            sanitize_table_field(&bug.impact),
        ));
    }
    Some(table)
}

fn make_bug_summary_table_from_bugs(bugs: &[SecurityReviewBug]) -> Option<String> {
    if bugs.is_empty() {
        return None;
    }
    let mut ordered: Vec<&SecurityReviewBug> = bugs.iter().collect();
    ordered.sort_by(|a, b| match (a.risk_rank, b.risk_rank) {
        (Some(ra), Some(rb)) => ra.cmp(&rb),
        (Some(_), None) => CmpOrdering::Less,
        (None, Some(_)) => CmpOrdering::Greater,
        _ => severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.summary_id.cmp(&b.summary_id)),
    });

    let mut table = String::new();
    table.push_str("| # | Severity | Title | Validation | Impact |\n");
    table.push_str("| --- | --- | --- | --- | --- |\n");
    for (display_idx, bug) in ordered.iter().enumerate() {
        let id = display_idx + 1;
        let anchor_id = bug.summary_id;
        let mut raw_title = sanitize_table_field(&bug.title);
        // Strip any leading bracketed numeric id from titles (e.g., "[5] Title")
        if let Some(stripped) = raw_title
            .trim_start()
            .strip_prefix('[')
            .and_then(|s| s.split_once(']'))
            .map(|(_, rest)| rest.trim_start())
        {
            raw_title = stripped.to_string();
        }
        let link_label = if raw_title == "-" {
            format!("Bug {anchor_id}")
        } else {
            raw_title.replace('[', r"\[").replace(']', r"\]")
        };
        let mut title_cell = format!("[{link_label}](#bug-{anchor_id})");
        if let Some(reason) = bug.risk_reason.as_ref() {
            let trimmed_reason = reason.trim();
            if !trimmed_reason.is_empty() {
                title_cell.push_str(" — ");
                let reason_display = sanitize_table_field(trimmed_reason);
                title_cell.push_str(&reason_display);
            }
        }
        let validation = validation_display(&bug.validation);
        table.push_str(&format!(
            "| {id} | {} | {} | {} | {} |\n",
            sanitize_table_field(&bug.severity),
            title_cell,
            sanitize_table_field(&validation),
            sanitize_table_field(&bug.impact),
        ));
    }
    Some(table)
}

fn validation_display(state: &BugValidationState) -> String {
    let mut label = validation_status_label(state);
    if state.status != BugValidationStatus::Pending
        && let Some(summary) = state.summary.as_ref().filter(|s| !s.is_empty())
    {
        label.push_str(" — ");
        label.push_str(&truncate_text(summary, VALIDATION_SUMMARY_GRAPHEMES));
    }
    label
}

fn validation_status_label(state: &BugValidationState) -> String {
    let mut label = match state.status {
        BugValidationStatus::Pending => "Pending".to_string(),
        BugValidationStatus::Passed => "Passed".to_string(),
        BugValidationStatus::Failed => "Failed".to_string(),
    };
    if let Some(tool) = state.tool.as_ref().filter(|tool| !tool.is_empty())
        && state.status != BugValidationStatus::Pending
    {
        label.push_str(" (");
        label.push_str(tool);
        label.push(')');
    }
    label
}

#[allow(dead_code)]
fn linkify_location(location: &str, _git_link_info: Option<&GitLinkInfo>) -> String {
    location.trim().to_string()
}

fn compile_regex(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|err| panic!("invalid regex {pattern}: {err}"))
}

fn parse_location_item(item: &str, git_link_info: &GitLinkInfo) -> Vec<(String, Option<String>)> {
    let mut results: Vec<(String, Option<String>)> = Vec::new();
    let main = item.split(" - http").next().unwrap_or(item).trim();
    if main.is_empty() {
        return results;
    }

    let path_re =
        compile_regex(r"(?P<path>[^\s,#:]+\.[A-Za-z0-9_]+)(?:[#:]?L(?P<a>\d+)(?:-L(?P<b>\d+))?)?");
    let range_tail_re = compile_regex(r"L\d+(?:-L\d+)?");

    if let Some(caps) = path_re.captures(main)
        && let Some(raw_path) = caps.name("path").map(|m| m.as_str())
        && !raw_path.starts_with("http")
        && let Some(rel_path) = to_relative_path(raw_path, git_link_info)
    {
        let mut has_range = false;
        if let Some(start_match) = caps.name("a") {
            let mut fragment = format!("L{}", start_match.as_str());
            if let Some(end_match) = caps.name("b") {
                fragment.push_str("-L");
                fragment.push_str(end_match.as_str());
            }
            results.push((rel_path.clone(), Some(fragment)));
            has_range = true;
        }
        if !has_range {
            results.push((rel_path.clone(), None));
        }
        let matched_end = caps.get(0).map(|m| m.end()).unwrap_or(0).min(main.len());
        let tail = &main[matched_end..];
        for range_match in range_tail_re.find_iter(tail) {
            results.push((rel_path.clone(), Some(range_match.as_str().to_string())));
        }
        return results;
    }

    let fallback_re = compile_regex(r"(?P<path>[^\s,#:]+\.[A-Za-z0-9_]+)#(?P<frag>L\d+(?:-L\d+)?)");
    for caps in fallback_re.captures_iter(main) {
        if let Some(raw_path) = caps.name("path").map(|m| m.as_str())
            && let Some(rel_path) = to_relative_path(raw_path, git_link_info)
        {
            let fragment = caps.name("frag").map(|m| m.as_str().to_string());
            results.push((rel_path, fragment));
        }
    }

    results
}

fn filter_location_pairs(pairs: Vec<(String, Option<String>)>) -> Vec<(String, Option<String>)> {
    if pairs.is_empty() {
        return pairs;
    }
    let mut has_range: HashMap<String, bool> = HashMap::new();
    for (path, fragment) in &pairs {
        if fragment.as_ref().is_some_and(|f| !f.is_empty()) {
            has_range.insert(path.clone(), true);
        } else {
            has_range.entry(path.clone()).or_insert(false);
        }
    }

    pairs
        .into_iter()
        .filter(|(path, fragment)| {
            if fragment.as_ref().is_some_and(|f| !f.is_empty()) {
                true
            } else {
                !has_range.get(path).copied().unwrap_or(false)
            }
        })
        .collect()
}

async fn enrich_bug_summaries_with_blame(
    bug_summaries: &mut [BugSummary],
    git_link_info: &GitLinkInfo,
    metrics: Arc<ReviewMetrics>,
) -> Vec<String> {
    let mut logs: Vec<String> = Vec::new();
    for summary in bug_summaries.iter_mut() {
        let pairs = parse_location_item(&summary.file, git_link_info);
        let filtered = filter_location_pairs(pairs);
        let primary = filtered
            .iter()
            .find(|(_, fragment)| fragment.as_ref().is_some())
            .or_else(|| filtered.first());
        let Some((rel_path, fragment_opt)) = primary else {
            continue;
        };
        let Some(fragment) = fragment_opt.as_ref() else {
            continue;
        };
        let Some((start, end)) = parse_line_fragment(fragment) else {
            continue;
        };

        metrics.record_tool_call(ToolCallKind::GitBlame);
        metrics.record_tool_call(ToolCallKind::Exec);
        let output = Command::new("git")
            .arg("blame")
            .arg(format!("-L{start},{end}"))
            .arg("--line-porcelain")
            .arg(rel_path)
            .current_dir(&git_link_info.repo_root)
            .output()
            .await;

        let Ok(output) = output else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        if text.is_empty() {
            continue;
        }

        let mut commit: Option<String> = None;
        let mut author: Option<String> = None;
        let mut author_mail: Option<String> = None;
        let mut author_time: Option<OffsetDateTime> = None;

        for line in text.lines() {
            if line.starts_with('\t') {
                break;
            }
            if commit.is_none() {
                let mut parts = line.split_whitespace();
                if let Some(hash) = parts.next() {
                    commit = Some(hash.to_string());
                }
            }
            if author.is_none()
                && let Some(rest) = line.strip_prefix("author ")
            {
                let trimmed = rest.trim();
                if !trimmed.is_empty() {
                    author = Some(trimmed.to_string());
                }
            }
            if author_mail.is_none()
                && let Some(rest) = line.strip_prefix("author-mail ")
            {
                let trimmed = rest.trim();
                if !trimmed.is_empty() {
                    author_mail = Some(trimmed.to_string());
                }
            }
            if author_time.is_none()
                && let Some(rest) = line.strip_prefix("author-time ")
                && let Ok(epoch) = rest.trim().parse::<i64>()
                && let Ok(ts) = OffsetDateTime::from_unix_timestamp(epoch)
            {
                author_time = Some(ts);
            }
        }

        let Some(commit_full) = commit else {
            continue;
        };
        let Some(author_name) = author.clone() else {
            continue;
        };
        let short_sha: String = commit_full.chars().take(7).collect();
        let date = author_time
            .map(|ts| {
                format!(
                    "{:04}-{:02}-{:02}",
                    ts.year(),
                    u8::from(ts.month()),
                    ts.day()
                )
            })
            .unwrap_or_else(|| "unknown-date".to_string());
        let range_display = if start == end {
            format!("L{start}")
        } else {
            format!("L{start}-L{end}")
        };
        let mut github_handle = author_mail
            .as_ref()
            .and_then(|mail| github_handle_from_email(mail));
        if github_handle.is_none() {
            github_handle = github_handle_from_author(&author_name);
        }
        if let Some(handle) = github_handle {
            summary.author_github = Some(handle);
        }
        summary.blame = Some(format!("{short_sha} {author_name} {date} {range_display}"));
        logs.push(format!(
            "Git blame for bug #{id}: {short_sha} {author_name} {date} {range}",
            id = summary.id,
            range = range_display
        ));
    }
    logs
}

fn github_handle_from_email(email: &str) -> Option<String> {
    let s = email
        .trim()
        .trim_matches('<')
        .trim_matches('>')
        .to_ascii_lowercase();
    let at_pos = s.find('@')?;
    let (local, domain) = s.split_at(at_pos);
    let domain = domain.trim_start_matches('@');
    if !domain.ends_with("users.noreply.github.com") {
        return None;
    }
    // Patterns:
    //  - 12345+handle@users.noreply.github.com
    //  - handle@users.noreply.github.com
    let handle = if let Some((_, h)) = local.split_once('+') {
        h
    } else {
        local
    };
    let handle = handle.trim_matches('.').trim_matches('+').trim();
    if handle.is_empty() {
        None
    } else {
        Some(format!("@{handle}"))
    }
}

fn github_handle_from_author(author: &str) -> Option<String> {
    let trimmed = author.trim().trim_matches('@');
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('-') || trimmed.ends_with('-') {
        return None;
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return None;
    }
    Some(format!("@{trimmed}"))
}

#[derive(Debug)]
struct RiskDecision {
    risk_score: f32,
    severity: Option<String>,
    reason: Option<String>,
}

struct RiskRerankChunkSuccess {
    output: ModelCallOutput,
    logs: Vec<String>,
}

struct RiskRerankChunkFailure {
    ids: Vec<usize>,
    error: String,
    logs: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
async fn run_risk_rerank_chunk(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    system_prompt: &str,
    base_prompt: String,
    metrics: Arc<ReviewMetrics>,
    repo_root: PathBuf,
    ids: Vec<usize>,
) -> Result<RiskRerankChunkSuccess, RiskRerankChunkFailure> {
    let mut conversation: Vec<String> = Vec::new();
    let mut seen_search_requests: HashSet<String> = HashSet::new();
    let mut seen_read_requests: HashSet<String> = HashSet::new();
    let mut command_error_count = 0usize;
    let mut tool_rounds = 0usize;
    let mut logs: Vec<String> = Vec::new();

    let id_list = ids
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let repo_display = repo_root.display().to_string();

    loop {
        if tool_rounds > BUG_RERANK_MAX_TOOL_ROUNDS {
            logs.push(format!(
                "Risk rerank chunk for bug id(s) {id_list} exceeded {BUG_RERANK_MAX_TOOL_ROUNDS} tool rounds."
            ));
            return Err(RiskRerankChunkFailure {
                ids,
                error: format!("Risk rerank exceeded {BUG_RERANK_MAX_TOOL_ROUNDS} tool rounds"),
                logs,
            });
        }

        let mut prompt = base_prompt.clone();
        if !conversation.is_empty() {
            prompt.push_str("\n\n# Conversation history\n");
            prompt.push_str(&conversation.join("\n\n"));
        }

        let call_output = match call_model(
            client,
            provider,
            auth,
            model,
            system_prompt,
            &prompt,
            metrics.clone(),
            0.0,
        )
        .await
        {
            Ok(output) => output,
            Err(err) => {
                logs.push(format!("Risk rerank model request failed: {err}"));
                return Err(RiskRerankChunkFailure {
                    ids,
                    error: err,
                    logs,
                });
            }
        };

        if let Some(reasoning) = call_output.reasoning.as_ref() {
            for line in reasoning
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
                logs.push(format!("Risk rerank reasoning: {truncated}"));
            }
        }

        let ModelCallOutput { text, reasoning } = call_output;

        if !text.trim().is_empty() {
            conversation.push(format!("Assistant:\n{}", text.trim()));
        } else {
            conversation.push("Assistant:".to_string());
        }

        let (after_read, read_requests) = extract_read_requests(&text);
        let (cleaned_text, search_requests) = parse_search_requests(&after_read);

        let mut executed_command = false;

        for request in read_requests {
            let cmd_label = request.command.label();
            let key = request.dedupe_key();
            if !seen_read_requests.insert(key) {
                logs.push(format!(
                    "Risk rerank {cmd_label} `{}` skipped (already provided).",
                    request.path.display(),
                ));
                conversation.push(format!(
                    "Tool {cmd_label} `{}` already provided earlier.",
                    request.path.display()
                ));
                executed_command = true;
                continue;
            }

            executed_command = true;
            match execute_auto_scope_read(
                &repo_root,
                &request.path,
                request.command,
                request.start,
                request.end,
                metrics.as_ref(),
            )
            .await
            {
                Ok(output) => {
                    logs.push(format!(
                        "Risk rerank {cmd_label} `{}` returned content.",
                        request.path.display(),
                    ));
                    conversation.push(format!(
                        "Tool {cmd_label} `{}`:\n{}",
                        request.path.display(),
                        output
                    ));
                }
                Err(err) => {
                    logs.push(format!(
                        "Risk rerank {cmd_label} `{}` failed: {err}",
                        request.path.display(),
                    ));
                    conversation.push(format!(
                        "Tool {cmd_label} `{}` error: {err}",
                        request.path.display()
                    ));
                    command_error_count += 1;
                    if command_error_count >= BUG_RERANK_MAX_COMMAND_ERRORS {
                        logs.push(format!(
                            "Risk rerank aborted after {BUG_RERANK_MAX_COMMAND_ERRORS} tool errors."
                        ));
                        return Err(RiskRerankChunkFailure {
                            ids,
                            error: format!(
                                "Risk rerank hit {BUG_RERANK_MAX_COMMAND_ERRORS} tool errors"
                            ),
                            logs,
                        });
                    }
                }
            }
        }

        let mut new_requests: Vec<ToolRequest> = Vec::new();
        for request in search_requests {
            let key = request.dedupe_key();
            if seen_search_requests.insert(key) {
                new_requests.push(request);
            } else {
                match &request {
                    ToolRequest::Content { term, mode, .. } => {
                        let display_term = summarize_search_term(term, 80);
                        logs.push(format!(
                            "Risk rerank search `{display_term}` ({}) skipped (already provided).",
                            mode.as_str()
                        ));
                        conversation.push(format!(
                            "Tool SEARCH `{display_term}` ({}) already provided earlier.",
                            mode.as_str()
                        ));
                    }
                    ToolRequest::GrepFiles { args, .. } => {
                        let mut shown = serde_json::json!({
                            "pattern": args.pattern,
                        });
                        if let Some(ref inc) = args.include {
                            shown["include"] = serde_json::Value::String(inc.clone());
                        }
                        if let Some(ref path) = args.path {
                            shown["path"] = serde_json::Value::String(path.clone());
                        }
                        if let Some(limit) = args.limit {
                            shown["limit"] =
                                serde_json::Value::Number(serde_json::Number::from(limit as u64));
                        }
                        logs.push(format!(
                            "Risk rerank GREP_FILES {shown} skipped (already provided)."
                        ));
                        conversation
                            .push(format!("Tool GREP_FILES {shown} already provided earlier."));
                    }
                }
                executed_command = true;
            }
        }

        for request in new_requests {
            if let Some(reason) = request.reason()
                && !reason.trim().is_empty()
            {
                let truncated = truncate_text(reason, MODEL_REASONING_LOG_MAX_GRAPHEMES);
                logs.push(format!(
                    "Risk rerank tool rationale ({}): {truncated}",
                    request.kind_label()
                ));
            }

            match request {
                ToolRequest::Content { term, mode, .. } => {
                    executed_command = true;
                    let display_term = summarize_search_term(&term, 80);
                    logs.push(format!(
                        "Risk rerank {mode} content search for `{display_term}` — path {repo_display}",
                        mode = mode.as_str()
                    ));
                    match run_content_search(&repo_root, &term, mode, &metrics).await {
                        SearchResult::Matches(output) => {
                            conversation.push(format!(
                                "Tool SEARCH `{display_term}` ({}) results:\n{output}",
                                mode.as_str()
                            ));
                        }
                        SearchResult::NoMatches => {
                            let message = format!(
                                "No content matches found for `{display_term}` — path {repo_display}"
                            );
                            logs.push(message.clone());
                            conversation.push(format!(
                                "Tool SEARCH `{display_term}` ({}) results:\n{message}",
                                mode.as_str()
                            ));
                        }
                        SearchResult::Error(err) => {
                            logs.push(format!(
                                "Risk rerank search `{display_term}` ({}) failed: {err} — path {repo_display}",
                                mode.as_str()
                            ));
                            conversation.push(format!(
                                "Tool SEARCH `{display_term}` ({}) error: {err}",
                                mode.as_str()
                            ));
                            command_error_count += 1;
                            if command_error_count >= BUG_RERANK_MAX_COMMAND_ERRORS {
                                logs.push(format!(
                                    "Risk rerank aborted after {BUG_RERANK_MAX_COMMAND_ERRORS} tool errors."
                                ));
                                return Err(RiskRerankChunkFailure {
                                    ids,
                                    error: format!(
                                        "Risk rerank hit {BUG_RERANK_MAX_COMMAND_ERRORS} tool errors"
                                    ),
                                    logs,
                                });
                            }
                        }
                    }
                }
                ToolRequest::GrepFiles { args, .. } => {
                    executed_command = true;
                    let mut shown = serde_json::json!({
                        "pattern": args.pattern,
                    });
                    if let Some(ref inc) = args.include {
                        shown["include"] = serde_json::Value::String(inc.clone());
                    }
                    if let Some(ref path) = args.path {
                        shown["path"] = serde_json::Value::String(path.clone());
                    }
                    if let Some(limit) = args.limit {
                        shown["limit"] =
                            serde_json::Value::Number(serde_json::Number::from(limit as u64));
                    }
                    logs.push(format!(
                        "Risk rerank GREP_FILES {shown} — path {repo_display}"
                    ));
                    match run_grep_files(&repo_root, &args, &metrics).await {
                        SearchResult::Matches(output) => {
                            conversation.push(format!("Tool GREP_FILES {shown}:\n{output}"));
                        }
                        SearchResult::NoMatches => {
                            let message = "No matches found.".to_string();
                            logs.push(format!(
                                "Risk rerank GREP_FILES {shown} returned no matches."
                            ));
                            conversation.push(format!("Tool GREP_FILES {shown}:\n{message}"));
                        }
                        SearchResult::Error(err) => {
                            logs.push(format!("Risk rerank GREP_FILES {shown} failed: {err}"));
                            conversation.push(format!("Tool GREP_FILES {shown} error: {err}"));
                            command_error_count += 1;
                            if command_error_count >= BUG_RERANK_MAX_COMMAND_ERRORS {
                                logs.push(format!(
                                    "Risk rerank aborted after {BUG_RERANK_MAX_COMMAND_ERRORS} tool errors."
                                ));
                                return Err(RiskRerankChunkFailure {
                                    ids,
                                    error: format!(
                                        "Risk rerank hit {BUG_RERANK_MAX_COMMAND_ERRORS} tool errors"
                                    ),
                                    logs,
                                });
                            }
                        }
                    }
                }
            }
        }

        if executed_command {
            tool_rounds += 1;
            continue;
        }

        let final_text = cleaned_text.trim().to_string();
        return Ok(RiskRerankChunkSuccess {
            output: ModelCallOutput {
                text: final_text,
                reasoning,
            },
            logs,
        });
    }
}

#[allow(clippy::too_many_arguments)]
async fn rerank_bugs_by_risk(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    summaries: &mut [BugSummary],
    repo_root: &Path,
    repository_summary: &str,
    spec_context: Option<&str>,
    metrics: Arc<ReviewMetrics>,
) -> Vec<String> {
    if summaries.is_empty() {
        return Vec::new();
    }

    let repo_summary_snippet =
        trim_prompt_context(repository_summary, BUG_RERANK_CONTEXT_MAX_CHARS);
    let spec_excerpt_snippet = spec_context
        .map(|text| trim_prompt_context(text, BUG_RERANK_CONTEXT_MAX_CHARS))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Not provided.".to_string());

    let chunk_size = BUG_RERANK_CHUNK_SIZE.max(1);

    let mut prompt_chunks: Vec<(Vec<usize>, String)> = Vec::new();
    for chunk in summaries.chunks(chunk_size) {
        let findings_payload = chunk
            .iter()
            .map(|summary| {
                json!({
                    "id": summary.id,
                    "title": summary.title,
                    "severity": summary.severity,
                    "impact": summary.impact,
                    "likelihood": summary.likelihood,
                    "location": summary.file,
                    "recommendation": summary.recommendation,
                    "blame": summary.blame,
                })
                .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let ids: Vec<usize> = chunk.iter().map(|summary| summary.id).collect();
        let prompt = BUG_RERANK_PROMPT_TEMPLATE
            .replace("{repository_summary}", &repo_summary_snippet)
            .replace("{spec_excerpt}", &spec_excerpt_snippet)
            .replace("{findings}", &findings_payload);
        prompt_chunks.push((ids, prompt));
    }

    let total_chunks = prompt_chunks.len();
    let max_concurrency = BUG_RERANK_MAX_CONCURRENCY.max(1).min(total_chunks.max(1));

    let chunk_results = futures::stream::iter(prompt_chunks.into_iter().map(|(ids, prompt)| {
        let provider = provider.clone();
        let auth_clone = auth.clone();
        let model_owned = model.to_string();
        let metrics_clone = metrics.clone();
        let repo_root = repo_root.to_path_buf();

        async move {
            run_risk_rerank_chunk(
                client,
                &provider,
                &auth_clone,
                model_owned.as_str(),
                BUG_RERANK_SYSTEM_PROMPT,
                prompt,
                metrics_clone,
                repo_root,
                ids,
            )
            .await
        }
    }))
    .buffer_unordered(max_concurrency)
    .collect::<Vec<_>>()
    .await;

    let mut logs: Vec<String> = Vec::new();
    let mut decisions: HashMap<usize, RiskDecision> = HashMap::new();

    for result in chunk_results {
        match result {
            Ok(mut success) => {
                logs.append(&mut success.logs);
                let ModelCallOutput { text, .. } = success.output;
                for raw_line in text.lines() {
                    let line = raw_line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    let Ok(value) = serde_json::from_str::<Value>(line) else {
                        continue;
                    };
                    let Some(id_val) = value_to_usize(&value["id"]) else {
                        continue;
                    };
                    let Some(score_val) = value_to_f32(&value["risk_score"]) else {
                        continue;
                    };
                    let clamped_score = score_val.clamp(0.0, 100.0);
                    let severity = value
                        .get("severity")
                        .and_then(Value::as_str)
                        .and_then(normalize_severity_label);
                    let reason = value
                        .get("reason")
                        .and_then(Value::as_str)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());

                    decisions
                        .entry(id_val)
                        .and_modify(|existing| {
                            if clamped_score > existing.risk_score {
                                existing.risk_score = clamped_score;
                                if severity.is_some() {
                                    existing.severity = severity.clone();
                                }
                                existing.reason = reason.clone();
                            }
                        })
                        .or_insert(RiskDecision {
                            risk_score: clamped_score,
                            severity: severity.clone(),
                            reason: reason.clone(),
                        });
                }
            }
            Err(mut failure) => {
                logs.append(&mut failure.logs);
                let id_list = failure
                    .ids
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                logs.push(format!(
                    "Risk rerank chunk failed for bug id(s) {id_list}: {error}",
                    error = failure.error
                ));
            }
        }
    }

    for summary in summaries.iter_mut() {
        if let Some(decision) = decisions.get(&summary.id) {
            summary.risk_score = Some(decision.risk_score.clamp(0.0, 100.0));
            if let Some(ref sev) = decision.severity {
                summary.severity = sev.clone();
            }
            summary.risk_reason = decision.reason.clone();
        } else {
            summary.risk_score = None;
            summary.risk_reason = None;
        }
    }

    summaries.sort_by(|a, b| match (a.risk_score, b.risk_score) {
        (Some(sa), Some(sb)) => sb.partial_cmp(&sa).unwrap_or(CmpOrdering::Equal),
        (Some(_), None) => CmpOrdering::Less,
        (None, Some(_)) => CmpOrdering::Greater,
        _ => severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.id.cmp(&b.id)),
    });

    for (idx, summary) in summaries.iter_mut().enumerate() {
        summary.risk_rank = Some(idx + 1);
        let log_entry = if let Some(score) = summary.risk_score {
            let reason = summary
                .risk_reason
                .as_deref()
                .unwrap_or("no reason provided");
            format!(
                "Risk rerank: bug #{id} -> priority {rank} (score {score:.1}, severity {severity}) — {reason}",
                id = summary.id,
                rank = idx + 1,
                score = score,
                severity = summary.severity,
                reason = reason
            )
        } else {
            format!(
                "Risk rerank: bug #{id} retained original severity {severity} (no model decision)",
                id = summary.id,
                severity = summary.severity
            )
        };
        logs.push(log_entry);
    }

    logs
}

fn parse_line_fragment(fragment: &str) -> Option<(usize, usize)> {
    let trimmed = fragment.trim().trim_start_matches('#');
    let without_prefix = trimmed.strip_prefix('L')?;
    if let Some((start_str, end_str)) = without_prefix.split_once("-L") {
        let start = start_str.trim().parse::<usize>().ok()?;
        let end = end_str.trim().parse::<usize>().ok()?;
        if start == 0 || end == 0 {
            return None;
        }
        Some((start.min(end), start.max(end)))
    } else {
        let start = without_prefix.trim().parse::<usize>().ok()?;
        if start == 0 {
            return None;
        }
        Some((start, start))
    }
}

fn to_relative_path(raw_path: &str, git_link_info: &GitLinkInfo) -> Option<String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = Path::new(trimmed);
    let relative = if candidate.is_absolute() {
        diff_paths(candidate, &git_link_info.repo_root)?
    } else {
        PathBuf::from(trimmed)
    };
    let mut normalized = relative.to_string_lossy().replace('\\', "/");
    while normalized.starts_with("./") {
        normalized = normalized[2..].to_string();
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

async fn build_git_link_info(repo_path: &Path) -> Option<GitLinkInfo> {
    let canonical_repo = repo_path.canonicalize().ok()?;
    let git_root = get_git_repo_root(&canonical_repo)?;
    let canonical_root = git_root.canonicalize().unwrap_or(git_root);
    let git_info = collect_git_info(&canonical_root).await?;
    let commit = git_info.commit_hash.as_ref()?.trim().to_string();
    if commit.is_empty() {
        return None;
    }
    let remote = git_info.repository_url.as_ref()?.trim().to_string();
    if remote.is_empty() {
        return None;
    }
    let github_prefix = normalize_github_url(&remote, &commit)?;
    Some(GitLinkInfo {
        repo_root: canonical_root,
        github_prefix,
    })
}

async fn collect_git_revision(repo_path: &Path) -> Option<(String, Option<String>, Option<i64>)> {
    let canonical_repo = repo_path.canonicalize().ok()?;
    let git_root = get_git_repo_root(&canonical_repo)?;
    let canonical_root = git_root.canonicalize().unwrap_or(git_root);
    let git_info = collect_git_info(&canonical_root).await?;
    let commit = git_info.commit_hash.as_deref()?.trim().to_string();
    if commit.is_empty() {
        return None;
    }
    let branch = git_info.branch.clone();
    let timestamp = recent_commits(&canonical_root, 1)
        .await
        .into_iter()
        .next()
        .map(|entry| entry.timestamp);
    Some((commit, branch, timestamp))
}

fn normalize_github_url(remote: &str, commit: &str) -> Option<String> {
    let trimmed_remote = remote.trim();
    let trimmed_commit = commit.trim();
    if trimmed_remote.is_empty() || trimmed_commit.is_empty() {
        return None;
    }

    let mut base =
        if trimmed_remote.starts_with("http://") || trimmed_remote.starts_with("https://") {
            trimmed_remote.to_string()
        } else if trimmed_remote.starts_with("ssh://") {
            let url = Url::parse(trimmed_remote).ok()?;
            let host = url.host_str()?;
            if !host.contains("github") {
                return None;
            }
            let path = url.path().trim_start_matches('/');
            format!("https://{host}/{path}")
        } else if let Some(idx) = trimmed_remote.find("@github.com:") {
            let path = &trimmed_remote[idx + "@github.com:".len()..];
            format!("https://github.com/{path}")
        } else if trimmed_remote.starts_with("git@github.com:") {
            trimmed_remote.replacen("git@github.com:", "https://github.com/", 1)
        } else {
            return None;
        };

    if !base.contains("github") {
        return None;
    }

    if base.ends_with(".git") {
        base.truncate(base.len() - 4);
    }

    while base.ends_with('/') {
        base.pop();
    }

    if base.is_empty() {
        return None;
    }

    Some(format!("{base}/blob/{trimmed_commit}/"))
}

fn sanitize_table_field(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "-".to_string()
    } else {
        trimmed.replace('\n', " ").replace('|', r"\|")
    }
}

fn severity_rank(severity: &str) -> usize {
    match severity.trim().to_ascii_lowercase().as_str() {
        "high" => 0,
        "medium" | "med" => 1,
        "low" => 2,
        "informational" | "info" => 3,
        _ => 4,
    }
}

fn normalize_severity_label(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    let label = match normalized.as_str() {
        "critical" | "crit" | "p0" | "sev0" | "sev-0" => "High",
        "high" | "p1" | "sev1" | "sev-1" => "High",
        "medium" | "med" | "p2" | "sev2" | "sev-2" => "Medium",
        "low" | "p3" | "sev3" | "sev-3" => "Low",
        "informational" | "info" | "p4" | "sev4" | "sev-4" | "note" => "Informational",
        _ => return None,
    };
    Some(label.to_string())
}

fn value_to_usize(value: &Value) -> Option<usize> {
    if let Some(n) = value.as_u64() {
        return Some(n as usize);
    }
    if let Some(s) = value.as_str() {
        return s.trim().parse::<usize>().ok();
    }
    None
}

fn value_to_f32(value: &Value) -> Option<f32> {
    if let Some(n) = value.as_f64() {
        return Some(n as f32);
    }
    if let Some(n) = value.as_i64() {
        return Some(n as f32);
    }
    if let Some(n) = value.as_u64() {
        return Some(n as f32);
    }
    if let Some(s) = value.as_str() {
        return s.trim().parse::<f32>().ok();
    }
    None
}

fn trim_prompt_context(input: &str, max_chars: usize) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut result = String::new();
    for (count, ch) in trimmed.chars().enumerate() {
        if count >= max_chars {
            result.push_str(" …");
            break;
        }
        result.push(ch);
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    Literal,
    Regex,
}

impl SearchMode {
    fn as_str(self) -> &'static str {
        match self {
            SearchMode::Literal => "literal",
            SearchMode::Regex => "regex",
        }
    }
}

#[derive(Debug, Clone)]
struct ReadRequest {
    command: ReadCommand,
    path: PathBuf,
    start: Option<usize>,
    end: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadCommand {
    Read,
    ListDir,
}

impl ReadCommand {
    fn label(self) -> &'static str {
        match self {
            ReadCommand::Read => "READ",
            ReadCommand::ListDir => "LIST_DIR",
        }
    }
}

impl ReadRequest {
    fn dedupe_key(&self) -> String {
        format!(
            "{}:{}:{}-{}",
            self.command.label(),
            self.path.to_string_lossy().to_ascii_lowercase(),
            self.start.unwrap_or(0),
            self.end.unwrap_or(0)
        )
    }
}

#[derive(Debug, Clone)]
enum ToolRequest {
    Content {
        term: String,
        mode: SearchMode,
        reason: Option<String>,
    }, // backward-compat
    GrepFiles {
        args: GrepFilesArgs,
        reason: Option<String>,
    },
}

impl ToolRequest {
    fn dedupe_key(&self) -> String {
        match self {
            ToolRequest::Content { term, mode, .. } => {
                let lower = term.to_ascii_lowercase();
                format!("content:{mode}:{lower}", mode = mode.as_str())
            }
            ToolRequest::GrepFiles { args, .. } => format!(
                "grep_files:{}:{}:{}:{}",
                args.pattern.to_ascii_lowercase(),
                args.include
                    .clone()
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
                args.path.clone().unwrap_or_default().to_ascii_lowercase(),
                args.limit.unwrap_or(100)
            ),
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            ToolRequest::Content { reason, .. } => reason.as_deref(),
            ToolRequest::GrepFiles { reason, .. } => reason.as_deref(),
        }
    }

    fn kind_label(&self) -> &'static str {
        match self {
            ToolRequest::Content { .. } => "search",
            ToolRequest::GrepFiles { .. } => "grep_files",
        }
    }
}

#[derive(Debug)]
enum SearchResult {
    Matches(String),
    NoMatches,
    Error(String),
}

fn strip_prefix_case_insensitive<'a>(input: &'a str, prefix: &str) -> Option<&'a str> {
    let head = input.get(..prefix.len())?;
    if !head.eq_ignore_ascii_case(prefix) {
        return None;
    }
    input.get(prefix.len()..)
}

fn parse_search_term(input: &str) -> (SearchMode, &str) {
    let trimmed = input.trim();
    if let Some(rest) = strip_prefix_case_insensitive(trimmed, "regex:") {
        return (SearchMode::Regex, rest.trim());
    }
    if let Some(rest) = strip_prefix_case_insensitive(trimmed, "literal:") {
        return (SearchMode::Literal, rest.trim());
    }
    (SearchMode::Literal, trimmed)
}

fn summarize_search_term(term: &str, limit: usize) -> String {
    let mut summary = term.replace('\n', "\\n");
    if summary.len() > limit {
        summary.truncate(limit);
        summary.push_str("...");
    }
    summary
}

fn extract_read_requests(response: &str) -> (String, Vec<ReadRequest>) {
    let mut requests = Vec::new();
    let mut cleaned = Vec::new();

    for line in response.lines() {
        let trimmed = line.trim();
        let (command, rest) = if let Some(rest) = strip_prefix_case_insensitive(trimmed, "READ:") {
            (ReadCommand::Read, rest)
        } else if let Some(rest) = strip_prefix_case_insensitive(trimmed, "LIST_DIR:") {
            (ReadCommand::ListDir, rest)
        } else {
            cleaned.push(line);
            continue;
        };

        let spec = rest.trim();
        if spec.is_empty() {
            cleaned.push(line);
            continue;
        }

        let (path_part, range_part) = spec.split_once('#').unwrap_or((spec, ""));
        let path_str = path_part.trim();
        if path_str.is_empty() {
            cleaned.push(line);
            continue;
        }

        let relative = PathBuf::from(path_str);
        if relative.as_os_str().is_empty() || relative.is_absolute() {
            cleaned.push(line);
            continue;
        }

        let mut start = None;
        let mut end = None;
        if command == ReadCommand::Read
            && let Some(range) = range_part.strip_prefix('L')
        {
            let mut parts = range.split('-');
            if let Some(start_str) = parts.next()
                && let Ok(value) = start_str.trim().parse::<usize>()
                && value > 0
            {
                start = Some(value);
            }
            if let Some(end_str) = parts.next()
                && let Ok(value) = end_str
                    .trim()
                    .trim_start_matches(['l', 'L'])
                    .parse::<usize>()
                && value > 0
                && start.map(|s| value >= s).unwrap_or(true)
            {
                end = Some(value);
            }
        }

        requests.push(ReadRequest {
            command,
            path: relative,
            start,
            end,
        });
    }

    (cleaned.join("\n"), requests)
}
fn emit_command_status(
    progress_sender: &Option<AppEventSender>,
    id: u64,
    summary: String,
    state: SecurityReviewCommandState,
    preview: Vec<String>,
) {
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewCommandStatus {
            id,
            summary,
            state,
            preview,
        });
    }
}

fn command_completion_state(result: &SearchResult) -> (SecurityReviewCommandState, Vec<String>) {
    match result {
        SearchResult::Matches(text) => (
            SecurityReviewCommandState::Matches,
            command_preview_snippets(text),
        ),
        SearchResult::NoMatches => (SecurityReviewCommandState::NoMatches, Vec::new()),
        SearchResult::Error(err) => {
            let preview = if err.is_empty() {
                Vec::new()
            } else {
                vec![format!(
                    "Error: {}",
                    truncate_text(err, COMMAND_PREVIEW_MAX_GRAPHEMES)
                )]
            };
            (SecurityReviewCommandState::Error, preview)
        }
    }
}

fn command_preview_snippets(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut iter = text.lines();
    for line in iter.by_ref().take(COMMAND_PREVIEW_MAX_LINES) {
        lines.push(truncate_text(line, COMMAND_PREVIEW_MAX_GRAPHEMES));
    }
    if iter.next().is_some() {
        lines.push("…".to_string());
    }
    lines
}

fn parse_search_requests(response: &str) -> (String, Vec<ToolRequest>) {
    let mut requests = Vec::new();
    let mut cleaned = Vec::new();
    let mut last_reason: Option<String> = None;
    for line in response.lines() {
        let trimmed = line.trim();
        let mut parsed_request: Option<ToolRequest> = None;
        if let Some(rest) = strip_prefix_case_insensitive(trimmed, "GREP_FILES:") {
            let spec = rest.trim();
            if !spec.is_empty()
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(spec)
            {
                let args = GrepFilesArgs {
                    pattern: v
                        .get("pattern")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                    include: v
                        .get("include")
                        .and_then(Value::as_str)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                    path: v
                        .get("path")
                        .and_then(Value::as_str)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                    limit: v.get("limit").and_then(Value::as_u64).map(|n| n as usize),
                };
                if !args.pattern.is_empty() {
                    parsed_request = Some(ToolRequest::GrepFiles {
                        args,
                        reason: last_reason.take(),
                    });
                }
            }
        } else if let Some(rest) = strip_prefix_case_insensitive(trimmed, "SEARCH_FILES:") {
            // Deprecated: treat as content search
            let (mode, term) = parse_search_term(rest.trim_matches('`'));
            if !term.is_empty() {
                parsed_request = Some(ToolRequest::Content {
                    term: term.to_string(),
                    mode,
                    reason: last_reason.take(),
                });
            }
        } else if let Some(rest) = strip_prefix_case_insensitive(trimmed, "SEARCH:") {
            let (mode, term) = parse_search_term(rest.trim_matches('`'));
            if !term.is_empty() {
                parsed_request = Some(ToolRequest::Content {
                    term: term.to_string(),
                    mode,
                    reason: last_reason.take(),
                });
            }
        }

        if let Some(request) = parsed_request {
            requests.push(request);
            continue;
        }

        cleaned.push(line);
        if trimmed.is_empty() {
            last_reason = None;
        } else {
            last_reason = Some(trimmed.to_string());
        }
    }
    (cleaned.join("\n"), requests)
}

struct CollectionState {
    repo_path: PathBuf,
    snippets: Vec<FileSnippet>,
    seen_dirs: HashSet<PathBuf>,
    max_files: usize,
    max_bytes_per_file: usize,
    max_total_bytes: usize,
    total_bytes: usize,
    progress_sender: Option<AppEventSender>,
    last_progress_instant: Instant,
    last_reported_files: usize,
    limit_hit: bool,
    limit_reason: Option<String>,
}

impl CollectionState {
    fn new(
        repo_path: PathBuf,
        max_files: usize,
        max_bytes_per_file: usize,
        max_total_bytes: usize,
        progress_sender: Option<AppEventSender>,
    ) -> Self {
        Self {
            repo_path,
            snippets: Vec::new(),
            seen_dirs: HashSet::new(),
            max_files,
            max_bytes_per_file,
            max_total_bytes,
            total_bytes: 0,
            progress_sender,
            last_progress_instant: Instant::now(),
            last_reported_files: 0,
            limit_hit: false,
            limit_reason: None,
        }
    }

    fn limit_reached(&self) -> bool {
        self.snippets.len() >= self.max_files || self.total_bytes >= self.max_total_bytes
    }

    fn visit_path(&mut self, path: &Path) -> Result<(), String> {
        if self.limit_reached() {
            self.record_limit_hit(format!(
                "Reached collection limits before finishing {}.",
                path.display()
            ));
            return Ok(());
        }

        let metadata = fs::symlink_metadata(path)
            .map_err(|e| format!("Failed to inspect {}: {e}", path.display()))?;

        if metadata.file_type().is_symlink() {
            return Ok(());
        }

        if metadata.is_dir() {
            self.visit_dir(path)
        } else if metadata.is_file() {
            self.visit_file(path, metadata.len() as usize)
        } else {
            Ok(())
        }
    }

    fn visit_dir(&mut self, path: &Path) -> Result<(), String> {
        if self.limit_reached() {
            self.record_limit_hit(format!(
                "Reached collection limits while scanning directory {}.",
                path.display()
            ));
            return Ok(());
        }

        if let Some(name) = path.file_name().and_then(|s| s.to_str())
            && EXCLUDED_DIR_NAMES
                .iter()
                .any(|excluded| excluded.eq_ignore_ascii_case(name))
        {
            self.emit_progress_message(format!("Skipping excluded directory {}", path.display()));
            return Ok(());
        }

        if !self.seen_dirs.insert(path.to_path_buf()) {
            return Ok(());
        }

        let entries = fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory {}: {e}", path.display()))?;
        for entry in entries {
            if self.limit_reached() {
                break;
            }
            let entry =
                entry.map_err(|e| format!("Failed to read entry in {}: {e}", path.display()))?;
            self.visit_path(&entry.path())?;
        }
        Ok(())
    }

    fn visit_file(&mut self, path: &Path, file_size: usize) -> Result<(), String> {
        if self.limit_reached() {
            self.record_limit_hit("Reached collection limits while visiting files.".to_string());
            return Ok(());
        }

        if is_ignored_file(path) {
            return Ok(());
        }

        let language = match determine_language(path) {
            Some(lang) => lang.to_string(),
            None => return Ok(()),
        };

        if file_size == 0 || file_size > self.max_bytes_per_file {
            return Ok(());
        }

        let mut file =
            fs::File::open(path).map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

        if buffer.is_empty() {
            return Ok(());
        }

        if buffer.len() > self.max_bytes_per_file {
            buffer.truncate(self.max_bytes_per_file);
        }

        let content = String::from_utf8_lossy(&buffer).to_string();
        let bytes = buffer.len();

        let new_total = self.total_bytes.saturating_add(bytes);
        if new_total > self.max_total_bytes {
            self.record_limit_hit(format!(
                "Reached byte limit ({} of {}).",
                human_readable_bytes(new_total),
                human_readable_bytes(self.max_total_bytes)
            ));
            return Ok(());
        }

        let relative_path = path
            .strip_prefix(&self.repo_path)
            .unwrap_or(path)
            .to_path_buf();

        self.snippets.push(FileSnippet {
            relative_path,
            language,
            content,
            bytes,
        });
        self.total_bytes = new_total;
        self.maybe_emit_file_progress();
        Ok(())
    }

    fn emit_progress_message(&self, message: String) {
        if let Some(tx) = &self.progress_sender {
            tx.send(AppEvent::SecurityReviewLog(message));
        }
    }

    fn record_limit_hit(&mut self, reason: String) {
        if !self.limit_hit {
            self.limit_hit = true;
            self.limit_reason = Some(reason.clone());
        }
        self.emit_progress_message(reason);
    }

    fn maybe_emit_file_progress(&mut self) {
        if self.progress_sender.is_none() {
            return;
        }

        let count = self.snippets.len();
        if count == 0 {
            return;
        }

        let now = Instant::now();
        let files_delta = count.saturating_sub(self.last_reported_files);
        if count == 1
            || files_delta >= 5
            || now.duration_since(self.last_progress_instant) >= Duration::from_secs(2)
        {
            let bytes = human_readable_bytes(self.total_bytes);
            if let Some(tx) = &self.progress_sender {
                tx.send(AppEvent::SecurityReviewLog(format!(
                    "Collected {count} files so far ({bytes})."
                )));
            }
            self.last_reported_files = count;
            self.last_progress_instant = now;
        }
    }
}

fn determine_language(path: &Path) -> Option<&'static str> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .and_then(|ext| match ext.as_str() {
            "ts" => Some("typescript"),
            "tsx" => Some("tsx"),
            "js" | "mjs" | "cjs" => Some("javascript"),
            "jsx" => Some("jsx"),
            "py" => Some("python"),
            "go" => Some("go"),
            "rb" => Some("ruby"),
            "rs" => Some("rust"),
            "java" => Some("java"),
            "kt" | "kts" => Some("kotlin"),
            "swift" => Some("swift"),
            "php" => Some("php"),
            "scala" => Some("scala"),
            "c" => Some("c"),
            "cc" | "cpp" | "cxx" | "c++" | "ixx" => Some("cpp"),
            "cs" => Some("csharp"),
            "sh" | "bash" | "zsh" => Some("bash"),
            "pl" => Some("perl"),
            "sql" => Some("sql"),
            "yaml" | "yml" => Some("yaml"),
            "json" => Some("json"),
            "toml" => Some("toml"),
            "env" => Some("env"),
            "ini" => Some("ini"),
            "md" => Some("markdown"),
            _ => None,
        })
}

fn build_repository_summary(snippets: &[FileSnippet]) -> String {
    let mut lines = Vec::new();
    lines.push("Included files:".to_string());
    for snippet in snippets {
        let size = human_readable_bytes(snippet.bytes);
        lines.push(format!("- {} ({size})", snippet.relative_path.display()));
    }
    lines.push(String::new());
    let platform = format!(
        "{} {} ({})",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::consts::FAMILY
    );
    lines.push(format!("Host platform: {platform}"));
    if let Ok(now) = OffsetDateTime::now_utc().format(&Rfc3339) {
        lines.push(format!("Generated at: {now}"));
    }
    lines.join("\n")
}

fn build_single_file_context(snippet: &FileSnippet) -> String {
    format!(
        "### {}\n```{}\n{}\n```\n",
        snippet.relative_path.display(),
        snippet.language,
        snippet.content
    )
}

fn build_threat_model_prompt(repository_summary: &str, spec: &SpecGenerationOutcome) -> String {
    let locations_block = if spec.locations.is_empty() {
        "repository root".to_string()
    } else {
        spec.locations.join("\n")
    };

    THREAT_MODEL_PROMPT_TEMPLATE
        .replace("{repository_summary}", repository_summary)
        .replace("{combined_spec}", spec.combined_markdown.trim())
        .replace("{locations}", &locations_block)
}

fn build_threat_model_retry_prompt(base_prompt: &str, previous_output: &str) -> String {
    format!(
        "{base_prompt}\n\nPrevious attempt:\n```\n{previous_output}\n```\nThe previous response did not populate the `Threat Model` table. Re-run the task above and respond with the summary paragraph followed by a complete Markdown table named `Threat Model` with populated rows (IDs starting at 1, with realistic data)."
    )
}

fn threat_table_has_rows(markdown: &str) -> bool {
    let mut seen_header = false;
    let mut seen_divider = false;
    for line in markdown.lines() {
        let trimmed = line.trim();
        if !seen_header {
            if trimmed.starts_with('|') && trimmed.to_ascii_lowercase().contains("threat id") {
                seen_header = true;
            }
            continue;
        }
        if !seen_divider {
            if trimmed.starts_with('|') && trimmed.contains("---") {
                seen_divider = true;
                continue;
            }
            if trimmed.is_empty() {
                break;
            }
            continue;
        }
        if trimmed.is_empty() {
            break;
        }
        if !trimmed.starts_with('|') {
            break;
        }
        let has_data = trimmed
            .trim_matches('|')
            .split('|')
            .any(|cell| !cell.trim().is_empty());
        if has_data {
            return true;
        }
    }
    false
}

fn sort_threat_table(markdown: &str) -> Option<String> {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut output: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if trimmed.starts_with('|') && trimmed.to_ascii_lowercase().contains("threat id") {
            let header_cells: Vec<String> = trimmed
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_string())
                .collect();
            let Some(priority_idx) = header_cells
                .iter()
                .position(|cell| cell.eq_ignore_ascii_case("priority"))
            else {
                output.push(line.to_string());
                i += 1;
                continue;
            };
            output.push(line.to_string());
            i += 1;
            if i < lines.len() {
                output.push(lines[i].to_string());
                i += 1;
            }
            let mut rows: Vec<(usize, String, u8)> = Vec::new();
            while i < lines.len() {
                let row_line = lines[i];
                let row_trim = row_line.trim();
                if row_trim.is_empty() || !row_trim.starts_with('|') {
                    break;
                }
                let priority_score = row_trim
                    .trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .nth(priority_idx)
                    .map(|value| match value.to_ascii_lowercase().as_str() {
                        "high" => 0,
                        "medium" => 1,
                        "low" => 2,
                        _ => 3,
                    })
                    .unwrap_or(3);
                rows.push((rows.len(), row_line.to_string(), priority_score));
                i += 1;
            }
            rows.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.cmp(&b.0)));
            for (_, row, _) in rows {
                output.push(row);
            }
            while i < lines.len() {
                output.push(lines[i].to_string());
                i += 1;
            }
            return Some(output.join("\n"));
        }
        output.push(line.to_string());
        i += 1;
    }
    None
}

fn build_bugs_user_prompt(
    repository_summary: &str,
    spec_markdown: Option<&str>,
    code_context: &str,
    scope_prompt: Option<&str>,
) -> BugPromptData {
    let mut logs = Vec::new();
    let scope_reminder = scope_prompt
        .and_then(|prompt| {
            let normalized = prompt
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            if normalized.is_empty() {
                return None;
            }
            let scope_summary = truncate_text(&normalized, BUG_SCOPE_PROMPT_MAX_GRAPHEMES);
            if scope_summary.len() < normalized.len() {
                logs.push(format!(
                    "User scope prompt truncated to {BUG_SCOPE_PROMPT_MAX_GRAPHEMES} graphemes for bug analysis."
                ));
            }
            Some(format!("- User scope prompt: {scope_summary}\n- After reading the file, if it does not meaningfully relate to that scope, skip it and move on (respond with `no bugs found` for this file).\n"))
        })
        .unwrap_or_default();
    let repository_section = format!("# Repository context\n{repository_summary}\n");
    let code_and_task = BUGS_USER_CODE_AND_TASK
        .replace("{code_context}", code_context)
        .replace("{scope_reminder}", scope_reminder.as_str());
    let base_len = repository_section.len() + code_and_task.len();
    let mut prompt =
        String::with_capacity(base_len + spec_markdown.map(str::len).unwrap_or_default());

    prompt.push_str(repository_section.as_str());

    if let Some(raw_spec) = spec_markdown {
        let trimmed_spec = raw_spec.trim();
        if !trimmed_spec.is_empty() {
            let available_for_spec = MAX_PROMPT_BYTES.saturating_sub(base_len);
            const SPEC_HEADER: &str = "\n# Specification context\n";
            if available_for_spec > SPEC_HEADER.len() {
                let max_spec_bytes = available_for_spec - SPEC_HEADER.len();
                let mut spec_section = String::from(SPEC_HEADER);
                if trimmed_spec.len() <= max_spec_bytes {
                    spec_section.push_str(trimmed_spec);
                    spec_section.push('\n');
                    prompt.push_str(spec_section.as_str());
                } else {
                    const SPEC_TRUNCATION_NOTE: &str =
                        "\n\n[Specification truncated to stay under context limit]";
                    if max_spec_bytes <= SPEC_TRUNCATION_NOTE.len() {
                        logs.push(format!(
                            "Omitted specification context from bug analysis prompt to stay under the {} limit.",
                            human_readable_bytes(MAX_PROMPT_BYTES)
                        ));
                    } else {
                        let available_for_content = max_spec_bytes - SPEC_TRUNCATION_NOTE.len();
                        let truncated =
                            truncate_to_char_boundary(trimmed_spec, available_for_content);
                        spec_section.push_str(truncated);
                        spec_section.push_str(SPEC_TRUNCATION_NOTE);
                        spec_section.push('\n');
                        prompt.push_str(spec_section.as_str());
                        logs.push(format!(
                            "Specification context trimmed to fit within the bug analysis prompt limit ({}).",
                            human_readable_bytes(MAX_PROMPT_BYTES)
                        ));
                    }
                }
            } else {
                logs.push(format!(
                    "Insufficient room to include specification context in bug analysis prompt (limit {}).",
                    human_readable_bytes(MAX_PROMPT_BYTES)
                ));
            }
        }
    }

    prompt.push_str(code_and_task.as_str());

    if prompt.len() > MAX_PROMPT_BYTES {
        logs.push(format!(
            "Bug analysis prompt exceeds limit ({}); continuing with {}.",
            human_readable_bytes(MAX_PROMPT_BYTES),
            human_readable_bytes(prompt.len())
        ));
    }

    BugPromptData { prompt, logs }
}

fn truncate_to_char_boundary(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    if max_bytes == 0 {
        return "";
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

pub(crate) fn build_follow_up_user_prompt(
    mode: SecurityReviewMode,
    scope_paths: &[String],
    report_path: &Path,
    repo_root: &Path,
    report_label: &str,
    question: &str,
) -> String {
    let scope_summary = if scope_paths.is_empty() {
        "entire repository".to_string()
    } else {
        scope_paths.join(", ")
    };
    let mode_label = mode.as_str();
    let report_display = display_path_for(report_path, repo_root);
    let label = if report_label.is_empty() {
        "Report".to_string()
    } else {
        report_label.to_string()
    };

    format!(
        "{SECURITY_REVIEW_FOLLOW_UP_MARKER}\nSecurity review follow-up context:\n- Mode: {mode_label}\n- Scope: {scope_summary}\n- {label}: {report_display}\n\nInstructions:\n- Consider the question first, then skim the report for relevant sections before reading in full.\n- Explore the scoped code paths (see Scope above), not just the report: use `rg` to locate definitions/usages and `read_file` to open the relevant files and nearby call sites.\n- Quote short report excerpts as supporting context, but ground confirmations and clarifications in the in-scope code.\n- Do not modify files or run destructive commands; you are only answering questions.\n- Keep answers concise and in Markdown.\n\nQuestion:\n{question}\n"
    )
}

pub(crate) fn parse_follow_up_question(message: &str) -> Option<String> {
    if !message.starts_with(SECURITY_REVIEW_FOLLOW_UP_MARKER) {
        return None;
    }
    let (_, tail) = message.split_once("\nQuestion:\n")?;
    let trimmed = tail.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

async fn run_trufflehog_scan(
    repo_path: &Path,
    output_root: &Path,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    logs: &mut Vec<String>,
) -> Option<PathBuf> {
    let target_path = output_root.join("trufflehog.jsonl");
    let start_message = format!(
        "Running trufflehog filesystem scan for open-source review; output: {}",
        target_path.display()
    );
    push_progress_log(&progress_sender, &log_sink, logs, start_message);

    let mut command = Command::new("trufflehog");
    command
        .arg("filesystem")
        .arg("--json")
        .arg("--no-update")
        .arg(repo_path)
        .current_dir(repo_path);

    let output = match command.output().await {
        Ok(output) => output,
        Err(err) => {
            let message = format!(
                "trufflehog is not available or failed to start: {err}. Skipping secret scan."
            );
            push_progress_log(&progress_sender, &log_sink, logs, message);
            return None;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let summary = if stderr.is_empty() {
            format!("trufflehog exited with status {}", output.status)
        } else {
            format!("trufflehog exited with status {}: {stderr}", output.status)
        };
        push_progress_log(&progress_sender, &log_sink, logs, summary);
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if stdout.trim().is_empty() {
        push_progress_log(
            &progress_sender,
            &log_sink,
            logs,
            "trufflehog completed with no findings; no JSON artifact written.".to_string(),
        );
        return None;
    }

    if let Some(parent) = target_path.parent()
        && let Err(err) = tokio_fs::create_dir_all(parent).await
    {
        let message = format!(
            "Failed to create directory for trufflehog output {}: {err}",
            parent.display()
        );
        push_progress_log(&progress_sender, &log_sink, logs, message);
        return None;
    }

    if let Err(err) = tokio_fs::write(&target_path, stdout.as_bytes()).await {
        let message = format!(
            "Failed to write trufflehog results to {}: {err}",
            target_path.display()
        );
        push_progress_log(&progress_sender, &log_sink, logs, message);
        return None;
    }

    let done_message = format!(
        "Trufflehog secret scan complete; JSONL results saved to {}.",
        target_path.display()
    );
    push_progress_log(&progress_sender, &log_sink, logs, done_message);
    Some(target_path)
}

#[allow(clippy::too_many_arguments)]
async fn persist_artifacts(
    output_root: &Path,
    repo_path: &Path,
    metadata: &SecurityReviewMetadata,
    bugs_markdown: &str,
    api_entries: &[ApiEntry],
    classification_rows: &[DataClassificationRow],
    classification_table: Option<&str>,
    report_markdown: Option<&str>,
    snapshot: &SecurityReviewSnapshot,
) -> Result<PersistedArtifacts, String> {
    let context_dir = output_root.join("context");
    tokio_fs::create_dir_all(&context_dir)
        .await
        .map_err(|e| format!("Failed to create {}: {e}", context_dir.display()))?;

    let bugs_path = context_dir.join("bugs.md");
    let sanitized_bugs = fix_mermaid_blocks(bugs_markdown);
    tokio_fs::write(&bugs_path, sanitized_bugs.as_bytes())
        .await
        .map_err(|e| format!("Failed to write {}: {e}", bugs_path.display()))?;

    let snapshot_path = context_dir.join("bugs_snapshot.json");
    let snapshot_bytes = serde_json::to_vec_pretty(snapshot)
        .map_err(|e| format!("Failed to serialize bug snapshot: {e}"))?;
    tokio_fs::write(&snapshot_path, snapshot_bytes)
        .await
        .map_err(|e| format!("Failed to write {}: {e}", snapshot_path.display()))?;

    let mut api_overview_path: Option<PathBuf> = None;
    if !api_entries.is_empty() {
        let mut content = String::new();
        for entry in api_entries {
            if entry.markdown.trim().is_empty() {
                continue;
            }
            content.push_str(&format!("## {}\n\n", entry.location_label));
            content.push_str(entry.markdown.trim());
            content.push_str("\n\n");
        }
        if !content.trim().is_empty() {
            let path = context_dir.join("apis.md");
            tokio_fs::write(&path, fix_mermaid_blocks(&content).as_bytes())
                .await
                .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
            api_overview_path = Some(path);
        }
    }

    let mut classification_json_path: Option<PathBuf> = None;
    let mut classification_table_path: Option<PathBuf> = None;
    if !classification_rows.is_empty() {
        let mut json_lines: Vec<String> = Vec::with_capacity(classification_rows.len());
        for row in classification_rows {
            let line = serde_json::to_string(row)
                .map_err(|e| format!("Failed to serialize classification row: {e}"))?;
            json_lines.push(line);
        }
        let json_path = context_dir.join("classification.jsonl");
        tokio_fs::write(&json_path, json_lines.join("\n").as_bytes())
            .await
            .map_err(|e| format!("Failed to write {}: {e}", json_path.display()))?;
        classification_json_path = Some(json_path);

        if let Some(table) = classification_table
            && !table.trim().is_empty()
        {
            let table_path = context_dir.join("classification.md");
            tokio_fs::write(&table_path, table.as_bytes())
                .await
                .map_err(|e| format!("Failed to write {}: {e}", table_path.display()))?;
            classification_table_path = Some(table_path);
        }
    }

    let mut report_html_path = None;
    let sanitized_report = report_markdown.map(fix_mermaid_blocks);
    let report_path = if let Some(report) = sanitized_report.as_ref() {
        let path = output_root.join("report.md");
        tokio_fs::write(&path, report.as_bytes())
            .await
            .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
        let repo_label = repo_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Security Review");
        let title = format!("{repo_label} Security Report");
        let html = build_report_html(&title, report);
        let html_path = output_root.join("report.html");
        tokio_fs::write(&html_path, html)
            .await
            .map_err(|e| format!("Failed to write {}: {e}", html_path.display()))?;
        report_html_path = Some(html_path);
        Some(path)
    } else {
        None
    };

    let metadata_path = output_root.join("metadata.json");
    let metadata_bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| format!("Failed to serialize metadata: {e}"))?;
    tokio_fs::write(&metadata_path, metadata_bytes)
        .await
        .map_err(|e| format!("Failed to write {}: {e}", metadata_path.display()))?;

    Ok(PersistedArtifacts {
        bugs_path,
        snapshot_path,
        report_path,
        report_html_path,
        metadata_path,
        api_overview_path,
        classification_json_path,
        classification_table_path,
    })
}

fn find_bug_index(snapshot: &SecurityReviewSnapshot, id: BugIdentifier) -> Option<usize> {
    match id {
        BugIdentifier::RiskRank(rank) => snapshot
            .bugs
            .iter()
            .position(|entry| entry.bug.risk_rank == Some(rank))
            .or_else(|| {
                snapshot
                    .bugs
                    .iter()
                    .position(|entry| entry.bug.summary_id == rank)
            }),
        BugIdentifier::SummaryId(summary_id) => snapshot
            .bugs
            .iter()
            .position(|entry| entry.bug.summary_id == summary_id),
    }
}

fn summarize_process_output(success: bool, stdout: &str, stderr: &str) -> String {
    let primary = if success { stdout } else { stderr };
    if let Some(line) = primary.lines().find(|line| !line.trim().is_empty()) {
        line.trim().to_string()
    } else if success {
        "Command succeeded".to_string()
    } else {
        "Command failed".to_string()
    }
}

fn build_bugs_markdown(
    snapshot: &SecurityReviewSnapshot,
    git_link_info: Option<&GitLinkInfo>,
) -> String {
    let bugs = snapshot_bugs(snapshot);
    let mut sections: Vec<String> = Vec::new();
    if let Some(table) = make_bug_summary_table_from_bugs(&bugs) {
        sections.push(table);
    }
    let details = render_bug_sections(&snapshot.bugs, git_link_info);
    if !details.trim().is_empty() {
        sections.push(details);
    }
    let combined = sections.join("\n\n");
    fix_mermaid_blocks(&combined)
}

async fn execute_bug_command(
    plan: BugCommandPlan,
    repo_path: PathBuf,
    work_dir: PathBuf,
) -> BugCommandResult {
    let mut logs: Vec<String> = Vec::new();
    let label = if let Some(rank) = plan.risk_rank {
        format!("#{rank} {}", plan.title)
    } else {
        format!("[{}] {}", plan.summary_id, plan.title)
    };
    logs.push(format!(
        "Running {} verification for {label}",
        plan.request.tool.as_str()
    ));

    let initial_target = plan.request.target.clone().filter(|t| !t.is_empty());
    let mut validation = BugValidationState {
        tool: Some(plan.request.tool.as_str().to_string()),
        target: initial_target,
        ..BugValidationState::default()
    };

    let start = Instant::now();

    match plan.request.tool {
        BugVerificationTool::Curl => {
            let Some(target) = plan.request.target.clone().filter(|t| !t.is_empty()) else {
                validation.status = BugValidationStatus::Failed;
                validation.summary = Some("Missing target URL".to_string());
                logs.push(format!("{label}: no target URL provided for curl"));
                validation.run_at = Some(OffsetDateTime::now_utc());
                return BugCommandResult {
                    index: plan.index,
                    validation,
                    logs,
                };
            };

            let mut command = Command::new("curl");
            command
                .arg("--silent")
                .arg("--show-error")
                .arg("--location")
                .arg("--max-time")
                .arg("15")
                .arg(&target)
                .current_dir(&repo_path);

            match command.output().await {
                Ok(output) => {
                    let duration = start.elapsed();
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let success = output.status.success();
                    validation.status = if success {
                        BugValidationStatus::Passed
                    } else {
                        BugValidationStatus::Failed
                    };
                    let summary_line = summarize_process_output(success, &stdout, &stderr);
                    let duration_label = fmt_elapsed_compact(duration.as_secs());
                    validation.summary = Some(format!("{summary_line} · {duration_label}"));
                    let snippet_source = if success { &stdout } else { &stderr };
                    let trimmed_snippet = snippet_source.trim();
                    if !trimmed_snippet.is_empty() {
                        validation.output_snippet =
                            Some(truncate_text(trimmed_snippet, VALIDATION_OUTPUT_GRAPHEMES));
                    }
                    logs.push(format!(
                        "{}: curl exited with status {}",
                        label, output.status
                    ));
                }
                Err(err) => {
                    validation.status = BugValidationStatus::Failed;
                    validation.summary = Some(format!("Failed to run curl: {err}"));
                    logs.push(format!("{label}: failed to run curl: {err}"));
                }
            }
        }
        BugVerificationTool::Python => {
            let script_path_owned: Option<PathBuf> =
                if let Some(path) = plan.request.script_path.as_ref() {
                    Some(path.clone())
                } else if let Some(code) = plan.request.script_inline.as_ref() {
                    let _ = tokio_fs::create_dir_all(&work_dir).await;
                    let file_name = if let Some(rank) = plan.risk_rank {
                        format!("bug_rank_{rank}.py")
                    } else {
                        format!("bug_{}.py", plan.summary_id)
                    };
                    let temp_path = work_dir.join(file_name);
                    if let Err(err) = tokio_fs::write(&temp_path, code.as_bytes()).await {
                        validation.status = BugValidationStatus::Failed;
                        validation.summary = Some(format!(
                            "Failed to write inline python to {}: {err}",
                            temp_path.display()
                        ));
                        logs.push(format!(
                            "{}: failed to write python script {}: {err}",
                            label,
                            temp_path.display()
                        ));
                        validation.run_at = Some(OffsetDateTime::now_utc());
                        return BugCommandResult {
                            index: plan.index,
                            validation,
                            logs,
                        };
                    }
                    Some(temp_path)
                } else {
                    None
                };

            let Some(script_path) = script_path_owned.as_ref() else {
                validation.status = BugValidationStatus::Failed;
                validation.summary = Some("Missing python script path".to_string());
                logs.push(format!("{label}: no python script provided"));
                validation.run_at = Some(OffsetDateTime::now_utc());
                return BugCommandResult {
                    index: plan.index,
                    validation,
                    logs,
                };
            };
            if !script_path.exists() {
                validation.status = BugValidationStatus::Failed;
                validation.summary =
                    Some(format!("Python script {} not found", script_path.display()));
                logs.push(format!(
                    "{}: python script {} not found",
                    label,
                    script_path.display()
                ));
                validation.run_at = Some(OffsetDateTime::now_utc());
                return BugCommandResult {
                    index: plan.index,
                    validation,
                    logs,
                };
            }

            let mut command = Command::new("python");
            command.arg(script_path);
            if let Some(target) = plan.request.target.as_ref().filter(|t| !t.is_empty()) {
                command.arg(target);
            }
            command.current_dir(&repo_path);

            match command.output().await {
                Ok(output) => {
                    let duration = start.elapsed();
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let success = output.status.success();
                    validation.status = if success {
                        BugValidationStatus::Passed
                    } else {
                        BugValidationStatus::Failed
                    };
                    let summary_line = summarize_process_output(success, &stdout, &stderr);
                    let duration_label = fmt_elapsed_compact(duration.as_secs());
                    validation.summary = Some(format!("{summary_line} · {duration_label}"));
                    let snippet_source = if success { &stdout } else { &stderr };
                    let trimmed_snippet = snippet_source.trim();
                    if !trimmed_snippet.is_empty() {
                        validation.output_snippet =
                            Some(truncate_text(trimmed_snippet, VALIDATION_OUTPUT_GRAPHEMES));
                    }
                    logs.push(format!(
                        "{}: python exited with status {}",
                        label, output.status
                    ));
                }
                Err(err) => {
                    validation.status = BugValidationStatus::Failed;
                    validation.summary = Some(format!("Failed to run python: {err}"));
                    logs.push(format!("{label}: failed to run python: {err}"));
                }
            }
        }
        BugVerificationTool::Playwright => {
            let Some(target) = plan.request.target.clone().filter(|t| !t.is_empty()) else {
                validation.status = BugValidationStatus::Failed;
                validation.summary = Some("Missing target URL".to_string());
                logs.push(format!("{label}: no target URL provided for playwright"));
                validation.run_at = Some(OffsetDateTime::now_utc());
                return BugCommandResult {
                    index: plan.index,
                    validation,
                    logs,
                };
            };
            let _ = tokio_fs::create_dir_all(&work_dir).await;
            let file_stem = if let Some(rank) = plan.risk_rank {
                format!("bug_rank_{rank}")
            } else {
                format!("bug_{}", plan.summary_id)
            };
            let screenshot_path = work_dir.join(format!("{file_stem}.png"));

            let mut command = Command::new("npx");
            command
                .arg("--yes")
                .arg("playwright")
                .arg("screenshot")
                .arg(&target)
                .arg(&screenshot_path)
                .current_dir(&repo_path);

            match command.output().await {
                Ok(output) => {
                    let duration = start.elapsed();
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let success = output.status.success();
                    validation.status = if success {
                        BugValidationStatus::Passed
                    } else {
                        BugValidationStatus::Failed
                    };
                    let duration_label = fmt_elapsed_compact(duration.as_secs());
                    if success {
                        validation.summary = Some(format!(
                            "Saved screenshot to {} · {duration_label}",
                            display_path_for(&screenshot_path, &repo_path)
                        ));
                    } else {
                        let summary_line = summarize_process_output(success, &stdout, &stderr);
                        validation.summary = Some(format!("{summary_line} · {duration_label}"));
                    }
                    let primary = if success { &stdout } else { &stderr };
                    let trimmed = primary.trim();
                    if !trimmed.is_empty() {
                        validation.output_snippet =
                            Some(truncate_text(trimmed, VALIDATION_OUTPUT_GRAPHEMES));
                    }
                    logs.push(format!(
                        "{}: playwright exited with status {}",
                        label, output.status
                    ));
                }
                Err(err) => {
                    validation.status = BugValidationStatus::Failed;
                    validation.summary = Some(format!("Failed to run playwright: {err}"));
                    logs.push(format!("{label}: failed to run playwright: {err}"));
                }
            }
        }
    }

    validation.run_at = Some(OffsetDateTime::now_utc());
    BugCommandResult {
        index: plan.index,
        validation,
        logs,
    }
}

pub(crate) async fn verify_bugs(
    batch: BugVerificationBatchRequest,
) -> Result<BugVerificationOutcome, BugVerificationFailure> {
    let mut logs: Vec<String> = Vec::new();

    let snapshot_bytes =
        tokio_fs::read(&batch.snapshot_path)
            .await
            .map_err(|err| BugVerificationFailure {
                message: format!("Failed to read {}: {err}", batch.snapshot_path.display()),
                logs: logs.clone(),
            })?;

    let mut snapshot: SecurityReviewSnapshot =
        serde_json::from_slice(&snapshot_bytes).map_err(|err| BugVerificationFailure {
            message: format!("Failed to parse {}: {err}", batch.snapshot_path.display()),
            logs: logs.clone(),
        })?;

    if batch.requests.is_empty() {
        logs.push("No verification requests provided.".to_string());
        let bugs = snapshot_bugs(&snapshot);
        return Ok(BugVerificationOutcome { bugs, logs });
    }

    let mut plans: Vec<BugCommandPlan> = Vec::new();
    for request in &batch.requests {
        let Some(index) = find_bug_index(&snapshot, request.id) else {
            return Err(BugVerificationFailure {
                message: "Requested bug identifier not found in snapshot".to_string(),
                logs,
            });
        };
        let entry = snapshot
            .bugs
            .get(index)
            .ok_or_else(|| BugVerificationFailure {
                message: "Snapshot bug index out of bounds".to_string(),
                logs: logs.clone(),
            })?;
        plans.push(BugCommandPlan {
            index,
            summary_id: entry.bug.summary_id,
            request: request.clone(),
            title: entry.bug.title.clone(),
            risk_rank: entry.bug.risk_rank,
        });
    }

    // Ensure work dir exists for artifacts/scripts
    let _ = tokio_fs::create_dir_all(&batch.work_dir).await;

    let mut command_results: Vec<BugCommandResult> = Vec::new();
    let mut futures = futures::stream::iter(plans.into_iter().map(|plan| {
        let repo_path = batch.repo_path.clone();
        let work_dir = batch.work_dir.clone();
        async move { execute_bug_command(plan, repo_path, work_dir).await }
    }))
    .buffer_unordered(8)
    .collect::<Vec<_>>()
    .await;

    command_results.append(&mut futures);

    for result in command_results {
        if let Some(entry) = snapshot.bugs.get_mut(result.index) {
            entry.bug.validation = result.validation;
            logs.extend(result.logs);
        }
    }

    let git_link_info = build_git_link_info(&batch.repo_path).await;
    let bugs_markdown = build_bugs_markdown(&snapshot, git_link_info.as_ref());

    tokio_fs::write(&batch.bugs_path, bugs_markdown.as_bytes())
        .await
        .map_err(|err| BugVerificationFailure {
            message: format!("Failed to write {}: {err}", batch.bugs_path.display()),
            logs: logs.clone(),
        })?;

    let mut sections = snapshot.report_sections_prefix.clone();
    if !bugs_markdown.trim().is_empty() {
        sections.push(format!("# Security Findings\n\n{}", bugs_markdown.trim()));
    }

    let report_markdown = if sections.is_empty() {
        None
    } else {
        Some(fix_mermaid_blocks(&sections.join("\n\n")))
    };

    if let Some(report_path) = batch.report_path.as_ref()
        && let Some(ref markdown) = report_markdown
    {
        tokio_fs::write(report_path, markdown.as_bytes())
            .await
            .map_err(|err| BugVerificationFailure {
                message: format!("Failed to write {}: {err}", report_path.display()),
                logs: logs.clone(),
            })?;

        if let Some(html_path) = batch.report_html_path.as_ref() {
            let repo_label = batch
                .repo_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Security Review");
            let title = format!("{repo_label} Security Report");
            let html = build_report_html(&title, markdown);
            tokio_fs::write(html_path, html)
                .await
                .map_err(|err| BugVerificationFailure {
                    message: format!("Failed to write {}: {err}", html_path.display()),
                    logs: logs.clone(),
                })?;
        }
    }

    let snapshot_bytes =
        serde_json::to_vec_pretty(&snapshot).map_err(|err| BugVerificationFailure {
            message: format!("Failed to serialize bug snapshot: {err}"),
            logs: logs.clone(),
        })?;
    tokio_fs::write(&batch.snapshot_path, snapshot_bytes)
        .await
        .map_err(|err| BugVerificationFailure {
            message: format!("Failed to write {}: {err}", batch.snapshot_path.display()),
            logs: logs.clone(),
        })?;

    let bugs = snapshot_bugs(&snapshot);
    Ok(BugVerificationOutcome { bugs, logs })
}

#[derive(Debug, Deserialize)]
struct AccountPlanItem {
    action: String,
    #[serde(default)]
    login_url: Option<String>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    script: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AccountsOutputJson {
    accounts: Vec<AccountPair>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AccountPair {
    username: String,
    password: String,
}

fn parse_accounts_inline(text: &str) -> Vec<AccountPair> {
    // Accept formats like: user:pass, user2:pass2
    let mut out = Vec::new();
    for chunk in text.split(',') {
        let part = chunk.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((u, p)) = part.split_once(':') {
            let u = u.trim();
            let p = p.trim();
            if !u.is_empty() && !p.is_empty() {
                out.push(AccountPair {
                    username: u.to_string(),
                    password: p.to_string(),
                });
            }
        }
    }
    out
}

async fn write_accounts(work_dir: &Path, creds: &[AccountPair]) -> Result<PathBuf, String> {
    let path = work_dir.join("credentials.json");
    let json = serde_json::to_vec_pretty(&AccountsOutputJson {
        accounts: creds.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    tokio_fs::write(&path, json)
        .await
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
    Ok(path)
}

#[allow(clippy::too_many_arguments)]
async fn setup_accounts(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    snapshot: &SecurityReviewSnapshot,
    work_dir: &Path,
    progress_sender: Option<AppEventSender>,
) -> Result<Option<Vec<AccountPair>>, String> {
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(
            "Preparing test accounts for validation...".to_string(),
        ));
    }

    let findings = build_validation_findings_context(snapshot);
    let prompt = VALIDATION_ACCOUNTS_PROMPT_TEMPLATE.replace("{findings}", &findings);
    let metrics = Arc::new(ReviewMetrics::default());
    let response = call_model(
        client,
        provider,
        auth,
        model,
        VALIDATION_ACCOUNTS_SYSTEM_PROMPT,
        &prompt,
        metrics,
        0.0,
    )
    .await
    .map_err(|e| format!("Account planning failed: {e}"))?;

    let mut chosen: Option<AccountPlanItem> = None;
    for line in response.text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(item) = serde_json::from_str::<AccountPlanItem>(trimmed) {
            chosen = Some(item);
            break;
        }
    }

    let Some(plan) = chosen else {
        return Ok(None);
    };

    if plan.action.eq_ignore_ascii_case("register")
        && plan.tool.as_deref().unwrap_or("") == "python"
        && plan.script.as_ref().is_some()
    {
        // Write and run inline script
        let _ = tokio_fs::create_dir_all(work_dir).await;
        let script_path = work_dir.join("register_accounts.py");
        let Some(code) = plan.script.as_ref() else {
            return Ok(None);
        };
        tokio_fs::write(&script_path, code.as_bytes())
            .await
            .map_err(|e| format!("Failed to write {}: {e}", script_path.display()))?;

        let mut cmd = Command::new("python");
        cmd.arg(&script_path);
        if let Some(url) = plan.login_url.as_ref() {
            cmd.arg(url);
        }
        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Failed to run python: {e}"))?;
        let success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !success {
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(format!(
                    "Account registration failed: {}",
                    summarize_process_output(false, &stdout, &stderr)
                )));
            }
            return Ok(None);
        }

        // Try to parse JSON from stdout
        let creds = if let Ok(json) = serde_json::from_str::<AccountsOutputJson>(stdout.trim()) {
            json.accounts
        } else {
            // best-effort: parse user:pass lines
            let pairs = parse_accounts_inline(stdout.trim());
            if pairs.len() >= 2 { pairs } else { Vec::new() }
        };
        if creds.len() < 2 {
            return Ok(None);
        }
        return Ok(Some(creds));
    }

    // Manual fallback
    if let Some(tx) = progress_sender.as_ref() {
        let (resp_tx, resp_rx) = oneshot::channel();
        tx.send(AppEvent::OpenRegistrationPrompt {
            url: plan.login_url.clone(),
            responder: resp_tx,
        });
        if let Ok(Some(input)) = resp_rx.await {
            let creds = parse_accounts_inline(&input);
            if creds.len() >= 2 {
                return Ok(Some(creds));
            }
        }
    }
    Ok(None)
}
#[derive(Debug, Deserialize)]
struct ValidationPlanItem {
    id_kind: String,
    id_value: usize,
    tool: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    script: Option<String>,
}

fn is_high_risk(bug: &SecurityReviewBug) -> bool {
    let sev = bug.severity.to_ascii_lowercase();
    if sev.contains("critical") || sev.contains("high") {
        return true;
    }
    if let Some(rank) = bug.risk_rank {
        return rank <= 5;
    }
    false
}

fn build_validation_findings_context(snapshot: &SecurityReviewSnapshot) -> String {
    let mut selected: Vec<&BugSnapshot> = snapshot
        .bugs
        .iter()
        .filter(|b| is_high_risk(&b.bug))
        .collect();
    // Keep to a reasonable number to bound prompt size
    selected.sort_by_key(|b| b.bug.risk_rank.unwrap_or(usize::MAX));
    if selected.len() > 6 {
        selected.truncate(6);
    }
    let mut out = String::new();
    for item in selected {
        let rank = item
            .bug
            .risk_rank
            .map(|r| format!("#{r}"))
            .unwrap_or_else(|| "N/A".to_string());
        let types = if item.bug.verification_types.is_empty() {
            "[]".to_string()
        } else {
            format!("{:?}", item.bug.verification_types)
        };
        // Include the original markdown so the model can infer concrete targets
        let _ = writeln!(
            &mut out,
            "- id_kind: {}\n  id_value: {}\n  risk_rank: {}\n  title: {}\n  severity: {}\n  verification_types: {}\n  details:\n{}\n---\n",
            if item.bug.risk_rank.is_some() {
                "risk_rank"
            } else {
                "summary_id"
            },
            item.bug.risk_rank.unwrap_or(item.bug.summary_id),
            rank,
            item.bug.title,
            item.bug.severity,
            types,
            indent_block(&item.original_markdown, 2)
        );
    }
    out
}

fn indent_block(s: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    s.lines()
        .map(|l| format!("{pad}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_web_validation(
    repo_path: PathBuf,
    snapshot_path: PathBuf,
    bugs_path: PathBuf,
    report_path: Option<PathBuf>,
    report_html_path: Option<PathBuf>,
    provider: ModelProviderInfo,
    auth: Option<CodexAuth>,
    model: String,
    progress_sender: Option<AppEventSender>,
) -> Result<(), BugVerificationFailure> {
    // Load snapshot
    let bytes = tokio_fs::read(&snapshot_path)
        .await
        .map_err(|e| BugVerificationFailure {
            message: format!("Failed to read {}: {e}", snapshot_path.display()),
            logs: vec![],
        })?;
    let snapshot: SecurityReviewSnapshot =
        serde_json::from_slice(&bytes).map_err(|e| BugVerificationFailure {
            message: format!("Failed to parse {}: {e}", snapshot_path.display()),
            logs: vec![],
        })?;

    let client = create_client();
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(
            "Planning web/API validation for high-risk findings...".to_string(),
        ));
    }

    // Ensure we have test accounts before validation
    let work_dir = snapshot_path
        .parent()
        .map(|p| p.join("validation"))
        .unwrap_or_else(|| repo_path.join(".codex_validation"));
    let _ = tokio_fs::create_dir_all(&work_dir).await;
    if let Some(creds) = setup_accounts(
        &client,
        &provider,
        &auth,
        &model,
        &snapshot,
        &work_dir,
        progress_sender.clone(),
    )
    .await
    .map_err(|e| BugVerificationFailure {
        message: e,
        logs: vec![],
    })? {
        let path = write_accounts(&work_dir, &creds)
            .await
            .map_err(|e| BugVerificationFailure {
                message: e,
                logs: vec![],
            })?;
        if let Some(tx) = progress_sender.as_ref() {
            let names: Vec<String> = creds.iter().map(|p| p.username.clone()).collect();
            tx.send(AppEvent::SecurityReviewLog(format!(
                "Registered {} test accounts: {} (stored in {})",
                creds.len(),
                names.join(", "),
                display_path_for(&path, &repo_path)
            )));
        }
    } else if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(
            "Proceeding without auto-registered accounts; user may have registered manually."
                .to_string(),
        ));
    }

    // Build prompt
    let findings = build_validation_findings_context(&snapshot);
    let prompt = VALIDATION_PLAN_PROMPT_TEMPLATE.replace("{findings}", &findings);

    let metrics = Arc::new(ReviewMetrics::default());
    let response = call_model(
        &client,
        &provider,
        &auth,
        &model,
        VALIDATION_PLAN_SYSTEM_PROMPT,
        &prompt,
        metrics.clone(),
        0.0,
    )
    .await
    .map_err(|err| BugVerificationFailure {
        message: format!("Validation planning failed: {err}"),
        logs: vec![],
    })?;

    let mut logs: Vec<String> = Vec::new();
    if let Some(reasoning) = response.reasoning.as_ref() {
        log_model_reasoning(reasoning, &progress_sender, &None, &mut logs);
    }

    let mut requests: Vec<BugVerificationRequest> = Vec::new();
    for line in response.text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: ValidationPlanItem = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if parsed.id_kind == "setup" {
            // Already handled by setup_accounts() pre-step; skip.
            continue;
        }
        let id = match parsed.id_kind.as_str() {
            "risk_rank" => BugIdentifier::RiskRank(parsed.id_value),
            "summary_id" => BugIdentifier::SummaryId(parsed.id_value),
            _ => continue,
        };
        let tool = match parsed.tool.to_ascii_lowercase().as_str() {
            "playwright" => BugVerificationTool::Playwright,
            "curl" => BugVerificationTool::Curl,
            "python" => BugVerificationTool::Python,
            _ => continue,
        };
        requests.push(BugVerificationRequest {
            id,
            tool,
            target: parsed.target.clone(),
            script_path: None,
            script_inline: parsed.script.clone(),
        });
    }

    let batch = BugVerificationBatchRequest {
        snapshot_path,
        bugs_path,
        report_path,
        report_html_path,
        repo_path,
        work_dir,
        requests,
    };
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(
            "Executing validation checks...".to_string(),
        ));
    }
    let outcome = verify_bugs(batch).await?;
    for line in outcome.logs {
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(line.clone()));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ModelCallOutput {
    text: String,
    reasoning: Option<String>,
}

async fn make_provider_request_builder(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    path: &str,
) -> Result<CodexRequestBuilder, String> {
    // Base URL: allow provider overrides, otherwise default to the standard OpenAI-style endpoint.
    let mut base_url = provider
        .base_url
        .clone()
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    base_url = base_url.trim_end_matches('/').to_string();

    let path = path.trim_start_matches('/');
    let mut url = base_url;
    if !path.is_empty() {
        url.push('/');
        url.push_str(path);
    }

    if let Some(params) = provider.query_params.as_ref()
        && !params.is_empty()
    {
        let qs = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        url.push('?');
        url.push_str(&qs);
    }

    // All model calls here use POST.
    let mut builder = client.post(url);

    if let Some(headers) = provider.http_headers.as_ref() {
        for (name, value) in headers {
            if let (Ok(header_name), Ok(header_value)) = (
                HeaderName::try_from(name.as_str()),
                HeaderValue::try_from(value.as_str()),
            ) {
                builder = builder.header(header_name, header_value);
            }
        }
    }

    if let Some(env_headers) = provider.env_http_headers.as_ref() {
        for (header, env_var) in env_headers {
            if let Ok(val) = std::env::var(env_var)
                && !val.trim().is_empty()
                && let (Ok(header_name), Ok(header_value)) = (
                    HeaderName::try_from(header.as_str()),
                    HeaderValue::try_from(val.as_str()),
                )
            {
                builder = builder.header(header_name, header_value);
            }
        }
    }

    // Authorization: prefer provider API key, then experimental token, then user auth.
    match provider.api_key() {
        Ok(Some(api_key)) if !api_key.trim().is_empty() => {
            builder = builder.bearer_auth(api_key);
            return Ok(builder);
        }
        Ok(None) => {}
        Ok(Some(_)) => {}
        Err(err) => {
            return Err(err.to_string());
        }
    }

    if let Some(token) = provider.experimental_bearer_token.as_ref()
        && !token.trim().is_empty()
    {
        builder = builder.bearer_auth(token);
        return Ok(builder);
    }

    if let Some(auth) = auth.as_ref()
        && let Ok(token) = auth.get_token().await
        && !token.trim().is_empty()
    {
        builder = builder.bearer_auth(token);
    }

    Ok(builder)
}

#[allow(clippy::too_many_arguments)]
async fn call_model(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    metrics: Arc<ReviewMetrics>,
    temperature: f32,
) -> Result<ModelCallOutput, String> {
    // Ensure multiple retries for transient issues like rate limits (5 total attempts minimum).
    let max_attempts = provider.request_max_retries().max(4);
    let mut attempt_errors: Vec<String> = Vec::new();

    for attempt in 0..=max_attempts {
        metrics.record_model_call();

        match call_model_attempt(
            client,
            provider,
            auth,
            model,
            system_prompt,
            user_prompt,
            temperature,
            metrics.clone(),
        )
        .await
        {
            Ok(output) => return Ok(output),
            Err(err) => {
                let sanitized = sanitize_model_error(&err);
                attempt_errors.push(format!("attempt {}: {}", attempt + 1, sanitized));

                if attempt == max_attempts {
                    let attempt_count = attempt + 1;
                    let plural = if attempt_count == 1 { "" } else { "s" };
                    let joined = attempt_errors.join("\n- ");
                    return Err(format!(
                        "Model request for {model} failed after {attempt_count} attempt{plural}. Details:\n- {joined}"
                    ));
                }

                if let Some(delay) = retry_after_duration(&sanitized) {
                    let jitter_ms = rand::random_range(250..=1250);
                    let jitter = Duration::from_millis(jitter_ms);
                    let total_delay = delay.saturating_add(jitter);
                    metrics.record_rate_limit_wait(total_delay);
                    let wait_secs = total_delay.as_secs_f32();
                    let base_secs = delay.as_secs_f32();
                    let jitter_ms_display = jitter.as_millis();
                    let attempt_number = attempt + 1;
                    let total_attempts = max_attempts + 1;
                    let log_line = format!(
                        "Rate limit: {sanitized}; waiting {wait_secs:.1}s (base {base_secs:.1}s + jitter {jitter_ms_display}ms, attempt {attempt_number}/{total_attempts})."
                    );
                    tracing::warn!(
                        model = model,
                        wait_seconds = wait_secs,
                        base_seconds = base_secs,
                        jitter_millis = jitter_ms_display,
                        attempt = attempt_number,
                        total_attempts,
                        %sanitized,
                        %log_line,
                        "Rate limit hit."
                    );
                    sleep(total_delay).await;
                } else {
                    let base = default_retry_backoff(attempt + 1);
                    let jitter_ms = rand::random_range(250..=750);
                    let jitter = Duration::from_millis(jitter_ms);
                    let total_delay = base.saturating_add(jitter);
                    metrics.record_rate_limit_wait(total_delay);
                    sleep(total_delay).await;
                }
            }
        }
    }

    unreachable!("call_model attempts should always return");
}

#[allow(clippy::too_many_arguments)]
async fn call_model_attempt(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f32,
    metrics: Arc<ReviewMetrics>,
) -> Result<ModelCallOutput, String> {
    match provider.wire_api {
        WireApi::Responses => {
            let builder =
                make_provider_request_builder(client, provider, auth, "responses").await?;

            let mut payload = json!({
                "model": model,
                "instructions": system_prompt,
                "input": [
                    {
                        "role": "user",
                        "content": [
                            { "type": "input_text", "text": user_prompt }
                        ]
                    }
                ]
            });
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("store".to_string(), json!(false));
                obj.insert("stream".to_string(), json!(true));
            }

            let response = builder
                .header(ACCEPT, "text/event-stream")
                .json(&payload)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            let status = response.status();
            let body = response.text().await.map_err(|e| e.to_string())?;

            if !status.is_success() {
                let is_unsupported = status == reqwest::StatusCode::BAD_REQUEST
                    && body.to_ascii_lowercase().contains("unsupported model");
                if is_unsupported {
                    let mut fallback = provider.clone();
                    fallback.wire_api = WireApi::Chat;
                    return send_chat_request(
                        client,
                        &fallback,
                        auth,
                        model,
                        system_prompt,
                        user_prompt,
                        temperature,
                        metrics.clone(),
                    )
                    .await;
                }
                return Err(format!("Model request failed with status {status}: {body}"));
            }

            match parse_responses_stream_output(&body, &metrics) {
                Ok(output) => Ok(output),
                Err(err) => {
                    let snippet = truncate_text(&body, 400);
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
                        // parse_responses_output may not include usage; nothing extra to record here.
                        parse_responses_output(value).map_err(|fallback_err| {
                            format!(
                                "{err}; fallback parse failed: {fallback_err}. Response snippet: {snippet}"
                            )
                        })
                    } else {
                        Err(format!(
                            "{err}. This usually means the provider returned non-JSON (missing credentials, network restrictions, or proxy HTML). Response snippet: {snippet}"
                        ))
                    }
                }
            }
        }
        WireApi::Chat => {
            send_chat_request(
                client,
                provider,
                auth,
                model,
                system_prompt,
                user_prompt,
                temperature,
                metrics,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn send_chat_request(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f32,
    metrics: Arc<ReviewMetrics>,
) -> Result<ModelCallOutput, String> {
    let builder = make_provider_request_builder(client, provider, auth, "chat/completions").await?;

    let mut payload = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ]
    });
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("temperature".to_string(), json!(temperature));
    }

    let response = builder
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let body_bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let body_text = String::from_utf8_lossy(&body_bytes).to_string();

    if !status.is_success() {
        return Err(format!(
            "Model request failed with status {status}: {body_text}"
        ));
    }

    let value = match serde_json::from_slice::<serde_json::Value>(&body_bytes) {
        Ok(value) => value,
        Err(err) => {
            let snippet = truncate_text(&body_text, 400);
            return Err(format!(
                "error decoding response body: {err}. This usually means the provider returned non-JSON (missing credentials, network restrictions, or proxy HTML). Response snippet: {snippet}"
            ));
        }
    };

    // Try to record token usage if present in chat response
    if let Some(usage) = value.get("usage") {
        let input_tokens = usage
            .get("input_tokens")
            .and_then(serde_json::Value::as_u64)
            .or_else(|| {
                usage
                    .get("prompt_tokens")
                    .and_then(serde_json::Value::as_u64)
            })
            .unwrap_or(0);
        let cached_input_tokens = usage
            .get("input_tokens_details")
            .and_then(|d| d.get("cached_tokens").and_then(serde_json::Value::as_u64))
            .or_else(|| {
                usage
                    .get("prompt_tokens_details")
                    .and_then(|d| d.get("cached_tokens").and_then(serde_json::Value::as_u64))
            })
            .unwrap_or(0);
        let output_tokens = usage
            .get("output_tokens")
            .and_then(serde_json::Value::as_u64)
            .or_else(|| {
                usage
                    .get("completion_tokens")
                    .and_then(serde_json::Value::as_u64)
            })
            .unwrap_or(0);
        let reasoning_output_tokens = usage
            .get("output_tokens_details")
            .and_then(|d| {
                d.get("reasoning_tokens")
                    .and_then(serde_json::Value::as_u64)
            })
            .or_else(|| {
                usage.get("completion_tokens_details").and_then(|d| {
                    d.get("reasoning_tokens")
                        .and_then(serde_json::Value::as_u64)
                })
            })
            .unwrap_or(0);
        let total_tokens = usage
            .get("total_tokens")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(input_tokens.saturating_add(output_tokens));
        metrics.record_usage_raw(
            i64::try_from(input_tokens).unwrap_or(i64::MAX),
            i64::try_from(cached_input_tokens).unwrap_or(i64::MAX),
            i64::try_from(output_tokens).unwrap_or(i64::MAX),
            i64::try_from(reasoning_output_tokens).unwrap_or(i64::MAX),
            i64::try_from(total_tokens).unwrap_or(i64::MAX),
        );
    }

    parse_chat_output(value).map_err(|err| {
        let snippet = truncate_text(&body_text, 400);
        format!("{err}; response snippet: {snippet}")
    })
}

fn sanitize_model_error(error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.is_empty() {
        return "unknown error".to_string();
    }

    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn retry_after_duration(message: &str) -> Option<Duration> {
    // Look for patterns like "Please try again in 11.054s" or "retry after 12s".
    // Keep this permissive; the server may include decimals.
    let re = Regex::new(r"(?i)(?:try again in|retry after)\s+([0-9]+(?:\.[0-9]+)?)s").ok()?;
    let caps = re.captures(message)?;
    let raw = caps.get(1)?.as_str();
    raw.parse::<f64>()
        .ok()
        .and_then(|secs| Duration::try_from_secs_f64(secs).ok())
}

fn parse_responses_stream_output(
    body: &str,
    metrics: &ReviewMetrics,
) -> Result<ModelCallOutput, String> {
    let mut combined = String::new();
    let mut reasoning = String::new();
    let mut fallback: Option<serde_json::Value> = None;
    let mut failed_error: Option<String> = None;
    let mut last_parse_error: Option<String> = None;

    let mut data_buffer = String::new();

    for raw_line in body.lines() {
        let line = raw_line.trim_end_matches('\r');

        if let Some(rest) = line.strip_prefix("data:") {
            if !data_buffer.is_empty() {
                data_buffer.push('\n');
            }
            data_buffer.push_str(rest.trim_start());
        } else if line.trim().is_empty() && !data_buffer.is_empty() {
            handle_responses_event(
                &data_buffer,
                &mut combined,
                &mut reasoning,
                &mut fallback,
                &mut failed_error,
                &mut last_parse_error,
                metrics,
            );
            data_buffer.clear();
        }
    }

    if !data_buffer.is_empty() {
        handle_responses_event(
            &data_buffer,
            &mut combined,
            &mut reasoning,
            &mut fallback,
            &mut failed_error,
            &mut last_parse_error,
            metrics,
        );
    }

    if let Some(err) = failed_error {
        return Err(err);
    }

    if !combined.trim().is_empty() {
        return Ok(ModelCallOutput {
            text: combined.trim().to_string(),
            reasoning: normalize_reasoning(reasoning),
        });
    }

    if let Some(value) = fallback {
        return parse_responses_output(value);
    }

    if let Some(err) = last_parse_error {
        return Err(format!("Unable to parse response output: {err}"));
    }

    Err("Unable to parse response output".to_string())
}

fn handle_responses_event(
    data: &str,
    combined: &mut String,
    reasoning: &mut String,
    fallback: &mut Option<serde_json::Value>,
    failed_error: &mut Option<String>,
    last_parse_error: &mut Option<String>,
    metrics: &ReviewMetrics,
) {
    let trimmed = data.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return;
    }

    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(event) => {
            let Some(kind) = event.get("type").and_then(|v| v.as_str()) else {
                return;
            };

            match kind {
                "response.output_text.delta" => {
                    if let Some(delta) = event.get("delta").and_then(|v| v.as_str()) {
                        combined.push_str(delta);
                    }
                }
                "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => {
                    if let Some(delta) = event.get("delta").and_then(|v| v.as_str()) {
                        reasoning.push_str(delta);
                    } else if let Some(delta_obj) = event.get("delta").and_then(|v| v.as_object()) {
                        if let Some(text) = delta_obj
                            .get("text")
                            .and_then(|v| v.as_str())
                            .filter(|t| !t.is_empty())
                        {
                            reasoning.push_str(text);
                        }
                        if let Some(content) = delta_obj.get("content").and_then(|v| v.as_array()) {
                            for block in content {
                                if let Some(text) = block
                                    .get("text")
                                    .and_then(|v| v.as_str())
                                    .filter(|t| !t.is_empty())
                                {
                                    reasoning.push_str(text);
                                }
                            }
                        }
                    }
                }
                "response.completed" => {
                    if let Some(resp) = event.get("response") {
                        *fallback = Some(resp.clone());
                        if let Some(usage) = resp.get("usage")
                            && let Some(input_tokens) = usage
                                .get("input_tokens")
                                .and_then(serde_json::Value::as_u64)
                                .or_else(|| {
                                    usage
                                        .get("prompt_tokens")
                                        .and_then(serde_json::Value::as_u64)
                                })
                        {
                            let cached_input_tokens = usage
                                .get("input_tokens_details")
                                .and_then(|d| {
                                    d.get("cached_tokens").and_then(serde_json::Value::as_u64)
                                })
                                .or_else(|| {
                                    usage.get("prompt_tokens_details").and_then(|d| {
                                        d.get("cached_tokens").and_then(serde_json::Value::as_u64)
                                    })
                                })
                                .unwrap_or(0);
                            let output_tokens = usage
                                .get("output_tokens")
                                .and_then(serde_json::Value::as_u64)
                                .or_else(|| {
                                    usage
                                        .get("completion_tokens")
                                        .and_then(serde_json::Value::as_u64)
                                })
                                .unwrap_or(0);
                            let reasoning_output_tokens = usage
                                .get("output_tokens_details")
                                .and_then(|d| {
                                    d.get("reasoning_tokens")
                                        .and_then(serde_json::Value::as_u64)
                                })
                                .or_else(|| {
                                    usage.get("completion_tokens_details").and_then(|d| {
                                        d.get("reasoning_tokens")
                                            .and_then(serde_json::Value::as_u64)
                                    })
                                })
                                .unwrap_or(0);
                            let total_tokens = usage
                                .get("total_tokens")
                                .and_then(serde_json::Value::as_u64)
                                .unwrap_or(input_tokens.saturating_add(output_tokens));
                            metrics.record_usage_raw(
                                i64::try_from(input_tokens).unwrap_or(i64::MAX),
                                i64::try_from(cached_input_tokens).unwrap_or(i64::MAX),
                                i64::try_from(output_tokens).unwrap_or(i64::MAX),
                                i64::try_from(reasoning_output_tokens).unwrap_or(i64::MAX),
                                i64::try_from(total_tokens).unwrap_or(i64::MAX),
                            );
                        }
                    }
                }
                "response.failed" => {
                    if failed_error.is_some() {
                        return;
                    }
                    let error = event.get("response").and_then(|resp| resp.get("error"));
                    let message = error
                        .and_then(|err| err.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("Model response failed");
                    if let Some(code) = error
                        .and_then(|err| err.get("code"))
                        .and_then(|c| c.as_str())
                    {
                        *failed_error = Some(format!("{message} (code: {code})"));
                    } else {
                        *failed_error = Some(message.to_string());
                    }
                }
                _ => {}
            }
        }
        Err(err) => {
            if last_parse_error.is_none() {
                *last_parse_error = Some(format!("failed to parse SSE event: {err}"));
            }
        }
    }
}

fn parse_responses_output(value: serde_json::Value) -> Result<ModelCallOutput, String> {
    if let Some(array) = value.get("output").and_then(|v| v.as_array()) {
        let mut combined = String::new();
        let mut reasoning = String::new();
        for item in array {
            match item.get("type").and_then(|t| t.as_str()) {
                Some("output_text") | Some("text") => {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        combined.push_str(text);
                    }
                }
                Some("message") => {
                    if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
                        for block in content {
                            match block.get("type").and_then(|t| t.as_str()) {
                                Some("text") | Some("output_text") => {
                                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                        combined.push_str(text);
                                    }
                                }
                                Some("reasoning") => {
                                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                        reasoning.push_str(text);
                                    }
                                }
                                _ => {}
                            };
                        }
                    }
                }
                _ => {}
            }
        }
        if !combined.trim().is_empty() {
            return Ok(ModelCallOutput {
                text: combined.trim().to_string(),
                reasoning: normalize_reasoning(reasoning)
                    .or_else(|| extract_reasoning_from_value(&value)),
            });
        }
    }

    if let Some(texts) = value.get("output_text").and_then(|v| v.as_array()) {
        let merged = texts
            .iter()
            .filter_map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !merged.trim().is_empty() {
            return Ok(ModelCallOutput {
                text: merged.trim().to_string(),
                reasoning: extract_reasoning_from_value(&value),
            });
        }
    }

    if let Some(reasoning) = extract_reasoning_from_value(&value)
        && let Some(text) = value
            .get("text")
            .and_then(|t| t.as_str())
            .or_else(|| value.get("output").and_then(|v| v.as_str()))
        && !text.trim().is_empty()
    {
        return Ok(ModelCallOutput {
            text: text.trim().to_string(),
            reasoning: Some(reasoning),
        });
    }

    Err("Unable to parse response output".to_string())
}

fn extract_reasoning_from_value(value: &serde_json::Value) -> Option<String> {
    fn dfs(node: &serde_json::Value, buffer: &mut String, in_reason_context: bool) {
        match node {
            Value::String(text) => {
                if in_reason_context {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        if !buffer.is_empty() && !buffer.ends_with(' ') {
                            buffer.push(' ');
                        }
                        buffer.push_str(trimmed);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    dfs(item, buffer, in_reason_context);
                }
            }
            Value::Object(map) => {
                let mut reason_context = in_reason_context;
                if let Some(obj_type) = map
                    .get("type")
                    .and_then(|t| t.as_str())
                    .map(str::to_ascii_lowercase)
                    && obj_type.contains("reasoning")
                {
                    reason_context = true;
                }
                for (key, val) in map {
                    let key_lower = key.to_ascii_lowercase();
                    let key_is_reason = key_lower.contains("reasoning")
                        || key_lower == "reasoning_text"
                        || key_lower == "reasoning_summary"
                        || key_lower == "reasoning_content"
                        || (reason_context
                            && matches!(
                                key_lower.as_str(),
                                "text" | "content" | "delta" | "message" | "parts"
                            ));
                    dfs(val, buffer, reason_context || key_is_reason);
                }
            }
            _ => {}
        }
    }

    let mut buffer = String::new();
    dfs(value, &mut buffer, false);
    normalize_reasoning(buffer)
}

fn parse_chat_output(value: serde_json::Value) -> Result<ModelCallOutput, String> {
    if let Some(choice) = value
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        && let Some(message) = choice.get("message")
        && let Some(content) = message.get("content")
    {
        if let Some(text) = content.as_str() {
            if !text.trim().is_empty() {
                return Ok(ModelCallOutput {
                    text: text.trim().to_string(),
                    reasoning: message
                        .get("reasoning")
                        .and_then(|r| r.as_str())
                        .map(|s| s.trim().to_string())
                        .and_then(normalize_reasoning)
                        .or_else(|| extract_reasoning_from_value(&value)),
                });
            }
        } else if let Some(array) = content.as_array() {
            let mut combined = String::new();
            let mut reasoning = String::new();
            for item in array {
                if let Some(part_text) = item.get("text").and_then(|t| t.as_str()) {
                    combined.push_str(part_text);
                    if !combined.ends_with('\n') {
                        combined.push('\n');
                    }
                }
                if let Some(reason_text) = item.get("reasoning").and_then(|r| r.as_str()) {
                    reasoning.push_str(reason_text);
                }
            }
            if !combined.trim().is_empty() {
                return Ok(ModelCallOutput {
                    text: combined.trim().to_string(),
                    reasoning: normalize_reasoning(reasoning)
                        .or_else(|| extract_reasoning_from_value(&value)),
                });
            }
        }
    }

    Err("Unable to parse chat completion output".to_string())
}

fn format_revision_label(commit: &str, branch: Option<&String>, timestamp: Option<i64>) -> String {
    let short = if commit.len() > 12 {
        commit[..12].to_string()
    } else {
        commit.to_string()
    };
    let date_str = timestamp.and_then(|ts| {
        OffsetDateTime::from_unix_timestamp(ts)
            .ok()
            .and_then(|dt| dt.format(&format_description!("[year]-[month]-[day]")).ok())
    });
    let mut parts: Vec<String> = Vec::new();
    parts.push(short);
    if let Some(date) = date_str {
        parts.push(date);
    }
    if let Some(branch_name) = branch
        && !branch_name.trim().is_empty()
    {
        parts.push(format!("branch {branch_name}"));
    }
    format!("Analyzed revision: {}", parts.join(" · "))
}

fn strip_operational_considerations_section(markdown: &str) -> String {
    let mut lines_out: Vec<String> = Vec::new();
    let mut skipping = false;

    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let heading_text = trimmed.trim_start_matches('#').trim_start();
            let is_operational = heading_text
                .to_ascii_lowercase()
                .starts_with("operational considerations");
            if is_operational {
                skipping = true;
                continue;
            }
            if skipping {
                skipping = false;
            }
        }
        if !skipping {
            lines_out.push(line.to_string());
        }
    }

    lines_out.join("\n")
}

fn ensure_threat_model_heading(markdown: String) -> String {
    let trimmed = markdown.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("# threat model")
        || lower.starts_with("## threat model")
        || lower.contains("\n# threat model")
        || lower.contains("\n## threat model")
    {
        return markdown;
    }
    let mut out = String::new();
    out.push_str("## Threat Model\n\n");
    out.push_str(trimmed);
    out
}

fn human_readable_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

const MARKDOWN_FIX_MODEL: &str = "gpt-5.1";
