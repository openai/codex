#![allow(dead_code)]

use crate::app_event::AppEvent;
use crate::app_event::SecurityReviewAutoScopeSelection;
use crate::app_event::SecurityReviewCommandState;
use crate::app_event_sender::AppEventSender;
use crate::diff_render::display_path_for;
use crate::mermaid::fix_mermaid_blocks;
use crate::security_prompts::*;
use crate::security_report_viewer::build_report_html;
use crate::status_indicator_widget::fmt_elapsed_compact;
use crate::text_formatting::truncate_text;
use codex_core::CodexAuth;
use codex_core::ModelProviderInfo;
use codex_core::WireApi;
use codex_core::config::GPT_5_CODEX_MEDIUM_MODEL;
use codex_core::default_retry_backoff;
use codex_core::git_info::collect_git_info;
use codex_core::git_info::get_git_repo_root;
use codex_core::protocol::TokenUsage;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use pathdiff::diff_paths;
use regex::Regex;
use reqwest::Client;
use reqwest::header::ACCEPT;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;
use std::cmp::Ordering as CmpOrdering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::future::Future;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
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
const FILE_TRIAGE_CHUNK_SIZE: usize = 20;
const FILE_TRIAGE_CONCURRENCY: usize = 8;
const MAX_SEARCH_REQUESTS_PER_FILE: usize = 3;
const MAX_SEARCH_OUTPUT_CHARS: usize = 4_000;
const MAX_COMMAND_ERROR_RETRIES: usize = 10;
const MAX_SEARCH_PATTERN_LEN: usize = 256;
const MAX_FILE_SEARCH_RESULTS: usize = 40;
const MAX_FILE_ANALYSIS_ATTEMPTS: usize = 2;
// Number of full passes over the triaged files during bug finding.
// Not related to per-file search/tool attempts. Defaults to 3.
const BUG_FINDING_PASSES: usize = 1;
const BUG_POLISH_CONCURRENCY: usize = 8;
const COMMAND_PREVIEW_MAX_LINES: usize = 2;
const COMMAND_PREVIEW_MAX_GRAPHEMES: usize = 96;
const MODEL_REASONING_LOG_MAX_GRAPHEMES: usize = 240;
const AUTO_SCOPE_MODEL: &str = "gpt-5-codex";
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) enum SecurityReviewMode {
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
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            SecurityReviewMode::Full => "full",
            SecurityReviewMode::Bugs => "bugs",
        }
    }
}

#[derive(Clone)]
pub(crate) struct SecurityReviewRequest {
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
    pub progress_sender: Option<AppEventSender>,
    // When true, accept auto-scoped directories without a confirmation dialog.
    pub skip_auto_scope_confirmation: bool,
    pub auto_scope_prompt: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct SecurityReviewResult {
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
}

#[derive(Clone, Debug)]
pub(crate) struct SecurityReviewFailure {
    pub message: String,
    pub logs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SecurityReviewMetadata {
    pub mode: SecurityReviewMode,
    #[serde(default)]
    pub scope_paths: Vec<String>,
}

pub(crate) fn read_security_review_metadata(path: &Path) -> Result<SecurityReviewMetadata, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_json::from_slice::<SecurityReviewMetadata>(&bytes)
        .map_err(|e| format!("Failed to parse {}: {e}", path.display()))
}

#[derive(Clone)]
struct FileSnippet {
    relative_path: PathBuf,
    language: String,
    content: String,
    bytes: usize,
}

struct FileCollectionResult {
    snippets: Vec<FileSnippet>,
    logs: Vec<String>,
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
    git_blame_calls: AtomicUsize,
    command_seq: AtomicU64,

