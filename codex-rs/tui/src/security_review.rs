use crate::app_event::AppEvent;
use crate::app_event::SecurityReviewAutoScopeSelection;
use crate::app_event::SecurityReviewCommandState;
use crate::app_event_sender::AppEventSender;
use crate::diff_render::display_path_for;
use crate::mermaid::fix_mermaid_blocks;
use crate::security_report_viewer::build_report_html;
use crate::status_indicator_widget::fmt_elapsed_compact;
use crate::text_formatting::truncate_text;
use codex_core::CodexAuth;
use codex_core::ModelProviderInfo;
use codex_core::WireApi;
use codex_core::git_info::collect_git_info;
use codex_core::git_info::get_git_repo_root;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use pathdiff::diff_paths;
use regex::Regex;
use reqwest::Client;
use reqwest::header::ACCEPT;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use std::cmp::Ordering as CmpOrdering;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
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
use url::Url;

const VALIDATION_SUMMARY_GRAPHEMES: usize = 96;
const VALIDATION_OUTPUT_GRAPHEMES: usize = 480;

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
const COMMAND_PREVIEW_MAX_LINES: usize = 2;
const COMMAND_PREVIEW_MAX_GRAPHEMES: usize = 96;
const AUTO_SCOPE_MODEL: &str = "gpt-5-codex";
const SPEC_GENERATION_MODEL: &str = "gpt-5-codex";
const BUG_RERANK_SYSTEM_PROMPT: &str = "You are a senior application security engineer triaging review findings. Reassess customer-facing risk using the supplied repository context and previously generated specs. Only respond with JSON Lines.";
const BUG_RERANK_CHUNK_SIZE: usize = 4;
const BUG_RERANK_MAX_CONCURRENCY: usize = 32;
const BUG_RERANK_CONTEXT_MAX_CHARS: usize = 2000;
const BUG_RERANK_PROMPT_TEMPLATE: &str = r#"
Repository summary (trimmed):
{repository_summary}

Spec excerpt (trimmed; pull in concrete details or note if unavailable):
{spec_excerpt}

Examples:
- External unauthenticated remote code execution on a production API ⇒ risk_score 95, severity "High", reason "unauth RCE takeover".
- Stored XSS on user dashboards that leaks session tokens ⇒ risk_score 72, severity "High", reason "persistent session theft".
- Missing rate limit on an internal admin tool behind SSO ⇒ risk_score 30, severity "Low", reason "internal-only behind SSO".
- Static analysis false positive in dead code ⇒ risk_score 10, severity "Informational", reason "dead code".

Instructions:
- Output severity **only** from ["High","Medium","Low","Informational"]. Map "critical"/"p0" to "High".
- Produce `risk_score` between 0-100 (higher means greater customer impact) and use the full range for comparability.
- Anchor decisions in the repository summary, spec excerpt, blame metadata, and locations above. If you still lack context, state the follow-up you’d run (e.g., repo_search, git blame) in the reason.
- Down-rank issues when strong mitigations (internal-only, dead code, unused feature) materially reduce customer risk. Upgrade when blast radius or exploitability was understated.
- Respond with one JSON object per finding, **in the same order**, formatted exactly as:
  {{"id": <number>, "risk_score": <0-100>, "severity": "<High|Medium|Low|Informational>", "reason": "<≤12 words>"}}

Findings:
{findings}
"#;
const SPEC_DIR_FILTER_TARGET: usize = 8;
const SPEC_DIR_FILTER_SYSTEM_PROMPT: &str = r#"
You triage directories for a security review specification. Only choose directories that hold core product or security-relevant code.
- Prefer application source directories (services, packages, libs).
- Exclude build artifacts, vendored dependencies, generated code, or documentation-only folders.
- Limit the selection to the most critical directories (ideally 3-8).
Respond with a newline-separated list containing only the directory paths chosen from the provided list. Respond with `ALL` if every directory should be included. Do not add quotes or extra commentary.
"#;
const AUTO_SCOPE_SYSTEM_PROMPT: &str = "You are an application security engineer helping select the minimal set of directories that should be examined for a security review. Only respond with JSON lines that follow the requested schema.";
const AUTO_SCOPE_PROMPT_TEMPLATE: &str = r#"
You are assisting with an application security review. Given the repository locations and a natural-language request, identify the minimal set of directories that should be in scope.

<locations>
{locations}
</locations>

# Request
<intent>{user_query}</intent>

# Selection rules
- Prefer code that serves production traffic, handles external input, or configures deployed infrastructure.
- Return directories (not files). Use the highest level that contains the relevant implementation; avoid returning both a parent and its child.
- Skip tests, docs, vendored dependencies, caches, build artefacts, editor configuration, or directories that do not exist.
- Limit to the most relevant 3–8 directories when possible.

# Output format
Return JSON Lines: each line must be a single JSON object with keys {"path", "include", "reason"}. Omit fences and additional commentary. If unsure, set include=false and explain in reason. Output `ALL` alone on one line to include the entire repository.
"#;
const AUTO_SCOPE_JSON_GUARD: &str =
    "Respond only with JSON Lines as described. Do not include markdown fences, prose, or lists.";
