#![allow(dead_code)]

use crate::app_event::AppEvent;
use crate::app_event::SecurityReviewAutoScopeSelection;
use crate::app_event::SecurityReviewCommandState;
use crate::app_event_sender::AppEventSender;
use crate::diff_render::display_path_for;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::history_cell;
use crate::mermaid::fix_mermaid_blocks;
use crate::security_prompts::*;
use crate::security_report_viewer::build_report_html;
use crate::status_indicator_widget::fmt_elapsed_compact;
use crate::text_formatting::truncate_text;
use base64::Engine;
use codex_client::CodexHttpClient;
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
use codex_protocol::openai_models::ReasoningEffort;
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
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fmt::Write;
use std::fs::OpenOptions;
use std::fs::{self};
use std::future::Future;
use std::hash::Hash;
use std::hash::Hasher;
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
use tokio::sync::Semaphore;
use tokio::sync::oneshot;
use tokio::task::spawn_blocking;
use tokio::time::sleep;
use url::Url;

const VALIDATION_SUMMARY_GRAPHEMES: usize = 96;
const VALIDATION_OUTPUT_GRAPHEMES: usize = 480;
const VALIDATION_REPORT_OUTPUT_MAX_BYTES: usize = 512 * 1024;
const VALIDATION_WORKER_MAX_CONCURRENCY: usize = 8;
const VALIDATION_PLAN_CONCURRENCY: usize = VALIDATION_WORKER_MAX_CONCURRENCY;
const VALIDATION_REFINE_CONCURRENCY: usize = VALIDATION_WORKER_MAX_CONCURRENCY;
const VALIDATION_EXEC_CONCURRENCY: usize = VALIDATION_WORKER_MAX_CONCURRENCY;
const VALIDATION_AGENT_TIMEOUT_SECS: u64 = 60 * 60;
const POST_VALIDATION_REFINE_WORKER_TIMEOUT_SECS: u64 = 30 * 60;
const VALIDATION_EXEC_TIMEOUT_SECS: u64 = 30 * 60;
const VALIDATION_PREFLIGHT_TIMEOUT_SECS: u64 = 30 * 60;
const VALIDATION_CURL_TIMEOUT_SECS: u64 = 60;
const VALIDATION_PLAYWRIGHT_TIMEOUT_SECS: u64 = 5 * 60;
const VALIDATION_TESTING_CONTEXT_MAX_CHARS: usize = 12_000;
const VALIDATION_TARGET_PREP_MAX_TURNS: usize = 3;

const VALIDATION_TESTING_SECTION_HEADER: &str = "## Validation prerequisites";
const VALIDATION_TESTING_SECTION_INTRO: &str = "This section is shared across findings; follow it before running per-bug validation scripts or Dockerfiles. Commands should only appear here after they have been executed successfully during validation target preparation.";
const VALIDATION_TARGET_SECTION_HEADER: &str = "## Validation target";
const VALIDATION_TARGET_SECTION_INTRO: &str =
    "This section records the deployed target URL and any credentials used for web validation.";
const WEB_VALIDATION_CREDS_FILE_NAME: &str = "web_validation_creds.json";

//

// Heuristic limits inspired by the AppSec review agent to keep prompts manageable.
const DEFAULT_MAX_FILES: usize = usize::MAX;
const DEFAULT_MAX_BYTES_PER_FILE: usize = 500_000; // ~488 KiB
const DEFAULT_MAX_TOTAL_BYTES: usize = 500 * 1024 * 1024; // ~500 MiB
const MAX_PROMPT_BYTES: usize = 9_000_000; // ~8.6 MiB safety margin under API cap
const MAX_CONCURRENT_FILE_ANALYSIS: usize = 32;
const FILE_TRIAGE_PREVIEW_CHARS: usize = 200;
const FILE_TRIAGE_CHUNK_SIZE: usize = 10;
const FILE_TRIAGE_CONCURRENCY: usize = 32;
const DIR_TRIAGE_LOG_LIMIT: usize = 30;
const MAX_SEARCH_REQUESTS_PER_FILE: usize = 3;
const MAX_SEARCH_OUTPUT_CHARS: usize = 4_000;
const MAX_COMMAND_ERROR_RETRIES: usize = 10;
const MAX_SEARCH_PATTERN_LEN: usize = 256;
const MAX_FILE_SEARCH_RESULTS: usize = 40;
// Number of full passes over the triaged files during bug finding.
// Not related to per-file search/tool attempts. Defaults to 3.
const BUG_FINDING_PASSES: usize = 1;
const BUG_POLISH_CONCURRENCY: usize = 8;
const BUG_DEDUP_CONFIDENCE_THRESHOLD: f32 = 0.85;
const BUG_LLM_DEDUP_CONCURRENCY: usize = 32;
const BUG_LLM_DEDUP_LINE_BUCKET_SIZE: u32 = 25;
const BUG_LLM_DEDUP_LOW_SEVERITY_MAX_FINDINGS: usize = 100;
const BUG_LLM_DEDUP_REASON_LOG_LIMIT: usize = 12;
const BUG_DEDUP_PROMPT_MAX_CHARS: usize = 120_000;
const BUG_LLM_DEDUP_PASS1_CACHE_FILE: &str = "llm_dedupe_pass1_cache.json";
const COMMAND_PREVIEW_MAX_LINES: usize = 2;
const COMMAND_PREVIEW_MAX_GRAPHEMES: usize = 96;
const MODEL_REASONING_LOG_MAX_GRAPHEMES: usize = 240;
const BUG_SCOPE_PROMPT_MAX_GRAPHEMES: usize = 600;
const BUG_REPOSITORY_SUMMARY_MAX_CHARS: usize = 20_000;
const BUG_SPEC_CONTEXT_MAX_CHARS: usize = 40_000;
const BUG_FILE_CONTEXT_FALLBACK_MAX_CHARS: usize = 60_000;
const BUG_FILE_CONTEXT_RETRY_MAX_CHARS: usize = 20_000;
const ANALYSIS_CONTEXT_MAX_CHARS: usize = 6_000;
const AUTO_SCOPE_MODEL: &str = "gpt-5.3-codex";
const FILE_TRIAGE_MODEL: &str = "gpt-5.2-codex";
const SPEC_GENERATION_MODEL: &str = "gpt-5.2";
const BUG_MODEL: &str = "gpt-5.2";
const DEFAULT_VALIDATION_MODEL: &str = "gpt-5.2";
const THREAT_MODEL_MODEL: &str = "gpt-5.3-codex";
const CLASSIFICATION_PROMPT_SPEC_LIMIT: usize = 16_000;
// prompts moved to `security_prompts.rs`
const BUG_RERANK_CHUNK_SIZE: usize = 1;
const BUG_RERANK_MAX_CONCURRENCY: usize = 32;
const BUG_RERANK_CONTEXT_MAX_CHARS: usize = 6_000;
const BUG_RERANK_MAX_COMMAND_ERRORS: usize = 10;
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

static EXCLUDED_DIR_NAMES: [&str; 20] = [
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "vendor",
    ".venv",
    "__pycache__",
    "test",
    "tests",
    "__tests__",
    "dist",
    "build",
    "buck-out",
    "bazel-out",
    "coverage",
    ".pytest_cache",
    ".idea",
    ".vscode",
    ".cache",
    "target",
];

fn path_has_excluded_dir_component(path: &Path) -> bool {
    let mut comps = path.components().peekable();
    while let Some(comp) = comps.next() {
        if comps.peek().is_none() {
            break;
        }
        if let std::path::Component::Normal(part) = comp
            && let Some(name) = part.to_str()
            && EXCLUDED_DIR_NAMES
                .iter()
                .any(|excluded| excluded.eq_ignore_ascii_case(name))
        {
            return true;
        }
    }
    false
}

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
            .unwrap_or_else(|error| panic!("failed to compile Linear issue regex: {error}"))
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
    let mut value = serde_json::to_value(checkpoint).map_err(std::io::Error::other)?;
    if let Value::Object(map) = &mut value
        && let Some(plan_statuses) = map.get_mut("plan_statuses")
        && let Value::Object(plan_map) = plan_statuses
    {
        let mut ordered: Map<String, Value> = Map::new();
        let mut known: HashSet<String> = HashSet::new();

        for step in plan_steps_for_mode(checkpoint.mode) {
            let slug = plan_step_slug(step.kind).to_string();
            known.insert(slug.clone());
            if let Some(value) = plan_map.get(&slug) {
                ordered.insert(slug, value.clone());
            }
        }

        let mut extras: Vec<(String, Value)> = plan_map
            .iter()
            .filter(|(key, _)| !known.contains(key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        extras.sort_by(|a, b| a.0.cmp(&b.0));
        for (key, value) in extras {
            ordered.insert(key, value);
        }

        *plan_map = ordered;
    }
    let bytes = serde_json::to_vec_pretty(&value).map_err(std::io::Error::other)?;
    fs::write(path, bytes)
}

#[derive(Clone, Debug)]
pub struct RunningSecurityReviewCandidate {
    pub output_root: PathBuf,
    pub checkpoint: SecurityReviewCheckpoint,
}

pub(crate) fn plan_progress_and_current_step(
    statuses: &HashMap<String, StepStatus>,
    mode: SecurityReviewMode,
) -> (usize, usize, Option<String>) {
    let steps = plan_steps_for_mode(mode);
    let total = steps.len();
    let mut completed = 0usize;
    let mut current: Option<String> = None;

    for step in &steps {
        let slug = plan_step_slug(step.kind);
        match statuses.get(slug) {
            Some(StepStatus::Completed) => completed = completed.saturating_add(1),
            Some(StepStatus::InProgress) => {
                if current.is_none() {
                    current = Some(step.title.clone());
                }
            }
            _ => {}
        }
    }

    if current.is_none() {
        current = steps
            .iter()
            .find(|step| {
                let slug = plan_step_slug(step.kind);
                !matches!(statuses.get(slug), Some(StepStatus::Completed))
            })
            .map(|step| step.title.clone());
    }

    (completed, total, current)
}

#[derive(Debug, Clone)]
pub(crate) struct SecurityReviewResumeProgress {
    pub completed_steps: usize,
    pub total_steps: usize,
    pub percent: usize,
    pub current_step: Option<String>,
    pub detail: Option<String>,
}

pub(crate) fn resume_progress_snapshot(
    checkpoint: &SecurityReviewCheckpoint,
    output_root: &Path,
) -> SecurityReviewResumeProgress {
    let repo_root = checkpoint.repo_root.as_path();
    let steps = plan_steps_for_mode(checkpoint.mode);
    let total_steps = steps.len();

    let mut completed_steps = 0usize;
    let mut current_step: Option<String> = None;
    let mut total_fraction: f64 = 0.0;
    let mut detail_parts: Vec<String> = Vec::new();

    let spec_progress = resume_spec_generation_progress(output_root, repo_root);
    let bug_progress = resume_bug_analysis_progress(output_root);
    let validation_prep_complete = validation_target_prep_complete(output_root, repo_root);

    for step in &steps {
        let slug = plan_step_slug(step.kind);
        let mut status = checkpoint
            .plan_statuses
            .get(slug)
            .cloned()
            .unwrap_or(StepStatus::Pending);
        if !matches!(status, StepStatus::Completed)
            && step.kind == SecurityReviewPlanStep::PrepareValidationTargets
            && validation_prep_complete
        {
            status = StepStatus::Completed;
        }

        match status {
            StepStatus::Completed => {
                completed_steps = completed_steps.saturating_add(1);
                total_fraction += 1.0;
            }
            StepStatus::InProgress => {
                if current_step.is_none() {
                    current_step = Some(step.title.clone());
                }

                let progress_fraction = match step.kind {
                    SecurityReviewPlanStep::GenerateSpecs => spec_progress
                        .as_ref()
                        .and_then(ResumeCountProgress::fraction)
                        .inspect(|_| {
                            if let Some(progress) = spec_progress.as_ref() {
                                detail_parts.push(progress.label("spec dirs"));
                            }
                        }),
                    SecurityReviewPlanStep::AnalyzeBugs => bug_progress
                        .as_ref()
                        .and_then(ResumeCountProgress::fraction)
                        .inspect(|_| {
                            if let Some(progress) = bug_progress.as_ref() {
                                detail_parts.push(progress.label("files"));
                            }
                        }),
                    _ => None,
                };

                if let Some(fraction) = progress_fraction {
                    total_fraction += fraction.clamp(0.0, 1.0);
                }
            }
            StepStatus::Pending => {}
        }
    }

    if current_step.is_none() {
        current_step = steps
            .iter()
            .find(|step| {
                let slug = plan_step_slug(step.kind);
                !matches!(
                    checkpoint.plan_statuses.get(slug),
                    Some(StepStatus::Completed)
                )
            })
            .map(|step| step.title.clone());
    }

    let percent = if total_steps == 0 {
        0
    } else {
        ((total_fraction / total_steps as f64) * 100.0)
            .round()
            .clamp(0.0, 100.0) as usize
    };

    let detail = detail_parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("; ");

    SecurityReviewResumeProgress {
        completed_steps,
        total_steps,
        percent,
        current_step,
        detail: (!detail.is_empty()).then_some(detail),
    }
}

#[derive(Debug, Clone, Copy)]
struct ResumeCountProgress {
    done: usize,
    total: usize,
}

impl ResumeCountProgress {
    fn fraction(&self) -> Option<f64> {
        if self.total == 0 {
            return None;
        }
        Some(self.done.min(self.total) as f64 / self.total as f64)
    }

    fn label(&self, noun: &str) -> String {
        format!("{noun}: {}/{}", self.done.min(self.total), self.total)
    }
}

fn resume_spec_generation_progress(
    output_root: &Path,
    repo_root: &Path,
) -> Option<ResumeCountProgress> {
    let path = output_root
        .join("context")
        .join("spec_generation_progress.json");
    let bytes = fs::read(path).ok()?;
    let progress = serde_json::from_slice::<SpecGenerationProgress>(&bytes).ok()?;
    if progress.version != 1 {
        return None;
    }
    if progress.repo_root != repo_root.display().to_string() {
        return None;
    }
    let total = progress.targets.len();
    let done = progress.completed.len();
    Some(ResumeCountProgress { done, total })
}

fn resume_bug_analysis_progress(output_root: &Path) -> Option<ResumeCountProgress> {
    let context_dir = output_root.join("context");
    let mut best_pass: Option<(i32, PathBuf)> = None;

    if let Ok(entries) = fs::read_dir(&context_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(pass_str) = file_name
                .strip_prefix("bug_analysis_pass_")
                .and_then(|rest| rest.strip_suffix(".json"))
            else {
                continue;
            };
            let Ok(pass) = pass_str.parse::<i32>() else {
                continue;
            };
            let should_replace = best_pass.as_ref().is_none_or(|(best, _)| pass > *best);
            if should_replace {
                best_pass = Some((pass, path));
            }
        }
    }

    let path = best_pass.map(|(_, path)| path).or_else(|| {
        let fallback = context_dir.join("bug_analysis_pass_1.json");
        fallback.exists().then_some(fallback)
    })?;

    let bytes = fs::read(path).ok()?;
    let progress = serde_json::from_slice::<BugAnalysisProgress>(&bytes).ok()?;
    if progress.version != 1 {
        return None;
    }

    let total = usize::try_from(progress.total_files.max(0)).unwrap_or(0);
    let done = progress.files.len();
    Some(ResumeCountProgress { done, total })
}

fn bug_analysis_progress_paths(output_root: &Path) -> Vec<(i32, PathBuf)> {
    let context_dir = output_root.join("context");
    let mut progress_paths: Vec<(i32, PathBuf)> = Vec::new();

    if let Ok(entries) = fs::read_dir(&context_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(pass_str) = file_name
                .strip_prefix("bug_analysis_pass_")
                .and_then(|rest| rest.strip_suffix(".json"))
            else {
                continue;
            };
            let Ok(pass) = pass_str.parse::<i32>() else {
                continue;
            };
            progress_paths.push((pass, path));
        }
    }

    progress_paths.sort_by_key(|(pass, _)| *pass);
    progress_paths
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ValidationTargetPrepProgress {
    version: i32,
    repo_root: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    local_build_ok: bool,
    #[serde(default)]
    local_run_ok: bool,
    #[serde(default)]
    docker_build_ok: bool,
    #[serde(default)]
    docker_run_ok: bool,
    #[serde(default)]
    local_entrypoint: Option<String>,
    #[serde(default)]
    local_build_command: Option<String>,
    #[serde(default)]
    local_smoke_command: Option<String>,
    #[serde(default)]
    dockerfile_path: Option<String>,
    #[serde(default)]
    docker_image_tag: Option<String>,
    #[serde(default)]
    docker_build_command: Option<String>,
    #[serde(default)]
    docker_smoke_command: Option<String>,
}

struct BugAnalysisProgressLoadOutcome {
    bug_summaries: Vec<BugSummary>,
    bug_details: Vec<BugDetail>,
    files_with_findings: Vec<FileSnippet>,
    logs: Vec<String>,
}

async fn load_bug_summaries_from_bug_analysis_progress(
    output_root: &Path,
    repo_root: &Path,
    selected_snippets: &[FileSnippet],
) -> Result<Option<BugAnalysisProgressLoadOutcome>, SecurityReviewFailure> {
    let paths = bug_analysis_progress_paths(output_root);
    if paths.is_empty() {
        return Ok(None);
    }

    let mut snippet_index_by_path: HashMap<PathBuf, usize> = HashMap::new();
    for (index, snippet) in selected_snippets.iter().enumerate() {
        snippet_index_by_path.insert(snippet.relative_path.clone(), index);
    }

    let mut bug_summaries: Vec<BugSummary> = Vec::new();
    let mut bug_details: Vec<BugDetail> = Vec::new();
    let mut files_with_findings: Vec<FileSnippet> = Vec::new();
    let mut logs: Vec<String> = Vec::new();
    let mut next_summary_id = 1usize;

    for (pass, path) in paths {
        let Some(progress) = read_bug_analysis_progress(&path).await? else {
            continue;
        };
        if progress.version != 1 {
            continue;
        }
        logs.push(format!(
            "Reusing bug analysis pass {pass} from {} for dedupe/polish.",
            display_path_for(&path, repo_root)
        ));

        let mut files = progress.files;
        files.sort_by_key(|entry| entry.index);
        for entry in files {
            let Some(section) = entry.bug_section else {
                continue;
            };
            if section.trim().is_empty() {
                continue;
            }

            let snippet_index = snippet_index_by_path
                .get(&PathBuf::from(entry.relative_path.as_str()))
                .copied()
                .or_else(|| usize::try_from(entry.index).ok())
                .filter(|index| *index < selected_snippets.len());
            let Some(snippet_index) = snippet_index else {
                continue;
            };

            let snippet = &selected_snippets[snippet_index];
            let file_path = snippet.relative_path.display().to_string();
            let (mut summaries, mut details) = extract_bug_summaries(
                &section,
                &file_path,
                snippet.relative_path.as_path(),
                &mut next_summary_id,
            );
            bug_summaries.append(&mut summaries);
            bug_details.append(&mut details);
            files_with_findings.push(snippet.clone());
        }
    }

    if bug_summaries.is_empty() {
        return Ok(None);
    }

    files_with_findings.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files_with_findings.dedup_by(|a, b| a.relative_path == b.relative_path);

    Ok(Some(BugAnalysisProgressLoadOutcome {
        bug_summaries,
        bug_details,
        files_with_findings,
        logs,
    }))
}

fn validation_target_prep_progress_path(output_root: &Path) -> PathBuf {
    output_root
        .join("context")
        .join("validation_target_prep.json")
}

fn validation_target_prep_matches_repo(
    progress: &ValidationTargetPrepProgress,
    repo_root: &Path,
) -> bool {
    if progress.repo_root == repo_root.display().to_string() {
        return true;
    }

    let Ok(repo_root) = repo_root.canonicalize() else {
        return false;
    };
    let Ok(progress_root) = PathBuf::from(progress.repo_root.as_str()).canonicalize() else {
        return false;
    };

    progress_root == repo_root
}

fn validation_target_prep_marker(
    output_root: &Path,
    repo_root: &Path,
) -> Option<ValidationTargetPrepProgress> {
    let path = validation_target_prep_progress_path(output_root);
    let bytes = fs::read(path).ok()?;
    let progress = serde_json::from_slice::<ValidationTargetPrepProgress>(&bytes).ok()?;
    validation_target_prep_matches_repo(&progress, repo_root).then_some(progress)
}

fn validation_target_prep_complete(output_root: &Path, repo_root: &Path) -> bool {
    validation_target_prep_marker(output_root, repo_root).is_some_and(|progress| {
        progress.version == 3 && progress.local_build_ok && progress.local_run_ok
    })
}

fn reconcile_validation_target_prep_resume_state(
    output_root: &Path,
    repo_root: &Path,
    plan_tracker: &mut SecurityReviewPlanTracker,
    progress_sender: &Option<AppEventSender>,
    log_sink: &Option<Arc<SecurityReviewLogSink>>,
    logs: &mut Vec<String>,
) -> bool {
    let validation_prep_marker = validation_target_prep_marker(output_root, repo_root);
    let validation_prep_complete = validation_prep_marker.as_ref().is_some_and(|progress| {
        progress.version == 3 && progress.local_build_ok && progress.local_run_ok
    });

    if validation_prep_marker.is_some() {
        if validation_prep_complete {
            push_progress_log(
                progress_sender,
                log_sink,
                logs,
                format!(
                    "Prepare runnable validation targets: existing prep marker found at {}; skipping.",
                    validation_target_prep_progress_path(output_root).display()
                ),
            );
        }
        plan_tracker.mark_complete(SecurityReviewPlanStep::PrepareValidationTargets);
    } else if matches!(
        plan_tracker.status_for(SecurityReviewPlanStep::PrepareValidationTargets),
        Some(StepStatus::InProgress)
    ) {
        push_progress_log(
            progress_sender,
            log_sink,
            logs,
            "Prepare runnable validation targets: checkpoint marked this step in progress, but no prep marker was found; rerunning."
                .to_string(),
        );
        plan_tracker.reset_step(SecurityReviewPlanStep::PrepareValidationTargets);
    }

    validation_prep_complete
}

#[cfg(test)]
mod validation_target_prep_resume_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    fn write_validation_target_prep_progress(
        output_root: &Path,
        progress: &ValidationTargetPrepProgress,
    ) {
        let path = validation_target_prep_progress_path(output_root);
        fs::create_dir_all(path.parent().expect("progress parent")).expect("create dir");
        let bytes = serde_json::to_vec_pretty(progress).expect("serialize progress");
        fs::write(&path, bytes).expect("write progress");
    }

    fn new_plan_tracker(repo_root: &Path) -> SecurityReviewPlanTracker {
        let include_paths: Vec<PathBuf> = Vec::new();
        SecurityReviewPlanTracker::new(
            SecurityReviewMode::Bugs,
            &include_paths,
            repo_root,
            HashMap::new(),
            None,
            None,
        )
    }

    #[test]
    fn does_not_reset_completed_prepare_validation_targets_when_marker_exists_but_not_ready() {
        let repo_dir = tempdir().expect("repo dir");
        let output_dir = tempdir().expect("output dir");
        let repo_root = repo_dir.path();
        let output_root = output_dir.path();

        let progress = ValidationTargetPrepProgress {
            version: 3,
            repo_root: repo_root.display().to_string(),
            summary: None,
            local_build_ok: false,
            local_run_ok: false,
            docker_build_ok: false,
            docker_run_ok: false,
            local_entrypoint: None,
            local_build_command: None,
            local_smoke_command: None,
            dockerfile_path: None,
            docker_image_tag: None,
            docker_build_command: None,
            docker_smoke_command: None,
        };
        write_validation_target_prep_progress(output_root, &progress);

        let mut plan_tracker = new_plan_tracker(repo_root);
        plan_tracker.mark_complete(SecurityReviewPlanStep::PrepareValidationTargets);

        let mut logs = Vec::new();
        let complete = reconcile_validation_target_prep_resume_state(
            output_root,
            repo_root,
            &mut plan_tracker,
            &None,
            &None,
            &mut logs,
        );

        assert_eq!(complete, false);
        assert!(
            matches!(
                plan_tracker.status_for(SecurityReviewPlanStep::PrepareValidationTargets),
                Some(StepStatus::Completed)
            ),
            "PrepareValidationTargets should remain completed"
        );
    }

    #[test]
    fn resets_in_progress_prepare_validation_targets_when_marker_missing() {
        let repo_dir = tempdir().expect("repo dir");
        let output_dir = tempdir().expect("output dir");
        let repo_root = repo_dir.path();
        let output_root = output_dir.path();

        let mut plan_tracker = new_plan_tracker(repo_root);
        plan_tracker.start_step(SecurityReviewPlanStep::PrepareValidationTargets);

        let mut logs = Vec::new();
        let complete = reconcile_validation_target_prep_resume_state(
            output_root,
            repo_root,
            &mut plan_tracker,
            &None,
            &None,
            &mut logs,
        );

        assert_eq!(complete, false);
        assert!(
            matches!(
                plan_tracker.status_for(SecurityReviewPlanStep::PrepareValidationTargets),
                Some(StepStatus::Pending)
            ),
            "PrepareValidationTargets should be reset to pending"
        );
        assert_eq!(logs.len(), 1);
    }
}

pub fn running_review_candidates(repo_path: &Path) -> Vec<RunningSecurityReviewCandidate> {
    let storage_root = security_review_storage_root(repo_path);
    let entries = match fs::read_dir(storage_root) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut candidates: Vec<(OffsetDateTime, String, PathBuf, SecurityReviewCheckpoint)> =
        Vec::new();
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
        candidates.push((checkpoint.started_at, name, path, checkpoint));
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    candidates
        .into_iter()
        .map(|(_, _, path, checkpoint)| RunningSecurityReviewCandidate {
            output_root: path,
            checkpoint,
        })
        .collect()
}

pub fn latest_running_review_candidate(repo_path: &Path) -> Option<RunningSecurityReviewCandidate> {
    running_review_candidates(repo_path).into_iter().next()
}

pub fn latest_completed_review_candidate(
    repo_path: &Path,
) -> Option<RunningSecurityReviewCandidate> {
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
        if checkpoint.status != SecurityReviewCheckpointStatus::Complete {
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
    pub spec_model: String,
    pub model: String,
    pub validation_model: String,
    pub writing_model: String,
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
    // Optional deployed target for web/API validation (enables curl/playwright tools).
    pub validation_target_url: Option<String>,
    // Optional credentials file for web validation (headers only; values redacted in output).
    pub validation_creds_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct SecurityReviewSetupResult {
    pub logs: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SecurityReviewRerunTarget {
    PrepareValidationTargets,
}

impl SecurityReviewRerunTarget {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            SecurityReviewRerunTarget::PrepareValidationTargets => "prepare_validation_targets",
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            SecurityReviewRerunTarget::PrepareValidationTargets => "Prepare validation targets",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SecurityReviewRerunResult {
    pub target: SecurityReviewRerunTarget,
    pub repo_root: PathBuf,
    pub output_root: PathBuf,
    pub testing_md_path: PathBuf,
    pub success: bool,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SecurityReviewCheckpointStatus {
    #[default]
    Running,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityReviewCheckpoint {
    #[serde(default)]
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
    pub(crate) triaged_dirs: Option<Vec<String>>,
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SecurityReviewPlanStep {
    GenerateSpecs,
    ThreatModel,
    DirTriage,
    FileTriage,
    AnalyzeBugs,
    PolishFindings,
    PrepareValidationTargets,
    ValidateFindings,
    PostValidationRefine,
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
    step_models: HashMap<SecurityReviewPlanStep, PlanStepModelInfo>,
    snapshots_enabled: bool,
}

#[derive(Clone)]
struct PlanStepModelInfo {
    model: String,
    reasoning_effort: Option<ReasoningEffort>,
}

impl SecurityReviewPlanTracker {
    fn new(
        mode: SecurityReviewMode,
        scope_paths: &[PathBuf],
        repo_root: &Path,
        step_models: HashMap<SecurityReviewPlanStep, PlanStepModelInfo>,
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
            step_models,
            snapshots_enabled: false,
        };
        tracker.emit_update();
        tracker
    }

    fn enable_snapshots(&mut self) {
        self.snapshots_enabled = true;
    }

    fn complete_and_start_next(
        &mut self,
        finished: SecurityReviewPlanStep,
        next: Option<SecurityReviewPlanStep>,
    ) {
        let mut changed = self.set_status_if_present(finished, StepStatus::Completed);
        if let Some(next_step) = next
            && !matches!(self.status_for(next_step), Some(StepStatus::Completed))
        {
            changed |= self.set_status_if_present(next_step, StepStatus::InProgress);
        }
        if changed {
            self.emit_update();
            if self.snapshots_enabled {
                self.emit_plan_snapshot();
            }
        }
    }

    fn start_step(&mut self, step: SecurityReviewPlanStep) {
        if matches!(self.status_for(step), Some(StepStatus::Completed)) {
            return;
        }

        if self.set_status_if_present(step, StepStatus::InProgress) {
            self.emit_update();
            if self.snapshots_enabled {
                self.emit_plan_snapshot();
            }
        }
    }

    fn mark_complete(&mut self, step: SecurityReviewPlanStep) {
        if self.set_status_if_present(step, StepStatus::Completed) {
            self.emit_update();
            if self.snapshots_enabled {
                self.emit_plan_snapshot();
            }
        }
    }

    fn mark_steps_complete(&mut self, steps: &[SecurityReviewPlanStep]) {
        let mut changed = false;
        for step in steps {
            changed |= self.set_status_if_present(*step, StepStatus::Completed);
        }
        if changed {
            self.emit_update();
            if self.snapshots_enabled {
                self.emit_plan_snapshot();
            }
        }
    }

    fn reset_step(&mut self, step: SecurityReviewPlanStep) {
        let Some(entry) = self.steps.iter_mut().find(|item| item.kind == step) else {
            return;
        };

        entry.status = StepStatus::Pending;
        entry.started_at = None;
        entry.completed_at = None;

        self.emit_update();
        if self.snapshots_enabled {
            self.emit_plan_snapshot();
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

    fn normalize_validation_in_progress(&mut self) -> bool {
        let mut found = false;
        let mut changed = false;
        for step in [
            SecurityReviewPlanStep::ValidateFindings,
            SecurityReviewPlanStep::PostValidationRefine,
            SecurityReviewPlanStep::AssembleReport,
        ] {
            let Some(entry) = self.steps.iter_mut().find(|item| item.kind == step) else {
                continue;
            };
            if !matches!(entry.status, StepStatus::InProgress) {
                continue;
            }
            if found {
                entry.status = StepStatus::Pending;
                entry.started_at = None;
                entry.completed_at = None;
                changed = true;
            } else {
                found = true;
            }
        }
        changed
    }

    fn emit_update(&self) {
        let summary = self.build_log_summary();
        if let Some(sender) = self.sender.as_ref() {
            sender.send(AppEvent::SecurityReviewLog(summary.clone()));
        }
        write_log_sink(&self.log_sink, summary.as_str());
    }

    fn emit_plan_snapshot(&self) {
        let Some(sender) = self.sender.as_ref() else {
            return;
        };

        let plan_items: Vec<PlanItemArg> = self
            .steps
            .iter()
            .map(|step| {
                let model_info = self.step_models.get(&step.kind);
                PlanItemArg {
                    step: build_step_title(step),
                    status: step.status.clone(),
                    model: model_info.map(|info| info.model.clone()),
                    reasoning_effort: model_info.and_then(|info| info.reasoning_effort),
                }
            })
            .collect();
        sender.send(AppEvent::InsertHistoryCell(Box::new(
            history_cell::new_plan_update(UpdatePlanArgs {
                explanation: Some(self.explanation.clone()),
                plan: plan_items,
            }),
        )));
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
        changed |= self.normalize_validation_in_progress();
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

    steps.push(SecurityReviewPlanItem::new(
        SecurityReviewPlanStep::DirTriage,
        "Triage directories",
    ));
    steps.push(SecurityReviewPlanItem::new(
        SecurityReviewPlanStep::FileTriage,
        "Triage files",
    ));

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
        SecurityReviewPlanStep::PrepareValidationTargets,
        "Prepare runnable validation targets",
    ));
    steps.push(SecurityReviewPlanItem::new(
        SecurityReviewPlanStep::ValidateFindings,
        "Validate findings",
    ));
    steps.push(SecurityReviewPlanItem::new(
        SecurityReviewPlanStep::PostValidationRefine,
        "Post-validation PoC refinement",
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
        SecurityReviewPlanStep::DirTriage => "dir_triage",
        SecurityReviewPlanStep::FileTriage => "file_triage",
        SecurityReviewPlanStep::AnalyzeBugs => "analyze_bugs",
        SecurityReviewPlanStep::PolishFindings => "polish_findings",
        SecurityReviewPlanStep::PrepareValidationTargets => "prepare_validation_targets",
        SecurityReviewPlanStep::AssembleReport => "assemble_report",
        SecurityReviewPlanStep::ValidateFindings => "validate_findings",
        SecurityReviewPlanStep::PostValidationRefine => "post_validation_refine",
    }
}

fn plan_step_from_slug(slug: &str) -> Option<SecurityReviewPlanStep> {
    match slug {
        "generate_specs" => Some(SecurityReviewPlanStep::GenerateSpecs),
        "threat_model" => Some(SecurityReviewPlanStep::ThreatModel),
        "dir_triage" => Some(SecurityReviewPlanStep::DirTriage),
        "file_triage" => Some(SecurityReviewPlanStep::FileTriage),
        "analyze_bugs" => Some(SecurityReviewPlanStep::AnalyzeBugs),
        "polish_findings" => Some(SecurityReviewPlanStep::PolishFindings),
        "prepare_validation_targets" => Some(SecurityReviewPlanStep::PrepareValidationTargets),
        "assemble_report" => Some(SecurityReviewPlanStep::AssembleReport),
        "validate_findings" => Some(SecurityReviewPlanStep::ValidateFindings),
        "post_validation_refine" => Some(SecurityReviewPlanStep::PostValidationRefine),
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

fn dedupe_file_snippets(snippets: &mut Vec<FileSnippet>) -> usize {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let original = snippets.len();
    snippets.retain(|snippet| seen.insert(snippet.relative_path.clone()));
    original.saturating_sub(snippets.len())
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

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
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
    error_message: Option<String>,
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
                    let mut boundary = MAX_SEARCH_OUTPUT_CHARS;
                    while boundary > 0 && !text.is_char_boundary(boundary) {
                        boundary -= 1;
                    }
                    text.truncate(boundary);
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
                                let mut boundary = MAX_SEARCH_OUTPUT_CHARS;
                                while boundary > 0 && !text.is_char_boundary(boundary) {
                                    boundary -= 1;
                                }
                                text.truncate(boundary);
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
    UnableToValidate,
}

fn validation_status_command_state(status: BugValidationStatus) -> SecurityReviewCommandState {
    match status {
        BugValidationStatus::Passed => SecurityReviewCommandState::Matches,
        BugValidationStatus::Failed => SecurityReviewCommandState::NoMatches,
        BugValidationStatus::UnableToValidate => SecurityReviewCommandState::Error,
        BugValidationStatus::Pending => SecurityReviewCommandState::Running,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BugValidationState {
    pub status: BugValidationStatus,
    pub tool: Option<String>,
    pub target: Option<String>,
    pub summary: Option<String>,
    pub output_snippet: Option<String>,
    #[serde(default)]
    pub repro_steps: Vec<String>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    pub control_target: Option<String>,
    pub control_summary: Option<String>,
    pub control_output_snippet: Option<String>,
    #[serde(default)]
    pub control_steps: Vec<String>,
    pub control_stdout_path: Option<String>,
    pub control_stderr_path: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<String>,
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
    expect_asan: bool,
}

struct BugCommandResult {
    index: usize,
    validation: BugValidationState,
    logs: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

#[derive(Clone)]
struct WebValidationConfig {
    base_url: Url,
    headers: Vec<(String, String)>,
    redactions: Vec<String>,
}

impl std::fmt::Debug for WebValidationConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header_names: Vec<&str> = self.headers.iter().map(|(k, _)| k.as_str()).collect();
        f.debug_struct("WebValidationConfig")
            .field("base_url", &self.base_url.as_str())
            .field("headers", &header_names)
            .finish()
    }
}

impl WebValidationConfig {
    fn origin(&self) -> String {
        self.base_url.origin().ascii_serialization()
    }

    fn redact(&self, text: &str) -> String {
        let mut scrubbed = text.to_string();
        for secret in &self.redactions {
            if secret.is_empty() {
                continue;
            }
            scrubbed = scrubbed.replace(secret, "[REDACTED]");
        }
        scrubbed
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
    web_validation: Option<WebValidationConfig>,
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

fn render_bug_sections(
    snapshots: &[BugSnapshot],
    git_link_info: Option<&GitLinkInfo>,
    repo_root: Option<&Path>,
    output_root: Option<&Path>,
) -> String {
    let mut sections: Vec<String> = Vec::new();
    let mut ordered: Vec<&BugSnapshot> = snapshots.iter().collect();
    ordered.sort_by(|a, b| match (a.bug.risk_rank, b.bug.risk_rank) {
        (Some(ra), Some(rb)) => ra.cmp(&rb),
        (Some(_), None) => CmpOrdering::Less,
        (None, Some(_)) => CmpOrdering::Greater,
        _ => severity_rank(&a.bug.severity)
            .cmp(&severity_rank(&b.bug.severity))
            .then_with(|| a.bug.summary_id.cmp(&b.bug.summary_id)),
    });

    for snapshot in ordered {
        let base_raw = snapshot.original_markdown.trim();
        if base_raw.is_empty() {
            continue;
        }
        let base_owned = prune_bug_markdown_file_lines_for_reporting(base_raw);
        let base = base_owned.trim();
        let anchor_snippet = format!("<a id=\"bug-{}\"", snapshot.bug.summary_id);
        let linked = linkify_file_lines(base, git_link_info);
        let linked =
            strip_standalone_bug_anchor_lines(linked.as_str()).unwrap_or_else(|| linked.clone());
        let mut composed = if linked.contains(&anchor_snippet) {
            linked
        } else {
            rewrite_bug_markdown_heading_id(linked.as_str(), snapshot.bug.summary_id)
                .unwrap_or(linked)
        };
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
        composed.push_str("\n\n#### Validation\n");
        let expects_asan = expects_asan_for_bug(&snapshot.bug);
        let status_label = validation_status_label(&snapshot.bug.validation);
        composed.push_str(&format!("- **Status:** {status_label}\n"));
        if let Some(tool) = snapshot
            .bug
            .validation
            .tool
            .as_ref()
            .filter(|tool| !tool.is_empty())
        {
            composed.push_str(&format!("- **Tool:** `{tool}`\n"));
        }
        if let Some(target) = snapshot
            .bug
            .validation
            .target
            .as_ref()
            .filter(|target| !target.is_empty())
        {
            composed.push_str(&format!("- **Target:** `{target}`\n"));
        }
        if let Some(control_target) = snapshot
            .bug
            .validation
            .control_target
            .as_ref()
            .filter(|target| !target.is_empty())
        {
            composed.push_str(&format!("- **Control target:** `{control_target}`\n"));
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
            let summary = summary.trim();
            composed.push_str(&format!("- **Summary:** {summary}\n"));
        }
        if let Some(control_summary) = snapshot
            .bug
            .validation
            .control_summary
            .as_ref()
            .filter(|summary| !summary.is_empty())
        {
            let control_summary = control_summary.trim();
            composed.push_str(&format!("- **Control summary:** {control_summary}\n"));
        }
        if !snapshot.bug.validation.repro_steps.is_empty() {
            let steps: Vec<&str> = snapshot
                .bug
                .validation
                .repro_steps
                .iter()
                .map(String::as_str)
                .filter(|step| {
                    let trimmed = step.trim_start();
                    trimmed.starts_with("Run:") || trimmed.starts_with("$ ")
                })
                .collect();
            let steps = if steps.is_empty() {
                snapshot
                    .bug
                    .validation
                    .repro_steps
                    .iter()
                    .map(String::as_str)
                    .collect()
            } else {
                steps
            };
            composed.push_str("- **Repro steps:**\n");
            for (i, step) in steps.iter().enumerate() {
                let n = i + 1;
                let step = step.trim();
                composed.push_str(&format!("  {n}. {step}\n"));
            }
        }
        if !snapshot.bug.validation.control_steps.is_empty() {
            let steps: Vec<&str> = snapshot
                .bug
                .validation
                .control_steps
                .iter()
                .map(String::as_str)
                .filter(|step| {
                    let trimmed = step.trim_start();
                    trimmed.starts_with("Run:") || trimmed.starts_with("$ ")
                })
                .collect();
            let steps = if steps.is_empty() {
                snapshot
                    .bug
                    .validation
                    .control_steps
                    .iter()
                    .map(String::as_str)
                    .collect()
            } else {
                steps
            };
            composed.push_str("- **Control steps:**\n");
            for (i, step) in steps.iter().enumerate() {
                let n = i + 1;
                let step = step.trim();
                composed.push_str(&format!("  {n}. {step}\n"));
            }
        }
        let artifacts: Vec<String> = snapshot
            .bug
            .validation
            .artifacts
            .iter()
            .filter_map(|a| {
                let trimmed = a.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(format!("`{trimmed}`"))
                }
            })
            .collect();
        if !artifacts.is_empty() {
            composed.push_str(&format!("- **Artifacts:** {}\n", artifacts.join(", ")));
        }
        let validation_output = build_validation_output_block(
            &snapshot.bug.validation,
            repo_root,
            output_root,
            expects_asan,
        );
        if let Some(output) = validation_output.as_deref() {
            composed.push_str("- **Validation Output:**\n```\n");
            composed.push_str(output.trim());
            composed.push_str("\n```\n");
        }
        if let Some(control_snippet) = snapshot
            .bug
            .validation
            .control_output_snippet
            .as_ref()
            .filter(|snippet| !snippet.is_empty())
        {
            composed.push_str("- **Control output:**\n```\n");
            composed.push_str(control_snippet.trim());
            composed.push_str("\n```\n");
        }
        let poc_artifact = first_validation_poc_artifact(&snapshot.bug.validation);
        if let Some(exploit) = build_exploit_scenario_block(
            &snapshot.bug,
            base_raw,
            validation_output.as_deref(),
            poc_artifact.as_deref(),
        ) {
            composed.push('\n');
            composed.push_str(exploit.trim());
            composed.push('\n');
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

#[allow(clippy::needless_collect, clippy::too_many_arguments)]
async fn polish_bug_markdowns(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    writing_model: &str,
    writing_reasoning_effort: Option<ReasoningEffort>,
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
        let writing_model = writing_model.to_string();
        let metrics = metrics.clone();
        async move {
            if content.trim().is_empty() {
                return Ok(BugPolishUpdate {
                    id: bug_id,
                    markdown: content,
                    logs: Vec::new(),
                });
            }
            let outcome = polish_markdown_block(
                client,
                provider,
                auth,
                &writing_model,
                writing_reasoning_effort,
                metrics,
                &content,
                None,
            )
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

fn snapshot_summaries_and_details(
    snapshot: &SecurityReviewSnapshot,
) -> (Vec<BugSummary>, Vec<BugDetail>) {
    let mut summaries: Vec<BugSummary> = Vec::with_capacity(snapshot.bugs.len());
    let mut details: Vec<BugDetail> = Vec::with_capacity(snapshot.bugs.len());

    for entry in &snapshot.bugs {
        let bug = entry.bug.clone();
        let markdown = entry.original_markdown.clone();
        let vulnerability_tag = bug.vulnerability_tag.clone().or_else(|| {
            extract_vulnerability_tag_from_bug_markdown(entry.original_markdown.as_str())
        });

        summaries.push(BugSummary {
            id: bug.summary_id,
            title: bug.title,
            file: bug.file,
            severity: bug.severity,
            impact: bug.impact,
            likelihood: bug.likelihood,
            recommendation: bug.recommendation,
            blame: bug.blame,
            risk_score: bug.risk_score,
            risk_rank: bug.risk_rank,
            risk_reason: bug.risk_reason,
            verification_types: bug.verification_types,
            vulnerability_tag,
            validation: bug.validation,
            source_path: PathBuf::new(),
            markdown: markdown.clone(),
            author_github: bug.assignee_github,
        });

        details.push(BugDetail {
            summary_id: bug.summary_id,
            original_markdown: markdown,
        });
    }

    (summaries, details)
}

fn extract_vulnerability_tag_from_bug_markdown(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .map(str::trim)
        .find_map(parse_taxonomy_vuln_tag)
}

fn count_files_with_findings_from_snapshots(snapshots: &[BugSnapshot]) -> usize {
    let mut files: HashSet<String> = HashSet::new();
    for snapshot in snapshots {
        for loc in extract_file_locations_for_dedupe(&snapshot.bug.file) {
            let Some((path, _)) = loc.split_once("#L") else {
                continue;
            };
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                files.insert(trimmed.to_string());
            }
        }
    }
    files.len()
}

#[cfg(test)]
fn dedupe_security_review_snapshot(snapshot: &mut SecurityReviewSnapshot) -> usize {
    let mut summaries: Vec<BugSummary> = Vec::with_capacity(snapshot.bugs.len());
    let mut details: Vec<BugDetail> = Vec::with_capacity(snapshot.bugs.len());

    for entry in snapshot.bugs.iter_mut() {
        if entry.bug.vulnerability_tag.is_none() {
            entry.bug.vulnerability_tag =
                extract_vulnerability_tag_from_bug_markdown(entry.original_markdown.as_str());
        }

        let bug = entry.bug.clone();
        let markdown = entry.original_markdown.clone();
        summaries.push(BugSummary {
            id: bug.summary_id,
            title: bug.title,
            file: bug.file,
            severity: bug.severity,
            impact: bug.impact,
            likelihood: bug.likelihood,
            recommendation: bug.recommendation,
            blame: bug.blame,
            risk_score: bug.risk_score,
            risk_rank: bug.risk_rank,
            risk_reason: bug.risk_reason,
            verification_types: bug.verification_types,
            vulnerability_tag: bug.vulnerability_tag,
            validation: bug.validation,
            source_path: PathBuf::new(),
            markdown: markdown.clone(),
            author_github: bug.assignee_github,
        });
        details.push(BugDetail {
            summary_id: bug.summary_id,
            original_markdown: markdown,
        });
    }

    let before = summaries.len();
    if before == 0 {
        return 0;
    }

    let (mut summaries, mut details, removed) = dedupe_bug_summaries(summaries, details);
    rank_bug_summaries_for_reporting(&mut summaries);
    normalize_bug_identifiers(&mut summaries, &mut details);
    let (_bugs, snapshots) = build_bug_records(summaries, details);

    snapshot.bugs = snapshots;
    snapshot.findings_summary = format_findings_summary(
        snapshot.bugs.len(),
        count_files_with_findings_from_snapshots(&snapshot.bugs),
    );

    removed
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

#[derive(Clone, Debug)]
struct GitRevisionInfo {
    commit: String,
    branch: Option<String>,
    commit_timestamp: Option<i64>,
    repository_url: Option<String>,
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
        "bench",
        "benches",
        "benchmark",
        "benchmarks",
        "ci",
        "cicd",
        "circleci",
        "github",
        "workflow",
        "workflows",
        "gitlab",
        "buildkite",
        "pipeline",
        "pipelines",
        "fuzz",
        "fuzzing",
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
    let matches_marker = |segment: &str, marker: &str| {
        if marker == "ci" {
            segment == "ci"
                || segment.starts_with("ci-")
                || segment.starts_with("ci_")
                || segment.starts_with("ci.")
                || segment.ends_with("-ci")
                || segment.ends_with("_ci")
                || segment.ends_with(".ci")
        } else {
            segment.contains(marker)
        }
    };
    components.iter().any(|segment| {
        skip_markers
            .iter()
            .any(|marker| matches_marker(segment, marker))
    })
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
                "Skipping specification for {display} (looks like tests/CI/fuzzing/tooling)."
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
        tool_timeout_sec: None,
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
        tool_timeout_sec: None,
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
        tool_timeout_sec: None,
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
        tool_timeout_sec: None,
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
                if existing.tool_timeout_sec.is_some() {
                    existing.tool_timeout_sec = None;
                    logs.push(format!("Removed MCP tool timeout for `{name}`."));
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

fn security_review_session_id() -> &'static str {
    static SECURITY_REVIEW_SESSION_ID: OnceLock<String> = OnceLock::new();
    SECURITY_REVIEW_SESSION_ID.get_or_init(|| {
        let rand: u64 = rand::random();
        let now = OffsetDateTime::now_utc().unix_timestamp_nanos();
        format!("secreview-{now:x}-{rand:x}")
    })
}

fn provider_with_beta_features(provider: &ModelProviderInfo, config: &Config) -> ModelProviderInfo {
    let enabled = codex_core::features::FEATURES
        .iter()
        .filter_map(|spec| {
            if spec.stage.beta_menu_description().is_some() && config.features.enabled(spec.id) {
                Some(spec.key)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(",");

    if enabled.is_empty() {
        return provider.clone();
    }

    let mut provider = provider.clone();
    provider
        .http_headers
        .get_or_insert_with(HashMap::new)
        .insert("x-codex-beta-features".to_string(), enabled);
    provider
}

pub async fn run_security_review(
    mut request: SecurityReviewRequest,
) -> Result<SecurityReviewResult, SecurityReviewFailure> {
    request.provider = provider_with_beta_features(&request.provider, &request.config);

    request.validation_target_url = request
        .validation_target_url
        .take()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    request.model = request.model.trim().to_string();
    if request.model.is_empty() {
        request.model = BUG_MODEL.to_string();
    }

    request.triage_model = request.triage_model.trim().to_string();
    if request.triage_model.is_empty() {
        request.triage_model = FILE_TRIAGE_MODEL.to_string();
    }

    request.spec_model = request.spec_model.trim().to_string();
    if request.spec_model.is_empty() {
        request.spec_model = SPEC_GENERATION_MODEL.to_string();
    }

    request.validation_model = request.validation_model.trim().to_string();
    if request.validation_model.is_empty() {
        request.validation_model = DEFAULT_VALIDATION_MODEL.to_string();
    }

    request.writing_model = request.writing_model.trim().to_string();
    if request.writing_model.is_empty() {
        request.writing_model = MARKDOWN_FIX_MODEL.to_string();
    }

    let threat_model_model = request
        .config
        .security_review_models
        .threat_model
        .clone()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| THREAT_MODEL_MODEL.to_string());

    let global_reasoning_effort = request.config.model_reasoning_effort;
    let triage_reasoning_effort = request
        .config
        .security_review_reasoning_efforts
        .file_triage
        .or(global_reasoning_effort)
        .or(Some(ReasoningEffort::Medium));
    let spec_reasoning_effort = request
        .config
        .security_review_reasoning_efforts
        .spec
        .or(global_reasoning_effort)
        .or(Some(ReasoningEffort::High));
    let threat_model_reasoning_effort = request
        .config
        .security_review_reasoning_efforts
        .threat_model
        .or(spec_reasoning_effort);
    let bug_reasoning_effort = request
        .config
        .security_review_reasoning_efforts
        .bugs
        .or(global_reasoning_effort)
        .or(Some(ReasoningEffort::XHigh));
    let validation_reasoning_effort = request
        .config
        .security_review_reasoning_efforts
        .validation
        .or(global_reasoning_effort)
        .or(Some(ReasoningEffort::XHigh));
    let writing_reasoning_effort = request
        .config
        .security_review_reasoning_efforts
        .writing
        .or(global_reasoning_effort)
        .or(Some(ReasoningEffort::High));

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
                        write_log_sink(&log_sink_for_task, message.as_str());
                        if log_sink_for_task.is_none() {
                            tracing::info!("{message}");
                        }
                    }
                    AppEvent::InsertHistoryCell(cell) => {
                        for line in cell
                            .transcript_lines(u16::MAX)
                            .into_iter()
                            .map(|line| {
                                line.spans
                                    .iter()
                                    .map(|span| span.content.as_ref())
                                    .collect::<String>()
                            })
                            .filter(|line| !line.trim().is_empty())
                        {
                            write_log_sink(&log_sink_for_task, line.as_str());
                            if log_sink_for_task.is_none() {
                                tracing::info!("{line}");
                            }
                        }
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
                        let command_message = format!("Command [{state_label}]: {summary}");
                        write_log_sink(&log_sink_for_task, command_message.as_str());
                        if log_sink_for_task.is_none() {
                            tracing::info!("{command_message}");
                        }
                        for line in preview {
                            if !line.trim().is_empty() {
                                write_log_sink(&log_sink_for_task, line.as_str());
                                if log_sink_for_task.is_none() {
                                    tracing::info!("{line}");
                                }
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
    let skip_validation = git_revision
        .as_ref()
        .and_then(|revision| revision.repository_url.as_deref())
        .is_some_and(is_openai_openai_repo);
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
        triaged_dirs: None,
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

    let mut plan_step_models: HashMap<SecurityReviewPlanStep, PlanStepModelInfo> = HashMap::new();
    if matches!(mode, SecurityReviewMode::Full) {
        plan_step_models.insert(
            SecurityReviewPlanStep::GenerateSpecs,
            PlanStepModelInfo {
                model: request.spec_model.clone(),
                reasoning_effort: spec_reasoning_effort,
            },
        );
        plan_step_models.insert(
            SecurityReviewPlanStep::ThreatModel,
            PlanStepModelInfo {
                model: threat_model_model.clone(),
                reasoning_effort: threat_model_reasoning_effort,
            },
        );
    }
    plan_step_models.insert(
        SecurityReviewPlanStep::DirTriage,
        PlanStepModelInfo {
            model: request.triage_model.clone(),
            reasoning_effort: triage_reasoning_effort,
        },
    );
    plan_step_models.insert(
        SecurityReviewPlanStep::FileTriage,
        PlanStepModelInfo {
            model: request.triage_model.clone(),
            reasoning_effort: triage_reasoning_effort,
        },
    );
    plan_step_models.insert(
        SecurityReviewPlanStep::AnalyzeBugs,
        PlanStepModelInfo {
            model: request.model.clone(),
            reasoning_effort: bug_reasoning_effort,
        },
    );
    plan_step_models.insert(
        SecurityReviewPlanStep::PolishFindings,
        PlanStepModelInfo {
            model: request.model.clone(),
            reasoning_effort: bug_reasoning_effort,
        },
    );
    plan_step_models.insert(
        SecurityReviewPlanStep::PrepareValidationTargets,
        PlanStepModelInfo {
            model: request.validation_model.clone(),
            reasoning_effort: validation_reasoning_effort,
        },
    );
    plan_step_models.insert(
        SecurityReviewPlanStep::ValidateFindings,
        PlanStepModelInfo {
            model: request.validation_model.clone(),
            reasoning_effort: validation_reasoning_effort,
        },
    );
    plan_step_models.insert(
        SecurityReviewPlanStep::PostValidationRefine,
        PlanStepModelInfo {
            model: request.validation_model.clone(),
            reasoning_effort: validation_reasoning_effort,
        },
    );
    plan_step_models.insert(
        SecurityReviewPlanStep::AssembleReport,
        PlanStepModelInfo {
            model: request.writing_model.clone(),
            reasoning_effort: writing_reasoning_effort,
        },
    );

    let mut plan_tracker = SecurityReviewPlanTracker::new(
        mode,
        &include_paths,
        &repo_path,
        plan_step_models,
        progress_sender.clone(),
        log_sink.clone(),
    );
    plan_tracker.restore_statuses(&checkpoint.plan_statuses);
    if checkpoint.selected_snippets.is_some()
        && !matches!(
            plan_tracker.status_for(SecurityReviewPlanStep::FileTriage),
            Some(StepStatus::Completed)
        )
    {
        plan_tracker.mark_complete(SecurityReviewPlanStep::DirTriage);
        plan_tracker.mark_complete(SecurityReviewPlanStep::FileTriage);
    }
    let validation_prep_complete = reconcile_validation_target_prep_resume_state(
        &request.output_root,
        &repo_path,
        &mut plan_tracker,
        &progress_sender,
        &log_sink,
        &mut logs,
    );
    if !validation_prep_complete
        && matches!(
            plan_tracker.status_for(SecurityReviewPlanStep::ValidateFindings),
            Some(StepStatus::InProgress)
        )
    {
        push_progress_log(
            &progress_sender,
            &log_sink,
            &mut logs,
            "Validate findings: waiting for validation target preparation; resetting status to pending."
                .to_string(),
        );
        plan_tracker.reset_step(SecurityReviewPlanStep::ValidateFindings);
    }

    if skip_validation {
        record(
            &mut logs,
            "Skipping validation for openai/openai: skipping validation target preparation, per-finding validation, and post-validation refinement."
                .to_string(),
        );
        plan_tracker.mark_steps_complete(&[
            SecurityReviewPlanStep::PrepareValidationTargets,
            SecurityReviewPlanStep::ValidateFindings,
            SecurityReviewPlanStep::PostValidationRefine,
        ]);
    }
    checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
    persist_checkpoint(&mut checkpoint, &mut logs);
    plan_tracker.emit_plan_snapshot();
    plan_tracker.enable_snapshots();

    let mut validation_target_prep_task: Option<
        tokio::task::JoinHandle<ValidationTargetPrepOutcome>,
    > = None;

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

        let triage_model = request.triage_model.as_str();

        // First prune at the directory level to keep triage manageable (and to avoid build/test
        // artifacts).
        let dir_key_for = |path: &Path| -> PathBuf {
            let Some(parent) = path.parent() else {
                return PathBuf::from(".");
            };
            if parent == Path::new("") || parent == Path::new(".") {
                return PathBuf::from(".");
            }
            let mut components = parent.components();
            match components.next() {
                Some(std::path::Component::Normal(part)) => PathBuf::from(part),
                _ => PathBuf::from("."),
            }
        };

        let mut directories: HashMap<PathBuf, Vec<FileSnippet>> = HashMap::new();
        let mut excluded_files = 0usize;
        for snippet in collection.snippets {
            if path_has_excluded_dir_component(&snippet.relative_path) {
                excluded_files = excluded_files.saturating_add(1);
                continue;
            }
            let dir_key = dir_key_for(&snippet.relative_path);
            directories.entry(dir_key).or_default().push(snippet);
        }

        let mut ranked_dirs: Vec<(PathBuf, Vec<FileSnippet>, usize)> = directories
            .into_iter()
            .map(|(dir, snippets)| {
                let bytes = snippets.iter().map(|s| s.bytes).sum::<usize>();
                (dir, snippets, bytes)
            })
            .collect();
        ranked_dirs.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));

        let mut refined_dirs: Vec<PathBuf> = Vec::new();
        if matches!(
            plan_tracker.status_for(SecurityReviewPlanStep::DirTriage),
            Some(StepStatus::Completed)
        ) && let Some(saved) = checkpoint.triaged_dirs.as_ref()
        {
            refined_dirs = saved
                .iter()
                .map(PathBuf::from)
                .filter(|dir| !path_has_excluded_dir_component(dir))
                .collect();
            if !refined_dirs.is_empty() {
                record(
                    &mut logs,
                    format!(
                        "Using {} triaged directory(ies) from checkpoint resume.",
                        refined_dirs.len()
                    ),
                );
            }
        }
        if refined_dirs.is_empty()
            && checkpoint.triaged_dirs.is_none()
            && let Some(spec) = checkpoint.spec.as_ref()
        {
            let mut seeded_dirs: Vec<PathBuf> = spec
                .locations
                .iter()
                .filter_map(|loc| {
                    let trimmed = loc.trim();
                    if trimmed.is_empty() {
                        return None;
                    }
                    if trimmed == "." {
                        return Some(PathBuf::from("."));
                    }
                    let mut path = PathBuf::from(trimmed);
                    if path.is_absolute() {
                        path = path.strip_prefix(&repo_path).ok()?.to_path_buf();
                    }
                    let mut components = path.components();
                    match components.next() {
                        Some(std::path::Component::Normal(part)) => Some(PathBuf::from(part)),
                        _ => Some(PathBuf::from(".")),
                    }
                })
                .filter(|dir| !path_has_excluded_dir_component(dir))
                .collect();
            seeded_dirs.sort();
            seeded_dirs.dedup();
            if !seeded_dirs.is_empty() {
                let count = seeded_dirs.len();
                refined_dirs = seeded_dirs;
                record(
                    &mut logs,
                    format!("Seeding directory triage with {count} spec location(s)."),
                );
                checkpoint.triaged_dirs = Some(
                    refined_dirs
                        .iter()
                        .map(|dir| dir.to_string_lossy().to_string())
                        .collect(),
                );
                plan_tracker.mark_complete(SecurityReviewPlanStep::DirTriage);
                checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
                persist_checkpoint(&mut checkpoint, &mut logs);
            }
        }

        if refined_dirs.is_empty() {
            if !matches!(
                plan_tracker.status_for(SecurityReviewPlanStep::DirTriage),
                Some(StepStatus::Completed | StepStatus::InProgress)
            ) {
                plan_tracker.start_step(SecurityReviewPlanStep::DirTriage);
                checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
                persist_checkpoint(&mut checkpoint, &mut logs);
            }
            let dir_candidates: Vec<(PathBuf, String)> = ranked_dirs
                .iter()
                .map(|(dir, _, _)| {
                    (
                        dir.clone(),
                        display_path_for(&repo_path.join(dir), &repo_path),
                    )
                })
                .collect();

            if dir_candidates.len() <= 1 {
                refined_dirs = dir_candidates
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect();
            } else {
                record(
                    &mut logs,
                    "Selecting high-signal directories for file triage...".to_string(),
                );
                match filter_spec_directories(
                    &model_client,
                    &request.provider,
                    &request.auth,
                    triage_model,
                    triage_reasoning_effort,
                    &repo_path,
                    &dir_candidates,
                    metrics.clone(),
                )
                .await
                {
                    Ok(selected) => {
                        refined_dirs = selected.iter().map(|(path, _)| path.clone()).collect();
                        if refined_dirs.len() < dir_candidates.len() {
                            record(
                                &mut logs,
                                format!(
                                    "Directory triage kept {}/{} directories for file triage.",
                                    refined_dirs.len(),
                                    dir_candidates.len()
                                ),
                            );
                        }
                    }
                    Err(err) => {
                        for line in &err.logs {
                            record(&mut logs, line.clone());
                        }
                        let total = dir_candidates.len();
                        let kept = total.min(SPEC_DIR_FILTER_TARGET.max(1));
                        record(
                            &mut logs,
                            format!(
                                "Directory triage failed; using top {kept}/{total} directories by size. {}",
                                err.message
                            ),
                        );
                        refined_dirs = dir_candidates
                            .iter()
                            .take(kept)
                            .map(|(path, _)| path.clone())
                            .collect();
                    }
                }
            }
            if !refined_dirs.iter().any(|dir| dir == Path::new("."))
                && dir_candidates.iter().any(|(dir, _)| dir == Path::new("."))
            {
                refined_dirs.push(PathBuf::from("."));
            }
            checkpoint.triaged_dirs = Some(
                refined_dirs
                    .iter()
                    .map(|dir| dir.to_string_lossy().to_string())
                    .collect(),
            );
            persist_checkpoint(&mut checkpoint, &mut logs);

            plan_tracker.complete_and_start_next(
                SecurityReviewPlanStep::DirTriage,
                Some(SecurityReviewPlanStep::FileTriage),
            );
            checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
            persist_checkpoint(&mut checkpoint, &mut logs);
        }

        if matches!(
            plan_tracker.status_for(SecurityReviewPlanStep::FileTriage),
            Some(StepStatus::Completed)
        ) {
            record(
                &mut logs,
                "File triage marked complete in checkpoint, but no triaged files were stored; re-running file triage."
                    .to_string(),
            );
            plan_tracker.start_step(SecurityReviewPlanStep::FileTriage);
            checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
            persist_checkpoint(&mut checkpoint, &mut logs);
        } else if !matches!(
            plan_tracker.status_for(SecurityReviewPlanStep::FileTriage),
            Some(StepStatus::Completed | StepStatus::InProgress)
        ) {
            plan_tracker.start_step(SecurityReviewPlanStep::FileTriage);
            checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
            persist_checkpoint(&mut checkpoint, &mut logs);
        }

        let refined_set: HashSet<PathBuf> = refined_dirs.into_iter().collect();
        let mut pruned_snippets: Vec<FileSnippet> = Vec::new();
        let mut refined_dir_count = 0usize;
        let mut refined_bytes = 0usize;
        let mut logged_dirs = 0usize;
        for (dir, snippets, bytes) in ranked_dirs {
            if !refined_set.contains(&dir) {
                continue;
            }
            refined_dir_count = refined_dir_count.saturating_add(1);
            refined_bytes = refined_bytes.saturating_add(bytes);
            if logged_dirs < DIR_TRIAGE_LOG_LIMIT {
                record(
                    &mut logs,
                    format!(
                        "Triaged directory {} ({} files, {}).",
                        display_path_for(&repo_path.join(&dir), &repo_path),
                        snippets.len(),
                        human_readable_bytes(bytes),
                    ),
                );
                logged_dirs = logged_dirs.saturating_add(1);
            }
            pruned_snippets.extend(snippets);
        }

        if excluded_files > 0 {
            record(
                &mut logs,
                format!(
                    "Directory triage excluded {excluded_files} file(s) from build/test artifacts."
                ),
            );
        }
        if refined_dir_count > logged_dirs {
            record(
                &mut logs,
                format!(
                    "…and {} more triaged director{}.",
                    refined_dir_count.saturating_sub(logged_dirs),
                    if refined_dir_count.saturating_sub(logged_dirs) == 1 {
                        "y"
                    } else {
                        "ies"
                    }
                ),
            );
        }
        if pruned_snippets.is_empty() {
            return Err(SecurityReviewFailure {
                message: "No candidate files remain after directory triage.".to_string(),
                logs,
            });
        }
        let file_count = pruned_snippets.len();
        record(
            &mut logs,
            format!(
                "Running LLM file triage (path + first {FILE_TRIAGE_PREVIEW_CHARS} chars); analyzing {file_count} files across {refined_dir_count} directories.",
            ),
        );
        let triage = match triage_files_for_bug_analysis(
            &model_client,
            &request.provider,
            &request.auth,
            triage_model,
            triage_reasoning_effort,
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
        if let Some(snippets) = selected_snippets.as_mut() {
            let removed = dedupe_file_snippets(snippets);
            if removed > 0 {
                record(
                    &mut logs,
                    format!("Removed {removed} duplicate file(s) from triaged selection."),
                );
            }
        }
        checkpoint.selected_snippets = selected_snippets.clone();
        persist_checkpoint(&mut checkpoint, &mut logs);

        plan_tracker.complete_and_start_next(
            SecurityReviewPlanStep::FileTriage,
            Some(SecurityReviewPlanStep::AnalyzeBugs),
        );
        checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
        persist_checkpoint(&mut checkpoint, &mut logs);
    } else {
        let removed = selected_snippets
            .as_mut()
            .map(dedupe_file_snippets)
            .unwrap_or(0);
        if removed > 0 {
            record(
                &mut logs,
                format!("Removed {removed} duplicate file(s) from checkpoint selection."),
            );
            checkpoint.selected_snippets = selected_snippets.clone();
            persist_checkpoint(&mut checkpoint, &mut logs);
        }
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
    if !matches!(
        plan_tracker.status_for(SecurityReviewPlanStep::PrepareValidationTargets),
        Some(StepStatus::Completed | StepStatus::InProgress)
    ) {
        plan_tracker.start_step(SecurityReviewPlanStep::PrepareValidationTargets);
        if let Some(linear_issue) = linear_issue.as_ref() {
            let prompt = build_linear_progress_prompt(
                linear_issue,
                &checkpoint.model,
                "Prepare runnable validation targets",
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
        plan_tracker.status_for(SecurityReviewPlanStep::PrepareValidationTargets),
        Some(StepStatus::Completed)
    ) && validation_target_prep_task.is_none()
    {
        let config = request.config.clone();
        let provider = request.provider.clone();
        let auth_manager = request.auth_manager.clone();
        let model = request.validation_model.clone();
        let reasoning_effort = validation_reasoning_effort;
        let repo_for_task = repo_path.clone();
        let output_root_for_task = request.output_root.clone();
        let progress_for_task = progress_sender.clone();
        let log_sink_for_task = log_sink.clone();
        let metrics_for_task = metrics.clone();
        validation_target_prep_task = Some(tokio::spawn(async move {
            prepare_validation_targets(
                &config,
                &provider,
                auth_manager,
                model.as_str(),
                reasoning_effort,
                &repo_for_task,
                &output_root_for_task,
                progress_for_task,
                log_sink_for_task,
                metrics_for_task,
            )
            .await
        }));
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

    let spec_targets_from_files = || -> Vec<PathBuf> {
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

    let mut spec_targets: Vec<PathBuf> = if !include_paths.is_empty() {
        include_paths.clone()
    } else if let Some(triaged_dirs) = checkpoint.triaged_dirs.as_ref()
        && !triaged_dirs.is_empty()
    {
        let mut targets: Vec<PathBuf> = Vec::new();
        for dir in triaged_dirs {
            let path = PathBuf::from(dir);
            if path_has_excluded_dir_component(&path) {
                continue;
            }
            let abs = if path.is_absolute() {
                path
            } else {
                repo_path.join(&path)
            };
            if abs.exists() {
                targets.push(abs);
            }
        }
        targets.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
        targets.dedup();
        if targets.is_empty() {
            spec_targets_from_files()
        } else {
            record(
                &mut logs,
                format!(
                    "Reusing {}/{} triaged director{} for specification generation.",
                    targets.len(),
                    triaged_dirs.len(),
                    if targets.len() == 1 { "y" } else { "ies" }
                ),
            );
            targets
        }
    } else {
        spec_targets_from_files()
    };

    spec_targets.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

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
                "Generating system specifications for {} scope path(s) (running in parallel with bug analysis) (spec model: {spec_model}, reasoning: {spec_reasoning_label}; writing model: {writing_model}, reasoning: {writing_reasoning_label}).",
                spec_targets.len(),
                spec_model = request.spec_model.as_str(),
                spec_reasoning_label =
                    reasoning_effort_label(normalize_reasoning_effort_for_model(
                        request.spec_model.as_str(),
                        spec_reasoning_effort,
                    )),
                writing_model = request.writing_model.as_str(),
                writing_reasoning_label =
                    reasoning_effort_label(normalize_reasoning_effort_for_model(
                        request.writing_model.as_str(),
                        writing_reasoning_effort,
                    ))
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
        let spec_model = request.spec_model.clone();
        let writing_model = request.writing_model.clone();
        let threat_model_model = threat_model_model.clone();
        Some(tokio::spawn(async move {
            let spec_generation = match generate_specs(
                &model_client,
                &provider,
                &auth,
                spec_model.as_str(),
                spec_reasoning_effort,
                writing_model.as_str(),
                writing_reasoning_effort,
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
                    threat_model_model.as_str(),
                    threat_model_reasoning_effort,
                    writing_model.as_str(),
                    writing_reasoning_effort,
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
    let mut snapshot: Option<SecurityReviewSnapshot> = None;
    let mut bugs_markdown: String = String::new();
    let mut findings_summary: String = String::new();

    let mut skip_bug_analysis = false;
    let reuse_bug_snapshot = matches!(
        plan_tracker.status_for(SecurityReviewPlanStep::PolishFindings),
        Some(StepStatus::Completed)
    );
    if resuming
        && reuse_bug_snapshot
        && let Some(path) = checkpoint.bug_snapshot_path.as_ref()
        && path.exists()
    {
        match tokio_fs::read(path).await {
            Ok(bytes) => match serde_json::from_slice::<SecurityReviewSnapshot>(&bytes) {
                Ok(mut loaded) => {
                    record(
                        &mut logs,
                        format!(
                            "Loaded prior bug snapshot from {}; skipping bug re-analysis.",
                            path.display()
                        ),
                    );

                    let polish_completed = matches!(
                        plan_tracker.status_for(SecurityReviewPlanStep::PolishFindings),
                        Some(StepStatus::Completed)
                    );
                    let mut removed = 0usize;
                    let mut filtered_low = 0usize;
                    if loaded.bugs.len() > 1 && !polish_completed {
                        let dedupe_model = if request.spec_model.trim().is_empty() {
                            SPEC_GENERATION_MODEL
                        } else {
                            request.spec_model.as_str()
                        };
                        let (summaries, details) = snapshot_summaries_and_details(&loaded);
                        let before_dedupe_count = summaries.len();
                        let llm_outcome = llm_dedupe_bug_summaries(
                            &model_client,
                            &request.provider,
                            &request.auth,
                            dedupe_model,
                            spec_reasoning_effort,
                            summaries,
                            details,
                            path.parent().map(Path::to_path_buf),
                            progress_sender.clone(),
                            log_sink.clone(),
                            metrics.clone(),
                        )
                        .await;
                        for line in llm_outcome.logs {
                            record(&mut logs, line);
                        }
                        record(
                            &mut logs,
                            format!(
                                "LLM dedupe findings: {before_dedupe_count} -> {}.",
                                llm_outcome.summaries.len()
                            ),
                        );
                        if llm_outcome.removed > 0 {
                            let llm_removed = llm_outcome.removed;
                            record(
                                &mut logs,
                                format!(
                                    "Deduplicated {llm_removed} additional finding{} via LLM while resuming from snapshot.",
                                    if llm_removed == 1 { "" } else { "s" }
                                ),
                            );
                            removed = removed.saturating_add(llm_removed);
                        }
                        if llm_outcome.filtered_low > 0 {
                            filtered_low = filtered_low.saturating_add(llm_outcome.filtered_low);
                        }
                        if llm_outcome.removed > 0 || llm_outcome.filtered_low > 0 {
                            let mut summaries = llm_outcome.summaries;
                            let mut details = llm_outcome.details;
                            rank_bug_summaries_for_reporting(&mut summaries);
                            normalize_bug_identifiers(&mut summaries, &mut details);
                            let (_bugs, snapshots) = build_bug_records(summaries, details);
                            loaded.bugs = snapshots;
                            loaded.findings_summary = format_findings_summary(
                                loaded.bugs.len(),
                                count_files_with_findings_from_snapshots(&loaded.bugs),
                            );
                        }
                    } else if loaded.bugs.len() > 1 && polish_completed {
                        record(
                            &mut logs,
                            "Polish findings already completed; skipping resume dedupe."
                                .to_string(),
                        );
                    }
                    if removed > 0 {
                        record(
                            &mut logs,
                            format!(
                                "Deduplicated {removed} finding{} while resuming from snapshot.",
                                if removed == 1 { "" } else { "s" }
                            ),
                        );
                    }
                    if filtered_low > 0 {
                        record(
                            &mut logs,
                            format!(
                                "Dropped {filtered_low} low severity finding{} while resuming from snapshot.",
                                if filtered_low == 1 { "" } else { "s" }
                            ),
                        );
                    }
                    if (removed > 0 || filtered_low > 0)
                        && let Ok(updated) = serde_json::to_vec_pretty(&loaded)
                        && let Err(err) = tokio_fs::write(path, updated).await
                    {
                        record(
                            &mut logs,
                            format!(
                                "Failed to persist updated snapshot {}: {err}",
                                path.display()
                            ),
                        );
                    }

                    plan_tracker.mark_complete(SecurityReviewPlanStep::AnalyzeBugs);
                    plan_tracker.mark_complete(SecurityReviewPlanStep::PolishFindings);
                    for step in [
                        SecurityReviewPlanStep::PrepareValidationTargets,
                        SecurityReviewPlanStep::ValidateFindings,
                        SecurityReviewPlanStep::PostValidationRefine,
                        SecurityReviewPlanStep::AssembleReport,
                    ] {
                        if matches!(
                            plan_tracker.status_for(step),
                            Some(StepStatus::Completed | StepStatus::InProgress)
                        ) {
                            continue;
                        }
                        plan_tracker.start_step(step);
                        break;
                    }
                    checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
                    persist_checkpoint(&mut checkpoint, &mut logs);

                    findings_summary = loaded.findings_summary.clone();
                    bugs_for_result = snapshot_bugs(&loaded);
                    bug_summary_table = make_bug_summary_table_from_bugs(&bugs_for_result);
                    bugs_markdown = build_bugs_markdown(
                        &loaded,
                        git_link_info.as_ref(),
                        Some(repo_path.as_path()),
                        Some(request.output_root.as_path()),
                    );
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
        if !matches!(
            plan_tracker.status_for(SecurityReviewPlanStep::AnalyzeBugs),
            Some(StepStatus::Completed | StepStatus::InProgress)
        ) {
            plan_tracker.start_step(SecurityReviewPlanStep::AnalyzeBugs);
            checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
            persist_checkpoint(&mut checkpoint, &mut logs);
        }

        // Run bug analysis in N full passes across all selected files.
        let total_passes = BUG_FINDING_PASSES.max(1);

        let mut aggregated_logs: Vec<String> = Vec::new();
        let mut all_summaries: Vec<BugSummary> = Vec::new();
        let mut all_details: Vec<BugDetail> = Vec::new();
        use std::collections::HashMap as StdHashMap;
        let mut files_map: StdHashMap<PathBuf, FileSnippet> = StdHashMap::new();

        let mut loaded_from_progress = false;
        if resuming
            && matches!(
                plan_tracker.status_for(SecurityReviewPlanStep::AnalyzeBugs),
                Some(StepStatus::Completed)
            )
        {
            match load_bug_summaries_from_bug_analysis_progress(
                &request.output_root,
                &request.repo_path,
                &selected_snippets,
            )
            .await
            {
                Ok(Some(outcome)) => {
                    loaded_from_progress = true;
                    all_summaries = outcome.bug_summaries;
                    all_details = outcome.bug_details;
                    for line in outcome.logs {
                        record(&mut logs, line.clone());
                        aggregated_logs.push(line);
                    }
                    for snippet in outcome.files_with_findings {
                        files_map
                            .entry(snippet.relative_path.clone())
                            .or_insert(snippet);
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    record(&mut logs, err.message.clone());
                    aggregated_logs.extend(err.logs);
                }
            }
        }

        if !loaded_from_progress {
            record(
                &mut logs,
                format!(
                    "Running bug analysis in {total_passes} pass(es) (model: {model}, reasoning: {bug_reasoning_label}).",
                    model = request.model.as_str(),
                    bug_reasoning_label =
                        reasoning_effort_label(normalize_reasoning_effort_for_model(
                            request.model.as_str(),
                            bug_reasoning_effort
                        ))
                ),
            );

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

                let mut pass_outcome = match analyze_files_individually(
                    &model_client,
                    &request.provider,
                    &request.auth,
                    &request.model,
                    bug_reasoning_effort,
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

                aggregated_logs.extend(std::mem::take(&mut pass_outcome.logs));

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
                    bug_reasoning_effort,
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
        for summary in all_summaries.iter_mut() {
            if let Some(update) = apply_severity_matrix(summary) {
                let id = summary.id;
                let previous = update.previous;
                let computed = summary.severity.as_str();
                let impact = update.impact.label();
                let likelihood = update.likelihood.label();
                let product = update.product;
                let message = format!(
                    "Severity matrix: bug #{id} updated from {previous} to {computed} (impact {impact} * likelihood {likelihood} = {product})."
                );
                record(&mut logs, message.clone());
                aggregated_logs.push(message);
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

        if all_summaries.len() > 1 {
            let before_dedupe_count = all_summaries.len();
            let dedupe_model = if request.spec_model.trim().is_empty() {
                SPEC_GENERATION_MODEL
            } else {
                request.spec_model.as_str()
            };
            let llm_outcome = llm_dedupe_bug_summaries(
                &model_client,
                &request.provider,
                &request.auth,
                dedupe_model,
                spec_reasoning_effort,
                all_summaries,
                all_details,
                Some(request.output_root.join("context")),
                progress_sender.clone(),
                log_sink.clone(),
                metrics.clone(),
            )
            .await;

            all_summaries = llm_outcome.summaries;
            all_details = llm_outcome.details;

            for line in llm_outcome.logs {
                record(&mut logs, line.clone());
                aggregated_logs.push(line);
            }
            let dedupe_count_line = format!(
                "LLM dedupe findings: {before_dedupe_count} -> {}.",
                all_summaries.len()
            );
            record(&mut logs, dedupe_count_line.clone());
            aggregated_logs.push(dedupe_count_line);
            if llm_outcome.removed > 0 {
                let removed = llm_outcome.removed;
                let msg = format!(
                    "Deduplicated {removed} additional finding{} via LLM clustering.",
                    if removed == 1 { "" } else { "s" }
                );
                record(&mut logs, msg.clone());
                aggregated_logs.push(msg);
            }
            if llm_outcome.filtered_low > 0 {
                let filtered_low = llm_outcome.filtered_low;
                let msg = format!(
                    "Dropped {filtered_low} low severity finding{} during LLM dedupe.",
                    if filtered_low == 1 { "" } else { "s" }
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
                bug_reasoning_effort,
                &mut all_summaries,
                &request.repo_path,
                &repository_summary,
                spec_for_rerank,
                progress_sender.clone(),
                log_sink.clone(),
                metrics.clone(),
            )
            .await;
            logs.extend(risk_logs.clone());
            aggregated_logs.extend(risk_logs);
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
            // Final filter in case rerank reduces a finding to informational/ignore
            let before = all_summaries.len() as i64;
            let mut filtered_informational = 0_i64;
            let mut filtered_ignore = 0_i64;
            let mut filtered_other = 0_i64;
            let mut retained: HashSet<usize> = HashSet::new();
            all_summaries.retain(|summary| {
                let normalized = summary.severity.trim().to_ascii_lowercase();
                let keep = matches!(normalized.as_str(), "high" | "medium" | "low");
                if keep {
                    retained.insert(summary.id);
                } else if matches!(normalized.as_str(), "informational" | "info") {
                    filtered_informational = filtered_informational.saturating_add(1);
                } else if matches!(normalized.as_str(), "ignore" | "ignored") {
                    filtered_ignore = filtered_ignore.saturating_add(1);
                } else {
                    filtered_other = filtered_other.saturating_add(1);
                }
                keep
            });
            all_details.retain(|detail| retained.contains(&detail.summary_id));
            let after = all_summaries.len() as i64;
            let filtered_total = before.saturating_sub(after);
            if filtered_total > 0 {
                let mut filtered_labels: Vec<String> = Vec::new();
                if filtered_informational > 0 {
                    filtered_labels.push(format!("{filtered_informational} informational"));
                }
                if filtered_ignore > 0 {
                    filtered_labels.push(format!("{filtered_ignore} ignored"));
                }
                if filtered_other > 0 {
                    filtered_labels.push(format!("{filtered_other} other"));
                }
                let filtered = if filtered_labels.is_empty() {
                    filtered_total.to_string()
                } else {
                    filtered_labels.join(", ")
                };
                let msg = format!("Filtered out {filtered} finding(s) after rerank.");
                record(&mut logs, msg.clone());
                aggregated_logs.push(msg);
            }

            normalize_bug_identifiers(&mut all_summaries, &mut all_details);
        }

        if !all_summaries.is_empty() {
            let polish_message = format!(
                "Polishing markdown for {} bug finding(s) (model: {model}, reasoning: {writing_reasoning_label}).",
                all_summaries.len(),
                model = request.writing_model.as_str(),
                writing_reasoning_label =
                    reasoning_effort_label(normalize_reasoning_effort_for_model(
                        request.writing_model.as_str(),
                        writing_reasoning_effort,
                    ))
            );
            record(&mut logs, polish_message.clone());
            aggregated_logs.push(polish_message);
            let polish_logs = match polish_bug_markdowns(
                &model_client,
                &request.provider,
                &request.auth,
                request.writing_model.as_str(),
                writing_reasoning_effort,
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
        let next_step = if matches!(
            plan_tracker.status_for(SecurityReviewPlanStep::PrepareValidationTargets),
            Some(StepStatus::Completed)
        ) {
            SecurityReviewPlanStep::ValidateFindings
        } else {
            SecurityReviewPlanStep::PrepareValidationTargets
        };
        plan_tracker
            .complete_and_start_next(SecurityReviewPlanStep::PolishFindings, Some(next_step));
        checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
        persist_checkpoint(&mut checkpoint, &mut logs);
        if let Some(linear_issue) = linear_issue.as_ref() {
            let step_title = match next_step {
                SecurityReviewPlanStep::PrepareValidationTargets => {
                    "Prepare runnable validation targets"
                }
                SecurityReviewPlanStep::ValidateFindings => "Validate findings",
                _ => "Security review update",
            };
            let prompt = build_linear_progress_prompt(
                linear_issue,
                &checkpoint.model,
                step_title,
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
        let mut report_sections_prefix = Vec::new();
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
            report_sections_prefix,
            bugs: bug_snapshots,
        };

        bugs_markdown = build_bugs_markdown(
            &built_snapshot,
            git_link_info.as_ref(),
            Some(repo_path.as_path()),
            Some(request.output_root.as_path()),
        );
        snapshot = Some(built_snapshot);
    }

    let mut snapshot = match snapshot {
        Some(snapshot) => snapshot,
        None => {
            return Err(SecurityReviewFailure {
                message: "Bug snapshot was not available after analysis.".to_string(),
                logs,
            });
        }
    };

    // Intentionally avoid logging the output path pre-write to keep logs concise.
    let (git_commit, git_branch, git_commit_timestamp) = match git_revision.as_ref() {
        Some(revision) => (
            Some(revision.commit.clone()),
            revision.branch.clone(),
            revision.commit_timestamp,
        ),
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
    let mut artifacts = match persist_artifacts(
        &request.output_root,
        &repo_path,
        &metadata,
        &bugs_markdown,
        &api_entries_for_persist,
        &classification_rows_for_persist,
        classification_table_for_persist.as_deref(),
        None,
        &snapshot,
    )
    .await
    {
        Ok(paths) => {
            record(
                &mut logs,
                "Prepared bugs snapshot for validation.".to_string(),
            );
            checkpoint.bug_snapshot_path = Some(paths.snapshot_path.clone());
            checkpoint.bugs_path = Some(paths.bugs_path.clone());
            // Report is assembled after validation.
            checkpoint.report_path = None;
            checkpoint.report_html_path = None;
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

    let mut validation_target_additions: Vec<String> = Vec::new();
    let mut validation_target_prep_succeeded = false;
    if let Some(task) = validation_target_prep_task.take() {
        match task.await {
            Ok(outcome) => {
                logs.extend(outcome.logs);
                validation_target_additions = outcome.testing_md_additions;
                validation_target_prep_succeeded = outcome.success;
            }
            Err(join_err) => {
                record(
                    &mut logs,
                    format!("Validation target preparation task failed: {join_err}"),
                );
            }
        }
    }

    let specs_root = request.output_root.join("specs");
    let testing_path = specs_root.join("TESTING.md");
    if !validation_target_additions.is_empty() {
        apply_validation_testing_md_additions(
            &testing_path,
            &repo_path,
            &validation_target_additions,
            &progress_sender,
            &mut logs,
        )
        .await;
    }

    let validate_was_started = matches!(
        plan_tracker.status_for(SecurityReviewPlanStep::ValidateFindings),
        Some(StepStatus::Completed | StepStatus::InProgress)
    );
    if !validate_was_started {
        if !matches!(
            plan_tracker.status_for(SecurityReviewPlanStep::PrepareValidationTargets),
            Some(StepStatus::Completed)
        ) && !validation_target_prep_succeeded
        {
            push_progress_log(
                &progress_sender,
                &log_sink,
                &mut logs,
                "Prepare runnable validation targets did not produce runnable targets; continuing to validation (may record UnableToValidate statuses)."
                    .to_string(),
            );
        }

        plan_tracker.complete_and_start_next(
            SecurityReviewPlanStep::PrepareValidationTargets,
            Some(SecurityReviewPlanStep::ValidateFindings),
        );
    } else if !matches!(
        plan_tracker.status_for(SecurityReviewPlanStep::PrepareValidationTargets),
        Some(StepStatus::Completed)
    ) {
        plan_tracker.mark_complete(SecurityReviewPlanStep::PrepareValidationTargets);
    }
    checkpoint.plan_statuses = plan_tracker.snapshot_statuses();
    persist_checkpoint(&mut checkpoint, &mut logs);

    if !validate_was_started && let Some(linear_issue) = linear_issue.as_ref() {
        let prompt = build_linear_progress_prompt(
            linear_issue,
            &checkpoint.model,
            "Validate findings",
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

    let validation_already_complete = skip_validation
        || matches!(
            plan_tracker.status_for(SecurityReviewPlanStep::ValidateFindings),
            Some(StepStatus::Completed)
        );
    let include_web_browser = request
        .validation_target_url
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    if validation_already_complete {
        if !skip_validation {
            record(
                &mut logs,
                "Validate findings already completed; skipping validation.".to_string(),
            );
        }
    } else {
        let validation_targets = build_validation_findings_context(&snapshot, include_web_browser);

        if validation_targets.ids.is_empty() {
            record(
                &mut logs,
                "No findings selected for validation; skipping.".to_string(),
            );
        } else {
            record(
                &mut logs,
                format!(
                    "Validating findings... (model: {model}, reasoning: {validation_reasoning_label}).",
                    model = request.validation_model.as_str(),
                    validation_reasoning_label =
                        reasoning_effort_label(normalize_reasoning_effort_for_model(
                            request.validation_model.as_str(),
                            validation_reasoning_effort,
                        ))
                ),
            );
            match run_asan_validation(
                repo_path.clone(),
                artifacts.snapshot_path.clone(),
                artifacts.bugs_path.clone(),
                None,
                None,
                request.provider.clone(),
                request.validation_model.clone(),
                validation_reasoning_effort,
                &request.config,
                request.auth_manager.clone(),
                progress_sender.clone(),
                metrics.clone(),
                request.validation_target_url.clone(),
                request.validation_creds_path.clone(),
            )
            .await
            {
                Ok(_) => {
                    record(
                        &mut logs,
                        "Validation complete; snapshot updated.".to_string(),
                    );
                }
                Err(err) => {
                    record(&mut logs, format!("Validation failed: {}", err.message));
                    logs.extend(err.logs);
                }
            }
        }
    }

    match tokio_fs::read(&artifacts.snapshot_path).await {
        Ok(bytes) => match serde_json::from_slice::<SecurityReviewSnapshot>(&bytes) {
            Ok(updated) => {
                snapshot = updated;
                findings_summary = snapshot.findings_summary.clone();
                bugs_for_result = snapshot_bugs(&snapshot);
                bug_summary_table = make_bug_summary_table_from_bugs(&bugs_for_result);
                bugs_markdown = build_bugs_markdown(
                    &snapshot,
                    git_link_info.as_ref(),
                    Some(repo_path.as_path()),
                    Some(request.output_root.as_path()),
                );
            }
            Err(err) => {
                record(
                    &mut logs,
                    format!(
                        "Failed to parse updated bug snapshot {}: {err}",
                        artifacts.snapshot_path.display()
                    ),
                );
            }
        },
        Err(err) => {
            record(
                &mut logs,
                format!(
                    "Failed to read updated bug snapshot {}: {err}",
                    artifacts.snapshot_path.display()
                ),
            );
        }
    }

    if !validation_already_complete {
        plan_tracker.complete_and_start_next(
            SecurityReviewPlanStep::ValidateFindings,
            Some(SecurityReviewPlanStep::PostValidationRefine),
        );
        plan_tracker.complete_and_start_next(
            SecurityReviewPlanStep::PostValidationRefine,
            Some(SecurityReviewPlanStep::AssembleReport),
        );
    }
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

    let findings_section = if bugs_markdown.trim().is_empty() {
        None
    } else {
        Some(format!("# Security Findings\n\n{}", bugs_markdown.trim()))
    };
    let report_markdown = match mode {
        SecurityReviewMode::Full => {
            let mut sections = snapshot.report_sections_prefix.clone();
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
                let cleaned = strip_dev_setup_sections(&cleaned);
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
                let cleaned = strip_dev_setup_sections(&cleaned);
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

    artifacts = match persist_artifacts(
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
    let resume_state_display = display_path_for(
        resume_state_path(&request.output_root).as_path(),
        &repo_path,
    );
    record(&mut logs, format!("Resume state: {resume_state_display}"));

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
        .map(|revision| {
            format_revision_label(
                revision.commit.as_str(),
                revision.branch.as_ref(),
                revision.commit_timestamp,
            )
        })
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

fn render_checklist_markdown(
    mode: SecurityReviewMode,
    statuses: &HashMap<String, StepStatus>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    for step in plan_steps_for_mode(mode) {
        let slug = plan_step_slug(step.kind);
        let title = step.title;
        let status = statuses.get(slug);
        let line = match status {
            Some(StepStatus::Completed) => format!("- [x] ~~{title}~~"),
            Some(StepStatus::InProgress) => format!("- [ ] ▶ **{title}** (current)"),
            _ => format!("- [ ] {title}"),
        };
        lines.push(line);
    }
    lines.join("\n")
}

fn build_linear_scope_context_prompt(issue_ref: &str) -> String {
    linear_scope_context_prompt(issue_ref)
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
    lin_config.model = Some("gpt-5.1".to_string());
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
        .disable(Feature::ViewImageTool);

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
    let checklist = render_checklist_markdown(mode, statuses);
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
    let repo = repo_root.display().to_string();
    let art_dir = output_root.display().to_string();
    linear_init_prompt(
        issue_ref,
        model_name,
        repo.as_str(),
        mode.as_str(),
        checklist.as_str(),
        scope.as_str(),
        include_text.as_str(),
        art_dir.as_str(),
        scope_file_text.as_str(),
        workspace_marker.as_str(),
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
    let checklist = render_checklist_markdown(checkpoint.mode, statuses);
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
    let artifacts_root = output_root.display().to_string();
    linear_progress_prompt(
        issue_ref,
        model_name,
        step_title,
        checklist.as_str(),
        scope_file_text,
        scope_paths_text.as_str(),
        artifacts_root.as_str(),
        artifacts_section.as_str(),
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
    let checklist = render_checklist_markdown(checkpoint.mode, statuses);
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

    let artifacts_root = output_root.display().to_string();
    linear_finalize_prompt(
        issue_ref,
        model_name,
        checklist.as_str(),
        scope_paths_text.as_str(),
        scope_file_text,
        md_path.as_str(),
        html_path.as_str(),
        trufflehog_text.as_str(),
        artifacts_root.as_str(),
        runtime_summary,
        revision_summary,
    )
}

fn build_linear_create_tickets_prompt(issue_ref: &str, bugs_markdown: &str) -> String {
    linear_create_tickets_prompt(issue_ref, bugs_markdown)
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
    let artifacts_root = output_root.display().to_string();
    linear_related_docs_prompt(
        issue_ref,
        model_name,
        scope_paths_text.as_str(),
        scope_file_text,
        artifacts_root.as_str(),
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
    lin_config.model = Some("gpt-5.1".to_string());
    lin_config.model_provider = provider.clone();
    lin_config.base_instructions = Some(LINEAR_STATUS_AGENT_BASE_INSTRUCTIONS.to_string());
    lin_config.user_instructions = None;
    lin_config.developer_instructions = None;
    lin_config.compact_prompt = None;
    lin_config.cwd = repo_root.to_path_buf();
    // Keep MCP servers as configured by the user. Avoid risky tools here.
    lin_config
        .features
        .disable(Feature::ApplyPatchFreeform)
        .disable(Feature::WebSearchRequest)
        .disable(Feature::ViewImageTool);

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

fn emit_progress_log(
    progress_sender: &Option<AppEventSender>,
    log_sink: &Option<Arc<SecurityReviewLogSink>>,
    message: String,
) {
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(message.clone()));
    }
    write_log_sink(log_sink, message.as_str());
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

struct ReasoningAccumulator {
    buffer: String,
}

impl ReasoningAccumulator {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    fn push_delta(
        &mut self,
        delta: &str,
        progress_sender: &Option<AppEventSender>,
        log_sink: &Option<Arc<SecurityReviewLogSink>>,
        logs: &mut Vec<String>,
    ) {
        if delta.is_empty() {
            return;
        }

        self.buffer.push_str(delta);
        self.flush_complete_lines(progress_sender, log_sink, logs);
        if self.buffer.len() >= MODEL_REASONING_LOG_MAX_GRAPHEMES {
            self.flush_remaining(progress_sender, log_sink, logs);
        }
    }

    fn push_full(
        &mut self,
        reasoning: &str,
        progress_sender: &Option<AppEventSender>,
        log_sink: &Option<Arc<SecurityReviewLogSink>>,
        logs: &mut Vec<String>,
    ) {
        self.flush_remaining(progress_sender, log_sink, logs);
        log_model_reasoning(reasoning, progress_sender, log_sink, logs);
    }

    fn flush_complete_lines(
        &mut self,
        progress_sender: &Option<AppEventSender>,
        log_sink: &Option<Arc<SecurityReviewLogSink>>,
        logs: &mut Vec<String>,
    ) {
        if !self.buffer.contains('\n') {
            return;
        }

        let mut parts: Vec<&str> = self.buffer.split('\n').collect();
        let remainder = parts.pop().unwrap_or_default();
        for part in parts {
            if part.trim().is_empty() {
                continue;
            }
            log_model_reasoning(part, progress_sender, log_sink, logs);
        }
        self.buffer = remainder.to_string();
    }

    fn flush_remaining(
        &mut self,
        progress_sender: &Option<AppEventSender>,
        log_sink: &Option<Arc<SecurityReviewLogSink>>,
        logs: &mut Vec<String>,
    ) {
        if self.buffer.trim().is_empty() {
            self.buffer.clear();
            return;
        }
        log_model_reasoning(self.buffer.as_str(), progress_sender, log_sink, logs);
        self.buffer.clear();
    }
}

fn log_dedupe_decision_reasons(
    decisions: &[BugDedupeDecision],
    log_prefix: &str,
    progress_sender: &Option<AppEventSender>,
    log_sink: &Option<Arc<SecurityReviewLogSink>>,
    logs: &mut Vec<String>,
) {
    let mut total_merges = 0usize;
    let mut logged = 0usize;
    for decision in decisions {
        if decision.canonical_id == decision.id {
            continue;
        }
        total_merges = total_merges.saturating_add(1);
        let Some(reason) = decision
            .reason
            .as_ref()
            .map(|reason| reason.trim())
            .filter(|reason| !reason.is_empty())
        else {
            continue;
        };
        if logged >= BUG_LLM_DEDUP_REASON_LOG_LIMIT {
            continue;
        }
        let truncated = truncate_text(reason, MODEL_REASONING_LOG_MAX_GRAPHEMES);
        let id = decision.id;
        let canonical_id = decision.canonical_id;
        let message = format!("{log_prefix} decision: {id} -> {canonical_id} ({truncated})");
        push_progress_log(progress_sender, log_sink, logs, message);
        logged = logged.saturating_add(1);
    }
    if total_merges > 0 && logged == 0 {
        push_progress_log(
            progress_sender,
            log_sink,
            logs,
            format!("{log_prefix} decision reasons unavailable for {total_merges} merge(s)."),
        );
        return;
    }
    let remaining = total_merges.saturating_sub(logged);
    if remaining > 0 {
        push_progress_log(
            progress_sender,
            log_sink,
            logs,
            format!("{log_prefix} decision reasons omitted for {remaining} merge(s)."),
        );
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
        vec![repo_path.clone()]
    } else {
        include_paths
    };

    let mut used_git_ls_files = false;
    if let Ok(git_root_output) = StdCommand::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&repo_path)
        .output()
        && git_root_output.status.success()
        && let Ok(git_root_stdout) = String::from_utf8(git_root_output.stdout)
        && let Ok(output) = StdCommand::new("git")
            .args(["ls-files", "-z"])
            .current_dir(&repo_path)
            .output()
        && output.status.success()
    {
        let git_root = PathBuf::from(git_root_stdout.trim());
        used_git_ls_files = true;
        state.emit_progress_message(
            "Collecting tracked files (git ls-files) to skip untracked files and avoid local build/test artifacts..."
                .to_string(),
        );
        let mut tracked_files: Vec<PathBuf> = output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|chunk| !chunk.is_empty())
            .map(|chunk| PathBuf::from(String::from_utf8_lossy(chunk).to_string()))
            .collect();
        tracked_files.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

        let in_scope =
            |path: &Path| -> bool { targets.iter().any(|target| path.starts_with(target)) };

        for rel in tracked_files {
            if state.limit_reached() {
                break;
            }
            let abs = git_root.join(&rel);
            if !in_scope(&abs) {
                continue;
            }
            if path_has_excluded_dir_component(&rel) {
                continue;
            }
            let metadata = match fs::symlink_metadata(&abs) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }
            if let Err(err) = state.visit_file(&abs, metadata.len() as usize) {
                logs.push(err.clone());
                return Err(SecurityReviewFailure { message: err, logs });
            }
        }
    }

    if !used_git_ls_files {
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
    }

    if state.snippets.is_empty() {
        if used_git_ls_files {
            logs.push("No eligible tracked files found during collection.".to_string());
        } else {
            logs.push("No eligible files found during collection.".to_string());
        }
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
    reasoning_effort: Option<ReasoningEffort>,
    scope_prompt: Option<String>,
    snippets: Vec<FileSnippet>,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
) -> Result<FileTriageResult, SecurityReviewFailure> {
    let total = snippets.len();
    let mut logs: Vec<String> = Vec::new();

    if total == 0 {
        return Ok(FileTriageResult {
            included: Vec::new(),
            logs,
        });
    }

    let scope_prompt = scope_prompt
        .map(|prompt| prompt.trim().to_string())
        .filter(|prompt| !prompt.is_empty())
        .map(Arc::from);

    let mut descriptors: Vec<FileTriageDescriptor> = Vec::with_capacity(total);
    for (idx, snippet) in snippets.iter().enumerate() {
        let id = idx.saturating_add(1);
        let path = snippet.relative_path.to_string_lossy().to_string();
        let preview: String = snippet
            .content
            .chars()
            .take(FILE_TRIAGE_PREVIEW_CHARS)
            .collect();
        let listing_json = json!({
            "id": id,
            "path": path.as_str(),
            "preview": preview,
        })
        .to_string();
        descriptors.push(FileTriageDescriptor {
            id,
            path,
            listing_json,
        });
    }

    let chunk_size = FILE_TRIAGE_CHUNK_SIZE.max(1);
    let mut chunk_requests: Vec<FileTriageChunkRequest> = Vec::new();
    for (chunk_idx, chunk) in descriptors.chunks(chunk_size).enumerate() {
        let start_idx = chunk_idx.saturating_mul(chunk_size);
        let end_idx = start_idx.saturating_add(chunk.len().saturating_sub(1));
        chunk_requests.push(FileTriageChunkRequest {
            start_idx,
            end_idx,
            descriptors: chunk.to_vec(),
        });
    }

    let total_chunks = chunk_requests.len();
    let max_concurrency = FILE_TRIAGE_CONCURRENCY.max(1).min(total_chunks.max(1));

    let chunk_results = futures::stream::iter(chunk_requests.into_iter().map(|request| {
        let provider = provider.clone();
        let auth = auth.clone();
        let triage_model = triage_model.to_string();
        let scope_prompt = scope_prompt.clone();
        let progress_sender = progress_sender.clone();
        let log_sink = log_sink.clone();
        let metrics = metrics.clone();
        let fallback_ids: Vec<usize> = request.descriptors.iter().map(|d| d.id).collect();
        let range_start = request.start_idx.saturating_add(1);
        let range_end = request.end_idx.saturating_add(1);

        async move {
            (
                fallback_ids,
                range_start,
                range_end,
                triage_chunk(
                    client,
                    provider,
                    auth,
                    triage_model,
                    reasoning_effort,
                    scope_prompt,
                    request,
                    progress_sender,
                    log_sink,
                    total,
                    metrics,
                )
                .await,
            )
        }
    }))
    .buffer_unordered(max_concurrency);
    futures::pin_mut!(chunk_results);

    let mut include_ids: HashSet<usize> = HashSet::new();
    let mut processed_files = 0usize;

    while let Some((fallback_ids, range_start, range_end, result)) = chunk_results.next().await {
        match result {
            Ok(mut chunk) => {
                logs.append(&mut chunk.logs);
                processed_files = processed_files.saturating_add(chunk.processed);
                include_ids.extend(chunk.include_ids);
            }
            Err(mut failure) => {
                logs.append(&mut failure.logs);
                let fallback_count = fallback_ids.len();
                let fallback_message = format!(
                    "File triage failed for files {range_start}-{range_end} ({fallback_count} file(s)); including all by default."
                );
                push_progress_log(&progress_sender, &log_sink, &mut logs, fallback_message);
                processed_files = processed_files.saturating_add(fallback_count);
                include_ids.extend(fallback_ids);
            }
        }

        if let Some(tx) = progress_sender.as_ref() {
            let percent = if total == 0 {
                0
            } else {
                (processed_files.min(total) * 100) / total
            };
            tx.send(AppEvent::SecurityReviewLog(format!(
                "File triage progress: {}/{} - {percent}%.",
                processed_files.min(total),
                total
            )));
        }
    }

    if include_ids.is_empty() {
        push_progress_log(
            &progress_sender,
            &log_sink,
            &mut logs,
            "File triage excluded every file; including all by default.".to_string(),
        );
        include_ids.extend(1..=total);
    }

    let included: Vec<FileSnippet> = snippets
        .into_iter()
        .enumerate()
        .filter_map(|(idx, snippet)| {
            let id = idx.saturating_add(1);
            include_ids.contains(&id).then_some(snippet)
        })
        .collect();

    let kept = included.len();
    push_progress_log(
        &progress_sender,
        &log_sink,
        &mut logs,
        format!("File triage kept {kept}/{total} file(s)."),
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
    reasoning_effort: Option<ReasoningEffort>,
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
        .join("\n");

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
            reasoning_effort,
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
    spec_model: &str,
    spec_reasoning_effort: Option<ReasoningEffort>,
    writing_model: &str,
    writing_reasoning_effort: Option<ReasoningEffort>,
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

    let heuristically_filtered: Vec<(PathBuf, String)> = if spec_progress_state.targets.is_empty() {
        let mut filtered: Vec<(PathBuf, String)> = Vec::new();
        for (path, label) in &directory_candidates {
            if is_spec_dir_likely_low_signal(path) {
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(format!(
                        "Heuristic skip for spec dir {label} (tests/CI/fuzzing/tooling)."
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

    let allow_llm_directory_filter = include_paths.is_empty();
    let mut used_llm_filter = false;
    let mut filtered_dirs = if spec_progress_state.targets.is_empty() {
        if !allow_llm_directory_filter {
            let kept = heuristically_filtered.len();
            let total = directory_candidates.len();
            let message = format!(
                "Skipping spec directory filter: using {kept}/{total} preselected director{}.",
                if kept == 1 { "y" } else { "ies" }
            );
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(message.clone()));
            }
            logs.push(message);
            heuristically_filtered
        } else if heuristically_filtered.len() <= SPEC_DIR_FILTER_TARGET {
            let kept = heuristically_filtered.len();
            let total = directory_candidates.len();
            let message = format!(
                "Skipping spec directory filter: {kept}/{total} candidates already <= {SPEC_DIR_FILTER_TARGET}."
            );
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(message.clone()));
            }
            logs.push(message);
            heuristically_filtered
        } else {
            match filter_spec_directories(
                client,
                provider,
                auth,
                spec_model,
                spec_reasoning_effort,
                repo_root,
                &heuristically_filtered,
                metrics.clone(),
            )
            .await
            {
                Ok(result) => {
                    used_llm_filter = true;
                    result
                }
                Err(err) => {
                    if let Some(tx) = progress_sender.as_ref() {
                        for line in &err.logs {
                            tx.send(AppEvent::SecurityReviewLog(line.clone()));
                        }
                    }
                    logs.extend(err.logs);
                    let kept = heuristically_filtered.len();
                    let total = directory_candidates.len();
                    let message = format!(
                        "Directory filter failed; using {kept}/{total} heuristic-filtered directories. {}",
                        err.message
                    );
                    if let Some(tx) = progress_sender.as_ref() {
                        tx.send(AppEvent::SecurityReviewLog(message.clone()));
                    }
                    logs.push(message);
                    heuristically_filtered
                }
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
            if used_llm_filter {
                format!("Spec directory filter kept {kept}/{total} directories using {spec_model}.")
            } else {
                format!("Selected {kept}/{total} directories for specification generation.")
            }
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
        spec_model,
        spec_reasoning_effort,
        writing_model,
        writing_reasoning_effort,
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
        spec_model,
        None,
        writing_model,
        None,
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
    match extract_data_classification(
        client,
        provider,
        auth,
        spec_model,
        spec_reasoning_effort,
        &combined_markdown,
        metrics.clone(),
    )
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
    reasoning_effort: Option<ReasoningEffort>,
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
        reasoning_effort,
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
    auto_config.model = Some(model.to_string());
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
        .disable(Feature::ViewImageTool);
    auto_config.mcp_servers.clear();

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

    let keyword_reasoning_effort = config
        .security_review_reasoning_efforts
        .file_triage
        .or(config.model_reasoning_effort);

    let mut keywords = match expand_auto_scope_keywords(
        client,
        provider,
        auth,
        keyword_reasoning_effort,
        user_query,
        metrics.clone(),
    )
    .await
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
            error_message: None,
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

#[cfg(test)]
mod bug_dedupe_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    fn make_bug_markdown(title: &str, file: &str) -> String {
        format!(
            "### {title}\n- **File & Lines:** `{file}`\n- **Severity:** High\n- **Impact:** x\n- **Likelihood:** x\n- **Recommendation:** x\n"
        )
    }

    fn make_bug_summary(id: usize, title: &str, file: &str, tag: Option<&str>) -> BugSummary {
        BugSummary {
            id,
            title: title.to_string(),
            file: file.to_string(),
            severity: "High".to_string(),
            impact: "x".to_string(),
            likelihood: "x".to_string(),
            recommendation: "x".to_string(),
            blame: None,
            risk_score: None,
            risk_rank: None,
            risk_reason: None,
            verification_types: Vec::new(),
            vulnerability_tag: tag.map(str::to_string),
            validation: BugValidationState::default(),
            source_path: PathBuf::new(),
            markdown: make_bug_markdown(title, file),
            author_github: None,
        }
    }

    #[test]
    fn prunes_file_lines_to_sinks_from_dataflow() {
        let markdown = r#"
### Example finding
- **File & Lines:** `src/source.rs#L1-L2, src/prop.rs#L3-L4, src/sink.rs#L10-L12`
- **Description:** Vulnerable behavior at `src/sink.rs#L10-L12`.
- **Dataflow:**
    - Source: `src/source.rs#L1-L2`
    - Propagation: `src/prop.rs#L3-L4`
    - Sink: `src/sink.rs#L10-L12`
- **Recommendation:** Fix it.
"#;

        let mut next_id = 1usize;
        let (summaries, details) = extract_bug_summaries(
            markdown,
            "default.rs#L1-L1",
            Path::new("src/lib.rs"),
            &mut next_id,
        );
        assert_eq!(summaries.len(), 1);
        assert_eq!(details.len(), 1);
        assert_eq!(summaries[0].file, "src/sink.rs#L10-L12");
        assert!(
            summaries[0]
                .markdown
                .contains("- **File & Lines:** `src/sink.rs#L10-L12`")
        );
        assert!(
            details[0]
                .original_markdown
                .contains("- **File & Lines:** `src/sink.rs#L10-L12`")
        );
    }

    #[test]
    fn dedupe_uses_sink_locations_for_file_lines() {
        fn bug_markdown(title: &str, file: &str, sink: &str) -> String {
            format!(
                "### {title}\n- **File & Lines:** `{file}`\n- **Severity:** High\n- **Impact:** x\n- **Likelihood:** x\n- **Description:** see `{sink}`\n- **Dataflow:**\n    - Sink: `{sink}`\n- **Recommendation:** x\n"
            )
        }

        let summaries = vec![
            BugSummary {
                id: 1,
                title: "Finding A".to_string(),
                file: "a.rs#L1-L2, a.rs#L10-L12".to_string(),
                severity: "High".to_string(),
                impact: "x".to_string(),
                likelihood: "x".to_string(),
                recommendation: "x".to_string(),
                blame: None,
                risk_score: None,
                risk_rank: None,
                risk_reason: None,
                verification_types: Vec::new(),
                vulnerability_tag: Some("example-sink".to_string()),
                validation: BugValidationState::default(),
                source_path: PathBuf::new(),
                markdown: bug_markdown("Finding A", "a.rs#L1-L2, a.rs#L10-L12", "a.rs#L10-L12"),
                author_github: None,
            },
            BugSummary {
                id: 2,
                title: "Finding B".to_string(),
                file: "b.rs#L3-L4, b.rs#L30-L34".to_string(),
                severity: "High".to_string(),
                impact: "x".to_string(),
                likelihood: "x".to_string(),
                recommendation: "x".to_string(),
                blame: None,
                risk_score: None,
                risk_rank: None,
                risk_reason: None,
                verification_types: Vec::new(),
                vulnerability_tag: Some("example-sink".to_string()),
                validation: BugValidationState::default(),
                source_path: PathBuf::new(),
                markdown: bug_markdown("Finding B", "b.rs#L3-L4, b.rs#L30-L34", "b.rs#L30-L34"),
                author_github: None,
            },
        ];
        let details = summaries
            .iter()
            .map(|summary| BugDetail {
                summary_id: summary.id,
                original_markdown: summary.markdown.clone(),
            })
            .collect::<Vec<_>>();

        let (summaries, _details, removed) = dedupe_bug_summaries(summaries, details);
        assert_eq!(removed, 1);
        assert_eq!(summaries.len(), 1);
        let file = summaries[0].file.clone();
        assert!(file.contains("a.rs#L10-L12"));
        assert!(file.contains("b.rs#L30-L34"));
        assert!(!file.contains("a.rs#L1-L2"));
        assert!(!file.contains("b.rs#L3-L4"));
    }

    #[test]
    fn parses_taxonomy_lines_in_common_formats() {
        let lines = [
            (
                "TAXONOMY: {\"vuln_tag\":\"tls-hostname-not-verified\"}",
                "tls-hostname-not-verified",
            ),
            (
                "- TAXONOMY: {\"vuln_tag\":\"tls-hostname-not-verified\"}",
                "tls-hostname-not-verified",
            ),
            (
                "- TAXONOMY: `{\"vuln_tag\":\"tls-hostname-not-verified\"}`",
                "tls-hostname-not-verified",
            ),
            (
                "- **TAXONOMY:** `{\"vuln_tag\":\"tls-hostname-not-verified\"}`",
                "tls-hostname-not-verified",
            ),
            (
                "- TAXONOMY: {{\"vuln_tag\":\"tls-hostname-not-verified\"}}",
                "tls-hostname-not-verified",
            ),
        ];

        for (line, expected) in lines {
            assert_eq!(
                parse_taxonomy_vuln_tag(line).as_deref(),
                Some(expected),
                "failed to parse taxonomy line: {line}"
            );
        }
    }

    #[test]
    fn dedupes_similar_vulnerability_tags() {
        let summaries = vec![
            make_bug_summary(
                1,
                "Missing hostname verification (A)",
                "a.rs#L1-L2",
                Some("tls-hostname-not-verified"),
            ),
            make_bug_summary(
                2,
                "Missing hostname verification (B)",
                "b.rs#L3-L4",
                Some("tls-hostname-bypass"),
            ),
        ];
        let details = vec![
            BugDetail {
                summary_id: 1,
                original_markdown: make_bug_markdown(
                    "Missing hostname verification (A)",
                    "a.rs#L1-L2",
                ),
            },
            BugDetail {
                summary_id: 2,
                original_markdown: make_bug_markdown(
                    "Missing hostname verification (B)",
                    "b.rs#L3-L4",
                ),
            },
        ];

        let (summaries, details, removed) = dedupe_bug_summaries(summaries, details);
        assert_eq!(removed, 1);
        assert_eq!(summaries.len(), 1);
        assert_eq!(details.len(), 1);
        let file = summaries[0].file.clone();
        assert!(file.contains("a.rs#L1-L2"));
        assert!(file.contains("b.rs#L3-L4"));
    }

    #[test]
    fn avoids_deduping_on_generic_overlap_only() {
        let summaries = vec![
            make_bug_summary(
                1,
                "Heap buffer overflow",
                "a.rs#L1-L2",
                Some("heap-buffer-overflow"),
            ),
            make_bug_summary(
                2,
                "Stack buffer overflow",
                "b.rs#L3-L4",
                Some("stack-buffer-overflow"),
            ),
        ];
        let details = vec![
            BugDetail {
                summary_id: 1,
                original_markdown: make_bug_markdown("Heap buffer overflow", "a.rs#L1-L2"),
            },
            BugDetail {
                summary_id: 2,
                original_markdown: make_bug_markdown("Stack buffer overflow", "b.rs#L3-L4"),
            },
        ];

        let (summaries, details, removed) = dedupe_bug_summaries(summaries, details);
        assert_eq!(removed, 0);
        assert_eq!(summaries.len(), 2);
        assert_eq!(details.len(), 2);
    }

    #[test]
    fn dedupes_loaded_snapshot_bugs() {
        let original_markdown_a = format!(
            "{}\n- TAXONOMY: {{\"vuln_tag\":\"tls-hostname-not-verified\"}}\n",
            make_bug_markdown("Missing hostname verification (A)", "a.rs#L1-L2")
        );
        let original_markdown_b = format!(
            "{}\n- **TAXONOMY:** `{{\"vuln_tag\":\"tls-hostname-bypass\"}}`\n",
            make_bug_markdown("Missing hostname verification (B)", "b.rs#L3-L4")
        );
        let mut snapshot = SecurityReviewSnapshot {
            generated_at: OffsetDateTime::now_utc(),
            findings_summary: "placeholder".to_string(),
            report_sections_prefix: Vec::new(),
            bugs: vec![
                BugSnapshot {
                    bug: SecurityReviewBug {
                        summary_id: 1,
                        risk_rank: Some(1),
                        risk_score: Some(9.0),
                        title: "Missing hostname verification (A)".to_string(),
                        severity: "High".to_string(),
                        impact: "x".to_string(),
                        likelihood: "x".to_string(),
                        recommendation: "x".to_string(),
                        file: "a.rs#L1-L2".to_string(),
                        blame: None,
                        risk_reason: None,
                        verification_types: Vec::new(),
                        vulnerability_tag: None,
                        validation: BugValidationState::default(),
                        assignee_github: None,
                    },
                    original_markdown: original_markdown_a,
                },
                BugSnapshot {
                    bug: SecurityReviewBug {
                        summary_id: 2,
                        risk_rank: Some(2),
                        risk_score: Some(8.0),
                        title: "Missing hostname verification (B)".to_string(),
                        severity: "High".to_string(),
                        impact: "x".to_string(),
                        likelihood: "x".to_string(),
                        recommendation: "x".to_string(),
                        file: "b.rs#L3-L4".to_string(),
                        blame: None,
                        risk_reason: None,
                        verification_types: Vec::new(),
                        vulnerability_tag: None,
                        validation: BugValidationState::default(),
                        assignee_github: None,
                    },
                    original_markdown: original_markdown_b,
                },
            ],
        };

        let removed = dedupe_security_review_snapshot(&mut snapshot);
        assert_eq!(removed, 1);
        assert_eq!(snapshot.bugs.len(), 1);
        assert!(snapshot.findings_summary.contains("Identified 1 finding"));
    }

    #[test]
    fn reranks_snapshot_bugs_by_severity_after_dedupe() {
        fn bug_markdown(title: &str, file: &str, severity: &str) -> String {
            format!(
                "### {title}\n- **File & Lines:** `{file}`\n- **Severity:** {severity}\n- **Impact:** x\n- **Likelihood:** x\n- **Recommendation:** x\n"
            )
        }

        let mut snapshot = SecurityReviewSnapshot {
            generated_at: OffsetDateTime::now_utc(),
            findings_summary: "placeholder".to_string(),
            report_sections_prefix: Vec::new(),
            bugs: vec![
                BugSnapshot {
                    bug: SecurityReviewBug {
                        summary_id: 1,
                        risk_rank: Some(1),
                        risk_score: Some(90.0),
                        title: "Medium first by old rank".to_string(),
                        severity: "Medium".to_string(),
                        impact: "x".to_string(),
                        likelihood: "x".to_string(),
                        recommendation: "x".to_string(),
                        file: "b.rs#L3-L4".to_string(),
                        blame: None,
                        risk_reason: None,
                        verification_types: Vec::new(),
                        vulnerability_tag: None,
                        validation: BugValidationState::default(),
                        assignee_github: None,
                    },
                    original_markdown: bug_markdown(
                        "Medium first by old rank",
                        "b.rs#L3-L4",
                        "Medium",
                    ),
                },
                BugSnapshot {
                    bug: SecurityReviewBug {
                        summary_id: 2,
                        risk_rank: Some(2),
                        risk_score: Some(50.0),
                        title: "Low second by old rank".to_string(),
                        severity: "Low".to_string(),
                        impact: "x".to_string(),
                        likelihood: "x".to_string(),
                        recommendation: "x".to_string(),
                        file: "c.rs#L5-L6".to_string(),
                        blame: None,
                        risk_reason: None,
                        verification_types: Vec::new(),
                        vulnerability_tag: None,
                        validation: BugValidationState::default(),
                        assignee_github: None,
                    },
                    original_markdown: bug_markdown("Low second by old rank", "c.rs#L5-L6", "Low"),
                },
                BugSnapshot {
                    bug: SecurityReviewBug {
                        summary_id: 3,
                        risk_rank: Some(3),
                        risk_score: Some(10.0),
                        title: "High last by old rank".to_string(),
                        severity: "High".to_string(),
                        impact: "x".to_string(),
                        likelihood: "x".to_string(),
                        recommendation: "x".to_string(),
                        file: "a.rs#L1-L2".to_string(),
                        blame: None,
                        risk_reason: None,
                        verification_types: Vec::new(),
                        vulnerability_tag: None,
                        validation: BugValidationState::default(),
                        assignee_github: None,
                    },
                    original_markdown: bug_markdown("High last by old rank", "a.rs#L1-L2", "High"),
                },
            ],
        };

        let removed = dedupe_security_review_snapshot(&mut snapshot);
        assert_eq!(removed, 0);
        assert_eq!(snapshot.bugs.len(), 3);

        let severities = snapshot
            .bugs
            .iter()
            .map(|entry| entry.bug.severity.as_str())
            .collect::<Vec<_>>();
        assert_eq!(severities, vec!["High", "Medium", "Low"]);

        let ranks = snapshot
            .bugs
            .iter()
            .map(|entry| entry.bug.risk_rank)
            .collect::<Vec<_>>();
        assert_eq!(ranks, vec![Some(1), Some(2), Some(3)]);

        let ids = snapshot
            .bugs
            .iter()
            .map(|entry| entry.bug.summary_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec![1, 2, 3]);

        snapshot.bugs.reverse();
        let markdown = build_bugs_markdown(&snapshot, None, None, None);
        let high = markdown
            .find("### <a id=\"bug-1\"></a> [1] High last by old rank")
            .expect("high heading");
        let medium = markdown
            .find("### <a id=\"bug-2\"></a> [2] Medium first by old rank")
            .expect("medium heading");
        let low = markdown
            .find("### <a id=\"bug-3\"></a> [3] Low second by old rank")
            .expect("low heading");
        assert!(high < medium);
        assert!(medium < low);
    }

    #[test]
    fn omits_ignored_findings_from_report_markdown() {
        fn bug_markdown(title: &str, file: &str, severity: &str) -> String {
            format!(
                "### {title}\n- **File & Lines:** `{file}`\n- **Severity:** {severity}\n- **Impact:** x\n- **Likelihood:** x\n- **Recommendation:** x\n"
            )
        }

        let snapshot = SecurityReviewSnapshot {
            generated_at: OffsetDateTime::now_utc(),
            findings_summary: "placeholder".to_string(),
            report_sections_prefix: Vec::new(),
            bugs: vec![
                BugSnapshot {
                    bug: SecurityReviewBug {
                        summary_id: 1,
                        risk_rank: Some(1),
                        risk_score: Some(90.0),
                        title: "High bug".to_string(),
                        severity: "High".to_string(),
                        impact: "x".to_string(),
                        likelihood: "x".to_string(),
                        recommendation: "x".to_string(),
                        file: "a.rs#L1-L2".to_string(),
                        blame: None,
                        risk_reason: None,
                        verification_types: Vec::new(),
                        vulnerability_tag: None,
                        validation: BugValidationState::default(),
                        assignee_github: None,
                    },
                    original_markdown: bug_markdown("High bug", "a.rs#L1-L2", "High"),
                },
                BugSnapshot {
                    bug: SecurityReviewBug {
                        summary_id: 2,
                        risk_rank: Some(2),
                        risk_score: Some(0.0),
                        title: "Ignored bug".to_string(),
                        severity: "Ignore".to_string(),
                        impact: "x".to_string(),
                        likelihood: "x".to_string(),
                        recommendation: "x".to_string(),
                        file: "b.rs#L3-L4".to_string(),
                        blame: None,
                        risk_reason: None,
                        verification_types: Vec::new(),
                        vulnerability_tag: None,
                        validation: BugValidationState::default(),
                        assignee_github: None,
                    },
                    original_markdown: bug_markdown("Ignored bug", "b.rs#L3-L4", "Ignore"),
                },
            ],
        };

        let markdown = build_bugs_markdown(&snapshot, None, None, None);
        assert!(markdown.contains("High bug"));
        assert!(!markdown.contains("Ignored bug"));
    }
}

#[cfg(test)]
mod validation_classification_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn python_exit_code_2_maps_to_unable_to_validate() {
        let status = classify_python_validation_status(false, Some(2), false, "", "some failure");
        assert_eq!(status, BugValidationStatus::UnableToValidate);
    }

    #[test]
    fn python_build_failures_map_to_unable_to_validate() {
        let status = classify_python_validation_status(
            false,
            Some(1),
            false,
            "",
            "error: could not compile `foo`",
        );
        assert_eq!(status, BugValidationStatus::UnableToValidate);
    }

    #[test]
    fn python_expect_asan_requires_signature() {
        let status = classify_python_validation_status(true, Some(0), true, "ok", "");
        assert_eq!(status, BugValidationStatus::Failed);
    }

    #[test]
    fn python_expect_asan_passes_when_signature_present_even_if_nonzero() {
        let status = classify_python_validation_status(
            true,
            Some(1),
            false,
            "AddressSanitizer: heap-buffer-overflow",
            "",
        );
        assert_eq!(status, BugValidationStatus::Passed);
    }
}

#[cfg(test)]
mod validation_target_selection_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_bug_snapshot(
        summary_id: usize,
        risk_rank: usize,
        title: &str,
        severity: &str,
        verification_types: Vec<String>,
    ) -> BugSnapshot {
        BugSnapshot {
            bug: SecurityReviewBug {
                summary_id,
                risk_rank: Some(risk_rank),
                risk_score: Some(0.0),
                title: title.to_string(),
                severity: severity.to_string(),
                impact: "x".to_string(),
                likelihood: "x".to_string(),
                recommendation: "x".to_string(),
                file: "x.rs#L1-L2".to_string(),
                blame: None,
                risk_reason: None,
                verification_types,
                vulnerability_tag: Some("idor".to_string()),
                validation: BugValidationState::default(),
                assignee_github: None,
            },
            original_markdown: format!("# {title}\n\nDetails.\n"),
        }
    }

    #[test]
    fn excludes_low_risk_findings_from_validation_targets() {
        let snapshot = SecurityReviewSnapshot {
            generated_at: OffsetDateTime::now_utc(),
            findings_summary: String::new(),
            report_sections_prefix: Vec::new(),
            bugs: vec![
                make_bug_snapshot(1, 1, "High risk finding", "High", Vec::new()),
                make_bug_snapshot(2, 10, "Low risk finding", "Low", Vec::new()),
            ],
        };

        let targets = build_validation_findings_context(&snapshot, false);
        assert_eq!(targets.ids, vec![BugIdentifier::RiskRank(1)]);
    }

    #[test]
    fn includes_web_browser_findings_in_validation_targets_even_when_disabled() {
        let snapshot = SecurityReviewSnapshot {
            generated_at: OffsetDateTime::now_utc(),
            findings_summary: String::new(),
            report_sections_prefix: Vec::new(),
            bugs: vec![
                make_bug_snapshot(
                    1,
                    1,
                    "Browser-only finding",
                    "High",
                    vec!["web_browser".to_string()],
                ),
                make_bug_snapshot(2, 2, "API finding", "High", vec!["network_api".to_string()]),
            ],
        };

        let targets = build_validation_findings_context(&snapshot, false);
        assert_eq!(
            targets.ids,
            vec![BugIdentifier::RiskRank(1), BugIdentifier::RiskRank(2)]
        );
    }

    #[test]
    fn includes_only_web_browser_findings_when_present() {
        let snapshot = SecurityReviewSnapshot {
            generated_at: OffsetDateTime::now_utc(),
            findings_summary: String::new(),
            report_sections_prefix: Vec::new(),
            bugs: vec![make_bug_snapshot(
                1,
                1,
                "Browser-only finding",
                "High",
                vec!["web_browser".to_string()],
            )],
        };

        let targets = build_validation_findings_context(&snapshot, false);
        assert_eq!(targets.ids, vec![BugIdentifier::RiskRank(1)]);
    }

    #[test]
    fn includes_web_browser_findings_when_enabled() {
        let snapshot = SecurityReviewSnapshot {
            generated_at: OffsetDateTime::now_utc(),
            findings_summary: String::new(),
            report_sections_prefix: Vec::new(),
            bugs: vec![
                make_bug_snapshot(
                    1,
                    1,
                    "Browser-only finding",
                    "High",
                    vec!["web_browser".to_string()],
                ),
                make_bug_snapshot(2, 2, "API finding", "High", vec!["network_api".to_string()]),
            ],
        };

        let targets = build_validation_findings_context(&snapshot, true);
        assert_eq!(
            targets.ids,
            vec![BugIdentifier::RiskRank(1), BugIdentifier::RiskRank(2)]
        );
    }

    #[test]
    fn includes_crash_poc_release_and_crash_poc_func() {
        let snapshot = SecurityReviewSnapshot {
            generated_at: OffsetDateTime::now_utc(),
            findings_summary: String::new(),
            report_sections_prefix: Vec::new(),
            bugs: vec![
                make_bug_snapshot(
                    1,
                    1,
                    "Standalone crash in a function",
                    "High",
                    vec!["crash_poc_func".to_string()],
                ),
                make_bug_snapshot(
                    2,
                    2,
                    "Crash reachable from shipped entrypoint",
                    "High",
                    vec!["crash_poc_release".to_string()],
                ),
            ],
        };

        let targets = build_validation_findings_context(&snapshot, true);
        assert_eq!(
            targets.ids,
            vec![BugIdentifier::RiskRank(1), BugIdentifier::RiskRank(2)]
        );
    }

    #[test]
    fn treats_crash_poc_release_bin_as_crash_poc_release() {
        let bug = SecurityReviewBug {
            summary_id: 1,
            risk_rank: Some(1),
            risk_score: Some(0.0),
            title: "Crash".to_string(),
            severity: "High".to_string(),
            impact: "x".to_string(),
            likelihood: "x".to_string(),
            recommendation: "x".to_string(),
            file: "x.rs#L1-L2".to_string(),
            blame: None,
            risk_reason: None,
            verification_types: vec!["crash_poc_release_bin".to_string()],
            vulnerability_tag: None,
            validation: BugValidationState::default(),
            assignee_github: None,
        };

        assert_eq!(crash_poc_category(&bug), Some("crash_poc_release"));
    }

    #[test]
    fn expects_asan_only_for_release_crash_categories() {
        let release = SecurityReviewBug {
            summary_id: 1,
            risk_rank: Some(1),
            risk_score: Some(0.0),
            title: "Crash".to_string(),
            severity: "High".to_string(),
            impact: "x".to_string(),
            likelihood: "x".to_string(),
            recommendation: "x".to_string(),
            file: "x.rs#L1-L2".to_string(),
            blame: None,
            risk_reason: None,
            verification_types: vec!["crash_poc_release".to_string()],
            vulnerability_tag: None,
            validation: BugValidationState::default(),
            assignee_github: None,
        };
        assert_eq!(expects_asan_for_bug(&release), true);

        let func = SecurityReviewBug {
            verification_types: vec!["crash_poc_func".to_string()],
            ..release
        };
        assert_eq!(expects_asan_for_bug(&func), false);
    }

    #[test]
    fn includes_rce_bin_findings_even_without_vuln_tag() {
        let snapshot = SecurityReviewSnapshot {
            generated_at: OffsetDateTime::now_utc(),
            findings_summary: String::new(),
            report_sections_prefix: Vec::new(),
            bugs: vec![BugSnapshot {
                bug: SecurityReviewBug {
                    summary_id: 1,
                    risk_rank: Some(1),
                    risk_score: Some(0.0),
                    title: "RCE via config".to_string(),
                    severity: "High".to_string(),
                    impact: "x".to_string(),
                    likelihood: "x".to_string(),
                    recommendation: "x".to_string(),
                    file: "x.rs#L1-L2".to_string(),
                    blame: None,
                    risk_reason: None,
                    verification_types: vec!["rce_bin".to_string()],
                    vulnerability_tag: None,
                    validation: BugValidationState::default(),
                    assignee_github: None,
                },
                original_markdown: "# RCE\n\nDetails.\n".to_string(),
            }],
        };

        let targets = build_validation_findings_context(&snapshot, true);
        assert_eq!(targets.ids, vec![BugIdentifier::RiskRank(1)]);
    }
}

#[cfg(test)]
mod web_validation_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalizes_base_url_and_strips_path_query() {
        let base =
            normalize_web_validation_base_url("https://example.com/foo/bar?x=1#frag").expect("ok");
        assert_eq!(base.as_str(), "https://example.com/");
    }

    #[test]
    fn rejects_target_url_with_userinfo() {
        let err = normalize_web_validation_base_url("https://user:pass@example.com/")
            .expect_err("should reject");
        assert!(err.contains("userinfo"));
    }

    #[test]
    fn resolves_relative_targets_and_rejects_other_origins() {
        let base = normalize_web_validation_base_url("https://example.com/app").expect("ok");
        let joined = resolve_web_validation_target(&base, Some("/api/v1/users")).expect("ok join");
        assert_eq!(joined.as_str(), "https://example.com/api/v1/users");

        let err = resolve_web_validation_target(&base, Some("https://evil.com/")).expect_err("bad");
        assert!(err.contains("non-target origin"));
    }

    #[test]
    fn parses_creds_from_json_headers_object() {
        let headers =
            parse_web_validation_creds(r#"{"headers":{"Authorization":"Bearer abcdefgh"}} "#);
        assert_eq!(
            headers,
            vec![("Authorization".to_string(), "Bearer abcdefgh".to_string())]
        );
    }

    #[test]
    fn parses_creds_from_header_lines() {
        let headers = parse_web_validation_creds(
            r#"
# comment
Authorization: Bearer abcdefgh
X_FOO: bar
"#,
        );
        assert_eq!(
            headers,
            vec![
                ("Authorization".to_string(), "Bearer abcdefgh".to_string()),
                ("X_FOO".to_string(), "bar".to_string())
            ]
        );
    }
}

#[cfg(test)]
mod exploit_scenario_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn extracts_trigger_excerpt_from_validation_output() {
        let output = r#"
=== CONTROL ===
$ ./control_cmd --foo bar

=== TRIGGER ===
$ ./target_cmd --evil input.bin
INPUT:
line1
line2

done
"#;

        let excerpt = extract_exploit_trigger_excerpt(output).expect("excerpt");
        assert!(excerpt.contains("$ ./target_cmd --evil input.bin"));
        assert!(excerpt.contains("INPUT:"));
    }

    #[test]
    fn exploit_scenario_prefers_idor_input_kind() {
        let bug = SecurityReviewBug {
            summary_id: 1,
            risk_rank: Some(1),
            risk_score: Some(0.0),
            title: "IDOR in get_user".to_string(),
            severity: "High".to_string(),
            impact: "High - Data exposure.".to_string(),
            likelihood: "High - Remote.".to_string(),
            recommendation: "x".to_string(),
            file: "x.rs#L1-L2".to_string(),
            blame: None,
            risk_reason: None,
            verification_types: vec!["network_api".to_string()],
            vulnerability_tag: Some("idor".to_string()),
            validation: BugValidationState::default(),
            assignee_github: None,
        };

        let kind = infer_exploit_input_kind(&bug, "", Some("$ curl http://example"));
        assert_eq!(kind, "another user's identifier in an API request");
    }

    #[test]
    fn exploit_scenario_includes_poc_or_trigger_excerpt() {
        let validation = BugValidationState {
            status: BugValidationStatus::Passed,
            ..BugValidationState::default()
        };
        let bug = SecurityReviewBug {
            summary_id: 1,
            risk_rank: Some(1),
            risk_score: Some(0.0),
            title: "Heap overflow".to_string(),
            severity: "High".to_string(),
            impact: "High - Crash.".to_string(),
            likelihood: "High - Remote.".to_string(),
            recommendation: "x".to_string(),
            file: "x.rs#L1-L2".to_string(),
            blame: None,
            risk_reason: None,
            verification_types: vec!["crash_poc_release".to_string()],
            vulnerability_tag: None,
            validation,
            assignee_github: None,
        };

        let scenario =
            build_exploit_scenario_block(&bug, "", None, Some("/tmp/poc.py")).expect("scenario");
        assert!(scenario.contains("#### Exploit scenario"));
        assert!(scenario.contains("PoC artifact"));
    }
}

#[cfg(test)]
mod severity_matrix_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_bug_summary(severity: &str, impact: &str, likelihood: &str) -> BugSummary {
        BugSummary {
            id: 1,
            title: "Example".to_string(),
            file: "x.rs#L1-L2".to_string(),
            severity: severity.to_string(),
            impact: impact.to_string(),
            likelihood: likelihood.to_string(),
            recommendation: "x".to_string(),
            blame: None,
            risk_score: None,
            risk_rank: None,
            risk_reason: None,
            verification_types: Vec::new(),
            vulnerability_tag: None,
            validation: BugValidationState::default(),
            source_path: PathBuf::new(),
            markdown: String::new(),
            author_github: None,
        }
    }

    #[test]
    fn upgrades_medium_when_high_impact_medium_likelihood() {
        let mut summary = make_bug_summary(
            "Medium",
            "High - Remote code execution in daemon context.",
            "Medium - Attacker-controlled input via untrusted ciphertext.",
        );
        let update = apply_severity_matrix(&mut summary).expect("matrix update");
        assert_eq!(update.previous, "Medium");
        assert_eq!(update.product, 6);
        assert_eq!(summary.severity, "High");
    }

    #[test]
    fn downgrades_high_when_high_impact_low_likelihood() {
        let mut summary = make_bug_summary(
            "High",
            "High - Arbitrary code execution in daemon context.",
            "Low - Requires local privileged attacker and unusual configuration.",
        );
        let update = apply_severity_matrix(&mut summary).expect("matrix update");
        assert_eq!(update.previous, "High");
        assert_eq!(update.product, 3);
        assert_eq!(summary.severity, "Medium");
    }

    #[test]
    fn does_not_override_ignore_severity() {
        let mut summary = make_bug_summary(
            "Ignore",
            "High - Remote code execution in daemon context.",
            "High - Unauthenticated remote reachability.",
        );
        let update = apply_severity_matrix(&mut summary);
        assert!(update.is_none());
        assert_eq!(summary.severity, "Ignore");
    }
}

#[allow(clippy::too_many_arguments)]
async fn filter_spec_directories(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
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
        model,
        reasoning_effort,
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
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
) -> Result<(String, Vec<String>), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();

    let mut spec_config = config.clone();
    spec_config.model = Some(model.to_string());
    spec_config.model_reasoning_effort =
        normalize_reasoning_effort_for_model(model, reasoning_effort);
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
        .disable(Feature::ViewImageTool);
    spec_config.mcp_servers.clear();

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
    reasoning_effort: Option<ReasoningEffort>,
) -> Result<BugAgentOutcome, SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();

    let mut bug_config = config.clone();
    bug_config.model = Some(model.to_string());
    bug_config.model_reasoning_effort =
        normalize_reasoning_effort_for_model(model, reasoning_effort);
    bug_config.model_provider = provider.clone();
    bug_config.base_instructions = Some(BUGS_SYSTEM_PROMPT.to_string());
    bug_config.user_instructions = None;
    bug_config.developer_instructions = None;
    bug_config.compact_prompt = None;
    bug_config.cwd = repo_root.to_path_buf();
    bug_config
        .features
        .disable(Feature::ApplyPatchFreeform)
        .disable(Feature::ViewImageTool);
    bug_config.mcp_servers.clear();

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
    spec_model: &str,
    spec_reasoning_effort: Option<ReasoningEffort>,
    writing_model: &str,
    writing_reasoning_effort: Option<ReasoningEffort>,
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
        spec_model,
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
        spec_model,
        spec_reasoning_effort,
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
        let outcome = polish_markdown_block(
            client,
            provider,
            auth,
            writing_model,
            writing_reasoning_effort,
            metrics.clone(),
            &sanitized,
            None,
        )
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

    sanitized = strip_dev_setup_sections(&sanitized);

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
    spec_model: &str,
    spec_reasoning_effort: Option<ReasoningEffort>,
    writing_model: &str,
    writing_reasoning_effort: Option<ReasoningEffort>,
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
                spec_model,
                spec_reasoning_effort,
                writing_model,
                writing_reasoning_effort,
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
    spec_model: &str,
    spec_reasoning_effort: Option<ReasoningEffort>,
    writing_model: &str,
    writing_reasoning_effort: Option<ReasoningEffort>,
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
        spec_model,
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
        spec_model,
        spec_reasoning_effort,
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
        let outcome = polish_markdown_block(
            client,
            &provider,
            &auth,
            writing_model,
            writing_reasoning_effort,
            metrics.clone(),
            &sanitized,
            None,
        )
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

    sanitized = strip_dev_setup_sections(&sanitized);

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
    reasoning_effort: Option<ReasoningEffort>,
    writing_model: &str,
    writing_reasoning_effort: Option<ReasoningEffort>,
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
    let reasoning_label = reasoning_effort_label(normalize_reasoning_effort_for_model(
        model,
        reasoning_effort,
    ));
    let start_message = format!(
        "Generating threat model from {} specification section(s) (model: {model}, reasoning: {reasoning_label}).",
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
        reasoning_effort,
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
            reasoning_effort,
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
        let writing_reasoning_label = reasoning_effort_label(normalize_reasoning_effort_for_model(
            writing_model,
            writing_reasoning_effort,
        ));
        let polish_message = format!(
            "Polishing threat model markdown formatting (model: {writing_model}, reasoning: {writing_reasoning_label})."
        );
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(polish_message.clone()));
        }
        logs.push(polish_message);
        let outcome = match polish_markdown_block(
            client,
            provider,
            auth,
            writing_model,
            writing_reasoning_effort,
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
    sanitized_response = nest_threat_model_subsections(sanitized_response);

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
    reasoning_effort: Option<ReasoningEffort>,
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
        "Condense the following specification and threat model into a concise context for finding security bugs. Keep architecture, data flows, authn/z, controls, data sensitivity, and notable threats. Aim for at most {ANALYSIS_CONTEXT_MAX_CHARS} characters. Return short paragraphs or bullets; no tables; do not include lists of file paths.\n\n{combined}"
    );

    let response = call_model(
        client,
        provider,
        auth,
        model,
        reasoning_effort,
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

struct ValidationTargetPrepOutcome {
    logs: Vec<String>,
    testing_md_additions: Vec<String>,
    success: bool,
}

fn prep_exec_succeeded(
    exec_commands: &[ValidationPrepExecCommand],
    expected: Option<&str>,
) -> bool {
    let Some(expected) = expected.map(str::trim).filter(|s| !s.is_empty()) else {
        return false;
    };
    exec_commands
        .iter()
        .filter(|cmd| cmd.exit_code == 0)
        .any(|cmd| cmd.command.contains(expected))
}

async fn prepare_validation_targets(
    config: &Config,
    provider: &ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
    repo_root: &Path,
    output_root: &Path,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
) -> ValidationTargetPrepOutcome {
    let mut logs: Vec<String> = Vec::new();
    let mut additions: Vec<String> = Vec::new();

    let (has_cargo, has_package_json, _, _, _, _, _) = guess_testing_defaults(repo_root);
    let has_go = repo_root.join("go.mod").exists();

    let compose_candidates = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ];
    let compose_paths: Vec<PathBuf> = compose_candidates
        .iter()
        .map(|name| repo_root.join(name))
        .filter(|path| path.exists())
        .collect();
    let has_dockerfile = repo_root.join("Dockerfile").exists();

    let specs_root = output_root.join("specs");
    let testing_path = specs_root.join("TESTING.md");
    let testing_md = tokio_fs::read_to_string(&testing_path)
        .await
        .unwrap_or_default();
    let testing_md_context = trim_prompt_context(&testing_md, VALIDATION_TESTING_CONTEXT_MAX_CHARS);

    let compose_files = if compose_paths.is_empty() {
        "none".to_string()
    } else {
        compose_paths
            .iter()
            .filter_map(|p| p.file_name().and_then(|name| name.to_str()))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let prompt = VALIDATION_TARGET_PREP_PROMPT_TEMPLATE
        .replace("{repo_root}", &repo_root.display().to_string())
        .replace("{output_root}", &output_root.display().to_string())
        .replace("{testing_md}", &testing_md_context)
        .replace("{has_cargo}", &has_cargo.to_string())
        .replace("{has_go}", &has_go.to_string())
        .replace("{has_package_json}", &has_package_json.to_string())
        .replace("{has_dockerfile}", &has_dockerfile.to_string())
        .replace("{compose_files}", &compose_files);

    let mut progress = ValidationTargetPrepProgress {
        version: 3,
        repo_root: repo_root.display().to_string(),
        summary: None,
        local_build_ok: false,
        local_run_ok: false,
        docker_build_ok: false,
        docker_run_ok: false,
        local_entrypoint: None,
        local_build_command: None,
        local_smoke_command: None,
        dockerfile_path: None,
        docker_image_tag: None,
        docker_build_command: None,
        docker_smoke_command: None,
    };

    let mut success = false;
    match run_validation_target_prep_agent(
        config,
        provider,
        auth_manager,
        repo_root,
        prompt,
        progress_sender.clone(),
        log_sink.clone(),
        metrics.clone(),
        model,
        reasoning_effort,
    )
    .await
    {
        Ok(output) => {
            logs.extend(output.logs);
            match parse_validation_target_prep_output(output.text.as_str()) {
                Some(parsed) => {
                    if let Some(summary) = parsed
                        .summary
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        progress.summary = Some(summary.to_string());
                        push_progress_log(
                            &progress_sender,
                            &log_sink,
                            &mut logs,
                            format!("Validation target prep: {summary}"),
                        );
                    }
                    if let Some(addition) = parsed
                        .testing_md_additions
                        .as_deref()
                        .map(str::trim_end)
                        .filter(|s| !s.trim().is_empty())
                    {
                        additions.push(addition.to_string());
                    }

                    progress.local_entrypoint = parsed
                        .local_entrypoint
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    progress.local_build_command = parsed
                        .local_build_command
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    progress.local_smoke_command = parsed
                        .local_smoke_command
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    progress.dockerfile_path = parsed
                        .dockerfile_path
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    progress.docker_image_tag = parsed
                        .docker_image_tag
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    progress.docker_build_command = parsed
                        .docker_build_command
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    progress.docker_smoke_command = parsed
                        .docker_smoke_command
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);

                    progress.local_build_ok = parsed.local_build_ok
                        && prep_exec_succeeded(
                            &output.exec_commands,
                            progress.local_build_command.as_deref(),
                        );
                    progress.local_run_ok = parsed.local_run_ok
                        && prep_exec_succeeded(
                            &output.exec_commands,
                            progress.local_smoke_command.as_deref(),
                        );
                    progress.docker_build_ok = parsed.docker_build_ok
                        && prep_exec_succeeded(
                            &output.exec_commands,
                            progress.docker_build_command.as_deref(),
                        );
                    progress.docker_run_ok = parsed.docker_run_ok
                        && prep_exec_succeeded(
                            &output.exec_commands,
                            progress.docker_smoke_command.as_deref(),
                        );

                    if parsed.local_build_ok && !progress.local_build_ok {
                        push_progress_log(
                            &progress_sender,
                            &log_sink,
                            &mut logs,
                            "Validation target prep: local_build_ok was true, but no successful local_build_command was observed; treating local build as not ready."
                                .to_string(),
                        );
                    }
                    if parsed.local_run_ok && !progress.local_run_ok {
                        push_progress_log(
                            &progress_sender,
                            &log_sink,
                            &mut logs,
                            "Validation target prep: local_run_ok was true, but no successful local_smoke_command was observed; treating local run as not ready."
                                .to_string(),
                        );
                    }
                    if parsed.docker_build_ok && !progress.docker_build_ok {
                        push_progress_log(
                            &progress_sender,
                            &log_sink,
                            &mut logs,
                            "Validation target prep: docker_build_ok was true, but no successful docker_build_command was observed; treating docker build as not ready."
                                .to_string(),
                        );
                    }
                    if parsed.docker_run_ok && !progress.docker_run_ok {
                        push_progress_log(
                            &progress_sender,
                            &log_sink,
                            &mut logs,
                            "Validation target prep: docker_run_ok was true, but no successful docker_smoke_command was observed; treating docker run as not ready."
                                .to_string(),
                        );
                    }

                    success = progress.local_build_ok && progress.local_run_ok;
                }
                None => {
                    push_progress_log(
                        &progress_sender,
                        &log_sink,
                        &mut logs,
                        "Validation target prep agent returned unparseable output; falling back to generic guidance."
                            .to_string(),
                    );
                }
            }
        }
        Err(err) => {
            logs.extend(err.logs);
            push_progress_log(
                &progress_sender,
                &log_sink,
                &mut logs,
                format!("Validation target prep agent failed: {}", err.message),
            );
        }
    }

    push_progress_log(
        &progress_sender,
        &log_sink,
        &mut logs,
        format!(
            "Validation target prep status: local_build_ok={}, local_run_ok={}, docker_build_ok={}, docker_run_ok={}",
            progress.local_build_ok,
            progress.local_run_ok,
            progress.docker_build_ok,
            progress.docker_run_ok
        ),
    );

    persist_validation_target_prep_progress(
        output_root,
        &progress,
        &progress_sender,
        &log_sink,
        &mut logs,
    )
    .await;

    ValidationTargetPrepOutcome {
        logs,
        testing_md_additions: additions,
        success,
    }
}

pub(crate) async fn rerun_prepare_validation_targets(
    config: &Config,
    provider: &ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    repo_root: &Path,
    output_root: &Path,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
) -> SecurityReviewRerunResult {
    let provider = provider_with_beta_features(provider, config);

    let mut model = config
        .security_review_models
        .validation
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();
    if model.is_empty() {
        model = DEFAULT_VALIDATION_MODEL.to_string();
    }

    let reasoning_effort = config
        .security_review_reasoning_efforts
        .validation
        .or(config.model_reasoning_effort)
        .or(Some(ReasoningEffort::XHigh));

    let metrics = Arc::new(ReviewMetrics::default());
    let outcome = prepare_validation_targets(
        config,
        &provider,
        auth_manager,
        model.as_str(),
        reasoning_effort,
        repo_root,
        output_root,
        progress_sender.clone(),
        log_sink,
        metrics,
    )
    .await;

    let specs_root = output_root.join("specs");
    let testing_path = specs_root.join("TESTING.md");

    let mut logs = outcome.logs;
    if !outcome.testing_md_additions.is_empty() {
        apply_validation_testing_md_additions(
            &testing_path,
            repo_root,
            &outcome.testing_md_additions,
            &progress_sender,
            &mut logs,
        )
        .await;
    }

    SecurityReviewRerunResult {
        target: SecurityReviewRerunTarget::PrepareValidationTargets,
        repo_root: repo_root.to_path_buf(),
        output_root: output_root.to_path_buf(),
        testing_md_path: testing_path,
        success: outcome.success,
        logs,
    }
}

async fn persist_validation_target_prep_progress(
    output_root: &Path,
    progress: &ValidationTargetPrepProgress,
    progress_sender: &Option<AppEventSender>,
    log_sink: &Option<Arc<SecurityReviewLogSink>>,
    logs: &mut Vec<String>,
) {
    let progress_path = validation_target_prep_progress_path(output_root);
    let bytes = match serde_json::to_vec_pretty(&progress) {
        Ok(bytes) => bytes,
        Err(err) => {
            push_progress_log(
                progress_sender,
                log_sink,
                logs,
                format!(
                    "Validation target prep: failed to serialize progress file {}: {err}",
                    progress_path.display()
                ),
            );
            return;
        }
    };

    if let Some(parent) = progress_path.parent()
        && let Err(err) = tokio_fs::create_dir_all(parent).await
    {
        push_progress_log(
            progress_sender,
            log_sink,
            logs,
            format!(
                "Validation target prep: failed to create progress directory {}: {err}",
                parent.display()
            ),
        );
        return;
    }

    let file_name = progress_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("validation_target_prep.json");
    let tmp_path = progress_path.with_file_name(format!("{file_name}.tmp"));
    let _ = tokio_fs::remove_file(&tmp_path).await;

    if let Err(err) = tokio_fs::write(&tmp_path, bytes).await {
        push_progress_log(
            progress_sender,
            log_sink,
            logs,
            format!(
                "Validation target prep: failed to write progress file {}: {err}",
                tmp_path.display()
            ),
        );
        return;
    }

    if let Err(err) = tokio_fs::rename(&tmp_path, &progress_path).await {
        let _ = tokio_fs::remove_file(&progress_path).await;
        if let Err(second_err) = tokio_fs::rename(&tmp_path, &progress_path).await {
            push_progress_log(
                progress_sender,
                log_sink,
                logs,
                format!(
                    "Validation target prep: failed to replace progress file {}: {err}; retry failed: {second_err}",
                    progress_path.display()
                ),
            );
            return;
        }
    }

    push_progress_log(
        progress_sender,
        log_sink,
        logs,
        format!(
            "Validation target prep: wrote progress marker at {}.",
            progress_path.display()
        ),
    );
}

fn validation_testing_md_dedupe_key(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed
        .trim_start_matches(['-', '*', '•'])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn merge_validation_testing_md(existing: &str, additions: &[String]) -> Option<String> {
    let mut addition_lines: Vec<String> = Vec::new();
    for addition in additions {
        for line in addition.lines() {
            let trimmed_end = line.trim_end();
            if trimmed_end.trim().is_empty() {
                continue;
            }
            if trimmed_end.trim_start().starts_with('#') {
                continue;
            }
            addition_lines.push(trimmed_end.to_string());
        }
    }
    if addition_lines.is_empty() {
        return None;
    }

    let mut file_lines: Vec<String> = existing.lines().map(str::to_string).collect();
    let header_index = file_lines
        .iter()
        .position(|line| line.trim() == VALIDATION_TESTING_SECTION_HEADER);

    let mut updated = false;

    match header_index {
        None => {
            if !file_lines.is_empty() && !file_lines.last().is_some_and(|l| l.trim().is_empty()) {
                file_lines.push(String::new());
            }
            file_lines.push(VALIDATION_TESTING_SECTION_HEADER.to_string());
            file_lines.push(VALIDATION_TESTING_SECTION_INTRO.to_string());
            file_lines.push(String::new());
            file_lines.extend(addition_lines);
            updated = true;
        }
        Some(header_index) => {
            let mut section_end = file_lines
                .iter()
                .enumerate()
                .skip(header_index + 1)
                .find(|(_, line)| line.trim_start().starts_with("## "))
                .map(|(index, _)| index)
                .unwrap_or(file_lines.len());

            let has_intro = file_lines
                .iter()
                .take(section_end)
                .skip(header_index + 1)
                .any(|line| line.trim() == VALIDATION_TESTING_SECTION_INTRO);
            if !has_intro {
                file_lines.insert(
                    header_index + 1,
                    VALIDATION_TESTING_SECTION_INTRO.to_string(),
                );
                file_lines.insert(header_index + 2, String::new());
                section_end = section_end.saturating_add(2);
                updated = true;
            }

            let mut seen: HashSet<String> = file_lines
                .iter()
                .take(section_end)
                .skip(header_index + 1)
                .filter_map(|line| validation_testing_md_dedupe_key(line))
                .collect();

            let mut filtered_additions: Vec<String> = Vec::new();
            for line in addition_lines {
                let Some(key) = validation_testing_md_dedupe_key(&line) else {
                    continue;
                };
                if seen.insert(key) {
                    filtered_additions.push(line);
                }
            }

            if !filtered_additions.is_empty() {
                let mut insert_at = section_end;
                while insert_at > header_index + 1
                    && file_lines
                        .get(insert_at.saturating_sub(1))
                        .is_some_and(|line| line.trim().is_empty())
                {
                    insert_at = insert_at.saturating_sub(1);
                }

                let mut insertion: Vec<String> = Vec::new();
                if insert_at > 0
                    && file_lines
                        .get(insert_at.saturating_sub(1))
                        .is_some_and(|line| !line.trim().is_empty())
                {
                    insertion.push(String::new());
                }
                insertion.extend(filtered_additions);

                file_lines.splice(insert_at..insert_at, insertion);
                updated = true;
            }
        }
    }

    if !updated {
        return None;
    }

    let mut out = file_lines.join("\n");
    out.push('\n');
    Some(out)
}

fn merge_validation_target_md(existing: &str, lines: &[String]) -> Option<String> {
    let mut section_lines: Vec<String> = lines
        .iter()
        .map(|line| line.trim_end())
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect();
    if section_lines.is_empty() {
        return None;
    }

    section_lines.insert(0, String::new());
    section_lines.insert(0, VALIDATION_TARGET_SECTION_INTRO.to_string());
    section_lines.insert(0, VALIDATION_TARGET_SECTION_HEADER.to_string());

    let mut file_lines: Vec<String> = existing.lines().map(str::to_string).collect();
    let header_index = file_lines
        .iter()
        .position(|line| line.trim() == VALIDATION_TARGET_SECTION_HEADER);

    match header_index {
        None => {
            if !file_lines.is_empty() && !file_lines.last().is_some_and(|l| l.trim().is_empty()) {
                file_lines.push(String::new());
            }
            file_lines.extend(section_lines);
        }
        Some(_) => return None,
    }

    let mut out = file_lines.join("\n");
    out.push('\n');

    let mut existing_normalized = existing.to_string();
    if !existing_normalized.ends_with('\n') {
        existing_normalized.push('\n');
    }
    if out == existing_normalized {
        None
    } else {
        Some(out)
    }
}

async fn apply_validation_testing_md_additions(
    testing_path: &Path,
    repo_root: &Path,
    additions: &[String],
    progress_sender: &Option<AppEventSender>,
    logs: &mut Vec<String>,
) {
    let existing = tokio_fs::read_to_string(testing_path)
        .await
        .unwrap_or_default();
    let Some(contents) = merge_validation_testing_md(&existing, additions) else {
        return;
    };

    if let Some(parent) = testing_path.parent() {
        let _ = tokio_fs::create_dir_all(parent).await;
    }

    match tokio_fs::write(testing_path, contents.as_bytes()).await {
        Ok(_) => {
            push_progress_log(
                progress_sender,
                &None,
                logs,
                format!(
                    "Updated shared testing instructions at {}.",
                    display_path_for(testing_path, repo_root)
                ),
            );
        }
        Err(err) => {
            push_progress_log(
                progress_sender,
                &None,
                logs,
                format!(
                    "Failed to update shared testing instructions at {}: {err}",
                    display_path_for(testing_path, repo_root)
                ),
            );
        }
    }
}

async fn apply_validation_target_md_section(
    testing_path: &Path,
    repo_root: &Path,
    lines: &[String],
    progress_sender: &Option<AppEventSender>,
    logs: &mut Vec<String>,
) {
    let existing = tokio_fs::read_to_string(testing_path)
        .await
        .unwrap_or_default();
    let Some(contents) = merge_validation_target_md(&existing, lines) else {
        return;
    };

    if let Some(parent) = testing_path.parent() {
        let _ = tokio_fs::create_dir_all(parent).await;
    }

    match tokio_fs::write(testing_path, contents.as_bytes()).await {
        Ok(_) => {
            push_progress_log(
                progress_sender,
                &None,
                logs,
                format!(
                    "Updated validation target notes at {}.",
                    display_path_for(testing_path, repo_root)
                ),
            );
        }
        Err(err) => {
            push_progress_log(
                progress_sender,
                &None,
                logs,
                format!(
                    "Failed to update validation target notes at {}: {err}",
                    display_path_for(testing_path, repo_root)
                ),
            );
        }
    }
}

fn build_web_validation_target_section_lines(
    repo_root: &Path,
    base_url: &Url,
    provided_creds_path: Option<&Path>,
    provided_headers: &[(String, String)],
    generated_creds_path: &Path,
    generated_headers: &[(String, String)],
) -> Vec<String> {
    let provided_header_names = if provided_headers.is_empty() {
        "(none)".to_string()
    } else {
        provided_headers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let generated_header_names = if generated_headers.is_empty() {
        "(none)".to_string()
    } else {
        generated_headers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("- target_url: {}", base_url.as_str()));
    match provided_creds_path {
        Some(path) => lines.push(format!(
            "- provided_creds_file: {}",
            display_path_for(path, repo_root)
        )),
        None => lines.push("- provided_creds_file: (none provided)".to_string()),
    }
    lines.push(format!("- provided_headers: {provided_header_names}"));
    lines.push(format!(
        "- generated_creds_file: {}",
        display_path_for(generated_creds_path, repo_root)
    ));
    lines.push(format!("- generated_headers: {generated_header_names}"));
    lines.push("- notes: do not commit credential files or tokens to source control".to_string());
    lines
}

#[allow(clippy::too_many_arguments)]
async fn combine_spec_markdown(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
    writing_model: &str,
    writing_reasoning_effort: Option<ReasoningEffort>,
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
            model,
            reasoning_effort,
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

    let writing_reasoning_label = reasoning_effort_label(normalize_reasoning_effort_for_model(
        writing_model,
        writing_reasoning_effort,
    ));
    let polish_message = format!(
        "Polishing combined specification markdown formatting (model: {writing_model}, reasoning: {writing_reasoning_label})."
    );
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(polish_message.clone()));
    }
    logs.push(polish_message);

    let fix_prompt = build_fix_markdown_prompt(&sanitized, Some(SPEC_COMBINED_MARKDOWN_TEMPLATE));
    let polished_response = match call_model(
        client,
        provider,
        auth,
        writing_model,
        writing_reasoning_effort,
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
    let polished = strip_dev_setup_sections(&polished);

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
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
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
        model,
        reasoning_effort,
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
    #[serde(default)]
    error_message: Option<String>,
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

#[allow(clippy::too_many_arguments)]
async fn polish_markdown_block(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
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
        model,
        reasoning_effort,
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
    reasoning_effort: Option<ReasoningEffort>,
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
    let mut failed_files: Vec<(String, String)> = Vec::new();
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
        if let Some(message) = entry.error_message.as_ref() {
            failed_files.push((entry.path_display.clone(), message.clone()));
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
                    reasoning_effort,
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

    while let Some(file_result) = in_flight.next().await {
        let FileBugResult {
            index,
            path_display,
            duration,
            logs,
            bug_section,
            error_message,
            snippet,
            findings_count: file_findings_count,
        } = file_result;

        aggregated_logs.extend(logs);
        findings_count = findings_count.saturating_add(file_findings_count);
        if let Some(message) = error_message.as_ref() {
            failed_files.push((path_display.clone(), message.clone()));
        }
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
            error_message,
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
                reasoning_effort,
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

    let successful_files = completed_files.saturating_sub(failed_files.len());
    aggregated_logs.push(format!(
        "Bug analysis summary: {total_files} file(s) total; {successful_files} succeeded; {} failed.",
        failed_files.len()
    ));
    if !failed_files.is_empty() {
        let failures = failed_files.len();
        aggregated_logs.push(format!(
            "Bug agent failures: {failures} file(s) encountered errors and were skipped."
        ));
        let sample = failed_files
            .iter()
            .take(10)
            .map(|(path, message)| format!("{path}: {}", truncate_text(message, 240)))
            .collect::<Vec<_>>()
            .join("; ");
        aggregated_logs.push(format!("Bug agent failure samples (up to 10): {sample}"));

        if let Some(path) = log_sink.as_ref().and_then(|sink| sink.path()) {
            aggregated_logs.push(format!(
                "Security review log written to {}.",
                path.display()
            ));
        }
        aggregated_logs.push(format!(
            "Bug analysis progress saved to {}.",
            display_path_for(&progress_path, repo_root)
        ));
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

    // Risk rerank runs after LLM dedupe in the full pipeline (see `run_security_review`),
    // so we avoid doing it here to prevent reranking duplicates.

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
    reasoning_effort: Option<ReasoningEffort>,
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
) -> FileBugResult {
    let started_at = Instant::now();
    let mut logs = Vec::new();
    let path_display = snippet.relative_path.display().to_string();
    let file_size = human_readable_bytes(snippet.bytes);
    let prefix = format!("{}/{}", index + 1, total_files);
    let start_message = format!("Analyzing file {prefix}: {path_display} ({file_size}).");
    push_progress_log(&progress_sender, &log_sink, &mut logs, start_message);

    let max_chars = bug_file_context_max_chars_for_model(model, config);
    let (base_context, context_logs) =
        build_single_file_context_for_bug_prompt(&snippet, max_chars);
    for line in context_logs {
        push_progress_log(&progress_sender, &log_sink, &mut logs, line);
    }
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
        auth_manager.clone(),
        repo_root,
        prompt_data.prompt.clone(),
        progress_sender.clone(),
        log_sink.clone(),
        metrics.clone(),
        model,
        reasoning_effort,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(agent_failure) => {
            logs.extend(agent_failure.logs);
            let failure_message = format!(
                "Bug agent loop failed for {path_display}: {}",
                agent_failure.message
            );

            let lower = agent_failure.message.to_ascii_lowercase();
            let is_context_window_error =
                lower.contains("context window") || lower.contains("ran out of room");
            if is_context_window_error {
                let retry_notice = format!(
                    "Bug agent hit context window limits for {path_display}; retrying with a compacted prompt."
                );
                push_progress_log(&progress_sender, &log_sink, &mut logs, retry_notice);

                let compact_repo_summary = format!(
                    "Included file:\n- {} ({file_size})\n",
                    snippet.relative_path.display()
                );
                let (retry_context, retry_logs) = build_single_file_context_for_bug_retry(&snippet);
                for line in retry_logs {
                    push_progress_log(&progress_sender, &log_sink, &mut logs, line);
                }
                let retry_prompt_data = build_bugs_user_prompt(
                    compact_repo_summary.as_str(),
                    None,
                    &retry_context,
                    scope_prompt,
                );
                for line in &retry_prompt_data.logs {
                    push_progress_log(&progress_sender, &log_sink, &mut logs, line.clone());
                }

                match run_bug_agent(
                    config,
                    provider,
                    auth_manager,
                    repo_root,
                    retry_prompt_data.prompt,
                    progress_sender.clone(),
                    log_sink.clone(),
                    metrics,
                    model,
                    reasoning_effort,
                )
                .await
                {
                    Ok(outcome) => outcome,
                    Err(retry_failure) => {
                        logs.extend(retry_failure.logs);
                        let message = format!(
                            "Bug agent loop failed for {path_display} after compaction retry: {}",
                            retry_failure.message
                        );
                        push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                        return FileBugResult {
                            index,
                            path_display,
                            duration: started_at.elapsed(),
                            logs,
                            bug_section: None,
                            error_message: Some(message),
                            snippet: None,
                            findings_count: 0,
                        };
                    }
                }
            } else {
                push_progress_log(
                    &progress_sender,
                    &log_sink,
                    &mut logs,
                    failure_message.clone(),
                );
                return FileBugResult {
                    index,
                    path_display,
                    duration: started_at.elapsed(),
                    logs,
                    bug_section: None,
                    error_message: Some(failure_message),
                    snippet: None,
                    findings_count: 0,
                };
            }
        }
    };

    logs.extend(outcome.logs);
    let trimmed = outcome.section.trim();
    if trimmed.is_empty() {
        let warn = format!(
            "Bug agent returned an empty response for {path_display}; treating as no findings."
        );
        push_progress_log(&progress_sender, &log_sink, &mut logs, warn);
        return FileBugResult {
            index,
            path_display,
            duration: started_at.elapsed(),
            logs,
            bug_section: None,
            error_message: None,
            snippet: None,
            findings_count: 0,
        };
    }
    if trimmed.eq_ignore_ascii_case("no bugs found") {
        let message = format!("No bugs found in {path_display}.");
        push_progress_log(&progress_sender, &log_sink, &mut logs, message);
        return FileBugResult {
            index,
            path_display,
            duration: started_at.elapsed(),
            logs,
            bug_section: None,
            error_message: None,
            snippet: None,
            findings_count: 0,
        };
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
    push_progress_log(&progress_sender, &log_sink, &mut logs, message);
    FileBugResult {
        index,
        path_display,
        duration: started_at.elapsed(),
        logs,
        bug_section: Some(outcome.section),
        error_message: None,
        snippet: Some(snippet),
        findings_count: file_findings,
    }
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

fn parse_taxonomy_vuln_tag(line: &str) -> Option<String> {
    let mut cursor = line.trim();

    if let Some(rest) = cursor.strip_prefix('-') {
        cursor = rest.trim_start();
    }
    cursor = cursor.trim_start_matches('*');

    let lower = cursor.to_ascii_lowercase();
    if !lower.starts_with("taxonomy") {
        return None;
    }

    let colon = cursor.find(':')?;
    let mut value = cursor[colon.saturating_add(1)..].trim();
    value = value.trim_start_matches('*').trim();
    value = value.trim_matches('`').trim();
    if value.is_empty() {
        return None;
    }

    let json = if value.starts_with("{{") && value.ends_with("}}") && value.len() >= 4 {
        value.get(1..value.len().saturating_sub(1)).unwrap_or(value)
    } else {
        value
    };

    let taxonomy = serde_json::from_str::<Value>(json).ok()?;
    let tag = taxonomy.get("vuln_tag")?.as_str()?.trim();
    if tag.is_empty() {
        None
    } else {
        Some(tag.to_string())
    }
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
            let mut markdown = trimmed;
            let preferred_locations =
                preferred_bug_locations_for_reporting(markdown.as_str(), summary.file.as_str());
            if !preferred_locations.is_empty() {
                let joined = preferred_locations.join(", ");
                if !joined.is_empty() && joined != summary.file {
                    summary.file = joined.clone();
                    if let Some(updated) =
                        rewrite_bug_markdown_location(markdown.as_str(), joined.as_str())
                    {
                        markdown = updated;
                    }
                }
            }
            summary.markdown = markdown.clone();
            details.push(BugDetail {
                summary_id: summary.id,
                original_markdown: markdown,
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
            } else if summary.vulnerability_tag.is_none()
                && let Some(tag) = parse_taxonomy_vuln_tag(trimmed)
            {
                summary.vulnerability_tag = Some(tag);
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

#[cfg(test)]
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

fn extract_file_locations_from_markdown(text: &str) -> Vec<String> {
    static LOCATION_RE: OnceLock<Regex> = OnceLock::new();
    let re = LOCATION_RE.get_or_init(|| {
        Regex::new(r"(?P<loc>[^\s,;()\[\]`<>]+#L\d+(?:-L\d+)?)")
            .unwrap_or_else(|error| panic!("failed to compile file location regex: {error}"))
    });

    let mut out: Vec<String> = Vec::new();
    for caps in re.captures_iter(text) {
        let Some(loc) = caps.name("loc").map(|m| m.as_str().trim()) else {
            continue;
        };
        if loc.is_empty() {
            continue;
        }
        if !out.iter().any(|existing| existing == loc) {
            out.push(loc.to_string());
        }
    }
    out
}

fn leading_whitespace_len(text: &str) -> usize {
    text.bytes()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count()
}

fn extract_bug_section_locations(markdown: &str, section: &str) -> Vec<String> {
    let mut in_section = false;
    let mut section_indent = 0usize;
    let header = format!("- **{section}:**");
    let mut out: Vec<String> = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if !in_section {
            if trimmed.starts_with(header.as_str()) {
                in_section = true;
                section_indent = leading_whitespace_len(line);
                for loc in extract_file_locations_from_markdown(trimmed) {
                    if !out.iter().any(|existing| existing == &loc) {
                        out.push(loc);
                    }
                }
            }
            continue;
        }

        let indent = leading_whitespace_len(line);
        if indent <= section_indent
            && trimmed.starts_with("- **")
            && !trimmed.starts_with(header.as_str())
        {
            break;
        }

        for loc in extract_file_locations_from_markdown(trimmed) {
            if !out.iter().any(|existing| existing == &loc) {
                out.push(loc);
            }
        }
    }

    out
}

fn extract_bug_sink_locations(markdown: &str) -> Vec<String> {
    let mut in_dataflow = false;
    let mut dataflow_indent = 0usize;
    let mut sink_block_indent: Option<usize> = None;
    let mut out: Vec<String> = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if !in_dataflow {
            if trimmed.starts_with("- **Dataflow:**") {
                in_dataflow = true;
                dataflow_indent = leading_whitespace_len(line);
            }
            continue;
        }

        let indent = leading_whitespace_len(line);
        if indent <= dataflow_indent
            && trimmed.starts_with("- **")
            && !trimmed.starts_with("- **Dataflow:**")
        {
            break;
        }

        let lowered = trimmed.to_ascii_lowercase();
        if lowered.contains("sink") {
            sink_block_indent = Some(indent);
        }

        let Some(sink_indent) = sink_block_indent else {
            continue;
        };

        if indent <= sink_indent && trimmed.starts_with('-') && !lowered.contains("sink") {
            sink_block_indent = None;
            continue;
        }

        for loc in extract_file_locations_from_markdown(trimmed) {
            if !out.iter().any(|existing| existing == &loc) {
                out.push(loc);
            }
        }
    }

    out
}

fn preferred_bug_locations_for_reporting(markdown: &str, file_field: &str) -> Vec<String> {
    let sinks = extract_bug_sink_locations(markdown);
    if !sinks.is_empty() {
        return sinks;
    }

    let description_locs = extract_bug_section_locations(markdown, "Description");
    if !description_locs.is_empty() {
        return description_locs;
    }

    let mut fallback = extract_file_locations_for_dedupe(file_field);
    if fallback.len() > 8 {
        fallback.truncate(8);
    }
    fallback
}

fn prune_bug_markdown_file_lines_for_reporting(markdown: &str) -> String {
    let mut file_field = None;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("- **File & Lines:**") {
            let value = rest.trim().trim_matches('`');
            if !value.is_empty() {
                file_field = Some(value.to_string());
            }
            break;
        }
    }

    let Some(file_field) = file_field else {
        return markdown.to_string();
    };

    let preferred = preferred_bug_locations_for_reporting(markdown, file_field.as_str());
    if preferred.is_empty() {
        return markdown.to_string();
    }

    let joined = preferred.join(", ");
    if joined.is_empty() || joined == file_field {
        return markdown.to_string();
    }

    rewrite_bug_markdown_location(markdown, joined.as_str()).unwrap_or_else(|| markdown.to_string())
}

fn extract_file_locations_for_dedupe(file_field: &str) -> Vec<String> {
    let trimmed = file_field.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    static LOCATION_RE: OnceLock<Regex> = OnceLock::new();
    let re = LOCATION_RE.get_or_init(|| {
        Regex::new(r"(?P<loc>[^\s,;()]+#L\d+(?:-L\d+)?)")
            .unwrap_or_else(|error| panic!("failed to compile file location regex: {error}"))
    });

    let mut out: Vec<String> = re
        .captures_iter(trimmed)
        .filter_map(|caps| caps.name("loc").map(|m| m.as_str().to_string()))
        .collect();

    if out.is_empty() {
        out = trimmed
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect();
    }

    out
}

fn normalize_vuln_tag_token(token: &str) -> Option<String> {
    let lower = token.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }

    let canonical = match lower.as_str() {
        "verification" | "verified" | "verifying" | "verify" => "verify",
        "validation" | "validated" | "validating" | "validate" => "verify",
        "checks" | "check" => "verify",
        "ssl" | "https" => "tls",
        "certificate" | "certificates" | "x509" => "cert",
        other => other,
    };

    Some(canonical.to_string())
}

#[cfg(test)]
fn vuln_tag_tokens_for_dedupe(tag: &str) -> HashSet<String> {
    let mut tokens: HashSet<String> = tag
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter_map(normalize_vuln_tag_token)
        .collect();

    if tokens.contains("hostname")
        && (tokens.contains("tls") || tokens.contains("cert"))
        && !tokens.contains("verify")
    {
        tokens.insert("verify".to_string());
    }

    tokens
}

#[cfg(test)]
fn is_generic_dedupe_token(token: &str) -> bool {
    matches!(
        token,
        "missing"
            | "not"
            | "no"
            | "without"
            | "lack"
            | "lacking"
            | "bypass"
            | "verify"
            | "bug"
            | "issue"
            | "vuln"
            | "vulnerability"
            | "buffer"
            | "overflow"
            | "oob"
            | "read"
            | "write"
    )
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

#[derive(Clone, Debug, Deserialize, Serialize)]
struct BugDedupeDecision {
    id: usize,
    canonical_id: usize,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LlmDedupePass1Cache {
    listing_hash: u64,
    listing_len: usize,
    decisions: Vec<BugDedupeDecision>,
}

struct BugLlmDedupeOutcome {
    summaries: Vec<BugSummary>,
    details: Vec<BugDetail>,
    removed: usize,
    filtered_low: usize,
    logs: Vec<String>,
}

fn llm_dedupe_extract_section_text(markdown: &str, section: &str) -> Option<String> {
    let mut in_section = false;
    let mut section_indent = 0usize;
    let header = format!("- **{section}:**");
    let mut out: Vec<String> = Vec::new();

    for line in markdown.lines() {
        let trimmed_start = line.trim_start();
        if !in_section {
            if trimmed_start.starts_with(header.as_str()) {
                in_section = true;
                section_indent = leading_whitespace_len(line);
                let after = trimmed_start[header.len()..].trim_start();
                if !after.is_empty() {
                    out.push(after.to_string());
                }
            }
            continue;
        }

        let indent = leading_whitespace_len(line);
        if indent <= section_indent
            && trimmed_start.starts_with("- **")
            && !trimmed_start.starts_with(header.as_str())
        {
            break;
        }

        if !trimmed_start.is_empty() {
            out.push(trimmed_start.to_string());
        }
    }

    let joined = out.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn llm_dedupe_sink_locations(summary: &BugSummary) -> Vec<String> {
    let mut locations = extract_bug_sink_locations(summary.markdown.as_str());
    if !locations.is_empty() {
        return locations;
    }

    locations = extract_bug_section_locations(summary.markdown.as_str(), "File & Lines");
    if locations.is_empty() {
        locations = extract_file_locations_for_dedupe(summary.file.as_str());
    }
    locations
}

fn llm_dedupe_file_locations(summary: &BugSummary) -> Vec<String> {
    let mut locations = extract_bug_section_locations(summary.markdown.as_str(), "File & Lines");
    if locations.is_empty() {
        locations = extract_file_locations_for_dedupe(summary.file.as_str());
    }
    locations
}

fn llm_dedupe_description_text(summary: &BugSummary) -> String {
    llm_dedupe_extract_section_text(summary.markdown.as_str(), "Description")
        .unwrap_or_else(|| summary.markdown.clone())
}

fn llm_dedupe_impact_text(summary: &BugSummary) -> String {
    if !summary.impact.trim().is_empty() {
        return summary.impact.clone();
    }

    llm_dedupe_extract_section_text(summary.markdown.as_str(), "Impact").unwrap_or_default()
}

fn llm_dedupe_root_cause_text(summary: &BugSummary) -> Option<String> {
    summary
        .vulnerability_tag
        .clone()
        .or_else(|| extract_vulnerability_tag_from_bug_markdown(summary.markdown.as_str()))
}

struct LlmDedupePromptChoice {
    listing: String,
    prompt: String,
    limits_log: String,
    fits: bool,
}

fn bug_dedupe_prompt_max_chars_for_model(model: &str) -> usize {
    let model = model_id_for_context_window(model);
    let Some(context_window) = infer_default_context_window_tokens(model) else {
        return BUG_DEDUP_PROMPT_MAX_CHARS;
    };
    if context_window < 200_000 {
        return BUG_DEDUP_PROMPT_MAX_CHARS;
    }

    // Reserve headroom for model output and other context.
    const DEDUPE_TOKEN_SHARE_NUM: i64 = 1;
    const DEDUPE_TOKEN_SHARE_DEN: i64 = 5; // 20%
    const CHARS_PER_TOKEN_ESTIMATE: i64 = 3;

    let budget_tokens = context_window
        .saturating_mul(DEDUPE_TOKEN_SHARE_NUM)
        .saturating_div(DEDUPE_TOKEN_SHARE_DEN)
        .max(1);
    let budget_chars = budget_tokens.saturating_mul(CHARS_PER_TOKEN_ESTIMATE);
    let min_chars = BUG_DEDUP_PROMPT_MAX_CHARS as i64;
    let max_chars = MAX_PROMPT_BYTES as i64;

    budget_chars.clamp(min_chars, max_chars) as usize
}

fn llm_dedupe_build_pass1_listing(
    ordered: &[&BugSummary],
    max_title: usize,
    max_files: usize,
) -> String {
    let max_files = max_files.max(1);
    ordered
        .iter()
        .map(|summary| {
            let title = truncate_text(&summary.title, max_title);
            let severity = truncate_text(summary.severity.as_str(), 32);
            let mut sinks = llm_dedupe_sink_locations(summary);
            if sinks.len() > max_files {
                let overflow = sinks.len().saturating_sub(max_files);
                sinks.truncate(max_files);
                sinks.push(format!("...(+{overflow})"));
            }
            let sink_locations = sinks.join(", ");

            json!({
                "id": summary.id,
                "title": title,
                "severity": severity,
                "sink_locations": sink_locations,
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn llm_dedupe_build_pass2_listing(
    ordered: &[&BugSummary],
    max_description: usize,
    max_impact: usize,
    max_root_cause: usize,
) -> String {
    ordered
        .iter()
        .map(|summary| {
            let description_source = llm_dedupe_description_text(summary);
            let description = truncate_text(description_source.as_str(), max_description);
            let impact_source = llm_dedupe_impact_text(summary);
            let impact = truncate_text(impact_source.as_str(), max_impact);
            let root_cause_source = llm_dedupe_root_cause_text(summary).unwrap_or_default();
            let root_cause = truncate_text(root_cause_source.as_str(), max_root_cause);

            json!({
                "id": summary.id,
                "description": description,
                "impact": impact,
                "root_cause": root_cause,
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn llm_dedupe_prompt_pass1(
    ordered: &[&BugSummary],
    max_prompt_chars: usize,
) -> LlmDedupePromptChoice {
    let mut fallback_listing = String::new();
    let mut fallback_prompt = String::new();
    let mut fallback_limits = (0_usize, 0_usize);
    for (max_title, max_files) in [(160_usize, 12_usize), (120, 10), (100, 8), (80, 6), (60, 4)] {
        let listing = llm_dedupe_build_pass1_listing(ordered, max_title, max_files);
        let prompt = BUG_DEDUP_PROMPT_TEMPLATE_PASS1.replace("{findings}", listing.as_str());
        if prompt.len() <= max_prompt_chars {
            return LlmDedupePromptChoice {
                listing,
                prompt,
                limits_log: format!("limits: title {max_title} chars; sinks {max_files}."),
                fits: true,
            };
        }
        fallback_listing = listing;
        fallback_prompt = prompt;
        fallback_limits = (max_title, max_files);
    }

    let (max_title, max_files) = fallback_limits;
    LlmDedupePromptChoice {
        listing: fallback_listing,
        prompt: fallback_prompt,
        limits_log: format!("limits: title {max_title} chars; sinks {max_files}."),
        fits: false,
    }
}

fn llm_dedupe_prompt_pass2(
    ordered: &[&BugSummary],
    max_prompt_chars: usize,
) -> LlmDedupePromptChoice {
    let mut fallback_listing = String::new();
    let mut fallback_prompt = String::new();
    let mut fallback_limits = (0_usize, 0_usize, 0_usize);
    for (max_description, max_impact, max_root_cause) in [
        (420_usize, 240_usize, 80_usize),
        (280, 200, 80),
        (200, 160, 60),
        (160, 120, 60),
        (120, 80, 60),
        (80, 60, 40),
    ] {
        let listing =
            llm_dedupe_build_pass2_listing(ordered, max_description, max_impact, max_root_cause);
        let prompt = BUG_DEDUP_PROMPT_TEMPLATE_PASS2.replace("{findings}", listing.as_str());
        if prompt.len() <= max_prompt_chars {
            return LlmDedupePromptChoice {
                listing,
                prompt,
                limits_log: format!(
                    "limits: description {max_description} chars; impact {max_impact} chars; root_cause {max_root_cause} chars."
                ),
                fits: true,
            };
        }
        fallback_listing = listing;
        fallback_prompt = prompt;
        fallback_limits = (max_description, max_impact, max_root_cause);
    }

    let (max_description, max_impact, max_root_cause) = fallback_limits;
    LlmDedupePromptChoice {
        listing: fallback_listing,
        prompt: fallback_prompt,
        limits_log: format!(
            "limits: description {max_description} chars; impact {max_impact} chars; root_cause {max_root_cause} chars."
        ),
        fits: false,
    }
}

fn llm_dedupe_parse_decisions(text: &str) -> Vec<BugDedupeDecision> {
    let mut decisions: Vec<BugDedupeDecision> = Vec::new();
    for raw_line in text.lines() {
        let trimmed = raw_line.trim().trim_matches('`');
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<BugDedupeDecision>(trimmed) {
            decisions.push(entry);
        }
    }
    decisions
}

fn llm_dedupe_debug_prefix(label: &str) -> String {
    let session_id = security_review_session_id();
    format!("llm_dedupe_{session_id}_{label}")
}

fn llm_dedupe_listing_hash(listing: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    listing.hash(&mut hasher);
    hasher.finish()
}

async fn llm_dedupe_read_pass1_cache(
    debug_dir: Option<&Path>,
    listing: &str,
    logs: &mut Vec<String>,
) -> Option<Vec<BugDedupeDecision>> {
    let dir = debug_dir?;

    let path = dir.join(BUG_LLM_DEDUP_PASS1_CACHE_FILE);
    let bytes = match tokio_fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(_) => return None,
    };
    let cache: LlmDedupePass1Cache = match serde_json::from_slice(&bytes) {
        Ok(cache) => cache,
        Err(err) => {
            logs.push(format!(
                "LLM dedupe pass1 cache parse failed at {}: {err}",
                path.display()
            ));
            return None;
        }
    };

    let listing_hash = llm_dedupe_listing_hash(listing);
    if cache.listing_hash != listing_hash || cache.listing_len != listing.len() {
        logs.push(format!(
            "LLM dedupe pass1 cache mismatch at {}; ignoring.",
            path.display()
        ));
        return None;
    }
    if cache.decisions.is_empty() {
        return None;
    }

    logs.push(format!(
        "LLM dedupe pass1 reused cached decisions from {}.",
        path.display()
    ));
    Some(cache.decisions)
}

async fn llm_dedupe_write_pass1_cache(
    debug_dir: Option<&Path>,
    listing: &str,
    decisions: &[BugDedupeDecision],
    logs: &mut Vec<String>,
) {
    if decisions.is_empty() {
        return;
    }
    let Some(dir) = debug_dir else {
        return;
    };

    if let Err(err) = tokio_fs::create_dir_all(dir).await {
        logs.push(format!(
            "LLM dedupe pass1 cache: failed to create {}: {err}",
            dir.display()
        ));
        return;
    }

    let cache = LlmDedupePass1Cache {
        listing_hash: llm_dedupe_listing_hash(listing),
        listing_len: listing.len(),
        decisions: decisions.to_vec(),
    };
    let bytes = match serde_json::to_vec_pretty(&cache) {
        Ok(bytes) => bytes,
        Err(err) => {
            logs.push(format!(
                "LLM dedupe pass1 cache: failed to serialize: {err}"
            ));
            return;
        }
    };
    let path = dir.join(BUG_LLM_DEDUP_PASS1_CACHE_FILE);
    if let Err(err) = tokio_fs::write(&path, bytes).await {
        logs.push(format!(
            "LLM dedupe pass1 cache: failed to write {}: {err}",
            path.display()
        ));
    }
}

async fn llm_dedupe_write_debug_prompt(
    debug_dir: Option<&Path>,
    prefix: &str,
    listing: &str,
    prompt: &str,
    logs: &mut Vec<String>,
) {
    let Some(dir) = debug_dir else {
        return;
    };

    if let Err(err) = tokio_fs::create_dir_all(dir).await {
        logs.push(format!(
            "LLM dedupe debug: failed to create {}: {err}",
            dir.display()
        ));
        return;
    }

    let listing_path = dir.join(format!("{prefix}_findings.jsonl"));
    if let Err(err) = tokio_fs::write(&listing_path, listing).await {
        logs.push(format!(
            "LLM dedupe debug: failed to write {}: {err}",
            listing_path.display()
        ));
    }

    let prompt_path = dir.join(format!("{prefix}_prompt.txt"));
    if let Err(err) = tokio_fs::write(&prompt_path, prompt).await {
        logs.push(format!(
            "LLM dedupe debug: failed to write {}: {err}",
            prompt_path.display()
        ));
    }
}

async fn llm_dedupe_write_debug_response(
    debug_dir: Option<&Path>,
    prefix: &str,
    response_text: &str,
    logs: &mut Vec<String>,
) {
    let Some(dir) = debug_dir else {
        return;
    };

    let path = dir.join(format!("{prefix}_response.txt"));
    if let Err(err) = tokio_fs::write(&path, response_text).await {
        logs.push(format!(
            "LLM dedupe debug: failed to write {}: {err}",
            path.display()
        ));
    }
}

#[allow(clippy::too_many_arguments)]
async fn llm_dedupe_execute_prompt(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
    prompt: &str,
    debug_dir: Option<&Path>,
    debug_prefix: &str,
    log_prefix: &str,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
) -> (Vec<BugDedupeDecision>, Vec<String>) {
    let mut logs: Vec<String> = Vec::new();
    let response = call_model(
        client,
        provider,
        auth,
        model,
        reasoning_effort,
        BUG_DEDUP_SYSTEM_PROMPT,
        prompt,
        metrics,
        0.0,
    )
    .await;

    let response_output = match response {
        Ok(output) => output,
        Err(err) => {
            logs.push(format!("{log_prefix} model request failed: {err}"));
            return (Vec::new(), logs);
        }
    };

    let decisions = llm_dedupe_parse_decisions(response_output.text.as_str());
    llm_dedupe_write_debug_response(
        debug_dir,
        debug_prefix,
        response_output.text.as_str(),
        &mut logs,
    )
    .await;

    if let Some(reasoning) = response_output.reasoning.as_ref() {
        log_model_reasoning(reasoning, &progress_sender, &log_sink, &mut logs);
    } else {
        log_dedupe_decision_reasons(
            &decisions,
            log_prefix,
            &progress_sender,
            &log_sink,
            &mut logs,
        );
    }
    if decisions.is_empty() {
        logs.push(format!(
            "{log_prefix} returned no parseable JSON Lines; skipping."
        ));
    }

    (decisions, logs)
}

#[allow(clippy::too_many_arguments)]
async fn llm_dedupe_bug_summaries(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
    summaries: Vec<BugSummary>,
    details: Vec<BugDetail>,
    debug_dir: Option<PathBuf>,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
) -> BugLlmDedupeOutcome {
    let total_findings = summaries.len();
    if total_findings < 2 {
        return BugLlmDedupeOutcome {
            summaries,
            details,
            removed: 0,
            filtered_low: 0,
            logs: Vec::new(),
        };
    }

    let low_severity_findings = summaries
        .iter()
        .filter(|summary| severity_rank(&summary.severity) == 2)
        .count();
    let use_non_low_count = total_findings > BUG_LLM_DEDUP_LOW_SEVERITY_MAX_FINDINGS;
    let non_low_findings = total_findings.saturating_sub(low_severity_findings);
    let detail_message = if use_non_low_count {
        format!("{non_low_findings} finding(s)")
    } else {
        format!("{total_findings} finding(s)")
    };

    let progress_sender_for_heartbeat = progress_sender.clone();
    let progress_sender_for_dedupe = progress_sender.clone();
    let log_sink_for_dedupe = log_sink.clone();
    let outcome = await_with_heartbeat(
        progress_sender_for_heartbeat,
        "running LLM dedupe",
        Some(detail_message.as_str()),
        async move {
            Ok::<BugLlmDedupeOutcome, std::convert::Infallible>(
                llm_dedupe_bug_summaries_single_bucket(
                    client,
                    provider,
                    auth,
                    model,
                    reasoning_effort,
                    summaries,
                    details,
                    metrics,
                    debug_dir,
                    progress_sender_for_dedupe,
                    log_sink_for_dedupe,
                )
                .await,
            )
        },
    )
    .await;

    match outcome {
        Ok(outcome) => outcome,
        Err(err) => match err {},
    }
}

#[allow(clippy::too_many_arguments)]
async fn llm_dedupe_bug_summaries_single_bucket(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
    mut summaries: Vec<BugSummary>,
    mut details: Vec<BugDetail>,
    metrics: Arc<ReviewMetrics>,
    debug_dir: Option<PathBuf>,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
) -> BugLlmDedupeOutcome {
    let mut logs: Vec<String> = Vec::new();

    if summaries.len() < 2 {
        return BugLlmDedupeOutcome {
            summaries,
            details,
            removed: 0,
            filtered_low: 0,
            logs,
        };
    }

    #[derive(Clone)]
    struct UnionFind {
        parent: Vec<usize>,
        rank: Vec<u8>,
    }

    impl UnionFind {
        fn new(len: usize) -> Self {
            Self {
                parent: (0..len).collect(),
                rank: vec![0; len],
            }
        }

        fn find(&mut self, mut x: usize) -> usize {
            let mut root = x;
            while self.parent[root] != root {
                root = self.parent[root];
            }
            while self.parent[x] != x {
                let parent = self.parent[x];
                self.parent[x] = root;
                x = parent;
            }
            root
        }

        fn union(&mut self, a: usize, b: usize) {
            let ra = self.find(a);
            let rb = self.find(b);
            if ra == rb {
                return;
            }

            let rank_a = self.rank[ra];
            let rank_b = self.rank[rb];
            if rank_a < rank_b {
                self.parent[ra] = rb;
            } else if rank_b < rank_a {
                self.parent[rb] = ra;
            } else {
                self.parent[rb] = ra;
                self.rank[ra] = rank_a.saturating_add(1);
            }
        }
    }

    let mut filtered_low = 0usize;
    if summaries.len() > BUG_LLM_DEDUP_LOW_SEVERITY_MAX_FINDINGS {
        let mut filtered_ids: HashSet<usize> = HashSet::new();
        summaries.retain(|summary| {
            let is_low = severity_rank(&summary.severity) == 2;
            if is_low {
                filtered_ids.insert(summary.id);
            }
            !is_low
        });
        let dropped = filtered_ids.len();
        if dropped > 0 {
            filtered_low = filtered_low.saturating_add(dropped);
            details.retain(|detail| !filtered_ids.contains(&detail.summary_id));
            logs.push(format!(
                "Dropped {dropped} low severity finding(s) because total findings exceeded {BUG_LLM_DEDUP_LOW_SEVERITY_MAX_FINDINGS}."
            ));
        }
        if summaries.len() < 2 {
            return BugLlmDedupeOutcome {
                summaries,
                details,
                removed: 0,
                filtered_low,
                logs,
            };
        }
    }

    emit_progress_log(
        &progress_sender,
        &log_sink,
        format!(
            "LLM dedupe pass1 requesting model decision for {} finding(s).",
            summaries.len()
        ),
    );

    let max_prompt_chars = bug_dedupe_prompt_max_chars_for_model(model);
    let mut ordered: Vec<&BugSummary> = summaries.iter().collect();
    ordered.sort_by_key(|summary| summary.id);
    let mut pass1_choice = llm_dedupe_prompt_pass1(&ordered, max_prompt_chars);
    if !pass1_choice.fits {
        let mut filtered_ids: HashSet<usize> = HashSet::new();
        summaries.retain(|summary| {
            let is_low = severity_rank(&summary.severity) == 2;
            if is_low {
                filtered_ids.insert(summary.id);
            }
            !is_low
        });
        let dropped = filtered_ids.len();
        if dropped > 0 {
            filtered_low = filtered_low.saturating_add(dropped);
            details.retain(|detail| !filtered_ids.contains(&detail.summary_id));
            logs.push(format!(
                "Dropped {dropped} low severity finding(s) to fit LLM dedupe prompt within {max_prompt_chars} chars."
            ));
            if summaries.len() < 2 {
                return BugLlmDedupeOutcome {
                    summaries,
                    details,
                    removed: 0,
                    filtered_low,
                    logs,
                };
            }
            ordered = summaries.iter().collect();
            ordered.sort_by_key(|summary| summary.id);
            pass1_choice = llm_dedupe_prompt_pass1(&ordered, max_prompt_chars);
        }
    }

    let pass1_prefix = llm_dedupe_debug_prefix("pass1");
    if !pass1_choice.fits {
        logs.push(format!(
            "Skipping LLM dedupe pass1: prompt too large ({} chars > {max_prompt_chars}).",
            pass1_choice.prompt.len()
        ));
        llm_dedupe_write_debug_prompt(
            debug_dir.as_deref(),
            pass1_prefix.as_str(),
            pass1_choice.listing.as_str(),
            pass1_choice.prompt.as_str(),
            &mut logs,
        )
        .await;
        return BugLlmDedupeOutcome {
            summaries,
            details,
            removed: 0,
            filtered_low,
            logs,
        };
    }

    logs.push(format!("LLM dedupe pass1 {}.", pass1_choice.limits_log));
    llm_dedupe_write_debug_prompt(
        debug_dir.as_deref(),
        pass1_prefix.as_str(),
        pass1_choice.listing.as_str(),
        pass1_choice.prompt.as_str(),
        &mut logs,
    )
    .await;

    let cached_decisions = llm_dedupe_read_pass1_cache(
        debug_dir.as_deref(),
        pass1_choice.listing.as_str(),
        &mut logs,
    )
    .await;
    let used_cache = cached_decisions.is_some();
    let (pass1_decisions, mut pass1_logs) = if let Some(decisions) = cached_decisions {
        (decisions, Vec::new())
    } else {
        llm_dedupe_execute_prompt(
            client,
            provider,
            auth,
            model,
            reasoning_effort,
            pass1_choice.prompt.as_str(),
            debug_dir.as_deref(),
            pass1_prefix.as_str(),
            "LLM dedupe pass1",
            progress_sender.clone(),
            log_sink.clone(),
            metrics.clone(),
        )
        .await
    };
    logs.append(&mut pass1_logs);
    if !used_cache {
        llm_dedupe_write_pass1_cache(
            debug_dir.as_deref(),
            pass1_choice.listing.as_str(),
            &pass1_decisions,
            &mut logs,
        )
        .await;
    }

    if pass1_decisions.is_empty() {
        return BugLlmDedupeOutcome {
            summaries,
            details,
            removed: 0,
            filtered_low,
            logs,
        };
    }

    let id_to_index: HashMap<usize, usize> = summaries
        .iter()
        .enumerate()
        .map(|(idx, summary)| (summary.id, idx))
        .collect();

    let mut pass1_uf = UnionFind::new(summaries.len());
    let mut pass1_proposed = 0usize;
    let mut pass1_accepted = 0usize;
    for decision in pass1_decisions {
        if decision.id == decision.canonical_id {
            continue;
        }
        pass1_proposed += 1;
        let Some(&a) = id_to_index.get(&decision.id) else {
            continue;
        };
        let Some(&b) = id_to_index.get(&decision.canonical_id) else {
            continue;
        };
        pass1_uf.union(a, b);
        pass1_accepted += 1;
    }

    let mut pass1_groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for (idx, summary) in summaries.iter().enumerate() {
        let root = pass1_uf.find(idx);
        pass1_groups.entry(root).or_default().push(summary.id);
    }

    let mut candidate_groups: Vec<Vec<usize>> = pass1_groups
        .values()
        .filter(|group| group.len() > 1)
        .cloned()
        .collect();
    for group in &mut candidate_groups {
        group.sort_unstable();
    }
    candidate_groups.sort_by_key(|group| group.first().copied().unwrap_or(usize::MAX));

    if candidate_groups.is_empty() {
        logs.push(format!(
            "LLM dedupe pass1 proposed {pass1_proposed} merge(s); accepted {pass1_accepted}; no candidate groups formed."
        ));
        return BugLlmDedupeOutcome {
            summaries,
            details,
            removed: 0,
            filtered_low,
            logs,
        };
    }

    logs.push(format!(
        "LLM dedupe pass1 proposed {pass1_proposed} merge(s); accepted {pass1_accepted}; {} candidate group(s).",
        candidate_groups.len()
    ));

    struct Pass2Outcome {
        decisions: Vec<BugDedupeDecision>,
        logs: Vec<String>,
    }

    let decisions: Vec<BugDedupeDecision> = {
        let id_to_summary: HashMap<usize, &BugSummary> = summaries
            .iter()
            .map(|summary| (summary.id, summary))
            .collect();
        let semaphore = Arc::new(Semaphore::new(BUG_LLM_DEDUP_CONCURRENCY));
        let mut pass2_tasks: FuturesUnordered<_> = FuturesUnordered::new();
        let mut pass2_task_count = 0usize;
        for (group_index, group_ids) in candidate_groups.into_iter().enumerate() {
            let mut group_summaries: Vec<&BugSummary> = group_ids
                .iter()
                .filter_map(|id| id_to_summary.get(id).copied())
                .collect();
            if group_summaries.len() < 2 {
                continue;
            }
            group_summaries.sort_by_key(|summary| summary.id);

            let client = client.clone();
            let provider = provider.clone();
            let auth = auth.clone();
            let model = model.to_string();
            let metrics = metrics.clone();
            let debug_dir = debug_dir.clone();
            let semaphore = semaphore.clone();
            let group_label = format!("pass2_group_{}", group_index + 1);
            let log_prefix = format!("LLM dedupe pass2 group {}", group_index + 1);
            let progress_sender = progress_sender.clone();
            let log_sink = log_sink.clone();

            pass2_tasks.push(async move {
                let _permit = semaphore.acquire_owned().await.ok();
                let choice = llm_dedupe_prompt_pass2(&group_summaries, max_prompt_chars);
                let debug_prefix = llm_dedupe_debug_prefix(&group_label);
                let mut logs: Vec<String> = Vec::new();

                if !choice.fits {
                    logs.push(format!(
                        "{log_prefix} skipped: prompt too large ({} chars > {max_prompt_chars}).",
                        choice.prompt.len()
                    ));
                    llm_dedupe_write_debug_prompt(
                        debug_dir.as_deref(),
                        debug_prefix.as_str(),
                        choice.listing.as_str(),
                        choice.prompt.as_str(),
                        &mut logs,
                    )
                    .await;
                    return Pass2Outcome {
                        decisions: Vec::new(),
                        logs,
                    };
                }

                logs.push(format!("{log_prefix} {}.", choice.limits_log));
                llm_dedupe_write_debug_prompt(
                    debug_dir.as_deref(),
                    debug_prefix.as_str(),
                    choice.listing.as_str(),
                    choice.prompt.as_str(),
                    &mut logs,
                )
                .await;

                let (decisions, mut call_logs) = llm_dedupe_execute_prompt(
                    &client,
                    &provider,
                    &auth,
                    model.as_str(),
                    reasoning_effort,
                    choice.prompt.as_str(),
                    debug_dir.as_deref(),
                    debug_prefix.as_str(),
                    log_prefix.as_str(),
                    progress_sender,
                    log_sink,
                    metrics,
                )
                .await;
                logs.append(&mut call_logs);
                Pass2Outcome { decisions, logs }
            });
            pass2_task_count += 1;
        }

        let mut decisions: Vec<BugDedupeDecision> = Vec::new();
        let total_groups = pass2_task_count;
        if total_groups > 0 {
            emit_progress_log(
                &progress_sender,
                &log_sink,
                format!(
                    "LLM dedupe pass2 starting for {total_groups} group(s) with concurrency {BUG_LLM_DEDUP_CONCURRENCY}.",
                ),
            );
        }
        let mut completed_groups = 0usize;
        while let Some(outcome) = pass2_tasks.next().await {
            logs.extend(outcome.logs);
            decisions.extend(outcome.decisions);
            completed_groups = completed_groups.saturating_add(1);
            if total_groups > 0
                && (completed_groups == total_groups || completed_groups.is_multiple_of(5))
            {
                emit_progress_log(
                    &progress_sender,
                    &log_sink,
                    format!(
                        "LLM dedupe pass2 progress: {completed_groups}/{total_groups} group(s) complete.",
                    ),
                );
            }
        }
        decisions
    };

    if decisions.is_empty() {
        logs.push("LLM dedupe pass2 produced no merge decisions; skipping.".to_string());
        return BugLlmDedupeOutcome {
            summaries,
            details,
            removed: 0,
            filtered_low,
            logs,
        };
    }

    let id_to_index: HashMap<usize, usize> = summaries
        .iter()
        .enumerate()
        .map(|(idx, summary)| (summary.id, idx))
        .collect();

    let mut uf = UnionFind::new(summaries.len());
    let mut proposed_merges = 0usize;
    let mut accepted_merges = 0usize;
    for decision in decisions {
        if decision.id == decision.canonical_id {
            continue;
        }
        proposed_merges += 1;
        let Some(&a) = id_to_index.get(&decision.id) else {
            continue;
        };
        let Some(&b) = id_to_index.get(&decision.canonical_id) else {
            continue;
        };
        uf.union(a, b);
        accepted_merges += 1;
    }

    #[derive(Clone)]
    struct GroupAgg {
        rep_index: usize,
        file_set: Vec<String>,
        members: Vec<usize>,
    }

    let mut root_to_group: HashMap<usize, GroupAgg> = HashMap::new();
    for (idx, summary) in summaries.iter().enumerate() {
        let root = uf.find(idx);
        let entry = root_to_group.entry(root).or_insert_with(|| GroupAgg {
            rep_index: idx,
            file_set: Vec::new(),
            members: Vec::new(),
        });

        let rep = &summaries[entry.rep_index];
        let rep_risk = rep.risk_rank.unwrap_or(usize::MAX);
        let cur_risk = summary.risk_rank.unwrap_or(usize::MAX);
        if cur_risk < rep_risk
            || (cur_risk == rep_risk
                && (severity_rank(&summary.severity) < severity_rank(&rep.severity)
                    || (severity_rank(&summary.severity) == severity_rank(&rep.severity)
                        && summary.id < rep.id)))
        {
            entry.rep_index = idx;
        }

        for loc in llm_dedupe_file_locations(summary) {
            if !entry.file_set.iter().any(|existing| existing == &loc) {
                entry.file_set.push(loc);
            }
        }
        entry.members.push(summary.id);
    }

    if root_to_group.len() == summaries.len() {
        logs.push(format!(
            "LLM dedupe pass2 proposed {proposed_merges} merge(s); accepted {accepted_merges}; no groups formed."
        ));
        return BugLlmDedupeOutcome {
            summaries,
            details,
            removed: 0,
            filtered_low,
            logs,
        };
    }

    logs.push(format!(
        "LLM dedupe pass2 proposed {proposed_merges} merge(s); accepted {accepted_merges}."
    ));

    let mut detail_by_id: HashMap<usize, String> = HashMap::new();
    for detail in &details {
        detail_by_id.insert(detail.summary_id, detail.original_markdown.clone());
    }

    let mut keep_ids: HashSet<usize> = HashSet::new();
    let id_to_index: HashMap<usize, usize> = summaries
        .iter()
        .enumerate()
        .map(|(idx, summary)| (summary.id, idx))
        .collect();

    #[derive(Serialize)]
    struct LlmDedupeClusterDebug {
        canonical_id: usize,
        member_ids: Vec<usize>,
        highest_severity: String,
        unique_instances: Vec<String>,
    }

    let mut clusters_debug: Vec<LlmDedupeClusterDebug> = Vec::new();
    for agg in root_to_group.values() {
        let rep_id = summaries[agg.rep_index].id;
        let location_joined_full = if agg.file_set.is_empty() {
            summaries[agg.rep_index].file.clone()
        } else {
            agg.file_set.join(", ")
        };
        let location_joined_display = if agg.file_set.is_empty() {
            location_joined_full.clone()
        } else {
            let max_locations = 12usize;
            let mut display_parts: Vec<String> =
                agg.file_set.iter().take(max_locations).cloned().collect();
            if agg.file_set.len() > max_locations {
                display_parts.push(format!("...(+{})", agg.file_set.len() - max_locations));
            }
            display_parts.join(", ")
        };

        let mut types: Vec<String> = Vec::new();
        for m_id in &agg.members {
            if let Some(&idx) = id_to_index.get(m_id) {
                for entry in &summaries[idx].verification_types {
                    if !types
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(entry))
                    {
                        types.push(entry.clone());
                    }
                }
            }
        }

        let mut best_severity = summaries[agg.rep_index].severity.clone();
        for m_id in &agg.members {
            if let Some(&idx) = id_to_index.get(m_id)
                && severity_rank(&summaries[idx].severity) < severity_rank(&best_severity)
            {
                best_severity = summaries[idx].severity.clone();
            }
        }

        let rep_mut = &mut summaries[agg.rep_index];
        rep_mut.file = location_joined_full.clone();
        rep_mut.severity = best_severity.clone();
        rep_mut.verification_types = types;
        if let Some(updated) =
            rewrite_bug_markdown_location(&rep_mut.markdown, location_joined_display.as_str())
        {
            rep_mut.markdown = updated.clone();
            detail_by_id.insert(rep_id, updated);
        }

        clusters_debug.push(LlmDedupeClusterDebug {
            canonical_id: rep_id,
            member_ids: agg.members.clone(),
            highest_severity: best_severity,
            unique_instances: agg.file_set.clone(),
        });
        keep_ids.insert(rep_id);
    }

    if let Some(dir) = debug_dir.as_ref() {
        let session_id = security_review_session_id();
        let prefix = format!("llm_dedupe_{session_id}");
        let path = dir.join(format!("{prefix}_clusters.json"));
        match serde_json::to_string_pretty(&clusters_debug) {
            Ok(json) => {
                if let Err(err) = tokio_fs::write(&path, json).await {
                    logs.push(format!(
                        "LLM dedupe debug: failed to write {}: {err}",
                        path.display()
                    ));
                }
            }
            Err(err) => logs.push(format!(
                "LLM dedupe debug: failed to serialize clusters: {err}"
            )),
        }
    }

    summaries.retain(|summary| keep_ids.contains(&summary.id));

    let mut new_details: Vec<BugDetail> = Vec::with_capacity(keep_ids.len());
    for id in &keep_ids {
        if let Some(markdown) = detail_by_id.get(id) {
            new_details.push(BugDetail {
                summary_id: *id,
                original_markdown: markdown.clone(),
            });
        }
    }

    let removed = details.len().saturating_sub(new_details.len());
    BugLlmDedupeOutcome {
        summaries,
        details: new_details,
        removed,
        filtered_low,
        logs,
    }
}

#[cfg(test)]
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

    #[derive(Clone)]
    struct UnionFind {
        parent: Vec<usize>,
        rank: Vec<u8>,
    }

    impl UnionFind {
        fn new(len: usize) -> Self {
            Self {
                parent: (0..len).collect(),
                rank: vec![0; len],
            }
        }

        fn find(&mut self, mut x: usize) -> usize {
            let mut root = x;
            while self.parent[root] != root {
                root = self.parent[root];
            }
            while self.parent[x] != x {
                let parent = self.parent[x];
                self.parent[x] = root;
                x = parent;
            }
            root
        }

        fn union(&mut self, a: usize, b: usize) {
            let ra = self.find(a);
            let rb = self.find(b);
            if ra == rb {
                return;
            }

            let rank_a = self.rank[ra];
            let rank_b = self.rank[rb];
            if rank_a < rank_b {
                self.parent[ra] = rb;
            } else if rank_b < rank_a {
                self.parent[rb] = ra;
            } else {
                self.parent[rb] = ra;
                self.rank[ra] = rank_a.saturating_add(1);
            }
        }
    }

    let tag_tokens: Vec<Option<HashSet<String>>> = summaries
        .iter()
        .map(|summary| {
            summary
                .vulnerability_tag
                .as_deref()
                .map(vuln_tag_tokens_for_dedupe)
        })
        .collect();

    let mut uf = UnionFind::new(summaries.len());

    for i in 0..summaries.len() {
        let Some(a_tokens) = tag_tokens[i].as_ref() else {
            continue;
        };
        for (j, tokens) in tag_tokens.iter().enumerate().skip(i + 1) {
            let Some(b_tokens) = tokens.as_ref() else {
                continue;
            };

            let (smaller, larger) = if a_tokens.len() <= b_tokens.len() {
                (a_tokens, b_tokens)
            } else {
                (b_tokens, a_tokens)
            };

            let mut intersection = 0usize;
            let mut informative = 0usize;
            for token in smaller {
                if larger.contains(token) {
                    intersection += 1;
                    if !is_generic_dedupe_token(token.as_str()) {
                        informative += 1;
                    }
                }
            }
            if intersection < 2 || informative < 1 {
                continue;
            }
            let union = a_tokens.len() + b_tokens.len() - intersection;
            if union == 0 {
                continue;
            }
            let jaccard = intersection as f32 / union as f32;
            if jaccard >= 0.5 {
                uf.union(i, j);
            }
        }
    }

    let mut key_to_group: HashMap<String, GroupAgg> = HashMap::new();
    for (idx, s) in summaries.iter().enumerate() {
        let key = if s.vulnerability_tag.is_some() {
            let root = uf.find(idx);
            format!("tag_group::{root}")
        } else {
            format!("title::{}", normalize_title_key(&s.title))
        };

        let entry = key_to_group.entry(key).or_insert_with(|| GroupAgg {
            rep_index: idx,
            file_set: Vec::new(),
            members: Vec::new(),
        });

        let rep = &summaries[entry.rep_index];
        let rep_risk = rep.risk_rank.unwrap_or(usize::MAX);
        let cur_risk = s.risk_rank.unwrap_or(usize::MAX);
        if cur_risk < rep_risk
            || (cur_risk == rep_risk
                && (severity_rank(&s.severity) < severity_rank(&rep.severity)
                    || (severity_rank(&s.severity) == severity_rank(&rep.severity)
                        && s.id < rep.id)))
        {
            entry.rep_index = idx;
        }

        for loc in preferred_bug_locations_for_reporting(s.markdown.as_str(), s.file.as_str()) {
            if !entry.file_set.iter().any(|existing| existing == &loc) {
                entry.file_set.push(loc);
            }
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

fn rank_bug_summaries_for_reporting(summaries: &mut [BugSummary]) {
    summaries.sort_by(|a, b| {
        severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| match (a.risk_score, b.risk_score) {
                (Some(sa), Some(sb)) => sb.partial_cmp(&sa).unwrap_or(CmpOrdering::Equal),
                (Some(_), None) => CmpOrdering::Less,
                (None, Some(_)) => CmpOrdering::Greater,
                _ => CmpOrdering::Equal,
            })
            .then_with(|| a.id.cmp(&b.id))
    });

    for (idx, summary) in summaries.iter_mut().enumerate() {
        summary.risk_rank = Some(idx + 1);
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
        if is_standalone_bug_anchor_line(line) {
            changed = true;
            continue;
        }
        if !updated_first_heading {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("### ") {
                // Drop any leading bracketed id like "[12] " from the heading text
                let clean = rest
                    .trim_start()
                    .strip_prefix("<a id=\"bug-")
                    .and_then(|rest| rest.split_once("</a>"))
                    .map(|(_, after)| after.trim_start())
                    .unwrap_or(rest.trim_start())
                    .trim_start_matches('[')
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .trim_start_matches(']')
                    .trim_start();
                // Embed an explicit anchor inline for stable linking without adding a separate
                // paragraph/line in rendered HTML output.
                out.push(format!(
                    "### <a id=\"bug-{summary_id}\"></a> [{summary_id}] {clean}"
                ));
                changed = true;
                updated_first_heading = true;
                continue;
            }
        }
        out.push(line.to_string());
    }
    if changed { Some(out.join("\n")) } else { None }
}

fn is_standalone_bug_anchor_line(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix("<a id=\"bug-") else {
        return false;
    };
    let Some(id) = rest.strip_suffix("\"></a>") else {
        return false;
    };
    !id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit())
}

fn strip_standalone_bug_anchor_lines(markdown: &str) -> Option<String> {
    let mut changed = false;
    let mut out: Vec<&str> = Vec::new();
    for line in markdown.lines() {
        if is_standalone_bug_anchor_line(line) {
            changed = true;
            continue;
        }
        out.push(line);
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
        BugValidationStatus::Pending => "Not validated".to_string(),
        BugValidationStatus::Passed => "Validated".to_string(),
        BugValidationStatus::Failed => "Not validated".to_string(),
        BugValidationStatus::UnableToValidate => "Not able to validate".to_string(),
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
    reasoning_effort: Option<ReasoningEffort>,
    system_prompt: &str,
    base_prompt: String,
    metrics: Arc<ReviewMetrics>,
    repo_root: PathBuf,
    ids: Vec<usize>,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
) -> Result<RiskRerankChunkSuccess, RiskRerankChunkFailure> {
    let mut conversation: Vec<String> = Vec::new();
    let mut seen_search_requests: HashSet<String> = HashSet::new();
    let mut seen_read_requests: HashSet<String> = HashSet::new();
    let mut command_error_count = 0usize;
    let mut logs: Vec<String> = Vec::new();

    let repo_display = repo_root.display().to_string();

    loop {
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
            reasoning_effort,
            system_prompt,
            &prompt,
            metrics.clone(),
            0.0,
        )
        .await
        {
            Ok(output) => output,
            Err(err) => {
                push_progress_log(
                    &progress_sender,
                    &log_sink,
                    &mut logs,
                    format!("Risk rerank model request failed: {err}"),
                );
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
                push_progress_log(
                    &progress_sender,
                    &log_sink,
                    &mut logs,
                    format!("Risk rerank reasoning: {truncated}"),
                );
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

        let mut saw_tool_request = false;
        let mut executed_new_tool = false;

        for request in read_requests {
            saw_tool_request = true;
            let cmd_label = request.command.label();
            let key = request.dedupe_key();
            if !seen_read_requests.insert(key) {
                push_progress_log(
                    &progress_sender,
                    &log_sink,
                    &mut logs,
                    format!(
                        "Risk rerank {cmd_label} `{}` skipped (already provided).",
                        request.path.display(),
                    ),
                );
                conversation.push(format!(
                    "Tool {cmd_label} `{}` already provided earlier.",
                    request.path.display()
                ));
                continue;
            }

            executed_new_tool = true;
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
                    conversation.push(format!(
                        "Tool {cmd_label} `{}`:\n{}",
                        request.path.display(),
                        output
                    ));
                }
                Err(err) => {
                    push_progress_log(
                        &progress_sender,
                        &log_sink,
                        &mut logs,
                        format!(
                            "Risk rerank {cmd_label} `{}` failed: {err}",
                            request.path.display(),
                        ),
                    );
                    conversation.push(format!(
                        "Tool {cmd_label} `{}` error: {err}",
                        request.path.display()
                    ));
                    command_error_count += 1;
                    if command_error_count >= BUG_RERANK_MAX_COMMAND_ERRORS {
                        push_progress_log(
                            &progress_sender,
                            &log_sink,
                            &mut logs,
                            format!(
                                "Risk rerank aborted after {BUG_RERANK_MAX_COMMAND_ERRORS} tool errors."
                            ),
                        );
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
            saw_tool_request = true;
            let key = request.dedupe_key();
            if seen_search_requests.insert(key) {
                new_requests.push(request);
            } else {
                match &request {
                    ToolRequest::Content { term, mode, .. } => {
                        let display_term = summarize_search_term(term, 80);
                        push_progress_log(
                            &progress_sender,
                            &log_sink,
                            &mut logs,
                            format!(
                                "Risk rerank search `{display_term}` ({}) skipped (already provided).",
                                mode.as_str()
                            ),
                        );
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
                        push_progress_log(
                            &progress_sender,
                            &log_sink,
                            &mut logs,
                            format!("Risk rerank GREP_FILES {shown} skipped (already provided)."),
                        );
                        conversation
                            .push(format!("Tool GREP_FILES {shown} already provided earlier."));
                    }
                }
            }
        }

        for request in new_requests {
            executed_new_tool = true;
            if let Some(reason) = request.reason()
                && !reason.trim().is_empty()
            {
                let truncated = truncate_text(reason, MODEL_REASONING_LOG_MAX_GRAPHEMES);
                push_progress_log(
                    &progress_sender,
                    &log_sink,
                    &mut logs,
                    format!(
                        "Risk rerank tool rationale ({}): {truncated}",
                        request.kind_label()
                    ),
                );
            }

            match request {
                ToolRequest::Content { term, mode, .. } => {
                    let display_term = summarize_search_term(&term, 80);
                    push_progress_log(
                        &progress_sender,
                        &log_sink,
                        &mut logs,
                        format!(
                            "Risk rerank {mode} content search for `{display_term}` — path {repo_display}",
                            mode = mode.as_str()
                        ),
                    );
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
                            push_progress_log(
                                &progress_sender,
                                &log_sink,
                                &mut logs,
                                message.clone(),
                            );
                            conversation.push(format!(
                                "Tool SEARCH `{display_term}` ({}) results:\n{message}",
                                mode.as_str()
                            ));
                        }
                        SearchResult::Error(err) => {
                            push_progress_log(
                                &progress_sender,
                                &log_sink,
                                &mut logs,
                                format!(
                                    "Risk rerank search `{display_term}` ({}) failed: {err} — path {repo_display}",
                                    mode.as_str()
                                ),
                            );
                            conversation.push(format!(
                                "Tool SEARCH `{display_term}` ({}) error: {err}",
                                mode.as_str()
                            ));
                            command_error_count += 1;
                            if command_error_count >= BUG_RERANK_MAX_COMMAND_ERRORS {
                                push_progress_log(
                                    &progress_sender,
                                    &log_sink,
                                    &mut logs,
                                    format!(
                                        "Risk rerank aborted after {BUG_RERANK_MAX_COMMAND_ERRORS} tool errors."
                                    ),
                                );
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
                    push_progress_log(
                        &progress_sender,
                        &log_sink,
                        &mut logs,
                        format!("Risk rerank GREP_FILES {shown} — path {repo_display}"),
                    );
                    match run_grep_files(&repo_root, &args, &metrics).await {
                        SearchResult::Matches(output) => {
                            conversation.push(format!("Tool GREP_FILES {shown}:\n{output}"));
                        }
                        SearchResult::NoMatches => {
                            let message = "No matches found.".to_string();
                            push_progress_log(
                                &progress_sender,
                                &log_sink,
                                &mut logs,
                                format!("Risk rerank GREP_FILES {shown} returned no matches."),
                            );
                            conversation.push(format!("Tool GREP_FILES {shown}:\n{message}"));
                        }
                        SearchResult::Error(err) => {
                            push_progress_log(
                                &progress_sender,
                                &log_sink,
                                &mut logs,
                                format!("Risk rerank GREP_FILES {shown} failed: {err}"),
                            );
                            conversation.push(format!("Tool GREP_FILES {shown} error: {err}"));
                            command_error_count += 1;
                            if command_error_count >= BUG_RERANK_MAX_COMMAND_ERRORS {
                                push_progress_log(
                                    &progress_sender,
                                    &log_sink,
                                    &mut logs,
                                    format!(
                                        "Risk rerank aborted after {BUG_RERANK_MAX_COMMAND_ERRORS} tool errors."
                                    ),
                                );
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

        if saw_tool_request {
            if !executed_new_tool {
                conversation.push(
                    "Note:\nAll requested tool outputs above were already provided. Return the final JSON Lines output without requesting more tools.".to_string(),
                );
            }
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
    reasoning_effort: Option<ReasoningEffort>,
    summaries: &mut [BugSummary],
    repo_root: &Path,
    _repository_summary: &str,
    spec_context: Option<&str>,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
) -> Vec<String> {
    if summaries.is_empty() {
        return Vec::new();
    }

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
            .replace("{spec_excerpt}", &spec_excerpt_snippet)
            .replace("{findings}", &findings_payload);
        prompt_chunks.push((ids, prompt));
    }

    let total_chunks = prompt_chunks.len();
    let max_concurrency = BUG_RERANK_MAX_CONCURRENCY.max(1).min(total_chunks.max(1));

    let mut logs: Vec<String> = Vec::new();
    let mut decisions: HashMap<usize, RiskDecision> = HashMap::new();
    if total_chunks > 0 {
        let rerank_reasoning_label = reasoning_effort_label(normalize_reasoning_effort_for_model(
            model,
            reasoning_effort,
        ));
        push_progress_log(
            &progress_sender,
            &log_sink,
            &mut logs,
            format!("Running risk rerank (model: {model}, reasoning: {rerank_reasoning_label})."),
        );
        push_progress_log(
            &progress_sender,
            &log_sink,
            &mut logs,
            format!(
                "Risk rerank starting for {} finding(s) with concurrency {max_concurrency}.",
                summaries.len()
            ),
        );
        emit_progress_log(
            &progress_sender,
            &log_sink,
            format!("  └ Launching parallel risk rerank ({max_concurrency} workers)"),
        );
    }

    let total_findings = summaries.len();
    let mut chunk_results =
        futures::stream::iter(prompt_chunks.into_iter().map(|(ids, prompt)| {
            let provider = provider.clone();
            let auth_clone = auth.clone();
            let model_owned = model.to_string();
            let metrics_clone = metrics.clone();
            let repo_root = repo_root.to_path_buf();
            let progress_sender = progress_sender.clone();
            let log_sink = log_sink.clone();
            let chunk_size = ids.len();

            async move {
                (
                    chunk_size,
                    run_risk_rerank_chunk(
                        client,
                        &provider,
                        &auth_clone,
                        model_owned.as_str(),
                        reasoning_effort,
                        BUG_RERANK_SYSTEM_PROMPT,
                        prompt,
                        metrics_clone,
                        repo_root,
                        ids,
                        progress_sender,
                        log_sink,
                    )
                    .await,
                )
            }
        }))
        .buffer_unordered(max_concurrency);

    let mut completed_findings = 0usize;
    while let Some((chunk_size, result)) = chunk_results.next().await {
        completed_findings = completed_findings.saturating_add(chunk_size);
        if total_chunks > 0
            && (completed_findings >= total_findings || completed_findings.is_multiple_of(5))
        {
            let clamped_completed = completed_findings.min(total_findings);
            let percent = if total_findings == 0 {
                0
            } else {
                (clamped_completed * 100) / total_findings
            };
            emit_progress_log(
                &progress_sender,
                &log_sink,
                format!("Risk rerank progress: {clamped_completed}/{total_findings} - {percent}%.",),
            );
        }

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
                push_progress_log(
                    &progress_sender,
                    &log_sink,
                    &mut logs,
                    format!(
                        "Risk rerank chunk failed for bug id(s) {id_list}: {error}",
                        error = failure.error
                    ),
                );
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

    for summary in summaries.iter_mut() {
        if let Some(update) = apply_severity_matrix(summary) {
            let id = summary.id;
            let previous = update.previous;
            let computed = summary.severity.as_str();
            let impact = update.impact.label();
            let likelihood = update.likelihood.label();
            let product = update.product;
            append_log(
                &log_sink,
                &mut logs,
                format!(
                    "Severity matrix: bug #{id} updated from {previous} to {computed} (impact {impact} * likelihood {likelihood} = {product})."
                ),
            );
        }
    }

    rank_bug_summaries_for_reporting(summaries);

    for (idx, summary) in summaries.iter().enumerate() {
        let id = summary.id;
        let rank = summary.risk_rank.unwrap_or(idx + 1);
        let severity = summary.severity.as_str();
        let log_entry = if let Some(score) = summary.risk_score {
            let reason = summary
                .risk_reason
                .as_deref()
                .unwrap_or("no reason provided");
            format!(
                "Risk rerank: bug #{id} -> priority {rank} (score {score:.1}, severity {severity}) — {reason}"
            )
        } else {
            format!(
                "Risk rerank: bug #{id} retained original severity {severity} (no model decision)"
            )
        };
        append_log(&log_sink, &mut logs, log_entry);
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

async fn collect_git_revision(repo_path: &Path) -> Option<GitRevisionInfo> {
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
    Some(GitRevisionInfo {
        commit,
        branch,
        commit_timestamp: timestamp,
        repository_url: git_info.repository_url,
    })
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

fn normalize_github_repo_url(remote: &str) -> Option<String> {
    let trimmed_remote = remote.trim();
    if trimmed_remote.is_empty() {
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

    Some(base)
}

fn github_owner_repo_from_remote(remote: &str) -> Option<(String, String)> {
    let normalized = normalize_github_repo_url(remote)?;
    let url = Url::parse(&normalized).ok()?;
    if url.host_str()? != "github.com" {
        return None;
    }
    let mut segments = url.path_segments()?;
    let owner = segments.next()?.trim();
    let repo = segments.next()?.trim();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

fn is_openai_openai_repo(remote: &str) -> bool {
    github_owner_repo_from_remote(remote).is_some_and(|(owner, repo)| {
        owner.eq_ignore_ascii_case("openai") && repo.eq_ignore_ascii_case("openai")
    })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    const fn weight(self) -> i64 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

fn parse_risk_level_prefix(value: &str) -> Option<RiskLevel> {
    let trimmed = value.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    fn starts_with_level(lower: &str, token: &str) -> bool {
        lower.strip_prefix(token).is_some_and(|rest| {
            let mut chars = rest.chars();
            match chars.next() {
                None => true,
                Some(ch) => !ch.is_ascii_alphabetic(),
            }
        })
    }

    let lower = trimmed.to_ascii_lowercase();
    if starts_with_level(&lower, "high") {
        Some(RiskLevel::High)
    } else if starts_with_level(&lower, "medium") || starts_with_level(&lower, "med") {
        Some(RiskLevel::Medium)
    } else if starts_with_level(&lower, "low") {
        Some(RiskLevel::Low)
    } else {
        None
    }
}

struct SeverityMatrixUpdate {
    previous: String,
    impact: RiskLevel,
    likelihood: RiskLevel,
    product: i64,
}

fn apply_severity_matrix(summary: &mut BugSummary) -> Option<SeverityMatrixUpdate> {
    let severity = summary.severity.trim();
    if severity.eq_ignore_ascii_case("ignore")
        || severity.eq_ignore_ascii_case("informational")
        || severity.eq_ignore_ascii_case("info")
    {
        return None;
    }

    let impact = parse_risk_level_prefix(&summary.impact)?;
    let likelihood = parse_risk_level_prefix(&summary.likelihood)?;
    let product = impact.weight().saturating_mul(likelihood.weight());
    let computed = if product >= 6 {
        "High"
    } else if product >= 3 {
        "Medium"
    } else {
        "Low"
    };

    if summary.severity.trim().eq_ignore_ascii_case(computed) {
        return None;
    }

    let previous = summary.severity.clone();
    summary.severity = computed.to_string();

    Some(SeverityMatrixUpdate {
        previous,
        impact,
        likelihood,
        product,
    })
}

#[cfg(test)]
mod openai_openai_validation_skip_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn detects_openai_openai_repo_remotes() {
        for remote in [
            "https://github.com/openai/openai",
            "https://github.com/openai/openai.git",
            "git@github.com:openai/openai.git",
            "ssh://git@github.com/openai/openai.git",
        ] {
            assert_eq!(is_openai_openai_repo(remote), true, "{remote}");
        }
    }

    #[test]
    fn ignores_non_openai_openai_remotes() {
        for remote in [
            "https://github.com/openai/other-repo.git",
            "git@github.com:openai/other-repo.git",
            "https://github.com/other/openai.git",
            "git@github.com:other/openai.git",
        ] {
            assert_eq!(is_openai_openai_repo(remote), false, "{remote}");
        }
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
        let mut boundary = limit;
        while boundary > 0 && !summary.is_char_boundary(boundary) {
            boundary -= 1;
        }
        summary.truncate(boundary);
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

#[derive(Clone)]
struct CommandStatusEmitter {
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
}

impl CommandStatusEmitter {
    fn new(progress_sender: Option<AppEventSender>, metrics: Arc<ReviewMetrics>) -> Self {
        Self {
            progress_sender,
            metrics,
        }
    }

    fn emit(
        &self,
        summary: impl Into<String>,
        state: SecurityReviewCommandState,
        preview: Vec<String>,
    ) {
        let id = self.metrics.next_command_id();
        emit_command_status(&self.progress_sender, id, summary.into(), state, preview);
    }

    fn emit_with_preview(
        &self,
        summary: impl Into<String>,
        state: SecurityReviewCommandState,
        command: Option<&str>,
        output: Option<&str>,
    ) {
        let preview = build_command_preview(command, output);
        self.emit(summary, state, preview);
    }
}

fn build_command_preview(command: Option<&str>, output: Option<&str>) -> Vec<String> {
    let mut preview = Vec::new();
    if let Some(command) = command.map(str::trim).filter(|s| !s.is_empty()) {
        preview.push(format!("$ {command}"));
    }
    if let Some(output) = output.map(str::trim).filter(|s| !s.is_empty()) {
        preview.extend(command_preview_snippets(output));
    }
    preview
}

fn emit_command_start(
    emitter: &CommandStatusEmitter,
    summary: impl Into<String>,
    command: Option<&str>,
) {
    emitter.emit_with_preview(summary, SecurityReviewCommandState::Running, command, None);
}

fn emit_command_result(
    emitter: &CommandStatusEmitter,
    summary: impl Into<String>,
    state: SecurityReviewCommandState,
    command: Option<&str>,
    output: Option<&str>,
) {
    emitter.emit_with_preview(summary, state, command, output);
}

fn emit_command_error(emitter: &CommandStatusEmitter, summary: impl Into<String>, message: &str) {
    emitter.emit_with_preview(
        summary,
        SecurityReviewCommandState::Error,
        None,
        Some(message),
    );
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
    seen_files: HashSet<PathBuf>,
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
            seen_files: HashSet::new(),
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

        let relative_path = path
            .strip_prefix(&self.repo_path)
            .unwrap_or(path)
            .to_path_buf();
        if !self.seen_files.insert(relative_path.clone()) {
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

fn build_repository_summary(_snippets: &[FileSnippet]) -> String {
    let mut lines = Vec::new();
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

fn looks_like_base_model_id(model: &str) -> bool {
    let trimmed = model.trim();
    trimmed.starts_with("gpt-")
        || trimmed.starts_with("codex-")
        || trimmed.starts_with("exp-")
        || trimmed.starts_with("o3")
        || trimmed.starts_with("o4")
}

fn model_id_for_context_window(model: &str) -> &str {
    let trimmed = model.trim();
    let trimmed = trimmed
        .rsplit_once('/')
        .map(|(_, model)| model)
        .unwrap_or(trimmed)
        .trim();

    let Some((left, right)) = trimmed.split_once(':') else {
        return trimmed;
    };

    let left = left.trim();
    let right = right.trim();
    let left_is_model = looks_like_base_model_id(left);
    let right_is_model = looks_like_base_model_id(right);

    match (left_is_model, right_is_model) {
        (false, true) => right,
        (true, false) => left,
        _ => trimmed,
    }
}

fn bug_file_context_max_chars_for_model(model: &str, config: &Config) -> usize {
    let model = model_id_for_context_window(model);

    // Prefer per-model inference over config-wide overrides, since security review runs multiple
    // models (triage/spec/bug/validation). `Config::model_context_window` may reflect a different
    // model than the one used for bug analysis.
    let context_window = infer_default_context_window_tokens(model).or(config.model_context_window);

    let Some(context_window) = context_window else {
        return BUG_FILE_CONTEXT_FALLBACK_MAX_CHARS;
    };

    // Reserve headroom for repo/spec context, tool output, and model output.
    // Use a conservative char/token ratio to reduce context window errors.
    const FILE_TOKEN_SHARE_NUM: i64 = 3;
    const FILE_TOKEN_SHARE_DEN: i64 = 10; // 30%
    const CHARS_PER_TOKEN_ESTIMATE: i64 = 3;

    let budget_tokens = context_window
        .saturating_mul(FILE_TOKEN_SHARE_NUM)
        .saturating_div(FILE_TOKEN_SHARE_DEN)
        .max(1);
    let budget_chars = budget_tokens.saturating_mul(CHARS_PER_TOKEN_ESTIMATE);
    let min_chars = BUG_FILE_CONTEXT_RETRY_MAX_CHARS as i64;
    let max_chars = DEFAULT_MAX_BYTES_PER_FILE as i64;

    budget_chars.clamp(min_chars, max_chars) as usize
}

fn infer_default_context_window_tokens(model: &str) -> Option<i64> {
    if model.starts_with("codex-mini-latest")
        || model.starts_with("o3")
        || model.starts_with("o4-mini")
    {
        Some(200_000)
    } else if model.starts_with("gpt-5") || model.starts_with("codex-") || model.starts_with("exp-")
    {
        Some(272_000)
    } else if model.starts_with("gpt-4.1") {
        Some(1_047_576)
    } else if model.starts_with("gpt-oss") {
        Some(96_000)
    } else if model.starts_with("gpt-4o") {
        Some(128_000)
    } else if model.starts_with("gpt-3.5") {
        Some(16_385)
    } else {
        None
    }
}

fn build_single_file_context_for_bug_prompt(
    snippet: &FileSnippet,
    max_chars: usize,
) -> (String, Vec<String>) {
    if snippet.content.chars().nth(max_chars).is_none() {
        return (build_single_file_context(snippet), Vec::new());
    }

    let mut logs = Vec::new();
    let head_chars = max_chars / 2;
    let tail_chars = max_chars.saturating_sub(head_chars);
    let head: String = snippet.content.chars().take(head_chars).collect();
    let tail_rev: String = snippet.content.chars().rev().take(tail_chars).collect();
    let tail: String = tail_rev.chars().rev().collect();

    logs.push(format!(
        "Truncated {path} content to {limit} chars for bug analysis.",
        path = snippet.relative_path.display(),
        limit = max_chars
    ));

    (
        format!(
            "### {}\n```{}\n{}\n\n... [truncated] ...\n\n{}\n```\n",
            snippet.relative_path.display(),
            snippet.language,
            head,
            tail
        ),
        logs,
    )
}

fn build_single_file_context_for_bug_retry(snippet: &FileSnippet) -> (String, Vec<String>) {
    if snippet.content.len() <= BUG_FILE_CONTEXT_RETRY_MAX_CHARS {
        return (build_single_file_context(snippet), Vec::new());
    }

    let mut logs = Vec::new();
    logs.push(format!(
        "Retrying {path} with truncated content ({limit} chars).",
        path = snippet.relative_path.display(),
        limit = BUG_FILE_CONTEXT_RETRY_MAX_CHARS
    ));

    let prefix = truncate_text(snippet.content.as_str(), BUG_FILE_CONTEXT_RETRY_MAX_CHARS);
    (
        format!(
            "### {}\n```{}\n{}\n\n... [truncated] ...\n```\n",
            snippet.relative_path.display(),
            snippet.language,
            prefix
        ),
        logs,
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
        "{base_prompt}\n\nPrevious attempt:\n```\n{previous_output}\n```\nThe previous response did not populate the `Threat Model` table. Re-run the task above and respond with the required subsections (Primary components, Trust Boundaries (including a `#### Diagram` mermaid block), Assets, Attacker model, Entry points, Top abuse paths) as `###` headings, followed by a complete Markdown table named `Threat Model` with populated rows (IDs starting at 1, with realistic data)."
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
    let repo_context = repository_summary.trim();
    let repo_context_truncated = truncate_text(repo_context, BUG_REPOSITORY_SUMMARY_MAX_CHARS);
    if repo_context_truncated.len() < repo_context.len() {
        logs.push(format!(
            "Repository context truncated to {BUG_REPOSITORY_SUMMARY_MAX_CHARS} chars for bug analysis."
        ));
    }
    let repository_section = format!("# Repository context\n{repo_context_truncated}\n");
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
            let spec_context = truncate_text(trimmed_spec, BUG_SPEC_CONTEXT_MAX_CHARS);
            if spec_context.len() < trimmed_spec.len() {
                logs.push(format!(
                    "Specification context truncated to {BUG_SPEC_CONTEXT_MAX_CHARS} chars for bug analysis."
                ));
            }
            let available_for_spec = MAX_PROMPT_BYTES.saturating_sub(base_len);
            const SPEC_HEADER: &str = "\n# Specification context\n";
            if available_for_spec > SPEC_HEADER.len() {
                let max_spec_bytes = available_for_spec - SPEC_HEADER.len();
                let mut spec_section = String::from(SPEC_HEADER);
                if spec_context.len() <= max_spec_bytes {
                    spec_section.push_str(spec_context.as_str());
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
                            truncate_to_char_boundary(spec_context.as_str(), available_for_content);
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

fn normalize_web_validation_base_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|err| format!("Invalid target URL: {err}"))?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(format!("Unsupported target URL scheme `{other}`")),
    }
    if url.host_str().is_none() {
        return Err("Target URL must include a host".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Target URL must not include embedded credentials (userinfo)".to_string());
    }

    let mut base = url;
    base.set_path("/");
    base.set_query(None);
    base.set_fragment(None);
    Ok(base)
}

fn resolve_web_validation_target(base_url: &Url, raw_target: Option<&str>) -> Result<Url, String> {
    let raw_target = raw_target.unwrap_or("").trim();
    if raw_target.is_empty() {
        return Ok(base_url.clone());
    }

    let candidate = if raw_target.starts_with("http://") || raw_target.starts_with("https://") {
        Url::parse(raw_target).map_err(|err| format!("Invalid target URL: {err}"))?
    } else {
        base_url
            .join(raw_target)
            .map_err(|err| format!("Invalid target URL path: {err}"))?
    };

    if !candidate.username().is_empty() || candidate.password().is_some() {
        return Err("Target URL must not include embedded credentials (userinfo)".to_string());
    }

    if candidate.origin().ascii_serialization() != base_url.origin().ascii_serialization() {
        return Err(format!(
            "Refusing to validate against non-target origin: {}",
            candidate.origin().ascii_serialization()
        ));
    }

    Ok(candidate)
}

fn parse_web_validation_creds(contents: &str) -> Vec<(String, String)> {
    fn is_valid_header_name(name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        name.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    }

    fn sanitize_header_value(value: &str) -> Option<String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.contains('\n') || trimmed.contains('\r') {
            return None;
        }
        Some(trimmed.to_string())
    }

    let mut headers: Vec<(String, String)> = Vec::new();
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return headers;
    }

    let json = serde_json::from_str::<Value>(trimmed).ok();
    if let Some(value) = json {
        let header_object = match value {
            Value::Object(map) => map
                .get("headers")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or(map),
            other => other.as_object().cloned().unwrap_or_default(),
        };
        if !header_object.is_empty() {
            for (key, value) in header_object {
                if !is_valid_header_name(&key) {
                    continue;
                }
                let Some(raw) = value.as_str() else {
                    continue;
                };
                if let Some(val) = sanitize_header_value(raw) {
                    headers.push((key.trim().to_string(), val));
                }
            }
            return headers;
        }
    }

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !is_valid_header_name(name) {
            continue;
        }
        if let Some(val) = sanitize_header_value(value) {
            headers.push((name.trim().to_string(), val));
        }
    }

    headers
}

fn redactions_for_headers(headers: &[(String, String)]) -> Vec<String> {
    let mut redactions: Vec<String> = Vec::new();
    for (_name, value) in headers {
        let value = value.trim();
        if value.len() >= 8 {
            redactions.push(value.to_string());
        }
        if let Some(token) = value.strip_prefix("Bearer ").map(str::trim)
            && token.len() >= 8
        {
            redactions.push(token.to_string());
        }
    }
    redactions.sort();
    redactions.dedup();
    redactions
}

fn base_url_for_control(target: &str) -> Option<String> {
    let url = Url::parse(target).ok()?;
    let mut control = url;
    control.set_path("/");
    control.set_query(None);
    control.set_fragment(None);
    Some(control.to_string())
}

fn is_local_target_url(target: &str) -> bool {
    let Ok(url) = Url::parse(target) else {
        return false;
    };
    matches!(
        url.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("::1")
    )
}

fn contains_asan_signature(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("addresssanitizer")
        || lower.contains("leaksan")
        || lower.contains("heap-buffer-overflow")
        || lower.contains("stack-buffer-overflow")
        || lower.contains("use-after-free")
}

fn extract_asan_trace_excerpt(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let start = lines
        .iter()
        .position(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("error: addresssanitizer") || lower.contains("error: leaksanitizer")
        })
        .or_else(|| lines.iter().position(|line| contains_asan_signature(line)));
    let start = start?;

    let mut out: Vec<String> = Vec::new();
    let mut captured = 0usize;
    for line in lines.iter().skip(start) {
        if captured >= 200 {
            break;
        }

        let lower = line.to_ascii_lowercase();
        if lower.contains("shadow bytes around") || lower.contains("shadow byte legend") {
            break;
        }

        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }

        out.push(trimmed.to_string());
        captured += 1;

        if lower.contains("aborting") {
            break;
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out.join("\n"))
    }
}

fn resolve_validation_display_path(
    display_path: &str,
    repo_root: Option<&Path>,
) -> Option<PathBuf> {
    let display_path = display_path.trim();
    if display_path.is_empty() {
        return None;
    }

    if let Some(tail) = display_path
        .strip_prefix("~/")
        .or_else(|| display_path.strip_prefix("~\\"))
    {
        return home_dir().map(|dir| dir.join(tail));
    }

    let path = PathBuf::from(display_path);
    if path.is_absolute() {
        Some(path)
    } else {
        repo_root.map(|root| root.join(path))
    }
}

fn is_within_output_root(path: &Path, output_root: &Path) -> bool {
    let Ok(candidate) = path.canonicalize() else {
        return false;
    };
    let Ok(root) = output_root.canonicalize() else {
        return false;
    };
    candidate.starts_with(root)
}

fn read_report_output_prefix(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes: Vec<u8> = Vec::new();
    let max = VALIDATION_REPORT_OUTPUT_MAX_BYTES;
    file.take((max + 1) as u64).read_to_end(&mut bytes).ok()?;
    if bytes.is_empty() {
        return None;
    }

    let truncated = bytes.len() > max;
    if truncated {
        bytes.truncate(max);
    }

    let mut text = String::from_utf8_lossy(&bytes).to_string();
    if text.trim().is_empty() {
        return None;
    }
    if truncated {
        text.push_str("\n\n… (truncated)");
    }
    Some(text)
}

fn read_validation_artifact_for_report(
    display_path: Option<&String>,
    repo_root: Option<&Path>,
    output_root: Option<&Path>,
) -> Option<String> {
    let display_path = display_path?;
    let path = resolve_validation_display_path(display_path, repo_root)?;
    let output_root = output_root?;
    if !is_within_output_root(&path, output_root) {
        return None;
    }
    read_report_output_prefix(&path)
}

fn build_validation_output_block(
    validation: &BugValidationState,
    repo_root: Option<&Path>,
    output_root: Option<&Path>,
    expects_asan: bool,
) -> Option<String> {
    let summary = validation
        .summary
        .as_deref()
        .map(str::trim)
        .filter(|summary| !summary.is_empty());

    let snippet = validation
        .output_snippet
        .as_deref()
        .map(str::trim)
        .filter(|snippet| !snippet.is_empty())
        .map(str::to_string);

    let stderr = read_validation_artifact_for_report(
        validation.stderr_path.as_ref(),
        repo_root,
        output_root,
    )
    .and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let stdout = read_validation_artifact_for_report(
        validation.stdout_path.as_ref(),
        repo_root,
        output_root,
    )
    .and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    let mut sections: Vec<String> = Vec::new();

    if matches!(validation.status, BugValidationStatus::UnableToValidate)
        && let Some(summary) = summary
    {
        sections.push(format!("Explanation: {summary}"));
    }

    if expects_asan {
        let output = snippet
            .as_deref()
            .or(stderr.as_deref())
            .or(stdout.as_deref());
        if let Some(output) = output {
            if let Some(trace) = extract_asan_trace_excerpt(output) {
                sections.push(trace);
            } else {
                sections.push(output.to_string());
            }
        }
    } else {
        match (stderr.as_ref(), stdout.as_ref()) {
            (Some(stderr), Some(stdout)) => {
                sections.push(format!("== STDERR ==\n{stderr}\n\n== STDOUT ==\n{stdout}"));
            }
            _ => {
                let output = if matches!(validation.status, BugValidationStatus::Passed) {
                    stdout.as_ref().or(stderr.as_ref())
                } else {
                    stderr.as_ref().or(stdout.as_ref())
                };
                if let Some(output) = output {
                    sections.push(output.to_string());
                }
            }
        }
    }

    if sections.is_empty() {
        if let Some(snippet) = snippet.as_deref() {
            sections.push(snippet.to_string());
        } else if matches!(validation.status, BugValidationStatus::UnableToValidate)
            && let Some(summary) = summary
        {
            sections.push(format!("Explanation: {summary}"));
        }
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}

fn first_validation_poc_artifact(validation: &BugValidationState) -> Option<String> {
    validation.artifacts.iter().find_map(|artifact| {
        let trimmed = artifact.trim();
        if trimmed.is_empty() {
            return None;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.ends_with(".py") || lower.ends_with(".sh") {
            Some(trimmed.to_string())
        } else {
            None
        }
    })
}

fn extract_exploit_trigger_excerpt(output: &str) -> Option<String> {
    let lines: Vec<&str> = output.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let mut start_idx = None;
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.to_ascii_lowercase().contains("trigger") {
            start_idx = Some(idx);
            break;
        }
    }

    let mut out: Vec<String> = Vec::new();

    let mut in_input_block = false;
    let mut input_lines = 0usize;
    let mut remaining_lines = 12usize;
    let max_chars = 1_200usize;

    let push_line = |line: &str, out: &mut Vec<String>, remaining_lines: &mut usize| {
        if *remaining_lines == 0 {
            return;
        }
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            return;
        }
        if out.iter().any(|existing| existing == trimmed) {
            return;
        }
        let current_chars: usize = out.iter().map(|s| s.len() + 1).sum();
        if current_chars >= max_chars {
            return;
        }
        out.push(trimmed.to_string());
        *remaining_lines = remaining_lines.saturating_sub(1);
    };

    let scan_from = start_idx.unwrap_or(0);
    for line in lines.iter().skip(scan_from) {
        let trimmed = line.trim_end();
        let lowered = trimmed.trim_start().to_ascii_lowercase();

        if (lowered.contains("=== control") || lowered.contains("=== setup")) && !out.is_empty() {
            break;
        }

        if lowered.starts_with("input:") {
            in_input_block = true;
            input_lines = 0;
            push_line("INPUT:", &mut out, &mut remaining_lines);
            continue;
        }

        if in_input_block {
            if trimmed.trim().is_empty() {
                in_input_block = false;
                continue;
            }
            if lowered.contains("=== ") && lowered.contains(" ===") {
                in_input_block = false;
                continue;
            }
            if input_lines < 8 {
                push_line(trimmed, &mut out, &mut remaining_lines);
                input_lines += 1;
            } else {
                in_input_block = false;
            }
            continue;
        }

        let leading = trimmed.trim_start();
        if leading.starts_with("$ ") || leading.starts_with("Run:") {
            push_line(trimmed, &mut out, &mut remaining_lines);
        }

        if remaining_lines == 0 {
            break;
        }
    }

    if out.is_empty() && start_idx.is_some() {
        let mut remaining_lines = 3usize;
        for line in lines.iter() {
            let trimmed = line.trim_end();
            if trimmed.trim_start().starts_with("$ ") {
                push_line(trimmed, &mut out, &mut remaining_lines);
            }
            if remaining_lines == 0 {
                break;
            }
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out.join("\n"))
    }
}

fn infer_exploit_input_kind(
    bug: &SecurityReviewBug,
    base_markdown: &str,
    validation_output: Option<&str>,
) -> &'static str {
    let tag = bug
        .vulnerability_tag
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if tag.contains("idor") {
        return "another user's identifier in an API request";
    }
    if tag.contains("sql-injection") {
        return "a crafted request parameter containing SQL injection payloads";
    }
    if tag.starts_with("path-traversal") {
        return "a crafted path containing traversal sequences (e.g. `../`)";
    }

    let title = bug.title.trim().to_ascii_lowercase();
    let validation = validation_output.unwrap_or("");
    let haystack = format!("{title}\n{tag}\n{base_markdown}\n{validation}").to_ascii_lowercase();

    if haystack.contains("ldap") {
        return "attacker-controlled LDAP/CRL distribution-point metadata";
    }
    if haystack.contains("hkps://")
        || haystack.contains("hkp://")
        || haystack.contains("keyserver")
        || haystack.contains("dirmngr")
    {
        return "a malicious keyserver/network response that the target fetches";
    }
    if haystack.contains(".tar") || haystack.contains("tar") {
        return "a crafted archive (e.g. a `.tar` file)";
    }
    if haystack.contains(".png")
        || haystack.contains(".jpg")
        || haystack.contains(".jpeg")
        || haystack.contains(".gif")
    {
        return "a crafted media file";
    }
    if crash_poc_category(bug).is_some() {
        return "a crafted input that reaches a memory-unsafe parsing/execution path";
    }
    if bug
        .verification_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case("network_api"))
    {
        return "a crafted network request";
    }

    "attacker-controlled input to a shipped entrypoint"
}

fn build_exploit_scenario_block(
    bug: &SecurityReviewBug,
    base_markdown: &str,
    validation_output: Option<&str>,
    poc_artifact: Option<&str>,
) -> Option<String> {
    let trigger_excerpt = validation_output.and_then(extract_exploit_trigger_excerpt);
    if trigger_excerpt.is_none() && poc_artifact.is_none() {
        return None;
    }

    let input_kind = infer_exploit_input_kind(bug, base_markdown, validation_output);

    let mut lines: Vec<String> = Vec::new();
    lines.push("#### Exploit scenario".to_string());
    lines.push(String::new());
    lines.push(format!(
        "An external attacker can realistically trigger this by supplying {input_kind}."
    ));
    lines.push(String::new());

    let status = validation_status_label(&bug.validation);
    lines.push(format!("- **Validation status:** {status}"));

    let impact = bug.impact.lines().next().unwrap_or("").trim();
    if !impact.is_empty() {
        lines.push(format!("- **Impact:** {impact}"));
    }

    if let Some(poc) = poc_artifact {
        lines.push(format!("- **PoC artifact:** `{poc}`"));
    }

    if let Some(excerpt) = trigger_excerpt.as_deref() {
        lines.push("- **Trigger example (from validation output):**".to_string());
        lines.push("```".to_string());
        lines.push(excerpt.trim().to_string());
        lines.push("```".to_string());
    }

    Some(lines.join("\n"))
}

fn looks_like_build_failure(stdout: &str, stderr: &str) -> bool {
    let stdout_lower = stdout.to_ascii_lowercase();
    let stderr_lower = stderr.to_ascii_lowercase();
    let sources = [stdout_lower.as_str(), stderr_lower.as_str()];

    let tool_missing = [
        "command not found: cargo",
        "cargo: command not found",
        "command not found: rustc",
        "rustc: command not found",
        "command not found: make",
        "make: command not found",
        "command not found: go",
        "go: command not found",
        "command not found: npm",
        "npm: command not found",
    ];

    let compile_failures = [
        "error: could not compile",
        "could not compile",
        "failed to compile",
        "failed to run custom build command",
        "linker ",
        "linking with",
        "ld: library not found",
        "collect2: error",
        "cmake error",
        "ninja: build stopped",
        "make: ***",
    ];

    let platform_mismatch = [
        "unsupported platform",
        "not supported on this platform",
        "only supported on",
        "requires macos",
        "requires windows",
        "requires linux",
    ];

    tool_missing
        .iter()
        .chain(compile_failures.iter())
        .chain(platform_mismatch.iter())
        .any(|needle| sources.iter().any(|source| source.contains(needle)))
}

fn classify_python_validation_status(
    expect_asan: bool,
    exit_code: Option<i32>,
    success: bool,
    stdout: &str,
    stderr: &str,
) -> BugValidationStatus {
    let observed_asan =
        expect_asan && (contains_asan_signature(stdout) || contains_asan_signature(stderr));
    if observed_asan {
        return BugValidationStatus::Passed;
    }

    let unable =
        matches!(exit_code, Some(2)) || (!success && looks_like_build_failure(stdout, stderr));
    if unable {
        return BugValidationStatus::UnableToValidate;
    }

    if expect_asan {
        BugValidationStatus::Failed
    } else if success {
        BugValidationStatus::Passed
    } else {
        BugValidationStatus::Failed
    }
}

async fn command_output(mut command: Command) -> Result<std::process::Output, std::io::Error> {
    command.kill_on_drop(true);
    command.output().await
}

enum CommandOutputOutcome {
    Completed(std::process::Output),
    TimedOut,
}

async fn command_output_with_timeout(
    command: Command,
    timeout: Duration,
) -> Result<CommandOutputOutcome, std::io::Error> {
    match tokio::time::timeout(timeout, command_output(command)).await {
        Ok(output) => Ok(CommandOutputOutcome::Completed(output?)),
        Err(_elapsed) => Ok(CommandOutputOutcome::TimedOut),
    }
}

async fn write_validation_output_files(
    work_dir: &Path,
    repo_path: &Path,
    file_stem: &str,
    prefix: &str,
    stdout: &str,
    stderr: &str,
) -> (String, String) {
    let _ = tokio_fs::create_dir_all(work_dir).await;

    let stdout_path = work_dir.join(format!("{file_stem}_{prefix}_stdout.txt"));
    let stderr_path = work_dir.join(format!("{file_stem}_{prefix}_stderr.txt"));

    let _ = tokio_fs::write(&stdout_path, stdout.as_bytes()).await;
    let _ = tokio_fs::write(&stderr_path, stderr.as_bytes()).await;

    (
        display_path_for(&stdout_path, repo_path),
        display_path_for(&stderr_path, repo_path),
    )
}

fn build_bugs_markdown(
    snapshot: &SecurityReviewSnapshot,
    git_link_info: Option<&GitLinkInfo>,
    repo_root: Option<&Path>,
    output_root: Option<&Path>,
) -> String {
    let report_bugs: Vec<BugSnapshot> = snapshot
        .bugs
        .iter()
        .filter(|entry| {
            !matches!(
                entry.bug.severity.trim().to_ascii_lowercase().as_str(),
                "ignore" | "ignored"
            )
        })
        .cloned()
        .collect();
    let bugs: Vec<SecurityReviewBug> = report_bugs.iter().map(|entry| entry.bug.clone()).collect();
    let mut sections: Vec<String> = Vec::new();
    if let Some(table) = make_bug_summary_table_from_bugs(&bugs) {
        sections.push(table);
    }
    let details = render_bug_sections(&report_bugs, git_link_info, repo_root, output_root);
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
    web_validation: Option<WebValidationConfig>,
    command_emitter: CommandStatusEmitter,
) -> BugCommandResult {
    let mut logs: Vec<String> = Vec::new();
    let label = if let Some(rank) = plan.risk_rank {
        format!("#{rank} {}", plan.title)
    } else {
        format!("[{}] {}", plan.summary_id, plan.title)
    };
    let bug_id = plan.risk_rank.unwrap_or(plan.summary_id);
    let bug_work_dir = work_dir.join(format!("bug{bug_id}"));
    let _ = tokio_fs::create_dir_all(&bug_work_dir).await;
    let file_stem = if let Some(rank) = plan.risk_rank {
        format!("bug_rank_{rank}")
    } else {
        format!("bug_{}", plan.summary_id)
    };
    let tool = plan.request.tool.as_str();
    let base_summary = format!("Validation {tool} for {label}");
    logs.push(format!("Running {tool} verification for {label}"));

    let initial_target = plan.request.target.clone().filter(|t| !t.is_empty());
    let mut validation = BugValidationState {
        tool: Some(plan.request.tool.as_str().to_string()),
        target: initial_target,
        ..BugValidationState::default()
    };

    if let Some(output_root) = work_dir.parent() {
        let testing_path = output_root.join("specs").join("TESTING.md");
        if testing_path.exists() {
            let step = format!("Read: `{}`", display_path_for(&testing_path, &repo_path));
            if !validation.repro_steps.iter().any(|s| s == &step) {
                validation.repro_steps.push(step);
            }
        }
    }

    let start = Instant::now();

    match plan.request.tool {
        BugVerificationTool::Curl => {
            let timeout = Duration::from_secs(VALIDATION_CURL_TIMEOUT_SECS);
            let control_summary = format!("{base_summary} (control)");
            let repro_summary = format!("{base_summary} (repro)");
            let Some(target_raw) = plan.request.target.clone().filter(|t| !t.is_empty()) else {
                validation.status = BugValidationStatus::UnableToValidate;
                validation.summary = Some("Missing target URL".to_string());
                logs.push(format!("{label}: no target URL provided for curl"));
                emit_command_error(
                    &command_emitter,
                    repro_summary.as_str(),
                    "Missing target URL",
                );
                validation.run_at = Some(OffsetDateTime::now_utc());
                return BugCommandResult {
                    index: plan.index,
                    validation,
                    logs,
                };
            };
            let target = match web_validation.as_ref() {
                Some(web_validation) => match resolve_web_validation_target(
                    &web_validation.base_url,
                    Some(target_raw.as_str()),
                ) {
                    Ok(url) => url.as_str().to_string(),
                    Err(err) => {
                        validation.status = BugValidationStatus::UnableToValidate;
                        validation.summary = Some(format!(
                            "Refusing to validate against non-target URL: {err}"
                        ));
                        logs.push(format!(
                            "{label}: refusing to validate against non-target url {target_raw}: {err}"
                        ));
                        emit_command_error(
                            &command_emitter,
                            repro_summary.as_str(),
                            validation
                                .summary
                                .as_deref()
                                .unwrap_or("Invalid target URL"),
                        );
                        validation.run_at = Some(OffsetDateTime::now_utc());
                        return BugCommandResult {
                            index: plan.index,
                            validation,
                            logs,
                        };
                    }
                },
                None => {
                    if !is_local_target_url(&target_raw) {
                        validation.status = BugValidationStatus::UnableToValidate;
                        validation.summary = Some(format!(
                            "Refusing to validate against non-local target: {target_raw}"
                        ));
                        logs.push(format!(
                            "{label}: refusing to validate against non-local target {target_raw}"
                        ));
                        emit_command_error(
                            &command_emitter,
                            repro_summary.as_str(),
                            validation
                                .summary
                                .as_deref()
                                .unwrap_or("Invalid local target"),
                        );
                        validation.run_at = Some(OffsetDateTime::now_utc());
                        return BugCommandResult {
                            index: plan.index,
                            validation,
                            logs,
                        };
                    }
                    target_raw
                }
            };
            validation.target = Some(target.clone());

            let control_target = base_url_for_control(&target);
            validation.control_target = control_target.clone();
            if let Some(control_target) = control_target.as_ref() {
                let header_steps = web_validation
                    .as_ref()
                    .filter(|web_validation| !web_validation.headers.is_empty())
                    .map(|web_validation| {
                        web_validation
                            .headers
                            .iter()
                            .map(|(name, _)| format!(" --header '{name}: [REDACTED]'"))
                            .collect::<String>()
                    })
                    .unwrap_or_default();
                let control_command = format!(
                    "curl --silent --show-error --location --max-time 15{header_steps} {control_target}"
                );
                validation
                    .control_steps
                    .push(format!("Run: `{control_command}`"));
                emit_command_start(
                    &command_emitter,
                    control_summary.as_str(),
                    Some(control_command.as_str()),
                );
                let mut control_cmd = Command::new("curl");
                control_cmd
                    .arg("--silent")
                    .arg("--show-error")
                    .arg("--location")
                    .arg("--max-time")
                    .arg("15")
                    .args(
                        web_validation
                            .as_ref()
                            .map(|web_validation| {
                                web_validation
                                    .headers
                                    .iter()
                                    .flat_map(|(name, value)| {
                                        ["--header".to_string(), format!("{name}: {value}")]
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default(),
                    )
                    .arg(control_target)
                    .current_dir(&repo_path);

                match command_output_with_timeout(control_cmd, timeout).await {
                    Ok(CommandOutputOutcome::Completed(output)) => {
                        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        if let Some(web_validation) = web_validation.as_ref() {
                            stdout = web_validation.redact(&stdout);
                            stderr = web_validation.redact(&stderr);
                        }
                        let success = output.status.success();
                        validation.control_summary =
                            Some(summarize_process_output(success, &stdout, &stderr));
                        let snippet_source = if success { &stdout } else { &stderr };
                        let trimmed = snippet_source.trim();
                        if !trimmed.is_empty() {
                            validation.control_output_snippet =
                                Some(truncate_text(trimmed, VALIDATION_OUTPUT_GRAPHEMES));
                        }
                        let preview = validation
                            .control_output_snippet
                            .as_deref()
                            .or(validation.control_summary.as_deref());
                        let state = if success {
                            SecurityReviewCommandState::Matches
                        } else {
                            SecurityReviewCommandState::Error
                        };
                        emit_command_result(
                            &command_emitter,
                            control_summary.as_str(),
                            state,
                            Some(control_command.as_str()),
                            preview,
                        );
                        let (stdout_path, stderr_path) = write_validation_output_files(
                            &bug_work_dir,
                            &repo_path,
                            &file_stem,
                            "control",
                            &stdout,
                            &stderr,
                        )
                        .await;
                        validation.control_stdout_path = Some(stdout_path);
                        validation.control_stderr_path = Some(stderr_path);
                    }
                    Ok(CommandOutputOutcome::TimedOut) => {
                        let summary = format!(
                            "curl control timed out after {}",
                            fmt_elapsed_compact(timeout.as_secs())
                        );
                        validation.control_summary = Some(summary.clone());
                        let (stdout_path, stderr_path) = write_validation_output_files(
                            &bug_work_dir,
                            &repo_path,
                            &file_stem,
                            "control",
                            "",
                            summary.as_str(),
                        )
                        .await;
                        validation.control_stdout_path = Some(stdout_path);
                        validation.control_stderr_path = Some(stderr_path);
                        emit_command_result(
                            &command_emitter,
                            control_summary.as_str(),
                            SecurityReviewCommandState::Error,
                            Some(control_command.as_str()),
                            Some(summary.as_str()),
                        );
                    }
                    Err(err) => {
                        validation.control_summary =
                            Some(format!("Failed to run curl control: {err}"));
                        emit_command_result(
                            &command_emitter,
                            control_summary.as_str(),
                            SecurityReviewCommandState::Error,
                            Some(control_command.as_str()),
                            validation.control_summary.as_deref(),
                        );
                    }
                }
            }

            let header_steps = web_validation
                .as_ref()
                .filter(|web_validation| !web_validation.headers.is_empty())
                .map(|web_validation| {
                    web_validation
                        .headers
                        .iter()
                        .map(|(name, _)| format!(" --header '{name}: [REDACTED]'"))
                        .collect::<String>()
                })
                .unwrap_or_default();
            let repro_command = format!(
                "curl --silent --show-error --location --max-time 15{header_steps} {target}"
            );
            validation
                .repro_steps
                .push(format!("Run: `{repro_command}`"));
            emit_command_start(
                &command_emitter,
                repro_summary.as_str(),
                Some(repro_command.as_str()),
            );

            let mut command = Command::new("curl");
            command
                .arg("--silent")
                .arg("--show-error")
                .arg("--location")
                .arg("--max-time")
                .arg("15")
                .args(
                    web_validation
                        .as_ref()
                        .map(|web_validation| {
                            web_validation
                                .headers
                                .iter()
                                .flat_map(|(name, value)| {
                                    ["--header".to_string(), format!("{name}: {value}")]
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default(),
                )
                .arg(&target)
                .current_dir(&repo_path);

            match command_output_with_timeout(command, timeout).await {
                Ok(CommandOutputOutcome::Completed(output)) => {
                    let duration = start.elapsed();
                    let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    if let Some(web_validation) = web_validation.as_ref() {
                        stdout = web_validation.redact(&stdout);
                        stderr = web_validation.redact(&stderr);
                    }
                    let success = output.status.success();
                    validation.status = if success {
                        BugValidationStatus::Passed
                    } else if matches!(output.status.code(), Some(6 | 7 | 28)) {
                        BugValidationStatus::UnableToValidate
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
                    let preview = validation
                        .output_snippet
                        .as_deref()
                        .or(validation.summary.as_deref());
                    emit_command_result(
                        &command_emitter,
                        repro_summary.as_str(),
                        validation_status_command_state(validation.status),
                        Some(repro_command.as_str()),
                        preview,
                    );
                    let (stdout_path, stderr_path) = write_validation_output_files(
                        &bug_work_dir,
                        &repo_path,
                        &file_stem,
                        "repro",
                        &stdout,
                        &stderr,
                    )
                    .await;
                    validation.stdout_path = Some(stdout_path);
                    validation.stderr_path = Some(stderr_path);
                    logs.push(format!(
                        "{}: curl exited with status {}",
                        label, output.status
                    ));
                }
                Ok(CommandOutputOutcome::TimedOut) => {
                    let duration_label = fmt_elapsed_compact(start.elapsed().as_secs());
                    validation.status = BugValidationStatus::UnableToValidate;
                    let summary = format!(
                        "Timed out after {duration_label} while running curl; unable to validate."
                    );
                    validation.summary = Some(summary.clone());
                    validation.output_snippet =
                        Some(truncate_text(summary.as_str(), VALIDATION_OUTPUT_GRAPHEMES));
                    let (stdout_path, stderr_path) = write_validation_output_files(
                        &bug_work_dir,
                        &repo_path,
                        &file_stem,
                        "repro",
                        "",
                        summary.as_str(),
                    )
                    .await;
                    validation.stdout_path = Some(stdout_path);
                    validation.stderr_path = Some(stderr_path);
                    emit_command_result(
                        &command_emitter,
                        repro_summary.as_str(),
                        SecurityReviewCommandState::Error,
                        Some(repro_command.as_str()),
                        Some(summary.as_str()),
                    );
                    logs.push(format!("{label}: curl timed out after {duration_label}"));
                }
                Err(err) => {
                    validation.status = BugValidationStatus::UnableToValidate;
                    validation.summary = Some(format!("Failed to run curl: {err}"));
                    emit_command_result(
                        &command_emitter,
                        repro_summary.as_str(),
                        SecurityReviewCommandState::Error,
                        Some(repro_command.as_str()),
                        validation.summary.as_deref(),
                    );
                    logs.push(format!("{label}: failed to run curl: {err}"));
                }
            }
        }
        BugVerificationTool::Python => {
            let script_path_owned: Option<PathBuf> =
                if let Some(path) = plan.request.script_path.as_ref() {
                    Some(path.clone())
                } else if let Some(code) = plan.request.script_inline.as_ref() {
                    let _ = tokio_fs::create_dir_all(&bug_work_dir).await;
                    let file_name = if let Some(rank) = plan.risk_rank {
                        format!("bug_rank_{rank}.py")
                    } else {
                        format!("bug_{}.py", plan.summary_id)
                    };
                    let temp_path = bug_work_dir.join(file_name);
                    if let Err(err) = tokio_fs::write(&temp_path, code.as_bytes()).await {
                        validation.status = BugValidationStatus::UnableToValidate;
                        validation.summary = Some(format!(
                            "Failed to write inline python to {}: {err}",
                            temp_path.display()
                        ));
                        logs.push(format!(
                            "{}: failed to write python script {}: {err}",
                            label,
                            temp_path.display()
                        ));
                        emit_command_error(
                            &command_emitter,
                            base_summary.as_str(),
                            validation
                                .summary
                                .as_deref()
                                .unwrap_or("Failed to write python script"),
                        );
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
                validation.status = BugValidationStatus::UnableToValidate;
                validation.summary = Some("Missing python script path".to_string());
                logs.push(format!("{label}: no python script provided"));
                emit_command_error(
                    &command_emitter,
                    base_summary.as_str(),
                    "Missing python script path",
                );
                validation.run_at = Some(OffsetDateTime::now_utc());
                return BugCommandResult {
                    index: plan.index,
                    validation,
                    logs,
                };
            };

            let display_script = display_path_for(script_path, &repo_path);
            validation.artifacts.push(display_script.clone());
            if !script_path.exists() {
                validation.status = BugValidationStatus::UnableToValidate;
                validation.summary =
                    Some(format!("Python script {} not found", script_path.display()));
                logs.push(format!(
                    "{}: python script {} not found",
                    label,
                    script_path.display()
                ));
                emit_command_error(
                    &command_emitter,
                    base_summary.as_str(),
                    validation
                        .summary
                        .as_deref()
                        .unwrap_or("Python script not found"),
                );
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
            if let Some(web_validation) = web_validation.as_ref() {
                command.env("CODEX_WEB_TARGET_URL", web_validation.base_url.as_str());
                command.env("CODEX_WEB_TARGET_ORIGIN", web_validation.origin());
                let headers: HashMap<String, String> =
                    web_validation.headers.iter().cloned().collect();
                if let Ok(headers_json) = serde_json::to_string(&headers) {
                    command.env("CODEX_WEB_HEADERS_JSON", headers_json);
                }
                if let Some(output_root) = work_dir.parent() {
                    command.env(
                        "CODEX_WEB_CREDS_OUT_PATH",
                        output_root
                            .join("specs")
                            .join(WEB_VALIDATION_CREDS_FILE_NAME),
                    );
                    command.env(
                        "CODEX_WEB_TESTING_MD_PATH",
                        output_root.join("specs").join("TESTING.md"),
                    );
                }
            }
            command.current_dir(&repo_path);

            let mut cmd = format!("python {display_script}");
            if let Some(target) = plan.request.target.as_ref().filter(|t| !t.is_empty()) {
                cmd.push(' ');
                cmd.push_str(target);
            }
            validation.repro_steps.push(format!("Run: `{cmd}`"));
            emit_command_start(&command_emitter, base_summary.as_str(), Some(cmd.as_str()));

            let timeout = Duration::from_secs(VALIDATION_EXEC_TIMEOUT_SECS);
            match command_output_with_timeout(command, timeout).await {
                Ok(CommandOutputOutcome::Completed(output)) => {
                    let duration = start.elapsed();
                    let stdout_raw = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr_raw = String::from_utf8_lossy(&output.stderr).to_string();
                    let success = output.status.success();
                    let duration_label = fmt_elapsed_compact(duration.as_secs());
                    let exit_code = output.status.code();
                    let observed_asan = plan.expect_asan
                        && (contains_asan_signature(&stdout_raw)
                            || contains_asan_signature(&stderr_raw));
                    validation.status = classify_python_validation_status(
                        plan.expect_asan,
                        exit_code,
                        success,
                        &stdout_raw,
                        &stderr_raw,
                    );
                    let mut stdout = stdout_raw;
                    let mut stderr = stderr_raw;
                    if let Some(web_validation) = web_validation.as_ref() {
                        stdout = web_validation.redact(&stdout);
                        stderr = web_validation.redact(&stderr);
                    }

                    if matches!(validation.status, BugValidationStatus::UnableToValidate) {
                        let summary_line = summarize_process_output(false, &stdout, &stderr);
                        validation.summary = Some(format!("{summary_line} · {duration_label}"));
                    } else if plan.expect_asan {
                        if observed_asan {
                            validation.summary =
                                Some(format!("ASan signature observed · {duration_label}"));
                        } else {
                            let summary_line = summarize_process_output(success, &stdout, &stderr);
                            validation.summary = Some(format!(
                                "{summary_line} (no ASan signature) · {duration_label}"
                            ));
                        }
                    } else {
                        let summary_line = summarize_process_output(
                            matches!(validation.status, BugValidationStatus::Passed),
                            &stdout,
                            &stderr,
                        );
                        validation.summary = Some(format!("{summary_line} · {duration_label}"));
                    }

                    let snippet_source = if plan.expect_asan {
                        if contains_asan_signature(&stderr) {
                            &stderr
                        } else if stderr.trim().is_empty() || contains_asan_signature(&stdout) {
                            &stdout
                        } else {
                            &stderr
                        }
                    } else if matches!(validation.status, BugValidationStatus::Passed) {
                        &stdout
                    } else {
                        &stderr
                    };
                    let trimmed_snippet = snippet_source.trim();
                    if !trimmed_snippet.is_empty() {
                        let snippet = if plan.expect_asan {
                            extract_asan_trace_excerpt(trimmed_snippet)
                                .unwrap_or_else(|| trimmed_snippet.to_string())
                        } else {
                            trimmed_snippet.to_string()
                        };
                        validation.output_snippet =
                            Some(truncate_text(&snippet, VALIDATION_OUTPUT_GRAPHEMES));
                    }
                    let preview = validation
                        .output_snippet
                        .as_deref()
                        .or(validation.summary.as_deref());
                    emit_command_result(
                        &command_emitter,
                        base_summary.as_str(),
                        validation_status_command_state(validation.status),
                        Some(cmd.as_str()),
                        preview,
                    );
                    let (stdout_path, stderr_path) = write_validation_output_files(
                        &bug_work_dir,
                        &repo_path,
                        &file_stem,
                        "repro",
                        &stdout,
                        &stderr,
                    )
                    .await;
                    validation.stdout_path = Some(stdout_path);
                    validation.stderr_path = Some(stderr_path);
                    logs.push(format!(
                        "{}: python exited with status {}",
                        label, output.status
                    ));
                }
                Ok(CommandOutputOutcome::TimedOut) => {
                    let duration_label = fmt_elapsed_compact(start.elapsed().as_secs());
                    validation.status = BugValidationStatus::UnableToValidate;
                    let summary = format!(
                        "Timed out after {duration_label} while running python validation; unable to validate."
                    );
                    validation.summary = Some(summary.clone());
                    validation.output_snippet =
                        Some(truncate_text(summary.as_str(), VALIDATION_OUTPUT_GRAPHEMES));
                    let (stdout_path, stderr_path) = write_validation_output_files(
                        &bug_work_dir,
                        &repo_path,
                        &file_stem,
                        "repro",
                        "",
                        summary.as_str(),
                    )
                    .await;
                    validation.stdout_path = Some(stdout_path);
                    validation.stderr_path = Some(stderr_path);
                    emit_command_result(
                        &command_emitter,
                        base_summary.as_str(),
                        SecurityReviewCommandState::Error,
                        Some(cmd.as_str()),
                        Some(summary.as_str()),
                    );
                    logs.push(format!("{label}: python timed out after {duration_label}"));
                }
                Err(err) => {
                    validation.status = BugValidationStatus::UnableToValidate;
                    validation.summary = Some(format!("Failed to run python: {err}"));
                    emit_command_result(
                        &command_emitter,
                        base_summary.as_str(),
                        SecurityReviewCommandState::Error,
                        Some(cmd.as_str()),
                        validation.summary.as_deref(),
                    );
                    logs.push(format!("{label}: failed to run python: {err}"));
                }
            }
        }
        BugVerificationTool::Playwright => {
            let timeout = Duration::from_secs(VALIDATION_PLAYWRIGHT_TIMEOUT_SECS);
            let control_summary = format!("{base_summary} (control)");
            let repro_summary = format!("{base_summary} (repro)");
            let Some(target_raw) = plan.request.target.clone().filter(|t| !t.is_empty()) else {
                validation.status = BugValidationStatus::UnableToValidate;
                validation.summary = Some("Missing target URL".to_string());
                logs.push(format!("{label}: no target URL provided for playwright"));
                emit_command_error(
                    &command_emitter,
                    repro_summary.as_str(),
                    "Missing target URL",
                );
                validation.run_at = Some(OffsetDateTime::now_utc());
                return BugCommandResult {
                    index: plan.index,
                    validation,
                    logs,
                };
            };
            let target = match web_validation.as_ref() {
                Some(web_validation) => match resolve_web_validation_target(
                    &web_validation.base_url,
                    Some(target_raw.as_str()),
                ) {
                    Ok(url) => url.as_str().to_string(),
                    Err(err) => {
                        validation.status = BugValidationStatus::UnableToValidate;
                        validation.summary = Some(format!(
                            "Refusing to validate against non-target URL: {err}"
                        ));
                        logs.push(format!(
                            "{label}: refusing to validate against non-target url {target_raw}: {err}"
                        ));
                        emit_command_error(
                            &command_emitter,
                            repro_summary.as_str(),
                            validation
                                .summary
                                .as_deref()
                                .unwrap_or("Invalid target URL"),
                        );
                        validation.run_at = Some(OffsetDateTime::now_utc());
                        return BugCommandResult {
                            index: plan.index,
                            validation,
                            logs,
                        };
                    }
                },
                None => {
                    if !is_local_target_url(&target_raw) {
                        validation.status = BugValidationStatus::UnableToValidate;
                        validation.summary = Some(format!(
                            "Refusing to validate against non-local target: {target_raw}"
                        ));
                        logs.push(format!(
                            "{label}: refusing to validate against non-local target {target_raw}"
                        ));
                        emit_command_error(
                            &command_emitter,
                            repro_summary.as_str(),
                            validation
                                .summary
                                .as_deref()
                                .unwrap_or("Invalid local target"),
                        );
                        validation.run_at = Some(OffsetDateTime::now_utc());
                        return BugCommandResult {
                            index: plan.index,
                            validation,
                            logs,
                        };
                    }
                    target_raw
                }
            };
            validation.target = Some(target.clone());
            let _ = tokio_fs::create_dir_all(&bug_work_dir).await;
            let screenshot_path = bug_work_dir.join(format!("{file_stem}.png"));
            let script_path = bug_work_dir.join("playwright_screenshot.js");
            const PLAYWRIGHT_SCREENSHOT_SCRIPT: &str = r#"
const { chromium } = require("playwright");

function originOf(urlString) {
  try {
    return new URL(urlString).origin;
  } catch {
    return null;
  }
}

(async () => {
  const target = process.argv[2];
  const screenshotPath = process.argv[3];

  if (!target || !screenshotPath) {
    console.error("usage: playwright_screenshot.js <target_url> <screenshot_path>");
    process.exit(2);
  }

  const allowedOrigin = process.env.CODEX_WEB_ALLOWED_ORIGIN || "";
  const headersJson = process.env.CODEX_WEB_HEADERS_JSON || "{}";
  let extraHeaders = {};
  try {
    extraHeaders = JSON.parse(headersJson) || {};
  } catch {
    extraHeaders = {};
  }

  if (allowedOrigin && originOf(target) !== allowedOrigin) {
    console.error(`Refusing to navigate to non-target origin: ${originOf(target)}`);
    process.exit(2);
  }

  const browser = await chromium.launch();
  const context = await browser.newContext();
  const page = await context.newPage();

  await page.route("**/*", async (route) => {
    const request = route.request();
    const requestOrigin = originOf(request.url());
    if (allowedOrigin && requestOrigin === allowedOrigin) {
      const merged = { ...request.headers(), ...extraHeaders };
      await route.continue({ headers: merged });
    } else {
      await route.continue();
    }
  });

  await page.goto(target, { waitUntil: "networkidle", timeout: 15000 });

  if (allowedOrigin) {
    const finalOrigin = originOf(page.url());
    if (finalOrigin !== allowedOrigin) {
      throw new Error(`Refusing to follow redirect to non-target origin: ${finalOrigin}`);
    }
  }

  await page.screenshot({ path: screenshotPath, fullPage: true });
  console.log(`Saved screenshot to ${screenshotPath}`);
  await browser.close();
})().catch((err) => {
  console.error(String(err && err.stack ? err.stack : err));
  process.exit(1);
});
"#;
            let _ = tokio_fs::write(&script_path, PLAYWRIGHT_SCREENSHOT_SCRIPT).await;
            let display_script = display_path_for(&script_path, &repo_path);
            if !validation.artifacts.iter().any(|a| a == &display_script) {
                validation.artifacts.push(display_script.clone());
            }

            let headers_json = web_validation.as_ref().map(|web_validation| {
                let headers: HashMap<String, String> =
                    web_validation.headers.iter().cloned().collect();
                serde_json::to_string(&headers).unwrap_or_else(|_| "{}".to_string())
            });
            let allowed_origin = web_validation.as_ref().map(WebValidationConfig::origin);

            let control_target = base_url_for_control(&target);
            validation.control_target = control_target.clone();
            if let Some(control_target) = control_target.as_ref() {
                let control_screenshot_path = bug_work_dir.join(format!("{file_stem}_control.png"));
                let display_control_screenshot =
                    display_path_for(&control_screenshot_path, &repo_path);
                let control_command = format!(
                    "npx --yes -p playwright node {display_script} {control_target} {display_control_screenshot}"
                );
                validation
                    .control_steps
                    .push(format!("Run: `{control_command}`"));
                emit_command_start(
                    &command_emitter,
                    control_summary.as_str(),
                    Some(control_command.as_str()),
                );

                let mut control_cmd = Command::new("npx");
                control_cmd
                    .arg("--yes")
                    .arg("-p")
                    .arg("playwright")
                    .arg("node")
                    .arg(&script_path)
                    .arg(control_target)
                    .arg(&control_screenshot_path)
                    .current_dir(&repo_path);
                if let Some(origin) = allowed_origin.as_ref() {
                    control_cmd.env("CODEX_WEB_ALLOWED_ORIGIN", origin);
                }
                if let Some(json) = headers_json.as_ref() {
                    control_cmd.env("CODEX_WEB_HEADERS_JSON", json);
                }

                match command_output_with_timeout(control_cmd, timeout).await {
                    Ok(CommandOutputOutcome::Completed(output)) => {
                        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        if let Some(web_validation) = web_validation.as_ref() {
                            stdout = web_validation.redact(&stdout);
                            stderr = web_validation.redact(&stderr);
                        }
                        let success = output.status.success();
                        if success {
                            validation.control_summary = Some(format!(
                                "Saved control screenshot to {display_control_screenshot}"
                            ));
                            validation
                                .artifacts
                                .push(display_control_screenshot.clone());
                        } else {
                            validation.control_summary =
                                Some(summarize_process_output(success, &stdout, &stderr));
                        }
                        let snippet_source = if success { &stdout } else { &stderr };
                        let trimmed = snippet_source.trim();
                        if !trimmed.is_empty() {
                            validation.control_output_snippet =
                                Some(truncate_text(trimmed, VALIDATION_OUTPUT_GRAPHEMES));
                        }
                        let preview = validation
                            .control_output_snippet
                            .as_deref()
                            .or(validation.control_summary.as_deref());
                        let state = if success {
                            SecurityReviewCommandState::Matches
                        } else {
                            SecurityReviewCommandState::Error
                        };
                        emit_command_result(
                            &command_emitter,
                            control_summary.as_str(),
                            state,
                            Some(control_command.as_str()),
                            preview,
                        );
                        let (stdout_path, stderr_path) = write_validation_output_files(
                            &bug_work_dir,
                            &repo_path,
                            &file_stem,
                            "control",
                            &stdout,
                            &stderr,
                        )
                        .await;
                        validation.control_stdout_path = Some(stdout_path);
                        validation.control_stderr_path = Some(stderr_path);
                    }
                    Ok(CommandOutputOutcome::TimedOut) => {
                        let summary = format!(
                            "playwright control timed out after {}",
                            fmt_elapsed_compact(timeout.as_secs())
                        );
                        validation.control_summary = Some(summary.clone());
                        let (stdout_path, stderr_path) = write_validation_output_files(
                            &bug_work_dir,
                            &repo_path,
                            &file_stem,
                            "control",
                            "",
                            summary.as_str(),
                        )
                        .await;
                        validation.control_stdout_path = Some(stdout_path);
                        validation.control_stderr_path = Some(stderr_path);
                        emit_command_result(
                            &command_emitter,
                            control_summary.as_str(),
                            SecurityReviewCommandState::Error,
                            Some(control_command.as_str()),
                            Some(summary.as_str()),
                        );
                    }
                    Err(err) => {
                        validation.control_summary =
                            Some(format!("Failed to run playwright control: {err}"));
                        emit_command_result(
                            &command_emitter,
                            control_summary.as_str(),
                            SecurityReviewCommandState::Error,
                            Some(control_command.as_str()),
                            validation.control_summary.as_deref(),
                        );
                    }
                }
            }

            let display_screenshot = display_path_for(&screenshot_path, &repo_path);
            let repro_command = format!(
                "npx --yes -p playwright node {display_script} {target} {display_screenshot}"
            );
            validation
                .repro_steps
                .push(format!("Run: `{repro_command}`"));
            emit_command_start(
                &command_emitter,
                repro_summary.as_str(),
                Some(repro_command.as_str()),
            );

            let mut command = Command::new("npx");
            command
                .arg("--yes")
                .arg("-p")
                .arg("playwright")
                .arg("node")
                .arg(&script_path)
                .arg(&target)
                .arg(&screenshot_path)
                .current_dir(&repo_path);
            if let Some(origin) = allowed_origin.as_ref() {
                command.env("CODEX_WEB_ALLOWED_ORIGIN", origin);
            }
            if let Some(json) = headers_json.as_ref() {
                command.env("CODEX_WEB_HEADERS_JSON", json);
            }

            match command_output_with_timeout(command, timeout).await {
                Ok(CommandOutputOutcome::Completed(output)) => {
                    let duration = start.elapsed();
                    let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    if let Some(web_validation) = web_validation.as_ref() {
                        stdout = web_validation.redact(&stdout);
                        stderr = web_validation.redact(&stderr);
                    }
                    let success = output.status.success();
                    validation.status = if success {
                        BugValidationStatus::Passed
                    } else {
                        BugValidationStatus::UnableToValidate
                    };
                    let duration_label = fmt_elapsed_compact(duration.as_secs());
                    if success {
                        validation.summary = Some(format!(
                            "Saved screenshot to {} · {duration_label}",
                            display_path_for(&screenshot_path, &repo_path)
                        ));
                        validation
                            .artifacts
                            .push(display_path_for(&screenshot_path, &repo_path));
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
                    let preview = validation
                        .output_snippet
                        .as_deref()
                        .or(validation.summary.as_deref());
                    emit_command_result(
                        &command_emitter,
                        repro_summary.as_str(),
                        validation_status_command_state(validation.status),
                        Some(repro_command.as_str()),
                        preview,
                    );
                    let (stdout_path, stderr_path) = write_validation_output_files(
                        &bug_work_dir,
                        &repo_path,
                        &file_stem,
                        "repro",
                        &stdout,
                        &stderr,
                    )
                    .await;
                    validation.stdout_path = Some(stdout_path);
                    validation.stderr_path = Some(stderr_path);
                    logs.push(format!(
                        "{}: playwright exited with status {}",
                        label, output.status
                    ));
                }
                Ok(CommandOutputOutcome::TimedOut) => {
                    let duration_label = fmt_elapsed_compact(start.elapsed().as_secs());
                    validation.status = BugValidationStatus::UnableToValidate;
                    let summary = format!(
                        "Timed out after {duration_label} while running playwright; unable to validate."
                    );
                    validation.summary = Some(summary.clone());
                    validation.output_snippet =
                        Some(truncate_text(summary.as_str(), VALIDATION_OUTPUT_GRAPHEMES));
                    let (stdout_path, stderr_path) = write_validation_output_files(
                        &bug_work_dir,
                        &repo_path,
                        &file_stem,
                        "repro",
                        "",
                        summary.as_str(),
                    )
                    .await;
                    validation.stdout_path = Some(stdout_path);
                    validation.stderr_path = Some(stderr_path);
                    emit_command_result(
                        &command_emitter,
                        repro_summary.as_str(),
                        SecurityReviewCommandState::Error,
                        Some(repro_command.as_str()),
                        Some(summary.as_str()),
                    );
                    logs.push(format!(
                        "{label}: playwright timed out after {duration_label}"
                    ));
                }
                Err(err) => {
                    validation.status = BugValidationStatus::UnableToValidate;
                    validation.summary = Some(format!("Failed to run playwright: {err}"));
                    emit_command_result(
                        &command_emitter,
                        repro_summary.as_str(),
                        SecurityReviewCommandState::Error,
                        Some(repro_command.as_str()),
                        validation.summary.as_deref(),
                    );
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

async fn verify_bugs(
    batch: BugVerificationBatchRequest,
    command_emitter: CommandStatusEmitter,
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
    } else {
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
                expect_asan: expects_asan_for_bug(&entry.bug),
            });
        }

        // Ensure work dir exists for artifacts/scripts
        let _ = tokio_fs::create_dir_all(&batch.work_dir).await;

        let mut command_results: Vec<BugCommandResult> = Vec::new();
        let web_validation = batch.web_validation.clone();
        let mut futures = futures::stream::iter(plans.into_iter().map(|plan| {
            let repo_path = batch.repo_path.clone();
            let work_dir = batch.work_dir.clone();
            let web_validation = web_validation.clone();
            let command_emitter = command_emitter.clone();
            async move {
                execute_bug_command(plan, repo_path, work_dir, web_validation, command_emitter)
                    .await
            }
        }))
        .buffer_unordered(VALIDATION_EXEC_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

        command_results.append(&mut futures);

        for result in command_results {
            if let Some(entry) = snapshot.bugs.get_mut(result.index) {
                entry.bug.validation = result.validation;
                logs.extend(result.logs);
            }
        }
    }

    let git_link_info = build_git_link_info(&batch.repo_path).await;
    let bugs_markdown = build_bugs_markdown(
        &snapshot,
        git_link_info.as_ref(),
        Some(batch.repo_path.as_path()),
        batch.work_dir.parent(),
    );

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
struct ValidationPlanItem {
    id_kind: String,
    #[serde(default)]
    id_value: Option<usize>,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    script: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    testing_md_additions: Option<String>,
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

fn is_crypto_validation_vuln_tag(tag: &str) -> bool {
    let tag = tag.trim().to_ascii_lowercase();
    if tag.is_empty() {
        return false;
    }

    matches!(
        tag.as_str(),
        "crypto"
            | "cryptography"
            | "crypto-misuse"
            | "weak-crypto"
            | "sig-bypass"
            | "signature-bypass"
            | "jwt-alg-none"
            | "jwt-alg-downgrade"
            | "alg-downgrade"
            | "algo-downgrade"
            | "aead-misuse"
            | "mac-misuse"
            | "nonce-reuse"
            | "iv-reuse"
            | "weak-rng"
            | "insecure-rng"
    ) || tag.starts_with("crypto-")
        || tag.starts_with("jwt-")
        || tag.starts_with("jws-")
        || tag.starts_with("jwe-")
        || tag.starts_with("sig-")
        || tag.starts_with("signature-")
        || tag.starts_with("mac-")
        || tag.starts_with("aead-")
        || tag.starts_with("nonce-")
        || tag.starts_with("iv-")
        || tag.starts_with("tls-")
        || tag.starts_with("x509-")
        || tag.contains("-crypto-")
        || tag.contains("-jwt-")
        || tag.contains("-jws-")
        || tag.contains("-jwe-")
        || tag.contains("-sig-")
        || tag.contains("-signature-")
        || tag.contains("-mac-")
        || tag.contains("-aead-")
        || tag.contains("-nonce-")
        || tag.contains("-iv-")
        || tag.contains("-tls-")
        || tag.contains("-x509-")
        || tag.ends_with("-crypto")
        || tag.ends_with("-jwt")
        || tag.ends_with("-jws")
        || tag.ends_with("-jwe")
        || tag.ends_with("-sig")
        || tag.ends_with("-signature")
        || tag.ends_with("-mac")
        || tag.ends_with("-aead")
        || tag.ends_with("-nonce")
        || tag.ends_with("-iv")
        || tag.ends_with("-tls")
        || tag.ends_with("-x509")
}

fn is_ssrf_validation_vuln_tag(tag: &str) -> bool {
    let tag = tag.trim().to_ascii_lowercase();
    if tag.is_empty() {
        return false;
    }

    matches!(
        tag.as_str(),
        "ssrf" | "server-side-request-forgery" | "server_side_request_forgery"
    ) || tag.starts_with("ssrf-")
        || tag.contains("-ssrf-")
        || tag.ends_with("-ssrf")
}

fn is_rce_validation_vuln_tag(tag: &str) -> bool {
    let tag = tag.trim().to_ascii_lowercase();
    if tag.is_empty() {
        return false;
    }

    matches!(
        tag.as_str(),
        "rce" | "remote-code-execution" | "remote_code_execution" | "rce-bin" | "rce_bin"
    ) || tag.starts_with("rce-")
        || tag.contains("-rce-")
        || tag.ends_with("-rce")
}

fn is_priority_validation_vuln_tag(tag: &str) -> bool {
    let tag = tag.trim().to_ascii_lowercase();
    if tag.is_empty() {
        return false;
    }
    matches!(
        tag.as_str(),
        "idor"
            | "auth-bypass"
            | "authn-bypass"
            | "authz-bypass"
            | "missing-authz-check"
            | "sql-injection"
            | "xxe"
            | "ssrf"
            | "rce"
            | "rce-bin"
            | "rce_bin"
    ) || tag.starts_with("path-traversal")
        || is_crypto_validation_vuln_tag(tag.as_str())
        || is_ssrf_validation_vuln_tag(tag.as_str())
        || is_rce_validation_vuln_tag(tag.as_str())
}

fn has_verification_type(bug: &SecurityReviewBug, needle: &str) -> bool {
    bug.verification_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case(needle))
}

fn expects_asan_for_bug(bug: &SecurityReviewBug) -> bool {
    has_verification_type(bug, "crash_poc_release")
        || has_verification_type(bug, "crash_poc_release_bin")
}

fn crash_poc_category(bug: &SecurityReviewBug) -> Option<&'static str> {
    if has_verification_type(bug, "crash_poc_release")
        || has_verification_type(bug, "crash_poc_release_bin")
    {
        Some("crash_poc_release")
    } else if has_verification_type(bug, "crash_poc_func") {
        Some("crash_poc_func")
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValidationPromptKind {
    Crash,
    RceBin,
    Ssrf,
    Crypto,
    Generic,
}

fn validation_prompt_kind(bug: &SecurityReviewBug, vuln_tag: &str) -> ValidationPromptKind {
    if expects_asan_for_bug(bug) {
        return ValidationPromptKind::Crash;
    }
    if has_verification_type(bug, "rce_bin") || is_rce_validation_vuln_tag(vuln_tag) {
        return ValidationPromptKind::RceBin;
    }
    if has_verification_type(bug, "ssrf") || is_ssrf_validation_vuln_tag(vuln_tag) {
        return ValidationPromptKind::Ssrf;
    }
    if has_verification_type(bug, "crypto") || is_crypto_validation_vuln_tag(vuln_tag) {
        return ValidationPromptKind::Crypto;
    }
    ValidationPromptKind::Generic
}

#[derive(Clone, Debug)]
struct ValidationFinding {
    id: BugIdentifier,
    label: String,
    context: String,
    prompt_kind: ValidationPromptKind,
}

struct ValidationFindingsContext {
    findings: Vec<ValidationFinding>,
    ids: Vec<BugIdentifier>,
}

fn build_validation_findings_context(
    snapshot: &SecurityReviewSnapshot,
    include_web_browser: bool,
) -> ValidationFindingsContext {
    let mut selected: Vec<&BugSnapshot> = snapshot
        .bugs
        .iter()
        .filter(|bug| is_high_risk(&bug.bug))
        .collect();
    selected.sort_by(|left, right| {
        let left_rank = left.bug.risk_rank.unwrap_or(usize::MAX);
        let right_rank = right.bug.risk_rank.unwrap_or(usize::MAX);
        (left_rank, left.bug.summary_id).cmp(&(right_rank, right.bug.summary_id))
    });

    let mut findings: Vec<ValidationFinding> = Vec::new();
    let mut ids: Vec<BugIdentifier> = Vec::new();
    for item in selected {
        let label = if let Some(rank) = item.bug.risk_rank {
            format!("#{rank} {}", item.bug.title)
        } else {
            format!("[{}] {}", item.bug.summary_id, item.bug.title)
        };
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
        let vuln_tag = item
            .bug
            .vulnerability_tag
            .clone()
            .or_else(|| {
                extract_vulnerability_tag_from_bug_markdown(item.original_markdown.as_str())
            })
            .unwrap_or_default();
        let prompt_kind = validation_prompt_kind(&item.bug, vuln_tag.as_str());
        let crash_poc_category_line = crash_poc_category(&item.bug)
            .map(|category| format!("\n  crash_poc_category: {category}"))
            .unwrap_or_default();
        let expects_asan_line = format!("\n  expects_asan: {}", expects_asan_for_bug(&item.bug));
        let web_validation_enabled_line = if has_verification_type(&item.bug, "web_browser") {
            format!("\n  web_validation_enabled: {include_web_browser}")
        } else {
            String::new()
        };
        let (id_kind, id_value, identifier) = if let Some(risk_rank) = item.bug.risk_rank {
            ("risk_rank", risk_rank, BugIdentifier::RiskRank(risk_rank))
        } else {
            (
                "summary_id",
                item.bug.summary_id,
                BugIdentifier::SummaryId(item.bug.summary_id),
            )
        };
        ids.push(identifier);
        // Include the original markdown so the model can infer concrete targets
        let context = format!(
            "- id_kind: {id_kind}\n  id_value: {id_value}\n  risk_rank: {rank}\n  title: {title}\n  severity: {severity}\n  vuln_tag: {vuln_tag}\n  verification_types: {types}{crash_poc_category_line}{expects_asan_line}{web_validation_enabled_line}\n  details:\n{details}\n---\n",
            title = item.bug.title,
            severity = item.bug.severity,
            types = types,
            crash_poc_category_line = crash_poc_category_line,
            expects_asan_line = expects_asan_line,
            web_validation_enabled_line = web_validation_enabled_line,
            details = indent_block(&item.original_markdown, 2)
        );
        findings.push(ValidationFinding {
            id: identifier,
            label,
            context,
            prompt_kind,
        });
    }

    ValidationFindingsContext { findings, ids }
}

fn indent_block(s: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    s.lines()
        .map(|l| format!("{pad}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_validation_plan_item(raw: &str) -> Option<ValidationPlanItem> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    for line in trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Ok(item) = serde_json::from_str::<ValidationPlanItem>(line) {
            return Some(item);
        }
    }

    for snippet in extract_json_objects(trimmed) {
        if let Ok(item) = serde_json::from_str::<ValidationPlanItem>(&snippet) {
            return Some(item);
        }
    }

    None
}

#[derive(Debug, Clone, Deserialize)]
struct ValidationTargetPrepModelOutput {
    outcome: Option<String>,
    summary: Option<String>,
    #[serde(default)]
    local_build_ok: bool,
    #[serde(default)]
    local_run_ok: bool,
    #[serde(default)]
    docker_build_ok: bool,
    #[serde(default)]
    docker_run_ok: bool,
    #[serde(default)]
    local_build_command: Option<String>,
    #[serde(default)]
    local_smoke_command: Option<String>,
    #[serde(default)]
    local_entrypoint: Option<String>,
    #[serde(default)]
    dockerfile_path: Option<String>,
    #[serde(default)]
    docker_image_tag: Option<String>,
    #[serde(default)]
    docker_build_command: Option<String>,
    #[serde(default)]
    docker_smoke_command: Option<String>,
    #[serde(default)]
    testing_md_additions: Option<String>,
}

fn parse_validation_target_prep_output(raw: &str) -> Option<ValidationTargetPrepModelOutput> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(item) = serde_json::from_str::<ValidationTargetPrepModelOutput>(trimmed) {
        return Some(item);
    }

    for snippet in extract_json_objects(trimmed) {
        if let Ok(item) = serde_json::from_str::<ValidationTargetPrepModelOutput>(&snippet) {
            return Some(item);
        }
    }

    None
}

struct ValidationPlanAgentOutput {
    text: String,
    logs: Vec<String>,
}

struct ValidationTargetPrepAgentOutput {
    text: String,
    logs: Vec<String>,
    exec_commands: Vec<ValidationPrepExecCommand>,
}

struct ValidationPrepExecCommand {
    command: String,
    exit_code: i32,
    duration_secs: u64,
}

struct ValidationTargetPrepFailure {
    message: String,
    logs: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
async fn run_validation_target_prep_agent(
    config: &Config,
    provider: &ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    repo_root: &Path,
    prompt: String,
    progress_sender: Option<AppEventSender>,
    log_sink: Option<Arc<SecurityReviewLogSink>>,
    metrics: Arc<ReviewMetrics>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
) -> Result<ValidationTargetPrepAgentOutput, ValidationTargetPrepFailure> {
    let mut logs: Vec<String> = Vec::new();
    push_progress_log(
        &progress_sender,
        &log_sink,
        &mut logs,
        "Preparing runnable validation targets...".to_string(),
    );
    let command_emitter = CommandStatusEmitter::new(progress_sender.clone(), metrics.clone());

    let mut prep_config = config.clone();
    prep_config.model = Some(model.to_string());
    prep_config.model_reasoning_effort =
        normalize_reasoning_effort_for_model(model, reasoning_effort);
    prep_config.model_provider = provider.clone();
    prep_config.base_instructions = Some(VALIDATION_TARGET_PREP_SYSTEM_PROMPT.to_string());
    prep_config.user_instructions = None;
    prep_config.developer_instructions = None;
    prep_config.compact_prompt = None;
    prep_config.cwd = repo_root.to_path_buf();

    let manager = ConversationManager::new(
        auth_manager,
        SessionSource::SubAgent(SubAgentSource::Other(
            "security_review_validation_prep".to_string(),
        )),
    );

    let conversation = match manager.new_conversation(prep_config).await {
        Ok(new_conversation) => new_conversation.conversation,
        Err(err) => {
            let message = format!("Failed to start validation prep agent: {err}");
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(ValidationTargetPrepFailure { message, logs });
        }
    };

    let mut exec_commands: Vec<ValidationPrepExecCommand> = Vec::new();
    let mut next_prompt = prompt;
    let mut final_text: Option<String> = None;
    let mut last_tool_log: Option<String> = None;
    let mut reasoning_accumulator = ReasoningAccumulator::new();

    for turn in 0..VALIDATION_TARGET_PREP_MAX_TURNS {
        if turn > 0 {
            push_progress_log(
                &progress_sender,
                &log_sink,
                &mut logs,
                format!(
                    "Validation prep: retrying (attempt {}/{})",
                    turn + 1,
                    VALIDATION_TARGET_PREP_MAX_TURNS
                ),
            );
        }

        if let Err(err) = conversation
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: next_prompt.clone(),
                }],
            })
            .await
        {
            let message = format!("Failed to submit validation prep prompt: {err}");
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(ValidationTargetPrepFailure { message, logs });
        }

        let exec_len_before_turn = exec_commands.len();
        let mut last_agent_message: Option<String> = None;

        loop {
            let event = match conversation.next_event().await {
                Ok(event) => event,
                Err(err) => {
                    let message = format!("Validation prep agent terminated unexpectedly: {err}");
                    push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                    return Err(ValidationTargetPrepFailure { message, logs });
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
                EventMsg::AgentReasoning(reason) => {
                    reasoning_accumulator.push_full(
                        &reason.text,
                        &progress_sender,
                        &log_sink,
                        &mut logs,
                    );
                }
                EventMsg::AgentReasoningRawContent(reason) => {
                    reasoning_accumulator.push_full(
                        &reason.text,
                        &progress_sender,
                        &log_sink,
                        &mut logs,
                    );
                }
                EventMsg::AgentReasoningDelta(delta) => {
                    reasoning_accumulator.push_delta(
                        &delta.delta,
                        &progress_sender,
                        &log_sink,
                        &mut logs,
                    );
                }
                EventMsg::AgentReasoningRawContentDelta(delta) => {
                    reasoning_accumulator.push_delta(
                        &delta.delta,
                        &progress_sender,
                        &log_sink,
                        &mut logs,
                    );
                }
                EventMsg::AgentReasoningSectionBreak(_) => {
                    reasoning_accumulator.flush_remaining(&progress_sender, &log_sink, &mut logs);
                }
                EventMsg::McpToolCallBegin(begin) => {
                    let tool = begin.invocation.tool.clone();
                    let message = format!("Validation prep: tool → {tool}");
                    if last_tool_log.as_deref() != Some(message.as_str()) {
                        push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                        last_tool_log = Some(message);
                    }
                }
                EventMsg::ExecCommandBegin(begin) => {
                    let command = strip_bash_lc_and_escape(&begin.command);
                    let summary = format!("Validation prep: {command}");
                    emit_command_start(&command_emitter, summary, Some(command.as_str()));
                    push_progress_log(
                        &progress_sender,
                        &log_sink,
                        &mut logs,
                        format!("Validation prep: run `{command}`"),
                    );
                }
                EventMsg::ExecCommandEnd(done) => {
                    let command = strip_bash_lc_and_escape(&done.command);
                    let summary = format!("Validation prep: {command}");
                    exec_commands.push(ValidationPrepExecCommand {
                        command: command.clone(),
                        exit_code: done.exit_code,
                        duration_secs: done.duration.as_secs(),
                    });
                    let output = if done.exit_code == 0 {
                        done.stdout.as_str()
                    } else {
                        done.stderr.as_str()
                    };
                    let state = if done.exit_code == 0 {
                        SecurityReviewCommandState::Matches
                    } else {
                        SecurityReviewCommandState::Error
                    };
                    emit_command_result(
                        &command_emitter,
                        summary,
                        state,
                        Some(command.as_str()),
                        Some(output),
                    );

                    let duration = fmt_elapsed_compact(done.duration.as_secs());
                    if done.exit_code == 0 {
                        push_progress_log(
                            &progress_sender,
                            &log_sink,
                            &mut logs,
                            format!("Validation prep: ok ({duration}) · `{command}`"),
                        );
                        continue;
                    }

                    let summary = summarize_process_output(false, &done.stdout, &done.stderr);
                    push_progress_log(
                        &progress_sender,
                        &log_sink,
                        &mut logs,
                        format!(
                            "Validation prep: `{command}` exited {} ({duration}) · {summary}",
                            done.exit_code
                        ),
                    );
                }
                EventMsg::Warning(warn) => {
                    push_progress_log(&progress_sender, &log_sink, &mut logs, warn.message);
                }
                EventMsg::Error(err) => {
                    let message = format!("Validation prep agent error: {}", err.message);
                    push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                    return Err(ValidationTargetPrepFailure { message, logs });
                }
                EventMsg::TurnAborted(aborted) => {
                    let message = format!("Validation prep agent aborted: {:?}", aborted.reason);
                    push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
                    return Err(ValidationTargetPrepFailure { message, logs });
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

        reasoning_accumulator.flush_remaining(&progress_sender, &log_sink, &mut logs);
        let Some(text) = last_agent_message
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
        else {
            final_text = None;
            next_prompt = "Your previous message was empty. Respond ONLY with a single JSON object per the required schema (no markdown/prose).".to_string();
            continue;
        };

        final_text = Some(text.clone());
        let parsed = parse_validation_target_prep_output(&text);
        let exec_count_this_turn = exec_commands.len().saturating_sub(exec_len_before_turn);

        let Some(parsed) = parsed else {
            next_prompt = "Your previous message was not parseable JSON. Respond ONLY with a single JSON object per the required schema (no markdown/prose).".to_string();
            continue;
        };

        let local_ready = parsed.local_build_ok && parsed.local_run_ok;
        let docker_ready = parsed.docker_build_ok && parsed.docker_run_ok;
        let docker_attempted = exec_commands.iter().any(|cmd| {
            cmd.command
                .lines()
                .any(|line| line.trim_start().starts_with("docker "))
        });

        if local_ready
            && (docker_ready || docker_attempted || turn + 1 == VALIDATION_TARGET_PREP_MAX_TURNS)
        {
            break;
        }

        if exec_count_this_turn == 0 {
            next_prompt = "You finished without running any commands. This is an execution step: run commands to build and smoke-test a local target and attempt a Docker build+run (or conclusively identify an unfixable blocker), then respond with ONLY the required JSON object.".to_string();
            continue;
        }

        if local_ready && !docker_attempted {
            next_prompt = "Local validation target is ready. Now attempt the Docker-based build+run path (Dockerfile under the output directory). If Docker is unavailable/blocked, report outcome=partial and set docker_* flags accordingly. Respond with ONLY the required JSON object.".to_string();
            continue;
        }

        next_prompt = "Not done yet. Keep iterating on build/runtime errors until you can (1) successfully build+run the local target and (2) successfully build+run the Docker target (or conclude Docker is unavailable/blocked and report outcome=partial). Then respond with ONLY the required JSON object.".to_string();
    }

    let text = match final_text {
        Some(text) => text,
        None => {
            let message = "Validation prep agent produced an empty response.".to_string();
            push_progress_log(&progress_sender, &log_sink, &mut logs, message.clone());
            return Err(ValidationTargetPrepFailure { message, logs });
        }
    };

    let _ = conversation.submit(Op::Shutdown).await;

    Ok(ValidationTargetPrepAgentOutput {
        text,
        logs,
        exec_commands,
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_validation_plan_agent(
    config: &Config,
    provider: &ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    repo_root: &Path,
    prompt: String,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
    label: &str,
) -> Result<ValidationPlanAgentOutput, BugVerificationFailure> {
    let mut logs: Vec<String> = Vec::new();
    let start = Instant::now();
    let deadline = start + Duration::from_secs(VALIDATION_AGENT_TIMEOUT_SECS);
    let remaining = || deadline.saturating_duration_since(Instant::now());
    let command_emitter = CommandStatusEmitter::new(progress_sender.clone(), metrics.clone());
    let mut reasoning_accumulator = ReasoningAccumulator::new();
    let mut last_tool_log: Option<String> = None;
    push_progress_log(
        &progress_sender,
        &None,
        &mut logs,
        format!("Planning validation for {label}..."),
    );

    let mut validation_config = config.clone();
    validation_config.model = Some(model.to_string());
    validation_config.model_reasoning_effort =
        normalize_reasoning_effort_for_model(model, reasoning_effort);
    validation_config.model_provider = provider.clone();
    validation_config.base_instructions = Some(VALIDATION_PLAN_SYSTEM_PROMPT.to_string());
    validation_config.user_instructions = None;
    validation_config.developer_instructions = None;
    validation_config.compact_prompt = None;
    validation_config.cwd = repo_root.to_path_buf();

    let manager = ConversationManager::new(
        auth_manager,
        SessionSource::SubAgent(SubAgentSource::Other(
            "security_review_validation".to_string(),
        )),
    );

    let conversation = match tokio::time::timeout(
        remaining(),
        manager.new_conversation(validation_config),
    )
    .await
    {
        Ok(Ok(new_conversation)) => new_conversation.conversation,
        Ok(Err(err)) => {
            let message = format!("Failed to start validation agent for {label}: {err}");
            push_progress_log(&progress_sender, &None, &mut logs, message.clone());
            return Err(BugVerificationFailure { message, logs });
        }
        Err(_) => {
            let elapsed = fmt_elapsed_compact(start.elapsed().as_secs());
            let message = format!("Validation planning timed out after {elapsed} for {label}.");
            push_progress_log(&progress_sender, &None, &mut logs, message.clone());
            return Err(BugVerificationFailure { message, logs });
        }
    };

    match tokio::time::timeout(
        remaining(),
        conversation.submit(Op::UserInput {
            items: vec![UserInput::Text { text: prompt }],
        }),
    )
    .await
    {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => {
            let message = format!("Failed to submit validation prompt for {label}: {err}");
            push_progress_log(&progress_sender, &None, &mut logs, message.clone());
            return Err(BugVerificationFailure { message, logs });
        }
        Err(_) => {
            let elapsed = fmt_elapsed_compact(start.elapsed().as_secs());
            let message = format!(
                "Validation planning timed out submitting the prompt after {elapsed} for {label}."
            );
            push_progress_log(&progress_sender, &None, &mut logs, message.clone());
            let _ = tokio::time::timeout(Duration::from_secs(1), conversation.submit(Op::Shutdown))
                .await;
            return Err(BugVerificationFailure { message, logs });
        }
    };

    let mut last_agent_message: Option<String> = None;
    loop {
        let remaining = remaining();
        if remaining.is_zero() {
            let elapsed = fmt_elapsed_compact(start.elapsed().as_secs());
            let message = format!("Validation planning timed out after {elapsed} for {label}.");
            push_progress_log(&progress_sender, &None, &mut logs, message.clone());
            let _ = tokio::time::timeout(Duration::from_secs(1), conversation.submit(Op::Shutdown))
                .await;
            return Err(BugVerificationFailure { message, logs });
        }

        let event = match tokio::time::timeout(remaining, conversation.next_event()).await {
            Ok(Ok(event)) => event,
            Ok(Err(err)) => {
                let message =
                    format!("Validation agent terminated unexpectedly for {label}: {err}");
                push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                return Err(BugVerificationFailure { message, logs });
            }
            Err(_) => {
                let elapsed = fmt_elapsed_compact(start.elapsed().as_secs());
                let message = format!("Validation planning timed out after {elapsed} for {label}.");
                push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                let _ =
                    tokio::time::timeout(Duration::from_secs(1), conversation.submit(Op::Shutdown))
                        .await;
                return Err(BugVerificationFailure { message, logs });
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
                reasoning_accumulator.push_full(&reason.text, &progress_sender, &None, &mut logs);
            }
            EventMsg::AgentReasoningRawContent(reason) => {
                reasoning_accumulator.push_full(&reason.text, &progress_sender, &None, &mut logs);
            }
            EventMsg::AgentReasoningDelta(delta) => {
                reasoning_accumulator.push_delta(&delta.delta, &progress_sender, &None, &mut logs);
            }
            EventMsg::AgentReasoningRawContentDelta(delta) => {
                reasoning_accumulator.push_delta(&delta.delta, &progress_sender, &None, &mut logs);
            }
            EventMsg::AgentReasoningSectionBreak(_) => {
                reasoning_accumulator.flush_remaining(&progress_sender, &None, &mut logs);
            }
            EventMsg::McpToolCallBegin(begin) => {
                let tool = begin.invocation.tool.clone();
                let message = format!("Validation planning: tool → {tool}");
                if last_tool_log.as_deref() != Some(message.as_str()) {
                    push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                    last_tool_log = Some(message);
                }
            }
            EventMsg::ExecCommandBegin(begin) => {
                let command = strip_bash_lc_and_escape(&begin.command);
                let summary = format!("Validation planning for {label}");
                emit_command_start(&command_emitter, summary, Some(command.as_str()));
                push_progress_log(
                    &progress_sender,
                    &None,
                    &mut logs,
                    format!("Validation planning: run `{command}`"),
                );
            }
            EventMsg::ExecCommandEnd(done) => {
                let command = strip_bash_lc_and_escape(&done.command);
                let output = if done.exit_code == 0 {
                    done.stdout.as_str()
                } else {
                    done.stderr.as_str()
                };
                let state = if done.exit_code == 0 {
                    SecurityReviewCommandState::Matches
                } else {
                    SecurityReviewCommandState::Error
                };
                emit_command_result(
                    &command_emitter,
                    format!("Validation planning for {label}"),
                    state,
                    Some(command.as_str()),
                    Some(output),
                );
                let duration = fmt_elapsed_compact(done.duration.as_secs());
                if done.exit_code == 0 {
                    push_progress_log(
                        &progress_sender,
                        &None,
                        &mut logs,
                        format!("Validation planning: ok ({duration}) · `{command}`"),
                    );
                } else {
                    let summary = summarize_process_output(false, &done.stdout, &done.stderr);
                    push_progress_log(
                        &progress_sender,
                        &None,
                        &mut logs,
                        format!(
                            "Validation planning: `{command}` exited {} ({duration}) · {summary}",
                            done.exit_code
                        ),
                    );
                }
            }
            EventMsg::Warning(warn) => {
                push_progress_log(&progress_sender, &None, &mut logs, warn.message);
            }
            EventMsg::Error(err) => {
                let message = format!("Validation agent error for {label}: {}", err.message);
                push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                return Err(BugVerificationFailure { message, logs });
            }
            EventMsg::TurnAborted(aborted) => {
                let message = format!("Validation agent aborted for {label}: {:?}", aborted.reason);
                push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                return Err(BugVerificationFailure { message, logs });
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

    reasoning_accumulator.flush_remaining(&progress_sender, &None, &mut logs);
    let text = match last_agent_message.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }) {
        Some(text) => text,
        None => {
            let message = format!("Validation agent produced an empty response for {label}.");
            push_progress_log(&progress_sender, &None, &mut logs, message.clone());
            return Err(BugVerificationFailure { message, logs });
        }
    };

    let _ = conversation.submit(Op::Shutdown).await;

    Ok(ValidationPlanAgentOutput { text, logs })
}

#[derive(Debug, Clone, Deserialize)]
struct ValidationRefineFile {
    path: String,
    contents: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ValidationRefineOutput {
    summary: Option<String>,
    dockerfile: Option<String>,
    docker_build: Option<String>,
    docker_run: Option<String>,
    #[serde(default)]
    testing_md_additions: Option<String>,
    #[serde(default)]
    files: Vec<ValidationRefineFile>,
}

fn parse_validation_refine_output(raw: &str) -> Option<ValidationRefineOutput> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(item) = serde_json::from_str::<ValidationRefineOutput>(trimmed) {
        return Some(item);
    }

    for snippet in extract_json_objects(trimmed) {
        if let Ok(item) = serde_json::from_str::<ValidationRefineOutput>(&snippet) {
            return Some(item);
        }
    }

    None
}

struct ValidationRefineAgentOutput {
    text: String,
    logs: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
async fn run_validation_refine_agent(
    config: &Config,
    provider: &ModelProviderInfo,
    auth_manager: Arc<AuthManager>,
    repo_root: &Path,
    prompt: String,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
    label: &str,
) -> Result<ValidationRefineAgentOutput, BugVerificationFailure> {
    let mut logs: Vec<String> = Vec::new();
    let start = Instant::now();
    let deadline = start + Duration::from_secs(POST_VALIDATION_REFINE_WORKER_TIMEOUT_SECS);
    let remaining = || deadline.saturating_duration_since(Instant::now());
    let command_emitter = CommandStatusEmitter::new(progress_sender.clone(), metrics.clone());
    let mut reasoning_accumulator = ReasoningAccumulator::new();
    let mut last_tool_log: Option<String> = None;
    push_progress_log(
        &progress_sender,
        &None,
        &mut logs,
        format!("Refining validation PoC for {label}..."),
    );

    let mut refine_config = config.clone();
    refine_config.model = Some(model.to_string());
    refine_config.model_reasoning_effort =
        normalize_reasoning_effort_for_model(model, reasoning_effort);
    refine_config.model_provider = provider.clone();
    refine_config.base_instructions = Some(VALIDATION_REFINE_SYSTEM_PROMPT.to_string());
    refine_config.user_instructions = None;
    refine_config.developer_instructions = None;
    refine_config.compact_prompt = None;
    refine_config.cwd = repo_root.to_path_buf();

    let manager = ConversationManager::new(
        auth_manager,
        SessionSource::SubAgent(SubAgentSource::Other(
            "security_review_validation_refine".to_string(),
        )),
    );

    let conversation =
        match tokio::time::timeout(remaining(), manager.new_conversation(refine_config)).await {
            Ok(Ok(new_conversation)) => new_conversation.conversation,
            Ok(Err(err)) => {
                let message = format!("Failed to start validation refine agent for {label}: {err}");
                push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                return Err(BugVerificationFailure { message, logs });
            }
            Err(_) => {
                let elapsed = fmt_elapsed_compact(start.elapsed().as_secs());
                let message =
                    format!("Validation PoC refinement timed out after {elapsed} for {label}.");
                push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                return Err(BugVerificationFailure { message, logs });
            }
        };

    match tokio::time::timeout(
        remaining(),
        conversation.submit(Op::UserInput {
            items: vec![UserInput::Text { text: prompt }],
        }),
    )
    .await
    {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => {
            let message = format!("Failed to submit validation refine prompt for {label}: {err}");
            push_progress_log(&progress_sender, &None, &mut logs, message.clone());
            return Err(BugVerificationFailure { message, logs });
        }
        Err(_) => {
            let elapsed = fmt_elapsed_compact(start.elapsed().as_secs());
            let message = format!(
                "Validation PoC refinement timed out submitting the prompt after {elapsed} for {label}."
            );
            push_progress_log(&progress_sender, &None, &mut logs, message.clone());
            let _ = tokio::time::timeout(Duration::from_secs(1), conversation.submit(Op::Shutdown))
                .await;
            return Err(BugVerificationFailure { message, logs });
        }
    };

    let mut last_agent_message: Option<String> = None;
    loop {
        let remaining = remaining();
        if remaining.is_zero() {
            let elapsed = fmt_elapsed_compact(start.elapsed().as_secs());
            let message =
                format!("Validation PoC refinement timed out after {elapsed} for {label}.");
            push_progress_log(&progress_sender, &None, &mut logs, message.clone());
            let _ = tokio::time::timeout(Duration::from_secs(1), conversation.submit(Op::Shutdown))
                .await;
            return Err(BugVerificationFailure { message, logs });
        }

        let event = match tokio::time::timeout(remaining, conversation.next_event()).await {
            Ok(Ok(event)) => event,
            Ok(Err(err)) => {
                let message =
                    format!("Validation refine agent terminated unexpectedly for {label}: {err}");
                push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                return Err(BugVerificationFailure { message, logs });
            }
            Err(_) => {
                let elapsed = fmt_elapsed_compact(start.elapsed().as_secs());
                let message =
                    format!("Validation PoC refinement timed out after {elapsed} for {label}.");
                push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                let _ =
                    tokio::time::timeout(Duration::from_secs(1), conversation.submit(Op::Shutdown))
                        .await;
                return Err(BugVerificationFailure { message, logs });
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
                reasoning_accumulator.push_full(&reason.text, &progress_sender, &None, &mut logs);
            }
            EventMsg::AgentReasoningRawContent(reason) => {
                reasoning_accumulator.push_full(&reason.text, &progress_sender, &None, &mut logs);
            }
            EventMsg::AgentReasoningDelta(delta) => {
                reasoning_accumulator.push_delta(&delta.delta, &progress_sender, &None, &mut logs);
            }
            EventMsg::AgentReasoningRawContentDelta(delta) => {
                reasoning_accumulator.push_delta(&delta.delta, &progress_sender, &None, &mut logs);
            }
            EventMsg::AgentReasoningSectionBreak(_) => {
                reasoning_accumulator.flush_remaining(&progress_sender, &None, &mut logs);
            }
            EventMsg::McpToolCallBegin(begin) => {
                let tool = begin.invocation.tool.clone();
                let message = format!("Validation refine: tool → {tool}");
                if last_tool_log.as_deref() != Some(message.as_str()) {
                    push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                    last_tool_log = Some(message);
                }
            }
            EventMsg::ExecCommandBegin(begin) => {
                let command = strip_bash_lc_and_escape(&begin.command);
                let summary = format!("Validation refine for {label}");
                emit_command_start(&command_emitter, summary, Some(command.as_str()));
                push_progress_log(
                    &progress_sender,
                    &None,
                    &mut logs,
                    format!("Validation refine: run `{command}`"),
                );
            }
            EventMsg::ExecCommandEnd(done) => {
                let command = strip_bash_lc_and_escape(&done.command);
                let output = if done.exit_code == 0 {
                    done.stdout.as_str()
                } else {
                    done.stderr.as_str()
                };
                let state = if done.exit_code == 0 {
                    SecurityReviewCommandState::Matches
                } else {
                    SecurityReviewCommandState::Error
                };
                emit_command_result(
                    &command_emitter,
                    format!("Validation refine for {label}"),
                    state,
                    Some(command.as_str()),
                    Some(output),
                );
                let duration = fmt_elapsed_compact(done.duration.as_secs());
                if done.exit_code == 0 {
                    push_progress_log(
                        &progress_sender,
                        &None,
                        &mut logs,
                        format!("Validation refine: ok ({duration}) · `{command}`"),
                    );
                } else {
                    let summary = summarize_process_output(false, &done.stdout, &done.stderr);
                    push_progress_log(
                        &progress_sender,
                        &None,
                        &mut logs,
                        format!(
                            "Validation refine: `{command}` exited {} ({duration}) · {summary}",
                            done.exit_code
                        ),
                    );
                }
            }
            EventMsg::Warning(warn) => {
                push_progress_log(&progress_sender, &None, &mut logs, warn.message);
            }
            EventMsg::Error(err) => {
                let message = format!("Validation refine agent error for {label}: {}", err.message);
                push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                return Err(BugVerificationFailure { message, logs });
            }
            EventMsg::TurnAborted(aborted) => {
                let message = format!(
                    "Validation refine agent aborted for {label}: {:?}",
                    aborted.reason
                );
                push_progress_log(&progress_sender, &None, &mut logs, message.clone());
                return Err(BugVerificationFailure { message, logs });
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

    reasoning_accumulator.flush_remaining(&progress_sender, &None, &mut logs);
    let text = match last_agent_message.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }) {
        Some(text) => text,
        None => {
            let message =
                format!("Validation refine agent produced an empty response for {label}.");
            push_progress_log(&progress_sender, &None, &mut logs, message.clone());
            return Err(BugVerificationFailure { message, logs });
        }
    };

    let _ = conversation.submit(Op::Shutdown).await;

    Ok(ValidationRefineAgentOutput { text, logs })
}

async fn write_validation_snapshot_and_reports(
    snapshot: &SecurityReviewSnapshot,
    batch: &BugVerificationBatchRequest,
    logs: &[String],
) -> Result<(), BugVerificationFailure> {
    let snapshot_bytes =
        serde_json::to_vec_pretty(snapshot).map_err(|err| BugVerificationFailure {
            message: format!("Failed to serialize bug snapshot: {err}"),
            logs: logs.to_owned(),
        })?;
    tokio_fs::write(&batch.snapshot_path, snapshot_bytes)
        .await
        .map_err(|err| BugVerificationFailure {
            message: format!("Failed to write {}: {err}", batch.snapshot_path.display()),
            logs: logs.to_owned(),
        })?;

    let git_link_info = build_git_link_info(&batch.repo_path).await;
    let bugs_markdown = build_bugs_markdown(
        snapshot,
        git_link_info.as_ref(),
        Some(batch.repo_path.as_path()),
        batch.snapshot_path.parent().and_then(Path::parent),
    );
    tokio_fs::write(&batch.bugs_path, bugs_markdown.as_bytes())
        .await
        .map_err(|err| BugVerificationFailure {
            message: format!("Failed to write {}: {err}", batch.bugs_path.display()),
            logs: logs.to_owned(),
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
                logs: logs.to_owned(),
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
                    logs: logs.to_owned(),
                })?;
        }
    }

    Ok(())
}

#[derive(Clone, Debug)]
struct ValidationBuildFailure {
    command: String,
    summary: String,
    stdout_path: String,
    stderr_path: String,
    output_snippet: Option<String>,
}

async fn run_validation_build_preflight(
    repo_root: &Path,
    work_dir: &Path,
    progress_sender: &Option<AppEventSender>,
    logs: &mut Vec<String>,
) -> Option<ValidationBuildFailure> {
    fn has_build_script(package_json: &Value) -> bool {
        package_json
            .get("scripts")
            .and_then(Value::as_object)
            .and_then(|scripts| scripts.get("build"))
            .and_then(Value::as_str)
            .is_some_and(|script| !script.trim().is_empty())
    }

    fn file_stem_for_command(program: &str, args: &[String]) -> String {
        let mut stem = format!("preflight_{program}");
        for arg in args.iter().take(3) {
            stem.push('_');
            stem.push_str(arg);
        }
        stem.chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
            .collect()
    }

    let has_cargo = repo_root.join("Cargo.toml").exists();
    let has_go = repo_root.join("go.mod").exists();
    let has_package_json = repo_root.join("package.json").exists();

    let mut attempts: Vec<Vec<(String, Vec<String>)>> = Vec::new();
    if has_cargo {
        attempts.push(vec![(
            "cargo".to_string(),
            vec!["build".to_string(), "--locked".to_string()],
        )]);
        attempts.push(vec![("cargo".to_string(), vec!["build".to_string()])]);
    } else if has_go {
        attempts.push(vec![(
            "go".to_string(),
            vec!["build".to_string(), "./...".to_string()],
        )]);
    } else if has_package_json {
        let package_json = tokio_fs::read_to_string(repo_root.join("package.json"))
            .await
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
        if package_json.as_ref().is_some_and(has_build_script) {
            let install_cmd = if repo_root.join("package-lock.json").exists() {
                vec!["ci".to_string()]
            } else {
                vec!["install".to_string()]
            };
            attempts.push(vec![
                ("npm".to_string(), install_cmd),
                (
                    "npm".to_string(),
                    vec!["run".to_string(), "build".to_string()],
                ),
            ]);
        }
    }

    if attempts.is_empty() {
        return None;
    }

    push_progress_log(
        progress_sender,
        &None,
        logs,
        "Validation preflight: compiling the target...".to_string(),
    );

    let mut last_failure: Option<ValidationBuildFailure> = None;
    for attempt in attempts {
        let mut attempt_failure: Option<ValidationBuildFailure> = None;
        for (program, args) in &attempt {
            let mut command = Command::new(program);
            command.args(args).current_dir(repo_root);
            let command_label = format!(
                "{} {}",
                program,
                args.iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            push_progress_log(
                progress_sender,
                &None,
                logs,
                format!("Run: `{command_label}`"),
            );

            let start = Instant::now();
            let timeout = Duration::from_secs(VALIDATION_PREFLIGHT_TIMEOUT_SECS);
            match command_output_with_timeout(command, timeout).await {
                Ok(CommandOutputOutcome::Completed(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    if !output.status.success() {
                        let duration_label = fmt_elapsed_compact(start.elapsed().as_secs());
                        let summary_line = summarize_process_output(false, &stdout, &stderr);
                        let summary = format!("{summary_line} · {duration_label}");
                        let snippet = if stderr.trim().is_empty() {
                            stdout.trim()
                        } else {
                            stderr.trim()
                        };
                        let output_snippet = if snippet.is_empty() {
                            None
                        } else {
                            Some(truncate_text(snippet, VALIDATION_OUTPUT_GRAPHEMES))
                        };
                        let file_stem = file_stem_for_command(program, args);
                        let (stdout_path, stderr_path) = write_validation_output_files(
                            work_dir, repo_root, &file_stem, "build", &stdout, &stderr,
                        )
                        .await;
                        attempt_failure = Some(ValidationBuildFailure {
                            command: command_label,
                            summary,
                            stdout_path,
                            stderr_path,
                            output_snippet,
                        });
                        break;
                    }
                }
                Ok(CommandOutputOutcome::TimedOut) => {
                    let duration_label = fmt_elapsed_compact(start.elapsed().as_secs());
                    let message = format!(
                        "Timed out after {duration_label} while running `{command_label}`."
                    );
                    let file_stem = file_stem_for_command(program, args);
                    let (stdout_path, stderr_path) = write_validation_output_files(
                        work_dir,
                        repo_root,
                        &file_stem,
                        "build",
                        "",
                        message.as_str(),
                    )
                    .await;
                    attempt_failure = Some(ValidationBuildFailure {
                        command: command_label,
                        summary: message,
                        stdout_path,
                        stderr_path,
                        output_snippet: None,
                    });
                    break;
                }
                Err(err) => {
                    let message = format!("Failed to run `{command_label}`: {err}");
                    let file_stem = file_stem_for_command(program, args);
                    let (stdout_path, stderr_path) = write_validation_output_files(
                        work_dir, repo_root, &file_stem, "build", "", &message,
                    )
                    .await;
                    attempt_failure = Some(ValidationBuildFailure {
                        command: command_label,
                        summary: message,
                        stdout_path,
                        stderr_path,
                        output_snippet: None,
                    });
                    break;
                }
            }
        }

        if attempt_failure.is_none() {
            push_progress_log(
                progress_sender,
                &None,
                logs,
                "Validation preflight succeeded.".to_string(),
            );
            return None;
        }

        last_failure = attempt_failure;
    }

    last_failure
}

#[allow(clippy::too_many_arguments)]
async fn run_asan_validation(
    repo_path: PathBuf,
    snapshot_path: PathBuf,
    bugs_path: PathBuf,
    report_path: Option<PathBuf>,
    report_html_path: Option<PathBuf>,
    provider: ModelProviderInfo,
    model: String,
    reasoning_effort: Option<ReasoningEffort>,
    config: &Config,
    auth_manager: Arc<AuthManager>,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
    web_target_url: Option<String>,
    web_creds_path: Option<PathBuf>,
) -> Result<(), BugVerificationFailure> {
    let mut logs: Vec<String> = Vec::new();
    let command_emitter = CommandStatusEmitter::new(progress_sender.clone(), metrics.clone());

    let bytes = match tokio_fs::read(&snapshot_path).await {
        Ok(bytes) => bytes,
        Err(err) => {
            let message = format!(
                "Validation skipped: failed to read {}: {err}",
                snapshot_path.display()
            );
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(message.clone()));
            }
            logs.push(message);
            return Ok(());
        }
    };
    let mut snapshot: SecurityReviewSnapshot = match serde_json::from_slice(&bytes) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            let message = format!(
                "Validation skipped: failed to parse {}: {err}",
                snapshot_path.display()
            );
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(message.clone()));
            }
            logs.push(message);
            return Ok(());
        }
    };

    let reasoning_label = reasoning_effort_label(normalize_reasoning_effort_for_model(
        model.as_str(),
        reasoning_effort,
    ));
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(format!(
            "Planning validation for findings... (model: {model}, reasoning: {reasoning_label}).",
            model = model.as_str()
        )));
    }

    let output_root = snapshot_path
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_path.join(".codex_validation"));
    let work_dir = output_root.join("validation");
    let _ = tokio_fs::create_dir_all(&work_dir).await;

    let specs_root = output_root.join("specs");
    let testing_path = specs_root.join("TESTING.md");
    let mut testing_md = tokio_fs::read_to_string(&testing_path)
        .await
        .unwrap_or_default();

    let generated_creds_path = specs_root.join(WEB_VALIDATION_CREDS_FILE_NAME);
    let web_validation = {
        let target_raw = web_target_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        match target_raw {
            Some(raw) => match normalize_web_validation_base_url(raw) {
                Ok(base_url) => {
                    let headers = if let Some(path) = web_creds_path.as_ref() {
                        match tokio_fs::read_to_string(path).await {
                            Ok(contents) => parse_web_validation_creds(&contents),
                            Err(err) => {
                                push_progress_log(
                                    &progress_sender,
                                    &None,
                                    &mut logs,
                                    format!(
                                        "Failed to read creds file {}: {err}. Continuing without creds.",
                                        path.display()
                                    ),
                                );
                                Vec::new()
                            }
                        }
                    } else {
                        Vec::new()
                    };
                    Some(WebValidationConfig {
                        base_url,
                        redactions: redactions_for_headers(&headers),
                        headers,
                    })
                }
                Err(err) => {
                    push_progress_log(
                        &progress_sender,
                        &None,
                        &mut logs,
                        format!("Web validation disabled: {err}."),
                    );
                    None
                }
            },
            None => None,
        }
    };

    if let Some(web_validation) = web_validation.as_ref() {
        let generated_headers = match tokio_fs::read_to_string(&generated_creds_path).await {
            Ok(contents) => parse_web_validation_creds(&contents),
            Err(_) => Vec::new(),
        };
        let target_section_lines = build_web_validation_target_section_lines(
            &repo_path,
            &web_validation.base_url,
            web_creds_path.as_deref(),
            &web_validation.headers,
            &generated_creds_path,
            &generated_headers,
        );

        apply_validation_target_md_section(
            &testing_path,
            &repo_path,
            &target_section_lines,
            &progress_sender,
            &mut logs,
        )
        .await;

        testing_md = tokio_fs::read_to_string(&testing_path)
            .await
            .unwrap_or_default();
    }

    let mut testing_md_context =
        trim_prompt_context(&testing_md, VALIDATION_TESTING_CONTEXT_MAX_CHARS);

    let web_validation_context = if let Some(web_validation) = web_validation.as_ref() {
        let header_names = if web_validation.headers.is_empty() {
            "(none)".to_string()
        } else {
            web_validation
                .headers
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "- enabled: true\n- target_url: {}\n- headers: {header_names}\n- notes: only request this origin; header values are available but must not be printed",
            web_validation.base_url.as_str()
        )
    } else {
        "- enabled: false (no target URL provided)".to_string()
    };

    // Build prompt
    let findings = build_validation_findings_context(&snapshot, web_validation.is_some());
    let finding_map: HashMap<BugIdentifier, ValidationFinding> = findings
        .findings
        .iter()
        .cloned()
        .map(|finding| (finding.id, finding))
        .collect();

    let mut handled: HashSet<BugIdentifier> = HashSet::new();
    let run_at = OffsetDateTime::now_utc();
    let mut requests: Vec<BugVerificationRequest> = Vec::new();
    if findings.ids.is_empty() {
        return Ok(());
    }

    let mark_unable =
        |entry: &mut BugSnapshot, tool: &str, summary: String, logs: &mut Vec<String>| {
            if !matches!(entry.bug.validation.status, BugValidationStatus::Pending) {
                return;
            }
            entry.bug.validation.status = BugValidationStatus::UnableToValidate;
            entry.bug.validation.tool = Some(tool.to_string());
            entry.bug.validation.target = None;
            let cleaned = summary
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            if cleaned.is_empty() {
                logs.push("Validation planning produced an empty failure summary.".to_string());
                entry.bug.validation.summary =
                    Some("Validation could not be completed in this environment.".to_string());
            } else {
                entry.bug.validation.summary = Some(cleaned);
            }
            entry.bug.validation.run_at = Some(run_at);
        };

    let planning_results = futures::stream::iter(findings.findings.into_iter().map(|finding| {
        let provider = provider.clone();
        let auth_manager = auth_manager.clone();
        let progress_sender = progress_sender.clone();
        let metrics = metrics.clone();
        let model = model.clone();
        let repo_root = repo_path.clone();
        let testing_md_context = testing_md_context.clone();
        let web_validation_context = web_validation_context.clone();
        async move {
            let ValidationFinding {
                id,
                label,
                context,
                prompt_kind,
            } = finding;
            let validation_focus = match prompt_kind {
                ValidationPromptKind::Crash => VALIDATION_FOCUS_CRASH,
                ValidationPromptKind::RceBin => VALIDATION_FOCUS_RCE_BIN,
                ValidationPromptKind::Ssrf => VALIDATION_FOCUS_SSRF,
                ValidationPromptKind::Crypto => VALIDATION_FOCUS_CRYPTO,
                ValidationPromptKind::Generic => VALIDATION_FOCUS_GENERIC,
            };
            let prompt = VALIDATION_PLAN_PROMPT_TEMPLATE
                .replace("{validation_focus}", validation_focus)
                .replace("{findings}", &context)
                .replace("{testing_md}", &testing_md_context)
                .replace("{web_validation}", &web_validation_context);
            let result = run_validation_plan_agent(
                config,
                &provider,
                auth_manager,
                repo_root.as_path(),
                prompt,
                progress_sender,
                metrics,
                model.as_str(),
                reasoning_effort,
                label.as_str(),
            )
            .await;
            (id, label, result)
        }
    }))
    .buffer_unordered(VALIDATION_PLAN_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;

    let mut testing_md_additions: HashMap<BugIdentifier, String> = HashMap::new();
    for (id, label, result) in planning_results {
        let Some(index) = find_bug_index(&snapshot, id) else {
            continue;
        };
        let entry = &mut snapshot.bugs[index];

        match result {
            Ok(output) => {
                logs.extend(output.logs);
                let Some(parsed) = parse_validation_plan_item(output.text.as_str()) else {
                    handled.insert(id);
                    mark_unable(
                        entry,
                        "none",
                        format!(
                            "Validation planning produced unparseable output for {label}; no validation was run."
                        ),
                        &mut logs,
                    );
                    continue;
                };
                if let Some(additions) = parsed
                    .testing_md_additions
                    .as_deref()
                    .map(str::trim_end)
                    .filter(|s| !s.trim().is_empty())
                {
                    testing_md_additions.insert(id, additions.to_string());
                }
                if let Some(reason) = parsed
                    .reason
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    log_model_reasoning(reason, &progress_sender, &None, &mut logs);
                }
                let Some(tool_raw) = parsed
                    .tool
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                else {
                    handled.insert(id);
                    mark_unable(
                        entry,
                        "none",
                        format!(
                            "Validation planning returned no tool for {label}; no validation was run."
                        ),
                        &mut logs,
                    );
                    continue;
                };
                handled.insert(id);

                match tool_raw.to_ascii_lowercase().as_str() {
                    "none" => {
                        if matches!(entry.bug.validation.status, BugValidationStatus::Pending) {
                            let reason = parsed
                                .reason
                                .as_deref()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .unwrap_or("No safe validation available in this environment.");
                            mark_unable(
                                entry,
                                "none",
                                format!("Validation planning determined no safe check: {reason}"),
                                &mut logs,
                            );
                        }
                    }
                    "python" => {
                        let Some(script) = parsed
                            .script
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        else {
                            let reason = parsed
                                .reason
                                .as_deref()
                                .map(str::trim)
                                .filter(|s| !s.is_empty());
                            let mut summary = format!(
                                "Validation planning selected python but provided no script for {label}; no validation was run."
                            );
                            if let Some(reason) = reason {
                                summary.push_str(" Reason: ");
                                summary.push_str(reason);
                            }
                            mark_unable(entry, "python", summary, &mut logs);
                            continue;
                        };

                        requests.push(BugVerificationRequest {
                            id,
                            tool: BugVerificationTool::Python,
                            target: None,
                            script_path: None,
                            script_inline: Some(script.to_string()),
                        });
                    }
                    "curl" | "playwright" => match web_validation.as_ref() {
                        None => {
                            if matches!(entry.bug.validation.status, BugValidationStatus::Pending) {
                                mark_unable(
                                    entry,
                                    tool_raw,
                                    format!(
                                        "Validation planning requested `{tool_raw}`, but web validation is disabled (no --target-url provided); no validation was run."
                                    ),
                                    &mut logs,
                                );
                            }
                        }
                        Some(web_validation) => {
                            let target = match resolve_web_validation_target(
                                &web_validation.base_url,
                                parsed.target.as_deref(),
                            ) {
                                Ok(url) => url.as_str().to_string(),
                                Err(err) => {
                                    if matches!(
                                        entry.bug.validation.status,
                                        BugValidationStatus::Pending
                                    ) {
                                        mark_unable(
                                            entry,
                                            tool_raw,
                                            format!("Invalid web validation target: {err}"),
                                            &mut logs,
                                        );
                                    }
                                    continue;
                                }
                            };
                            let tool = match tool_raw.to_ascii_lowercase().as_str() {
                                "curl" => BugVerificationTool::Curl,
                                _ => BugVerificationTool::Playwright,
                            };
                            requests.push(BugVerificationRequest {
                                id,
                                tool,
                                target: Some(target),
                                script_path: None,
                                script_inline: None,
                            });
                        }
                    },
                    other => {
                        if matches!(entry.bug.validation.status, BugValidationStatus::Pending) {
                            mark_unable(
                                entry,
                                other,
                                format!(
                                    "Validation planning returned unsupported tool `{other}`; no validation was run."
                                ),
                                &mut logs,
                            );
                        }
                    }
                }
            }
            Err(err) => {
                logs.extend(err.logs);
                mark_unable(
                    entry,
                    "none",
                    format!(
                        "Validation planning failed for {label}: {}; no validation was run.",
                        err.message
                    ),
                    &mut logs,
                );
                handled.insert(id);
            }
        }
    }

    for id in findings.ids.iter().copied() {
        if handled.contains(&id) {
            continue;
        }
        if let Some(index) = find_bug_index(&snapshot, id) {
            let entry = &mut snapshot.bugs[index];
            let label = finding_map
                .get(&id)
                .map(|finding| finding.label.as_str())
                .unwrap_or("this finding");
            mark_unable(
                entry,
                "none",
                format!(
                    "Validation planning did not produce a plan for {label}; no validation was run."
                ),
                &mut logs,
            );
        }
    }

    let planned_snapshot = snapshot.clone();
    let requested_ids: Vec<BugIdentifier> = requests.iter().map(|req| req.id).collect();

    let batch = BugVerificationBatchRequest {
        snapshot_path,
        bugs_path,
        report_path,
        report_html_path,
        repo_path: repo_path.clone(),
        work_dir,
        requests,
        web_validation: web_validation.clone(),
    };

    let mut verification_failed: Option<String> = None;
    if batch.requests.is_empty() {
        logs.push("No validation checks to execute.".to_string());
    } else {
        if batch.web_validation.is_none()
            && let Some(build_failure) = run_validation_build_preflight(
                batch.repo_path.as_path(),
                &batch.work_dir,
                &progress_sender,
                &mut logs,
            )
            .await
        {
            let note = format!(
                "Validation preflight failed; recording UnableToValidate statuses: {}",
                build_failure.summary
            );
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(note.clone()));
            }
            logs.push(note);

            for request in &batch.requests {
                if let Some(index) = find_bug_index(&snapshot, request.id) {
                    let validation = &mut snapshot.bugs[index].bug.validation;
                    if !matches!(validation.status, BugValidationStatus::Pending) {
                        continue;
                    }
                    validation.status = BugValidationStatus::UnableToValidate;
                    validation.tool = Some("python".to_string());
                    validation.target = None;
                    validation.summary = Some(
                        format!(
                            "Unable to validate (build failed): {}. Command: `{}`.",
                            build_failure.summary, build_failure.command
                        )
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string(),
                    );
                    if let Some(snippet) = build_failure.output_snippet.as_ref() {
                        validation.output_snippet = Some(snippet.clone());
                    }
                    validation
                        .repro_steps
                        .push(format!("Run: `{}`", build_failure.command));
                    validation.stdout_path = Some(build_failure.stdout_path.clone());
                    validation.stderr_path = Some(build_failure.stderr_path.clone());
                    validation.run_at = Some(run_at);
                }
            }

            let _ = write_validation_snapshot_and_reports(&snapshot, &batch, &logs).await;
            return Ok(());
        }

        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(
                "Executing validation checks...".to_string(),
            ));
        }
        match verify_bugs(batch.clone(), command_emitter.clone()).await {
            Ok(outcome) => {
                for line in outcome.logs {
                    if let Some(tx) = progress_sender.as_ref() {
                        tx.send(AppEvent::SecurityReviewLog(line.clone()));
                    }
                }
            }
            Err(err) => {
                verification_failed = Some(err.message.clone());
                logs.extend(err.logs);
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(format!(
                        "Validation checks encountered an error; recording UnableToValidate statuses: {}",
                        err.message
                    )));
                }
            }
        }
    }

    let snapshot_bytes = tokio_fs::read(&batch.snapshot_path).await.ok();
    let mut snapshot: SecurityReviewSnapshot = snapshot_bytes
        .as_deref()
        .and_then(|bytes| serde_json::from_slice(bytes).ok())
        .unwrap_or_else(|| planned_snapshot.clone());

    if verification_failed.is_none() {
        let mut applied_testing_md_additions: Vec<String> = Vec::new();
        for request in &batch.requests {
            if !matches!(request.tool, BugVerificationTool::Python) {
                continue;
            }
            let Some(index) = find_bug_index(&snapshot, request.id) else {
                continue;
            };
            let status = snapshot.bugs[index].bug.validation.status;
            if !matches!(
                status,
                BugValidationStatus::Passed | BugValidationStatus::Failed
            ) {
                continue;
            }
            let Some(additions) = testing_md_additions.get(&request.id) else {
                continue;
            };
            applied_testing_md_additions.push(additions.to_string());
        }

        if !applied_testing_md_additions.is_empty() {
            apply_validation_testing_md_additions(
                &testing_path,
                &repo_path,
                &applied_testing_md_additions,
                &progress_sender,
                &mut logs,
            )
            .await;
            let updated_testing_md = tokio_fs::read_to_string(&testing_path)
                .await
                .unwrap_or_default();
            testing_md_context =
                trim_prompt_context(&updated_testing_md, VALIDATION_TESTING_CONTEXT_MAX_CHARS);
        }
    }

    for id in &findings.ids {
        let Some(planned_index) = find_bug_index(&planned_snapshot, *id) else {
            continue;
        };
        let Some(index) = find_bug_index(&snapshot, *id) else {
            continue;
        };
        let planned_validation = planned_snapshot.bugs[planned_index].bug.validation.clone();
        let current_validation = snapshot.bugs[index].bug.validation.clone();
        if matches!(current_validation.status, BugValidationStatus::Pending)
            && !matches!(planned_validation.status, BugValidationStatus::Pending)
        {
            snapshot.bugs[index].bug.validation = planned_validation;
        }
    }

    if let Some(message) = verification_failed.as_deref() {
        for id in requested_ids {
            if let Some(index) = find_bug_index(&snapshot, id) {
                let validation = &mut snapshot.bugs[index].bug.validation;
                if matches!(validation.status, BugValidationStatus::Pending) {
                    validation.status = BugValidationStatus::UnableToValidate;
                    validation.tool = Some("python".to_string());
                    validation.target = None;
                    validation.summary = Some(
                        format!(
                            "Validation could not be completed (verification error): {message}"
                        )
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string(),
                    );
                    validation.run_at = Some(run_at);
                }
            }
        }
    }

    if let Err(err) = write_validation_snapshot_and_reports(&snapshot, &batch, &logs).await {
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(format!(
                "Validation results could not be written: {}",
                err.message
            )));
        }
        return Ok(());
    }

    if let Some(web_validation) = web_validation.as_ref() {
        let generated_headers = match tokio_fs::read_to_string(&generated_creds_path).await {
            Ok(contents) => parse_web_validation_creds(&contents),
            Err(_) => Vec::new(),
        };
        let target_section_lines = build_web_validation_target_section_lines(
            &repo_path,
            &web_validation.base_url,
            web_creds_path.as_deref(),
            &web_validation.headers,
            &generated_creds_path,
            &generated_headers,
        );
        apply_validation_target_md_section(
            &testing_path,
            &repo_path,
            &target_section_lines,
            &progress_sender,
            &mut logs,
        )
        .await;

        let updated_testing_md = tokio_fs::read_to_string(&testing_path)
            .await
            .unwrap_or_default();
        testing_md_context =
            trim_prompt_context(&updated_testing_md, VALIDATION_TESTING_CONTEXT_MAX_CHARS);
    }

    let build_validation_state_prompt = |state: &BugValidationState| -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("status: {}", validation_status_label(state)));
        if let Some(tool) = state
            .tool
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            lines.push(format!("tool: {tool}"));
        }
        if let Some(summary) = state
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            lines.push(format!("summary: {summary}"));
        }
        if !state.repro_steps.is_empty() {
            lines.push("repro_steps:".to_string());
            for step in state.repro_steps.iter().take(8) {
                let step = step.trim();
                if !step.is_empty() {
                    lines.push(format!("- {step}"));
                }
            }
        }
        if !state.artifacts.is_empty() {
            lines.push("artifacts:".to_string());
            for artifact in state.artifacts.iter().take(8) {
                let artifact = artifact.trim();
                if !artifact.is_empty() {
                    lines.push(format!("- {artifact}"));
                }
            }
        }
        if let Some(snippet) = state
            .output_snippet
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            lines.push("output_snippet:".to_string());
            lines.push(truncate_text(snippet, 1_000));
        }
        if lines.is_empty() {
            "(none)".to_string()
        } else {
            lines.join("\n")
        }
    };

    #[derive(Clone, Debug)]
    struct ValidationRefineWorkItem {
        id: BugIdentifier,
        label: String,
        context: String,
        validation_state: String,
        summary_id: usize,
        risk_rank: Option<usize>,
    }

    let refine_items: Vec<ValidationRefineWorkItem> = findings
        .ids
        .iter()
        .copied()
        .filter_map(|id| {
            let index = find_bug_index(&snapshot, id)?;
            let entry = snapshot.bugs.get(index)?;
            if entry.bug.validation.tool.as_deref() != Some("python") {
                return None;
            }
            if entry.bug.validation.status != BugValidationStatus::Passed {
                return None;
            }
            let finding = finding_map.get(&id)?;
            Some(ValidationRefineWorkItem {
                id,
                label: finding.label.clone(),
                context: finding.context.clone(),
                validation_state: build_validation_state_prompt(&entry.bug.validation),
                summary_id: entry.bug.summary_id,
                risk_rank: entry.bug.risk_rank,
            })
        })
        .collect();

    if verification_failed.is_none() && !refine_items.is_empty() {
        let refine_results = futures::stream::iter(refine_items.into_iter().map(|item| {
            let provider = provider.clone();
            let auth_manager = auth_manager.clone();
            let progress_sender = progress_sender.clone();
            let metrics = metrics.clone();
            let model = model.clone();
            let repo_root = batch.repo_path.clone();
            let work_dir = batch.work_dir.clone();
            let testing_md_context = testing_md_context.clone();
            async move {
                let bug_work_dir = work_dir.join(format!("bug{}", item.summary_id));
                let mut python_script = "(none)".to_string();
                let candidates: Vec<PathBuf> = if let Some(rank) = item.risk_rank {
                    vec![
                        bug_work_dir.join(format!("bug_rank_{rank}.py")),
                        bug_work_dir.join(format!("bug_{}.py", item.summary_id)),
                    ]
                } else {
                    vec![bug_work_dir.join(format!("bug_{}.py", item.summary_id))]
                };
                for path in candidates {
                    if let Ok(contents) = tokio_fs::read_to_string(&path).await
                        && !contents.trim().is_empty()
                    {
                        python_script = contents;
                        break;
                    }
                }

                let prompt = VALIDATION_REFINE_PROMPT_TEMPLATE
                    .replace("{finding}", &item.context)
                    .replace("{validation_state}", &item.validation_state)
                    .replace("{python_script}", python_script.trim())
                    .replace("{testing_md}", &testing_md_context);

                let result = run_validation_refine_agent(
                    config,
                    &provider,
                    auth_manager,
                    repo_root.as_path(),
                    prompt,
                    progress_sender,
                    metrics,
                    model.as_str(),
                    reasoning_effort,
                    item.label.as_str(),
                )
                .await;
                (item.id, item.label, result)
            }
        }))
        .buffer_unordered(VALIDATION_REFINE_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

        fn safe_rel_path(path: &str) -> bool {
            let path = path.trim();
            if path.is_empty() {
                return false;
            }
            let parsed = Path::new(path);
            if parsed.is_absolute() {
                return false;
            }
            !parsed
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        }

        let mut updated = false;
        let mut refine_testing_md_additions: Vec<String> = Vec::new();
        for (id, label, result) in refine_results {
            match result {
                Ok(output) => {
                    logs.extend(output.logs);
                    let Some(parsed) = parse_validation_refine_output(output.text.as_str()) else {
                        if let Some(index) = find_bug_index(&snapshot, id) {
                            let validation = &mut snapshot.bugs[index].bug.validation;
                            let addition = format!(
                                "PoC refinement produced unparseable output for {label}; no Dockerfile was created."
                            );
                            let cleaned = addition
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ")
                                .trim()
                                .to_string();
                            match validation.summary.as_mut() {
                                Some(existing) if !existing.trim().is_empty() => {
                                    existing.push_str(" · ");
                                    existing.push_str(&cleaned);
                                }
                                _ => {
                                    validation.summary = Some(cleaned);
                                }
                            }
                            updated = true;
                        }
                        continue;
                    };
                    if let Some(additions) = parsed
                        .testing_md_additions
                        .as_deref()
                        .map(str::trim_end)
                        .filter(|s| !s.trim().is_empty())
                    {
                        refine_testing_md_additions.push(additions.to_string());
                    }

                    let Some(index) = find_bug_index(&snapshot, id) else {
                        continue;
                    };
                    let Some(summary_id) =
                        snapshot.bugs.get(index).map(|entry| entry.bug.summary_id)
                    else {
                        continue;
                    };

                    let bug_work_dir = batch.work_dir.join(format!("bug{summary_id}"));

                    let mut summary_additions: Vec<String> = Vec::new();
                    if let Some(summary) = parsed
                        .summary
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        summary_additions.push(
                            summary
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ")
                                .trim()
                                .to_string(),
                        );
                    }

                    let mut artifacts_to_add: Vec<String> = Vec::new();
                    let mut dockerfile_display: Option<String> = None;

                    if let Some(dockerfile) = parsed
                        .dockerfile
                        .as_deref()
                        .map(str::trim_end)
                        .filter(|s| !s.trim().is_empty())
                    {
                        let _ = tokio_fs::create_dir_all(&bug_work_dir).await;
                        let path = bug_work_dir.join("Dockerfile");
                        match tokio_fs::write(&path, dockerfile.as_bytes()).await {
                            Ok(_) => {
                                let display = display_path_for(&path, &batch.repo_path);
                                dockerfile_display = Some(display.clone());
                                artifacts_to_add.push(display);
                            }
                            Err(err) => {
                                summary_additions.push(
                                    format!("Failed to write Dockerfile: {err}")
                                        .split_whitespace()
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                        .trim()
                                        .to_string(),
                                );
                            }
                        }
                    }

                    for file in &parsed.files {
                        if !safe_rel_path(&file.path) {
                            continue;
                        }
                        let path = bug_work_dir.join(file.path.trim());
                        if let Some(parent) = path.parent() {
                            let _ = tokio_fs::create_dir_all(parent).await;
                        }
                        match tokio_fs::write(&path, file.contents.as_bytes()).await {
                            Ok(_) => {
                                artifacts_to_add.push(display_path_for(&path, &batch.repo_path));
                            }
                            Err(err) => {
                                summary_additions.push(
                                    format!("Failed to write {}: {err}", file.path.trim())
                                        .split_whitespace()
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                        .trim()
                                        .to_string(),
                                );
                            }
                        }
                    }

                    let docker_steps = dockerfile_display.as_deref().map(|display_dockerfile| {
                        let image_tag = format!("codex-validate-bug{summary_id}");
                        let build_cmd = parsed
                            .docker_build
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                            .unwrap_or_else(|| {
                                format!("docker build -f {display_dockerfile} -t {image_tag} .")
                            });
                        let run_cmd = parsed
                            .docker_run
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("docker run --rm {image_tag}"));
                        (format!("Build: `{build_cmd}`"), format!("Run: `{run_cmd}`"))
                    });

                    let needs_update = !summary_additions.is_empty()
                        || !artifacts_to_add.is_empty()
                        || docker_steps.is_some();
                    if !needs_update {
                        continue;
                    }

                    let Some(entry) = snapshot.bugs.get_mut(index) else {
                        continue;
                    };
                    let validation = &mut entry.bug.validation;

                    for addition in summary_additions {
                        let cleaned = addition
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .trim()
                            .to_string();
                        if cleaned.is_empty() {
                            continue;
                        }
                        match validation.summary.as_mut() {
                            Some(existing) if !existing.trim().is_empty() => {
                                if !existing.contains(&cleaned) {
                                    existing.push_str(" · ");
                                    existing.push_str(&cleaned);
                                }
                            }
                            _ => {
                                validation.summary = Some(cleaned);
                            }
                        }
                    }

                    for artifact in artifacts_to_add {
                        let artifact = artifact.trim();
                        if artifact.is_empty() {
                            continue;
                        }
                        if !validation.artifacts.iter().any(|a| a == artifact) {
                            validation.artifacts.push(artifact.to_string());
                        }
                    }

                    if let Some((build_step, run_step)) = docker_steps {
                        if !validation.repro_steps.iter().any(|s| s == &build_step) {
                            validation.repro_steps.push(build_step);
                        }
                        if !validation.repro_steps.iter().any(|s| s == &run_step) {
                            validation.repro_steps.push(run_step);
                        }
                    }

                    updated = true;
                }
                Err(err) => {
                    logs.extend(err.logs);
                    if let Some(index) = find_bug_index(&snapshot, id) {
                        let validation = &mut snapshot.bugs[index].bug.validation;
                        let addition =
                            format!("PoC refinement failed for {label}: {}", err.message);
                        let cleaned = addition
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .trim()
                            .to_string();
                        match validation.summary.as_mut() {
                            Some(existing) if !existing.trim().is_empty() => {
                                existing.push_str(" · ");
                                existing.push_str(&cleaned);
                            }
                            _ => {
                                validation.summary = Some(cleaned);
                            }
                        }
                        updated = true;
                    }
                }
            }
        }

        apply_validation_testing_md_additions(
            &testing_path,
            &batch.repo_path,
            &refine_testing_md_additions,
            &progress_sender,
            &mut logs,
        )
        .await;

        if updated
            && let Err(err) = write_validation_snapshot_and_reports(&snapshot, &batch, &logs).await
            && let Some(tx) = progress_sender.as_ref()
        {
            tx.send(AppEvent::SecurityReviewLog(format!(
                "Validation refinement results could not be written: {}",
                err.message
            )));
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
    // Base URL: allow provider overrides, otherwise match codex-core defaults (ChatGPT auth uses
    // chatgpt.com backend API).
    let is_chatgpt_auth = auth
        .as_ref()
        .is_some_and(|auth| matches!(auth.mode, codex_app_server_protocol::AuthMode::ChatGPT));
    let default_base_url = if is_chatgpt_auth {
        "https://chatgpt.com/backend-api/codex"
    } else {
        "https://api.openai.com/v1"
    };
    let mut base_url = provider
        .base_url
        .clone()
        .unwrap_or_else(|| default_base_url.to_string());
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

    let session_id = security_review_session_id();
    builder = builder.header("conversation_id", session_id);
    builder = builder.header("session_id", session_id);
    builder = builder.header("x-openai-subagent", "review");

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
        if let Some(account_id) = auth.get_account_id()
            && !account_id.trim().is_empty()
            && let Ok(header_value) = HeaderValue::try_from(account_id.as_str())
        {
            builder = builder.header("ChatGPT-Account-ID", header_value);
        }
    }

    Ok(builder)
}

fn normalize_reasoning_effort_for_model(
    model: &str,
    requested: Option<ReasoningEffort>,
) -> Option<ReasoningEffort> {
    if !model.starts_with("gpt-5") || !model.contains("codex") {
        return requested;
    }

    fn is_supported_by_gpt5_codex(effort: ReasoningEffort) -> bool {
        matches!(
            effort,
            ReasoningEffort::Low | ReasoningEffort::Medium | ReasoningEffort::High
        )
    }

    requested.filter(|&effort| is_supported_by_gpt5_codex(effort))
}

fn reasoning_effort_label(effort: Option<ReasoningEffort>) -> &'static str {
    match effort {
        None => "default",
        Some(ReasoningEffort::None) => "none",
        Some(ReasoningEffort::Minimal) => "minimal",
        Some(ReasoningEffort::Low) => "low",
        Some(ReasoningEffort::Medium) => "medium",
        Some(ReasoningEffort::High) => "high",
        Some(ReasoningEffort::XHigh) => "xhigh",
    }
}

#[allow(clippy::too_many_arguments)]
async fn call_model(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
    system_prompt: &str,
    user_prompt: &str,
    metrics: Arc<ReviewMetrics>,
    temperature: f32,
) -> Result<ModelCallOutput, String> {
    // Ensure multiple retries for transient issues and allow longer recovery when rate limited.
    let default_max_retries = provider.request_max_retries().max(4);
    let rate_limit_max_retries = provider.request_max_retries().max(20);
    let mut max_retries = default_max_retries;
    let mut attempt_errors: Vec<String> = Vec::new();
    let mut attempt = 0;
    loop {
        metrics.record_model_call();

        match call_model_attempt(
            client,
            provider,
            auth,
            model,
            reasoning_effort,
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

                let retry_after = retry_after_duration(&sanitized);
                let is_rate_limited = retry_after.is_some() || is_rate_limit_error(&sanitized);
                if is_rate_limited || is_transient_decode_error(&sanitized) {
                    max_retries = max_retries.max(rate_limit_max_retries);
                }

                if attempt >= max_retries {
                    let attempt_count = attempt + 1;
                    let plural = if attempt_count == 1 { "" } else { "s" };
                    let joined = attempt_errors.join("\n- ");
                    return Err(format!(
                        "Model request for {model} failed after {attempt_count} attempt{plural}. Details:\n- {joined}"
                    ));
                }

                let total_attempts = max_retries + 1;
                let attempt_number = attempt + 1;
                attempt += 1;

                if is_rate_limited {
                    let base_backoff = default_retry_backoff(attempt);
                    let min_delay = retry_after.unwrap_or(base_backoff).max(base_backoff);
                    let jitter_ms = rand::random_range(250..=1250);
                    let jitter = Duration::from_millis(jitter_ms);
                    let total_delay = min_delay.saturating_add(jitter);
                    metrics.record_rate_limit_wait(total_delay);
                    let wait_secs = total_delay.as_secs_f32();
                    let base_secs = min_delay.as_secs_f32();
                    let jitter_ms_display = jitter.as_millis();
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
                    let base = default_retry_backoff(attempt);
                    let jitter_ms = rand::random_range(250..=750);
                    let jitter = Duration::from_millis(jitter_ms);
                    let total_delay = base.saturating_add(jitter);
                    metrics.record_rate_limit_wait(total_delay);
                    sleep(total_delay).await;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn call_model_attempt(
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    reasoning_effort: Option<ReasoningEffort>,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f32,
    metrics: Arc<ReviewMetrics>,
) -> Result<ModelCallOutput, String> {
    match provider.wire_api {
        WireApi::Responses => {
            let builder =
                make_provider_request_builder(client, provider, auth, "responses").await?;

            let input = vec![codex_protocol::models::ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![codex_protocol::models::ContentItem::InputText {
                    text: user_prompt.to_string(),
                }],
            }];

            let mut payload = json!({
                "model": model,
                "instructions": system_prompt,
                "input": input,
                "tools": [],
                "tool_choice": "auto",
                "parallel_tool_calls": false,
                "store": false,
                "stream": true,
                "include": [],
                "prompt_cache_key": security_review_session_id(),
            });

            if let Some(effort) = normalize_reasoning_effort_for_model(model, reasoning_effort)
                && let Some(map) = payload.as_object_mut()
            {
                map.insert("reasoning".to_string(), json!({ "effort": effort }));
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

fn is_rate_limit_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("rate limit")
        || lower.contains("rate_limit")
        || lower.contains("rate_limit_exceeded")
        || lower.contains("too many requests")
        || lower.contains("429")
}

fn is_transient_decode_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("error decoding response body")
        || lower.contains("transport error: network error")
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
    let mut saw_output_delta = false;

    let mut data_buffer = String::new();
    let mut event_name: Option<String> = None;

    for raw_line in body.lines() {
        let line = raw_line.trim_end_matches('\r');

        if let Some(rest) = line.strip_prefix("event:") {
            event_name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !data_buffer.is_empty() {
                data_buffer.push('\n');
            }
            data_buffer.push_str(rest.trim_start());
        } else if line.trim().is_empty() {
            if !data_buffer.is_empty() {
                handle_responses_event(
                    &data_buffer,
                    event_name.as_deref(),
                    &mut combined,
                    &mut reasoning,
                    &mut fallback,
                    &mut failed_error,
                    &mut last_parse_error,
                    &mut saw_output_delta,
                    metrics,
                );
                data_buffer.clear();
            }
            event_name = None;
        }
    }

    if !data_buffer.is_empty() {
        handle_responses_event(
            &data_buffer,
            event_name.as_deref(),
            &mut combined,
            &mut reasoning,
            &mut fallback,
            &mut failed_error,
            &mut last_parse_error,
            &mut saw_output_delta,
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
    event_name: Option<&str>,
    combined: &mut String,
    reasoning: &mut String,
    fallback: &mut Option<serde_json::Value>,
    failed_error: &mut Option<String>,
    last_parse_error: &mut Option<String>,
    saw_output_delta: &mut bool,
    metrics: &ReviewMetrics,
) {
    fn append_text(target: &mut String, text: &str) {
        if !text.is_empty() {
            target.push_str(text);
        }
    }

    fn extract_text_from_output_item(item: &serde_json::Value) -> Option<String> {
        match item.get("type").and_then(|t| t.as_str()) {
            Some("output_text") | Some("text") => item
                .get("text")
                .and_then(|t| t.as_str())
                .filter(|t| !t.is_empty())
                .map(ToString::to_string),
            Some("message") => {
                let Some(content) = item.get("content").and_then(|c| c.as_array()) else {
                    return None;
                };
                let mut combined = String::new();
                for block in content {
                    match block.get("type").and_then(|t| t.as_str()) {
                        Some("text") | Some("output_text") => {
                            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                combined.push_str(text);
                            }
                        }
                        _ => {}
                    }
                }
                if combined.is_empty() {
                    None
                } else {
                    Some(combined)
                }
            }
            _ => None,
        }
    }

    fn append_chat_completions_delta(event: &serde_json::Value, combined: &mut String) -> bool {
        let Some(choices) = event.get("choices").and_then(|v| v.as_array()) else {
            return false;
        };
        let mut appended = false;
        for choice in choices {
            if let Some(delta) = choice.get("delta") {
                if let Some(content) = delta
                    .get("content")
                    .and_then(|c| c.as_str())
                    .filter(|c| !c.is_empty())
                {
                    combined.push_str(content);
                    appended = true;
                } else if let Some(text) = delta
                    .get("text")
                    .and_then(|c| c.as_str())
                    .filter(|c| !c.is_empty())
                {
                    combined.push_str(text);
                    appended = true;
                }
            }
            if let Some(content) = choice
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .filter(|c| !c.is_empty())
            {
                combined.push_str(content);
                appended = true;
            } else if let Some(text) = choice
                .get("text")
                .and_then(|t| t.as_str())
                .filter(|t| !t.is_empty())
            {
                combined.push_str(text);
                appended = true;
            }
        }
        appended
    }

    let trimmed = data.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return;
    }

    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(event) => {
            let Some(kind) = event.get("type").and_then(|v| v.as_str()).or(event_name) else {
                if append_chat_completions_delta(&event, combined) {
                    *saw_output_delta = true;
                    return;
                }
                if failed_error.is_none()
                    && let Some(message) = event
                        .get("error")
                        .and_then(|err| err.get("message"))
                        .and_then(|m| m.as_str())
                {
                    *failed_error = Some(message.to_string());
                }
                return;
            };

            match kind {
                "response.output_text.delta" => {
                    if let Some(delta) = event.get("delta").and_then(|v| v.as_str()) {
                        append_text(combined, delta);
                        *saw_output_delta = true;
                    } else if let Some(delta_obj) = event.get("delta").and_then(|v| v.as_object()) {
                        if let Some(text) = delta_obj.get("text").and_then(|v| v.as_str()) {
                            append_text(combined, text);
                            *saw_output_delta = true;
                        } else if let Some(content) =
                            delta_obj.get("content").and_then(|v| v.as_array())
                        {
                            for block in content {
                                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                                    append_text(combined, text);
                                    *saw_output_delta = true;
                                }
                            }
                        }
                    }
                }
                "response.output_text.done" => {
                    if !*saw_output_delta
                        && let Some(text) = event
                            .get("text")
                            .and_then(|v| v.as_str())
                            .or_else(|| event.get("delta").and_then(|v| v.as_str()))
                        && text.trim().len() >= combined.trim().len()
                    {
                        combined.clear();
                        combined.push_str(text);
                    }
                }
                "response.output_item.added" | "response.output_item.done" => {
                    if !*saw_output_delta
                        && let Some(item) = event.get("item")
                        && let Some(text) = extract_text_from_output_item(item)
                    {
                        combined.push_str(&text);
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
            if let Some(kind) = event_name {
                match kind {
                    "response.output_text.delta" => {
                        append_text(combined, trimmed);
                        *saw_output_delta = true;
                        return;
                    }
                    "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => {
                        append_text(reasoning, trimmed);
                        return;
                    }
                    _ => {}
                }
            }
            if last_parse_error.is_none() {
                *last_parse_error = Some(format!("failed to parse SSE event: {err}"));
            }
        }
    }
}

#[cfg(test)]
mod responses_stream_parse_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_chat_completions_style_sse() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
            "data: [DONE]\n\n",
        );
        let metrics = ReviewMetrics::default();
        let output = parse_responses_stream_output(body, &metrics).expect("parsed output");
        assert_eq!(output.text, "Hello world");
    }

    #[test]
    fn parses_sse_event_name_without_type_field() {
        let body = concat!(
            "event: response.output_text.delta\n",
            "data: {\"delta\":\"1\"}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"delta\":\"2\"}\n\n",
            "event: response.completed\n",
            "data: {\"response\":{\"output\":[{\"type\":\"output_text\",\"text\":\"12\"}]}}\n\n",
        );
        let metrics = ReviewMetrics::default();
        let output = parse_responses_stream_output(body, &metrics).expect("parsed output");
        assert_eq!(output.text, "12");
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

fn parse_markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() || bytes[0] != b'#' {
        return None;
    }

    let mut level = 0usize;
    while level < bytes.len() && bytes[level] == b'#' {
        level += 1;
    }
    if level == 0 || level > 6 {
        return None;
    }

    let heading = trimmed[level..].trim_start();
    if heading.is_empty() {
        None
    } else {
        Some((level, heading))
    }
}

fn strip_markdown_sections<F>(markdown: &str, mut should_strip: F) -> String
where
    F: FnMut(&str) -> bool,
{
    let mut lines_out: Vec<String> = Vec::new();
    let mut skip_level: Option<usize> = None;

    for line in markdown.lines() {
        if let Some((level, heading)) = parse_markdown_heading(line) {
            if let Some(active) = skip_level
                && level <= active
            {
                skip_level = None;
            }
            if skip_level.is_none() && should_strip(heading) {
                skip_level = Some(level);
                continue;
            }
        }

        if skip_level.is_none() {
            lines_out.push(line.to_string());
        }
    }

    lines_out.join("\n")
}

fn is_dev_setup_heading(heading: &str) -> bool {
    let lower = heading.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }

    if lower == "ci" {
        return true;
    }

    if let Some(rest) = lower.strip_prefix("ci")
        && let Some(next) = rest.chars().next()
        && matches!(next, ' ' | '/' | '-' | '_' | ':' | '.')
    {
        return true;
    }

    let prefixes = [
        "ci/cd",
        "cicd",
        "continuous integration",
        "github actions",
        "gitlab ci",
        "circleci",
        "buildkite",
        "pipeline",
        "pipelines",
        "fuzz",
        "fuzzing",
        "oss-fuzz",
        "testing",
        "tests",
        "test suite",
        "bench",
        "benches",
        "benchmark",
        "benchmarks",
        "development setup",
        "developer setup",
        "dev setup",
        "local development",
        "build & test",
        "build and test",
        "build/test",
        "build pipeline",
        "contributing",
    ];
    prefixes.iter().any(|prefix| lower.starts_with(prefix))
}

fn strip_dev_setup_sections(markdown: &str) -> String {
    strip_markdown_sections(markdown, is_dev_setup_heading)
}

fn strip_operational_considerations_section(markdown: &str) -> String {
    strip_markdown_sections(markdown, |heading| {
        heading
            .trim()
            .to_ascii_lowercase()
            .starts_with("operational considerations")
    })
}

#[cfg(test)]
mod report_stripping_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn strips_dev_setup_sections_with_nested_headings() {
        let input = "\
# Report
## Overview
Keep this.
## CI/CD
Drop this.
### GitHub Actions
Also drop this.
## Core
Keep that.";
        let expected = "\
# Report
## Overview
Keep this.
## Core
Keep that.";
        assert_eq!(strip_dev_setup_sections(input), expected);
    }

    #[test]
    fn strips_operational_considerations_with_nested_headings() {
        let input = "\
# Report
## Overview
Keep this.
## Operational Considerations
Drop this.
### Rotations
Still drop this.
## Core
Keep that.";
        let expected = "\
# Report
## Overview
Keep this.
## Core
Keep that.";
        assert_eq!(strip_operational_considerations_section(input), expected);
    }
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

fn nest_threat_model_subsections(markdown: String) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut seen_trust_boundaries = false;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let Some((hashes, rest)) = trimmed.split_once(' ') else {
            out.push(line.to_string());
            continue;
        };
        if hashes.is_empty() || !hashes.chars().all(|ch| ch == '#') {
            out.push(line.to_string());
            continue;
        }
        let heading_text = rest.trim();
        let normalized = heading_text
            .trim_matches('`')
            .trim()
            .trim_end_matches(':')
            .to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "trust boundaries" | "trust boundary" | "trust-boundaries"
        ) {
            seen_trust_boundaries = true;
            out.push("### Trust Boundaries".to_string());
            continue;
        }
        if matches!(
            normalized.as_str(),
            "components & trust boundary diagram" | "components and trust boundary diagram"
        ) {
            if seen_trust_boundaries {
                out.push("#### Diagram".to_string());
            } else {
                seen_trust_boundaries = true;
                out.push("### Trust Boundaries".to_string());
                out.push(String::new());
                out.push("#### Diagram".to_string());
            }
            continue;
        }
        let is_threat_model_subsection = matches!(
            normalized.as_str(),
            "primary components" | "assets" | "attacker model" | "entry points" | "top abuse paths"
        );
        if is_threat_model_subsection {
            let title = heading_text.trim_matches('`').trim().trim_end_matches(':');
            out.push(format!("### {title}"));
            continue;
        }
        out.push(line.to_string());
    }
    out.join("\n")
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

const MARKDOWN_FIX_MODEL: &str = "gpt-5.2";