    // Aggregated token usage across all model calls
    input_tokens: std::sync::atomic::AtomicU64,
    cached_input_tokens: std::sync::atomic::AtomicU64,
    output_tokens: std::sync::atomic::AtomicU64,
    reasoning_output_tokens: std::sync::atomic::AtomicU64,
    total_tokens: std::sync::atomic::AtomicU64,
}

#[derive(Clone, Copy)]
enum ToolCallKind {
    Search,
    GrepFiles,
    ReadFile,
    GitBlame,
}

struct MetricsSnapshot {
    model_calls: usize,
    search_calls: usize,
    grep_files_calls: usize,
    read_calls: usize,
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
        input_tokens: u64,
        cached_input_tokens: u64,
        output_tokens: u64,
        reasoning_output_tokens: u64,
        total_tokens: u64,
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
}

struct FileBugResult {
    index: usize,
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

#[derive(Clone)]
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

struct SpecGenerationOutcome {
    combined_markdown: String,
    locations: Vec<String>,
    logs: Vec<String>,
    api_entries: Vec<ApiEntry>,
    classification_rows: Vec<DataClassificationRow>,
    classification_table: Option<String>,
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
    start: Option<usize>,
    end: Option<usize>,
    metrics: &ReviewMetrics,
) -> Result<String, String> {
    metrics.record_tool_call(ToolCallKind::ReadFile);
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
    if !canonical.is_file() {
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
pub(crate) enum BugValidationStatus {
    #[default]
    Pending,
    Passed,
    Failed,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct BugValidationState {
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
pub(crate) struct SecurityReviewBug {
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
        // If there is no explicit Assignee/Author/Owner line, and we have a GitHub handle extracted
        // from blame metadata, add an Assignee line to help prepopulate the UI.
        let lower = composed.to_ascii_lowercase();
        let has_assignee =
            lower.contains("assignee:") || lower.contains("author:") || lower.contains("owner:");
        if !has_assignee && let Some(handle) = snapshot.bug.assignee_github.as_ref() {
            let line = format!("\n\nAssignee: {handle}\n");
            composed.push_str(&line);
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
    if git_link_info.is_none() {
        return markdown.to_string();
    }
    let info = git_link_info.unwrap();
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

async fn polish_bug_markdowns(
    client: &Client,
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

#[derive(Clone)]
struct GitLinkInfo {
    repo_root: PathBuf,
    github_prefix: String,
}

struct BugPromptData {
    prompt: String,
    logs: Vec<String>,
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

pub(crate) async fn run_security_review(
    request: SecurityReviewRequest,
) -> Result<SecurityReviewResult, SecurityReviewFailure> {
    let progress_sender = request.progress_sender.clone();
    let mut logs = Vec::new();
    let metrics = Arc::new(ReviewMetrics::default());
    let client = Client::new();
    let overall_start = Instant::now();

    let mut record = |line: String| {
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(line.clone()));
        }
        logs.push(line);
    };

    record(format!(
        "Starting security review in {} (mode: {}, model: {})",
        request.repo_path.display(),
        request.mode.as_str(),
        request.model
    ));
    let repo_path = request.repo_path.clone();
    let mut include_paths = request.include_paths.clone();
    let git_link_info = build_git_link_info(&repo_path).await;

    if matches!(request.mode, SecurityReviewMode::Bugs)
        && include_paths.is_empty()
        && let Some(prompt) = request.auto_scope_prompt.as_ref().and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    {
        let auto_scope_model = if request.triage_model.trim().is_empty() {
            AUTO_SCOPE_MODEL
        } else {
            request.triage_model.as_str()
        };

        record(format!(
            "Auto-detecting review scope from user prompt: {prompt}"
        ));
        match auto_detect_scope(
            &client,
            &request.provider,
            &request.auth,
            auto_scope_model,
            &repo_path,
            prompt,
            metrics.clone(),
        )
        .await
        {
            Ok((selections, scope_logs)) => {
                for line in scope_logs {
                    record(line);
                }
                if selections.is_empty() {
                    record(
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
                        record(message);
                        resolved_paths.push(abs_path);
                        selection_summaries.push((display_path, reason));
                    }

                    if let Some(tx) = request.progress_sender.as_ref() {
                        let display_paths: Vec<String> = selection_summaries
                            .iter()
                            .map(|(path, _)| path.clone())
                            .collect();

                        if request.skip_auto_scope_confirmation {
                            // Option 2 (Quick bug sweep): auto-accept detected scope and continue.
                            include_paths = resolved_paths;
                            record("Auto scope selections accepted.".to_string());
                            tx.send(AppEvent::SecurityReviewScopeResolved {
                                paths: display_paths,
                            });
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
                                mode: request.mode,
                                prompt: prompt.to_string(),
                                selections: selections_for_ui,
                                responder: confirm_tx,
                            });

                            record(
                                "Waiting for user confirmation of auto-detected scope..."
                                    .to_string(),
                            );

                            match confirm_rx.await {
                                Ok(true) => {
                                    record("Auto scope confirmed by user.".to_string());
                                    include_paths = resolved_paths;
                                    tx.send(AppEvent::SecurityReviewScopeResolved {
                                        paths: display_paths,
                                    });
                                }
                                Ok(false) => {
                                    record(
                                        "Auto scope selection rejected by user; cancelling review."
                                            .to_string(),
                                    );
                                    tx.send(AppEvent::OpenSecurityReviewPathPrompt(request.mode));
                                    return Err(SecurityReviewFailure {
                                        message:
                                            "Security review cancelled after auto scope rejection."
                                                .to_string(),
                                        logs,
                                    });
                                }
                                Err(_) => {
                                    record(
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
                record(format!("Auto scope detection failed: {}", failure.message));
                for line in failure.logs {
                    record(line);
                }
            }
        }
    }

    record("Collecting candidate files...".to_string());

    let progress_sender_for_collection = progress_sender.clone();
    let collection_paths = include_paths.clone();
    let collection = match spawn_blocking(move || {
        collect_snippets_blocking(
            repo_path,
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
            record(format!("File collection task failed: {e}"));
            return Err(SecurityReviewFailure {
                message: format!("File collection task failed: {e}"),
                logs,
            });
        }
    };

    for line in collection.logs {
        record(line);
    }

    if collection.snippets.is_empty() {
        record("No candidate files found for review.".to_string());
        return Err(SecurityReviewFailure {
            message: "No candidate files found for review.".to_string(),
            logs,
        });
    }

    record("Running LLM file triage to prioritize analysis...".to_string());
    let triage = match triage_files_for_bug_analysis(
        &client,
        &request.provider,
        &request.auth,
        &request.triage_model,
        collection.snippets,
        progress_sender.clone(),
        metrics.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            record(err.message.clone());
            let mut combined_logs = logs.clone();
            combined_logs.extend(err.logs);
            return Err(SecurityReviewFailure {
                message: err.message,
                logs: combined_logs,
            });
        }
    };

    for line in &triage.logs {
        record(line.clone());
    }

    let selected_snippets = triage.included;

    if selected_snippets.is_empty() {
        record("No files selected for bug analysis after triage.".to_string());
        return Err(SecurityReviewFailure {
            message: "No files selected for bug analysis after triage.".to_string(),
            logs,
        });
    }

    let total_bytes = selected_snippets.iter().map(|s| s.bytes).sum::<usize>();
    let total_size = human_readable_bytes(total_bytes);
    record(format!(
        "Preparing bug analysis for {} files ({} total).",
        selected_snippets.len(),
        total_size
    ));

    let mut spec_targets: Vec<PathBuf> = if !include_paths.is_empty() {
        include_paths.clone()
    } else {
        let mut unique_dirs: HashSet<PathBuf> = HashSet::new();
        for snippet in &selected_snippets {
            let absolute = request.repo_path.join(&snippet.relative_path);
            let dir = absolute.parent().unwrap_or(&request.repo_path);
            unique_dirs.insert(dir.to_path_buf());
        }
        if unique_dirs.is_empty() {
            vec![request.repo_path.clone()]
        } else {
            unique_dirs.into_iter().collect()
        }
    };

    spec_targets.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    let spec_generation = if matches!(request.mode, SecurityReviewMode::Full) {
        record(format!(
            "Generating system specifications for {} scope path(s).",
            spec_targets.len()
        ));
        match generate_specs(
            &client,
            &request.provider,
            &request.auth,
            &request.repo_path,
            &spec_targets,
            &request.output_root,
            progress_sender.clone(),
            metrics.clone(),
        )
        .await
        {
            Ok(Some(spec)) => {
                for line in &spec.logs {
                    record(line.clone());
                }
                Some(spec)
            }
            Ok(None) => {
                record("Specification step skipped (no targets).".to_string());
                None
            }
            Err(err) => {
                record(err.message.clone());
                let mut combined_logs = logs.clone();
                combined_logs.extend(err.logs);
                return Err(SecurityReviewFailure {
                    message: err.message,
                    logs: combined_logs,
                });
            }
        }
    } else {
        None
    };

    let repository_summary = build_repository_summary(&selected_snippets);

    let spec_for_bug_analysis = if request.include_spec_in_bug_analysis {
        if let Some(spec) = spec_generation.as_ref() {
            record("Including specification context in bug analysis prompts.".to_string());
            Some(spec.combined_markdown.as_str())
        } else {
            record(
                "Specification context unavailable; bug analysis prompts will omit it.".to_string(),
            );
            None
        }
    } else {
        if spec_generation.is_some() {
            record(
                "Skipping specification context in bug analysis prompts (disabled by config)."
                    .to_string(),
            );
        }
        None
    };

    let threat_model = if matches!(request.mode, SecurityReviewMode::Full) {
        if let Some(spec) = spec_generation.as_ref() {
            match generate_threat_model(
                &client,
                &request.provider,
                &request.auth,
                THREAT_MODEL_MODEL,
                &repository_summary,
                &request.repo_path,
                spec,
                &request.output_root,
                progress_sender.clone(),
                metrics.clone(),
            )
            .await
            {
                Ok(Some(threat)) => {
                    for line in &threat.logs {
                        record(line.clone());
                    }
                    Some(threat)
                }
                Ok(None) => None,
                Err(err) => {
                    record(err.message.clone());
                    let mut combined_logs = logs.clone();
                    combined_logs.extend(err.logs);
                    return Err(SecurityReviewFailure {
                        message: err.message,
                        logs: combined_logs,
                    });
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Run bug analysis in N full passes across all selected files.
    let total_passes = BUG_FINDING_PASSES.max(1);
    record(format!("Running bug analysis in {total_passes} pass(es)."));

    let mut aggregated_logs: Vec<String> = Vec::new();
    let mut all_summaries: Vec<BugSummary> = Vec::new();
    let mut all_details: Vec<BugDetail> = Vec::new();
    use std::collections::HashMap as StdHashMap;
    let mut files_map: StdHashMap<PathBuf, FileSnippet> = StdHashMap::new();

    for pass in 1..=total_passes {
        record(format!(
            "Starting bug analysis pass {}/{} over {} files.",
            pass,
            total_passes,
            selected_snippets.len()
        ));

        let pass_outcome = match analyze_files_individually(
            &client,
            &request.provider,
            &request.auth,
            &request.model,
            &repository_summary,
            spec_for_bug_analysis,
            &request.repo_path,
            &selected_snippets,
            git_link_info.clone(),
            progress_sender.clone(),
            metrics.clone(),
        )
        .await
        {
            Ok(outcome) => outcome,
            Err(err) => {
                record(err.message.clone());
                let mut combined_logs = logs.clone();
                combined_logs.extend(err.logs);
                return Err(SecurityReviewFailure {
                    message: err.message,
                    logs: combined_logs,
                });
            }
        };

        for line in &pass_outcome.logs {
            record(line.clone());
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

        record(format!(
            "Completed bug analysis pass {pass}/{total_passes}."
        ));
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
        record(msg.clone());
        aggregated_logs.push(msg);
    }
    if all_summaries.is_empty() {
        let msg = "No high, medium, or low severity findings remain after filtering.".to_string();
        record(msg.clone());
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
            record(msg.clone());
            aggregated_logs.push(msg);
        }
    }

    // Run risk rerank after deduplication to avoid redundant work.
    if !all_summaries.is_empty() {
        let risk_logs = rerank_bugs_by_risk(
            &client,
            &request.provider,
            &request.auth,
            &request.model,
            &mut all_summaries,
            &request.repo_path,
            &repository_summary,
            spec_for_bug_analysis,
            metrics.clone(),
        )
        .await;
        aggregated_logs.extend(risk_logs.clone());
        for line in risk_logs {
            record(line);
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
            if let Some(updated) =
                rewrite_bug_markdown_severity(summary.markdown.as_str(), summary.severity.as_str())
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
            record(msg.clone());
            aggregated_logs.push(msg);
        }

        normalize_bug_identifiers(&mut all_summaries, &mut all_details);
    }

    if !all_summaries.is_empty() {
        let polish_message = format!(
            "Polishing markdown for {} bug finding(s).",
            all_summaries.len()
        );
        record(polish_message.clone());
        aggregated_logs.push(polish_message);
        let polish_logs = match polish_bug_markdowns(
            &client,
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
            record(line.clone());
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
    let bug_markdown = if all_summaries.is_empty() {
        "No high, medium, or low severity findings.".to_string()
    } else {
        all_summaries
            .iter()
            .map(|summary| summary.markdown.clone())
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    record(format!(
        "Aggregated bug findings across {} file(s).",
        files_with_findings.len()
    ));
    aggregated_logs.push(format!(
        "Aggregated bug findings across {} file(s).",
        files_with_findings.len()
    ));

    let bug_summary_table = make_bug_summary_table(&all_summaries);
    let bug_summaries = all_summaries;
    let bug_details = all_details;

    let findings_summary = format_findings_summary(findings_count, files_with_findings.len());
    record(format!(
        "Bug analysis summary: {}",
        findings_summary.as_str()
    ));
    record("Bug analysis complete.".to_string());

    let (bugs_for_result, bug_snapshots) = build_bug_records(bug_summaries, bug_details);
    let mut bugs_markdown = bug_markdown.clone();
    if let Some(table) = bug_summary_table.as_ref() {
        bugs_markdown = format!("{table}\n\n{bugs_markdown}");
    }
    bugs_markdown = fix_mermaid_blocks(&bugs_markdown);

    let mut report_sections_prefix: Vec<String> = Vec::new();
    if matches!(request.mode, SecurityReviewMode::Full) {
        if let Some(spec) = spec_generation.as_ref() {
            record("Including combined specification in final report.".to_string());
            let trimmed = spec.combined_markdown.trim();
            if !trimmed.is_empty() {
                report_sections_prefix.push(trimmed.to_string());
            }
        }
        if let Some(threat) = threat_model.as_ref() {
            record("Including threat model in final report.".to_string());
            let trimmed = threat.markdown.trim();
            if !trimmed.is_empty() {
                report_sections_prefix.push(trimmed.to_string());
            }
        }
    }
    let findings_section = if bugs_markdown.trim().is_empty() {
        None
    } else {
        Some(format!("# Security Findings\n\n{}", bugs_markdown.trim()))
    };
    let report_markdown = match request.mode {
        SecurityReviewMode::Full => {
            let mut sections = report_sections_prefix.clone();
            if let Some(section) = findings_section.clone() {
                sections.push(section);
            }
            if sections.is_empty() {
                record("No content available for final report.".to_string());
                None
            } else {
                record(
                    "Final report assembled from specification, threat model, and findings."
                        .to_string(),
                );
                Some(fix_mermaid_blocks(&sections.join("\n\n")))
            }
        }
        SecurityReviewMode::Bugs => {
            if let Some(section) = findings_section {
                record("Generated findings-only report for bug sweep.".to_string());
                Some(fix_mermaid_blocks(&section))
            } else {
                record("No findings available for bug sweep report.".to_string());
                None
            }
        }
    };

    let snapshot = SecurityReviewSnapshot {
        generated_at: OffsetDateTime::now_utc(),
        findings_summary: findings_summary.clone(),
        report_sections_prefix: report_sections_prefix.clone(),
        bugs: bug_snapshots.clone(),
    };

    // Intentionally avoid logging the output path pre-write to keep logs concise.
    let metadata = SecurityReviewMetadata {
        mode: request.mode,
        scope_paths: request.scope_display_paths.clone(),
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
        &request.repo_path,
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
            record(format!(
                "Artifacts written to {}",
                request.output_root.display()
            ));
            if let Some(ref report) = paths.report_path {
                record(format!("  • Report markdown: {}", report.display()));
            }
            if let Some(ref html) = paths.report_html_path {
                record(format!("  • Report HTML: {}", html.display()));
            }
            paths
        }
        Err(err) => {
            record(format!("Failed to write artifacts: {err}"));
            return Err(SecurityReviewFailure {
                message: format!("Failed to write artifacts: {err}"),
                logs,
            });
        }
    };

    let elapsed = overall_start.elapsed();
    let metrics_snapshot = metrics.snapshot();
    let elapsed_secs = elapsed.as_secs_f32();
    let tool_summary = metrics_snapshot.tool_call_summary();
    record(format!(
        "Security review duration: {elapsed_secs:.1}s (model calls: {model_calls}; tool calls: {tool_summary}).",
        model_calls = metrics_snapshot.model_calls,
    ));
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
        token_usage: metrics.snapshot_usage(),
    })
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

#[allow(clippy::needless_collect)]
async fn triage_files_for_bug_analysis(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    triage_model: &str,
    snippets: Vec<FileSnippet>,
    progress_sender: Option<AppEventSender>,
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

    let start_message = format!("Running LLM triage over {total} file(s) to prioritize analysis.");
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(start_message.clone()));
    }
    logs.push(start_message);

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
        tx.send(AppEvent::SecurityReviewLog(
            "    Sample tool calls (simulated):".to_string(),
        ));
        tx.send(AppEvent::SecurityReviewLog(
            "  └ SEARCH literal:'auth'".to_string(),
        ));
        tx.send(AppEvent::SecurityReviewLog(
            "    GREP_FILES: {\"pattern\": \"token|password\", \"include\": \"*.rs\"}".to_string(),
        ));
    }

    for _ in 0..concurrency {
        if let Some(request) = remaining.next() {
            in_flight.push(triage_chunk(
                client.clone(),
                provider.clone(),
                auth.clone(),
                triage_model.to_string(),
                request,
                progress_sender.clone(),
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
                        client.clone(),
                        provider.clone(),
                        auth.clone(),
                        triage_model.to_string(),
                        next_request,
                        progress_sender.clone(),
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
        logs.push("LLM triage excluded all files.".to_string());
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

    logs.push(format!(
        "File triage selected {} of {} files (excluded {}).",
        included.len(),
        total,
        total.saturating_sub(included.len())
    ));

    Ok(FileTriageResult { included, logs })
}

async fn triage_chunk(
    client: Client,
    provider: ModelProviderInfo,
    auth: Option<CodexAuth>,
    triage_model: String,
    request: FileTriageChunkRequest,
    progress_sender: Option<AppEventSender>,
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

    let response = await_with_heartbeat(
        progress_sender.clone(),
        "running file triage",
        Some(detail.as_str()),
        call_model(
            &client,
            &provider,
            &auth,
            &triage_model,
            FILE_TRIAGE_SYSTEM_PROMPT,
            &FILE_TRIAGE_PROMPT_TEMPLATE.replace("{files}", &listing),
            metrics.clone(),
            0.0,
        ),
    )
    .await;

    let response_output = match response {
        Ok(output) => output,
        Err(err) => {
            let message = format!("File triage failed: {err}");
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(message.clone()));
            }
            return Err(SecurityReviewFailure {
                message,
                logs: vec![],
            });
        }
    };
    let mut chunk_logs = Vec::new();
    if let Some(reasoning) = response_output.reasoning.as_ref() {
        for line in reasoning
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
            let msg = format!("Model reasoning: {truncated}");
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(msg.clone()));
            }
            chunk_logs.push(msg);
        }
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

async fn generate_specs(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    repo_root: &Path,
    include_paths: &[PathBuf],
    output_root: &Path,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
) -> Result<Option<SpecGenerationOutcome>, SecurityReviewFailure> {
    let mut targets: Vec<PathBuf> = if include_paths.is_empty() {
        vec![repo_root.to_path_buf()]
    } else {
        include_paths.to_vec()
    };

    if targets.is_empty() {
        return Ok(None);
    }

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

    let mut logs: Vec<String> = Vec::new();

    let mut directory_candidates: Vec<(PathBuf, String)> = normalized
        .into_iter()
        .map(|path| {
            let label = display_path_for(&path, repo_root);
            (path, label)
        })
        .collect();
    directory_candidates.sort_by(|a, b| a.1.cmp(&b.1));

    let filtered_dirs = match filter_spec_directories(
        client,
        provider,
        auth,
        repo_root,
        &directory_candidates,
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
    };

    let normalized: Vec<PathBuf> = if filtered_dirs.is_empty() {
        directory_candidates
            .iter()
            .map(|(path, _)| path.clone())
            .collect()
    } else {
        filtered_dirs.iter().map(|(path, _)| path.clone()).collect()
    };

    if let Some(tx) = progress_sender.as_ref() {
        let kept = normalized.len();
        let total = directory_candidates.len();
        let message = format!(
            "Spec directory filter kept {kept}/{total} directories using {SPEC_GENERATION_MODEL}."
        );
        tx.send(AppEvent::SecurityReviewLog(message.clone()));
        logs.push(message);
    }

    let mut display_locations: Vec<String> = normalized
        .iter()
        .map(|p| display_path_for(p, repo_root))
        .collect();
    display_locations.sort();

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

    let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
    for path in normalized {
        in_flight.push(generate_spec_for_location(
            client.clone(),
            provider.clone(),
            auth.clone(),
            repo_root.to_path_buf(),
            path,
            display_locations.clone(),
            raw_dir.clone(),
            progress_sender.clone(),
            metrics.clone(),
        ));
    }

    let mut spec_entries: Vec<SpecEntry> = Vec::new();

    while let Some(result) = in_flight.next().await {
        match result {
            Ok((entry, mut entry_logs)) => {
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
    client: &Client,
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
    let include = map
        .get("include")
        .and_then(parse_include_flag)
        .unwrap_or(false);
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

async fn auto_detect_scope(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    repo_root: &Path,
    user_query: &str,
    metrics: Arc<ReviewMetrics>,
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

    let mut conversation: Vec<String> = Vec::new();
    let mut tool_rounds = 0usize;

    if !keywords.is_empty() {
        let mut initial_search_count = 0usize;
        for keyword in keywords.iter().take(AUTO_SCOPE_INITIAL_KEYWORD_PROBES) {
            let trimmed = keyword.trim();
            if trimmed.is_empty() {
                continue;
            }
            let term = trimmed.to_string();
            initial_search_count += 1;
            logs.push(format!(
                "Auto scope executing initial SEARCH literal:{term}."
            ));
            conversation.push(format!("Assistant:\nSEARCH: literal:{term}"));
            let (log_line, output) =
                execute_auto_scope_search_content(repo_root, &term, SearchMode::Literal, &metrics)
                    .await;
            logs.push(log_line);
            conversation.push(format!("Tool SEARCH `{term}`:\n{output}"));
            let preview = command_preview_snippets(&output);
            if !preview.is_empty() {
                logs.push(format!(
                    "Auto scope SEARCH literal:{term} preview:\n{}",
                    preview.join("\n")
                ));
            }

            let pattern_str = term.clone();
            let grep_limit = Some(200usize);
            let grep_args = GrepFilesArgs {
                pattern: pattern_str.clone(),
                include: None,
                path: None,
                limit: grep_limit,
            };
            let mut shown = serde_json::json!({ "pattern": pattern_str });
            if let Some(limit) = grep_limit {
                shown["limit"] = serde_json::Value::Number(serde_json::Number::from(limit as u64));
            }
            let shown_text = shown.to_string();
            logs.push(format!(
                "Auto scope executing initial GREP_FILES {shown_text}."
            ));
            conversation.push(format!("Assistant:\nGREP_FILES: {shown_text}"));
            let (grep_log, grep_output) =
                match run_grep_files(repo_root, &grep_args, &metrics).await {
                    SearchResult::Matches(out) => (
                        format!("Auto scope file search `{term}` returned matching files."),
                        out,
                    ),
                    SearchResult::NoMatches => (
                        format!("Auto scope file search `{term}` returned no matches."),
                        "No matches found.".to_string(),
                    ),
                    SearchResult::Error(err) => (
                        format!("Auto scope file search `{term}` failed: {err}"),
                        format!("Search error: {err}"),
                    ),
                };
            logs.push(grep_log);
            conversation.push(format!("Tool GREP_FILES {shown_text}:\n{grep_output}"));
            let preview = command_preview_snippets(&grep_output);
            if !preview.is_empty() {
                logs.push(format!(
                    "Auto scope GREP_FILES {shown_text} preview:\n{}",
                    preview.join("\n")
                ));
            }
        }
        if initial_search_count > 0 {
            let label = if initial_search_count == 1 {
                "search"
            } else {
                "searches"
            };
            logs.push(format!(
                "Auto scope seeded the agent loop with {initial_search_count} initial keyword {label} (content + file matches)."
            ));
        }
    }

    let repo_overview = summarize_top_level(repo_root);

    loop {
        if tool_rounds >= AUTO_SCOPE_MAX_AGENT_STEPS {
            return Err(SecurityReviewFailure {
                message: format!(
                    "Auto scope exceeded the maximum number ({AUTO_SCOPE_MAX_AGENT_STEPS}) of tool interactions."
                ),
                logs,
            });
        }

        let conversation_text = conversation.join("\n\n");
        let prompt =
            build_auto_scope_prompt(&repo_overview, user_query, &keywords, &conversation_text);
        let response = match call_model(
            client,
            provider,
            auth,
            model,
            AUTO_SCOPE_SYSTEM_PROMPT,
            &prompt,
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
                        logs.push(format!("Model reasoning: {truncated}"));
                    }
                }
                output.text
            }
            Err(err) => {
                logs.push(format!("Auto scope model request failed: {err}"));
                return Err(SecurityReviewFailure {
                    message: format!("Failed to auto-detect scope: {err}"),
                    logs,
                });
            }
        };

        let assistant_reply = response.trim();
        if assistant_reply.is_empty() {
            return Err(SecurityReviewFailure {
                message: "Auto scope model returned an empty response.".to_string(),
                logs,
            });
        }
        conversation.push(format!("Assistant:\n{assistant_reply}"));

        let commands = extract_auto_scope_commands(assistant_reply);
        if !commands.is_empty() {
            tool_rounds += 1;
            for command in commands {
                match command {
                    AutoScopeToolCommand::SearchContent { pattern, mode } => {
                        let (log_line, output) =
                            execute_auto_scope_search_content(repo_root, &pattern, mode, &metrics)
                                .await;
                        logs.push(log_line);
                        conversation.push(format!("Tool SEARCH `{pattern}`:\n{output}"));
                    }
                    AutoScopeToolCommand::GrepFiles(args) => {
                        let (log_line, output) =
                            match run_grep_files(repo_root, &args, &metrics).await {
                                SearchResult::Matches(out) => (
                                    "Auto scope grep_files search returned results.".to_string(),
                                    out,
                                ),
                                SearchResult::NoMatches => (
                                    "Auto scope grep_files search returned no matches.".to_string(),
                                    "No matches found.".to_string(),
                                ),
                                SearchResult::Error(err) => (
                                    format!("Auto scope grep_files search failed: {err}"),
                                    format!("Search error: {err}"),
                                ),
                            };
                        logs.push(log_line);
                        // Show the tool line with compact JSON for reproducibility
                        let mut shown = serde_json::json!({
                            "pattern": args.pattern,
                        });
                        if let Some(ref inc) = args.include {
                            shown["include"] = serde_json::Value::String(inc.clone());
                        }
                        if let Some(ref p) = args.path {
                            shown["path"] = serde_json::Value::String(p.clone());
                        }
                        if let Some(l) = args.limit {
                            shown["limit"] =
                                serde_json::Value::Number(serde_json::Number::from(l as u64));
                        }
                        conversation.push(format!("Tool GREP_FILES {shown}:\n{output}"));
                    }
                    AutoScopeToolCommand::ReadFile { path, start, end } => {
                        match execute_auto_scope_read(
                            repo_root,
                            &path,
                            start,
                            end,
                            metrics.as_ref(),
                        )
                        .await
                        {
                            Ok(output) => {
                                logs.push(format!(
                                    "Auto scope read `{}` returned content.",
                                    path.display()
                                ));
                                conversation.push(format!(
                                    "Tool READ `{}`:\n{}",
                                    path.display(),
                                    output
                                ));
                            }
                            Err(err) => {
                                logs.push(err.clone());
                                conversation.push(format!(
                                    "Tool READ `{}` error: {}",
                                    path.display(),
                                    err
                                ));
                            }
                        }
                    }
                }
            }
            continue;
        }

        let parse_result = parse_auto_scope_response(assistant_reply);
        match parse_result {
            AutoScopeParseResult::All => {
                let canonical = repo_root
                    .canonicalize()
                    .unwrap_or_else(|_| repo_root.to_path_buf());
                logs.push("Auto scope model requested the entire repository.".to_string());
                return Ok((
                    vec![AutoScopeSelection {
                        display_path: display_path_for(&canonical, repo_root),
                        abs_path: canonical,
                        reason: Some("LLM requested full repository".to_string()),
                        is_dir: true,
                    }],
                    logs,
                ));
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

                // Prefer specific children over broad parents, then cap to max.
                prune_auto_scope_parent_child_overlaps(&mut selections, &mut logs);
                truncate_auto_scope_selections(&mut selections, &mut logs);

                return Ok((selections, logs));
            }
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

async fn filter_spec_directories(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    repo_root: &Path,
    candidates: &[(PathBuf, String)],
    metrics: Arc<ReviewMetrics>,
) -> Result<Vec<(PathBuf, String)>, SecurityReviewFailure> {
    if candidates.len() <= SPEC_DIR_FILTER_TARGET {
        return Ok(candidates.to_vec());
    }

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
        logs: Vec::new(),
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
async fn generate_spec_for_location(
    client: Client,
    provider: ModelProviderInfo,
    auth: Option<CodexAuth>,
    repo_root: PathBuf,
    target: PathBuf,
    project_locations: Vec<String>,
    raw_dir: PathBuf,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
) -> Result<(SpecEntry, Vec<String>), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();
    let location_label = display_path_for(&target, &repo_root);
    let start_message = format!("Generating specification for {location_label}...");
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(start_message.clone()));
    }
    logs.push(start_message);

    let date = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown-date".to_string());
    let prompt = build_spec_prompt_text(
        &project_locations,
        &location_label,
        SPEC_GENERATION_MODEL,
        &date,
    );

    let response = call_model(
        &client,
        &provider,
        &auth,
        SPEC_GENERATION_MODEL,
        SPEC_SYSTEM_PROMPT,
        &prompt,
        metrics.clone(),
        0.1,
    )
    .await
    .map_err(|err| SecurityReviewFailure {
        message: format!("Specification generation failed for {location_label}: {err}"),
        logs: Vec::new(),
    })?;
    if let Some(reasoning) = response.reasoning.as_ref() {
        for line in reasoning
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
            let msg = format!("Model reasoning: {truncated}");
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(msg.clone()));
            }
            logs.push(msg);
        }
    }
    let mut sanitized = fix_mermaid_blocks(&response.text);

    if !sanitized.trim().is_empty() {
        let polish_message = format!("Polishing specification markdown for {location_label}.");
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(polish_message.clone()));
        }
        logs.push(polish_message);
        let outcome =
            polish_markdown_block(&client, &provider, &auth, metrics.clone(), &sanitized, None)
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
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(done_message.clone()));
    }
    logs.push(done_message);

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

async fn generate_threat_model(
    client: &Client,
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
        for line in reasoning
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
            let msg = format!("Model reasoning: {truncated}");
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(msg.clone()));
            }
            logs.push(msg);
        }
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
            for line in reasoning
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
                let msg = format!("Model reasoning: {truncated}");
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(msg.clone()));
                }
                logs.push(msg);
            }
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

async fn combine_spec_markdown(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    project_locations: &[String],
    specs: &[SpecEntry],
    combined_path: &Path,
    repo_root: &Path,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
) -> Result<(String, Vec<String>), SecurityReviewFailure> {
    let mut logs: Vec<String> = Vec::new();
    let message = format!(
        "Merging {} specification draft(s) into a single report.",
        specs.len()
    );
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(message.clone()));
    }
    logs.push(message);

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
                return Err(SecurityReviewFailure {
                    message: format!("Failed to combine specifications: {err}"),
                    logs,
                });
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
            let key = request.dedupe_key();
            if !seen_read_requests.insert(key) {
                let msg = format!(
                    "Spec merge READ `{}` skipped (already provided).",
                    request.path.display()
                );
                logs.push(msg.clone());
                conversation.push(format!(
                    "Tool READ `{}` already provided earlier.",
                    request.path.display()
                ));
                executed_command = true;
                continue;
            }

            executed_command = true;
            match execute_auto_scope_read(
                repo_root,
                &request.path,
                request.start,
                request.end,
                metrics.as_ref(),
            )
            .await
            {
                Ok(output) => {
                    logs.push(format!(
                        "Spec merge READ `{}` returned content.",
                        request.path.display()
                    ));
                    conversation.push(format!(
                        "Tool READ `{}`:\n{}",
                        request.path.display(),
                        output
                    ));
                }
                Err(err) => {
                    logs.push(format!(
                        "Spec merge READ `{}` failed: {err}",
                        request.path.display()
                    ));
                    conversation.push(format!(
                        "Tool READ `{}` error: {err}",
                        request.path.display()
                    ));
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
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(message.clone()));
            }
            logs.push(message.clone());
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

fn build_spec_prompt_text(
    project_locations: &[String],
    target_label: &str,
    model_name: &str,
    date: &str,
) -> String {
    let locations_block = if project_locations.is_empty() {
        target_label.to_string()
    } else {
        project_locations.join("\n")
    };

    let template_body = SPEC_MARKDOWN_TEMPLATE
        .replace("{project_locations}", &locations_block)
        .replace("{target_label}", target_label)
        .replace("{model_name}", model_name)
        .replace("{date}", date);

    SPEC_PROMPT_TEMPLATE
        .replace("{project_locations}", &locations_block)
        .replace("{target_label}", target_label)
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
        spec_block.push_str("## Draft\n");
        spec_block.push_str(&format!("Location: {}\n\n", entry.location_label));
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

    let mut lines: Vec<String> = Vec::new();
    lines.push("## Data Classification".to_string());
    lines.push(String::new());
    lines.push("| Data Type | Sensitivity | Storage Location | Retention | Encryption At Rest | In Transit | Accessed By |".to_string());
    lines.push("|---|---|---|---|---|---|---|".to_string());
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
    client: &Client,
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
        for line in reasoning
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
            reasoning_logs.push(format!("Model reasoning: {truncated}"));
        }
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

async fn polish_markdown_block(
    client: &Client,
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
        for line in reasoning
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
            reasoning_logs.push(format!("Model reasoning: {truncated}"));
        }
    }

    Ok(MarkdownPolishOutcome {
        text: output.text,
        reasoning_logs,
    })
}

async fn analyze_files_individually(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    repository_summary: &str,
    spec_markdown: Option<&str>,
    repo_root: &Path,
    snippets: &[FileSnippet],
    git_link_info: Option<GitLinkInfo>,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
) -> Result<BugAnalysisOutcome, SecurityReviewFailure> {
    let mut aggregated_logs: Vec<String> = Vec::new();
    let mut sections: Vec<(usize, String)> = Vec::new();
    let mut snippets_with_findings: Vec<(usize, FileSnippet)> = Vec::new();
    let mut findings_count = 0usize;
    let mut bug_details: Vec<BugDetail> = Vec::new();
    let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
    let mut remaining = snippets.iter().enumerate();
    let total_files = snippets.len();
    let mut completed_files: usize = 0;

    let concurrency = MAX_CONCURRENT_FILE_ANALYSIS.min(snippets.len());
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(format!(
            "  └ Launching parallel bug analysis ({concurrency} workers)"
        )));
        tx.send(AppEvent::SecurityReviewLog(
            "    Sample tool calls (simulated):".to_string(),
        ));
        tx.send(AppEvent::SecurityReviewLog(
            "  └ READ: src/main.rs#L1-L120".to_string(),
        ));
        tx.send(AppEvent::SecurityReviewLog(
            "    SEARCH regex:'(?i)api_key|secret|token'".to_string(),
        ));
    }
    for _ in 0..concurrency {
        if let Some((index, snippet)) = remaining.next() {
            in_flight.push(analyze_single_file(
                client,
                provider,
                auth,
                model,
                repository_summary,
                spec_markdown,
                repo_root,
                snippet.clone(),
                index,
                snippets.len(),
                progress_sender.clone(),
                metrics.clone(),
            ));
        }
    }

    while let Some(result) = in_flight.next().await {
        match result {
            Ok(file_result) => {
                aggregated_logs.extend(file_result.logs);
                findings_count = findings_count.saturating_add(file_result.findings_count);
                if let Some(section) = file_result.bug_section {
                    sections.push((file_result.index, section));
                }
                if let Some(snippet) = file_result.snippet {
                    snippets_with_findings.push((file_result.index, snippet));
                }
                completed_files = completed_files.saturating_add(1);
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
                        repository_summary,
                        spec_markdown,
                        repo_root,
                        snippet.clone(),
                        index,
                        snippets.len(),
                        progress_sender.clone(),
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
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    repository_summary: &str,
    spec_markdown: Option<&str>,
    repo_root: &Path,
    snippet: FileSnippet,
    index: usize,
    total_files: usize,
    progress_sender: Option<AppEventSender>,
    metrics: Arc<ReviewMetrics>,
) -> Result<FileBugResult, SecurityReviewFailure> {
    let mut logs = Vec::new();
    let path_display = snippet.relative_path.display().to_string();
    let search_root_display = repo_root.display().to_string();
    let file_size = human_readable_bytes(snippet.bytes);
    let prefix = format!("{}/{}", index + 1, total_files);
    let mut prompt_adjustment_logs: HashSet<String> = HashSet::new();

    let start_message = format!("Analyzing file {prefix}: {path_display} ({file_size}).");
    if let Some(tx) = progress_sender.as_ref() {
        tx.send(AppEvent::SecurityReviewLog(start_message.clone()));
    }
    logs.push(start_message);

    let base_context = build_single_file_context(&snippet);

    'analysis: for analysis_attempt in 0..MAX_FILE_ANALYSIS_ATTEMPTS {
        if analysis_attempt > 0 {
            let retry_message = format!(
                "Retrying bug analysis for {path_display} (attempt {}/{MAX_FILE_ANALYSIS_ATTEMPTS}).",
                analysis_attempt + 1
            );
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(retry_message.clone()));
            }
            logs.push(retry_message);
        }

        let mut code_context = base_context.clone();
        let mut seen_requests: HashSet<String> = HashSet::new();
        let mut content_header_added = false;
        let mut file_header_added = false;
        let mut command_error_header_added = false;
        let mut command_error_count = 0usize;

        for search_attempt in 0..=MAX_SEARCH_REQUESTS_PER_FILE {
            let bug_prompt =
                build_bugs_user_prompt(repository_summary, spec_markdown, &code_context);
            for line in &bug_prompt.logs {
                if prompt_adjustment_logs.insert(line.clone()) {
                    if let Some(tx) = progress_sender.as_ref() {
                        tx.send(AppEvent::SecurityReviewLog(line.clone()));
                    }
                    logs.push(line.clone());
                }
            }
            let prompt_size = human_readable_bytes(bug_prompt.prompt.len());
            if search_attempt == 0 {
                let prompt_message = format!(
                    "Sending bug analysis request for {path_display} (prompt {prompt_size})."
                );
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(prompt_message.clone()));
                }
                logs.push(prompt_message);
            }

            // Keep per-file details informative; overall % is logged when each file completes.
            let detail_string = format!(
                "{} ({} • prompt {}) • model {} via {}",
                path_display, file_size, prompt_size, model, provider.name
            );

            let response = await_with_heartbeat(
                progress_sender.clone(),
                "waiting for bug analysis response from model",
                Some(detail_string.as_str()),
                call_model(
                    client,
                    provider,
                    auth,
                    model,
                    BUGS_SYSTEM_PROMPT,
                    &bug_prompt.prompt,
                    metrics.clone(),
                    0.2,
                ),
            )
            .await;

            let call_output = match response {
                Ok(output) => output,
                Err(err) => {
                    let attempt_number = analysis_attempt + 1;
                    let message = format!(
                        "Bug analysis failed for {path_display} (attempt {attempt_number}/{MAX_FILE_ANALYSIS_ATTEMPTS}): {err}"
                    );
                    if let Some(tx) = progress_sender.as_ref() {
                        tx.send(AppEvent::SecurityReviewLog(message.clone()));
                    }
                    logs.push(message.clone());
                    tracing::error!(
                        target: "codex_security_review",
                        file = %path_display,
                        attempt = attempt_number,
                        total_attempts = MAX_FILE_ANALYSIS_ATTEMPTS,
                        error = %err,
                        "bug analysis request failed"
                    );
                    if attempt_number < MAX_FILE_ANALYSIS_ATTEMPTS {
                        continue 'analysis;
                    } else {
                        logs.push(format!(
                            "Giving up on {path_display} after {MAX_FILE_ANALYSIS_ATTEMPTS} attempts; skipping file."
                        ));
                        return Ok(FileBugResult {
                            index,
                            logs,
                            bug_section: None,
                            snippet: None,
                            findings_count: 0,
                        });
                    }
                }
            };

            if let Some(reasoning) = call_output.reasoning.as_ref() {
                for line in reasoning
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                {
                    let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
                    let reasoning_message = format!("Model reasoning: {truncated}");
                    if let Some(tx) = progress_sender.as_ref() {
                        tx.send(AppEvent::SecurityReviewLog(reasoning_message.clone()));
                    }
                    logs.push(reasoning_message);
                }
            }

            let text = call_output.text;
            let (cleaned_text, requested_terms) = parse_search_requests(&text);
            let trimmed = cleaned_text.trim();

            if trimmed.eq_ignore_ascii_case("no bugs found") && requested_terms.is_empty() {
                let message = format!("No bugs found in {path_display}.");
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(message.clone()));
                }
                logs.push(message);
                return Ok(FileBugResult {
                    index,
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

            let new_requests: Vec<_> = requested_terms
                .into_iter()
                .filter_map(|request| {
                    let key = request.dedupe_key();
                    if seen_requests.insert(key) {
                        Some(request)
                    } else {
                        None
                    }
                })
                .collect();

            // Stop early if there are no new tool requests from the model,
            // or after the final search attempt.
            if new_requests.is_empty() || search_attempt == MAX_SEARCH_REQUESTS_PER_FILE {
                let message = if file_findings == 0 {
                    format!("Recorded findings for {path_display}.")
                } else {
                    let plural = if file_findings == 1 { "" } else { "s" };
                    format!("Recorded {file_findings} finding{plural} for {path_display}.")
                };
                if let Some(tx) = progress_sender.as_ref() {
                    tx.send(AppEvent::SecurityReviewLog(message.clone()));
                }
                logs.push(message);
                return Ok(FileBugResult {
                    index,
                    logs,
                    bug_section: Some(cleaned_text),
                    snippet: Some(snippet.clone()),
                    findings_count: file_findings,
                });
            }

            for request in new_requests {
                if let Some(reason) = request.reason()
                    && !reason.trim().is_empty()
                {
                    let truncated_reason = truncate_text(reason, MODEL_REASONING_LOG_MAX_GRAPHEMES);
                    let rationale_message = format!(
                        "Tool rationale ({}): {}",
                        request.kind_label(),
                        truncated_reason
                    );
                    if let Some(tx) = progress_sender.as_ref() {
                        tx.send(AppEvent::SecurityReviewLog(rationale_message.clone()));
                    }
                    logs.push(rationale_message);
                }

                match request {
                    ToolRequest::Content { term, mode, .. } => {
                        let display_term = summarize_search_term(&term, 80);
                        let mode_label = mode.as_str();
                        let log_message = format!(
                            "Search `{display_term}` in content ({mode_label}) — path {search_root_display}"
                        );
                        if let Some(tx) = progress_sender.as_ref() {
                            tx.send(AppEvent::SecurityReviewLog(log_message.clone()));
                        }
                        logs.push(log_message);
                        let command_id = metrics.next_command_id();
                        let summary = format!("{mode_label} content search for `{display_term}`");
                        emit_command_status(
                            &progress_sender,
                            command_id,
                            summary.clone(),
                            SecurityReviewCommandState::Running,
                            Vec::new(),
                        );
                        let search_result =
                            run_content_search(repo_root, &term, mode, &metrics).await;
                        let (state, preview) = command_completion_state(&search_result);
                        emit_command_status(&progress_sender, command_id, summary, state, preview);
                        match search_result {
                            SearchResult::Matches(output) => {
                                if !content_header_added {
                                    code_context.push_str("\n# Additional ripgrep results\n");
                                    content_header_added = true;
                                }
                                let heading_term = summarize_search_term(&term, 120);
                                code_context.push_str(&format!(
                                    "## {mode_label} content search for `{heading_term}`\n```\n{output}\n```\n"
                                ));
                            }
                            SearchResult::NoMatches => {
                                let miss = format!(
                                    "No content matches found for `{display_term}` — path {search_root_display}"
                                );
                                if let Some(tx) = progress_sender.as_ref() {
                                    tx.send(AppEvent::SecurityReviewLog(miss.clone()));
                                }
                                logs.push(miss);
                            }
                            SearchResult::Error(err) => {
                                let error_message = format!(
                                    "Ripgrep content search for `{display_term}` failed: {err} — path {search_root_display}"
                                );
                                if let Some(tx) = progress_sender.as_ref() {
                                    tx.send(AppEvent::SecurityReviewLog(error_message.clone()));
                                }
                                logs.push(error_message);
                                if !command_error_header_added {
                                    code_context.push_str("\n# Search command errors\n");
                                    command_error_header_added = true;
                                }
                                let heading_term = summarize_search_term(&term, 120);
                                let mut error_for_context = err;
                                if error_for_context.is_empty() {
                                    error_for_context = "rg returned an error".to_string();
                                }
                                let truncated_error = truncate_text(
                                    &error_for_context,
                                    COMMAND_PREVIEW_MAX_GRAPHEMES,
                                );
                                code_context.push_str(&format!(
                                    "## {mode_label} content search for `{heading_term}` failed\n```\n{truncated_error}\n```\n"
                                ));
                                command_error_count += 1;
                                if command_error_count >= MAX_COMMAND_ERROR_RETRIES {
                                    let abort_message = format!(
                                        "Stopping search commands for {path_display} after {MAX_COMMAND_ERROR_RETRIES} errors."
                                    );
                                    if let Some(tx) = progress_sender.as_ref() {
                                        tx.send(AppEvent::SecurityReviewLog(abort_message.clone()));
                                    }
                                    logs.push(abort_message);
                                    return Ok(FileBugResult {
                                        index,
                                        logs,
                                        bug_section: None,
                                        snippet: None,
                                        findings_count: 0,
                                    });
                                }
                            }
                        }
                    }
                    ToolRequest::GrepFiles { args, .. } => {
                        let display_term = summarize_search_term(&args.pattern, 80);
                        let log_message =
                            format!("grep_files for `{display_term}` — path {search_root_display}");
                        if let Some(tx) = progress_sender.as_ref() {
                            tx.send(AppEvent::SecurityReviewLog(log_message.clone()));
                        }
                        logs.push(log_message);
                        let command_id = metrics.next_command_id();
                        let summary = format!("grep_files for `{display_term}`");
                        emit_command_status(
                            &progress_sender,
                            command_id,
                            summary.clone(),
                            SecurityReviewCommandState::Running,
                            Vec::new(),
                        );
                        let search_result = run_grep_files(repo_root, &args, &metrics).await;
                        let (state, preview) = command_completion_state(&search_result);
                        emit_command_status(&progress_sender, command_id, summary, state, preview);
                        match search_result {
                            SearchResult::Matches(output) => {
                                if !file_header_added {
                                    code_context.push_str("\n# Additional file search results\n");
                                    file_header_added = true;
                                }
                                let heading_term = summarize_search_term(&args.pattern, 120);
                                code_context.push_str(&format!(
                                    "## grep_files for `{heading_term}`\n```\n{output}\n```\n"
                                ));
                            }
                            SearchResult::NoMatches => {
                                let miss = format!(
                                    "No files matched `{display_term}` — path {search_root_display}"
                                );
                                if let Some(tx) = progress_sender.as_ref() {
                                    tx.send(AppEvent::SecurityReviewLog(miss.clone()));
                                }
                                logs.push(miss);
                            }
                            SearchResult::Error(err) => {
                                let error_message = format!(
                                    "grep_files for `{display_term}` failed: {err} — path {search_root_display}"
                                );
                                if let Some(tx) = progress_sender.as_ref() {
                                    tx.send(AppEvent::SecurityReviewLog(error_message.clone()));
                                }
                                logs.push(error_message);
                                if !command_error_header_added {
                                    code_context.push_str("\n# Search command errors\n");
                                    command_error_header_added = true;
                                }
                                let heading_term = summarize_search_term(&args.pattern, 120);
                                let mut error_for_context = err;
                                if error_for_context.is_empty() {
                                    error_for_context = "rg returned an error".to_string();
                                }
                                let truncated_error = truncate_text(
                                    &error_for_context,
                                    COMMAND_PREVIEW_MAX_GRAPHEMES,
                                );
                                code_context.push_str(&format!(
                                    "## grep_files for `{heading_term}` failed\n```\n{truncated_error}\n```\n"
                                ));
                                command_error_count += 1;
                                if command_error_count >= MAX_COMMAND_ERROR_RETRIES {
                                    let abort_message = format!(
                                        "Stopping search commands for {path_display} after {MAX_COMMAND_ERROR_RETRIES} errors."
                                    );
                                    if let Some(tx) = progress_sender.as_ref() {
                                        tx.send(AppEvent::SecurityReviewLog(abort_message.clone()));
                                    }
                                    logs.push(abort_message);
                                    return Ok(FileBugResult {
                                        index,
                                        logs,
                                        bug_section: None,
                                        snippet: None,
                                        findings_count: 0,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    unreachable!("analyze_single_file should return within analysis attempts");
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

fn parse_location_item(item: &str, git_link_info: &GitLinkInfo) -> Vec<(String, Option<String>)> {
    let mut results: Vec<(String, Option<String>)> = Vec::new();
    let main = item.split(" - http").next().unwrap_or(item).trim();
    if main.is_empty() {
        return results;
    }

    let path_re =
        Regex::new(r"(?P<path>[^\s,#:]+\.[A-Za-z0-9_]+)(?:[#:]?L(?P<a>\d+)(?:-L(?P<b>\d+))?)?")
            .unwrap_or_else(|_| Regex::new(r"$^").unwrap());
    let range_tail_re =
        Regex::new(r"L\d+(?:-L\d+)?").unwrap_or_else(|_| Regex::new(r"$^").unwrap());

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

    let fallback_re = Regex::new(r"(?P<path>[^\s,#:]+\.[A-Za-z0-9_]+)#(?P<frag>L\d+(?:-L\d+)?)")
        .unwrap_or_else(|_| Regex::new(r"$^").unwrap());
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
        // Try to derive a GitHub handle from the author-mail if it uses the noreply pattern.
        if let Some(mail) = author_mail.as_ref()
            && let Some(handle) = github_handle_from_email(mail)
        {
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

async fn run_risk_rerank_chunk(
    client: &Client,
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
            let key = request.dedupe_key();
            if !seen_read_requests.insert(key) {
                logs.push(format!(
                    "Risk rerank read `{}` skipped (already provided).",
                    request.path.display()
                ));
                conversation.push(format!(
                    "Tool READ `{}` already provided earlier.",
                    request.path.display()
                ));
                executed_command = true;
                continue;
            }

            executed_command = true;
            match execute_auto_scope_read(
                &repo_root,
                &request.path,
                request.start,
                request.end,
                metrics.as_ref(),
            )
            .await
            {
                Ok(output) => {
                    logs.push(format!(
                        "Risk rerank read `{}` returned content.",
                        request.path.display()
                    ));
                    conversation.push(format!(
                        "Tool READ `{}`:\n{}",
                        request.path.display(),
                        output
                    ));
                }
                Err(err) => {
                    logs.push(format!(
                        "Risk rerank read `{}` failed: {err}",
                        request.path.display()
                    ));
                    conversation.push(format!(
                        "Tool READ `{}` error: {err}",
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

async fn rerank_bugs_by_risk(
    client: &Client,
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
        let client = client.clone();
        let provider = provider.clone();
        let auth_clone = auth.clone();
        let model_owned = model.to_string();
        let metrics_clone = metrics.clone();
        let repo_root = repo_root.to_path_buf();

        async move {
            run_risk_rerank_chunk(
                &client,
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
    let mut count = 0usize;
    for ch in trimmed.chars() {
        if count >= max_chars {
            result.push_str(" …");
            break;
        }
        result.push(ch);
        count += 1;
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
    path: PathBuf,
    start: Option<usize>,
    end: Option<usize>,
}

impl ReadRequest {
    fn dedupe_key(&self) -> String {
        format!(
            "{}:{}-{}",
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
        if let Some(rest) = strip_prefix_case_insensitive(trimmed, "READ:") {
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
            if let Some(range) = range_part.strip_prefix('L') {
                let mut parts = range.split('-');
                if let Some(start_str) = parts.next()
                    && let Ok(value) = start_str.trim().parse::<usize>()
                    && value > 0
                {
                    start = Some(value);
                } else if parts.next().is_some() {
                    cleaned.push(line);
                    continue;
                }
                if let Some(end_str) = parts.next() {
                    let clean_end = end_str.trim().trim_start_matches('L');
                    if let Ok(value) = clean_end.parse::<usize>()
                        && value > 0
                    {
                        end = Some(value);
                    } else {
                        cleaned.push(line);
                        continue;
                    }
                }
            }

            requests.push(ReadRequest {
                path: relative,
                start,
                end,
            });
            continue;
        }

        cleaned.push(line);
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
            let priority_idx = header_cells
                .iter()
                .position(|cell| cell.eq_ignore_ascii_case("priority"));
            if priority_idx.is_none() {
                output.push(line.to_string());
                i += 1;
                continue;
            }
            let priority_idx = priority_idx.unwrap();
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
) -> BugPromptData {
    let repository_section = format!("# Repository context\n{repository_summary}\n");
    let code_and_task = BUGS_USER_CODE_AND_TASK.replace("{code_context}", code_context);
    let base_len = repository_section.len() + code_and_task.len();
    let mut prompt =
        String::with_capacity(base_len + spec_markdown.map(str::len).unwrap_or_default());
    let mut logs = Vec::new();

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
    client: &Client,
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
        let code = plan.script.as_ref().unwrap();
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

    let client = Client::new();
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
        for line in reasoning
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let truncated = truncate_text(line, MODEL_REASONING_LOG_MAX_GRAPHEMES);
            let msg = format!("Model reasoning: {truncated}");
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(msg.clone()));
            }
            logs.push(msg);
        }
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

async fn call_model(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    metrics: Arc<ReviewMetrics>,
    temperature: f32,
) -> Result<ModelCallOutput, String> {
    let max_attempts = provider.request_max_retries();
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

                sleep(default_retry_backoff(attempt + 1)).await;
            }
        }
    }

    unreachable!("call_model attempts should always return");
}

async fn call_model_attempt(
    client: &Client,
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
            let builder = provider
                .create_request_builder(client, auth)
                .await
                .map_err(|e| e.to_string())?;

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

async fn send_chat_request(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f32,
    metrics: Arc<ReviewMetrics>,
) -> Result<ModelCallOutput, String> {
    let builder = provider
        .create_request_builder(client, auth)
        .await
        .map_err(|e| e.to_string())?;

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
            input_tokens,
            cached_input_tokens,
            output_tokens,
            reasoning_output_tokens,
            total_tokens,
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
                                input_tokens,
                                cached_input_tokens,
                                output_tokens,
                                reasoning_output_tokens,
                                total_tokens,
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

fn human_readable_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

const MARKDOWN_FIX_MODEL: &str = GPT_5_CODEX_MEDIUM_MODEL;