const AUTO_SCOPE_MAX_DIR_DEPTH: usize = 3;
const AUTO_SCOPE_MAX_DIRS: usize = 64;
const AUTO_SCOPE_MAX_LANGUAGES: usize = 4;
const AUTO_SCOPE_MAX_MARKERS: usize = 4;
const AUTO_SCOPE_CHILD_PREVIEW: usize = 4;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum SecurityReviewMode {
    #[default]
    Full,
    Bugs,
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
    pub output_root: PathBuf,
    pub mode: SecurityReviewMode,
    pub include_spec_in_bug_analysis: bool,
    pub triage_model: String,
    pub model: String,
    pub provider: ModelProviderInfo,
    pub auth: Option<CodexAuth>,
    pub progress_sender: Option<AppEventSender>,
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
    pub logs: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct SecurityReviewFailure {
    pub message: String,
    pub logs: Vec<String>,
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
    shell_calls: AtomicUsize,
    command_seq: AtomicU64,
}

impl ReviewMetrics {
    fn record_model_call(&self) {
        self.model_calls.fetch_add(1, Ordering::Relaxed);
    }

    fn record_shell_call(&self) {
        self.shell_calls.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> (usize, usize) {
        (
            self.model_calls.load(Ordering::Relaxed),
            self.shell_calls.load(Ordering::Relaxed),
        )
    }

    fn next_command_id(&self) -> u64 {
        self.command_seq
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1)
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
}

#[derive(Clone)]
struct SpecEntry {
    location_label: String,
    markdown: String,
}

struct SpecGenerationOutcome {
    combined_markdown: String,
    locations: Vec<String>,
    logs: Vec<String>,
}

struct AutoScopeCandidate {
    display_path: String,
    depth: usize,
    languages: Vec<String>,
    markers: Vec<String>,
    child_preview: Vec<String>,
    child_count: usize,
}

impl AutoScopeCandidate {
    fn summary(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();

        if !self.languages.is_empty() {
            let languages_summary = self.languages.join(", ");
            parts.push(format!("languages: {languages_summary}"));
        }

        if !self.markers.is_empty() {
            let marker_summary = self.markers.join(", ");
            parts.push(format!("markers: {marker_summary}"));
        }

        if !self.child_preview.is_empty() {
            let preview = self.child_preview.join(", ");
            if self.child_count > self.child_preview.len() {
                let remaining = self.child_count - self.child_preview.len();
                parts.push(format!("subs: {preview} (+{remaining} more)"));
            } else {
                parts.push(format!("subs: {preview}"));
            }
        } else if self.child_count > 0 {
            let child_count = self.child_count;
            parts.push(format!("subs: {child_count} dirs"));
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("; "))
        }
    }
}

struct AutoScopeSelection {
    abs_path: PathBuf,
    display_path: String,
    reason: Option<String>,
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
}

impl BugVerificationTool {
    fn as_str(self) -> &'static str {
        match self {
            BugVerificationTool::Curl => "curl",
            BugVerificationTool::Python => "python",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BugVerificationRequest {
    pub id: BugIdentifier,
    pub tool: BugVerificationTool,
    pub target: Option<String>,
    pub script_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub(crate) struct BugVerificationBatchRequest {
    pub snapshot_path: PathBuf,
    pub bugs_path: PathBuf,
    pub report_path: Option<PathBuf>,
    pub report_html_path: Option<PathBuf>,
    pub repo_path: PathBuf,
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

fn render_bug_sections(snapshots: &[BugSnapshot]) -> String {
    let mut sections: Vec<String> = Vec::new();
    for snapshot in snapshots {
        let base = snapshot.original_markdown.trim();
        if base.is_empty() {
            continue;
        }
        let mut composed = base.to_string();
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

    if include_paths.is_empty()
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
            Ok(selections) => {
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
                        } = selection;
                        let message = if let Some(reason) = reason.as_ref() {
                            format!("Auto scope included {display_path} — {reason}")
                        } else {
                            format!("Auto scope included {display_path}")
                        };
                        record(message);
                        resolved_paths.push(abs_path);
                        selection_summaries.push((display_path, reason));
                    }

                    if let Some(tx) = request.progress_sender.as_ref() {
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
                            "Waiting for user confirmation of auto-detected scope...".to_string(),
                        );

                        match confirm_rx.await {
                            Ok(true) => {
                                record("Auto scope confirmed by user.".to_string());
                                include_paths = resolved_paths;
                                let display_paths: Vec<String> = selection_summaries
                                    .iter()
                                    .map(|(path, _)| path.clone())
                                    .collect();
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
                &request.model,
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

    let bug_outcome = match analyze_files_individually(
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

    for line in &bug_outcome.logs {
        record(line.clone());
    }
    let BugAnalysisOutcome {
        bug_markdown,
        bug_summary_table,
        findings_count,
        bug_summaries,
        bug_details,
        files_with_findings,
        logs: _bug_logs,
    } = bug_outcome;

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

    record(format!(
        "Writing artifacts to {}...",
        request.output_root.display()
    ));
    let artifacts = match persist_artifacts(
        &request.output_root,
        &request.repo_path,
        &bugs_markdown,
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
            record(format!("  • Bugs markdown: {}", paths.bugs_path.display()));
            record(format!(
                "  • Bug snapshot: {}",
                paths.snapshot_path.display()
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
    let (model_calls, shell_calls) = metrics.snapshot();
    let elapsed_secs = elapsed.as_secs_f32();
    record(format!(
        "Security review duration: {elapsed_secs:.1}s (model calls: {model_calls}, shell searches: {shell_calls})."
    ));
    record("Security review complete.".to_string());

    Ok(SecurityReviewResult {
        findings_summary,
        bug_summary_table,
        bugs: bugs_for_result,
        bugs_path: artifacts.bugs_path,
        report_path: artifacts.report_path,
        report_html_path: artifacts.report_html_path,
        snapshot_path: artifacts.snapshot_path,
        logs,
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
                    let elapsed = start.elapsed().as_secs();
                    let extra = detail
                        .map(|d| format!(" — {d}"))
                        .unwrap_or_default();
                    tx.send(AppEvent::SecurityReviewLog(format!(
                        "Still {stage} (elapsed {elapsed}s){extra}."
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

    let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
    let mut remaining = chunk_requests.into_iter();
    let total_chunks = total.div_ceil(FILE_TRIAGE_CHUNK_SIZE.max(1));
    let concurrency = FILE_TRIAGE_CONCURRENCY.min(total_chunks.max(1));

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

    let text = match response {
        Ok(text) => text,
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

    let combined_path = combined_dir.join("combined_specification.md");
    let (combined_markdown, mut combine_logs) = combine_spec_markdown(
        client,
        provider,
        auth,
        &display_locations,
        &spec_entries,
        &combined_path,
        progress_sender.clone(),
        metrics,
    )
    .await?;
    logs.append(&mut combine_logs);

    Ok(Some(SpecGenerationOutcome {
        combined_markdown,
        locations: display_locations,
        logs,
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

fn collect_auto_scope_candidates(repo_root: &Path) -> Vec<AutoScopeCandidate> {
    let canonical_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::from([(canonical_root.clone(), 0_usize)]);
    let mut seen: HashSet<PathBuf> = HashSet::new();
    seen.insert(canonical_root.clone());

    let mut root_candidate: Option<AutoScopeCandidate> = None;
    let mut candidates: Vec<AutoScopeCandidate> = Vec::new();

    while let Some((dir, depth)) = queue.pop_front() {
        let mut child_dirs: Vec<PathBuf> = Vec::new();
        let mut child_names: Vec<String> = Vec::new();
        let mut markers: BTreeSet<String> = BTreeSet::new();
        let mut languages: BTreeSet<String> = BTreeSet::new();

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry_result in entries {
                let Ok(entry) = entry_result else {
                    continue;
                };
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_symlink() {
                    continue;
                }
                let path = entry.path();
                let name_os = entry.file_name();
                let name = name_os.to_string_lossy().into_owned();
                if file_type.is_dir() {
                    if is_auto_scope_excluded_dir(&name) {
                        continue;
                    }
                    child_dirs.push(path.clone());
                    child_names.push(name);
                } else if file_type.is_file() {
                    if markers.len() < AUTO_SCOPE_MAX_MARKERS && is_auto_scope_marker(&name) {
                        markers.insert(name.clone());
                    }
                    if languages.len() < AUTO_SCOPE_MAX_LANGUAGES
                        && let Some(lang) = determine_language(&path)
                    {
                        languages.insert(lang.to_string());
                    }
                }
            }
        }

        let child_count = child_dirs.len();

        child_names.sort();
        if child_names.len() > AUTO_SCOPE_CHILD_PREVIEW {
            child_names.truncate(AUTO_SCOPE_CHILD_PREVIEW);
        }

        let candidate = AutoScopeCandidate {
            display_path: display_path_for(&dir, &canonical_root),
            depth,
            languages: languages.into_iter().collect(),
            markers: markers.into_iter().collect(),
            child_preview: child_names,
            child_count,
        };

        if depth == 0 {
            root_candidate = Some(candidate);
        } else {
            candidates.push(candidate);
            if candidates.len() >= AUTO_SCOPE_MAX_DIRS {
                break;
            }
        }

        if depth >= AUTO_SCOPE_MAX_DIR_DEPTH {
            continue;
        }

        child_dirs.sort();
        for child in child_dirs {
            let canonical_child = match child.canonicalize() {
                Ok(path) => path,
                Err(_) => continue,
            };
            if !canonical_child.starts_with(&canonical_root) {
                continue;
            }
            if seen.insert(canonical_child.clone()) {
                queue.push_back((canonical_child, depth + 1));
            }
        }
    }

    let mut ordered: Vec<AutoScopeCandidate> = Vec::new();
    if let Some(root) = root_candidate {
        ordered.push(root);
    } else {
        ordered.push(AutoScopeCandidate {
            display_path: display_path_for(repo_root, repo_root),
            depth: 0,
            languages: Vec::new(),
            markers: Vec::new(),
            child_preview: Vec::new(),
            child_count: 0,
        });
    }
    ordered.extend(candidates);
    ordered
}

fn build_auto_scope_prompt(
    repo_root: &Path,
    candidates: &[AutoScopeCandidate],
    user_query: &str,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    if candidates.is_empty() {
        lines.push(display_path_for(repo_root, repo_root));
    } else {
        for candidate in candidates {
            let indent = "  ".repeat(candidate.depth);
            let mut label = format!("{indent}{}", candidate.display_path);
            if let Some(summary) = candidate.summary() {
                label.push_str(" — ");
                label.push_str(&summary);
            }
            lines.push(label);
        }
    }

    let merged_locations = if lines.is_empty() {
        display_path_for(repo_root, repo_root)
    } else {
        lines.join("\n")
    };
    let base = AUTO_SCOPE_PROMPT_TEMPLATE
        .replace("{locations}", &merged_locations)
        .replace("{user_query}", user_query.trim());
    format!("{base}\n{AUTO_SCOPE_JSON_GUARD}")
}

async fn auto_detect_scope(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    repo_root: &Path,
    user_query: &str,
    metrics: Arc<ReviewMetrics>,
) -> Result<Vec<AutoScopeSelection>, SecurityReviewFailure> {
    let candidates = collect_auto_scope_candidates(repo_root);
    let prompt = build_auto_scope_prompt(repo_root, &candidates, user_query);
    let response = call_model(
        client,
        provider,
        auth,
        model,
        AUTO_SCOPE_SYSTEM_PROMPT,
        &prompt,
        metrics,
        0.0,
    )
    .await
    .map_err(|err| SecurityReviewFailure {
        message: format!("Failed to auto-detect scope: {err}"),
        logs: Vec::new(),
    })?;

    let trimmed = response.trim();
    if trimmed.eq_ignore_ascii_case("all") {
        let canonical = repo_root
            .canonicalize()
            .unwrap_or_else(|_| repo_root.to_path_buf());
        return Ok(vec![AutoScopeSelection {
            display_path: display_path_for(&canonical, repo_root),
            abs_path: canonical,
            reason: Some("LLM requested full repository".to_string()),
        }]);
    }

    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut selections: Vec<AutoScopeSelection> = Vec::new();

    for line in response.lines() {
        let s = line.trim();
        if s.is_empty() {
            continue;
        }
        if s.eq_ignore_ascii_case("all") {
            let canonical = repo_root
                .canonicalize()
                .unwrap_or_else(|_| repo_root.to_path_buf());
            if seen.insert(canonical.clone()) {
                selections.push(AutoScopeSelection {
                    display_path: display_path_for(&canonical, repo_root),
                    abs_path: canonical,
                    reason: Some("LLM requested full repository".to_string()),
                });
            }
            continue;
        }
        let obj: serde_json::Value = match serde_json::from_str(s) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let include = obj
            .get("include")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !include {
            continue;
        }
        let path_value = obj
            .get("path")
            .or_else(|| obj.get("dir"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|p| !p.is_empty());
        let Some(raw_path) = path_value else {
            continue;
        };

        let mut candidate = PathBuf::from(raw_path);
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
        if !canonical.is_dir() {
            continue;
        }
        if !seen.insert(canonical.clone()) {
            continue;
        }
        let reason = obj
            .get("reason")
            .and_then(|v| v.as_str())
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        selections.push(AutoScopeSelection {
            display_path: display_path_for(&canonical, repo_root),
            abs_path: canonical,
            reason,
        });
    }

    Ok(selections)
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
    for raw_line in response.lines() {
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
    let sanitized = fix_mermaid_blocks(&response);

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

    Ok((
        SpecEntry {
            location_label,
            markdown: sanitized,
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
    let mut response = call_model(
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
    .map_err(|err| SecurityReviewFailure {
        message: format!("Threat model generation failed: {err}"),
        logs: Vec::new(),
    })?;
    let mut sanitized_response = fix_mermaid_blocks(&response);

    if !threat_table_has_rows(&sanitized_response) {
        let warn = "Threat model is missing table rows; requesting correction.";
        if let Some(tx) = progress_sender.as_ref() {
            tx.send(AppEvent::SecurityReviewLog(warn.to_string()));
        }
        logs.push(warn.to_string());

        let retry_prompt = build_threat_model_retry_prompt(&prompt, &sanitized_response);
        response = call_model(
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
        .map_err(|err| SecurityReviewFailure {
            message: format!("Threat model regeneration failed: {err}"),
            logs: Vec::new(),
        })?;
        sanitized_response = fix_mermaid_blocks(&response);

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

    let prompt = build_combine_specs_prompt(project_locations, specs);
    let response = match call_model(
        client,
        provider,
        auth,
        SPEC_GENERATION_MODEL,
        SPEC_COMBINE_SYSTEM_PROMPT,
        &prompt,
        metrics,
        0.1,
    )
    .await
    {
        Ok(text) => text,
        Err(err) => {
            return Err(SecurityReviewFailure {
                message: format!("Failed to combine specifications: {err}"),
                logs,
            });
        }
    };
    let sanitized = fix_mermaid_blocks(&response);

    if let Err(e) = tokio_fs::write(combined_path, sanitized.as_bytes()).await {
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

    Ok((sanitized, logs))
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
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if ch == '/' || ch == '\\' || ch == '-' || ch == '_' {
            slug.push('_');
        } else if ch.is_whitespace() {
            slug.push('_');
        }
    }
    while slug.contains("__") {
        slug = slug.replace("__", "_");
    }
    let mut trimmed = slug.trim_matches('_').to_string();
    if trimmed.is_empty() {
        trimmed = "spec".to_string();
    }
    trimmed
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

    let concurrency = MAX_CONCURRENT_FILE_ANALYSIS.min(snippets.len());
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
        aggregated_logs.extend(blame_logs);
    }

    if !bug_summaries.is_empty() {
        let risk_logs = rerank_bugs_by_risk(
            client,
            provider,
            auth,
            model,
            &mut bug_summaries,
            repository_summary,
            spec_markdown,
            metrics.clone(),
        )
        .await;
        aggregated_logs.extend(risk_logs);
    }

    for summary in bug_summaries.iter_mut() {
        if let Some(normalized) = normalize_severity_label(&summary.severity) {
            summary.severity = normalized;
        } else {
            summary.severity = summary.severity.trim().to_string();
        }
    }

    let original_summary_count = bug_summaries.len();
    let mut retained_ids: HashSet<usize> = HashSet::new();
    bug_summaries.retain(|summary| {
        let keep = matches!(
            summary.severity.trim().to_ascii_lowercase().as_str(),
            "high" | "medium"
        );
        if keep {
            retained_ids.insert(summary.id);
        }
        keep
    });
    bug_details.retain(|detail| retained_ids.contains(&detail.summary_id));
    if bug_summaries.len() < original_summary_count {
        aggregated_logs.push(format!(
            "Filtered out {} low/informational findings.",
            original_summary_count - bug_summaries.len()
        ));
    }
    if bug_summaries.is_empty() {
        aggregated_logs
            .push("No high or medium severity findings remain after filtering.".to_string());
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
        "No high or medium severity findings.".to_string()
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
        bug_summary_table: make_bug_summary_table(&bug_summaries, git_link_info.as_ref()),
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
            let prompt_message = if search_attempt == 0 {
                format!("Sending bug analysis request for {path_display} (prompt {prompt_size}).")
            } else {
                format!(
                    "Retrying bug analysis for {path_display} after additional searches (prompt {prompt_size})."
                )
            };
            if let Some(tx) = progress_sender.as_ref() {
                tx.send(AppEvent::SecurityReviewLog(prompt_message.clone()));
            }
            logs.push(prompt_message);

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

            let text = match response {
                Ok(text) => text,
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
                match request {
                    SearchRequest::Content { term, mode } => {
                        let display_term = summarize_search_term(&term, 80);
                        let mode_label = mode.as_str();
                        let log_message =
                            format!("Search `{display_term}` in content ({mode_label})");
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
                                let miss = format!("No content matches found for `{display_term}`");
                                if let Some(tx) = progress_sender.as_ref() {
                                    tx.send(AppEvent::SecurityReviewLog(miss.clone()));
                                }
                                logs.push(miss);
                            }
                            SearchResult::Error(err) => {
                                let error_message = format!(
                                    "Ripgrep content search for `{display_term}` failed: {err}"
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
                    SearchRequest::Files { term, mode } => {
                        let display_term = summarize_search_term(&term, 80);
                        let mode_label = mode.as_str();
                        let log_message =
                            format!("Search files for `{display_term}` ({mode_label})");
                        if let Some(tx) = progress_sender.as_ref() {
                            tx.send(AppEvent::SecurityReviewLog(log_message.clone()));
                        }
                        logs.push(log_message);
                        let command_id = metrics.next_command_id();
                        let summary = format!("{mode_label} file search for `{display_term}`");
                        emit_command_status(
                            &progress_sender,
                            command_id,
                            summary.clone(),
                            SecurityReviewCommandState::Running,
                            Vec::new(),
                        );
                        let search_result = run_file_search(repo_root, &term, mode, &metrics).await;
                        let (state, preview) = command_completion_state(&search_result);
                        emit_command_status(&progress_sender, command_id, summary, state, preview);
                        match search_result {
                            SearchResult::Matches(output) => {
                                if !file_header_added {
                                    code_context.push_str("\n# Additional file search results\n");
                                    file_header_added = true;
                                }
                                let heading_term = summarize_search_term(&term, 120);
                                code_context.push_str(&format!(
                                    "## {mode_label} file search for `{heading_term}`\n```\n{output}\n```\n"
                                ));
                            }
                            SearchResult::NoMatches => {
                                let miss = format!("No files matched pattern `{display_term}`");
                                if let Some(tx) = progress_sender.as_ref() {
                                    tx.send(AppEvent::SecurityReviewLog(miss.clone()));
                                }
                                logs.push(miss);
                            }
                            SearchResult::Error(err) => {
                                let error_message = format!(
                                    "Ripgrep file search for `{display_term}` failed: {err}"
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
                                    "## {mode_label} file search for `{heading_term}` failed\n```\n{truncated_error}\n```\n"
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

    metrics.record_shell_call();

    let mut command = Command::new("rg");
    command
        .arg("--max-count")
        .arg("20")
        .arg("--with-filename")
        .arg("--color")
        .arg("never")
        .arg("--line-number");

    if matches!(mode, SearchMode::Literal) {
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
            SearchResult::Matches(text)
        }
        Some(1) => SearchResult::NoMatches,
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.is_empty() {
                SearchResult::Error("rg returned an error".to_string())
            } else {
                SearchResult::Error(format!("rg error: {stderr}"))
            }
        }
    }
}

async fn run_file_search(
    repo_root: &Path,
    pattern: &str,
    mode: SearchMode,
    metrics: &Arc<ReviewMetrics>,
) -> SearchResult {
    if pattern.is_empty() || pattern.len() > MAX_SEARCH_PATTERN_LEN {
        return SearchResult::NoMatches;
    }

    let mut command = Command::new("rg");
    metrics.record_shell_call();
    command
        .arg("--files-with-matches")
        .arg("--no-messages")
        .arg("--sortr=modified");

    if matches!(mode, SearchMode::Literal) {
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
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut lines = Vec::new();
            let mut total = 0usize;
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                total += 1;
                if lines.len() < MAX_FILE_SEARCH_RESULTS {
                    lines.push(format!("- {trimmed}"));
                }
            }
            if lines.is_empty() {
                return SearchResult::NoMatches;
            }
            let mut text = lines.join("\n");
            if text.len() > MAX_SEARCH_OUTPUT_CHARS {
                text.truncate(MAX_SEARCH_OUTPUT_CHARS);
                text.push_str("\n... (truncated)");
            } else if total > MAX_FILE_SEARCH_RESULTS {
                text.push_str("\n... (truncated)");
            }
            SearchResult::Matches(text)
        }
        Some(1) => SearchResult::NoMatches,
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.is_empty() {
                SearchResult::Error("rg returned an error".to_string())
            } else {
                SearchResult::Error(format!("rg error: {stderr}"))
            }
        }
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

fn make_bug_summary_table(
    bugs: &[BugSummary],
    git_link_info: Option<&GitLinkInfo>,
) -> Option<String> {
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
    table.push_str("| # | Title | Severity | Validation | Location | Impact | Recommendation |\n");
    table.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for (display_idx, bug) in ordered.iter().enumerate() {
        let id = display_idx + 1;
        let location = linkify_location(&bug.file, git_link_info);
        let mut location_with_details = if let Some(blame) = bug.blame.as_ref() {
            if location.is_empty() {
                blame.clone()
            } else {
                format!("{location} ({blame})")
            }
        } else {
            location
        };
        if let Some(reason) = bug.risk_reason.as_ref() {
            let trimmed_reason = reason.trim();
            if !trimmed_reason.is_empty() {
                if !location_with_details.is_empty() {
                    location_with_details.push_str(" — ");
                }
                location_with_details.push_str(trimmed_reason);
            }
        }
        let validation = validation_display(&bug.validation);
        table.push_str(&format!(
            "| {id} | {} | {} | {} | {} | {} | {} |\n",
            sanitize_table_field(&bug.title),
            sanitize_table_field(&bug.severity),
            sanitize_table_field(&validation),
            sanitize_table_field(&location_with_details),
            sanitize_table_field(&bug.impact),
            sanitize_table_field(&bug.recommendation),
        ));
    }
    Some(table)
}

fn make_bug_summary_table_from_bugs(
    bugs: &[SecurityReviewBug],
    git_link_info: Option<&GitLinkInfo>,
) -> Option<String> {
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
    table.push_str("| # | Title | Severity | Validation | Location | Impact | Recommendation |\n");
    table.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for (display_idx, bug) in ordered.iter().enumerate() {
        let id = display_idx + 1;
        let location = linkify_location(&bug.file, git_link_info);
        let mut location_with_details = if let Some(blame) = bug.blame.as_ref() {
            if location.is_empty() {
                blame.clone()
            } else {
                format!("{location} ({blame})")
            }
        } else {
            location
        };
        if let Some(reason) = bug.risk_reason.as_ref() {
            let trimmed_reason = reason.trim();
            if !trimmed_reason.is_empty() {
                if !location_with_details.is_empty() {
                    location_with_details.push_str(" — ");
                }
                location_with_details.push_str(trimmed_reason);
            }
        }
        let validation = validation_display(&bug.validation);
        table.push_str(&format!(
            "| {id} | {} | {} | {} | {} | {} | {} |\n",
            sanitize_table_field(&bug.title),
            sanitize_table_field(&bug.severity),
            sanitize_table_field(&validation),
            sanitize_table_field(&location_with_details),
            sanitize_table_field(&bug.impact),
            sanitize_table_field(&bug.recommendation),
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

fn linkify_location(location: &str, git_link_info: Option<&GitLinkInfo>) -> String {
    let trimmed = location.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let Some(info) = git_link_info else {
        return trimmed.to_string();
    };

    let colon_re = Regex::new(r"(?P<p>[^\s#]+\.[A-Za-z0-9_]+):(?P<a>\d+)(?:-(?P<b>\d+))?")
        .unwrap_or_else(|_| Regex::new(r"$^").unwrap());
    let normalized = colon_re
        .replace_all(trimmed, |caps: &regex::Captures<'_>| {
            let path = caps.name("p").map(|m| m.as_str()).unwrap_or_default();
            let start = caps.name("a").map(|m| m.as_str()).unwrap_or_default();
            let fragment = if let Some(end) = caps.name("b").map(|m| m.as_str()) {
                format!("L{start}-L{end}")
            } else {
                format!("L{start}")
            };
            format!("{path}#{fragment}")
        })
        .into_owned();

    let segments: Vec<&str> = normalized
        .split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.is_empty() {
        return trimmed.to_string();
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut outputs: Vec<String> = Vec::new();
    let mut has_link = false;

    for segment in segments {
        if segment.starts_with("http://") || segment.starts_with("https://") {
            let value = segment.to_string();
            if seen.insert(value.clone()) {
                outputs.push(value);
            }
            continue;
        }

        let pairs = parse_location_item(segment, info);
        let filtered = filter_location_pairs(pairs);

        if filtered.is_empty() {
            let value = segment.to_string();
            if seen.insert(value.clone()) {
                outputs.push(value);
            }
            continue;
        }

        for (rel_path, fragment_opt) in filtered {
            let mut fragment = fragment_opt.unwrap_or_default();
            if !fragment.is_empty() && !fragment.starts_with('#') {
                fragment.insert(0, '#');
            }
            let link_text = format!("{rel_path}{fragment}");
            let url = format!("{}{}{}", info.github_prefix, rel_path, fragment);
            let link = format!("[{link_text}]({url})");
            if seen.insert(link.clone()) {
                outputs.push(link);
                has_link = true;
            }
        }
    }

    if has_link {
        outputs.join(", ")
    } else {
        trimmed.to_string()
    }
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

        metrics.record_shell_call();
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
        summary.blame = Some(format!("{short_sha} {author_name} {date} {range_display}"));
        logs.push(format!(
            "Git blame for bug #{id}: {short_sha} {author_name} {date} {range}",
            id = summary.id,
            range = range_display
        ));
    }
    logs
}

#[derive(Debug)]
struct RiskDecision {
    risk_score: f32,
    severity: Option<String>,
    reason: Option<String>,
}

async fn rerank_bugs_by_risk(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    summaries: &mut [BugSummary],
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

        async move {
            call_model(
                &client,
                &provider,
                &auth_clone,
                model_owned.as_str(),
                BUG_RERANK_SYSTEM_PROMPT,
                prompt.as_str(),
                metrics_clone,
                0.0,
            )
            .await
            .map_err(|err| (ids, err))
        }
    }))
    .buffer_unordered(max_concurrency)
    .collect::<Vec<_>>()
    .await;

    let mut logs: Vec<String> = Vec::new();
    let mut decisions: HashMap<usize, RiskDecision> = HashMap::new();

    for result in chunk_results {
        match result {
            Ok(output) => {
                for raw_line in output.lines() {
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
            Err((ids, err)) => {
                let id_list = ids
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                logs.push(format!(
                    "Risk rerank chunk failed for bug id(s) {id_list}: {err}"
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
enum SearchRequest {
    Content { term: String, mode: SearchMode },
    Files { term: String, mode: SearchMode },
}

impl SearchRequest {
    fn dedupe_key(&self) -> String {
        match self {
            SearchRequest::Content { term, mode } => {
                let lower = term.to_ascii_lowercase();
                format!("content:{mode}:{lower}", mode = mode.as_str())
            }
            SearchRequest::Files { term, mode } => {
                let lower = term.to_ascii_lowercase();
                format!("files:{mode}:{lower}", mode = mode.as_str())
            }
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

fn parse_search_requests(response: &str) -> (String, Vec<SearchRequest>) {
    let mut requests = Vec::new();
    let mut cleaned = Vec::new();
    for line in response.lines() {
        let trimmed = line.trim();
        if let Some(rest) = strip_prefix_case_insensitive(trimmed, "SEARCH_FILES:") {
            let (mode, term) = parse_search_term(rest.trim_matches('`'));
            if !term.is_empty() {
                requests.push(SearchRequest::Files {
                    term: term.to_string(),
                    mode,
                });
            }
        } else if let Some(rest) = strip_prefix_case_insensitive(trimmed, "SEARCH:") {
            let (mode, term) = parse_search_term(rest.trim_matches('`'));
            if !term.is_empty() {
                requests.push(SearchRequest::Content {
                    term: term.to_string(),
                    mode,
                });
            }
        } else {
            cleaned.push(line);
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

fn build_bugs_user_prompt(
    repository_summary: &str,
    spec_markdown: Option<&str>,
    code_context: &str,
) -> BugPromptData {
    let repository_section = format!("# Repository context\n{repository_summary}\n");
    let code_and_task = format!(
        "\n# Code excerpts\n{code_context}\n\n# Task\nEvaluate the project for concrete, exploitable security vulnerabilities. Prefer precise, production-relevant issues to theoretical concerns.\n\nFollow these rules:\n- Read the code and provided context to understand intended behavior before judging safety.\n- Trace attacker-controlled inputs through the call graph to the ultimate sink. Highlight any sanitization or missing validation along the way.\n- Ignore unit tests, example scripts, or tooling unless they ship to production in this repo.\n- Only report real vulnerabilities that an attacker can trigger with meaningful impact. If none are found, respond with exactly `no bugs found` (no additional text).\n- Quote code snippets and locations using GitHub-style ranges (e.g. `src/service.rs#L10-L24`). Include git blame details when you have them: `<short-sha> <author> <YYYY-MM-DD> L<start>-L<end>`.\n- Keep all output in markdown and avoid generic disclaimers.\n- If you need more repository context, request it explicitly:\n  - Emit `SEARCH: <pattern>` to run ripgrep across the repository and append matching snippets (patterns are literal by default; prefix with `regex:` to use a regular expression).\n  - Emit `SEARCH_FILES: <pattern>` to list files whose contents match, mirroring the `grep_files` tool's behavior.\n  Only three unique search requests per file will be honored.\n\n# Output format\nFor each vulnerability, emit a markdown block:\n\n### <short title>\n- **File & Lines:** `<relative path>#Lstart-Lend`\n- **Severity:** <high|medium|low|ignore>\n- **Impact:** <concise impact analysis>\n- **Likelihood:** <likelihood analysis>\n- **Description:** Detailed narrative with annotated code references explaining the bug.\n- **Snippet:** Fenced code block (specify language) showing only the relevant lines with inline comments or numbered markers that you reference in the description.\n- **Dataflow:** Describe sources, propagation, sanitization, and sinks using relative paths and `L<start>-L<end>` ranges.\n- **PoC:** Concrete steps or payload to reproduce (or `n/a` if infeasible).\n- **Recommendation:** Actionable remediation guidance.\n- **Verification Type:** JSON array subset of [\"network_api\", \"crash_poc\", \"web_browser\"].\n- TAXONOMY: {{\"vuln_class\": \"...\", \"cwe_ids\": [...], \"owasp_categories\": [...], \"vuln_tag\": \"...\"}}\n\nEnsure severity selections are justified by the described impact and likelihood."
    );
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
        "{SECURITY_REVIEW_FOLLOW_UP_MARKER}\nSecurity review follow-up context:\n- Mode: {mode_label}\n- Scope: {scope_summary}\n- {label}: {report_display}\n\nInstructions:\n- Consider the question first, then skim the report for relevant sections before reading in full.\n- Prefer quoting short report excerpts; consult in-scope code using `rg`, `read_file`, or shell commands when needed.\n- Do not modify files or run destructive commands; you are only answering questions.\n- Keep answers concise and in Markdown.\n\nQuestion:\n{question}\n"
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
    bugs_markdown: &str,
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

    Ok(PersistedArtifacts {
        bugs_path,
        snapshot_path,
        report_path,
        report_html_path,
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
    if let Some(table) = make_bug_summary_table_from_bugs(&bugs, git_link_info) {
        sections.push(table);
    }
    let details = render_bug_sections(&snapshot.bugs);
    if !details.trim().is_empty() {
        sections.push(details);
    }
    let combined = sections.join("\n\n");
    fix_mermaid_blocks(&combined)
}

async fn execute_bug_command(plan: BugCommandPlan, repo_path: PathBuf) -> BugCommandResult {
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
            let Some(script_path) = plan.request.script_path.as_ref() else {
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

    let mut command_results: Vec<BugCommandResult> = Vec::new();
    let mut futures = futures::stream::iter(plans.into_iter().map(|plan| {
        let repo_path = batch.repo_path.clone();
        async move { execute_bug_command(plan, repo_path).await }
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

async fn call_model(
    client: &Client,
    provider: &ModelProviderInfo,
    auth: &Option<CodexAuth>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    metrics: Arc<ReviewMetrics>,
    temperature: f32,
) -> Result<String, String> {
    metrics.record_model_call();
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
                return Err(format!("Model request failed with status {status}: {body}"));
            }

            parse_responses_stream_output(&body)
        }
        WireApi::Chat => {
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
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(format!("Model request failed with status {status}: {body}"));
            }

            let value: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;

            parse_chat_output(value)
        }
    }
}

fn parse_responses_stream_output(body: &str) -> Result<String, String> {
    let mut combined = String::new();
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
                &mut fallback,
                &mut failed_error,
                &mut last_parse_error,
            );
            data_buffer.clear();
        }
    }

    if !data_buffer.is_empty() {
        handle_responses_event(
            &data_buffer,
            &mut combined,
            &mut fallback,
            &mut failed_error,
            &mut last_parse_error,
        );
    }

    if let Some(err) = failed_error {
        return Err(err);
    }

    if !combined.trim().is_empty() {
        return Ok(combined.trim().to_string());
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
    fallback: &mut Option<serde_json::Value>,
    failed_error: &mut Option<String>,
    last_parse_error: &mut Option<String>,
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
                "response.completed" => {
                    if let Some(resp) = event.get("response") {
                        *fallback = Some(resp.clone());
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

fn parse_responses_output(value: serde_json::Value) -> Result<String, String> {
    if let Some(array) = value.get("output").and_then(|v| v.as_array()) {
        let mut combined = String::new();
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
                                _ => {}
                            };
                        }
                    }
                }
                _ => {}
            }
        }
        if !combined.trim().is_empty() {
            return Ok(combined.trim().to_string());
        }
    }

    if let Some(texts) = value.get("output_text").and_then(|v| v.as_array()) {
        let merged = texts
            .iter()
            .filter_map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !merged.trim().is_empty() {
            return Ok(merged.trim().to_string());
        }
    }

    Err("Unable to parse response output".to_string())
}

fn parse_chat_output(value: serde_json::Value) -> Result<String, String> {
    if let Some(choice) = value
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        && let Some(message) = choice.get("message")
        && let Some(content) = message.get("content")
    {
        if let Some(text) = content.as_str() {
            if !text.trim().is_empty() {
                return Ok(text.trim().to_string());
            }
        } else if let Some(array) = content.as_array() {
            let merged = array
                .iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n");
            if !merged.trim().is_empty() {
                return Ok(merged.trim().to_string());
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

const SPEC_SYSTEM_PROMPT: &str = "You are an application security engineer documenting how a project is built. Produce an architecture specification that focuses on components, flows, and controls. Stay within the provided code locations and keep the output in markdown.";

const SPEC_COMBINE_SYSTEM_PROMPT: &str = "You are consolidating multiple specification drafts into a single, cohesive project specification. Merge overlapping content, keep terminology consistent, and follow the supplied template. Preserve important security-relevant details while avoiding repetition.";

const SPEC_PROMPT_TEMPLATE: &str = "You have access to the source code inside the following locations:\n{project_locations}\n\nFocus on {target_label}.\nGenerate a security-focused project specification. Parallelize discovery when enumerating files and avoid spending time on tests, vendored dependencies, or build artefacts. Follow the template exactly and return only markdown.\n\nTemplate:\n{spec_template}\n";

const SPEC_MARKDOWN_TEMPLATE: &str = "# Project Specification\n- Location: {target_label}\n- Prepared by: {model_name}\n- Date: {date}\n- In-scope paths:\n```\n{project_locations}\n```\n\n## Overview\nSummarize the product or service, primary users, and the business problem it solves. Highlight the most security relevant entry points.\n\n## Architecture Summary\nDescribe the high-level system architecture, major services, data stores, and external integrations. Include a concise mermaid flowchart when it improves clarity.\n\n## Components\nList 5-8 major components. For each, note the role, responsibilities, key dependencies, and security-critical behavior.\n\n## Business Flows\nDocument up to 5 important flows (CRUD, external integrations, workflow orchestration). For each flow capture triggers, main steps, data touched, and security notes. Include a short mermaid sequence diagram if helpful.\n\n## Authentication\nExplain how principals authenticate, token lifecycles, libraries used, and how secrets are managed.\n\n## Authorization\nDescribe the authorization model, enforcement points, privileged roles, and escalation paths.\n\n## Data Classification\nIdentify sensitive data types handled by the project and where they are stored or transmitted.\n\n## Infrastructure and Deployment\nSummarize infrastructure-as-code, runtime platforms, and configuration or secret handling that affects security posture.\n";

const SPEC_COMBINE_PROMPT_TEMPLATE: &str = "You previously generated specification drafts for the following code locations:\n{project_locations}\n\nDraft content:\n{spec_drafts}\n\nTask: merge these drafts into one comprehensive specification that describes the entire project. Remove duplication, keep terminology consistent, and ensure the final document reads as a single report. Follow the template exactly and return only markdown.\n\nTemplate:\n{combined_template}\n";

const SPEC_COMBINED_MARKDOWN_TEMPLATE: &str = "# Project Specification\n## Executive Overview\nProvide a concise overview of the system, its primary entry points, and the highest-value assets.\n\n## Architecture\nDescribe the overall architecture, including diagrams (mermaid flowchart) where they add clarity. Call out trust boundaries and external dependencies.\n\n## Components\nSummarize each major component grouped by domain (frontend, API, workers, data stores, external integrations). For each component include responsibilities, key dependencies, and notable security considerations.\n\n## Business Flows\nDocument 3-6 critical flows (CRUD, integrations, orchestrations). Explain inputs, key steps, data touched, and defensive controls. Include concise mermaid sequence diagrams when useful.\n\n## Authentication\nDocument authentication methods, token lifecycles, libraries, and secret storage.\n\n## Authorization\nDescribe the authorization model, enforcement mechanisms, privilege boundaries, and escalation paths.\n\n## Data Classification\nSummarize sensitive data handled by the system and where it resides.\n\n## Infrastructure and Deployment\nHighlight infrastructure-as-code, runtime platforms, and configuration or secret delivery mechanisms that influence security posture.\n";

const THREAT_MODEL_SYSTEM_PROMPT: &str = "You are a senior application security engineer preparing a threat model. Use the provided architecture specification and repository summary to enumerate realistic threats, prioritised by risk.";

const THREAT_MODEL_PROMPT_TEMPLATE: &str = "# Repository Summary\n{repository_summary}\n\n# Architecture Specification\n{combined_spec}\n\n# In-Scope Locations\n{locations}\n\n# Task\nConstruct a concise threat model for the system. Focus on meaningful attacker goals and concrete impacts.\n\n## Output Requirements\n- Start with a short paragraph summarising the most important threat themes and high-risk areas.\n- Follow with a markdown table named `Threat Model` with columns: `Threat ID`, `Threat source`, `Prerequisites`, `Threat action`, `Threat impact`, `Impacted assets`, `Priority`, `Recommended mitigations`.\n- Use integer IDs starting at 1. Priority must be one of high, medium, low.\n- Keep prerequisite and mitigation text succinct (single sentence each).\n- Do not include any other sections or commentary outside the summary paragraph and table.\n";

const FILE_TRIAGE_SYSTEM_PROMPT: &str = "You are an application security engineer triaging source files to decide which ones warrant deep security review.\nFocus on entry points, authentication and authorization, network or process interactions, secrets handling, and other security-sensitive functionality.\nWhen uncertain, err on the side of including a file for further analysis.";

const FILE_TRIAGE_PROMPT_TEMPLATE: &str = "You will receive JSON objects describing candidate files from a repository. For each object, output a single JSON line with the same `id`, a boolean `include`, and a short `reason`.\n- Use include=true for files that likely influence production behaviour, handle user input, touch the network/filesystem, perform authentication/authorization, execute commands, or otherwise impact security.\n- Use include=false for files that are clearly documentation, tests, generated artefacts, or otherwise irrelevant to security review.\n\nReply with one JSON object per line in this exact form:\n{\"id\": <number>, \"include\": true|false, \"reason\": \"...\"}\n\nFiles:\n{files}";

const BUGS_SYSTEM_PROMPT: &str = "You are an application security engineer reviewing a codebase.\nYou read the provided project context and code excerpts to identify concrete, exploitable security vulnerabilities.\nFor each vulnerability you find, produce a thorough, actionable write-up that a security team could ship directly to engineers.\n\nStrict requirements:\n- Only report real vulnerabilities with a plausible attacker-controlled input and a meaningful impact.\n- Quote exact file paths and GitHub-style line fragments, e.g. `src/server/auth.ts#L42-L67`.\n- Provide dataflow analysis (source, propagation, sink) where relevant.\n- Include a severity rating (high, medium, low, ignore) plus impact and likelihood reasoning.\n- Include a taxonomy line exactly as `TAXONOMY: {...}` containing JSON with keys vuln_class, cwe_ids[], owasp_categories[], vuln_tag.\n- If you cannot find a security-relevant issue, respond with exactly `no bugs found`.\n- Do not invent commits or authors if unavailable; leave fields blank instead.\n- Keep the response in markdown.";
