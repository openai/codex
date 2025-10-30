// Poisoned mutex should fail the program
#![allow(clippy::unwrap_used)]

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::Weak;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use async_channel::Receiver;
use async_channel::Sender;
use base64::Engine;
use code_apply_patch::ApplyPatchAction;
use code_apply_patch::MaybeApplyPatchVerified;
use code_browser::BrowserConfig as CodexBrowserConfig;
use code_browser::BrowserManager;
use code_otel::otel_event_manager::OtelEventManager;
use code_otel::otel_event_manager::ToolDecisionSource;
use code_protocol::config_types::ReasoningEffort as ProtoReasoningEffort;
use code_protocol::config_types::ReasoningSummary as ProtoReasoningSummary;
use code_protocol::protocol::AskForApproval as ProtoAskForApproval;
use code_protocol::protocol::ReviewDecision as ProtoReviewDecision;
use code_protocol::protocol::SandboxPolicy as ProtoSandboxPolicy;
use crate::config_types::ReasoningEffort as ReasoningEffortConfig;
use crate::config_types::ReasoningSummary as ReasoningSummaryConfig;
use crate::config_types::ClientTools;
// unused: AuthManager
// unused: ConversationHistoryResponseEvent
use code_protocol::protocol::TurnAbortReason;
use code_protocol::protocol::TurnAbortedEvent;
use futures::prelude::*;
use mcp_types::CallToolResult;
use serde::Serialize;
use serde_json;
use serde_json::json;
use tokio::runtime::{Handle, Runtime};
use tokio::sync::oneshot;
use tokio::task::AbortHandle;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;
use tracing::warn;
use uuid::Uuid;
use crate::AuthManager;
use crate::CodexAuth;
use crate::agent_tool::AgentStatusUpdatePayload;
use crate::git_worktree;
use crate::protocol::ApprovedCommandMatchKind;
use crate::protocol::WebSearchBeginEvent;
use crate::protocol::WebSearchCompleteEvent;
use code_protocol::mcp_protocol::AuthMode;
use crate::account_usage;
use crate::auth_accounts;
use crate::agent_defaults::{default_agent_configs, enabled_agent_model_specs};
use code_protocol::models::WebSearchAction;
use code_protocol::protocol::RolloutItem;
use shlex::split as shlex_split;
use shlex::try_join as shlex_try_join;
use chrono::Utc;

pub mod compact;
use self::compact::build_compacted_history;
use self::compact::collect_user_messages;

/// Initial submission ID for session configuration
pub(crate) const INITIAL_SUBMIT_ID: &str = "";
const HOOK_OUTPUT_LIMIT: usize = 2048;
const PENDING_ONLY_SENTINEL: &str = "__code_pending_only__";
const MIN_SHELL_TIMEOUT_MS: u64 = 30 * 60 * 1000;

#[derive(Clone, Default)]
struct ConfirmGuardRuntime {
    patterns: Vec<ConfirmGuardPatternRuntime>,
}

#[derive(Clone)]
struct ConfirmGuardPatternRuntime {
    regex: regex_lite::Regex,
    message: Option<String>,
    raw: String,
}

impl ConfirmGuardRuntime {
    fn from_config(config: &crate::config_types::ConfirmGuardConfig) -> Self {
        let mut patterns = Vec::new();
        for pattern in &config.patterns {
            match regex_lite::Regex::new(&pattern.regex) {
                Ok(regex) => patterns.push(ConfirmGuardPatternRuntime {
                    regex,
                    message: pattern.message.clone(),
                    raw: pattern.regex.clone(),
                }),
                Err(err) => {
                    tracing::warn!("Skipping confirm guard pattern `{}`: {err}", pattern.regex);
                }
            }
        }
        Self { patterns }
    }

    fn matched_pattern(&self, input: &str) -> Option<&ConfirmGuardPatternRuntime> {
        self.patterns.iter().find(|pat| pat.regex.is_match(input))
    }

    fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

impl ConfirmGuardPatternRuntime {
    fn guidance(&self, original_label: &str, original_value: &str, suggested: &str) -> String {
        let header = self
            .message
            .clone()
            .unwrap_or_else(|| {
                format!(
                    "Blocked command matching confirm guard pattern `{}`. Resend with 'confirm:' if you intend to proceed.",
                    self.raw
                )
            });
        format!("{header}\n\n{original_label}: {original_value}\nresend_exact_argv: {suggested}")
    }
}

fn to_proto_reasoning_effort(effort: ReasoningEffortConfig) -> ProtoReasoningEffort {
    match effort {
        ReasoningEffortConfig::Minimal => ProtoReasoningEffort::Minimal,
        ReasoningEffortConfig::Low => ProtoReasoningEffort::Low,
        ReasoningEffortConfig::Medium => ProtoReasoningEffort::Medium,
        ReasoningEffortConfig::High => ProtoReasoningEffort::High,
        ReasoningEffortConfig::None => ProtoReasoningEffort::Minimal,
    }
}

fn to_proto_reasoning_summary(summary: ReasoningSummaryConfig) -> ProtoReasoningSummary {
    match summary {
        ReasoningSummaryConfig::Auto => ProtoReasoningSummary::Auto,
        ReasoningSummaryConfig::Concise => ProtoReasoningSummary::Concise,
        ReasoningSummaryConfig::Detailed => ProtoReasoningSummary::Detailed,
        ReasoningSummaryConfig::None => ProtoReasoningSummary::None,
    }
}

fn to_proto_approval_policy(policy: AskForApproval) -> ProtoAskForApproval {
    match policy {
        AskForApproval::UnlessTrusted => ProtoAskForApproval::UnlessTrusted,
        AskForApproval::OnFailure => ProtoAskForApproval::OnFailure,
        AskForApproval::OnRequest => ProtoAskForApproval::OnRequest,
        AskForApproval::Never => ProtoAskForApproval::Never,
    }
}

fn to_proto_sandbox_policy(policy: SandboxPolicy) -> ProtoSandboxPolicy {
    match policy {
        SandboxPolicy::DangerFullAccess => ProtoSandboxPolicy::DangerFullAccess,
        SandboxPolicy::ReadOnly => ProtoSandboxPolicy::ReadOnly,
        SandboxPolicy::WorkspaceWrite {
            writable_roots,
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
            allow_git_writes,
        } => ProtoSandboxPolicy::WorkspaceWrite {
            writable_roots,
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
            allow_git_writes,
        },
    }
}

fn to_proto_review_decision(decision: ReviewDecision) -> ProtoReviewDecision {
    match decision {
        ReviewDecision::Approved => ProtoReviewDecision::Approved,
        ReviewDecision::ApprovedForSession => ProtoReviewDecision::ApprovedForSession,
        ReviewDecision::Denied => ProtoReviewDecision::Denied,
        ReviewDecision::Abort => ProtoReviewDecision::Abort,
    }
}

#[allow(dead_code)]
trait MutexExt<T> {
    fn lock_unchecked(&self) -> std::sync::MutexGuard<'_, T>;
}

#[allow(dead_code)]
impl<T> MutexExt<T> for Mutex<T> {
    fn lock_unchecked(&self) -> std::sync::MutexGuard<'_, T> {
        #[expect(clippy::expect_used)]
        self.lock().expect("poisoned lock")
    }
}

#[derive(Clone)]
pub(crate) struct TurnContext {
    pub(crate) client: ModelClient,
    pub(crate) cwd: PathBuf,
    pub(crate) base_instructions: Option<String>,
    pub(crate) user_instructions: Option<String>,
    pub(crate) approval_policy: AskForApproval,
    pub(crate) sandbox_policy: SandboxPolicy,
    pub(crate) shell_environment_policy: ShellEnvironmentPolicy,
    pub(crate) is_review_mode: bool,
    pub(crate) text_format_override: Option<TextFormat>,
}

/// Gather ephemeral, per-turn context that should not be persisted to history.
/// Combines environment info and (when enabled) a live browser snapshot and status.
struct EphemeralJar {
    items: Vec<ResponseItem>,
}

impl EphemeralJar {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn into_items(self) -> Vec<ResponseItem> {
        self.items
    }
}

/// Convert a vector of core `InputItem`s into a single `ResponseInputItem`
/// suitable for sending to the model. Handles images (local and pre‑encoded)
/// and our fork's ephemeral image variant by inlining a brief metadata marker
/// followed by the image as a data URL.
fn response_input_from_core_items(items: Vec<InputItem>) -> ResponseInputItem {
    let mut content_items = Vec::new();

    for item in items {
        match item {
            InputItem::Text { text } => {
                content_items.push(ContentItem::InputText { text });
            }
            InputItem::Image { image_url } => {
                content_items.push(ContentItem::InputImage { image_url });
            }
            InputItem::LocalImage { path } => match std::fs::read(&path) {
                Ok(bytes) => {
                    let mime = mime_guess::from_path(&path)
                        .first()
                        .map(|m| m.essence_str().to_owned())
                        .unwrap_or_else(|| "application/octet-stream".to_string());
                    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                    content_items.push(ContentItem::InputImage {
                        image_url: format!("data:{mime};base64,{encoded}"),
                    });
                }
                Err(err) => {
                    tracing::warn!(
                        "Skipping image {} – could not read file: {}",
                        path.display(),
                        err
                    );
                }
            },
            InputItem::EphemeralImage { path, metadata } => {
                tracing::info!(
                    "Processing ephemeral image: {} with metadata: {:?}",
                    path.display(),
                    metadata
                );

                if let Some(meta) = metadata {
                    content_items.push(ContentItem::InputText {
                        text: format!("[EPHEMERAL:{}]", meta),
                    });
                }

                match std::fs::read(&path) {
                    Ok(bytes) => {
                        let mime = mime_guess::from_path(&path)
                            .first()
                            .map(|m| m.essence_str().to_owned())
                            .unwrap_or_else(|| "application/octet-stream".to_string());
                        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                        tracing::info!("Created ephemeral image data URL with mime: {}", mime);
                        content_items.push(ContentItem::InputImage {
                            image_url: format!("data:{mime};base64,{encoded}"),
                        });
                    }
                    Err(err) => {
                        tracing::error!(
                            "Failed to read ephemeral image {} – {}",
                            path.display(),
                            err
                        );
                    }
                }
            }
        }
    }

    ResponseInputItem::Message {
        role: "user".to_string(),
        content: content_items,
    }
}

fn convert_call_tool_result_to_function_call_output_payload(
    result: &Result<CallToolResult, String>,
) -> FunctionCallOutputPayload {
    match result {
        Ok(ok) => FunctionCallOutputPayload {
            content: serde_json::to_string(ok)
                .unwrap_or_else(|e| format!("JSON serialization error: {e}")),
            success: Some(true),
        },
        Err(e) => FunctionCallOutputPayload {
            content: format!("err: {e:?}"),
            success: Some(false),
        },
    }
}

fn get_git_branch(cwd: &std::path::Path) -> Option<String> {
    let head_path = cwd.join(".git/HEAD");
    if let Ok(contents) = std::fs::read_to_string(&head_path) {
        if let Some(rest) = contents.trim().strip_prefix("ref: ") {
            if let Some(branch) = rest.trim().rsplit('/').next() {
                return Some(branch.to_string());
            }
        }
    }
    None
}

fn maybe_update_from_model_info<T: Copy + PartialEq>(
    field: &mut Option<T>,
    old_default: Option<T>,
    new_default: Option<T>,
) {
    if field.is_none() {
        if let Some(new_val) = new_default {
            *field = Some(new_val);
        }
        return;
    }

    if let (Some(current), Some(old_val)) = (*field, old_default) {
        if current == old_val {
            *field = new_default;
        }
    }
}

async fn build_turn_status_items(sess: &Session) -> Vec<ResponseItem> {
    let mut jar = EphemeralJar::new();

    // Collect environment context
    let cwd = sess.cwd.to_string_lossy().to_string();
    let branch = get_git_branch(&sess.cwd).unwrap_or_else(|| "unknown".to_string());
    let reasoning_effort = sess.client.get_reasoning_effort();

    // Build current system status (UI-only; not persisted)
    let mut current_status = format!(
        r#"== System Status ==
 [automatic message added by system]

 cwd: {cwd}
 branch: {branch}
 reasoning: {reasoning_effort:?}"#
    );

    // Prepare browser context + optional screenshot
    let mut screenshot_content: Option<ContentItem> = None;
    let mut include_screenshot = false;

    if let Some(browser_manager) = code_browser::global::get_browser_manager().await {
        if browser_manager.is_enabled().await {
            // Get current URL and browser info
            let url = browser_manager
                .get_current_url()
                .await
                .unwrap_or_else(|| "unknown".to_string());

            // Try to get a tab title if available
            let title = match browser_manager.get_or_create_page().await {
                Ok(page) => page.get_title().await,
                Err(_) => None,
            };

            // Get browser type description
            let browser_type = browser_manager.get_browser_type().await;

            // Get viewport dimensions
            let (viewport_width, viewport_height) = browser_manager.get_viewport_size().await;
            let viewport_info = format!(" | Viewport: {}x{}", viewport_width, viewport_height);

            // Get cursor position
            let cursor_info = match browser_manager.get_cursor_position().await {
                Ok((x, y)) => format!(
                    " | Mouse position: ({:.0}, {:.0}) [shown as a blue cursor in the screenshot]",
                    x, y
                ),
                Err(_) => String::new(),
            };

            // Try to capture screenshot and compare with last one
            let screenshot_status = match capture_browser_screenshot(sess).await {
                Ok((screenshot_path, _url)) => {
                    // Always update the UI with the latest screenshot, even if unchanged for LLM payload
                    // This ensures the user sees that a fresh capture occurred each turn.
                    add_pending_screenshot(sess, screenshot_path.clone(), url.clone());
                    // Check if screenshot has changed using image hashing
                    let mut last_screenshot_info = sess.last_screenshot_info.lock().unwrap();

                    // Compute hash for current screenshot
                    let current_hash =
                        crate::image_comparison::compute_image_hash(&screenshot_path).ok();

                    let should_include_screenshot = if let (
                        Some((_last_path, last_phash, last_dhash)),
                        Some((cur_phash, cur_dhash)),
                    ) =
                        (last_screenshot_info.as_ref(), current_hash.as_ref())
                    {
                        // Compare hashes to see if screenshots are similar
                        let similar = crate::image_comparison::are_hashes_similar(
                            last_phash, last_dhash, cur_phash, cur_dhash,
                        );

                        if !similar {
                            // Screenshot has changed, include it
                            *last_screenshot_info = Some((
                                screenshot_path.clone(),
                                cur_phash.clone(),
                                cur_dhash.clone(),
                            ));
                            true
                        } else {
                            // Screenshot unchanged
                            false
                        }
                    } else {
                        // No previous screenshot or hash computation failed, include it
                        if let Some((phash, dhash)) = current_hash {
                            *last_screenshot_info = Some((screenshot_path.clone(), phash, dhash));
                        }
                        true
                    };

                    if should_include_screenshot {
                        if let Ok(bytes) = std::fs::read(&screenshot_path) {
                            let mime = mime_guess::from_path(&screenshot_path)
                                .first()
                                .map(|m| m.to_string())
                                .unwrap_or_else(|| "image/png".to_string());
                            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                            screenshot_content = Some(ContentItem::InputImage {
                                image_url: format!("data:{mime};base64,{encoded}"),
                            });
                            include_screenshot = true;
                            ""
                        } else {
                            " [Screenshot file read failed]"
                        }
                    } else {
                        " [Screenshot unchanged]"
                    }
                }
                Err(err_msg) => {
                    // Include error message so LLM knows screenshot failed
                    format!(" [Screenshot unavailable: {}]", err_msg).leak()
                }
            };

            let status_line = if let Some(t) = title {
                format!(
                    "Browser url: {} — {} ({}){}{}{}. You can interact with it using browser_* tools.",
                    url, t, browser_type, viewport_info, cursor_info, screenshot_status
                )
            } else {
                format!(
                    "Browser url: {} ({}){}{}{}. You can interact with it using browser_* tools.",
                    url, browser_type, viewport_info, cursor_info, screenshot_status
                )
            };
            current_status.push_str("\n");
            current_status.push_str(&status_line);
        }
    }

    // Check if system status has changed
    let mut last_status = sess.last_system_status.lock().unwrap();
    let status_changed = last_status.as_ref() != Some(&current_status);

    if status_changed {
        // Update last status
        *last_status = Some(current_status.clone());
    }

    // Only include items if something has changed or is new
    let mut content: Vec<ContentItem> = Vec::new();

    if status_changed {
        content.push(ContentItem::InputText {
            text: current_status,
        });
    }

    if include_screenshot {
        if let Some(image) = screenshot_content {
            content.push(image);
        }
    }

    if !content.is_empty() {
        jar.items.push(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content,
        });
    }

    jar.into_items()
}
use crate::agent_tool::AGENT_MANAGER;
use crate::agent_tool::AgentStatus;
use crate::agent_tool::AgentToolRequest;
use crate::agent_tool::CancelAgentParams;
use crate::agent_tool::CheckAgentStatusParams;
use crate::agent_tool::GetAgentResultParams;
use crate::agent_tool::ListAgentsParams;
use crate::agent_tool::normalize_agent_name;
use crate::agent_tool::RunAgentParams;
use crate::agent_tool::WaitForAgentParams;
use crate::apply_patch::convert_apply_patch_to_protocol;
use crate::apply_patch::get_writable_roots;
use crate::apply_patch::{self, ApplyPatchResult};
use crate::client::ModelClient;
use crate::client_common::{Prompt, ResponseEvent, TextFormat, REVIEW_PROMPT};
use crate::environment_context::EnvironmentContext;
use crate::user_instructions::UserInstructions;
use crate::config::{persist_model_selection, Config};
use crate::config_types::ProjectHookEvent;
use crate::config_types::ShellEnvironmentPolicy;
use crate::conversation_history::ConversationHistory;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::error::SandboxErr;
use crate::error::get_error_message_ui;
use crate::exec::ExecParams;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::StreamOutput;
use crate::exec::EXEC_CAPTURE_MAX_BYTES;
use crate::exec::process_exec_tool_call;
use crate::review_format::format_review_findings_block;
use crate::exec_env::create_env;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::mcp_tool_call::handle_mcp_tool_call;
use crate::model_family::{derive_default_model_family, find_family_for_model};
use code_protocol::models::ContentItem;
use code_protocol::models::FunctionCallOutputPayload;
use code_protocol::models::LocalShellAction;
use code_protocol::models::ReasoningItemContent;
use code_protocol::models::ReasoningItemReasoningSummary;
use code_protocol::models::ResponseInputItem;
use code_protocol::models::ResponseItem;
use code_protocol::models::ShellToolCallParams;
use crate::openai_model_info::get_model_info;
use crate::openai_tools::ToolsConfig;
use crate::openai_tools::get_openai_tools;
use crate::slash_commands::get_enabled_agents;
use crate::dry_run_guard::{analyze_command, DryRunAnalysis, DryRunDisposition, DryRunGuardState};
use crate::parse_command::parse_command;
use crate::plan_tool::handle_update_plan;
use crate::project_doc::get_user_instructions;
use crate::project_features::{ProjectCommand, ProjectHook, ProjectHooks};
use crate::protocol::AgentMessageDeltaEvent;
use crate::protocol::AgentMessageEvent;
use crate::protocol::AgentReasoningDeltaEvent;
use crate::protocol::AgentReasoningEvent;
use crate::protocol::AgentReasoningRawContentDeltaEvent;
use crate::protocol::AgentReasoningRawContentEvent;
use crate::protocol::AgentReasoningSectionBreakEvent;
use crate::protocol::AgentStatusUpdateEvent;
use crate::protocol::ApplyPatchApprovalRequestEvent;
use crate::protocol::AskForApproval;
use crate::protocol::BackgroundEventEvent;
use crate::protocol::BrowserScreenshotUpdateEvent;
use crate::protocol::ErrorEvent;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::ExecApprovalRequestEvent;
use crate::protocol::ExecCommandBeginEvent;
use crate::protocol::ExecCommandEndEvent;
use crate::protocol::FileChange;
use crate::protocol::InputItem;
use crate::protocol::Op;
use crate::protocol::PatchApplyBeginEvent;
use crate::protocol::PatchApplyEndEvent;
use crate::protocol::RateLimitSnapshotEvent;
use crate::protocol::TokenCountEvent;
use crate::protocol::TokenUsageInfo;
use crate::protocol::ReviewDecision;
use crate::protocol::ValidationGroup;
use crate::protocol::ReviewOutputEvent;
use crate::protocol::ReviewRequest;
use crate::protocol::SandboxPolicy;
use crate::protocol::SessionConfiguredEvent;
use crate::protocol::Submission;
use crate::protocol::TaskCompleteEvent;
use std::sync::OnceLock;
use tokio::sync::Notify;
use crate::protocol::TurnDiffEvent;
use crate::rollout::RolloutRecorder;
use crate::safety::SafetyCheck;
use crate::safety::assess_command_safety;
use crate::safety::assess_safety_for_untrusted_command;
use crate::shell;
use crate::turn_diff_tracker::TurnDiffTracker;
use crate::user_notification::UserNotification;
use crate::util::backoff;
use code_protocol::protocol::SessionSource;
use crate::rollout::recorder::SessionStateSnapshot;
use serde_json::Value;
use crate::exec_command::ExecSessionManager;

/// The high-level interface to the Codex system.
/// It operates as a queue pair where you send submissions and receive events.
pub struct Codex {
    next_id: AtomicU64,
    tx_sub: Sender<Submission>,
    rx_event: Receiver<Event>,
}

// Allow internal components (like background exec completions) to trigger a new
// turn without fabricating a visible user message. We enqueue an empty
// UserInput; the model will only see queued developer/system items.
static TX_SUB_GLOBAL: OnceLock<Sender<Submission>> = OnceLock::new();
static ANY_BG_NOTIFY: OnceLock<std::sync::Arc<Notify>> = OnceLock::new();

/// Wrapper returned by [`Codex::spawn`] containing the spawned [`Codex`],
/// the submission id for the initial `ConfigureSession` request and the
/// unique session id.
pub struct CodexSpawnOk {
    pub codex: Codex,
    pub init_id: String,
    pub session_id: Uuid,
}

impl Codex {
    /// Spawn a new [`Codex`] and initialize the session.
    pub async fn spawn(config: Config, auth: Option<CodexAuth>) -> CodexResult<CodexSpawnOk> {
        let auth_manager = auth.map(crate::AuthManager::from_auth_for_testing);
        Self::spawn_with_auth_manager(config, auth_manager).await
    }

    pub async fn spawn_with_auth_manager(
        config: Config,
        auth_manager: Option<Arc<AuthManager>>,
    ) -> CodexResult<CodexSpawnOk> {
        // experimental resume path (undocumented)
        let resume_path = config.experimental_resume.clone();
        info!("resume_path: {resume_path:?}");
        // Use an unbounded submission queue to avoid any possibility of back‑pressure
        // between the TUI submit worker and the core loop during interrupts/cancels.
        let (tx_sub, rx_sub) = async_channel::unbounded();
        let (tx_event, rx_event) = async_channel::unbounded();

        let user_instructions = get_user_instructions(&config).await;

        let configure_session = Op::ConfigureSession {
            provider: config.model_provider.clone(),
            model: config.model.clone(),
            model_reasoning_effort: config.model_reasoning_effort,
            model_reasoning_summary: config.model_reasoning_summary,
            model_text_verbosity: config.model_text_verbosity,
            user_instructions,
            base_instructions: config.base_instructions.clone(),
            approval_policy: config.approval_policy,
            sandbox_policy: config.sandbox_policy.clone(),
            disable_response_storage: config.disable_response_storage,
            notify: config.notify.clone(),
            cwd: config.cwd.clone(),
            resume_path: resume_path.clone(),
        };

        let config = Arc::new(config);

        // Generate a unique ID for the lifetime of this Codex session.
        let session_id = Uuid::new_v4();

        // This task will run until Op::Shutdown is received.
        tokio::spawn(submission_loop(
            session_id,
            config,
            auth_manager,
            rx_sub,
            tx_event,
        ));
        let codex = Codex {
            next_id: AtomicU64::new(0),
            tx_sub,
            rx_event,
        };
        // Make a clone of tx_sub available for internal auto-turn triggers.
        let _ = TX_SUB_GLOBAL.set(codex.tx_sub.clone());
        let _ = ANY_BG_NOTIFY.set(std::sync::Arc::new(Notify::new()));
        let init_id = codex.submit(configure_session).await?;

        Ok(CodexSpawnOk {
            codex,
            init_id,
            session_id,
        })
    }

    /// Submit the `op` wrapped in a `Submission` with a unique ID.
    pub async fn submit(&self, op: Op) -> CodexResult<String> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            .to_string();
        let sub = Submission { id: id.clone(), op };
        self.submit_with_id(sub).await?;
        Ok(id)
    }

    /// Use sparingly: prefer `submit()` so Codex is responsible for generating
    /// unique IDs for each submission.
    pub async fn submit_with_id(&self, sub: Submission) -> CodexResult<()> {
        self.tx_sub
            .send(sub)
            .await
            .map_err(|_| CodexErr::InternalAgentDied)?;
        Ok(())
    }

    pub async fn next_event(&self) -> CodexResult<Event> {
        let event = self
            .rx_event
            .recv()
            .await
            .map_err(|_| CodexErr::InternalAgentDied)?;
        Ok(event)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ApprovedCommandPattern {
    argv: Vec<String>,
    kind: ApprovedCommandMatchKind,
    semantic_prefix: Option<Vec<String>>,
}

impl ApprovedCommandPattern {
    pub(crate) fn new(
        argv: Vec<String>,
        kind: ApprovedCommandMatchKind,
        semantic_prefix: Option<Vec<String>>,
    ) -> Self {
        let semantic_prefix = if matches!(kind, ApprovedCommandMatchKind::Prefix) {
            semantic_prefix.or_else(|| Some(argv.clone()))
        } else {
            None
        };
        Self {
            argv,
            kind,
            semantic_prefix,
        }
    }

    pub(crate) fn matches(&self, command: &[String]) -> bool {
        match self.kind {
            ApprovedCommandMatchKind::Exact => command == self.argv.as_slice(),
            ApprovedCommandMatchKind::Prefix => {
                if command.starts_with(&self.argv) {
                    return true;
                }
                if let (Some(pattern), Some(candidate)) = (
                    self.semantic_prefix.as_ref(),
                    semantic_tokens(command),
                ) {
                    return candidate.starts_with(pattern);
                }
                false
            }
        }
    }

    pub fn argv(&self) -> &[String] { &self.argv }

    pub fn kind(&self) -> ApprovedCommandMatchKind { self.kind }
}

fn semantic_tokens(command: &[String]) -> Option<Vec<String>> {
    if command.is_empty() {
        return None;
    }
    if let Some(tokens) = shell_script_tokens(command) {
        return Some(tokens);
    }
    Some(command.to_vec())
}

fn shell_script_tokens(command: &[String]) -> Option<Vec<String>> {
    if command.len() == 3 && is_shell_wrapper(&command[0], &command[1]) {
        if let Some(tokens) = shlex_split(&command[2]) {
            return Some(tokens);
        }
        return Some(vec![command[2].clone()]);
    }
    None
}

fn is_shell_wrapper(shell: &str, flag: &str) -> bool {
    let file_name = Path::new(shell)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(shell)
        .to_ascii_lowercase();
    matches!(
        file_name.as_str(),
        "bash" | "sh" | "zsh" | "ksh" | "fish" | "dash"
    ) && matches!(flag, "-lc" | "-c")
}

#[derive(Clone)]
struct RunningExecMeta {
    sub_id: String,
    order_meta: crate::protocol::OrderMeta,
    cancel_flag: Arc<AtomicBool>,
    end_emitted: Arc<AtomicBool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaitInterruptReason {
    UserMessage,
    SessionAborted,
}

#[derive(Default)]
struct State {
    approved_commands: HashSet<ApprovedCommandPattern>,
    current_task: Option<AgentTask>,
    pending_approvals: HashMap<String, oneshot::Sender<ReviewDecision>>,
    pending_input: Vec<ResponseInputItem>,
    pending_user_input: Vec<QueuedUserInput>,
    history: ConversationHistory,
    /// Tracks which completed agents (by id) have already been returned to the
    /// model for a given batch when using `agent` with `action="wait"` and
    /// `return_all=false`.
    /// This enables sequential waiting behavior across multiple calls.
    seen_completed_agents_by_batch: HashMap<String, HashSet<String>>,
    /// Scratchpad that buffers streamed items/deltas for the current HTTP attempt
    /// so we can seed retries without losing progress.
    turn_scratchpad: Option<TurnScratchpad>,
    /// Per-submission monotonic event sequence (resets at TaskStarted)
    event_seq_by_sub_id: HashMap<String, u64>,
    /// Per-submission sequence used when synthesizing background OrderMeta.
    background_seq_by_sub_id: HashMap<String, u64>,
    /// 1-based ordinal of the current HTTP request attempt in this session.
    request_ordinal: u64,
    dry_run_guard: DryRunGuardState,
    /// Background execs by call_id
    background_execs: std::collections::HashMap<String, BackgroundExecState>,
    /// Active foreground exec calls keyed by call_id (ExecCommandBegin/End lifecycle)
    running_execs: HashMap<String, RunningExecMeta>,
    next_internal_sub_id: u64,
    token_usage_info: Option<TokenUsageInfo>,
    latest_rate_limits: Option<RateLimitSnapshotEvent>,
    pending_manual_compacts: VecDeque<String>,
    wait_interrupt_epoch: u64,
    wait_interrupt_reason: Option<WaitInterruptReason>,
}

#[derive(Clone)]
pub(crate) struct QueuedUserInput {
    submission_id: String,
    response_item: ResponseInputItem,
    core_items: Vec<InputItem>,
}

/// Buffers partial turn progress produced during a single HTTP streaming attempt.
/// This is not recorded to persistent history. It is only used to seed retries
/// when the SSE stream disconnects mid‑turn.
#[derive(Default, Clone, Debug)]
struct TurnScratchpad {
    /// Output items that reached `response.output_item.done` during this attempt
    items: Vec<ResponseItem>,
    /// Tool outputs we produced locally in reaction to output items
    responses: Vec<ResponseInputItem>,
    /// Last assistant text fragment received via deltas (not yet finalized)
    partial_assistant_text: String,
    /// Last reasoning summary fragment received via deltas (not yet finalized)
    partial_reasoning_summary: String,
}

#[derive(Clone)]
struct AccountUsageContext {
    code_home: PathBuf,
    account_id: String,
    plan: Option<String>,
}

fn account_usage_context(sess: &Session) -> Option<AccountUsageContext> {
    let code_home = sess.client.code_home().to_path_buf();
    let account_id = auth_accounts::get_active_account_id(&code_home).ok().flatten()?;
    let plan = auth_accounts::find_account(&code_home, &account_id)
        .ok()
        .flatten()
        .and_then(|account| {
            account
                .tokens
                .as_ref()
                .and_then(|tokens| tokens.id_token.get_chatgpt_plan_type())
        });
    Some(AccountUsageContext {
        code_home,
        account_id,
        plan,
    })
}

fn spawn_usage_task<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    let _ = tokio::task::spawn_blocking(task);
}

#[derive(Debug)]
struct BackgroundExecState {
    notify: std::sync::Arc<tokio::sync::Notify>,
    result_cell: std::sync::Arc<std::sync::Mutex<Option<ExecToolCallOutput>>>,
    tail_buf: Option<std::sync::Arc<std::sync::Mutex<Vec<u8>>>>,
    cmd_display: String,
    suppress_event: std::sync::Arc<std::sync::atomic::AtomicBool>,
    task_handle: Option<tokio::task::JoinHandle<()>>,
    order_meta_for_end: crate::protocol::OrderMeta,
    sub_id: String,
}

/// Context for an initialized model agent
///
/// A session has at most 1 running agent at a time, and can be interrupted by user input.
pub(crate) struct Session {
    id: Uuid,
    client: ModelClient,
    tx_event: Sender<Event>,

    /// The session's current working directory. All relative paths provided by
    /// the model as well as sandbox policies are resolved against this path
    /// instead of `std::env::current_dir()`.
    cwd: PathBuf,
    base_instructions: Option<String>,
    user_instructions: Option<String>,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    shell_environment_policy: ShellEnvironmentPolicy,
    _writable_roots: Vec<PathBuf>,
    disable_response_storage: bool,
    tools_config: ToolsConfig,

    /// Manager for external MCP servers/tools.
    mcp_connection_manager: McpConnectionManager,
    client_tools: Option<ClientTools>,
    #[allow(dead_code)]
    session_manager: Arc<ExecSessionManager>,

    /// Configuration for available agent models
    agents: Vec<crate::config_types::AgentConfig>,

    /// External notifier command (will be passed as args to exec()). When
    /// `None` this feature is disabled.
    notify: Option<Vec<String>>,

    /// Optional rollout recorder for persisting the conversation transcript so
    /// sessions can be replayed or inspected later.
    rollout: Mutex<Option<RolloutRecorder>>,
    state: Mutex<State>,
    code_linux_sandbox_exe: Option<PathBuf>,
    user_shell: shell::Shell,
    show_raw_agent_reasoning: bool,
    /// Pending browser screenshots to include in the next model request
    #[allow(dead_code)]
    pending_browser_screenshots: Mutex<Vec<PathBuf>>,
    /// Track the last system status to detect changes
    last_system_status: Mutex<Option<String>>,
    /// Track the last screenshot path and hash to detect changes
    last_screenshot_info: Mutex<Option<(PathBuf, Vec<u8>, Vec<u8>)>>, // (path, phash, dhash)
    confirm_guard: ConfirmGuardRuntime,
    project_hooks: ProjectHooks,
    project_commands: Vec<ProjectCommand>,
    hook_guard: AtomicBool,
    github: Arc<RwLock<crate::config_types::GithubConfig>>,
    validation: Arc<RwLock<crate::config_types::ValidationConfig>>,
    self_handle: Weak<Session>,
    active_review: Mutex<Option<ReviewRequest>>,
    next_turn_text_format: Mutex<Option<TextFormat>>,
}

struct HookGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> HookGuard<'a> {
    fn try_acquire(flag: &'a AtomicBool) -> Option<Self> {
        flag
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self { flag })
    }
}

impl Drop for HookGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolCallCtx {
    pub sub_id: String,
    pub call_id: String,
    pub seq_hint: Option<u64>,
    pub output_index: Option<u32>,
}

impl ToolCallCtx {
    pub fn new(sub_id: String, call_id: String, seq_hint: Option<u64>, output_index: Option<u32>) -> Self {
        Self { sub_id, call_id, seq_hint, output_index }
    }

    pub fn order_meta(&self, req_ordinal: u64) -> crate::protocol::OrderMeta {
        crate::protocol::OrderMeta { request_ordinal: req_ordinal, output_index: self.output_index, sequence_number: self.seq_hint }
    }
}

impl Session {
    #[allow(dead_code)]
    pub(crate) fn get_writable_roots(&self) -> &[PathBuf] {
        &self._writable_roots
    }

    pub(crate) fn get_approval_policy(&self) -> AskForApproval {
        self.approval_policy
    }

    fn next_background_sequence(&self, sub_id: &str) -> u64 {
        let mut state = self.state.lock().unwrap();
        let entry = state
            .background_seq_by_sub_id
            .entry(sub_id.to_string())
            .or_insert(0);
        let current = *entry;
        *entry = entry.saturating_add(1);
        current
    }

    pub(crate) fn next_background_order(
        &self,
        sub_id: &str,
        req_ordinal: u64,
        output_index: Option<u32>,
    ) -> crate::protocol::OrderMeta {
        let normalized_req = if req_ordinal == 0 { 1 } else { req_ordinal };
        let sequence = self.next_background_sequence(sub_id);
        let stored_output_index = output_index.unwrap_or(i32::MAX as u32);
        crate::protocol::OrderMeta {
            request_ordinal: normalized_req,
            output_index: Some(stored_output_index),
            sequence_number: Some(sequence),
        }
    }

    pub(crate) fn background_order_for_ctx(
        &self,
        ctx: &ToolCallCtx,
        req_ordinal: u64,
    ) -> crate::protocol::OrderMeta {
        let base_output = ctx.output_index.unwrap_or(i32::MAX as u32);
        self.next_background_order(&ctx.sub_id, req_ordinal, Some(base_output))
    }

    pub(crate) fn get_cwd(&self) -> &Path {
        &self.cwd
    }

    pub(crate) fn get_sandbox_policy(&self) -> &SandboxPolicy {
        &self.sandbox_policy
    }

    pub(crate) fn session_uuid(&self) -> Uuid {
        self.id
    }

    pub(crate) fn get_github_config(&self) -> Arc<RwLock<crate::config_types::GithubConfig>> {
        Arc::clone(&self.github)
    }

    pub(crate) fn validation_config(&self) -> Arc<RwLock<crate::config_types::ValidationConfig>> {
        Arc::clone(&self.validation)
    }

    pub(crate) fn client_tools(&self) -> Option<&ClientTools> {
        self.client_tools.as_ref()
    }

    fn set_active_review(&self, review_request: ReviewRequest) {
        let mut guard = self.active_review.lock().unwrap();
        *guard = Some(review_request);
    }

    fn take_active_review(&self) -> Option<ReviewRequest> {
        self.active_review.lock().unwrap().take()
    }

    pub(crate) fn mcp_connection_manager(&self) -> &McpConnectionManager {
        &self.mcp_connection_manager
    }

    pub(crate) async fn shutdown_mcp_clients(&self) {
        self.mcp_connection_manager.shutdown_all().await;
    }

    pub(crate) fn update_validation_tool(&self, name: &str, enable: bool) {
        if name == "actionlint" {
            if let Ok(mut github) = self.github.write() {
                github.actionlint_on_patch = enable;
            }
            return;
        }

        if let Ok(mut cfg) = self.validation.write() {
            let tools = &mut cfg.tools;
            match name {
                "shellcheck" => tools.shellcheck = Some(enable),
                "markdownlint" => tools.markdownlint = Some(enable),
                "hadolint" => tools.hadolint = Some(enable),
                "yamllint" => tools.yamllint = Some(enable),
                "cargo-check" => tools.cargo_check = Some(enable),
                "shfmt" => tools.shfmt = Some(enable),
                "prettier" => tools.prettier = Some(enable),
                _ => {}
            }
        }
    }

    pub(crate) fn update_validation_group(&self, group: ValidationGroup, enable: bool) {
        if let Ok(mut cfg) = self.validation.write() {
            match group {
                ValidationGroup::Functional => cfg.groups.functional = enable,
                ValidationGroup::Stylistic => cfg.groups.stylistic = enable,
            }
        }
    }

    fn resolve_path(&self, path: Option<String>) -> PathBuf {
        path.as_ref()
            .map(PathBuf::from)
            .map_or_else(|| self.cwd.clone(), |p| self.cwd.join(p))
    }

    pub(crate) async fn maybe_parse_apply_patch_verified(
        &self,
        argv: &[String],
        cwd: &Path,
    ) -> MaybeApplyPatchVerified {
        // Upstream parser no longer needs a filesystem; it is pure and sync.
        let _ = self.client_tools.as_ref();
        code_apply_patch::maybe_parse_apply_patch_verified(argv, cwd)
    }

    // ────────────────────────────
    // Scratchpad helpers
    // ────────────────────────────
    fn begin_attempt_scratchpad(&self) {
        let mut state = self.state.lock().unwrap();
        state.turn_scratchpad = Some(TurnScratchpad::default());
    }

    /// Bump the per-session HTTP request attempt ordinal so `OrderMeta`
    /// reflects the correct provider request index for this attempt.
    fn begin_http_attempt(&self) {
        let mut state = self.state.lock().unwrap();
        state.request_ordinal = state.request_ordinal.saturating_add(1);
    }

    fn scratchpad_push(&self, item: &ResponseItem, response: &Option<ResponseInputItem>, sub_id: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(sp) = &mut state.turn_scratchpad {
            sp.items.push(item.clone());
            if let Some(r) = response {
                let mut truncated = r.clone();
                self.enforce_user_message_limits(sub_id, &mut truncated);
                sp.responses.push(truncated);
            }
        }
    }

    fn scratchpad_add_text_delta(&self, delta: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(sp) = &mut state.turn_scratchpad {
            sp.partial_assistant_text.push_str(delta);
            // Keep memory bounded (ensure UTF-8 char boundary when trimming)
            if sp.partial_assistant_text.len() > 4000 {
                let mut drain_up_to = sp.partial_assistant_text.len() - 4000;
                while !sp.partial_assistant_text.is_char_boundary(drain_up_to) {
                    drain_up_to -= 1;
                }
                sp.partial_assistant_text.drain(..drain_up_to);
            }
        }
    }

    fn scratchpad_add_reasoning_delta(&self, delta: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(sp) = &mut state.turn_scratchpad {
            sp.partial_reasoning_summary.push_str(delta);
            if sp.partial_reasoning_summary.len() > 4000 {
                let mut drain_up_to = sp.partial_reasoning_summary.len() - 4000;
                while !sp.partial_reasoning_summary.is_char_boundary(drain_up_to) {
                    drain_up_to -= 1;
                }
                sp.partial_reasoning_summary.drain(..drain_up_to);
            }
        }
    }

    fn scratchpad_clear_partial_message(&self) {
        let mut state = self.state.lock().unwrap();
        if let Some(sp) = &mut state.turn_scratchpad {
            sp.partial_assistant_text.clear();
        }
    }

    fn take_scratchpad(&self) -> Option<TurnScratchpad> {
        let mut state = self.state.lock().unwrap();
        state.turn_scratchpad.take()
    }

    fn clear_scratchpad(&self) {
        let mut state = self.state.lock().unwrap();
        state.turn_scratchpad = None;
    }
}

impl Session {
    pub fn set_task(&self, agent: AgentTask) {
        let mut state = self.state.lock().unwrap();
        if let Some(current_task) = state.current_task.take() {
            current_task.abort(TurnAbortReason::Replaced);
        }
        state.current_task = Some(agent);
    }

    pub fn remove_task(&self, sub_id: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(agent) = &state.current_task {
            if agent.sub_id == sub_id {
                state.current_task.take();
            }
        }
    }

    pub fn has_running_task(&self) -> bool {
        self.state.lock().unwrap().current_task.is_some()
    }

    pub fn queue_user_input(&self, queued: QueuedUserInput) {
        let mut state = self.state.lock().unwrap();
        state.pending_user_input.push(queued);
    }

    fn notify_wait_interrupted(&self, reason: WaitInterruptReason) {
        let mut state = self.state.lock().unwrap();
        state.wait_interrupt_epoch = state.wait_interrupt_epoch.saturating_add(1);
        state.wait_interrupt_reason = Some(reason);
    }

    fn spawn_kill_exec_sessions(manager: Arc<ExecSessionManager>) {
        if let Ok(handle) = Handle::try_current() {
            handle.spawn(async move {
                manager.kill_all().await;
            });
        } else {
            thread::spawn(move || {
                if let Ok(runtime) = Runtime::new() {
                    runtime.block_on(async { manager.kill_all().await });
                }
            });
        }
    }

    fn wait_interrupt_snapshot(&self) -> (u64, Option<WaitInterruptReason>) {
        let state = self.state.lock().unwrap();
        (state.wait_interrupt_epoch, state.wait_interrupt_reason)
    }

    fn enforce_user_message_limits(
        &self,
        sub_id: &str,
        response_item: &mut ResponseInputItem,
    ) {
        let ResponseInputItem::Message { role, content } = response_item else {
            return;
        };
        if role != "user" {
            return;
        }

        let mut aggregated = String::new();
        let mut text_segments: Vec<(usize, usize)> = Vec::new();
        for item in content.iter() {
            if let ContentItem::InputText { text } = item {
                let start = aggregated.len();
                aggregated.push_str(text);
                let end = aggregated.len();
                text_segments.push((start, end));
            }
        }

        if text_segments.is_empty() {
            return;
        }

        let (_, was_truncated, prefix_end, suffix_start) =
            truncate_middle_bytes(&aggregated, MAX_TOOL_OUTPUT_BYTES_FOR_MODEL);
        if !was_truncated {
            return;
        }

        let cwd = self.get_cwd().to_path_buf();
        let filename = format!("user-message-{}-{}.txt", sub_id, Uuid::new_v4());
        let file_note = match ensure_user_dir(&cwd)
            .and_then(|dir| write_agent_file(&dir, &filename, &aggregated))
        {
            Ok(path) => format!("\n\n[Full output saved to: {}]", path.display()),
            Err(e) => format!("\n\n[Full output was too large and truncation applied; failed to save file: {e}]")
        };

        let original = std::mem::take(content);
        let mut new_content = Vec::with_capacity(original.len());
        let mut segment_iter = text_segments.into_iter();
        let mut marker_inserted = false;
        let mut last_text_idx: Option<usize> = None;

        for item in original.into_iter() {
            match item {
                ContentItem::InputText { text } => {
                    if let Some((seg_start, seg_end)) = segment_iter.next() {
                        let mut new_text = String::new();

                        if seg_start < prefix_end {
                            let slice_end = seg_end.min(prefix_end) - seg_start;
                            if let Some(prefix_slice) = text.get(..slice_end) {
                                new_text.push_str(prefix_slice);
                            }
                        }

                        if !marker_inserted && seg_end > prefix_end && seg_start < suffix_start {
                            new_text.push_str(TRUNCATION_MARKER);
                            marker_inserted = true;
                        }

                        if seg_end > suffix_start {
                            let slice_start = seg_start.max(suffix_start) - seg_start;
                            if let Some(suffix_slice) = text.get(slice_start..) {
                                new_text.push_str(suffix_slice);
                            }
                        }

                        new_content.push(ContentItem::InputText { text: new_text });
                        last_text_idx = Some(new_content.len() - 1);
                    }
                }
                other => new_content.push(other),
            }
        }

        if !marker_inserted {
            if let Some(idx) = last_text_idx {
                if let ContentItem::InputText { text } = &mut new_content[idx] {
                    text.push_str(TRUNCATION_MARKER);
                }
            } else {
                new_content.push(ContentItem::InputText {
                    text: TRUNCATION_MARKER.to_string(),
                });
                last_text_idx = Some(new_content.len() - 1);
            }
        }

        if let Some(idx) = last_text_idx {
            if let ContentItem::InputText { text } = &mut new_content[idx] {
                text.push_str(&file_note);
            }
        } else {
            new_content.push(ContentItem::InputText { text: file_note });
        }

        *content = new_content;
    }

    pub fn pop_next_queued_user_input(&self) -> Option<QueuedUserInput> {
        let mut state = self.state.lock().unwrap();
        if state.pending_user_input.is_empty() {
            None
        } else {
            Some(state.pending_user_input.remove(0))
        }
    }

    /// Enqueue a response item that should be surfaced to the model at the start of the
    /// next turn. Returns `true` if no agent is currently running and a new turn should be
    /// scheduled immediately.
    pub fn enqueue_out_of_turn_item(&self, item: ResponseInputItem) -> bool {
        let mut state = self.state.lock().unwrap();
        let should_start_turn = state.current_task.is_none();
        state.pending_input.push(item);
        should_start_turn
    }

    pub(crate) fn next_internal_sub_id(&self) -> String {
        let mut state = self.state.lock().unwrap();
        let id = state.next_internal_sub_id;
        state.next_internal_sub_id = state.next_internal_sub_id.saturating_add(1);
        format!("auto-compact-{id}")
    }

    /// Sends the given event to the client and swallows the send error, if
    /// any, logging it as an error.
    pub(crate) async fn send_event(&self, event: Event) {
        if let Err(e) = self.tx_event.send(event).await {
            error!("failed to send tool call event: {e}");
        }
    }

    /// Persist an event into the rollout log if appropriate.
    fn persist_event(&self, event: &Event) {
        if !crate::rollout::policy::should_persist_event_msg(&event.msg) {
            return;
        }
        let Some(msg) = crate::protocol::event_msg_to_protocol(&event.msg) else {
            return;
        };
        let recorder = {
            let guard = self.rollout.lock().unwrap();
            guard.as_ref().cloned()
        };
        if let Some(rec) = recorder {
            let order = event
                .order
                .as_ref()
                .map(crate::protocol::order_meta_to_protocol);
            let protocol_event = code_protocol::protocol::RecordedEvent {
                id: event.id.clone(),
                event_seq: event.event_seq,
                order,
                msg,
            };
            tokio::spawn(async move {
                if let Err(e) = rec.record_events(&[protocol_event]).await {
                    warn!("failed to persist rollout event: {e}");
                }
            });
        }
    }

    /// Create a stamped Event with a per-turn sequence number.
    fn stamp_event(&self, sub_id: &str, msg: EventMsg) -> Event {
        let mut state = self.state.lock().unwrap();
        let seq = match msg {
            EventMsg::TaskStarted => {
                // Reset per-sub_id sequence at the start of a turn.
                // We increment request_ordinal per HTTP attempt instead
                // (see `begin_http_attempt`).
                let e = state
                    .event_seq_by_sub_id
                    .entry(sub_id.to_string())
                    .or_insert(0);
                *e = 0;
                0
            }
            _ => {
                let e = state
                    .event_seq_by_sub_id
                    .entry(sub_id.to_string())
                    .or_insert(0);
                *e = e.saturating_add(1);
                *e
            }
        };
        Event {
            id: sub_id.to_string(),
            event_seq: seq,
            msg,
            order: None,
        }
    }

    pub(crate) fn make_event(&self, sub_id: &str, msg: EventMsg) -> Event {
        let event = self.stamp_event(sub_id, msg);
        self.persist_event(&event);
        event
    }

    /// Same as make_event but allows supplying a provider sequence_number
    /// (e.g., Responses API SSE event). We DO NOT overwrite `event_seq`
    /// with this hint because `event_seq` must remain monotonic per turn
    /// and local to our runtime. Provider ordering is carried via
    /// `OrderMeta` when applicable.
    fn make_event_with_hint(&self, sub_id: &str, msg: EventMsg, _seq_hint: Option<u64>) -> Event {
        let event = self.stamp_event(sub_id, msg);
        self.persist_event(&event);
        event
    }

    fn make_event_with_order(
        &self,
        sub_id: &str,
        msg: EventMsg,
        order: crate::protocol::OrderMeta,
        _seq_hint: Option<u64>,
    ) -> Event {
        let mut ev = self.stamp_event(sub_id, msg);
        ev.order = Some(order);
        self.persist_event(&ev);
        ev
    }

    // Kept private helpers focused on ctx-based flow to avoid misuse.

    pub(crate) async fn send_ordered_from_ctx(&self, ctx: &ToolCallCtx, msg: EventMsg) {
        let order = ctx.order_meta(self.current_request_ordinal());
        let ev = self.make_event_with_order(&ctx.sub_id, msg, order, ctx.seq_hint);
        let _ = self.tx_event.send(ev).await;
    }

    fn current_request_ordinal(&self) -> u64 {
        let state = self.state.lock().unwrap();
        state.request_ordinal
    }

    fn make_turn_context(&self) -> Arc<TurnContext> {
        Arc::new(TurnContext {
            client: self.client.clone(),
            cwd: self.cwd.clone(),
            base_instructions: self.base_instructions.clone(),
            user_instructions: self.user_instructions.clone(),
            approval_policy: self.approval_policy,
            sandbox_policy: self.sandbox_policy.clone(),
            shell_environment_policy: self.shell_environment_policy.clone(),
            is_review_mode: false,
            text_format_override: self.next_turn_text_format.lock().unwrap().take(),
        })
    }

    pub async fn request_command_approval(
        &self,
        sub_id: String,
        call_id: String,
        command: Vec<String>,
        cwd: PathBuf,
        reason: Option<String>,
    ) -> oneshot::Receiver<ReviewDecision> {
        let (tx_approve, rx_approve) = oneshot::channel();
        let event = self.make_event(
            &sub_id,
            EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                call_id: call_id.clone(),
                command,
                cwd,
                reason,
            }),
        );
        let _ = self.tx_event.send(event).await;
        {
            let mut state = self.state.lock().unwrap();
            // Track pending approval by call_id (unique per request) rather than sub_id
            // so parallel approvals in the same turn do not clobber each other.
            state.pending_approvals.insert(call_id, tx_approve);
        }
        rx_approve
    }

    pub async fn request_patch_approval(
        &self,
        sub_id: String,
        call_id: String,
        action: &ApplyPatchAction,
        reason: Option<String>,
        grant_root: Option<PathBuf>,
    ) -> oneshot::Receiver<ReviewDecision> {
        let (tx_approve, rx_approve) = oneshot::channel();
        let event = self.make_event(
            &sub_id,
            EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                call_id: call_id.clone(),
                changes: convert_apply_patch_to_protocol(action),
                reason,
                grant_root,
            }),
        );
        let _ = self.tx_event.send(event).await;
        {
            let mut state = self.state.lock().unwrap();
            // Track pending approval by call_id to avoid collisions.
            state.pending_approvals.insert(call_id, tx_approve);
        }
        rx_approve
    }

    pub fn notify_approval(&self, call_id: &str, decision: ReviewDecision) {
        let mut state = self.state.lock().unwrap();
        if let Some(tx_approve) = state.pending_approvals.remove(call_id) {
            let _ = tx_approve.send(decision);
        } else {
            // If we cannot find a pending approval for this call id, surface a warning
            // to aid debugging of stuck approvals.
            tracing::warn!("no pending approval found for call_id={}", call_id);
        }
    }

    pub fn add_approved_command(&self, pattern: ApprovedCommandPattern) {
        let mut state = self.state.lock().unwrap();
        state.approved_commands.insert(pattern);
    }

    /// Records items to both the rollout and the chat completions/ZDR
    /// transcript, if enabled.
    async fn record_conversation_items(&self, items: &[ResponseItem]) {
        debug!("Recording items for conversation: {items:?}");
        self.record_state_snapshot(items).await;

        self.state.lock().unwrap().history.record_items(items);

    }

    /// Clean up old screenshots and system status messages from conversation history
    /// This is called when a new user message arrives to keep history manageable
    async fn cleanup_old_status_items(&self) {
        let mut state = self.state.lock().unwrap();

        // Get current history items
        let current_items = state.history.contents();

        // Track various message types and their positions
        let mut real_user_messages = Vec::new(); // Non-status user messages
        let mut status_messages = Vec::new(); // Messages with screenshots or status

        for (idx, item) in current_items.iter().enumerate() {
            match item {
                ResponseItem::Message { role, content, .. } if role == "user" => {
                    // Check message content
                    let has_status = content.iter().any(|c| {
                        if let ContentItem::InputText { text } = c {
                            text.contains("== System Status ==")
                                || text.contains("Current working directory:")
                                || text.contains("Git branch:")
                        } else {
                            false
                        }
                    });

                    let has_screenshot = content
                        .iter()
                        .any(|c| matches!(c, ContentItem::InputImage { .. }));

                    let has_real_text = content.iter().any(|c| {
                        if let ContentItem::InputText { text } = c {
                            // Real user text doesn't contain system status markers
                            !text.contains("== System Status ==")
                                && !text.contains("Current working directory:")
                                && !text.contains("Git branch:")
                                && !text.trim().is_empty()
                        } else {
                            false
                        }
                    });

                    if has_real_text && !has_status && !has_screenshot {
                        // This is a real user message
                        real_user_messages.push(idx);
                    } else if has_status || has_screenshot {
                        // This is a status/screenshot message
                        status_messages.push(idx);
                    }
                }
                _ => {}
            }
        }

        // Find screenshots to keep: last 2 that directly follow real user commands
        let mut screenshots_to_keep = std::collections::HashSet::new();

        // Work backwards through real user messages
        for &user_idx in real_user_messages.iter().rev().take(2) {
            // Find the first status message after this user message
            for &status_idx in status_messages.iter() {
                if status_idx > user_idx {
                    // Check if this status message contains a screenshot
                    if let Some(ResponseItem::Message { content, .. }) =
                        current_items.get(status_idx)
                    {
                        let has_screenshot = content
                            .iter()
                            .any(|c| matches!(c, ContentItem::InputImage { .. }));
                        if has_screenshot {
                            screenshots_to_keep.insert(status_idx);
                            break; // Only keep one screenshot per user message
                        }
                    }
                }
            }
        }

        // Build the filtered history
        let mut items_to_keep = Vec::new();
        let mut removed_screenshots = 0;
        let mut removed_status = 0;

        for (idx, item) in current_items.iter().enumerate() {
            let should_keep = if status_messages.contains(&idx) {
                // This is a status/screenshot message
                if screenshots_to_keep.contains(&idx) {
                    true // Keep this screenshot
                } else {
                    // Count what we're removing
                    if let ResponseItem::Message { content, .. } = item {
                        let has_screenshot = content
                            .iter()
                            .any(|c| matches!(c, ContentItem::InputImage { .. }));
                        if has_screenshot {
                            removed_screenshots += 1;
                        } else {
                            removed_status += 1;
                        }
                    }
                    false // Remove this status/screenshot
                }
            } else {
                true // Keep all non-status messages (real user messages, assistant messages, etc.)
            };

            if should_keep {
                items_to_keep.push(item.clone());
            }
        }

        // Replace the history with cleaned items
        state.history = ConversationHistory::new();
        state.history.record_items(&items_to_keep);

        if removed_screenshots > 0 || removed_status > 0 {
            info!(
                "Cleaned up history: removed {} old screenshots and {} status messages, kept {} recent screenshots",
                removed_screenshots,
                removed_status,
                screenshots_to_keep.len()
            );
        }
    }

    async fn record_state_snapshot(&self, items: &[ResponseItem]) {
        let snapshot = { SessionStateSnapshot {} };

        let recorder = self.clone_rollout_recorder();

        if let Some(rec) = recorder {
            if let Err(e) = rec.record_state(snapshot).await {
                error!("failed to record rollout state: {e:#}");
            }
            if let Err(e) = rec.record_response_items(items).await {
                error!("failed to record rollout items: {e:#}");
            }
        }
    }

    fn clone_rollout_recorder(&self) -> Option<RolloutRecorder> {
        let guard = self.rollout.lock().unwrap();
        guard.as_ref().cloned()
    }

    pub(crate) async fn persist_rollout_items(&self, items: &[RolloutItem]) {
        let recorder = {
            let guard = self.rollout.lock().unwrap();
            guard.as_ref().cloned()
        };
        if let Some(rec) = recorder {
            if let Err(e) = rec.record_items(items).await {
                error!("failed to record rollout items: {e:#}");
            }
        }
    }

    async fn on_exec_command_begin(
        &self,
        turn_diff_tracker: &mut TurnDiffTracker,
        exec_command_context: ExecCommandContext,
        seq_hint: Option<u64>,
        output_index: Option<u32>,
        attempt_req: u64,
    ) {
        let ExecCommandContext {
            sub_id,
            call_id,
            command_for_display,
            cwd,
            apply_patch,
        } = exec_command_context;
        let msg = match apply_patch {
            Some(ApplyPatchCommandContext {
                user_explicitly_approved_this_action,
                changes,
            }) => {
                turn_diff_tracker.on_patch_begin(&changes);

                EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                    call_id,
                    auto_approved: !user_explicitly_approved_this_action,
                    changes,
                })
            }
            None => EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id,
                command: command_for_display.clone(),
                cwd,
                parsed_cmd: parse_command(&command_for_display),
            }),
        };
        let order = crate::protocol::OrderMeta { request_ordinal: attempt_req, output_index, sequence_number: seq_hint };
        let event = self.make_event_with_order(&sub_id, msg, order, seq_hint);
        let _ = self.tx_event.send(event).await;
    }

    async fn on_exec_command_end(
        &self,
        turn_diff_tracker: &mut TurnDiffTracker,
        sub_id: &str,
        call_id: &str,
        output: &ExecToolCallOutput,
        is_apply_patch: bool,
        seq_hint: Option<u64>,
        output_index: Option<u32>,
        attempt_req: u64,
    ) {
        let ExecToolCallOutput {
            stdout,
            stderr,
            aggregated_output: _,
            duration,
            exit_code,
            timed_out: _,
        } = output;
        // Because stdout and stderr could each be up to 100 KiB, we send
        // truncated versions.
        const MAX_STREAM_OUTPUT: usize = 5 * 1024; // 5KiB
        let stdout = stdout.text.chars().take(MAX_STREAM_OUTPUT).collect();
        let stderr = stderr.text.chars().take(MAX_STREAM_OUTPUT).collect();
        // Precompute formatted output if needed in future for logging/pretty UI.

        let msg = if is_apply_patch {
            EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                call_id: call_id.to_string(),
                stdout,
                stderr,
                success: *exit_code == 0,
            })
        } else {
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: call_id.to_string(),
                stdout,
                stderr,
                exit_code: *exit_code,
                duration: *duration,
            })
        };
        let order = crate::protocol::OrderMeta { request_ordinal: attempt_req, output_index, sequence_number: seq_hint };
        let event = self.make_event_with_order(sub_id, msg, order, seq_hint);
        let _ = self.tx_event.send(event).await;

        // If this is an apply_patch, after we emit the end patch, emit a second event
        // with the full turn diff if there is one.
        if is_apply_patch {
            let unified_diff = turn_diff_tracker.get_unified_diff();
            if let Ok(Some(unified_diff)) = unified_diff {
                let msg = EventMsg::TurnDiff(TurnDiffEvent { unified_diff });
                let event = self.make_event(sub_id, msg);
                let _ = self.tx_event.send(event).await;
            }
        }

    }
    /// Runs the exec tool call and emits events for the begin and end of the
    /// command even on error.
    ///
    /// Returns the output of the exec tool call.
    async fn run_exec_with_events<'a>(
        &self,
        turn_diff_tracker: &mut TurnDiffTracker,
        begin_ctx: ExecCommandContext,
        exec_args: ExecInvokeArgs<'a>,
        seq_hint: Option<u64>,
        output_index: Option<u32>,
        attempt_req: u64,
    ) -> crate::error::Result<ExecToolCallOutput> {
        self
            .run_exec_with_events_inner(
                turn_diff_tracker,
                begin_ctx,
                exec_args,
                seq_hint,
                output_index,
                attempt_req,
                true,
            )
            .await
    }

    fn track_running_exec(
        &self,
        call_id: &str,
        sub_id: &str,
        order_meta: crate::protocol::OrderMeta,
        cancel_flag: Arc<AtomicBool>,
        end_emitted: Arc<AtomicBool>,
    ) {
        let mut state = self.state.lock().unwrap();
        state.running_execs.insert(
            call_id.to_string(),
            RunningExecMeta {
                sub_id: sub_id.to_string(),
                order_meta,
                cancel_flag,
                end_emitted,
            },
        );
    }

    fn unregister_running_exec(&self, call_id: &str) {
        let mut state = self.state.lock().unwrap();
        state.running_execs.remove(call_id);
    }

    fn mark_running_exec_as_cancelled(&self, sub_id: &str) {
        let state = self.state.lock().unwrap();
        for meta in state.running_execs.values() {
            if meta.sub_id == sub_id {
                meta.cancel_flag.store(true, Ordering::Release);
            }
        }
    }

    fn mark_all_running_execs_as_cancelled(&self) {
        let sub_ids: Vec<String> = {
            let state = self.state.lock().unwrap();
            state
                .running_execs
                .values()
                .map(|meta| meta.sub_id.clone())
                .collect()
        };
        for sub_id in sub_ids {
            self.mark_running_exec_as_cancelled(&sub_id);
        }
    }

    async fn finalize_cancelled_execs(&self, sub_id: &str) {
        let mut to_emit = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            let mut remove_keys = Vec::new();
            for (call_id, meta) in state.running_execs.iter() {
                if meta.sub_id == sub_id && !meta.end_emitted.load(Ordering::Acquire) {
                    to_emit.push((
                        call_id.clone(),
                        meta.order_meta.clone(),
                        meta.cancel_flag.clone(),
                        meta.end_emitted.clone(),
                    ));
                    remove_keys.push(call_id.clone());
                }
            }
            for key in remove_keys {
                state.running_execs.remove(&key);
            }
        }

        for (call_id, order_meta, cancel_flag, end_emitted) in to_emit {
            cancel_flag.store(true, Ordering::Release);
            if !end_emitted.swap(true, Ordering::AcqRel) {
                let (exit_code, stderr) = synthetic_exec_end_payload(true);
                let msg = EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                    call_id,
                    stdout: String::new(),
                    stderr,
                    exit_code,
                    duration: Duration::ZERO,
                });
                let event = self.make_event_with_order(sub_id, msg, order_meta.clone(), order_meta.sequence_number);
                let _ = self.tx_event.send(event).await;
            }
        }
    }

    async fn run_exec_with_events_inner<'a>(
        &self,
        turn_diff_tracker: &mut TurnDiffTracker,
        begin_ctx: ExecCommandContext,
        exec_args: ExecInvokeArgs<'a>,
        seq_hint: Option<u64>,
        output_index: Option<u32>,
        attempt_req: u64,
        enable_hooks: bool,
    ) -> crate::error::Result<ExecToolCallOutput> {
        let is_apply_patch = begin_ctx.apply_patch.is_some();
        let sub_id = begin_ctx.sub_id.clone();
        let call_id = begin_ctx.call_id.clone();

        let order_for_end = crate::protocol::OrderMeta {
            request_ordinal: attempt_req,
            output_index,
            sequence_number: seq_hint.map(|h| h.saturating_add(1)),
        };

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let end_emitted = Arc::new(AtomicBool::new(false));
        self.track_running_exec(&call_id, &sub_id, order_for_end.clone(), cancel_flag.clone(), end_emitted.clone());

        let mut exec_guard = ExecDropGuard::new(
            self.self_handle.clone(),
            self.tx_event.clone(),
            sub_id.clone(),
            call_id.clone(),
            order_for_end.clone(),
            cancel_flag,
            end_emitted,
        );

        let ExecInvokeArgs { params, sandbox_type, sandbox_policy, sandbox_cwd, code_linux_sandbox_exe, stdout_stream } = exec_args;
        let tracking_command = params.command.clone();
        let dry_run_analysis = analyze_command(&tracking_command);
        let params = maybe_run_with_user_profile(params, self);
        let params_for_hooks = if enable_hooks {
            Some(params.clone())
        } else {
            None
        };

        if enable_hooks {
            if let Some(params_ref) = params_for_hooks.as_ref() {
                let before_event = if is_apply_patch {
                    ProjectHookEvent::FileBeforeWrite
                } else {
                    ProjectHookEvent::ToolBefore
                };
                self
                    .run_hooks_for_exec_event(
                        turn_diff_tracker,
                        before_event,
                        &begin_ctx,
                        params_ref,
                        None,
                        attempt_req,
                    )
                    .await;
            }
        }

        self.on_exec_command_begin(turn_diff_tracker, begin_ctx.clone(), seq_hint, output_index, attempt_req)
            .await;

        let result = process_exec_tool_call(params, sandbox_type, sandbox_policy, sandbox_cwd, code_linux_sandbox_exe, stdout_stream)
        .await;

        let output_stderr;
        let borrowed: &ExecToolCallOutput = match &result {
            Ok(output) => output,
            Err(CodexErr::Sandbox(SandboxErr::Timeout { output })) => output,
            Err(e) => {
                output_stderr = ExecToolCallOutput {
                    exit_code: -1,
                    stdout: StreamOutput::new(String::new()),
                    stderr: StreamOutput::new(get_error_message_ui(e)),
                    aggregated_output: StreamOutput::new(get_error_message_ui(e)),
                    duration: Duration::default(),
                    timed_out: false,
                };
                &output_stderr
            }
        };
        self.on_exec_command_end(
            turn_diff_tracker,
            &sub_id,
            &call_id,
            borrowed,
            is_apply_patch,
            seq_hint.map(|h| h.saturating_add(1)),
            output_index,
            attempt_req,
        )
        .await;

        exec_guard.mark_completed();
        self.finalize_cancelled_execs(&sub_id).await;

        if enable_hooks {
            if let Some(params_ref) = params_for_hooks.as_ref() {
                let after_event = if is_apply_patch {
                    ProjectHookEvent::FileAfterWrite
                } else {
                    ProjectHookEvent::ToolAfter
                };
                self
                    .run_hooks_for_exec_event(
                        turn_diff_tracker,
                        after_event,
                        &begin_ctx,
                        params_ref,
                        Some(borrowed),
                        attempt_req,
                    )
                    .await;
            }
        }

        if let Some(analysis) = dry_run_analysis.as_ref() {
            let mut state = self.state.lock().unwrap();
            state.dry_run_guard.note_execution(analysis);
        }

        result
    }

    /// Helper that emits a BackgroundEvent with explicit ordering metadata.
    pub(crate) async fn notify_background_event_with_order(
        &self,
        sub_id: &str,
        order: crate::protocol::OrderMeta,
        message: impl Into<String>,
    ) {
        let event = self.make_event_with_order(
            sub_id,
            EventMsg::BackgroundEvent(BackgroundEventEvent { message: message.into() }),
            order,
            None,
        );
        let _ = self.tx_event.send(event).await;
    }

    async fn notify_stream_error(&self, sub_id: &str, message: impl Into<String>) {
        let event = self.make_event(
            sub_id,
            EventMsg::Error(ErrorEvent { message: message.into() }),
        );
        let _ = self.tx_event.send(event).await;
    }

    fn resolve_internal_sandbox(&self, with_escalated_permissions: bool) -> SandboxType {
        match assess_safety_for_untrusted_command(
            self.approval_policy,
            &self.sandbox_policy,
            with_escalated_permissions,
        ) {
            SafetyCheck::AutoApprove { sandbox_type, .. } => sandbox_type,
            SafetyCheck::AskUser | SafetyCheck::Reject { .. } => {
                crate::safety::get_platform_sandbox().unwrap_or(SandboxType::None)
            }
        }
    }

    async fn run_hooks_for_exec_event(
        &self,
        turn_diff_tracker: &mut TurnDiffTracker,
        event: ProjectHookEvent,
        exec_ctx: &ExecCommandContext,
        params: &ExecParams,
        output: Option<&ExecToolCallOutput>,
        attempt_req: u64,
    ) {
        if self.project_hooks.is_empty() {
            return;
        }
        let hooks: Vec<ProjectHook> = self.project_hooks.hooks_for(event).cloned().collect();
        if hooks.is_empty() {
            return;
        }
        let Some(_guard) = HookGuard::try_acquire(&self.hook_guard) else {
            return;
        };
        let payload = build_exec_hook_payload(event, exec_ctx, params, output);
        for (idx, hook) in hooks.into_iter().enumerate() {
            self
                .run_hook_command(turn_diff_tracker, &hook, event, &payload, Some(exec_ctx), attempt_req, idx)
                .await;
        }
    }

    async fn run_session_hooks(&self, event: ProjectHookEvent) {
        if self.project_hooks.is_empty() {
            return;
        }
        let hooks: Vec<ProjectHook> = self.project_hooks.hooks_for(event).cloned().collect();
        if hooks.is_empty() {
            return;
        }
        let Some(_guard) = HookGuard::try_acquire(&self.hook_guard) else {
            return;
        };
        let payload = self.build_session_payload(event);
        let mut tracker = TurnDiffTracker::new();
        let attempt_req = self.current_request_ordinal();
        for (idx, hook) in hooks.into_iter().enumerate() {
            self
                .run_hook_command(&mut tracker, &hook, event, &payload, None, attempt_req, idx)
                .await;
        }
    }

    fn build_session_payload(&self, event: ProjectHookEvent) -> Value {
        match event {
            ProjectHookEvent::SessionStart => json!({
                "event": event.as_str(),
                "cwd": self.cwd.to_string_lossy(),
                "sandbox_policy": format!("{}", self.sandbox_policy),
                "approval_policy": format!("{}", self.approval_policy),
            }),
            ProjectHookEvent::SessionEnd => json!({
                "event": event.as_str(),
                "cwd": self.cwd.to_string_lossy(),
                "sandbox_policy": format!("{}", self.sandbox_policy),
                "approval_policy": format!("{}", self.approval_policy),
            }),
            _ => json!({ "event": event.as_str() }),
        }
    }

    async fn run_hook_command(
        &self,
        turn_diff_tracker: &mut TurnDiffTracker,
        hook: &ProjectHook,
        event: ProjectHookEvent,
        payload: &Value,
        base_ctx: Option<&ExecCommandContext>,
        attempt_req: u64,
        index: usize,
    ) {
        let sub_id = base_ctx
            .map(|ctx| ctx.sub_id.clone())
            .unwrap_or_else(|| INITIAL_SUBMIT_ID.to_string());
        let base_slug = base_ctx
            .map(|ctx| sanitize_identifier(&ctx.call_id))
            .unwrap_or_else(|| event.slug().to_string());
        let call_id = format!("{base_slug}_hook_{}_{}", event.slug(), index + 1);

        let mut env = hook.env.clone();
        env.entry("CODE_HOOK_EVENT".to_string())
            .or_insert_with(|| event.as_str().to_string());
        env.entry("CODE_HOOK_TRIGGER".to_string())
            .or_insert_with(|| event.slug().to_string());
        env.insert("CODE_HOOK_CALL_ID".to_string(), call_id.clone());
        env.insert("CODE_HOOK_SUB_ID".to_string(), sub_id.clone());
        env.insert("CODE_HOOK_INDEX".to_string(), (index + 1).to_string());
        env.insert("CODE_HOOK_PAYLOAD".to_string(), payload.to_string());
        env.entry("CODE_SESSION_CWD".to_string())
            .or_insert_with(|| self.cwd.to_string_lossy().to_string());
        if let Some(name) = &hook.name {
            env.entry("CODE_HOOK_NAME".to_string())
                .or_insert_with(|| name.clone());
        }
        if let Some(ctx) = base_ctx {
            env.entry("CODE_HOOK_SOURCE_CALL_ID".to_string())
                .or_insert_with(|| ctx.call_id.clone());
        }

        let exec_params = ExecParams {
            command: hook.command.clone(),
            cwd: hook.resolved_cwd(self.get_cwd()),
            timeout_ms: hook.timeout_ms,
            env,
            with_escalated_permissions: Some(false),
            justification: None,
        };

        let exec_ctx = ExecCommandContext {
            sub_id: sub_id.clone(),
            call_id: call_id.clone(),
            command_for_display: exec_params.command.clone(),
            cwd: exec_params.cwd.clone(),
            apply_patch: None,
        };

        let sandbox_type = self.resolve_internal_sandbox(false);
        let exec_args = ExecInvokeArgs {
            params: exec_params,
            sandbox_type,
            sandbox_policy: &self.sandbox_policy,
            sandbox_cwd: self.get_cwd(),
            code_linux_sandbox_exe: &self.code_linux_sandbox_exe,
            stdout_stream: None,
        };

        if let Err(err) = Box::pin(self.run_exec_with_events_inner(
            turn_diff_tracker,
            exec_ctx,
            exec_args,
            None,
            None,
            attempt_req,
            false,
        ))
        .await
        {
            let hook_label = hook
                .name
                .as_deref()
                .unwrap_or_else(|| hook.command.first().map(String::as_str).unwrap_or("hook"));
            let order = self.next_background_order(&sub_id, attempt_req, None);
            self
                .notify_background_event_with_order(
                    &sub_id,
                    order,
                    format!("Hook `{}` failed: {}", hook_label, get_error_message_ui(&err)),
                )
                .await;
        }
    }

    fn find_project_command(&self, candidate: &str) -> Option<ProjectCommand> {
        self.project_commands
            .iter()
            .find(|cmd| cmd.matches(candidate))
            .cloned()
    }

    async fn run_project_command(
        &self,
        turn_diff_tracker: &mut TurnDiffTracker,
        sub_id: &str,
        name: &str,
        attempt_req: u64,
    ) {
        let Some(command) = self.find_project_command(name) else {
            let order = self.next_background_order(sub_id, attempt_req, None);
            self
                .notify_background_event_with_order(
                    sub_id,
                    order,
                    format!("Unknown project command `{}`", name.trim()),
                )
                .await;
            return;
        };

        let mut env = command.env.clone();
        env.entry("CODE_PROJECT_COMMAND_NAME".to_string())
            .or_insert_with(|| command.name.clone());
        if let Some(desc) = &command.description {
            env.entry("CODE_PROJECT_COMMAND_DESCRIPTION".to_string())
                .or_insert_with(|| desc.clone());
        }
        env.entry("CODE_SESSION_CWD".to_string())
            .or_insert_with(|| self.cwd.to_string_lossy().to_string());

        let exec_params = ExecParams {
            command: command.command.clone(),
            cwd: command.resolved_cwd(self.get_cwd()),
            timeout_ms: command.timeout_ms,
            env,
            with_escalated_permissions: Some(false),
            justification: None,
        };

        let call_id = format!("project_cmd_{}", sanitize_identifier(&command.name));
        let exec_ctx = ExecCommandContext {
            sub_id: sub_id.to_string(),
            call_id: call_id.clone(),
            command_for_display: exec_params.command.clone(),
            cwd: exec_params.cwd.clone(),
            apply_patch: None,
        };

        let sandbox_type = self.resolve_internal_sandbox(false);
        let exec_args = ExecInvokeArgs {
            params: exec_params,
            sandbox_type,
            sandbox_policy: &self.sandbox_policy,
            sandbox_cwd: self.get_cwd(),
            code_linux_sandbox_exe: &self.code_linux_sandbox_exe,
            stdout_stream: None,
        };

        if let Err(err) = self
            .run_exec_with_events(turn_diff_tracker, exec_ctx, exec_args, None, None, attempt_req)
            .await
        {
            let order = self.next_background_order(sub_id, attempt_req, None);
            self
                .notify_background_event_with_order(
                    sub_id,
                    order,
                    format!(
                        "Project command `{}` failed: {}",
                        command.name,
                        get_error_message_ui(&err)
                    ),
                )
                .await;
        }
    }

    /// Build the full turn input by concatenating the current conversation
    /// history with additional items for this turn.
    /// Browser screenshots are filtered out from history to keep them ephemeral.
    pub fn turn_input_with_history(&self, extra: Vec<ResponseItem>) -> Vec<ResponseItem> {
        let history = self.state.lock().unwrap().history.contents();

        // Debug: Count function call outputs in history
        let fc_output_count = history
            .iter()
            .filter(|item| matches!(item, ResponseItem::FunctionCallOutput { .. }))
            .count();
        if fc_output_count > 0 {
            debug!(
                "History contains {} FunctionCallOutput items",
                fc_output_count
            );
        }

        // Count images in extra for debugging (we can't distinguish ephemeral at this level anymore)
        let images_in_extra = extra
            .iter()
            .filter(|item| {
                if let ResponseItem::Message { content, .. } = item {
                    content
                        .iter()
                        .any(|c| matches!(c, ContentItem::InputImage { .. }))
                } else {
                    false
                }
            })
            .count();

        if images_in_extra > 0 {
            tracing::info!(
                "Found {} images in current turn's extra items",
                images_in_extra
            );
        }

        // Filter out browser screenshots from historical messages
        // We identify them by the [EPHEMERAL:...] marker that precedes them
        let filtered_history: Vec<ResponseItem> = history
            .into_iter()
            .map(|item| {
                if let ResponseItem::Message { id, role, content } = item {
                    if role == "user" {
                        // Filter out ephemeral content from user messages
                        let mut filtered_content: Vec<ContentItem> = Vec::new();
                        let mut skip_next_image = false;

                        for content_item in content {
                            match &content_item {
                                ContentItem::InputText { text }
                                    if text.starts_with("[EPHEMERAL:") =>
                                {
                                    // This is an ephemeral marker, skip it and the next image
                                    skip_next_image = true;
                                    tracing::info!("Filtering out ephemeral marker: {}", text);
                                }
                                ContentItem::InputImage { .. }
                                    if skip_next_image =>
                                {
                                    // Skip this image as it follows an ephemeral marker
                                    skip_next_image = false;
                                    tracing::info!("Filtering out ephemeral image from history");
                                }
                                _ => {
                                    // Keep everything else
                                    filtered_content.push(content_item);
                                }
                            }
                        }

                        ResponseItem::Message {
                            id,
                            role,
                            content: filtered_content,
                        }
                    } else {
                        // Keep assistant messages unchanged
                        ResponseItem::Message { id, role, content }
                    }
                } else {
                    item
                }
            })
            .collect();

        // Concatenate filtered history with current turn's extras (which includes current ephemeral images)
        let mut result = [filtered_history, extra].concat();

        let current_auth_mode = self
            .client
            .get_auth_manager()
            .and_then(|manager| manager.auth())
            .map(|auth| auth.mode);
        let sanitize_encrypted_reasoning = !matches!(current_auth_mode, Some(AuthMode::ChatGPT));

        if sanitize_encrypted_reasoning {
            let mut stripped = 0usize;
            result = result
                .into_iter()
                .map(|item| match item {
                    ResponseItem::Reasoning {
                        id,
                        summary,
                        content,
                        encrypted_content,
                    } => {
                        if encrypted_content.is_some() {
                            stripped += 1;
                        }
                        ResponseItem::Reasoning {
                            id,
                            summary,
                            content,
                            encrypted_content: None,
                        }
                    }
                    other => other,
                })
                .collect();
            if stripped > 0 {
                debug!(
                    "Stripped encrypted reasoning from {} history items before sending request",
                    stripped
                );
            }
        }

        debug_history("turn_input_with_history", &result);

        // Count total images in result for debugging
        let total_images = result
            .iter()
            .filter(|item| {
                if let ResponseItem::Message { content, .. } = item {
                    content
                        .iter()
                        .any(|c| matches!(c, ContentItem::InputImage { .. }))
                } else {
                    false
                }
            })
            .count();

        if total_images > 0 {
            tracing::info!("Total images being sent to model: {}", total_images);
        }

        result
    }

    pub(crate) fn build_initial_context(&self, turn_context: &TurnContext) -> Vec<ResponseItem> {
        let mut items = Vec::new();
        if let Some(user_instructions) = turn_context.user_instructions.as_deref() {
            items.push(UserInstructions::new(user_instructions.to_string()).into());
        }
        items.push(ResponseItem::from(EnvironmentContext::new(
            Some(turn_context.cwd.clone()),
            Some(turn_context.approval_policy),
            Some(turn_context.sandbox_policy.clone()),
            Some(self.user_shell.clone()),
        )));
        items
    }

    pub(crate) fn reconstruct_history_from_rollout(
        &self,
        turn_context: &TurnContext,
        rollout_items: &[RolloutItem],
    ) -> Vec<ResponseItem> {
        let mut history = self.build_initial_context(turn_context);
        for item in rollout_items {
            match item {
                RolloutItem::ResponseItem(response_item) => {
                    history.push(response_item.clone());
                }
                RolloutItem::Compacted(compacted) => {
                    let user_messages = collect_user_messages(&history);
                    history = build_compacted_history(
                        self.build_initial_context(turn_context),
                        &user_messages,
                        &compacted.message,
                    );
                }
                RolloutItem::Event(recorded_event) => {
                    if let code_protocol::protocol::EventMsg::UserMessage(user_msg_event) = &recorded_event.msg {
                        history.push(ResponseItem::Message {
                            id: Some(recorded_event.id.clone()),
                            role: "user".to_string(),
                            content: vec![ContentItem::InputText {
                                text: user_msg_event.message.clone(),
                            }],
                        });
                    }
                }
                _ => {}
            }
        }
        history
    }

    /// Returns the input if there was no agent running to inject into
    pub fn inject_input(&self, input: Vec<InputItem>) -> Result<(), Vec<InputItem>> {
        let mut state = self.state.lock().unwrap();
        if let Some(task) = state.current_task.as_ref() {
            let mut response = response_input_from_core_items(input);
            self.enforce_user_message_limits(&task.sub_id, &mut response);
            state.pending_input.push(response);
            Ok(())
        } else {
            Err(input)
        }
    }

    pub fn enqueue_manual_compact(&self, sub_id: String) -> bool {
        let mut state = self.state.lock().unwrap();
        let was_empty = state.pending_manual_compacts.is_empty();
        state.pending_manual_compacts.push_back(sub_id);
        was_empty
    }

    pub fn dequeue_manual_compact(&self) -> Option<String> {
        let mut state = self.state.lock().unwrap();
        state.pending_manual_compacts.pop_front()
    }


    pub fn get_pending_input(&self) -> Vec<ResponseInputItem> {
        self.get_pending_input_filtered(true)
    }

    /// Returns pending input for the current turn. Callers can decide whether
    /// queued user inputs should be drained immediately (`drain_user_inputs = true`)
    /// or preserved for a later turn—for example, review mode keeps them queued
    /// so the primary agent can resume once the review finishes.
    pub fn get_pending_input_filtered(&self, drain_user_inputs: bool) -> Vec<ResponseInputItem> {
        let mut state = self.state.lock().unwrap();
        if state.pending_input.is_empty()
            && (drain_user_inputs || state.pending_user_input.is_empty())
        {
            Vec::with_capacity(0)
        } else {
            let mut ret = Vec::new();
            if !state.pending_input.is_empty() {
                let mut model_inputs = Vec::new();
                std::mem::swap(&mut model_inputs, &mut state.pending_input);
                ret.extend(model_inputs);
            }

            if !state.pending_user_input.is_empty() {
                if drain_user_inputs {
                    let mut queued_user_inputs = Vec::new();
                    std::mem::swap(&mut queued_user_inputs, &mut state.pending_user_input);
                    ret.extend(
                        queued_user_inputs
                            .into_iter()
                            .map(|queued| queued.response_item),
                    );
                } else {
                    ret.extend(
                        state
                            .pending_user_input
                            .iter()
                            .map(|queued| queued.response_item.clone()),
                    );
                }
            }
            ret
        }
    }

    pub fn add_pending_input(&self, mut input: ResponseInputItem) {
        let mut state = self.state.lock().unwrap();
        if let Some(task) = state.current_task.as_ref() {
            self.enforce_user_message_limits(&task.sub_id, &mut input);
        }
        state.pending_input.push(input);
    }

    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> anyhow::Result<CallToolResult> {
        self.mcp_connection_manager
            .call_tool(server, tool, arguments, timeout)
            .await
    }

    fn abort(&self) {
        info!("Aborting existing session");

        self.mark_all_running_execs_as_cancelled();

        Self::spawn_kill_exec_sessions(Arc::clone(&self.session_manager));

        let mut state = self.state.lock().unwrap();
        state.pending_approvals.clear();
        // Do not clear `pending_input` here. When a user submits a new message
        // immediately after an interrupt, it may have been routed to
        // `pending_input` by an earlier code path. Clearing it would drop the
        // user's message and prevent the next turn from ever starting.
        state.turn_scratchpad = None;
        // Take current task while holding the lock, then drop the lock BEFORE calling abort
        let current = state.current_task.take();
        drop(state);
        if let Some(agent) = current {
            agent.abort(TurnAbortReason::Interrupted);
        }
        // Also terminate any running exec sessions (PTY-based) so child processes do not linger.
        // Best-effort cleanup for PTY-based exec sessions would go here. The
        // PTY implementation already kills processes on session drop; in the
        // common LocalShellCall path we also kill processes immediately via
        // KillOnDrop in exec.rs.
    }

    /// Spawn the configured notifier (if any) with the given JSON payload as
    /// the last argument. Failures are logged but otherwise ignored so that
    /// notification issues do not interfere with the main workflow.
    fn maybe_notify(&self, notification: UserNotification) {
        let Some(notify_command) = &self.notify else {
            return;
        };

        if notify_command.is_empty() {
            return;
        }

        let Ok(json) = serde_json::to_string(&notification) else {
            error!("failed to serialise notification payload");
            return;
        };

        let mut command = std::process::Command::new(&notify_command[0]);
        if notify_command.len() > 1 {
            command.args(&notify_command[1..]);
        }
        command.arg(json);

        // Fire-and-forget – we do not wait for completion.
        if let Err(e) = command.spawn() {
            warn!("failed to spawn notifier '{}': {e}", notify_command[0]);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Interrupt any running turn when the session is dropped.
        self.abort();
    }
}

#[cfg(test)]
mod tests {
    use async_channel::unbounded;
    use code_app_server_protocol::ConversationId as OtelConversationId;
    use crate::exec_command::exec_command_params::{ExecCommandParams, WriteStdinParams};
    use crate::exec_command::session_manager::SessionManager;
    use tempfile::TempDir;
    use tokio::time::{sleep, timeout, Duration};
    use super::*;

    #[cfg(all(unix, not(miri)))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn abort_kills_foreground_exec_session() {
        let session_manager = Arc::new(SessionManager::default());
        let cmd = r#"python3 - <<'PY'
import time
while True:
    time.sleep(1)
PY"#
            .to_string();

        let params = ExecCommandParams {
            cmd,
            yield_time_ms: 500,
            max_output_tokens: 128,
            shell: "/bin/bash".to_string(),
            login: false,
        };

        let initial_output = match session_manager
            .handle_exec_command_request(params)
            .await
        {
            Ok(output) => output,
            Err(e) => {
                if e.contains("openpty") || e.contains("Operation not permitted") {
                    eprintln!("skipping test due to restricted PTY: {e}");
                    return;
                }
                panic!("exec request failed unexpectedly: {e}");
            }
        };

        let session_id = match initial_output.exit_status {
            ExitStatus::Ongoing(id) => id,
            other => {
                eprintln!(
                    "skipping test: command exited early: {other:?} output={}",
                    initial_output.output
                );
                return;
            }
        };

        assert_eq!(session_manager.active_session_count().await, 1);

        let temp_dir = TempDir::new().expect("temp dir");
        let config = Arc::new(
            Config::load_from_base_config_with_overrides(
                ConfigToml::default(),
                ConfigOverrides::default(),
                temp_dir.path().to_path_buf(),
            )
            .expect("load config"),
        );

        let provider = config.model_provider.clone();
        let effort = config.model_reasoning_effort;
        let summary = config.model_reasoning_summary;
        let verbosity = config.model_text_verbosity;
        let debug_logger = Arc::new(Mutex::new(DebugLogger::new(false).expect("debug logger")));
        let otel_manager = OtelEventManager::new(
            OtelConversationId::default(),
            config.model.as_str(),
            config.model_family.slug.as_str(),
            None,
            None,
            config.otel.log_user_prompt,
            "test".to_string(),
        );

        let client = ModelClient::new(
            Arc::clone(&config),
            None,
            Some(otel_manager),
            provider,
            effort,
            summary,
            verbosity,
            Uuid::new_v4(),
            Arc::clone(&debug_logger),
        );

        let tools_config = client.build_tools_config();
        let (tx_event, _rx_event) = unbounded();

        let approval_policy = config.approval_policy;
        let sandbox_policy = config.sandbox_policy.clone();
        let shell_environment_policy = config.shell_environment_policy.clone();
        let disable_response_storage = config.disable_response_storage;
        let client_tools = config.experimental_client_tools.clone();
        let agents = config.agents.clone();
        let notify = config.notify.clone();
        let code_linux_sandbox_exe = config.code_linux_sandbox_exe.clone();
        let show_raw_agent_reasoning = config.show_raw_agent_reasoning;
        let confirm_guard = ConfirmGuardRuntime::from_config(&config.confirm_guard);
        let project_hooks = config.project_hooks.clone();
        let project_commands = config.project_commands.clone();
        let github_cfg = config.github.clone();
        let validation_cfg = config.validation.clone();
        let tx_event_clone = tx_event.clone();
        let session_manager_clone = Arc::clone(&session_manager);

        let session = Arc::new_cyclic(|weak| Session {
            id: Uuid::new_v4(),
            client,
            tx_event: tx_event_clone,
            cwd: temp_dir.path().to_path_buf(),
            base_instructions: None,
            user_instructions: None,
            approval_policy,
            sandbox_policy,
            shell_environment_policy,
            _writable_roots: Vec::new(),
            disable_response_storage,
            tools_config,
            mcp_connection_manager: McpConnectionManager::default(),
            client_tools,
            session_manager: session_manager_clone,
            agents,
            notify,
            rollout: Mutex::new(None),
            state: Mutex::new(State::default()),
            code_linux_sandbox_exe,
            user_shell: shell::Shell::Unknown,
            show_raw_agent_reasoning,
            pending_browser_screenshots: Mutex::new(Vec::new()),
            last_system_status: Mutex::new(None),
            last_screenshot_info: Mutex::new(None),
            confirm_guard,
            project_hooks,
            project_commands,
            hook_guard: AtomicBool::new(false),
            github: Arc::new(RwLock::new(github_cfg)),
            validation: Arc::new(RwLock::new(validation_cfg)),
            self_handle: weak.clone(),
            active_review: Mutex::new(None),
            next_turn_text_format: Mutex::new(None),
        });

        session.abort();

        timeout(Duration::from_secs(3), async {
            loop {
                if session_manager.active_session_count().await == 0 {
                    break;
                }
                sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("exec sessions should stop promptly");

        let write_err = session_manager
            .handle_write_stdin_request(WriteStdinParams {
                session_id,
                chars: String::new(),
                yield_time_ms: 100,
                max_output_tokens: 8,
            })
            .await
            .expect_err("session should be gone after abort");
        assert!(
            write_err.contains("session"),
            "unexpected error message: {write_err}"
        );
    }
}

impl State {
    pub fn partial_clone(&self) -> Self {
        Self {
            approved_commands: self.approved_commands.clone(),
            history: self.history.clone(),
            // Preserve request_ordinal so reconfigurations (e.g., /reasoning)
            // do not reset provider ordering mid-session.
            request_ordinal: self.request_ordinal,
            background_seq_by_sub_id: self.background_seq_by_sub_id.clone(),
            dry_run_guard: self.dry_run_guard.clone(),
            next_internal_sub_id: self.next_internal_sub_id,
            ..Default::default()
        }
    }
}

fn synthetic_exec_end_payload(cancelled: bool) -> (i32, String) {
    if cancelled {
        (130, "Command cancelled by user.".to_string())
    } else {
        (130, "Command interrupted before completion.".to_string())
    }
}

struct ExecDropGuard {
    sub_id: String,
    call_id: String,
    order_meta: crate::protocol::OrderMeta,
    tx_event: Sender<Event>,
    cancel_flag: Arc<AtomicBool>,
    end_emitted: Arc<AtomicBool>,
    session: Weak<Session>,
    completed: bool,
}

impl ExecDropGuard {
    fn new(
        session: Weak<Session>,
        tx_event: Sender<Event>,
        sub_id: String,
        call_id: String,
        order_meta: crate::protocol::OrderMeta,
        cancel_flag: Arc<AtomicBool>,
        end_emitted: Arc<AtomicBool>,
    ) -> Self {
        Self {
            sub_id,
            call_id,
            order_meta,
            tx_event,
            cancel_flag,
            end_emitted,
            session,
            completed: false,
        }
    }

    fn mark_completed(&mut self) {
        self.completed = true;
        self.end_emitted.store(true, Ordering::Release);
        self.remove_from_registry();
    }

    fn remove_from_registry(&self) {
        if let Some(session) = self.session.upgrade() {
            session.unregister_running_exec(&self.call_id);
        }
    }
}

impl Drop for ExecDropGuard {
    fn drop(&mut self) {
        self.remove_from_registry();

        if self.completed {
            return;
        }

        if self.end_emitted.swap(true, Ordering::AcqRel) {
            return;
        }

        let (exit_code, stderr) = synthetic_exec_end_payload(
            self.cancel_flag.load(Ordering::Acquire),
        );
        let msg = EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: self.call_id.clone(),
            stdout: String::new(),
            stderr,
            exit_code,
            duration: Duration::ZERO,
        });

        if let Some(session) = self.session.upgrade() {
            let event = session.make_event_with_order(
                &self.sub_id,
                msg,
                self.order_meta.clone(),
                self.order_meta.sequence_number,
            );
            let _ = self.tx_event.try_send(event);
        } else {
            // Fallback: emit directly if session no longer exists.
            let event = Event {
                id: self.sub_id.clone(),
                event_seq: 0,
                msg,
                order: Some(self.order_meta.clone()),
            };
            let _ = self.tx_event.try_send(event);
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ExecCommandContext {
    pub(crate) sub_id: String,
    pub(crate) call_id: String,
    pub(crate) command_for_display: Vec<String>,
    pub(crate) cwd: PathBuf,
    pub(crate) apply_patch: Option<ApplyPatchCommandContext>,
}

#[derive(Clone, Debug)]
pub(crate) struct ApplyPatchCommandContext {
    pub(crate) user_explicitly_approved_this_action: bool,
    pub(crate) changes: HashMap<PathBuf, FileChange>,
}

fn sanitize_identifier(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else {
            slug.push('_');
        }
    }
    while slug.starts_with('_') {
        slug.remove(0);
    }
    if slug.is_empty() {
        slug.push_str("hook");
    }
    slug
}

fn truncate_payload(text: &str, limit: usize) -> String {
    let mut iter = text.chars();
    let truncated: String = iter.by_ref().take(limit).collect();
    if iter.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn build_exec_hook_payload(
    event: ProjectHookEvent,
    ctx: &ExecCommandContext,
    params: &ExecParams,
    output: Option<&ExecToolCallOutput>,
) -> Value {
    let base = json!({
        "event": event.as_str(),
        "call_id": ctx.call_id,
        "cwd": ctx.cwd.to_string_lossy(),
        "command": params.command,
        "timeout_ms": params.timeout_ms,
    });

    match event {
        ProjectHookEvent::ToolBefore => base,
        ProjectHookEvent::ToolAfter => {
            if let Some(out) = output {
                json!({
                    "event": event.as_str(),
                    "call_id": ctx.call_id,
                    "cwd": ctx.cwd.to_string_lossy(),
                    "command": params.command,
                    "timeout_ms": params.timeout_ms,
                    "exit_code": out.exit_code,
                    "duration_ms": out.duration.as_millis(),
                    "timed_out": out.timed_out,
                    "stdout": truncate_payload(&out.stdout.text, HOOK_OUTPUT_LIMIT),
                    "stderr": truncate_payload(&out.stderr.text, HOOK_OUTPUT_LIMIT),
                })
            } else {
                base
            }
        }
        ProjectHookEvent::FileBeforeWrite => {
            let changes = ctx
                .apply_patch
                .as_ref()
                .and_then(|p| serde_json::to_value(&p.changes).ok())
                .unwrap_or(Value::Null);
            json!({
                "event": event.as_str(),
                "call_id": ctx.call_id,
                "cwd": ctx.cwd.to_string_lossy(),
                "command": params.command,
                "timeout_ms": params.timeout_ms,
                "changes": changes,
            })
        }
        ProjectHookEvent::FileAfterWrite => {
            let changes = ctx
                .apply_patch
                .as_ref()
                .and_then(|p| serde_json::to_value(&p.changes).ok())
                .unwrap_or(Value::Null);
            if let Some(out) = output {
                json!({
                    "event": event.as_str(),
                    "call_id": ctx.call_id,
                    "cwd": ctx.cwd.to_string_lossy(),
                    "command": params.command,
                    "timeout_ms": params.timeout_ms,
                    "changes": changes,
                    "exit_code": out.exit_code,
                    "duration_ms": out.duration.as_millis(),
                    "timed_out": out.timed_out,
                    "stdout": truncate_payload(&out.stdout.text, HOOK_OUTPUT_LIMIT),
                    "stderr": truncate_payload(&out.stderr.text, HOOK_OUTPUT_LIMIT),
                    "success": out.exit_code == 0,
                })
            } else {
                json!({
                    "event": event.as_str(),
                    "call_id": ctx.call_id,
                    "cwd": ctx.cwd.to_string_lossy(),
                    "command": params.command,
                    "timeout_ms": params.timeout_ms,
                    "changes": changes,
                })
            }
        }
        _ => base,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AgentTaskKind {
    Regular,
    Review,
    Compact,
}

/// A series of Turns in response to user input.
pub(crate) struct AgentTask {
    sess: Arc<Session>,
    sub_id: String,
    handle: AbortHandle,
    kind: AgentTaskKind,
}

impl AgentTask {
    fn spawn(
        sess: Arc<Session>,
        turn_context: Arc<TurnContext>,
        sub_id: String,
        input: Vec<InputItem>,
    ) -> Self {
        let handle = {
            let sess_clone = Arc::clone(&sess);
            let tc_clone = Arc::clone(&turn_context);
            let sub_clone = sub_id.clone();
            tokio::spawn(async move {
                run_agent(sess_clone, tc_clone, sub_clone, input).await;
            })
            .abort_handle()
        };
        Self {
            sess,
            sub_id,
            handle,
            kind: AgentTaskKind::Regular,
        }
    }

    fn compact(
        sess: Arc<Session>,
        turn_context: Arc<TurnContext>,
        sub_id: String,
        input: Vec<InputItem>,
        compact_instructions: String,
    ) -> Self {
        let handle = {
            let sess_clone = Arc::clone(&sess);
            let tc_clone = Arc::clone(&turn_context);
            let sub_clone = sub_id.clone();
            tokio::spawn(async move {
                compact::run_compact_task(
                    sess_clone,
                    tc_clone,
                    sub_clone,
                    input,
                    compact_instructions,
                )
                .await;
            })
            .abort_handle()
        };
        Self {
            sess,
            sub_id,
            handle,
            kind: AgentTaskKind::Compact,
        }
    }

    fn review(
        sess: Arc<Session>,
        turn_context: Arc<TurnContext>,
        sub_id: String,
        input: Vec<InputItem>,
    ) -> Self {
        let handle = {
            let sess_clone = Arc::clone(&sess);
            let tc_clone = Arc::clone(&turn_context);
            let sub_clone = sub_id.clone();
            tokio::spawn(async move {
                run_agent(sess_clone, tc_clone, sub_clone, input).await;
            })
            .abort_handle()
        };
        Self {
            sess,
            sub_id,
            handle,
            kind: AgentTaskKind::Review,
        }
    }

    fn abort(self, reason: TurnAbortReason) {
        if !self.handle.is_finished() {
            self.handle.abort();
            let event = self
                .sess
                .make_event(&self.sub_id, EventMsg::TurnAborted(TurnAbortedEvent { reason }));
            let sess = self.sess.clone();
            let sub_id = self.sub_id.clone();
            let kind = self.kind;
            tokio::spawn(async move {
                if kind == AgentTaskKind::Review {
                    exit_review_mode(sess.clone(), sub_id, None).await;
                }
                sess.send_event(event).await;
            });
        }
    }
}

async fn submission_loop(
    mut session_id: Uuid,
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    rx_sub: Receiver<Submission>,
    tx_event: Sender<Event>,
) {
    let mut config = config;
    let mut sess: Option<Arc<Session>> = None;
    let mut agent_manager_initialized = false;
    // shorthand - send an event when there is no active session
    let send_no_session_event = |sub_id: String| async {
        let event = Event {
            id: sub_id,
            event_seq: 0,
            msg: EventMsg::Error(ErrorEvent { message: "No session initialized, expected 'ConfigureSession' as first Op".to_string() }),
            order: None,
        };
        tx_event.send(event).await.ok();
    };

    // To break out of this loop, send Op::Shutdown.
    while let Ok(sub) = rx_sub.recv().await {
        debug!(?sub, "Submission");
        match sub.op {
            Op::Interrupt => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess.clone(),
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };
                tokio::spawn(async move {
                    sess.notify_wait_interrupted(WaitInterruptReason::SessionAborted);
                    sess.abort();
                });
            }
            Op::CancelAgents { batch_ids, agent_ids } => {
                let sess_arc = match sess.as_ref() {
                    Some(sess) => Arc::clone(sess),
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };

                let mut manager = AGENT_MANAGER.write().await;
                let mut seen_batches: HashSet<String> = HashSet::new();
                let mut seen_agents: HashSet<String> = HashSet::new();
                let mut cancelled = 0usize;

                for batch in batch_ids {
                    let trimmed = batch.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if !seen_batches.insert(trimmed.to_string()) {
                        continue;
                    }
                    cancelled += manager.cancel_batch(trimmed).await;
                }

                for agent_id in agent_ids {
                    let trimmed = agent_id.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if !seen_agents.insert(trimmed.to_string()) {
                        continue;
                    }
                    if manager.cancel_agent(trimmed).await {
                        cancelled += 1;
                    }
                }

                drop(manager);

                send_agent_status_update(&sess_arc).await;

                let message = if cancelled == 0 {
                    "No running agents to cancel.".to_string()
                } else {
                    let suffix = if cancelled == 1 { "" } else { "s" };
                    format!("Cancelled {cancelled} running agent{suffix}.")
                };

                let event = sess_arc.make_event(
                    &sub.id,
                    EventMsg::AgentMessage(AgentMessageEvent { message }),
                );
                sess_arc.send_event(event).await;
            }
            Op::AddPendingInputDeveloper { text } => {
                let sess = match sess.as_ref() { Some(s) => s.clone(), None => { send_no_session_event(sub.id).await; continue; } };
                let dev_msg = ResponseInputItem::Message { role: "developer".to_string(), content: vec![ContentItem::InputText { text }] };
                let should_start_turn = sess.enqueue_out_of_turn_item(dev_msg);
                if should_start_turn {
                    sess.cleanup_old_status_items().await;
                    let turn_context = sess.make_turn_context();
                    let sub_id = sess.next_internal_sub_id();
                    let sentinel_input = vec![InputItem::Text {
                        text: PENDING_ONLY_SENTINEL.to_string(),
                    }];
                    let agent = AgentTask::spawn(Arc::clone(&sess), turn_context, sub_id, sentinel_input);
                    sess.set_task(agent);
                }
            }
            Op::ConfigureSession {
                provider,
                model,
                model_reasoning_effort,
                model_reasoning_summary,
                model_text_verbosity,
                user_instructions: provided_user_instructions,
                base_instructions,
                approval_policy,
                sandbox_policy,
                disable_response_storage,
                notify,
                cwd,
                resume_path,
            } => {
                debug!(
                    "Configuring session: model={model}; provider={provider:?}; resume={resume_path:?}"
                );
                if !cwd.is_absolute() {
                    let message = format!("cwd is not absolute: {cwd:?}");
                    error!(message);
                    let event = Event { id: sub.id, event_seq: 0, msg: EventMsg::Error(ErrorEvent { message }), order: None };
                    if let Err(e) = tx_event.send(event).await {
                        error!("failed to send error message: {e:?}");
                    }
                    return;
                }
                let current_config = Arc::clone(&config);
                let mut updated_config = (*current_config).clone();

                let model_changed = !updated_config.model.eq_ignore_ascii_case(&model);
                let effort_changed = updated_config.model_reasoning_effort != model_reasoning_effort;

                let old_model_family = updated_config.model_family.clone();
                let old_model_info = get_model_info(&old_model_family);

                updated_config.model = model.clone();
                updated_config.model_provider = provider.clone();
                updated_config.model_reasoning_effort = model_reasoning_effort;
                updated_config.model_reasoning_summary = model_reasoning_summary;
                updated_config.model_text_verbosity = model_text_verbosity;
                updated_config.user_instructions = provided_user_instructions.clone();
                updated_config.base_instructions = base_instructions.clone();
                updated_config.approval_policy = approval_policy;
                updated_config.sandbox_policy = sandbox_policy.clone();
                updated_config.disable_response_storage = disable_response_storage;
                updated_config.notify = notify.clone();
                updated_config.cwd = cwd.clone();

                updated_config.model_family = find_family_for_model(&updated_config.model)
                    .unwrap_or_else(|| derive_default_model_family(&updated_config.model));

                let new_model_info = get_model_info(&updated_config.model_family);

                let old_context_window = old_model_info.as_ref().map(|info| info.context_window);
                let new_context_window = new_model_info.as_ref().map(|info| info.context_window);
                let old_max_tokens = old_model_info.as_ref().map(|info| info.max_output_tokens);
                let new_max_tokens = new_model_info.as_ref().map(|info| info.max_output_tokens);
                let old_auto_compact = old_model_info
                    .as_ref()
                    .and_then(|info| info.auto_compact_token_limit);
                let new_auto_compact = new_model_info
                    .as_ref()
                    .and_then(|info| info.auto_compact_token_limit);

                maybe_update_from_model_info(
                    &mut updated_config.model_context_window,
                    old_context_window,
                    new_context_window,
                );
                maybe_update_from_model_info(
                    &mut updated_config.model_max_output_tokens,
                    old_max_tokens,
                    new_max_tokens,
                );
                maybe_update_from_model_info(
                    &mut updated_config.model_auto_compact_token_limit,
                    old_auto_compact,
                    new_auto_compact,
                );

                let computed_user_instructions =
                    get_user_instructions(&updated_config).await;
                updated_config.user_instructions = computed_user_instructions.clone();

                let effective_user_instructions = computed_user_instructions.clone();

                // Optionally resume an existing rollout.
                let mut restored_items: Option<Vec<RolloutItem>> = None;
                let mut restored_history_snapshot: Option<crate::history::HistorySnapshot> = None;
                let mut resume_notice: Option<String> = None;
                let mut rollout_recorder: Option<RolloutRecorder> = None;
                if let Some(path) = resume_path.as_ref() {
                    match RolloutRecorder::resume(&updated_config, path).await {
                        Ok((rec, saved)) => {
                            session_id = saved.session_id;
                            if !saved.items.is_empty() {
                                restored_items = Some(saved.items);
                            }
                            if let Some(snapshot) = saved.history_snapshot {
                                restored_history_snapshot = Some(snapshot);
                            }
                            rollout_recorder = Some(rec);
                        }
                        Err(e) => {
                            warn!("failed to resume rollout from {path:?}: {e}");
                            resume_notice = Some(format!(
                                "⚠️ Failed to load previous session from {}: {e}. Starting a new conversation instead.",
                                path.display()
                            ));
                            updated_config.experimental_resume = None;
                        }
                    }
                }

                let new_config = Arc::new(updated_config);

                if model_changed || effort_changed {
                    if let Err(err) = persist_model_selection(
                        &new_config.code_home,
                        new_config.active_profile.as_deref(),
                        &new_config.model,
                        Some(new_config.model_reasoning_effort),
                    )
                    .await
                    {
                        warn!("failed to persist model selection: {err:#}");
                    }
                }

                config = Arc::clone(&new_config);

                let rollout_recorder = match rollout_recorder {
                    Some(rec) => Some(rec),
                    None => {
                        match RolloutRecorder::new(
                            &config,
                            crate::rollout::recorder::RolloutRecorderParams::new(
                                code_protocol::mcp_protocol::ConversationId::from(session_id),
                                effective_user_instructions.clone(),
                                SessionSource::Cli,
                            ),
                        )
                            .await
                        {
                            Ok(r) => Some(r),
                            Err(e) => {
                                warn!("failed to initialise rollout recorder: {e}");
                                None
                            }
                        }
                    }
                };

                // Create debug logger based on config
                let debug_logger = match crate::debug_logger::DebugLogger::new(config.debug) {
                    Ok(logger) => std::sync::Arc::new(std::sync::Mutex::new(logger)),
                    Err(e) => {
                        warn!("Failed to create debug logger: {}", e);
                        // Create a disabled logger as fallback
                        std::sync::Arc::new(std::sync::Mutex::new(
                            crate::debug_logger::DebugLogger::new(false).unwrap(),
                        ))
                    }
                };

                if config.debug {
                    if let Ok(logger) = debug_logger.lock() {
                        if let Err(e) = logger.set_session_usage_file(&session_id) {
                            warn!("failed to initialise session usage log: {e}");
                        }
                    }
                }

                let conversation_id = code_protocol::mcp_protocol::ConversationId::from(session_id);
                let auth_snapshot = auth_manager.as_ref().and_then(|mgr| mgr.auth());
                let otel_event_manager = {
                    let manager = OtelEventManager::new(
                        conversation_id,
                        config.model.as_str(),
                        config.model_family.slug.as_str(),
                        auth_snapshot
                            .as_ref()
                            .and_then(|auth| auth.get_account_id()),
                        auth_snapshot.as_ref().map(|auth| auth.mode),
                        config.otel.log_user_prompt,
                        crate::terminal::user_agent(),
                    );
                    manager.conversation_starts(
                        config.model_provider.name.as_str(),
                        Some(to_proto_reasoning_effort(model_reasoning_effort)),
                        to_proto_reasoning_summary(model_reasoning_summary),
                        config.model_context_window,
                        config.model_max_output_tokens,
                        config.model_auto_compact_token_limit,
                        to_proto_approval_policy(approval_policy),
                        to_proto_sandbox_policy(sandbox_policy.clone()),
                        config
                            .mcp_servers
                            .keys()
                            .map(String::as_str)
                            .collect(),
                        config.active_profile.clone(),
                    );
                    manager
                };

                // Wrap provided auth (if any) in a minimal AuthManager for client usage.
                let client = ModelClient::new(
                    config.clone(),
                    auth_manager.clone(),
                    Some(otel_event_manager.clone()),
                    provider.clone(),
                    model_reasoning_effort,
                    model_reasoning_summary,
                    model_text_verbosity,
                    session_id,
                    debug_logger,
                );

                // abort any current running session and clone its state
                let old_session = sess.take();
                let state = if let Some(sess_arc) = old_session.as_ref() {
                    sess_arc.notify_wait_interrupted(WaitInterruptReason::SessionAborted);
                    sess_arc.abort();
                    sess_arc.state.lock().unwrap().partial_clone()
                } else {
                    State {
                        history: ConversationHistory::new(),
                        ..Default::default()
                    }
                };

                let writable_roots = get_writable_roots(&cwd);

                // Error messages to dispatch after SessionConfigured is sent.
                let mut mcp_connection_errors = Vec::<String>::new();
                let mut excluded_tools = HashSet::new();
                if let Some(client_tools) = config.experimental_client_tools.as_ref() {
                    for tool in [
                        client_tools.request_permission.as_ref(),
                        client_tools.read_text_file.as_ref(),
                        client_tools.write_text_file.as_ref(),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        excluded_tools.insert((
                            tool.mcp_server.to_string(),
                            tool.tool_name.to_string(),
                        ));
                    }
                }

                if let Some(old_session_arc) = old_session {
                    old_session_arc.shutdown_mcp_clients().await;
                    drop(old_session_arc);
                }

                let (mcp_connection_manager, failed_clients) = match McpConnectionManager::new(
                    config.mcp_servers.clone(),
                    config.use_experimental_use_rmcp_client,
                    excluded_tools,
                )
                .await
                {
                    Ok((mgr, failures)) => (mgr, failures),
                    Err(e) => {
                        let message = format!("Failed to create MCP connection manager: {e:#}");
                        error!("{message}");
                        mcp_connection_errors.push(message);
                        (McpConnectionManager::default(), Default::default())
                    }
                };

                // Surface individual client start-up failures to the user.
                if !failed_clients.is_empty() {
                    for (server_name, err) in failed_clients {
                        let message =
                            format!("MCP client for `{server_name}` failed to start: {err:#}");
                        error!("{message}");
                        mcp_connection_errors.push(message);
                    }
                }
                let default_shell = shell::default_user_shell().await;
                let mut tools_config = ToolsConfig::new(
                    &config.model_family,
                    approval_policy,
                    sandbox_policy.clone(),
                    config.include_plan_tool,
                    config.include_apply_patch_tool,
                    config.tools_web_search_request,
                    config.use_experimental_streamable_shell_tool,
                    config.include_view_image_tool,
                );
                tools_config.web_search_allowed_domains =
                    config.tools_web_search_allowed_domains.clone();

                let mut agent_models: Vec<String> = if config.agents.is_empty() {
                    default_agent_configs()
                        .into_iter()
                        .filter(|cfg| cfg.enabled)
                        .map(|cfg| cfg.name)
                        .collect()
                } else {
                    get_enabled_agents(&config.agents)
                };
                if agent_models.is_empty() {
                    agent_models = enabled_agent_model_specs()
                        .into_iter()
                        .map(|spec| spec.slug.to_string())
                        .collect();
                }
                agent_models.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
                agent_models.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
                tools_config.set_agent_models(agent_models);

                let mut new_session = Arc::new(Session {
                    id: session_id,
                    client,
                    tools_config,
                    tx_event: tx_event.clone(),
                    user_instructions: effective_user_instructions.clone(),
                    base_instructions,
                    approval_policy,
                    sandbox_policy,
                    shell_environment_policy: config.shell_environment_policy.clone(),
                    cwd,
                    _writable_roots: writable_roots,
            mcp_connection_manager,
            client_tools: config.experimental_client_tools.clone(),
                    session_manager: Arc::new(crate::exec_command::ExecSessionManager::default()),
                    agents: config.agents.clone(),
                    notify,
                    state: Mutex::new(state),
                    rollout: Mutex::new(rollout_recorder),
                    code_linux_sandbox_exe: config.code_linux_sandbox_exe.clone(),
                    disable_response_storage,
                    user_shell: default_shell,
                    show_raw_agent_reasoning: config.show_raw_agent_reasoning,
                    pending_browser_screenshots: Mutex::new(Vec::new()),
                    last_system_status: Mutex::new(None),
                    last_screenshot_info: Mutex::new(None),
                    confirm_guard: ConfirmGuardRuntime::from_config(&config.confirm_guard),
                    project_hooks: config.project_hooks.clone(),
                    project_commands: config.project_commands.clone(),
                    hook_guard: AtomicBool::new(false),
                    github: Arc::new(RwLock::new(config.github.clone())),
                    validation: Arc::new(RwLock::new(config.validation.clone())),
                    self_handle: Weak::new(),
                    active_review: Mutex::new(None),
                    next_turn_text_format: Mutex::new(None),
                });
                let weak_handle = Arc::downgrade(&new_session);
                if let Some(inner) = Arc::get_mut(&mut new_session) {
                    inner.self_handle = weak_handle;
                }
                sess = Some(new_session);
                if let Some(sess_arc) = &sess {
                    if !config.always_allow_commands.is_empty() {
                        let mut st = sess_arc.state.lock().unwrap();
                        for pattern in &config.always_allow_commands {
                            st.approved_commands.insert(pattern.clone());
                        }
                    }
                }
                let mut replay_history_items: Option<Vec<ResponseItem>> = None;


                // Patch restored state into the newly created session.
                if let Some(sess_arc) = &sess {
                    if let Some(items) = &restored_items {
                        let turn_context = sess_arc.make_turn_context();
                        let reconstructed = sess_arc.reconstruct_history_from_rollout(&turn_context, items);
                        {
                            let mut st = sess_arc.state.lock().unwrap();
                            st.history = ConversationHistory::new();
                            st.history.record_items(reconstructed.iter());
                        }
                        replay_history_items = Some(reconstructed);
                    }
                }

                // Gather history metadata for SessionConfiguredEvent.
                let (history_log_id, history_entry_count) =
                    crate::message_history::history_metadata(&config).await;

                // ack
                let sess_arc = sess.as_ref().expect("session initialized");
                let events = std::iter::once(sess_arc.make_event(
                    INITIAL_SUBMIT_ID,
                    EventMsg::SessionConfigured(SessionConfiguredEvent {
                        session_id,
                        model,
                        history_log_id,
                        history_entry_count,
                    }),
                ))
                .chain(mcp_connection_errors.into_iter().map(|message| {
                    sess_arc.make_event(&sub.id, EventMsg::Error(ErrorEvent { message }))
                }));
                for event in events {
                    if let Err(e) = tx_event.send(event).await {
                        error!("failed to send event: {e:?}");
                    }
                }
                // If we resumed from a rollout, replay the prior transcript into the UI.
                if replay_history_items.is_some()
                    || restored_history_snapshot.is_some()
                    || restored_items.is_some()
                {
                    let items = replay_history_items.clone().unwrap_or_default();
                    let history_snapshot_value = restored_history_snapshot
                        .as_ref()
                        .and_then(|snapshot| serde_json::to_value(snapshot).ok());
                    let event = sess_arc.make_event(
                        &sub.id,
                        EventMsg::ReplayHistory(crate::protocol::ReplayHistoryEvent {
                            items,
                            history_snapshot: history_snapshot_value,
                        }),
                    );
                    if let Err(e) = tx_event.send(event).await {
                        warn!("failed to send ReplayHistory event: {e}");
                    }
                }

                if let Some(notice) = resume_notice {
                    let event = sess_arc.make_event(
                        &sub.id,
                        EventMsg::BackgroundEvent(BackgroundEventEvent { message: notice }),
                    );
                    if let Err(e) = tx_event.send(event).await {
                        warn!("failed to send resume notice event: {e}");
                    }
                }

                if let Some(sess_arc) = &sess {
                    sess_arc.run_session_hooks(ProjectHookEvent::SessionStart).await;
                }

                // Initialize agent manager after SessionConfigured is sent
                if !agent_manager_initialized {
                    let mut manager = AGENT_MANAGER.write().await;
                    let (agent_tx, mut agent_rx) =
                        tokio::sync::mpsc::unbounded_channel::<AgentStatusUpdatePayload>();
                    manager.set_event_sender(agent_tx);
                    drop(manager);

                    let sess_for_agents = sess.as_ref().expect("session active").clone();
                    // Forward agent events to the main event channel
                    let tx_event_clone = tx_event.clone();
                    tokio::spawn(async move {
                        while let Some(payload) = agent_rx.recv().await {
                            let status_event = sess_for_agents.make_event(
                                "agent_status",
                                EventMsg::AgentStatusUpdate(AgentStatusUpdateEvent {
                                    agents: payload.agents.clone(),
                                    context: payload.context.clone(),
                                    task: payload.task.clone(),
                                }),
                            );
                            let _ = tx_event_clone.send(status_event).await;
                        }
                    });
                    agent_manager_initialized = true;
                }
            }
            Op::UserInput { items } => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess,
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };

                // Clean up old status items when new user input arrives
                // This prevents token buildup from old screenshots/status messages
                sess.cleanup_old_status_items().await;

                // Abort synchronously here to avoid a race that can kill the
                // newly spawned agent if the async abort runs after set_task.
                sess.notify_wait_interrupted(WaitInterruptReason::UserMessage);
                sess.abort();

                // Spawn a new agent for this user input.
                let turn_context = sess.make_turn_context();
                let agent = AgentTask::spawn(Arc::clone(&sess), turn_context, sub.id.clone(), items);
                sess.set_task(agent);
            }
            Op::QueueUserInput { items } => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess,
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };

                if sess.has_running_task() {
                    let mut response_item = response_input_from_core_items(items.clone());
                    sess.enforce_user_message_limits(&sub.id, &mut response_item);
                    sess.notify_wait_interrupted(WaitInterruptReason::UserMessage);
                    let queued = QueuedUserInput {
                        submission_id: sub.id.clone(),
                        response_item,
                        core_items: items,
                    };
                    sess.queue_user_input(queued);
                } else {
                    // No task running: treat this as immediate user input without aborting.
                    sess.cleanup_old_status_items().await;
                    let turn_context = sess.make_turn_context();
                    let agent = AgentTask::spawn(Arc::clone(&sess), turn_context, sub.id.clone(), items);
                    sess.set_task(agent);
                }
            }
            Op::ExecApproval { id, decision } => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess,
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };
                match decision {
                    ReviewDecision::Abort => {
                        sess.notify_wait_interrupted(WaitInterruptReason::SessionAborted);
                        sess.abort();
                    }
                    other => sess.notify_approval(&id, other),
                }
            }
            Op::RegisterApprovedCommand {
                command,
                match_kind,
                semantic_prefix,
            } => {
                if command.is_empty() {
                    continue;
                }
                if let Some(sess) = sess.as_ref() {
                    sess.add_approved_command(ApprovedCommandPattern::new(
                        command,
                        match_kind,
                        semantic_prefix,
                    ));
                } else {
                    send_no_session_event(sub.id).await;
                }
            }
            Op::PatchApproval { id, decision } => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess,
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };
                match decision {
                    ReviewDecision::Abort => {
                        sess.notify_wait_interrupted(WaitInterruptReason::SessionAborted);
                        sess.abort();
                    }
                    other => sess.notify_approval(&id, other),
                }
            }
            Op::UpdateValidationTool { name, enable } => {
                if let Some(sess) = sess.as_ref() {
                    sess.update_validation_tool(&name, enable);
                } else {
                    send_no_session_event(sub.id).await;
                }
            }
            Op::UpdateValidationGroup { group, enable } => {
                if let Some(sess) = sess.as_ref() {
                    sess.update_validation_group(group, enable);
                } else {
                    send_no_session_event(sub.id).await;
                }
            }
            Op::AddToHistory { text } => {
                // TODO: What should we do if we got AddToHistory before ConfigureSession?
                // currently, if ConfigureSession has resume path, this history will be ignored
                let id = session_id;
                let config = config.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::message_history::append_entry(&text, &id, &config).await
                    {
                        warn!("failed to append to message history: {e}");
                    }
                });
            }

            Op::PersistHistorySnapshot { snapshot } => {
                let Some(sess) = sess.as_ref() else {
                    send_no_session_event(sub.id).await;
                    continue;
                };
                if let Some(recorder) = sess.clone_rollout_recorder() {
                    tokio::spawn(async move {
                        if let Err(e) = recorder.set_history_snapshot(snapshot).await {
                            warn!("failed to persist history snapshot: {e}");
                        }
                    });
                }
            }

            Op::RunProjectCommand { name } => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess,
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };
                let mut tracker = TurnDiffTracker::new();
                let attempt_req = sess.current_request_ordinal();
                sess.run_project_command(&mut tracker, &sub.id, &name, attempt_req)
                    .await;
            }

            Op::GetHistoryEntryRequest { offset, log_id } => {
                let config = config.clone();
                let tx_event = tx_event.clone();
                let sub_id = sub.id.clone();

                tokio::spawn(async move {
                    // Run lookup in blocking thread because it does file IO + locking.
                    let entry_opt = tokio::task::spawn_blocking(move || {
                        crate::message_history::lookup(log_id, offset, &config)
                    })
                    .await
                    .unwrap_or(None);

                    let event = Event {
                        id: sub_id,
                        event_seq: 0,
                        msg: EventMsg::GetHistoryEntryResponse(
                            crate::protocol::GetHistoryEntryResponseEvent {
                                offset,
                                log_id,
                                entry: entry_opt,
                            },
                        ),
                        order: None,
                    };

                    if let Err(e) = tx_event.send(event).await {
                        warn!("failed to send GetHistoryEntryResponse event: {e}");
                    }
                });
            }
            // Upstream protocol no longer includes ListMcpTools; skip handling here.
            Op::Compact => {
                let sess = match sess.as_ref() {
                    Some(sess) => sess,
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };

                // Attempt to inject input into current task
                if let Err(items) = sess.inject_input(vec![InputItem::Text {
                    text: compact::SUMMARIZATION_PROMPT.to_string(),
                }]) {
                    let turn_context = sess.make_turn_context();
                    compact::spawn_compact_task(sess.clone(), turn_context, sub.id.clone(), items);
                } else {
                    let was_empty = sess.enqueue_manual_compact(sub.id.clone());
                    let message = if was_empty {
                        "Manual compact queued; it will run after the current response finishes.".to_string()
                    } else {
                        "Manual compact already queued; waiting for the current response to finish.".to_string()
                    };
                    let event = sess.make_event(
                        &sub.id,
                        EventMsg::AgentMessage(AgentMessageEvent { message }),
                    );
                    sess.send_event(event).await;
                }
            }
            Op::Review { review_request } => {
                let sess = match sess.as_ref() {
                    Some(sess) => Arc::clone(sess),
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };
                let config = Arc::clone(&config);
                let sub_id = sub.id.clone();
                spawn_review_thread(sess, config, sub_id, review_request).await;
            }
            Op::SetNextTextFormat { format } => {
                let sess_arc = match sess.as_ref() {
                    Some(sess) => Arc::clone(sess),
                    None => {
                        send_no_session_event(sub.id).await;
                        continue;
                    }
                };
                *sess_arc.next_turn_text_format.lock().unwrap() = Some(format);
            }
            Op::Shutdown => {
                info!("Shutting down Codex instance");

                // Ensure any running agent is aborted so streaming stops promptly.
                if let Some(sess_arc) = sess.as_ref() {
                    let s2 = sess_arc.clone();
                    tokio::spawn(async move {
                        s2.notify_wait_interrupted(WaitInterruptReason::SessionAborted);
                        s2.abort();
                    });
                }

                // Gracefully flush and shutdown rollout recorder on session end so tests
                // that inspect the rollout file do not race with the background writer.
                if let Some(ref sess_arc) = sess {
                    let recorder_opt = sess_arc.rollout.lock().unwrap().take();
                    if let Some(rec) = recorder_opt {
                        if let Err(e) = rec.shutdown().await {
                            warn!("failed to shutdown rollout recorder: {e}");
                            let event = sess_arc.make_event(
                                &sub.id,
                                EventMsg::Error(ErrorEvent {
                                    message: "Failed to shutdown rollout recorder".to_string(),
                                }),
                            );
                            if let Err(e) = tx_event.send(event).await {
                                warn!("failed to send error message: {e:?}");
                            }
                        }
                    }
                }
                if let Some(ref sess_arc) = sess {
                    sess_arc.run_session_hooks(ProjectHookEvent::SessionEnd).await;
                }
                let event = match sess {
                    Some(ref sess_arc) => sess_arc.make_event(&sub.id, EventMsg::ShutdownComplete),
                    None => Event {
                        id: sub.id.clone(),
                        event_seq: 0,
                        msg: EventMsg::ShutdownComplete,
                        order: None,
                    },
                };
                if let Err(e) = tx_event.send(event).await {
                    warn!("failed to send Shutdown event: {e}");
                }
                break;
            }
        }
    }
    debug!("Agent loop exited");
}

async fn spawn_review_thread(
    sess: Arc<Session>,
    config: Arc<Config>,
    sub_id: String,
    review_request: ReviewRequest,
) {
    // Ensure any running task is stopped before starting the review flow.
    sess.notify_wait_interrupted(WaitInterruptReason::SessionAborted);
    sess.abort();

    let parent_turn_context = sess.make_turn_context();

    // Determine model + family for review mode.
    let review_model = config.review_model.clone();
    let review_family = find_family_for_model(&review_model)
        .unwrap_or_else(|| derive_default_model_family(&review_model));

    // Prepare a per-review configuration that favors deterministic feedback.
    let mut review_config = (*config).clone();
    review_config.model = review_model.clone();
    review_config.model_family = review_family.clone();
    review_config.model_reasoning_effort = ReasoningEffortConfig::Low;
    review_config.model_reasoning_summary = ReasoningSummaryConfig::Detailed;
    review_config.model_text_verbosity = config.model_text_verbosity;
    review_config.user_instructions = None;
    review_config.base_instructions = Some(REVIEW_PROMPT.to_string());
    if let Some(info) = get_model_info(&review_family) {
        review_config.model_context_window = Some(info.context_window);
        review_config.model_max_output_tokens = Some(info.max_output_tokens);
    }
    let review_config = Arc::new(review_config);

    let review_debug_logger = match crate::debug_logger::DebugLogger::new(review_config.debug) {
        Ok(logger) => Arc::new(Mutex::new(logger)),
        Err(err) => {
            warn!("failed to create review debug logger: {err}");
            Arc::new(Mutex::new(
                crate::debug_logger::DebugLogger::new(false).unwrap(),
            ))
        }
    };

    let review_otel = parent_turn_context
        .client
        .get_otel_event_manager()
        .map(|mgr| mgr.with_model(review_config.model.as_str(), review_config.model_family.slug.as_str()));

    let review_client = ModelClient::new(
        review_config.clone(),
        parent_turn_context.client.get_auth_manager(),
        review_otel,
        parent_turn_context.client.get_provider(),
        review_config.model_reasoning_effort,
        review_config.model_reasoning_summary,
        review_config.model_text_verbosity,
        sess.session_uuid(),
        review_debug_logger,
    );

    let review_turn_context = Arc::new(TurnContext {
        client: review_client,
        cwd: parent_turn_context.cwd.clone(),
        base_instructions: Some(REVIEW_PROMPT.to_string()),
        user_instructions: None,
        approval_policy: parent_turn_context.approval_policy,
        sandbox_policy: parent_turn_context.sandbox_policy.clone(),
        shell_environment_policy: parent_turn_context.shell_environment_policy.clone(),
        is_review_mode: true,
        text_format_override: None,
    });

    let review_prompt_text = format!(
        "{}\n\n---\n\nNow, here's your task: {}",
        REVIEW_PROMPT.trim(),
        review_request.prompt.trim()
    );
    let review_input = vec![InputItem::Text {
        text: review_prompt_text,
    }];

    let task = AgentTask::review(Arc::clone(&sess), Arc::clone(&review_turn_context), sub_id.clone(), review_input);
    sess.set_active_review(review_request.clone());
    sess.set_task(task);

    let event = sess.make_event(
        &sub_id,
        EventMsg::EnteredReviewMode(review_request.clone()),
    );
    sess.send_event(event).await;
}

async fn exit_review_mode(
    session: Arc<Session>,
    task_sub_id: String,
    review_output: Option<ReviewOutputEvent>,
) {
    let event = session.make_event(&task_sub_id, EventMsg::ExitedReviewMode(review_output.clone()));
    session.send_event(event).await;

    let _active_request = session.take_active_review();

    let developer_text = match review_output.clone() {
        Some(output) => {
            let mut sections: Vec<String> = Vec::new();
            if !output.overall_explanation.trim().is_empty() {
                sections.push(output.overall_explanation.trim().to_string());
            }
            if !output.findings.is_empty() {
                sections.push(format_review_findings_block(&output.findings, None));
            }
            if !output.overall_correctness.trim().is_empty() {
                sections.push(format!(
                    "Overall correctness: {}",
                    output.overall_correctness.trim()
                ));
            }
            if output.overall_confidence_score > 0.0 {
                sections.push(format!(
                    "Confidence score: {:.1}",
                    output.overall_confidence_score
                ));
            }

            let results = if sections.is_empty() {
                "Reviewer did not provide any findings.".to_string()
            } else {
                sections.join("\n\n")
            };

            format!(
                "<user_action>\n  <context>User initiated a review task. Here's the full review output from reviewer model. User may select one or more comments to resolve.</context>\n  <action>review</action>\n  <results>\n  {}\n  </results>\n</user_action>\n",
                results
            )
        }
        None => {
            "<user_action>\n  <context>User initiated a review task, but it ended without a final response. If the user asks about this, tell them to re-initiate a review with `/review` and wait for it to complete.</context>\n  <action>review</action>\n  <results>\n  None.\n  </results>\n</user_action>\n"
                .to_string()
        }
    };

    let developer_message = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text: developer_text.clone() }],
    };

    session
        .record_conversation_items(&[developer_message])
        .await;
}

fn parse_review_output_event(text: &str) -> ReviewOutputEvent {
    if let Ok(parsed) = serde_json::from_str::<ReviewOutputEvent>(text) {
        return parsed;
    }

    // Attempt to extract JSON from fenced code blocks if present.
    if let Some(idx) = text.find("```json") {
        if let Some(end_idx) = text[idx + 7..].find("```") {
            let json_slice = &text[idx + 7..idx + 7 + end_idx];
            if let Ok(parsed) = serde_json::from_str::<ReviewOutputEvent>(json_slice) {
                return parsed;
            }
        }
    }

    ReviewOutputEvent {
        findings: Vec::new(),
        overall_correctness: String::new(),
        overall_explanation: text.trim().to_string(),
        overall_confidence_score: 0.0,
    }
}

// Intentionally omit upstream review thread spawning; our fork handles review flows differently.
/// Takes a user message as input and runs a loop where, at each turn, the model
/// replies with either:
///
/// - requested function calls
/// - an assistant message
///
/// While it is possible for the model to return multiple of these items in a
/// single turn, in practice, we generally one item per turn:
///
/// - If the model requests a function call, we execute it and send the output
///   back to the model in the next turn.
/// - If the model sends only an assistant message, we record it in the
///   conversation history and consider the agent complete.
async fn run_agent(sess: Arc<Session>, turn_context: Arc<TurnContext>, sub_id: String, input: Vec<InputItem>) {
    if input.is_empty() {
        return;
    }
    let event = sess.make_event(&sub_id, EventMsg::TaskStarted);
    if sess.tx_event.send(event).await.is_err() {
        return;
    }
    // Continue with our fork's history and input handling.

    let is_review_mode = turn_context.is_review_mode;
    let mut review_history: Vec<ResponseItem> = Vec::new();
    let mut review_messages: Vec<String> = Vec::new();
    let mut review_exit_emitted = false;

    let pending_only_turn = input.len() == 1
        && matches!(
            &input[0],
            InputItem::Text { text } if text == PENDING_ONLY_SENTINEL
        );

    // Debug logging for ephemeral images
    let ephemeral_count = input
        .iter()
        .filter(|item| matches!(item, InputItem::EphemeralImage { .. }))
        .count();

    if ephemeral_count > 0 {
        tracing::info!(
            "Processing {} ephemeral images in user input",
            ephemeral_count
        );
    }

    let mut initial_response_item: Option<ResponseItem> = None;

    if !pending_only_turn {
        // Convert input to ResponseInputItem
        let mut response_input = response_input_from_core_items(input.clone());
        sess.enforce_user_message_limits(&sub_id, &mut response_input);
        let response_item: ResponseItem = response_input.into();

        if is_review_mode {
            review_history.push(response_item.clone());
        } else {
            // Record to history but we'll handle ephemeral images separately
            sess.record_conversation_items(&[response_item.clone()])
                .await;
        }
        initial_response_item = Some(response_item);
    }

    let mut last_task_message: Option<String> = None;
    // Although from the perspective of codex.rs, TurnDiffTracker has the lifecycle of a Agent which contains
    // many turns, from the perspective of the user, it is a single turn.
    let mut turn_diff_tracker = TurnDiffTracker::new();

    // Track if this is the first iteration - if so, include the initial input
    let mut first_iteration = true;

    loop {
        // Note that pending_input would be something like a message the user
        // submitted through the UI while the model was running. Though the UI
        // may support this, the model might not.
        // IMPORTANT: Do not inject queued user inputs into the review thread.
        // Doing so routes user messages (e.g., auto-resolve fix prompts) to the
        // review model, causing loops. Only include queued user inputs when not in
        // review mode. They will be picked up after TaskComplete via
        // pop_next_queued_user_input.
        let pending_input = if is_review_mode {
            sess.get_pending_input_filtered(false)
        } else {
            sess.get_pending_input()
        }
        .into_iter()
        .map(ResponseItem::from)
        .collect::<Vec<ResponseItem>>();
        let mut pending_input_tail = pending_input.clone();

        if initial_response_item.is_none() {
            if let Some(first_pending) = pending_input_tail.first().cloned() {
                pending_input_tail.remove(0);
                if is_review_mode {
                    review_history.push(first_pending.clone());
                } else {
                    sess.record_conversation_items(&[first_pending.clone()])
                        .await;
                }
                initial_response_item = Some(first_pending);
            } else {
                tracing::warn!(
                    "pending-only turn had no queued input; skipping model invocation"
                );
                break;
            }
        }

        // Do not duplicate the initial input in `pending_input`.
        // It is already recorded to history above; ephemeral items are appended separately.
        if first_iteration {
            first_iteration = false;
        } else {
            // Only record pending input to history on subsequent iterations
            sess.record_conversation_items(&pending_input).await;
        }

        // Construct the input that we will send to the model. When using the
        // Chat completions API (or ZDR clients), the model needs the full
        // conversation history on each turn. The rollout file, however, should
        // only record the new items that originated in this turn so that it
        // represents an append-only log without duplicates.
        let turn_input: Vec<ResponseItem> = if is_review_mode {
            if !pending_input_tail.is_empty() {
                review_history.extend(pending_input_tail.clone());
            }
            review_history.clone()
        } else {
            sess.turn_input_with_history(pending_input_tail.clone())
        };

        let turn_input_messages: Vec<String> = turn_input
            .iter()
            .filter_map(|item| match item {
                ResponseItem::Message { content, .. } => Some(content),
                _ => None,
            })
            .flat_map(|content| {
                content.iter().filter_map(|item| match item {
                    ContentItem::OutputText { text } => Some(text.clone()),
                    _ => None,
                })
            })
            .collect();
        match run_turn(
            &sess,
            &turn_context,
            &mut turn_diff_tracker,
            sub_id.clone(),
            initial_response_item.clone(),
            pending_input_tail,
            turn_input,
        )
        .await
        {
            Ok(turn_output) => {
                let mut items_to_record_in_conversation_history = Vec::<ResponseItem>::new();
                let mut responses = Vec::<ResponseInputItem>::new();
                for processed_response_item in turn_output {
                    let ProcessedResponseItem { item, response } = processed_response_item;
                    match (&item, &response) {
                        (ResponseItem::Message { role, .. }, None) if role == "assistant" => {
                            // If the model returned a message, we need to record it.
                            items_to_record_in_conversation_history.push(item.clone());
                            if is_review_mode {
                                if let ResponseItem::Message { content, .. } = &item {
                                    for ci in content {
                                        if let ContentItem::OutputText { text } = ci {
                                            review_messages.push(text.clone());
                                        }
                                    }
                                }
                            }
                        }
                        (
                            ResponseItem::LocalShellCall { .. },
                            Some(ResponseInputItem::FunctionCallOutput { call_id, output }),
                        ) => {
                            items_to_record_in_conversation_history.push(item.clone());
                            items_to_record_in_conversation_history.push(
                                ResponseItem::FunctionCallOutput {
                                    call_id: call_id.clone(),
                                    output: output.clone(),
                                },
                            );
                        }
                        (
                            ResponseItem::FunctionCall { .. },
                            Some(ResponseInputItem::FunctionCallOutput { call_id, output }),
                        ) => {
                            debug!(
                                "Recording function call and output for call_id: {}",
                                call_id
                            );
                            items_to_record_in_conversation_history.push(item.clone());
                            items_to_record_in_conversation_history.push(
                                ResponseItem::FunctionCallOutput {
                                    call_id: call_id.clone(),
                                    output: output.clone(),
                                },
                            );
                        }
                        (
                            ResponseItem::CustomToolCall { .. },
                            Some(ResponseInputItem::CustomToolCallOutput { call_id, output }),
                        ) => {
                            items_to_record_in_conversation_history.push(item.clone());
                            items_to_record_in_conversation_history.push(
                                ResponseItem::CustomToolCallOutput {
                                    call_id: call_id.clone(),
                                    output: output.clone(),
                                },
                            );
                        }
                        (
                            ResponseItem::FunctionCall { .. },
                            Some(ResponseInputItem::McpToolCallOutput { call_id, result }),
                        ) => {
                            items_to_record_in_conversation_history.push(item.clone());
                            let output =
                                convert_call_tool_result_to_function_call_output_payload(&result);
                            items_to_record_in_conversation_history.push(
                                ResponseItem::FunctionCallOutput {
                                    call_id: call_id.clone(),
                                    output,
                                },
                            );
                        }
                        (
                            ResponseItem::Reasoning {
                                id,
                                summary,
                                content,
                                encrypted_content,
                            },
                            None,
                        ) => {
                            items_to_record_in_conversation_history.push(ResponseItem::Reasoning {
                                id: id.clone(),
                                summary: summary.clone(),
                                content: content.clone(),
                                encrypted_content: encrypted_content.clone(),
                            });
                        }
                        _ => {
                            warn!("Unexpected response item: {item:?} with response: {response:?}");
                        }
                    };
                    if let Some(response) = response {
                        responses.push(response);
                    }
                }

                // Only attempt to take the lock if there is something to record.
                if !items_to_record_in_conversation_history.is_empty() {
                    if is_review_mode {
                        review_history.extend(items_to_record_in_conversation_history.clone());
                    } else {
                        // Record items in their original chronological order to maintain
                        // proper sequence of events. This ensures function calls and their
                        // outputs appear in the correct order in conversation history.
                        sess.record_conversation_items(&items_to_record_in_conversation_history)
                            .await;
                    }
                }

                // If there are responses, add them to pending input for the next iteration
                if !responses.is_empty() {
                    if !is_review_mode {
                        for response in &responses {
                            sess.add_pending_input(response.clone());
                        }
                    }
                }

                if responses.is_empty() {
                    debug!("Turn completed");
                    last_task_message = get_last_assistant_message_from_turn(
                        &items_to_record_in_conversation_history,
                    );
                    if let Some(m) = last_task_message.as_ref() {
                        tracing::info!("core.turn completed: last_assistant_message.len={}", m.len());
                    }
                    sess.maybe_notify(UserNotification::AgentTurnComplete {
                        turn_id: sub_id.clone(),
                        input_messages: turn_input_messages,
                        last_assistant_message: last_task_message.clone(),
                    });
                    break;
                }
            }
            Err(e) => {
                info!("Turn error: {e:#}");
                let event = sess.make_event(
                    &sub_id,
                    EventMsg::Error(ErrorEvent { message: e.to_string() }),
                );
                sess.tx_event.send(event).await.ok();
                if is_review_mode && !review_exit_emitted {
                    exit_review_mode(sess.clone(), sub_id.clone(), None).await;
                    review_exit_emitted = true;
                }
                // let the user continue the conversation
                break;
            }
        }
    }
    if is_review_mode && !review_exit_emitted {
        let combined = if !review_messages.is_empty() {
            review_messages.join("\n\n")
        } else {
            last_task_message.clone().unwrap_or_default()
        };
        let output = if combined.trim().is_empty() {
            None
        } else {
            Some(parse_review_output_event(&combined))
        };
        exit_review_mode(sess.clone(), sub_id.clone(), output).await;
    }

    sess.remove_task(&sub_id);
    let event = sess.make_event(
        &sub_id,
        EventMsg::TaskComplete(TaskCompleteEvent {
            last_agent_message: last_task_message,
        }),
    );
    match &event.msg {
        EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message: Some(m) }) => {
            tracing::info!("core.emit TaskComplete last_agent_message.len={}", m.len());
        }
        _ => {}
    }
    sess.tx_event.send(event).await.ok();

    if let Some(compact_sub_id) = sess.dequeue_manual_compact() {
        let turn_context = sess.make_turn_context();
        compact::spawn_compact_task(
            Arc::clone(&sess),
            turn_context,
            compact_sub_id,
            vec![InputItem::Text {
                text: compact::SUMMARIZATION_PROMPT.to_string(),
            }],
        );
        return;
    }

    if let Some(queued) = sess.pop_next_queued_user_input() {
        let sess_clone = Arc::clone(&sess);
        tokio::spawn(async move {
            sess_clone.cleanup_old_status_items().await;
            let turn_context = sess_clone.make_turn_context();
            let submission_id = queued.submission_id;
            let items = queued.core_items;
            let agent = AgentTask::spawn(Arc::clone(&sess_clone), turn_context, submission_id, items);
            sess_clone.set_task(agent);
        });
    }
}

async fn run_turn(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: String,
    initial_user_item: Option<ResponseItem>,
    pending_input_tail: Vec<ResponseItem>,
    mut input: Vec<ResponseItem>,
) -> CodexResult<Vec<ProcessedResponseItem>> {
    // Check if browser is enabled
    let browser_enabled = code_browser::global::get_browser_manager().await.is_some();

    let tc = &**turn_context;
    let agents_active = {
        let manager = AGENT_MANAGER.read().await;
        manager.has_active_agents()
    };

    let tools = get_openai_tools(
        &sess.tools_config,
        Some(sess.mcp_connection_manager.list_all_tools()),
        browser_enabled,
        agents_active,
    );

    let mut retries = 0;
    // Ensure we only auto-compact once per turn to avoid loops
    let mut did_auto_compact = false;
    // Attempt input starts as the provided input, and may be augmented with
    // items from a previous dropped stream attempt so we don't lose progress.
    let mut attempt_input: Vec<ResponseItem> = input.clone();
    loop {
        // Each loop iteration corresponds to a single provider HTTP request.
        // Increment the attempt ordinal first and capture its value so all
        // OrderMeta emitted during this attempt share the same `req`, even if
        // later attempts start before all events have been delivered.
        sess.begin_http_attempt();
        let attempt_req = sess.current_request_ordinal();
        // Build status items (screenshots, system status) fresh for each attempt
        let status_items = build_turn_status_items(sess).await;

        let prompt = Prompt {
            input: attempt_input.clone(),
            store: !sess.disable_response_storage,
            user_instructions: tc.user_instructions.clone(),
            environment_context: Some(EnvironmentContext::new(
                Some(tc.cwd.clone()),
                Some(tc.approval_policy),
                Some(tc.sandbox_policy.clone()),
                Some(sess.user_shell.clone()),
            )),
            tools: tools.clone(),
            status_items, // Include status items with this request
            base_instructions_override: tc.base_instructions.clone(),
            include_additional_instructions: true,
            text_format: tc.text_format_override.clone(),
            model_override: None,
            model_family_override: None,
            output_schema: None,
            log_tag: Some("codex/turn".to_string()),
            session_id_override: None,
        };

        // Start a new scratchpad for this HTTP attempt
        sess.begin_attempt_scratchpad();

        match try_run_turn(sess, turn_diff_tracker, &sub_id, &prompt, attempt_req).await {
            Ok(output) => {
                // Record status items to conversation history after successful turn
                // This ensures they persist for future requests in the right chronological order
                if !prompt.status_items.is_empty() {
                    sess.record_conversation_items(&prompt.status_items).await;
                }
                // Commit successful attempt – scratchpad is no longer needed.
                sess.clear_scratchpad();
                return Ok(output);
            }
            Err(CodexErr::Interrupted) => return Err(CodexErr::Interrupted),
            Err(CodexErr::EnvVar(var)) => return Err(CodexErr::EnvVar(var)),
            Err(e @ (CodexErr::UsageLimitReached(_) | CodexErr::UsageNotIncluded)) => {
                if let CodexErr::UsageLimitReached(limit_err) = &e {
                    if let Some(ctx) = account_usage_context(&sess) {
                        let usage_home = ctx.code_home.clone();
                        let usage_account = ctx.account_id.clone();
                        let usage_plan = ctx.plan.clone();
                        let resets = limit_err.resets_in_seconds;
                        spawn_usage_task(move || {
                            if let Err(err) = account_usage::record_usage_limit_hint(
                                &usage_home,
                                &usage_account,
                                usage_plan.as_deref(),
                                resets,
                                Utc::now(),
                            ) {
                                warn!("Failed to persist usage limit hint: {err}");
                            }
                        });
                    }
                }
                return Err(e);
            }
            Err(e) => {
                // Detect context-window overflow and auto-run a compact summarization once
                if !did_auto_compact {
                    if let CodexErr::Stream(msg, _maybe_delay) = &e {
                        let lower = msg.to_ascii_lowercase();
                        let looks_like_context_overflow =
                            lower.contains("exceeds the context window")
                                || lower.contains("exceed the context window")
                                || lower.contains("context length exceeded")
                                || lower.contains("maximum context length")
                                || (lower.contains("context window")
                                    && (lower.contains("exceed")
                                        || lower.contains("exceeded")
                                        || lower.contains("full")
                                        || lower.contains("too long")));

                        if looks_like_context_overflow {
                            did_auto_compact = true;
                            sess
                                .notify_stream_error(
                                    &sub_id,
                                    "Model hit context-window limit; running /compact and retrying…"
                                        .to_string(),
                                )
                                .await;

                            let previous_input_snapshot = input.clone();
                            let compacted_history = compact::run_inline_auto_compact_task(
                                Arc::clone(&sess),
                                Arc::clone(&turn_context),
                            )
                            .await;

                            // Reset any partial attempt state and rebuild the request payload using the
                            // newly compacted history plus the current user turn items.
                            sess.clear_scratchpad();

                            if compacted_history.is_empty() {
                                attempt_input = input.clone();
                            } else {
                                let mut rebuilt = compacted_history;
                                if let Some(initial_item) = initial_user_item.clone() {
                                    rebuilt.push(initial_item);
                                }
                                if !pending_input_tail.is_empty() {
                                    let (missing_calls, filtered_outputs) =
                                        reconcile_pending_tool_outputs(&pending_input_tail, &rebuilt, &previous_input_snapshot);
                                    if !missing_calls.is_empty() {
                                        rebuilt.extend(missing_calls);
                                    }
                                    if !filtered_outputs.is_empty() {
                                        rebuilt.extend(filtered_outputs);
                                    }
                                }
                                input = rebuilt.clone();
                                attempt_input = rebuilt;
                            }
                            continue;
                        }
                    }
                }

                // Use the configured provider-specific stream retry budget.
                let max_retries = tc.client.get_provider().stream_max_retries();
                if retries < max_retries {
                    retries += 1;
                    let delay = match e {
                        CodexErr::Stream(_, Some(delay)) => delay,
                        _ => backoff(retries),
                    };
                    warn!(
                        "stream disconnected - retrying turn ({retries}/{max_retries} in {delay:?})...",
                    );

                    // Surface retry information to any UI/front‑end so the
                    // user understands what is happening instead of staring
                    // at a seemingly frozen screen.
                    sess.notify_stream_error(
                        &sub_id,
                        format!(
                            "stream error: {e}; retrying {retries}/{max_retries} in {delay:?}…"
                        ),
                    )
                    .await;
                    // Pull any partial progress from this attempt and append to
                    // the next request's input so we do not lose tool progress
                    // or already-finalized items.
                    if let Some(sp) = sess.take_scratchpad() {
                        // Build a set of call_ids we have already included to avoid duplicate calls
                        let mut seen_calls: std::collections::HashSet<String> = attempt_input
                            .iter()
                            .filter_map(|ri| match ri {
                                ResponseItem::FunctionCall { call_id, .. } => Some(call_id.clone()),
                                ResponseItem::LocalShellCall { call_id: Some(c), .. } => Some(c.clone()),
                                _ => None,
                            })
                            .collect();

                        // Append finalized function/local shell calls from the dropped attempt
                        for item in sp.items {
                            match &item {
                                ResponseItem::FunctionCall { call_id, .. } => {
                                    if seen_calls.insert(call_id.clone()) {
                                        attempt_input.push(item.clone());
                                    }
                                }
                                ResponseItem::LocalShellCall { call_id: Some(c), .. } => {
                                    if seen_calls.insert(c.clone()) {
                                        attempt_input.push(item.clone());
                                    }
                                }
                                _ => {
                                    // Avoid injecting assistant/Reasoning messages on retry to reduce duplication.
                                }
                            }
                        }

                        // Append tool outputs produced during the dropped attempt
                        for resp in sp.responses {
                            attempt_input.push(ResponseItem::from(resp));
                        }

                        // If we have partial deltas, include a short ephemeral hint so the model can resume.
                        if !sp.partial_assistant_text.is_empty() || !sp.partial_reasoning_summary.is_empty() {
                            use code_protocol::models::ContentItem;
                            let mut hint = String::from(
                                "[EPHEMERAL:RETRY_HINT]\nPrevious attempt aborted mid-stream. Continue without repeating.\n",
                            );
                            if !sp.partial_reasoning_summary.is_empty() {
                                let s = &sp.partial_reasoning_summary;
                                // Take the last 800 characters, respecting UTF-8 boundaries
                                let start_idx = if s.chars().count() > 800 {
                                    s.char_indices()
                                        .rev()
                                        .nth(800 - 1)
                                        .map(|(i, _)| i)
                                        .unwrap_or(0)
                                } else {
                                    0
                                };
                                let tail = &s[start_idx..];
                                hint.push_str(&format!("Last reasoning summary fragment:\n{}\n\n", tail));
                            }
                            if !sp.partial_assistant_text.is_empty() {
                                let s = &sp.partial_assistant_text;
                                // Take the last 800 characters, respecting UTF-8 boundaries
                                let start_idx = if s.chars().count() > 800 {
                                    s.char_indices()
                                        .rev()
                                        .nth(800 - 1)
                                        .map(|(i, _)| i)
                                        .unwrap_or(0)
                                } else {
                                    0
                                };
                                let tail = &s[start_idx..];
                                hint.push_str(&format!("Last assistant text fragment:\n{}\n", tail));
                            }
                            attempt_input.push(ResponseItem::Message {
                                id: None,
                                role: "user".to_string(),
                                content: vec![ContentItem::InputText { text: hint }],
                            });
                        }
                    }

                    tokio::time::sleep(delay).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

fn reconcile_pending_tool_outputs(
    pending_outputs: &[ResponseItem],
    rebuilt_history: &[ResponseItem],
    previous_input_snapshot: &[ResponseItem],
) -> (Vec<ResponseItem>, Vec<ResponseItem>) {
    let mut call_ids = collect_tool_call_ids(rebuilt_history);
    let mut missing_calls = Vec::new();
    let mut filtered_outputs = Vec::new();

    for item in pending_outputs {
        match item {
            ResponseItem::FunctionCallOutput { call_id, .. }
            | ResponseItem::CustomToolCallOutput { call_id, .. } => {
                if call_ids.contains(call_id) {
                    filtered_outputs.push(item.clone());
                    continue;
                }

                if let Some(call_item) = find_call_item_by_id(previous_input_snapshot, call_id) {
                    call_ids.insert(call_id.clone());
                    missing_calls.push(call_item);
                    filtered_outputs.push(item.clone());
                } else {
                    warn!("Skipping tool output for missing call_id={call_id} after auto-compact");
                }
            }
            _ => {
                filtered_outputs.push(item.clone());
            }
        }
    }

    (missing_calls, filtered_outputs)
}

fn collect_tool_call_ids(items: &[ResponseItem]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for item in items {
        match item {
            ResponseItem::FunctionCall { call_id, .. } => {
                ids.insert(call_id.clone());
            }
            ResponseItem::LocalShellCall { call_id: Some(call_id), .. } => {
                ids.insert(call_id.clone());
            }
            ResponseItem::CustomToolCall { call_id, .. } => {
                ids.insert(call_id.clone());
            }
            _ => {}
        }
    }
    ids
}

fn find_call_item_by_id(items: &[ResponseItem], call_id: &str) -> Option<ResponseItem> {
    items.iter().rev().find_map(|item| match item {
        ResponseItem::FunctionCall { call_id: existing, .. } if existing == call_id => Some(item.clone()),
        ResponseItem::LocalShellCall { call_id: Some(existing), .. } if existing == call_id => Some(item.clone()),
        ResponseItem::CustomToolCall { call_id: existing, .. } if existing == call_id => Some(item.clone()),
        _ => None,
    })
}

/// When the model is prompted, it returns a stream of events. Some of these
/// events map to a `ResponseItem`. A `ResponseItem` may need to be
/// "handled" such that it produces a `ResponseInputItem` that needs to be
/// sent back to the model on the next turn.
#[derive(Debug)]
struct ProcessedResponseItem {
    item: ResponseItem,
    response: Option<ResponseInputItem>,
}

async fn try_run_turn(
    sess: &Session,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: &str,
    prompt: &Prompt,
    attempt_req: u64,
) -> CodexResult<Vec<ProcessedResponseItem>> {
    // call_ids that are part of this response.
    let completed_call_ids = prompt
        .input
        .iter()
        .filter_map(|ri| match ri {
            ResponseItem::FunctionCallOutput { call_id, .. } => Some(call_id),
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } => Some(call_id),
            ResponseItem::CustomToolCallOutput { call_id, .. } => Some(call_id),
            _ => None,
        })
        .collect::<Vec<_>>();

    // call_ids that were pending but are not part of this response.
    // This usually happens because the user interrupted the model before we responded to one of its tool calls
    // and then the user sent a follow-up message.
    let missing_calls = {
        prompt
            .input
            .iter()
            .filter_map(|ri| match ri {
                ResponseItem::FunctionCall { call_id, .. } => Some(call_id),
                ResponseItem::LocalShellCall {
                    call_id: Some(call_id),
                    ..
                } => Some(call_id),
                ResponseItem::CustomToolCall { call_id, .. } => Some(call_id),
                _ => None,
            })
            .filter_map(|call_id| {
                if completed_call_ids.contains(&call_id) {
                    None
                } else {
                    Some(call_id.clone())
                }
            })
            .map(|call_id| ResponseItem::CustomToolCallOutput {
                call_id: call_id.clone(),
                output: "aborted".to_string(),
            })
            .collect::<Vec<_>>()
    };
    let prompt: Cow<Prompt> = if missing_calls.is_empty() {
        Cow::Borrowed(prompt)
    } else {
        // Add the synthetic aborted missing calls to the beginning of the input to ensure all call ids have responses.
        let input = [missing_calls, prompt.input.clone()].concat();
        Cow::Owned(Prompt {
            input,
            ..prompt.clone()
        })
    };

    let mut stream = sess.client.clone().stream(&prompt).await?;

    let mut output = Vec::new();
    loop {
        // Poll the next item from the model stream. We must inspect *both* Ok and Err
        // cases so that transient stream failures (e.g., dropped SSE connection before
        // `response.completed`) bubble up and trigger the caller's retry logic.
        let event = stream.next().await;
        let Some(event) = event else {
            // Channel closed without yielding a final Completed event or explicit error.
            // Treat as a disconnected stream so the caller can retry.
            return Err(CodexErr::Stream(
                "stream closed before response.completed".into(),
                None,
            ));
        };

        let event = match event {
            Ok(ev) => ev,
            Err(e) => {
                // Propagate the underlying stream error to the caller (run_turn), which
                // will apply the configured `stream_max_retries` policy.
                return Err(e);
            }
        };

        match event {
            ResponseEvent::Created => {}
            ResponseEvent::OutputItemDone { item, sequence_number, output_index } => {
                let response =
                    handle_response_item(sess, turn_diff_tracker, sub_id, item.clone(), sequence_number, output_index, attempt_req).await?;

                // Save into scratchpad so we can seed a retry if the stream drops later.
                sess.scratchpad_push(&item, &response, &sub_id);

                // If this was a finalized assistant message, clear partial text buffer
                if let ResponseItem::Message { .. } = &item {
                    sess.scratchpad_clear_partial_message();
                }

                output.push(ProcessedResponseItem { item, response });
            }
            ResponseEvent::WebSearchCallBegin { call_id } => {
                // Stamp OrderMeta so the TUI can place the search block within
                // the correct request window instead of using an internal epilogue.
                let ctx = ToolCallCtx::new(sub_id.to_string(), call_id.clone(), None, None);
                let order = ctx.order_meta(attempt_req);
                let ev = sess.make_event_with_order(
                    &sub_id,
                    EventMsg::WebSearchBegin(WebSearchBeginEvent { call_id, query: None }),
                    order,
                    None,
                );
                sess.send_event(ev).await;
            }
            ResponseEvent::WebSearchCallCompleted { call_id, query } => {
                let ctx = ToolCallCtx::new(sub_id.to_string(), call_id.clone(), None, None);
                let order = ctx.order_meta(attempt_req);
                let ev = sess.make_event_with_order(
                    &sub_id,
                    EventMsg::WebSearchComplete(WebSearchCompleteEvent { call_id, query }),
                    order,
                    None,
                );
                sess.send_event(ev).await;
            }
            ResponseEvent::Completed {
                response_id: _,
                token_usage,
            } => {
                let (new_info, rate_limits, should_emit);
                {
                    let mut state = sess.state.lock().unwrap();
                    let info = TokenUsageInfo::new_or_append(
                        &state.token_usage_info,
                        &token_usage,
                        sess.client.get_model_context_window(),
                    );
                    let limits = state.latest_rate_limits.clone();
                    let emit = info.is_some() || limits.is_some();
                    state.token_usage_info = info.clone();
                    new_info = info;
                    rate_limits = limits;
                    should_emit = emit;
                }

                if should_emit {
                    let payload = TokenCountEvent {
                        info: new_info,
                        rate_limits,
                    };
                    sess.tx_event
                        .send(sess.make_event(&sub_id, EventMsg::TokenCount(payload)))
                        .await
                        .ok();
                }

                if let Some(usage) = token_usage.as_ref() {
                    if let Some(ctx) = account_usage_context(sess) {
                        let usage_home = ctx.code_home.clone();
                        let usage_account = ctx.account_id.clone();
                        let usage_plan = ctx.plan.clone();
                        let usage_clone = usage.clone();
                        spawn_usage_task(move || {
                            if let Err(err) = account_usage::record_token_usage(
                                &usage_home,
                                &usage_account,
                                usage_plan.as_deref(),
                                &usage_clone,
                                Utc::now(),
                            ) {
                                warn!("Failed to persist token usage: {err}");
                            }
                        });
                    }
                }

                let unified_diff = turn_diff_tracker.get_unified_diff();
                if let Ok(Some(unified_diff)) = unified_diff {
                    let msg = EventMsg::TurnDiff(TurnDiffEvent { unified_diff });
                    let _ = sess.tx_event.send(sess.make_event(&sub_id, msg)).await;
                }

                return Ok(output);
            }
            ResponseEvent::OutputTextDelta { delta, item_id, sequence_number, output_index } => {
                // Don't append to history during streaming - only send UI events.
                // The complete message will be added to history when OutputItemDone arrives.
                // This ensures items are recorded in the correct chronological order.

                // Use the item_id if present, otherwise fall back to sub_id
                let event_id = item_id.unwrap_or_else(|| sub_id.to_string());
                let order = crate::protocol::OrderMeta {
                    request_ordinal: attempt_req,
                    output_index,
                    sequence_number,
                };
                let stamped = sess.make_event_with_order(&event_id, EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta: delta.clone() }), order, sequence_number);
                sess.tx_event.send(stamped).await.ok();

                // Track partial assistant text in the scratchpad to help resume on retry.
                // Only accumulate when we have an item context or a single active stream.
                // We deliberately do not scope by item_id to keep implementation simple.
                sess.scratchpad_add_text_delta(&delta);
            }
            ResponseEvent::ReasoningSummaryDelta { delta, item_id, sequence_number, output_index, summary_index } => {
                // Use the item_id if present, otherwise fall back to sub_id
                let mut event_id = item_id.unwrap_or_else(|| sub_id.to_string());
                if let Some(si) = summary_index { event_id = format!("{}#s{}", event_id, si); }
                let order = crate::protocol::OrderMeta { request_ordinal: attempt_req, output_index, sequence_number };
                let stamped = sess.make_event_with_order(&event_id, EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta: delta.clone() }), order, sequence_number);
                sess.tx_event.send(stamped).await.ok();

                // Buffer reasoning summary so we can include a hint on retry.
                sess.scratchpad_add_reasoning_delta(&delta);
            }
            ResponseEvent::ReasoningSummaryPartAdded => {
                let stamped = sess.make_event(&sub_id, EventMsg::AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent {}));
                sess.tx_event.send(stamped).await.ok();
            }
            ResponseEvent::ReasoningContentDelta { delta, item_id, sequence_number, output_index, content_index } => {
                if sess.show_raw_agent_reasoning {
                    // Use the item_id if present, otherwise fall back to sub_id
                    let mut event_id = item_id.unwrap_or_else(|| sub_id.to_string());
                    if let Some(ci) = content_index { event_id = format!("{}#c{}", event_id, ci); }
                    let order = crate::protocol::OrderMeta { request_ordinal: attempt_req, output_index, sequence_number };
                    let stamped = sess.make_event_with_order(&event_id, EventMsg::AgentReasoningRawContentDelta(AgentReasoningRawContentDeltaEvent { delta }), order, sequence_number);
                    sess.tx_event.send(stamped).await.ok();
                }
            }
            ResponseEvent::RateLimits(snapshot) => {
                let mut state = sess.state.lock().unwrap();
                state.latest_rate_limits = Some(snapshot.clone());
                if let Some(ctx) = account_usage_context(sess) {
                    let usage_home = ctx.code_home.clone();
                    let usage_account = ctx.account_id.clone();
                    let usage_plan = ctx.plan.clone();
                    let snapshot_clone = snapshot.clone();
                    spawn_usage_task(move || {
                        if let Err(err) = account_usage::record_rate_limit_snapshot(
                            &usage_home,
                            &usage_account,
                            usage_plan.as_deref(),
                            &snapshot_clone,
                            Utc::now(),
                        ) {
                            warn!("Failed to persist rate limit snapshot: {err}");
                        }
                    });
                }
            }
            // Note: ReasoningSummaryPartAdded handled above without scratchpad mutation.
        }
    }
}

async fn handle_response_item(
    sess: &Session,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: &str,
    item: ResponseItem,
    seq_hint: Option<u64>,
    output_index: Option<u32>,
    attempt_req: u64,
) -> CodexResult<Option<ResponseInputItem>> {
    debug!(?item, "Output item");
    let output = match item {
        ResponseItem::Message { content, id, .. } => {
            // Use the item_id if present, otherwise fall back to sub_id
            let event_id = id.unwrap_or_else(|| sub_id.to_string());
            for item in content {
                if let ContentItem::OutputText { text } = item {
                    let order = crate::protocol::OrderMeta { request_ordinal: attempt_req, output_index, sequence_number: seq_hint };
                    let stamped = sess.make_event_with_order(&event_id, EventMsg::AgentMessage(AgentMessageEvent { message: text }), order, seq_hint);
                    sess.tx_event.send(stamped).await.ok();
                }
            }
            None
        }
        ResponseItem::Reasoning {
            id,
            summary,
            content,
            encrypted_content: _,
        } => {
            // Use the item_id if present and not empty, otherwise fall back to sub_id
            let event_id = if !id.is_empty() {
                id.clone()
            } else {
                sub_id.to_string()
            };
            for (i, item) in summary.into_iter().enumerate() {
                let text = match item {
                    ReasoningItemReasoningSummary::SummaryText { text } => text,
                };
                let eid = format!("{}#s{}", event_id, i);
                let order = crate::protocol::OrderMeta { request_ordinal: attempt_req, output_index, sequence_number: seq_hint };
                let stamped = sess.make_event_with_order(&eid, EventMsg::AgentReasoning(AgentReasoningEvent { text }), order, seq_hint);
                sess.tx_event.send(stamped).await.ok();
            }
            if sess.show_raw_agent_reasoning && content.is_some() {
                let content = content.unwrap();
                for item in content.into_iter() {
                    let text = match item {
                        ReasoningItemContent::ReasoningText { text } => text,
                        ReasoningItemContent::Text { text } => text,
                    };
                    let order = crate::protocol::OrderMeta { request_ordinal: attempt_req, output_index, sequence_number: seq_hint };
                    let stamped = sess.make_event_with_order(&event_id, EventMsg::AgentReasoningRawContent(AgentReasoningRawContentEvent { text }), order, seq_hint);
                    sess.tx_event.send(stamped).await.ok();
                }
            }
            None
        }
        ResponseItem::FunctionCall {
            name,
            arguments,
            call_id,
            ..
        } => {
            info!("FunctionCall: {name}({arguments})");
            Some(
                handle_function_call(
                    sess,
                    turn_diff_tracker,
                    sub_id.to_string(),
                    name,
                    arguments,
                    call_id,
                    seq_hint,
                    output_index,
                    attempt_req,
                )
                .await,
            )
        }
        ResponseItem::LocalShellCall {
            id,
            call_id,
            status: _,
            action,
        } => {
            let LocalShellAction::Exec(action) = action;
            tracing::info!("LocalShellCall: {action:?}");
            let params = ShellToolCallParams {
                command: action.command,
                workdir: action.working_directory,
                timeout_ms: action.timeout_ms,
                with_escalated_permissions: None,
                justification: None,
            };
            let effective_call_id = match (call_id, id) {
                (Some(call_id), _) => call_id,
                (None, Some(id)) => id,
                (None, None) => {
                    error!("LocalShellCall without call_id or id");
                    return Ok(Some(ResponseInputItem::FunctionCallOutput {
                        call_id: "".to_string(),
                        output: FunctionCallOutputPayload {
                            content: "LocalShellCall without call_id or id".to_string(),
                            success: None,
                        },
                    }));
                }
            };

            let exec_params = to_exec_params(params, sess);
            Some(
            handle_container_exec_with_params(
                exec_params,
                sess,
                turn_diff_tracker,
                sub_id.to_string(),
                effective_call_id,
                seq_hint,
                output_index,
                attempt_req,
            )
            .await,
            )
        }
        ResponseItem::CustomToolCall { call_id, name, .. } => {
            // Minimal placeholder: custom tools are not handled here.
            Some(ResponseInputItem::FunctionCallOutput {
                call_id,
                output: FunctionCallOutputPayload {
                    content: format!("Custom tool '{name}' is not supported in this build"),
                    success: Some(false),
                },
            })
        }
        ResponseItem::FunctionCallOutput { .. } => {
            debug!("unexpected FunctionCallOutput from stream");
            None
        }
        ResponseItem::CustomToolCallOutput { .. } => {
            debug!("unexpected CustomToolCallOutput from stream");
            None
        }
        ResponseItem::WebSearchCall { id, action, .. } => {
            if let WebSearchAction::Search { query } = action {
                let call_id = id.unwrap_or_else(|| "".to_string());
                let event = sess.make_event_with_hint(&sub_id, EventMsg::WebSearchComplete(WebSearchCompleteEvent { call_id, query: Some(query) }), seq_hint);
                sess.tx_event.send(event).await.ok();
            }
            None
        }
        ResponseItem::Other => None,
    };
    Ok(output)
}

// Helper utilities for agent output/progress management
fn ensure_agent_dir(cwd: &Path, agent_id: &str) -> Result<PathBuf, String> {
    let dir = cwd.join(".code").join("agents").join(agent_id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create agent dir {}: {}", dir.display(), e))?;
    Ok(dir)
}

fn ensure_user_dir(cwd: &Path) -> Result<PathBuf, String> {
    let dir = cwd.join(".code").join("users");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create user dir {}: {}", dir.display(), e))?;
    Ok(dir)
}

fn write_agent_file(dir: &Path, filename: &str, content: &str) -> Result<PathBuf, String> {
    let path = dir.join(filename);
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
    Ok(path)
}

const AGENT_PREVIEW_MAX_BYTES: usize = 32 * 1024; // 32 KiB

fn preview_first_n_lines(s: &str, n: usize) -> (String, usize) {
    let total_lines = s.lines().count();
    let mut preview = s.lines().take(n).collect::<Vec<_>>().join("\n");

    let (maybe_truncated, was_truncated, _, _) =
        truncate_middle_bytes(&preview, AGENT_PREVIEW_MAX_BYTES);
    if was_truncated {
        preview = maybe_truncated;
        preview.push_str(&format!(
            "\n…preview truncated to roughly {AGENT_PREVIEW_MAX_BYTES} bytes…"
        ));
    } else {
        preview = maybe_truncated;
    }

    if total_lines > n {
        if !preview.ends_with('\n') {
            preview.push('\n');
        }
        preview.push_str("…additional lines omitted…");
    }

    (preview, total_lines)
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    #[test]
    fn truncates_excessively_long_single_line() {
        let input = "x".repeat(AGENT_PREVIEW_MAX_BYTES + 1024);
        let (preview, total_lines) = preview_first_n_lines(&input, 500);
        assert_eq!(total_lines, 1);
        assert!(preview.contains("…truncated…"));
        assert!(preview.contains("preview truncated to roughly"));
    }

    #[test]
    fn notes_when_additional_lines_omitted() {
        let input = (0..600)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (preview, total_lines) = preview_first_n_lines(&input, 500);
        assert_eq!(total_lines, 600);
        assert!(preview.contains("…additional lines omitted…"));
        assert!(!preview.contains("preview truncated to roughly"));
    }
}

async fn handle_function_call(
    sess: &Session,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: String,
    name: String,
    arguments: String,
    call_id: String,
    seq_hint: Option<u64>,
    output_index: Option<u32>,
    attempt_req: u64,
) -> ResponseInputItem {
    let ctx = ToolCallCtx::new(sub_id.clone(), call_id.clone(), seq_hint, output_index);
    match name.as_str() {
        "container.exec" | "shell" => {
            let params = match parse_container_exec_arguments(arguments, sess, &call_id) {
                Ok(params) => params,
                Err(output) => {
                    return *output;
                }
            };
            handle_container_exec_with_params(params, sess, turn_diff_tracker, sub_id, call_id, seq_hint, output_index, attempt_req)
                .await
        }
        "update_plan" => handle_update_plan(sess, &ctx, arguments).await,
        // agent tool
        "agent" => handle_agent_tool(sess, &ctx, arguments).await,
        // unified browser tool
        "browser" => handle_browser_tool(sess, &ctx, arguments).await,
        "web_fetch" => handle_web_fetch(sess, &ctx, arguments).await,
        "wait" => handle_wait(sess, &ctx, arguments).await,
        "kill" => handle_kill(sess, &ctx, arguments).await,
        _ => {
            match sess.mcp_connection_manager.parse_tool_name(&name) {
                Some((server, tool_name)) => {
                    // Tool timeouts are derived from per-server config; no per-call override here.
                    handle_mcp_tool_call(sess, &ctx, server, tool_name, arguments).await
                }
                None => {
                    // Unknown function: reply with structured failure so the model can adapt.
                    ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload {
                            content: format!("unsupported call: {name}"),
                            success: None,
                        },
                    }
                }
            }
        }
    }
}

async fn handle_browser_cleanup(sess: &Session, ctx: &ToolCallCtx) -> ResponseInputItem {
    let call_id_clone = ctx.call_id.clone();
    let _sess_clone = sess;
    execute_custom_tool(
        sess,
        ctx,
        "browser_cleanup".to_string(),
        Some(serde_json::json!({})),
        || async move {
            if let Some(browser_manager) = get_browser_manager_for_session(_sess_clone).await {
                match browser_manager.cleanup().await {
                    Ok(_) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload { content: "Browser cleanup completed".to_string(), success: Some(true) },
                    },
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload { content: format!("Cleanup failed: {}", e), success: Some(false) },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload { content: "Browser is not initialized. Use browser_open to start the browser.".to_string(), success: Some(false) },
                }
            }
        }
    ).await
}

async fn handle_web_fetch(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    // Include raw params in begin event for observability
    let mut params_for_event = serde_json::from_str::<serde_json::Value>(&arguments).ok();
    // If call_id is provided, include a friendly "for" string with the command we are waiting on
    if let Some(serde_json::Value::Object(map)) = params_for_event.as_mut() {
        if let Some(serde_json::Value::String(cid)) = map.get("call_id") {
            let st = sess.state.lock().unwrap();
            if let Some(bg) = st.background_execs.get(cid) {
                map.insert("for".to_string(), serde_json::Value::String(bg.cmd_display.clone()));
            }
        }
    }
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "web_fetch".to_string(),
        params_for_event,
        || async move {
            #[derive(serde::Deserialize)]
            struct WebFetchParams {
                url: String,
                #[serde(default)]
                timeout_ms: Option<u64>,
                #[serde(default)]
                mode: Option<String>, // "auto" (default), "browser", or "http"
            }

            let parsed: Result<WebFetchParams, _> = serde_json::from_str(&arguments_clone);
            let params = match parsed {
                Ok(p) => p,
                Err(e) => {
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: format!("Invalid web_fetch arguments: {e}"),
                            success: None,
                        },
                    };
                }
            };

            struct BrowserFetchOutcome {
                html: String,
                final_url: Option<String>,
                headless: bool,
            }

            async fn fetch_html_via_headless_browser(
                url: &str,
                timeout: Duration,
            ) -> Result<BrowserFetchOutcome, String> {
                let mut config = CodexBrowserConfig::default();
                config.enabled = true;
                config.headless = true;
                config.fullpage = false;
                config.segments_max = 2;
                config.persist_profile = false;
                config.idle_timeout_ms = 10_000;

                let manager = BrowserManager::new(config);
                manager.set_enabled_sync(true);

                const CHECK_JS: &str = r#"(function(){
  const discuss = document.querySelectorAll('[data-test-selector=\"issue-comment-body\"]');
  const timeline = document.querySelectorAll('.js-timeline-item');
  const article = document.querySelectorAll('article, main');
  return (discuss.length + timeline.length + article.length);
})()"#;
                const HTML_JS: &str =
                    "(function(){ return { html: document.documentElement.outerHTML, title: document.title||'' }; })()";

                let goto_result = match tokio::time::timeout(timeout, manager.goto(url)).await {
                    Ok(Ok(res)) => res,
                    Ok(Err(e)) => {
                        let _ = manager.stop().await;
                        return Err(format!("Headless goto failed: {e}"));
                    }
                    Err(_) => {
                        let _ = manager.stop().await;
                        return Err("Headless goto timed out".to_string());
                    }
                };

                for _ in 0..6 {
                    match tokio::time::timeout(Duration::from_millis(1500), manager.execute_javascript(CHECK_JS)).await {
                        Ok(Ok(val)) => {
                            let count = val
                                .get("value")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            if count > 0 {
                                break;
                            }
                        }
                        Ok(Err(e)) => {
                            tracing::debug!("Headless readiness check failed: {}", e);
                            break;
                        }
                        Err(_) => {
                            tracing::debug!("Headless readiness check timed out");
                            break;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(800)).await;
                }

                let html_value = match tokio::time::timeout(timeout, manager.execute_javascript(HTML_JS)).await {
                    Ok(Ok(val)) => val,
                    Ok(Err(e)) => {
                        let _ = manager.stop().await;
                        return Err(format!("Headless HTML extraction failed: {e}"));
                    }
                    Err(_) => {
                        let _ = manager.stop().await;
                        return Err("Headless HTML extraction timed out".to_string());
                    }
                };

                let html = html_value
                    .get("value")
                    .and_then(|v| v.get("html"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if html.trim().is_empty() {
                    let _ = manager.stop().await;
                    return Err("Headless browser returned empty HTML".to_string());
                }

                let final_url = Some(goto_result.url.clone());
                let _ = manager.stop().await;

                Ok(BrowserFetchOutcome {
                    html,
                    final_url,
                    headless: true,
                })
            }

            async fn fetch_html_via_browser(
                url: &str,
                timeout: Duration,
                prefer_global: bool,
            ) -> Option<BrowserFetchOutcome> {
                const HTML_JS: &str =
                    "(function(){ return { html: document.documentElement.outerHTML, title: document.title||'' }; })()";
                const CHECK_JS: &str = r#"(function(){
  const discuss = document.querySelectorAll('[data-test-selector=\"issue-comment-body\"]');
  const timeline = document.querySelectorAll('.js-timeline-item');
  const article = document.querySelectorAll('article, main');
  return (discuss.length + timeline.length + article.length);
})()"#;

                if prefer_global {
                    if let Some(manager) = code_browser::global::get_browser_manager().await {
                        if manager.is_enabled_sync() {
                            match tokio::time::timeout(timeout, manager.goto(url)).await {
                                Ok(Ok(res)) => {
                                    for _ in 0..6 {
                                        match tokio::time::timeout(Duration::from_millis(1500), manager.execute_javascript(CHECK_JS)).await {
                                            Ok(Ok(val)) => {
                                                let count = val
                                                    .get("value")
                                                    .and_then(|v| v.as_i64())
                                                    .unwrap_or(0);
                                                if count > 0 {
                                                    break;
                                                }
                                            }
                                            Ok(Err(e)) => {
                                                tracing::debug!("Global browser readiness check failed: {}", e);
                                                break;
                                            }
                                            Err(_) => {
                                                tracing::debug!("Global browser readiness timed out");
                                                break;
                                            }
                                        }
                                        tokio::time::sleep(Duration::from_millis(800)).await;
                                    }

                                    match tokio::time::timeout(timeout, manager.execute_javascript(HTML_JS)).await {
                                        Ok(Ok(val)) => {
                                            if let Some(html) = val
                                                .get("value")
                                                .and_then(|v| v.get("html"))
                                                .and_then(|v| v.as_str())
                                            {
                                                if !html.trim().is_empty() {
                                                    return Some(BrowserFetchOutcome {
                                                        html: html.to_string(),
                                                        final_url: Some(res.url.clone()),
                                                        headless: false,
                                                    });
                                                }
                                            }
                                        }
                                        Ok(Err(e)) => {
                                            tracing::debug!("Global browser HTML extraction failed: {}", e);
                                        }
                                        Err(_) => {
                                            tracing::debug!("Global browser HTML extraction timed out");
                                        }
                                    }
                                }
                                Ok(Err(e)) => {
                                    tracing::warn!("Global browser navigation failed: {}", e);
                                }
                                Err(_) => {
                                    tracing::warn!("Global browser navigation timed out");
                                }
                            }
                        } else {
                            tracing::debug!("Global browser manager disabled; skipping UI fetch");
                        }
                    }
                }

                match fetch_html_via_headless_browser(url, timeout).await {
                    Ok(outcome) => Some(outcome),
                    Err(err) => {
                        tracing::warn!("Headless browser fallback failed for {}: {}", url, err);
                        None
                    }
                }
            }

            // Helper: build a client with a specific UA and common headers.
            async fn do_request(
                url: &str,
                ua: &str,
                timeout: Duration,
                extra_headers: Option<&[(reqwest::header::HeaderName, &'static str)]>,
            ) -> Result<reqwest::Response, reqwest::Error> {
                let client = reqwest::Client::builder()
                    .timeout(timeout)
                    .user_agent(ua)
                    .build()?;
                let mut req = client.get(url)
                    // Add a few browser-like headers to reduce blocks
                    .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
                    .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9");
                if let Some(pairs) = extra_headers {
                    for (k, v) in pairs.iter() {
                        req = req.header(k, *v);
                    }
                }
                req.send().await
            }

            // Helper: remove obvious noisy blocks before markdown conversion.
            // This uses a lightweight ASCII-insensitive scan to drop whole
            // elements whose contents should never be surfaced to the model
            // (scripts, styles, templates, headers/footers/navigation, etc.).
            fn strip_noisy_tags(mut html: String) -> String {
                // Remove <script>, <style>, and <noscript> blocks with a simple
                // ASCII case-insensitive scan that preserves UTF-8 boundaries.
                // This avoids allocating lowercase copies and accidentally using
                // indices from a different string representation.
                fn eq_ascii_ci(a: u8, b: u8) -> bool {
                    a.to_ascii_lowercase() == b.to_ascii_lowercase()
                }
                fn starts_with_tag_ci(bytes: &[u8], tag: &[u8]) -> bool {
                    if bytes.len() < tag.len() { return false; }
                    for i in 0..tag.len() {
                        if !eq_ascii_ci(bytes[i], tag[i]) { return false; }
                    }
                    true
                }
                // Find the next opening tag like "<script" (allowing whitespace after '<').
                fn find_open_tag_ci(s: &str, tag: &str, from: usize) -> Option<usize> {
                    let bytes = s.as_bytes();
                    let tag_bytes = tag.as_bytes();
                    let mut i = from;
                    while i + 1 < bytes.len() {
                        if bytes[i] == b'<' {
                            let mut j = i + 1;
                            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t' || bytes[j] == b'\n' || bytes[j] == b'\r') {
                                j += 1;
                            }
                            if j < bytes.len() && starts_with_tag_ci(&bytes[j..], tag_bytes) {
                                return Some(i);
                            }
                        }
                        i += 1;
                    }
                    None
                }
                // Find the corresponding closing tag like "</script>" starting at or after `from`.
                // Returns the byte index just after the closing '>' if found.
                fn find_close_after_ci(s: &str, tag: &str, from: usize) -> Option<usize> {
                    let bytes = s.as_bytes();
                    let tag_bytes = tag.as_bytes();
                    let mut i = from;
                    while i + 2 < bytes.len() { // need at least '<' '/' and one tag byte
                        if bytes[i] == b'<' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                            let mut j = i + 2;
                            // Optional whitespace before tag name
                            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t' || bytes[j] == b'\n' || bytes[j] == b'\r') {
                                j += 1;
                            }
                            if starts_with_tag_ci(&bytes[j..], tag_bytes) {
                                // Advance past tag name
                                j += tag_bytes.len();
                                // Skip optional whitespace until '>'
                                while j < bytes.len() && bytes[j] != b'>' {
                                    j += 1;
                                }
                                if j < bytes.len() && bytes[j] == b'>' {
                                    return Some(j + 1);
                                }
                                return None; // No closing '>'
                            }
                        }
                        i += 1;
                    }
                    None
                }

                // Keep this conservative to avoid dropping content.
                let tags = ["script", "style", "noscript"];
                for tag in tags.iter() {
                    let mut guard = 0;
                    loop {
                        if guard > 64 { break; }
                        let Some(start) = find_open_tag_ci(&html, tag, 0) else { break; };
                        let search_from = start + 1; // after '<'
                        if let Some(end) = find_close_after_ci(&html, tag, search_from) {
                            // Safe because both start and end are on ASCII boundaries ('<' and '>')
                            html.replace_range(start..end, "");
                        } else {
                            // No close tag found; drop from the opening tag to end
                            html.truncate(start);
                            break;
                        }
                        guard += 1;
                    }
                }
                html
            }

            // Try to keep only <main> content if present; drastically reduces
            // boilerplate from navigation and login banners on many sites.
            fn extract_main(html: &str) -> Option<String> {
                // Find opening <main ...>
                let bytes = html.as_bytes();
                let open = {
                    let mut i = 0usize;
                    let tag = b"main";
                    while i + 5 < bytes.len() { // < m a i n > (min)
                        if bytes[i] == b'<' {
                            // skip '<' and whitespace
                            let mut j = i + 1;
                            while j < bytes.len() && bytes[j].is_ascii_whitespace() { j += 1; }
                            if j + tag.len() <= bytes.len() && bytes[j..j+tag.len()].eq_ignore_ascii_case(tag) {
                                // Found '<main'; now find '>'
                                while j < bytes.len() && bytes[j] != b'>' { j += 1; }
                                if j < bytes.len() { Some((i, j + 1)) } else { None }
                            } else { None }
                        } else { None }
                            .map(|pair| return pair);
                        i += 1;
                    }
                    None
                };
                let (start, after_open) = open?;
                // Find closing </main>
                let mut i = after_open;
                let tag_close = b"</main";
                while i + tag_close.len() + 1 < bytes.len() {
                    if bytes[i] == b'<' && bytes[i+1] == b'/' {
                        if bytes[i..].len() >= tag_close.len() && bytes[i..i+tag_close.len()].eq_ignore_ascii_case(tag_close) {
                            // Find closing '>'
                            let mut j = i + tag_close.len();
                            while j < bytes.len() && bytes[j] != b'>' { j += 1; }
                            if j < bytes.len() {
                                return Some(html[start..j+1].to_string());
                            } else {
                                return Some(html[start..].to_string());
                            }
                        }
                    }
                    i += 1;
                }
                Some(html[start..].to_string())
            }

            // Inside fenced code blocks, collapse massively-escaped Windows paths like
            // `C:\\Users\\...` to `C:\Users\...`. Only applies to drive-rooted paths.
            fn unescape_windows_paths(line: &str) -> String {
                let bytes = line.as_bytes();
                let mut out = String::with_capacity(line.len());
                let mut i = 0usize;
                while i < bytes.len() {
                    // Pattern: [A-Za-z] : \\+
                    if i + 3 < bytes.len()
                        && bytes[i].is_ascii_alphabetic()
                        && bytes[i+1] == b':'
                        && bytes[i+2] == b'\\'
                        && bytes[i+3] == b'\\'
                    {
                        // Emit drive and a single backslash
                        out.push(bytes[i] as char);
                        out.push(':');
                        out.push('\\');
                        // Skip all following backslashes in this run
                        i += 4;
                        while i < bytes.len() && bytes[i] == b'\\' { i += 1; }
                        continue;
                    }
                    out.push(bytes[i] as char);
                    i += 1;
                }
                out
            }

            // Lightweight cleanup on the resulting markdown to remove leaked
            // JSON blobs and obvious client boot payloads that sometimes escape
            // the <script> filter on complex sites. Avoids touching fenced code.
            fn postprocess_markdown(md: &str) -> String {
                let mut out: Vec<String> = Vec::with_capacity(md.len() / 64 + 1);
                let mut in_fence = false;
                let mut empty_run = 0usize;
                for line in md.lines() {
                    // Track fenced code blocks
                    if let Some(rest) = line.trim_start().strip_prefix("```") {
                        in_fence = !in_fence;
                        let _lang = if in_fence { Some(rest.trim()) } else { None };
                        out.push(line.to_string());
                        empty_run = 0;
                        continue;
                    }
                    if in_fence {
                        // Only normalize Windows path over-escaping; do not alter other content.
                        let normalized = unescape_windows_paths(line);
                        out.push(normalized);
                        continue;
                    }

                    let trimmed = line.trim();
                    // Drop extremely long single lines only if they're likely SPA boot payloads
                    if trimmed.len() > 8000 { continue; }
                    // Common SPA boot keys that shouldn't appear in human output.
                    // Keep this list tight to avoid dropping legitimate examples.
                    if trimmed.contains("\"payload\"") || trimmed.contains("\"props\"") || trimmed.contains("\"preloaded_records\"") || trimmed.contains("\"appPayload\"") || trimmed.contains("\"preloadedQueries\"") {
                        continue;
                    }

                    if trimmed.is_empty() {
                        // Collapse multiple empty lines to max 1
                        if empty_run == 0 {
                            out.push(String::new());
                        }
                        empty_run += 1;
                    } else {
                        out.push(line.to_string());
                        empty_run = 0;
                    }
                }
                // Trim leading/trailing blank lines
                let mut s = out.join("\n");
                while s.starts_with('\n') { s.remove(0); }
                while s.ends_with('\n') { s.pop(); }
                s
            }

            // Domain-specific: extract rich content from GitHub issue/PR pages
            // without requiring a JS-capable browser. We parse JSON-LD and the
            // inlined GraphQL payload (preloadedQueries) to reconstruct the
            // issue body and comments into readable markdown.
            fn try_extract_github_issue_markdown(html: &str) -> Option<String> {
                // Helper: extract the first <script type="application/ld+json"> block
                fn extract_ld_json(html: &str) -> Option<serde_json::Value> {
                    let mut s = html;
                    loop {
                        let start = s.find("<script").map(|i| i)?;
                        let rest = &s[start + 7..];
                        if rest.to_lowercase().contains("type=\"application/ld+json\"") {
                            // Find end of script open tag
                            let open_end_rel = rest.find('>')?;
                            let open_end = start + 7 + open_end_rel + 1;
                            let after_open = &s[open_end..];
                            // Find closing </script>
                            if let Some(close_rel) = after_open.to_lowercase().find("</script>") {
                                let json_str = &after_open[..close_rel];
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                                    return Some(v);
                                }
                                // Some pages JSON-encode the JSON-LD; try to unescape once
                                if let Ok(un) = serde_json::from_str::<String>(json_str) {
                                    if let Ok(v2) = serde_json::from_str::<serde_json::Value>(&un) {
                                        return Some(v2);
                                    }
                                }
                                // Advance after this script to search for next
                                s = &after_open[close_rel + 9..];
                                continue;
                            }
                        }
                        // Advance and continue search
                        s = &rest[1..];
                    }
                }

                // Helper: extract substring for the JSON array that follows key
                fn extract_json_array_after(html: &str, key: &str) -> Option<String> {
                    let idx = html.find(key)?;
                    let bytes = html.as_bytes();
                    // Find the first '[' after key
                    let mut i = idx + key.len();
                    while i < bytes.len() && bytes[i] != b'[' { i += 1; }
                    if i >= bytes.len() { return None; }
                    let start = i;
                    // Scan to matching ']' accounting for strings and escapes
                    let mut depth: i32 = 0;
                    let mut in_str = false;
                    let mut escape = false;
                    while i < bytes.len() {
                        let c = bytes[i] as char;
                        if in_str {
                            if escape { escape = false; }
                            else if c == '\\' { escape = true; }
                            else if c == '"' { in_str = false; }
                            i += 1; continue;
                        }
                        match c {
                            '"' => { in_str = true; },
                            '[' => { depth += 1; },
                            ']' => { depth -= 1; if depth == 0 { let end = i + 1; return Some(html[start..end].to_string()); } },
                            _ => {}
                        }
                        i += 1;
                    }
                    None
                }

                // Parse JSON-LD for headline, articleBody, author, date
                let mut title: Option<String> = None;
                let mut issue_body_md: Option<String> = None;
                let mut opened_by: Option<String> = None;
                let mut opened_at: Option<String> = None;
                if let Some(ld) = extract_ld_json(html) {
                    if ld.get("@type").and_then(|v| v.as_str()) == Some("DiscussionForumPosting") {
                        title = ld.get("headline").and_then(|v| v.as_str()).map(|s| s.to_string());
                        issue_body_md = ld.get("articleBody").and_then(|v| v.as_str()).map(|s| s.to_string());
                        opened_by = ld.get("author").and_then(|a| a.get("name")).and_then(|v| v.as_str()).map(|s| s.to_string());
                        opened_at = ld.get("datePublished").and_then(|v| v.as_str()).map(|s| s.to_string());
                    }
                }

                // Parse GraphQL payload for comments and state
                let arr_str = extract_json_array_after(html, "\"preloadedQueries\"")?;
                let arr: serde_json::Value = serde_json::from_str(&arr_str).ok()?;
                let mut comments: Vec<(String, String, String)> = Vec::new();
                let mut state: Option<String> = None;
                let mut state_reason: Option<String> = None;
                if let Some(items) = arr.as_array() {
                    for item in items {
                        let repo = item.get("result").and_then(|v| v.get("data")).and_then(|v| v.get("repository"));
                        let issue = repo.and_then(|r| r.get("issue"));
                        if let Some(issue) = issue {
                            if state.is_none() {
                                state = issue.get("state").and_then(|v| v.as_str()).map(|s| s.to_string());
                                state_reason = issue.get("stateReason").and_then(|v| v.as_str()).map(|s| s.to_string());
                            }
                            if let Some(edges) = issue.get("frontTimelineItems").and_then(|v| v.get("edges")).and_then(|v| v.as_array()) {
                                for e in edges {
                                    let node = e.get("node");
                                    let typename = node.and_then(|n| n.get("__typename")).and_then(|v| v.as_str()).unwrap_or("");
                                    if typename == "IssueComment" {
                                        let author = node.and_then(|n| n.get("author")).and_then(|a| a.get("login")).and_then(|v| v.as_str()).unwrap_or("");
                                        let created = node.and_then(|n| n.get("createdAt")).and_then(|v| v.as_str()).unwrap_or("");
                                        let body = node.and_then(|n| n.get("body")).and_then(|v| v.as_str()).unwrap_or("");
                                        if !body.is_empty() {
                                            comments.push((author.to_string(), created.to_string(), body.to_string()));
                                        } else {
                                            let body_html = node.and_then(|n| n.get("bodyHTML")).and_then(|v| v.as_str()).unwrap_or("");
                                            if !body_html.is_empty() {
                                                // Minimal HTML→MD for comments if body missing
                                                let options = htmd::options::Options { heading_style: htmd::options::HeadingStyle::Atx, code_block_style: htmd::options::CodeBlockStyle::Fenced, link_style: htmd::options::LinkStyle::Inlined, ..Default::default() };
                                                let conv = htmd::HtmlToMarkdown::builder().options(options).build();
                                                if let Ok(md) = conv.convert(body_html) {
                                                    comments.push((author.to_string(), created.to_string(), md));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // If nothing meaningful extracted, bail out.
                if title.is_none() && comments.is_empty() && issue_body_md.is_none() {
                    return None;
                }

                // Compose readable markdown
                let mut out = String::new();
                if let Some(t) = title { out.push_str(&format!("# {}\n\n", t)); }
                if let (Some(by), Some(at)) = (opened_by, opened_at) { out.push_str(&format!("Opened by {} on {}\n\n", by, at)); }
                if let (Some(s), _) = (state.clone(), state_reason.clone()) { out.push_str(&format!("State: {}\n\n", s)); }
                if let Some(body) = issue_body_md { out.push_str(&format!("{}\n\n", body)); }
                if !comments.is_empty() {
                    out.push_str("## Comments\n\n");
                    for (author, created, body) in comments {
                        out.push_str(&format!("- {} — {}\n\n{}\n\n", author, created, body));
                    }
                }
                Some(out)
            }

            // Helper: convert HTML to markdown and truncate if too large.
            fn convert_html_to_markdown_trimmed(html: String, max_chars: usize) -> crate::error::Result<(String, bool)> {
                let options = htmd::options::Options {
                    heading_style: htmd::options::HeadingStyle::Atx,
                    code_block_style: htmd::options::CodeBlockStyle::Fenced,
                    link_style: htmd::options::LinkStyle::Inlined,
                    ..Default::default()
                };
                let converter = htmd::HtmlToMarkdown::builder().options(options).build();
                let reduced = extract_main(&html).unwrap_or(html);
                let sanitized = strip_noisy_tags(reduced);
                let markdown = converter.convert(&sanitized)?;
                let markdown = postprocess_markdown(&markdown);
                let mut truncated = false;
                let rendered = {
                    let char_count = markdown.chars().count();
                    if char_count > max_chars {
                        truncated = true;
                        let mut s: String = markdown.chars().take(max_chars).collect();
                        s.push_str("\n\n… (truncated)\n");
                        s
                    } else {
                        markdown
                    }
                };
                Ok((rendered, truncated))
            }

            // Helper: detect WAF/challenge pages to avoid dumping challenge content.
            fn detect_block_vendor(_status: reqwest::StatusCode, body: &str) -> Option<&'static str> {
                // Identify common bot-challenge pages regardless of HTTP status.
                // Cloudflare often returns 200 with a challenge that requires JS/cookies.
                let lower = body.to_lowercase();
                if lower.contains("cloudflare")
                    || lower.contains("cf-ray")
                    || lower.contains("_cf_chl_opt")
                    || lower.contains("challenge-platform")
                    || lower.contains("checking if the site connection is secure")
                    || lower.contains("waiting for")
                    || lower.contains("just a moment")
                {
                    return Some("cloudflare");
                }
                None
            }

            fn headers_indicate_block(headers: &reqwest::header::HeaderMap) -> bool {
                let h = headers;
                let has_cf_ray = h.get("cf-ray").is_some();
                let has_cf_mitigated = h.get("cf-mitigated").is_some();
                let has_cf_bm = h.get("set-cookie").and_then(|v| v.to_str().ok()).map(|s| s.contains("__cf_bm=")).unwrap_or(false);
                let has_chlray = h.get("server-timing").and_then(|v| v.to_str().ok()).map(|s| s.to_lowercase().contains("chlray")).unwrap_or(false);
                has_cf_ray || has_cf_mitigated || has_cf_bm || has_chlray
            }

            fn looks_like_challenge_markdown(md: &str) -> bool {
                let l = md.to_lowercase();
                l.contains("just a moment") || l.contains("enable javascript and cookies") || l.contains("waiting for ")
            }

            let timeout = Duration::from_millis(params.timeout_ms.unwrap_or(15000));
            let code_ua = crate::default_client::get_code_user_agent(Some("web_fetch"));

            if matches!(params.mode.as_deref(), Some("browser")) {
                if let Some(browser_fetch) = fetch_html_via_browser(&params.url, timeout, true).await {
                    let (markdown, truncated) = match convert_html_to_markdown_trimmed(browser_fetch.html, 120_000) {
                        Ok(t) => t,
                        Err(e) => {
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload { content: format!("Markdown conversion failed: {e}"), success: Some(false) },
                            };
                        }
                    };

                    let body = serde_json::json!({
                        "url": params.url,
                        "status": 200,
                        "final_url": browser_fetch.final_url.unwrap_or_else(|| params.url.clone()),
                        "content_type": "text/html",
                        "used_browser_ua": true,
                        "via_browser": true,
                        "headless": browser_fetch.headless,
                        "truncated": truncated,
                        "markdown": markdown,
                    });
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload { content: body.to_string(), success: Some(true) },
                    };
                }
            }
            // Attempt 1: Codex UA + polite headers
            let resp = match do_request(&params.url, &code_ua, timeout, None).await {
                Ok(r) => r,
                Err(e) => {
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload { content: format!("Request failed: {e}"), success: Some(false) },
                    };
                }
            };

            // Capture metadata before consuming the response body.
            let mut status = resp.status();
            let mut final_url = resp.url().to_string();
            let mut headers = resp.headers().clone();
            // Read body
            let mut body_text = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload { content: format!("Failed to read response body: {e}"), success: Some(false) },
                    };
                }
            };
            let mut used_browser_ua = false;
            let browser_ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36";
            if !matches!(params.mode.as_deref(), Some("http")) && (detect_block_vendor(status, &body_text).is_some() || headers_indicate_block(&headers)) {
                // Simple retry with a browser UA and extra headers
                let extra = [
                    (reqwest::header::HeaderName::from_static("upgrade-insecure-requests"), "1"),
                ];
                if let Ok(r2) = do_request(&params.url, browser_ua, timeout, Some(&extra)).await {
                    let status2 = r2.status();
                    let final_url2 = r2.url().to_string();
                    let headers2 = r2.headers().clone();
                    if let Ok(t2) = r2.text().await {
                        used_browser_ua = true;
                        status = status2;
                        final_url = final_url2;
                        headers = headers2;
                        body_text = t2;
                    }
                }
            }

            // Response metadata
            let content_type = headers
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            // Provide structured diagnostics if blocked by WAF (even if HTTP 200)
            if !matches!(params.mode.as_deref(), Some("http")) && (detect_block_vendor(status, &body_text).is_some() || headers_indicate_block(&headers)) {
                let vendor = "cloudflare";
                let retry_after = headers
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let cf_ray = headers
                    .get("cf-ray")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                let mut diag = serde_json::json!({
                    "final_url": final_url,
                    "content_type": content_type,
                    "used_browser_ua": used_browser_ua,
                    "blocked_by_waf": true,
                    "vendor": vendor,
                });
                if let Some(ra) = retry_after { diag["retry_after"] = serde_json::json!(ra); }
                if let Some(ray) = cf_ray { diag["cf_ray"] = serde_json::json!(ray); }

                if let Some(browser_fetch) = fetch_html_via_browser(&params.url, timeout, false).await {
                    let (markdown, truncated) = match convert_html_to_markdown_trimmed(browser_fetch.html, 120_000) {
                        Ok(t) => t,
                        Err(e) => {
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload { content: format!("Markdown conversion failed: {e}"), success: Some(false) },
                            };
                        }
                    };

                    diag["via_browser"] = serde_json::json!(true);
                    if browser_fetch.headless {
                        diag["headless"] = serde_json::json!(true);
                    }

                    let body = serde_json::json!({
                        "url": params.url,
                        "status": 200,
                        "final_url": browser_fetch.final_url.unwrap_or_else(|| final_url.clone()),
                        "content_type": content_type,
                        "used_browser_ua": true,
                        "via_browser": true,
                        "headless": browser_fetch.headless,
                        "truncated": truncated,
                        "markdown": markdown,
                    });
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload { content: body.to_string(), success: Some(true) },
                    };
                }

                let (md_preview, _trunc) = match convert_html_to_markdown_trimmed(body_text, 2000) {
                    Ok(t) => t,
                    Err(_) => ("".to_string(), false),
                };

                let body = serde_json::json!({
                    "url": params.url,
                    "status": status.as_u16(),
                    "error": "Blocked by site challenge",
                    "diagnostics": diag,
                    "markdown": md_preview,
                });

                return ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload { content: body.to_string(), success: Some(false) },
                };
            }

            // If not success, provide structured, minimal diagnostics without dumping content.
            if !status.is_success() {
                let waf_vendor = detect_block_vendor(status, &body_text);
                let retry_after = headers
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let cf_ray = headers
                    .get("cf-ray")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                let mut diag = serde_json::json!({
                    "final_url": final_url,
                    "content_type": content_type,
                    "used_browser_ua": used_browser_ua,
                });
                if let Some(vendor) = waf_vendor { diag["blocked_by_waf"] = serde_json::json!(true); diag["vendor"] = serde_json::json!(vendor); }
                if let Some(ra) = retry_after { diag["retry_after"] = serde_json::json!(ra); }
                if let Some(ray) = cf_ray { diag["cf_ray"] = serde_json::json!(ray); }

                // Provide a tiny, safe preview of visible text only (converted and truncated).
                let (md_preview, _trunc) = match convert_html_to_markdown_trimmed(body_text, 2000) {
                    Ok(t) => t,
                    Err(_) => ("".to_string(), false),
                };

                let body = serde_json::json!({
                    "url": params.url,
                    "status": status.as_u16(),
                    "error": format!("HTTP {} {}", status.as_u16(), status.canonical_reason().unwrap_or("")),
                    "diagnostics": diag,
                    // Keep a short, human-friendly preview; avoid dumping raw HTML or long JS.
                    "markdown": md_preview,
                });

                return ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload { content: body.to_string(), success: Some(false) },
                };
            }

            // Domain-specific extraction first (e.g., GitHub issues)
            if params.url.contains("github.com/") && params.url.contains("/issues/") {
                if let Some(md) = try_extract_github_issue_markdown(&body_text) {
                    let body = serde_json::json!({
                        "url": params.url,
                        "status": status.as_u16(),
                        "final_url": final_url,
                        "content_type": content_type,
                        "used_browser_ua": used_browser_ua,
                        "truncated": false,
                        "markdown": md,
                    });
                    return ResponseInputItem::FunctionCallOutput { call_id: call_id_clone, output: FunctionCallOutputPayload { content: body.to_string(), success: Some(true) } };
                }
            }

            // Success: convert to markdown (sanitized and size-limited)
            let (markdown, truncated) = match convert_html_to_markdown_trimmed(body_text, 120_000) {
                Ok(t) => t,
                Err(e) => {
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload { content: format!("Markdown conversion failed: {e}"), success: Some(false) },
                    };
                }
            };

            // If the rendered markdown still looks like a challenge page, attempt browser fallback (unless http-only).
            if !matches!(params.mode.as_deref(), Some("http")) && looks_like_challenge_markdown(&markdown) {
                if let Some(browser_fetch) = fetch_html_via_browser(&params.url, timeout, false).await {
                    let (md2, truncated2) = match convert_html_to_markdown_trimmed(browser_fetch.html, 120_000) {
                        Ok(t) => t,
                        Err(e) => {
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload { content: format!("Markdown conversion failed: {e}"), success: Some(false) },
                            };
                        }
                    };

                    let body = serde_json::json!({
                        "url": params.url,
                        "status": 200,
                        "final_url": browser_fetch.final_url.unwrap_or_else(|| final_url.clone()),
                        "content_type": content_type,
                        "used_browser_ua": true,
                        "via_browser": true,
                        "headless": browser_fetch.headless,
                        "truncated": truncated2,
                        "markdown": md2,
                    });
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload { content: body.to_string(), success: Some(true) },
                    };
                }

                // If fallback not possible, return structured error rather than a useless challenge page
                let body = serde_json::json!({
                    "url": params.url,
                    "status": 200,
                    "error": "Blocked by site challenge",
                    "diagnostics": { "final_url": final_url, "content_type": content_type, "used_browser_ua": used_browser_ua, "blocked_by_waf": true, "vendor": "cloudflare", "detected_via": "markdown" },
                    "markdown": markdown.chars().take(2000).collect::<String>(),
                });
                return ResponseInputItem::FunctionCallOutput { call_id: call_id_clone, output: FunctionCallOutputPayload { content: body.to_string(), success: Some(false) } };
            }

            let body = serde_json::json!({
                "url": params.url,
                "status": status.as_u16(),
                "final_url": final_url,
                "content_type": content_type,
                "used_browser_ua": used_browser_ua,
                "truncated": truncated,
                "markdown": markdown,
            });

            ResponseInputItem::FunctionCallOutput { call_id: call_id_clone, output: FunctionCallOutputPayload { content: body.to_string(), success: Some(true) } }
        },
    ).await
}

// Wait for a background shell execution to complete.
// Parameters: { call_id?: string, timeout_ms?: number }
async fn handle_wait(
    sess: &Session,
    ctx: &ToolCallCtx,
    arguments: String,
) -> ResponseInputItem {
    use serde::Deserialize;
    #[derive(Deserialize, Clone)]
    struct Params { #[serde(default)] call_id: Option<String>, #[serde(default)] timeout_ms: Option<u64> }
    let mut params_for_event = serde_json::from_str::<serde_json::Value>(&arguments).ok();
    if let Some(serde_json::Value::Object(map)) = params_for_event.as_mut() {
        if let Some(serde_json::Value::String(cid)) = map.get("call_id") {
            let st = sess.state.lock().unwrap();
            if let Some(bg) = st.background_execs.get(cid) {
                map.insert("for".to_string(), serde_json::Value::String(bg.cmd_display.clone()));
            }
        }
    }
    let arguments_clone = arguments.clone();
    let ctx_clone = ToolCallCtx::new(ctx.sub_id.clone(), ctx.call_id.clone(), ctx.seq_hint, ctx.output_index);
    let ctx_for_closure = ctx_clone.clone();
    execute_custom_tool(
        sess,
        &ctx_clone,
        "wait".to_string(),
        params_for_event,
        move || async move {
            let ctx_inner = ctx_for_closure.clone();
                let parsed: Params = match serde_json::from_str(&arguments_clone) {
                    Ok(p) => p,
                    Err(e) => {
                    return ResponseInputItem::FunctionCallOutput { call_id: ctx_inner.call_id.clone(), output: FunctionCallOutputPayload { content: format!("Invalid wait arguments: {}", e), success: Some(false) } };
                    }
                };
                let call_id = match parsed.call_id {
                    Some(cid) if !cid.is_empty() => cid,
                    _ => {
                        return ResponseInputItem::FunctionCallOutput {
                            call_id: ctx_inner.call_id.clone(),
                            output: FunctionCallOutputPayload {
                                content: "wait requires a call_id".to_string(),
                                success: Some(false),
                            },
                        };
                    }
                };
                let max_ms: u64 = 3_600_000; // 60 minutes cap
                let default_ms: u64 = 600_000; // 10 minutes default
                let timeout_ms = parsed.timeout_ms.unwrap_or(default_ms).min(max_ms);
                use std::sync::atomic::Ordering;
                let (notify_opt, done_opt, tail, suppress_flag) = {
                    let st = sess.state.lock().unwrap();
                    match st.background_execs.get(&call_id) {
                        Some(bg) => (
                            Some(bg.notify.clone()),
                            bg.result_cell.lock().unwrap().clone(),
                            bg.tail_buf.clone(),
                            Some(bg.suppress_event.clone()),
                        ),
                        None => (None, None, None, None),
                    }
                };
                if let Some(flag) = &suppress_flag {
                    flag.store(true, Ordering::Relaxed);
                }
                if let Some(done) = done_opt {
                    let content = format_exec_output_with_limit(sess, &ctx_inner.sub_id, &ctx_inner.call_id, &done);
                    if let Some(flag) = &suppress_flag {
                        flag.store(true, Ordering::Relaxed);
                    }
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: ctx_inner.call_id.clone(),
                        output: FunctionCallOutputPayload { content, success: Some(done.exit_code == 0) },
                    };
                }
                let Some(spec_notify) = notify_opt else {
                    if let Some(flag) = &suppress_flag {
                        flag.store(false, Ordering::Relaxed);
                    }
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: ctx_inner.call_id.clone(),
                        output: FunctionCallOutputPayload {
                            content: format!("No background job found for call_id={call_id}"),
                            success: Some(false),
                        },
                    };
                };
                let any_notify = ANY_BG_NOTIFY.get().cloned().unwrap();
                let raced = tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), async move {
                    tokio::select! {
                        _ = spec_notify.notified() => (),
                        _ = any_notify.notified() => (),
                    }
                }).await;
                if raced.is_err() {
                    let tail_text = tail
                        .map(|arc| String::from_utf8_lossy(&arc.lock().unwrap()).to_string())
                        .unwrap_or_default();
                    let msg = if tail_text.is_empty() {
                        format!("Background job {call_id} still running...")
                    } else {
                        format!("Background job {call_id} still running...\n\nOutput so far (tail):\n{tail_text}")
                    };
                    if let Some(flag) = &suppress_flag {
                        flag.store(false, Ordering::Relaxed);
                    }
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: ctx_inner.call_id.clone(),
                        output: FunctionCallOutputPayload { content: msg, success: Some(false) },
                    };
                }
                let done = {
                    let mut st = sess.state.lock().unwrap();
                    if let Some(bg) = st.background_execs.remove(&call_id) {
                        bg.result_cell.lock().unwrap().clone()
                    } else {
                        let found = st
                            .background_execs
                            .iter()
                            .find_map(|(k, v)| if v.result_cell.lock().unwrap().is_some() { Some(k.clone()) } else { None });
                        found
                            .and_then(|k| st.background_execs.remove(&k))
                            .and_then(|bg| bg.result_cell.lock().unwrap().clone())
                    }
                };
                if let Some(done) = done {
                    let content = format_exec_output_with_limit(sess, &ctx_inner.sub_id, &ctx_inner.call_id, &done);
                    ResponseInputItem::FunctionCallOutput {
                        call_id: ctx_inner.call_id.clone(),
                        output: FunctionCallOutputPayload { content, success: Some(done.exit_code == 0) },
                    }
                } else {
                    if let Some(flag) = &suppress_flag {
                        flag.store(false, Ordering::Relaxed);
                    }
                    ResponseInputItem::FunctionCallOutput {
                        call_id: ctx_inner.call_id.clone(),
                        output: FunctionCallOutputPayload {
                            content: "No completed background job found".to_string(),
                            success: Some(false),
                        },
                    }
                }
        }
    ).await
}

// Kill a background shell execution by call_id.
async fn handle_kill(
    sess: &Session,
    ctx: &ToolCallCtx,
    arguments: String,
) -> ResponseInputItem {
    use serde::Deserialize;
    #[derive(Deserialize, Clone)]
    struct Params {
        call_id: String,
    }

    let mut params_for_event = serde_json::from_str::<serde_json::Value>(&arguments).ok();
    let arguments_clone = arguments.clone();
    let ctx_clone = ToolCallCtx::new(ctx.sub_id.clone(), ctx.call_id.clone(), ctx.seq_hint, ctx.output_index);
    let ctx_for_closure = ctx_clone.clone();
    let tx_event = sess.tx_event.clone();

    execute_custom_tool(
        sess,
        &ctx_clone,
        "kill".to_string(),
        params_for_event.take(),
        move || async move {
            let ctx_inner = ctx_for_closure.clone();
            let parsed: Params = match serde_json::from_str(&arguments_clone) {
                Ok(p) => p,
                Err(e) => {
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: ctx_inner.call_id.clone(),
                        output: FunctionCallOutputPayload {
                            content: format!("Invalid kill arguments: {e}"),
                            success: Some(false),
                        },
                    };
                }
            };

            use std::sync::atomic::Ordering;

            let (
                notify,
                result_cell,
                suppress_flag,
                cmd_display,
                order_meta_for_end,
                sub_id_for_end,
                handle_opt,
                already_done,
            ) = {
                let mut st = sess.state.lock().unwrap();
                match st.background_execs.get_mut(&parsed.call_id) {
                    Some(bg) => {
                        let done = bg.result_cell.lock().unwrap().is_some();
                        let handle = bg.task_handle.take();
                        (
                            bg.notify.clone(),
                            bg.result_cell.clone(),
                            bg.suppress_event.clone(),
                            bg.cmd_display.clone(),
                            bg.order_meta_for_end.clone(),
                            bg.sub_id.clone(),
                            handle,
                            done,
                        )
                    }
                    None => {
                        return ResponseInputItem::FunctionCallOutput {
                            call_id: ctx_inner.call_id.clone(),
                            output: FunctionCallOutputPayload {
                                content: format!("No background job found for call_id={}", parsed.call_id),
                                success: Some(false),
                            },
                        };
                    }
                }
            };

            if already_done {
                return ResponseInputItem::FunctionCallOutput {
                    call_id: ctx_inner.call_id.clone(),
                    output: FunctionCallOutputPayload {
                        content: format!("Background job {} has already completed.", parsed.call_id),
                        success: Some(false),
                    },
                };
            }

            suppress_flag.store(true, Ordering::Relaxed);
            if let Some(handle) = handle_opt {
                handle.abort();
                let _ = handle.await;
            }

            let cancel_message = "Cancelled by user.".to_string();
            let output = ExecToolCallOutput {
                exit_code: 130,
                stdout: StreamOutput::new(String::new()),
                stderr: StreamOutput::new(cancel_message.clone()),
                aggregated_output: StreamOutput::new(cancel_message.clone()),
                duration: std::time::Duration::ZERO,
                timed_out: false,
            };

            {
                let mut slot = result_cell.lock().unwrap();
                *slot = Some(output.clone());
            }

            notify.notify_waiters();
            if let Some(global) = ANY_BG_NOTIFY.get() {
                global.notify_waiters();
            }

            let end_msg = EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: parsed.call_id.clone(),
                stdout: output.stdout.text.clone(),
                stderr: output.stderr.text.clone(),
                exit_code: output.exit_code,
                duration: output.duration,
            });
            let event = Event {
                id: sub_id_for_end.clone(),
                event_seq: 0,
                msg: end_msg,
                order: Some(order_meta_for_end),
            };
            let _ = tx_event.send(event).await;

            let status = if cmd_display.trim().is_empty() {
                format!("Killed background job {}", parsed.call_id)
            } else {
                format!("Killed background command: {}", cmd_display)
            };

            ResponseInputItem::FunctionCallOutput {
                call_id: ctx_inner.call_id.clone(),
                output: FunctionCallOutputPayload {
                    content: status,
                    success: Some(true),
                },
            }
        },
    ).await
}

fn to_exec_params(params: ShellToolCallParams, sess: &Session) -> ExecParams {
    let timeout_ms = params
        .timeout_ms
        .map(|ms| ms.max(MIN_SHELL_TIMEOUT_MS));
    ExecParams {
        command: params.command,
        cwd: sess.resolve_path(params.workdir.clone()),
        timeout_ms,
        env: create_env(&sess.shell_environment_policy),
        with_escalated_permissions: params.with_escalated_permissions,
        justification: params.justification,
    }
}

fn resolve_agent_read_only(
    write: Option<bool>,
    read_only: Option<bool>,
    config: Option<&crate::config_types::AgentConfig>,
) -> bool {
    if let Some(flag) = write {
        return !flag;
    }
    if let Some(flag) = read_only {
        return flag;
    }
    config.map(|c| c.read_only).unwrap_or(false)
}

#[cfg(test)]
mod resolve_read_only_tests {
    use super::*;
    use crate::config_types::AgentConfig;

    fn make_config(read_only: bool) -> AgentConfig {
        AgentConfig {
            name: "test".into(),
            command: "test".into(),
            args: Vec::new(),
            read_only,
            enabled: true,
            description: None,
            env: None,
            args_read_only: None,
            args_write: None,
            instructions: None,
        }
    }

    #[test]
    fn explicit_write_overrides_config_read_only() {
        let cfg = make_config(true);
        assert!(
            !resolve_agent_read_only(Some(true), None, Some(&cfg)),
            "write=true should allow writes even when config prefers read-only"
        );
    }

    #[test]
    fn explicit_read_only_flag_takes_precedence() {
        let cfg = make_config(false);
        assert!(
            resolve_agent_read_only(None, Some(true), Some(&cfg)),
            "read_only=true should force read-only even when config allows writes"
        );
        assert!(
            resolve_agent_read_only(Some(false), None, Some(&cfg)),
            "write=false should force read-only"
        );
    }

    #[test]
    fn falls_back_to_config_when_request_absent() {
        let cfg = make_config(true);
        assert!(resolve_agent_read_only(None, None, Some(&cfg)));
    }

    #[test]
    fn defaults_to_false_without_config() {
        assert!(!resolve_agent_read_only(None, None, None));
    }
}

fn parse_container_exec_arguments(
    arguments: String,
    sess: &Session,
    call_id: &str,
) -> Result<ExecParams, Box<ResponseInputItem>> {
    // parse command
    match serde_json::from_str::<ShellToolCallParams>(&arguments) {
        Ok(shell_tool_call_params) => Ok(to_exec_params(shell_tool_call_params, sess)),
        Err(e) => {
            // allow model to re-sample
            let output = ResponseInputItem::FunctionCallOutput {
                call_id: call_id.to_string(),
                output: FunctionCallOutputPayload {
                    content: format!("failed to parse function arguments: {e}"),
                    success: None,
                },
            };
            Err(Box::new(output))
        }
    }
}

pub struct ExecInvokeArgs<'a> {
    pub params: ExecParams,
    pub sandbox_type: SandboxType,
    pub sandbox_policy: &'a SandboxPolicy,
    pub sandbox_cwd: &'a std::path::Path,
    pub code_linux_sandbox_exe: &'a Option<PathBuf>,
    pub stdout_stream: Option<StdoutStream>,
}

fn maybe_run_with_user_profile(params: ExecParams, sess: &Session) -> ExecParams {
    if sess.shell_environment_policy.use_profile {
        let maybe_command = sess
            .user_shell
            .format_default_shell_invocation(params.command.clone());
        if let Some(command) = maybe_command {
            return ExecParams { command, ..params };
        }
    }
    params
}

fn agent_tool_failure(ctx: &ToolCallCtx, message: impl Into<String>) -> ResponseInputItem {
    ResponseInputItem::FunctionCallOutput {
        call_id: ctx.call_id.clone(),
        output: FunctionCallOutputPayload {
            content: message.into(),
            success: Some(false),
        },
    }
}

pub(crate) async fn handle_agent_tool(
    sess: &Session,
    ctx: &ToolCallCtx,
    arguments: String,
) -> ResponseInputItem {
    let parsed = serde_json::from_str::<AgentToolRequest>(&arguments);
    let mut req = match parsed {
        Ok(req) => req,
        Err(e) => {
            return agent_tool_failure(ctx, format!("Invalid agent arguments: {}", e));
        }
    };

    let action = req.action.to_ascii_lowercase();
    match action.as_str() {
        "create" => {
            let mut create_opts = match req.create.take() {
                Some(opts) => opts,
                None => {
                    return agent_tool_failure(
                        ctx,
                        "action=create requires a 'create' object",
                    );
                }
            };

            let task = match create_opts.task.take() {
                Some(task) if !task.trim().is_empty() => task,
                _ => {
                    return agent_tool_failure(
                        ctx,
                        "action=create requires a non-empty 'create.task' field",
                    );
                }
            };

            let models = std::mem::take(&mut create_opts.models);
            let context = create_opts.context.take();
            let output = create_opts.output.take();
            let files = create_opts.files.take();
            let write = create_opts.write.take();
            let read_only = create_opts.read_only.take();
            let mut normalized_name = normalize_agent_name(create_opts.name.take());
            if normalized_name.is_none() {
                normalized_name = derive_agent_name_from_task(&task);
            }

            let run_params = RunAgentParams {
                task: task.clone(),
                models: models.clone(),
                context: context.clone(),
                output: output.clone(),
                files: files.clone(),
                write,
                read_only,
                name: normalized_name.clone(),
            };

            let mut create_event = serde_json::Map::new();
            create_event.insert("task".to_string(), serde_json::Value::String(task));
            if !models.is_empty() {
                create_event.insert(
                    "models".to_string(),
                    serde_json::Value::Array(
                        models
                            .iter()
                            .cloned()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
            if let Some(ref ctx_str) = context {
                if !ctx_str.is_empty() {
                    create_event.insert("context".to_string(), serde_json::Value::String(ctx_str.clone()));
                }
            }
            if let Some(ref output_str) = output {
                if !output_str.is_empty() {
                    create_event.insert("output".to_string(), serde_json::Value::String(output_str.clone()));
                }
            }
            if let Some(ref files_vec) = files {
                if !files_vec.is_empty() {
                    create_event.insert(
                        "files".to_string(),
                        serde_json::Value::Array(
                            files_vec
                                .iter()
                                .cloned()
                                .map(serde_json::Value::String)
                                .collect(),
                        ),
                    );
                }
            }
            if let Some(flag) = write {
                create_event.insert("write".to_string(), serde_json::Value::Bool(flag));
            }
            if let Some(flag) = read_only {
                create_event.insert("read_only".to_string(), serde_json::Value::Bool(flag));
            }
            if let Some(ref name_str) = normalized_name {
                if !name_str.is_empty() {
                    create_event.insert("name".to_string(), serde_json::Value::String(name_str.clone()));
                }
            }

            let mut event_root = serde_json::Map::new();
            event_root.insert("action".to_string(), serde_json::Value::String("create".to_string()));
            event_root.insert("create".to_string(), serde_json::Value::Object(create_event));
            let event_payload = serde_json::Value::Object(event_root);

            match serde_json::to_string(&run_params) {
                Ok(json) => handle_run_agent(sess, ctx, json, event_payload).await,
                Err(e) => agent_tool_failure(ctx, format!("Failed to encode create arguments: {}", e)),
            }
        }
        "status" => {
            let mut status_opts = match req.status.take() {
                Some(opts) => opts,
                None => {
                    return agent_tool_failure(
                        ctx,
                        "action=status requires a 'status' object",
                    );
                }
            };
            let agent_id = match status_opts.agent_id.take() {
                Some(id) if !id.trim().is_empty() => id,
                _ => {
                    return agent_tool_failure(
                        ctx,
                        "action=status requires 'status.agent_id'",
                    );
                }
            };
            let batch_id = match status_opts.batch_id.take() {
                Some(batch) if !batch.trim().is_empty() => batch,
                _ => {
                    return agent_tool_failure(
                        ctx,
                        "action=status requires 'status.batch_id'",
                    );
                }
            };
            let params = CheckAgentStatusParams {
                agent_id: agent_id.clone(),
                batch_id: batch_id.clone(),
            };
            let mut status_event = serde_json::Map::new();
            status_event.insert("agent_id".to_string(), serde_json::Value::String(agent_id));
            status_event.insert("batch_id".to_string(), serde_json::Value::String(batch_id));
            let mut status_event_root = serde_json::Map::new();
            status_event_root.insert("action".to_string(), serde_json::Value::String("status".to_string()));
            status_event_root.insert("status".to_string(), serde_json::Value::Object(status_event));
            let status_event_payload = serde_json::Value::Object(status_event_root);
            match serde_json::to_string(&params) {
                Ok(json) => handle_check_agent_status(sess, ctx, json, status_event_payload).await,
                Err(e) => agent_tool_failure(ctx, format!("Failed to encode status arguments: {}", e)),
            }
        }
        "result" => {
            let mut result_opts = match req.result.take() {
                Some(opts) => opts,
                None => {
                    return agent_tool_failure(
                        ctx,
                        "action=result requires a 'result' object",
                    );
                }
            };
            let agent_id = match result_opts.agent_id.take() {
                Some(id) if !id.trim().is_empty() => id,
                _ => {
                    return agent_tool_failure(
                        ctx,
                        "action=result requires 'result.agent_id'",
                    );
                }
            };
            let batch_id = match result_opts.batch_id.take() {
                Some(batch) if !batch.trim().is_empty() => batch,
                _ => {
                    return agent_tool_failure(
                        ctx,
                        "action=result requires 'result.batch_id'",
                    );
                }
            };
            let params = GetAgentResultParams {
                agent_id: agent_id.clone(),
                batch_id: batch_id.clone(),
            };
            let mut result_event = serde_json::Map::new();
            result_event.insert("agent_id".to_string(), serde_json::Value::String(agent_id));
            result_event.insert("batch_id".to_string(), serde_json::Value::String(batch_id));
            let mut result_event_root = serde_json::Map::new();
            result_event_root.insert("action".to_string(), serde_json::Value::String("result".to_string()));
            result_event_root.insert("result".to_string(), serde_json::Value::Object(result_event));
            let result_event_payload = serde_json::Value::Object(result_event_root);
            match serde_json::to_string(&params) {
                Ok(json) => handle_get_agent_result(sess, ctx, json, result_event_payload).await,
                Err(e) => agent_tool_failure(ctx, format!("Failed to encode result arguments: {}", e)),
            }
        }
        "cancel" => {
            let mut cancel_opts = match req.cancel.take() {
                Some(opts) => opts,
                None => {
                    return agent_tool_failure(
                        ctx,
                        "action=cancel requires a 'cancel' object",
                    );
                }
            };
            let cancel_agent_id = cancel_opts.agent_id.clone();
            let cancel_batch_id = match cancel_opts.batch_id.take() {
                Some(batch) if !batch.trim().is_empty() => batch,
                _ => {
                    return agent_tool_failure(
                        ctx,
                        "action=cancel requires 'cancel.batch_id'",
                    );
                }
            };
            let params = CancelAgentParams {
                agent_id: cancel_opts.agent_id.take(),
                batch_id: Some(cancel_batch_id.clone()),
            };
            let mut cancel_event = serde_json::Map::new();
            if let Some(id) = cancel_agent_id {
                cancel_event.insert("agent_id".to_string(), serde_json::Value::String(id));
            }
            cancel_event.insert("batch_id".to_string(), serde_json::Value::String(cancel_batch_id));
            let mut cancel_event_root = serde_json::Map::new();
            cancel_event_root.insert("action".to_string(), serde_json::Value::String("cancel".to_string()));
            cancel_event_root.insert("cancel".to_string(), serde_json::Value::Object(cancel_event));
            let cancel_event_payload = serde_json::Value::Object(cancel_event_root);
            match serde_json::to_string(&params) {
                Ok(json) => handle_cancel_agent(sess, ctx, json, cancel_event_payload).await,
                Err(e) => agent_tool_failure(ctx, format!("Failed to encode cancel arguments: {}", e)),
            }
        }
        "wait" => {
            let mut wait_opts = match req.wait.take() {
                Some(opts) => opts,
                None => {
                    return agent_tool_failure(
                        ctx,
                        "action=wait requires a 'wait' object",
                    );
                }
            };
            let wait_agent_id = wait_opts.agent_id.clone();
            let wait_batch_id = match wait_opts.batch_id.take() {
                Some(batch) if !batch.trim().is_empty() => batch,
                _ => {
                    return agent_tool_failure(
                        ctx,
                        "action=wait requires 'wait.batch_id'",
                    );
                }
            };
            let wait_timeout = wait_opts.timeout_seconds;
            let wait_return_all = wait_opts.return_all;
            let params = WaitForAgentParams {
                agent_id: wait_opts.agent_id.take(),
                batch_id: Some(wait_batch_id.clone()),
                timeout_seconds: wait_timeout,
                return_all: wait_return_all,
            };
            let mut wait_event = serde_json::Map::new();
            if let Some(id) = wait_agent_id {
                wait_event.insert("agent_id".to_string(), serde_json::Value::String(id));
            }
            wait_event.insert("batch_id".to_string(), serde_json::Value::String(wait_batch_id));
            if let Some(timeout) = wait_timeout {
                wait_event.insert("timeout_seconds".to_string(), serde_json::Value::from(timeout));
            }
            if let Some(return_all) = wait_return_all {
                wait_event.insert("return_all".to_string(), serde_json::Value::Bool(return_all));
            }
            let mut wait_event_root = serde_json::Map::new();
            wait_event_root.insert("action".to_string(), serde_json::Value::String("wait".to_string()));
            wait_event_root.insert("wait".to_string(), serde_json::Value::Object(wait_event));
            let wait_event_payload = serde_json::Value::Object(wait_event_root);
            match serde_json::to_string(&params) {
                Ok(json) => handle_wait_for_agent(sess, ctx, json, wait_event_payload).await,
                Err(e) => agent_tool_failure(ctx, format!("Failed to encode wait arguments: {}", e)),
            }
        }
        "list" => {
            let mut list_opts = match req.list.take() {
                Some(opts) => opts,
                None => {
                    return agent_tool_failure(
                        ctx,
                        "action=list requires a 'list' object",
                    );
                }
            };
            let status_filter = list_opts.status_filter.take();
            let batch_id = match list_opts.batch_id.take() {
                Some(batch) if !batch.trim().is_empty() => batch,
                _ => {
                    return agent_tool_failure(
                        ctx,
                        "action=list requires 'list.batch_id'",
                    );
                }
            };
            let recent_only = list_opts.recent_only;
            let params = ListAgentsParams {
                status_filter: status_filter.clone(),
                batch_id: Some(batch_id.clone()),
                recent_only,
            };
            let mut list_event = serde_json::Map::new();
            if let Some(ref status) = status_filter {
                if !status.is_empty() {
                    list_event.insert("status_filter".to_string(), serde_json::Value::String(status.clone()));
                }
            }
            list_event.insert("batch_id".to_string(), serde_json::Value::String(batch_id));
            if let Some(recent) = recent_only {
                list_event.insert("recent_only".to_string(), serde_json::Value::Bool(recent));
            }
            let mut list_event_root = serde_json::Map::new();
            list_event_root.insert("action".to_string(), serde_json::Value::String("list".to_string()));
            list_event_root.insert("list".to_string(), serde_json::Value::Object(list_event));
            let list_event_payload = serde_json::Value::Object(list_event_root);
            match serde_json::to_string(&params) {
                Ok(json) => handle_list_agents(sess, ctx, json, list_event_payload).await,
                Err(e) => agent_tool_failure(ctx, format!("Failed to encode list arguments: {}", e)),
            }
        }
        other => agent_tool_failure(ctx, format!("Unsupported agent action: {}", other)),
    }
}

pub(crate) async fn handle_run_agent(
    sess: &Session,
    ctx: &ToolCallCtx,
    arguments: String,
    event_payload: serde_json::Value,
) -> ResponseInputItem {
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();
    let generated_batch_id = Uuid::new_v4().to_string();
    let payload_with_batch = match event_payload {
        serde_json::Value::Object(mut map) => {
            map.insert(
                "batch_id".to_string(),
                serde_json::Value::String(generated_batch_id.clone()),
            );
            serde_json::Value::Object(map)
        }
        other => other,
    };
    let closure_batch_id = generated_batch_id.clone();
    execute_custom_tool(
        sess,
        ctx,
        "agent".to_string(),
        Some(payload_with_batch),
        move || async move {
            let batch_id = closure_batch_id.clone();
    match serde_json::from_str::<RunAgentParams>(&arguments_clone) {
        Ok(mut params) => {
            let trimmed_task = params.task.trim().to_string();
            let word_count = trimmed_task
                .split_whitespace()
                .filter(|segment| !segment.is_empty())
                .count();

            if trimmed_task.is_empty() || word_count < 4 {
                let guidance = format!(
                    "⚠️ Agent prompt too short: give the manager more context (at least a full sentence) before running agents. Current prompt: \"{}\".",
                    trimmed_task
                );
                let req = sess.current_request_ordinal();
                let order = sess.background_order_for_ctx(ctx, req);
                sess
                    .notify_background_event_with_order(&ctx.sub_id, order, guidance.clone())
                    .await;

                let response = serde_json::json!({
                    "status": "blocked",
                    "reason": "prompt_too_short",
                    "message": guidance,
                });
                return ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: response.to_string(),
                        success: Some(false),
                    },
                };
            }

            let mut manager = AGENT_MANAGER.write().await;
            let mut agent_name = params.name.clone();
            if agent_name.is_none() {
                if let Some(fallback) = derive_agent_name_from_task(trimmed_task.as_str()) {
                    agent_name = Some(fallback.clone());
                    params.name = Some(fallback);
                }
            }

            // Collect requested models from the `models` field.
            let raw_models: Vec<String> = params.models.clone();

            // Split comma-delimited strings, trim whitespace, and deduplicate case-insensitively.
            let mut seen_models = HashSet::new();
            let mut models: Vec<String> = Vec::new();
            for entry in raw_models {
                for candidate in entry.split(',') {
                    let trimmed = candidate.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let dedupe_key = trimmed.to_lowercase();
                    if seen_models.insert(dedupe_key) {
                        models.push(trimmed.to_string());
                    }
                }
            }

            if models.is_empty() {
                if sess.tools_config.agent_model_allowed_values.is_empty() {
                    models.push("code".to_string());
                } else {
                    models.extend(
                        sess
                            .tools_config
                            .agent_model_allowed_values
                            .iter()
                            .cloned(),
                    );
                }
            }

            models.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
            models.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

            // Helper: derive the command to check for a given model/config pair.
            fn resolve_command_for_check(model: &str, cfg: Option<&crate::config_types::AgentConfig>) -> (String, bool) {
                if let Some(c) = cfg { return (c.command.clone(), false); }
                let m = model.to_lowercase();
                match m.as_str() {
                    // Built-in: always available via current_exe fallback.
                    "code" | "codex" => (m, true),
                    // External CLIs expected to be in PATH
                    "claude" => ("claude".to_string(), false),
                    "gemini" => ("gemini".to_string(), false),
                    "qwen" => ("qwen".to_string(), false),
                    // Cloud agent: treat as built-in via current executable (code cloud submit)
                    "cloud" => ("cloud".to_string(), true),
                    _ => (m, false),
                }
            }

            // Helper: PATH lookup to determine if a command exists.
            fn command_exists(cmd: &str) -> bool {
                // Absolute/relative path with separators: verify it points to a file.
                if cmd.contains(std::path::MAIN_SEPARATOR) || cmd.contains('/') || cmd.contains('\\') {
                    return std::fs::metadata(cmd).map(|m| m.is_file()).unwrap_or(false);
                }

                #[cfg(target_os = "windows")]
                {
                    return which::which(cmd).map(|p| p.is_file()).unwrap_or(false);
                }

                #[cfg(not(target_os = "windows"))]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let Some(path_os) = std::env::var_os("PATH") else { return false; };
                    for dir in std::env::split_paths(&path_os) {
                        if dir.as_os_str().is_empty() { continue; }
                        let candidate = dir.join(cmd);
                        if let Ok(meta) = std::fs::metadata(&candidate) {
                            if meta.is_file() {
                                let mode = meta.permissions().mode();
                                if mode & 0o111 != 0 { return true; }
                            }
                        }
                    }
                    false
                }
            }

            let multi_model = models.len() > 1;
            let display_label_for = |model: &str| -> String {
                agent_name
                    .as_ref()
                    .and_then(|value| {
                        if value.is_empty() {
                            None
                        } else if multi_model {
                            Some(format!("{} ({})", value, model))
                        } else {
                            Some(value.to_string())
                        }
                    })
                    .unwrap_or_else(|| model.to_string())
            };

            let mut agent_ids = Vec::new();
            let mut agent_labels: Vec<(String, String)> = Vec::new();
            let mut skipped: Vec<String> = Vec::new();
            for model in models {
                let model_key = model.to_lowercase();
                // Check if this model is configured and enabled
                let agent_config = sess.agents.iter().find(|a| {
                    a.name.to_lowercase() == model_key
                        || a.command.to_lowercase() == model_key
                });

                if let Some(config) = agent_config {
                    if !config.enabled {
                        continue; // Skip disabled agents
                    }

                    let (cmd_to_check, is_builtin) = resolve_command_for_check(&model, Some(config));
                    if !is_builtin && !command_exists(&cmd_to_check) {
                        skipped.push(format!("{} (missing: {})", model, cmd_to_check));
                        continue;
                    }

                    // Respect explicit read_only flag from the caller; otherwise fall back to the config default.
                    let read_only = resolve_agent_read_only(
                        params.write,
                        params.read_only,
                        Some(config),
                    );

                    let agent_id = manager
                        .create_agent_with_config(
                            model.clone(),
                            agent_name.clone(),
                            params.task.clone(),
                            params.context.clone(),
                            params.output.clone(),
                            params.files.clone().unwrap_or_default(),
                            read_only,
                            Some(batch_id.clone()),
                            config.clone(),
                        )
                        .await;
                    agent_ids.push(agent_id);
                    let label = display_label_for(&model);
                    agent_labels.push((agent_ids.last().cloned().unwrap(), label));
                } else {
                    // Use default configuration for unknown agents
                    let (cmd_to_check, is_builtin) = resolve_command_for_check(&model, None);
                    if !is_builtin && !command_exists(&cmd_to_check) {
                        skipped.push(format!("{} (missing: {})", model, cmd_to_check));
                        continue;
                    }
                    let read_only = resolve_agent_read_only(params.write, params.read_only, None);
                    let agent_id = manager
                        .create_agent(
                            model.clone(),
                            agent_name.clone(),
                            params.task.clone(),
                            params.context.clone(),
                            params.output.clone(),
                            params.files.clone().unwrap_or_default(),
                            read_only,
                            Some(batch_id.clone()),
                        )
                        .await;
                    agent_ids.push(agent_id);
                    let label = display_label_for(&model);
                    agent_labels.push((agent_ids.last().cloned().unwrap(), label));
                }
            }

            // If nothing runnable remains, fall back to a single built‑in Codex agent.
            if agent_ids.is_empty() {
                let read_only = resolve_agent_read_only(params.write, params.read_only, None);
                let agent_id = manager
                    .create_agent(
                        "code".to_string(),
                        agent_name.clone(),
                        params.task.clone(),
                        params.context.clone(),
                        params.output.clone(),
                        params.files.clone().unwrap_or_default(),
                        read_only,
                        Some(batch_id.clone()),
                    )
                    .await;
                agent_ids.push(agent_id);
                let label = display_label_for("code");
                agent_labels.push((agent_ids.last().cloned().unwrap(), label));
            }

            // Send agent status update event
            drop(manager); // Release the write lock first
            if agent_ids.len() > 0 {
                send_agent_status_update(sess).await;
            }

            let launch_hint = if agent_ids.len() > 1 {
                let short_batch = short_id(&batch_id);
                let agent_phrase = agent_labels
                    .iter()
                    .map(|(id, label)| format!("{} [{}]", short_id(id), label))
                    .collect::<Vec<_>>()
                    .join(", ");
                let first_agent = agent_labels
                    .first()
                    .map(|(id, _)| id.as_str())
                    .unwrap_or(batch_id.as_str());
                format!(
                    "🤖 Agent batch {short_batch} started: {agent_phrase}.\nUse `agent {{\"action\":\"wait\",\"wait\":{{\"batch_id\":\"{batch}\",\"return_all\":true}}}}` to wait for all agents, then `agent {{\"action\":\"result\",\"result\":{{\"agent_id\":\"{first_agent}\"}}}}` for a detailed report.",
                    batch = batch_id,
                )
            } else {
                let (single_id, single_model) = agent_labels
                    .first()
                    .map(|(id, model)| (id.as_str(), model.as_str()))
                    .unwrap();
                let short_batch = short_id(&batch_id);
                format!(
                    "🤖 Agent batch {short_batch} started with {model}. Use `agent {{\"action\":\"wait\",\"wait\":{{\"batch_id\":\"{batch}\",\"return_all\":true}}}}` to follow progress, or `agent {{\"action\":\"result\",\"result\":{{\"agent_id\":\"{agent}\"}}}}` when it finishes.",
                    model = single_model,
                    batch = batch_id,
                    agent = single_id,
                )
            };

            let mut response_map = serde_json::Map::new();
            response_map.insert(
                "batch_id".to_string(),
                serde_json::Value::String(batch_id.clone()),
            );
            response_map.insert(
                "agent_ids".to_string(),
                serde_json::Value::Array(
                    agent_ids
                        .iter()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
            response_map.insert(
                "status".to_string(),
                serde_json::Value::String("started".to_string()),
            );
            let message = if agent_ids.len() > 1 {
                format!("Started {} agents", agent_labels.len())
            } else {
                "Agent started successfully".to_string()
            };
            response_map.insert(
                "message".to_string(),
                serde_json::Value::String(message),
            );
            response_map.insert(
                "next_steps".to_string(),
                serde_json::Value::String(launch_hint.clone()),
            );
            if agent_ids.len() == 1 {
                if let Some(first) = agent_ids.first() {
                    response_map.insert(
                        "agent_id".to_string(),
                        serde_json::Value::String(first.clone()),
                    );
                }
            }
            if skipped.is_empty() {
                response_map.insert("skipped".to_string(), serde_json::Value::Null);
            } else {
                response_map.insert(
                    "skipped".to_string(),
                    serde_json::Value::Array(
                        skipped
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
            let response = serde_json::Value::Object(response_map);

            ResponseInputItem::FunctionCallOutput {
                call_id: call_id_clone,
                output: FunctionCallOutputPayload {
                    content: response.to_string(),
                    success: Some(true),
                },
            }
        }
        Err(e) => ResponseInputItem::FunctionCallOutput {
            call_id: call_id_clone,
            output: FunctionCallOutputPayload {
                content: format!("Invalid agent arguments: {}", e),
                success: None,
            },
        },
    }
        }
    ).await
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn derive_agent_name_from_task(task: &str) -> Option<String> {
    let trimmed = task.trim();
    if trimmed.is_empty() {
        return None;
    }

    let first_clause = trimmed
        .split(|c: char| matches!(c, '.' | '!' | '?' | '\n'))
        .find(|part| !part.trim().is_empty())
        .unwrap_or(trimmed)
        .trim();

    let words: Vec<&str> = first_clause.split_whitespace().take(5).collect();
    if words.is_empty() {
        return None;
    }

    normalize_agent_name(Some(words.join(" ")))
}

async fn handle_check_agent_status(
    sess: &Session,
    ctx: &ToolCallCtx,
    arguments: String,
    event_payload: serde_json::Value,
) -> ResponseInputItem {
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();
    execute_custom_tool(
        sess,
        ctx,
        "agent".to_string(),
        Some(event_payload),
        || async move {
    match serde_json::from_str::<CheckAgentStatusParams>(&arguments_clone) {
        Ok(params) => {
            let manager = AGENT_MANAGER.read().await;

            if let Some(agent) = manager.get_agent(&params.agent_id) {
                match agent.batch_id.as_deref() {
                    Some(batch) if batch == params.batch_id => {}
                    _ => {
                        return ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone,
                            output: FunctionCallOutputPayload {
                                content: format!(
                                    "Agent {} does not belong to batch {}",
                                    params.agent_id, params.batch_id
                                ),
                                success: Some(false),
                            },
                        };
                    }
                }

                // Limit progress in the response; write full progress to file if large
                let max_progress_lines = 50usize;
                let total_progress = agent.progress.len();
                let progress_preview: Vec<String> = if total_progress > max_progress_lines {
                    agent
                        .progress
                        .iter()
                        .skip(total_progress - max_progress_lines)
                        .cloned()
                        .collect()
                } else {
                    agent.progress.clone()
                };

                let mut progress_file: Option<String> = None;
                if total_progress > max_progress_lines {
                    let cwd = sess.get_cwd().to_path_buf();
                    drop(manager);
                    let dir = match ensure_agent_dir(&cwd, &agent.id) {
                        Ok(d) => d,
                        Err(e) => {
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload {
                                    content: format!("Failed to prepare agent progress file: {}", e),
                                    success: Some(false),
                                },
                            };
                        }
                    };
                    // Re-acquire manager to get fresh progress after potential delay
                    let manager = AGENT_MANAGER.read().await;
                    if let Some(agent) = manager.get_agent(&params.agent_id) {
                        let joined = agent.progress.join("\n");
                        match write_agent_file(&dir, "progress.log", &joined) {
                            Ok(p) => progress_file = Some(p.display().to_string()),
                            Err(e) => {
                                return ResponseInputItem::FunctionCallOutput {
                                    call_id: call_id_clone,
                                    output: FunctionCallOutputPayload {
                                        content: format!("Failed to write progress file: {}", e),
                                        success: Some(false),
                                    },
                                };
                            }
                        }
                    }
                } else {
                    drop(manager);
                }

                let response = serde_json::json!({
                    "agent_id": params.agent_id,
                    "name": agent.name,
                    "status": agent.status,
                    "model": agent.model,
                    "batch_id": agent.batch_id,
                    "created_at": agent.created_at,
                    "started_at": agent.started_at,
                    "completed_at": agent.completed_at,
                    "progress_preview": progress_preview,
                    "progress_total": total_progress,
                    "progress_file": progress_file,
                    "error": agent.error,
                    "worktree_path": agent.worktree_path,
                    "branch_name": agent.branch_name,
                });

                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: response.to_string(),
                        success: Some(true),
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: format!("Agent not found: {}", params.agent_id),
                        success: Some(false),
                    },
                }
            }
        }
        Err(e) => ResponseInputItem::FunctionCallOutput {
            call_id: call_id_clone,
            output: FunctionCallOutputPayload {
                content: format!("Invalid agent arguments for action=status: {}", e),
                success: None,
            },
        },
    }
        },
    ).await
}

async fn handle_get_agent_result(
    sess: &Session,
    ctx: &ToolCallCtx,
    arguments: String,
    event_payload: serde_json::Value,
) -> ResponseInputItem {
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();
    execute_custom_tool(
        sess,
        ctx,
        "agent".to_string(),
        Some(event_payload),
        || async move {
    match serde_json::from_str::<GetAgentResultParams>(&arguments_clone) {
        Ok(params) => {
            let manager = AGENT_MANAGER.read().await;

            if let Some(agent) = manager.get_agent(&params.agent_id) {
                match agent.batch_id.as_deref() {
                    Some(batch) if batch == params.batch_id => {}
                    _ => {
                        return ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone,
                            output: FunctionCallOutputPayload {
                                content: format!(
                                    "Agent {} does not belong to batch {}",
                                    params.agent_id, params.batch_id
                                ),
                                success: Some(false),
                            },
                        };
                    }
                }
                let cwd = sess.get_cwd().to_path_buf();
                let dir = match ensure_agent_dir(&cwd, &params.agent_id) {
                    Ok(d) => d,
                    Err(e) => {
                        return ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone,
                            output: FunctionCallOutputPayload {
                                content: format!("Failed to prepare agent output dir: {}", e),
                                success: Some(false),
                            },
                        };
                    }
                };

                match agent.status {
                    AgentStatus::Completed => {
                        let output_text = agent.result.unwrap_or_default();
                        let (preview, total_lines) = preview_first_n_lines(&output_text, 500);
                        let file_path = match write_agent_file(&dir, "result.txt", &output_text) {
                            Ok(p) => p.display().to_string(),
                            Err(e) => format!("Failed to write result file: {}", e),
                        };
                        let response = serde_json::json!({
                            "agent_id": params.agent_id,
                            "batch_id": params.batch_id.clone(),
                            "status": agent.status,
                            "output_preview": preview,
                            "output_total_lines": total_lines,
                            "output_file": file_path,
                        });
                        ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone,
                            output: FunctionCallOutputPayload {
                                content: response.to_string(),
                                success: Some(true),
                            },
                        }
                    }
                    AgentStatus::Failed => {
                        let error_text = agent.error.unwrap_or_else(|| "Unknown error".to_string());
                        let (preview, total_lines) = preview_first_n_lines(&error_text, 500);
                        let file_path = match write_agent_file(&dir, "error.txt", &error_text) {
                            Ok(p) => p.display().to_string(),
                            Err(e) => format!("Failed to write error file: {}", e),
                        };
                        let response = serde_json::json!({
                            "agent_id": params.agent_id,
                            "batch_id": params.batch_id.clone(),
                            "status": agent.status,
                            "error_preview": preview,
                            "error_total_lines": total_lines,
                            "error_file": file_path,
                        });
                        ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone,
                            output: FunctionCallOutputPayload {
                                content: response.to_string(),
                                success: Some(false),
                            },
                        }
                    }
                    _ => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: format!(
                                "Agent is still {}: cannot get result yet",
                                serde_json::to_string(&agent.status)
                                    .unwrap_or_else(|_| "running".to_string())
                            ),
                            success: Some(false),
                        },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: format!("Agent not found: {}", params.agent_id),
                        success: Some(false),
                    },
                }
            }
        }
        Err(e) => ResponseInputItem::FunctionCallOutput {
            call_id: call_id_clone,
            output: FunctionCallOutputPayload {
                content: format!("Invalid agent arguments for action=result: {}", e),
                success: None,
            },
        },
    }
        },
    ).await
}

async fn handle_cancel_agent(
    sess: &Session,
    ctx: &ToolCallCtx,
    arguments: String,
    event_payload: serde_json::Value,
) -> ResponseInputItem {
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();
    execute_custom_tool(
        sess,
        ctx,
        "agent".to_string(),
        Some(event_payload),
        || async move {
    match serde_json::from_str::<CancelAgentParams>(&arguments_clone) {
        Ok(params) => {
            let mut manager = AGENT_MANAGER.write().await;

            if let Some(agent_id) = params.agent_id {
                let batch_id = match params.batch_id.as_ref() {
                    Some(batch) => batch,
                    None => {
                        return ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone,
                            output: FunctionCallOutputPayload {
                                content: "action=cancel requires 'cancel.batch_id'".to_string(),
                                success: Some(false),
                            },
                        };
                    }
                };
                if let Some(agent) = manager.get_agent(&agent_id) {
                    if agent.batch_id.as_deref() != Some(batch_id.as_str()) {
                        return ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone,
                            output: FunctionCallOutputPayload {
                                content: format!(
                                    "Agent {} does not belong to batch {}",
                                    agent_id, batch_id
                                ),
                                success: Some(false),
                            },
                        };
                    }
                }
                if manager.cancel_agent(&agent_id).await {
                    ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: format!("Agent {} cancelled", agent_id),
                            success: Some(true),
                        },
                    }
                } else {
                    ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to cancel agent {}", agent_id),
                            success: Some(false),
                        },
                    }
                }
            } else if let Some(batch_id) = params.batch_id {
                let count = manager.cancel_batch(&batch_id).await;
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: format!("Cancelled {} agents in batch {}", count, batch_id),
                        success: Some(true),
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: "Either agent_id or batch_id must be provided".to_string(),
                        success: Some(false),
                    },
                }
            }
        }
        Err(e) => ResponseInputItem::FunctionCallOutput {
            call_id: call_id_clone,
            output: FunctionCallOutputPayload {
                content: format!("Invalid agent arguments for action=cancel: {}", e),
                success: None,
            },
        },
    }
        },
    ).await
}

async fn handle_wait_for_agent(
    sess: &Session,
    ctx: &ToolCallCtx,
    arguments: String,
    event_payload: serde_json::Value,
) -> ResponseInputItem {
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();
    execute_custom_tool(
        sess,
        ctx,
        "agent".to_string(),
        Some(event_payload),
        || async move {
            let (initial_wait_epoch, _) = sess.wait_interrupt_snapshot();
            match serde_json::from_str::<WaitForAgentParams>(&arguments_clone) {
                Ok(params) => {
                    let batch_id = match params.batch_id.as_ref() {
                        Some(batch) if !batch.trim().is_empty() => batch.clone(),
                        _ => {
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload {
                                    content: "action=wait requires 'wait.batch_id'".to_string(),
                                    success: Some(false),
                                },
                            };
                        }
                    };
                    let timeout = std::time::Duration::from_secs(
                        params.timeout_seconds.unwrap_or(300).min(600),
                    );
                    let start = std::time::Instant::now();

                    loop {
                        if start.elapsed() > timeout {
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload {
                                    content: "Timeout waiting for agent completion".to_string(),
                                    success: Some(false),
                                },
                            };
                        }

                        let manager = AGENT_MANAGER.read().await;

                        if let Some(agent_id) = &params.agent_id {
                            if let Some(agent) = manager.get_agent(agent_id) {
                                match agent.batch_id.as_deref() {
                                    Some(batch) if batch == batch_id => {}
                                    _ => {
                                        return ResponseInputItem::FunctionCallOutput {
                                            call_id: call_id_clone,
                                            output: FunctionCallOutputPayload {
                                                content: format!(
                                                    "Agent {} does not belong to batch {}",
                                                    agent_id, batch_id
                                                ),
                                                success: Some(false),
                                            },
                                        };
                                    }
                                }
                                if matches!(
                            agent.status,
                            AgentStatus::Completed | AgentStatus::Failed | AgentStatus::Cancelled
                        ) {
                            // Include output/error preview and file path
                            // Avoid holding manager lock during filesystem I/O
                            drop(manager);
                            let cwd = sess.get_cwd().to_path_buf();
                            let dir = match ensure_agent_dir(&cwd, &agent.id) {
                                Ok(d) => d,
                                Err(e) => {
                                    return ResponseInputItem::FunctionCallOutput {
                                        call_id: call_id_clone,
                                        output: FunctionCallOutputPayload {
                                            content: format!("Failed to prepare agent output dir: {}", e),
                                            success: Some(false),
                                        },
                                    };
                                }
                            };
                            let (preview_key, file_key, preview, file_path, total_lines) = match agent.status {
                                AgentStatus::Completed => {
                                    let text = agent.result.clone().unwrap_or_default();
                                    let (p, total) = preview_first_n_lines(&text, 500);
                                    let fp = write_agent_file(&dir, "result.txt", &text)
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_else(|e| format!("Failed to write result file: {}", e));
                                    ("output_preview", "output_file", p, fp, total)
                                }
                                AgentStatus::Failed => {
                                    let text = agent.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                                    let (p, total) = preview_first_n_lines(&text, 500);
                                    let fp = write_agent_file(&dir, "error.txt", &text)
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_else(|e| format!("Failed to write error file: {}", e));
                                    ("error_preview", "error_file", p, fp, total)
                                }
                                AgentStatus::Cancelled => {
                                    let text = "Agent cancelled".to_string();
                                    let (p, total) = preview_first_n_lines(&text, 500);
                                    let fp = write_agent_file(&dir, "status.txt", &text)
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_else(|e| format!("Failed to write status file: {}", e));
                                    ("status_preview", "status_file", p, fp, total)
                                }
                                _ => unreachable!(),
                            };

                            let hint = format!(
                                "agent {{\"action\":\"result\",\"result\":{{\"agent_id\":\"{}\",\"batch_id\":\"{}\"}}}}",
                                agent.id,
                                batch_id
                            );
                            let mut response = serde_json::json!({
                                "agent_id": agent.id,
                                "batch_id": batch_id,
                                "status": agent.status,
                                "wait_time_seconds": start.elapsed().as_secs(),
                                "total_lines": total_lines,
                                "agent_result_hint": hint,
                                "agent_result_params": { "action": "result", "result": { "agent_id": agent.id, "batch_id": batch_id } },
                            });
                            if let Some(obj) = response.as_object_mut() {
                                obj.insert(preview_key.to_string(), serde_json::Value::String(preview));
                                obj.insert(file_key.to_string(), serde_json::Value::String(file_path));
                            }
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload {
                                    content: response.to_string(),
                                    success: Some(true),
                                },
                            };
                        }
                    }
                } else {
                    let agents = manager.list_agents(None, Some(batch_id.clone()), false);

                    // Separate terminal vs non-terminal agents
                    let completed_agents: Vec<_> = agents
                        .iter()
                        .filter(|t| {
                            matches!(
                                t.status,
                                AgentStatus::Completed
                                    | AgentStatus::Failed
                                    | AgentStatus::Cancelled
                            )
                        })
                        .cloned()
                        .collect();
                    let any_in_progress = agents.iter().any(|a| {
                        matches!(a.status, AgentStatus::Pending | AgentStatus::Running)
                    });

                    if params.return_all.unwrap_or(false) {
                        // Wait for ALL agents in the batch to reach a terminal state
                        if !any_in_progress {
                            // Enriched response: include per-agent previews and file paths
                            // Avoid holding manager lock during filesystem I/O
                            drop(manager);
                            let cwd = sess.get_cwd().to_path_buf();
                            let mut summaries: Vec<serde_json::Value> = Vec::new();
                            for a in &completed_agents {
                                let dir = match ensure_agent_dir(&cwd, &a.id) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        return ResponseInputItem::FunctionCallOutput {
                                            call_id: call_id_clone,
                                            output: FunctionCallOutputPayload {
                                                content: format!("Failed to prepare agent output dir: {}", e),
                                                success: Some(false),
                                            },
                                        };
                                    }
                                };
                                let (preview_key, file_key, preview, file_path, total_lines) = match a.status {
                                    AgentStatus::Completed => {
                                        let text = a.result.clone().unwrap_or_default();
                                        let (p, total) = preview_first_n_lines(&text, 500);
                                        let fp = write_agent_file(&dir, "result.txt", &text)
                                            .map(|p| p.display().to_string())
                                            .unwrap_or_else(|e| format!("Failed to write result file: {}", e));
                                        ("output_preview", "output_file", p, fp, total)
                                    }
                                    AgentStatus::Failed => {
                                        let text = a.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                                        let (p, total) = preview_first_n_lines(&text, 500);
                                        let fp = write_agent_file(&dir, "error.txt", &text)
                                            .map(|p| p.display().to_string())
                                            .unwrap_or_else(|e| format!("Failed to write error file: {}", e));
                                        ("error_preview", "error_file", p, fp, total)
                                    }
                                    AgentStatus::Cancelled => {
                                        let text = "Agent cancelled".to_string();
                                        let (p, total) = preview_first_n_lines(&text, 500);
                                        let fp = write_agent_file(&dir, "status.txt", &text)
                                            .map(|p| p.display().to_string())
                                            .unwrap_or_else(|e| format!("Failed to write status file: {}", e));
                                        ("status_preview", "status_file", p, fp, total)
                                    }
                                    _ => unreachable!(),
                                };

                                let hint = format!(
                                    "agent {{\"action\":\"result\",\"result\":{{\"agent_id\":\"{}\",\"batch_id\":\"{}\"}}}}",
                                    a.id,
                                    batch_id
                                );
                                let mut obj = serde_json::json!({
                                    "agent_id": a.id,
                                    "status": a.status,
                                    "total_lines": total_lines,
                                    "agent_result_hint": hint,
                                "agent_result_params": { "action": "result", "result": { "agent_id": a.id, "batch_id": batch_id } },
                                });
                                if let Some(map) = obj.as_object_mut() {
                                    map.insert(preview_key.to_string(), serde_json::Value::String(preview));
                                    map.insert(file_key.to_string(), serde_json::Value::String(file_path));
                                }
                                summaries.push(obj);
                            }

                            let response = serde_json::json!({
                                "batch_id": batch_id,
                                "completed_agents": completed_agents.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
                                "completed_summaries": summaries,
                                "wait_time_seconds": start.elapsed().as_secs(),
                            });
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload {
                                    content: response.to_string(),
                                    success: Some(true),
                                },
                            };
                        }
                    } else {
                        // Sequential behavior: return the next unseen completed agent if available
                        let mut state = sess.state.lock().unwrap();
                        let seen = state
                            .seen_completed_agents_by_batch
                            .entry(batch_id.clone())
                            .or_default();

                        // Find the first completed agent that we haven't returned yet
                        if let Some(unseen) = completed_agents
                            .iter()
                            .find(|a| !seen.contains(&a.id))
                            .cloned()
                        {
                            // Record as seen and return immediately
                            seen.insert(unseen.id.clone());
                            drop(state);

                            // Include output/error preview for the unseen completed agent
                            // Avoid holding manager lock during filesystem I/O
                            drop(manager);
                            let cwd = sess.get_cwd().to_path_buf();
                            let dir = match ensure_agent_dir(&cwd, &unseen.id) {
                                Ok(d) => d,
                                Err(e) => {
                                    return ResponseInputItem::FunctionCallOutput {
                                        call_id: call_id_clone,
                                        output: FunctionCallOutputPayload {
                                            content: format!("Failed to prepare agent output dir: {}", e),
                                            success: Some(false),
                                        },
                                    };
                                }
                            };
                            let (preview_key, file_key, preview, file_path, total_lines) = match unseen.status {
                                AgentStatus::Completed => {
                                    let text = unseen.result.clone().unwrap_or_default();
                                    let (p, total) = preview_first_n_lines(&text, 500);
                                    let fp = write_agent_file(&dir, "result.txt", &text)
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_else(|e| format!("Failed to write result file: {}", e));
                                    ("output_preview", "output_file", p, fp, total)
                                }
                                AgentStatus::Failed => {
                                    let text = unseen.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                                    let (p, total) = preview_first_n_lines(&text, 500);
                                    let fp = write_agent_file(&dir, "error.txt", &text)
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_else(|e| format!("Failed to write error file: {}", e));
                                    ("error_preview", "error_file", p, fp, total)
                                }
                                AgentStatus::Cancelled => {
                                    let text = "Agent cancelled".to_string();
                                    let (p, total) = preview_first_n_lines(&text, 500);
                                    let fp = write_agent_file(&dir, "status.txt", &text)
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_else(|e| format!("Failed to write status file: {}", e));
                                    ("status_preview", "status_file", p, fp, total)
                                }
                                _ => unreachable!(),
                            };

                            let hint = format!(
                                "agent {{\"action\":\"result\",\"result\":{{\"agent_id\":\"{}\",\"batch_id\":\"{}\"}}}}",
                                unseen.id,
                                batch_id
                            );
                            let mut response = serde_json::json!({
                                "agent_id": unseen.id,
                                "status": unseen.status,
                                "wait_time_seconds": start.elapsed().as_secs(),
                                "total_lines": total_lines,
                                "agent_result_hint": hint,
                                "agent_result_params": { "action": "result", "result": { "agent_id": unseen.id, "batch_id": batch_id } },
                            });
                            if let Some(obj) = response.as_object_mut() {
                                obj.insert(preview_key.to_string(), serde_json::Value::String(preview));
                                obj.insert(file_key.to_string(), serde_json::Value::String(file_path));
                            }
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload {
                                    content: response.to_string(),
                                    success: Some(true),
                                },
                            };
                        }

                        // If all agents in the batch are terminal and all have been seen, return immediately
                        if !any_in_progress && !completed_agents.is_empty() {
                            // Mark all as seen to keep state consistent
                            for a in &completed_agents {
                                seen.insert(a.id.clone());
                            }
                            drop(state);

                            let response = serde_json::json!({
                                "batch_id": batch_id,
                                "status": "no_agents_remaining",
                                "wait_time_seconds": start.elapsed().as_secs(),
                            });
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload {
                                    content: response.to_string(),
                                    success: Some(true),
                                },
                            };
                        }
                    }
                }

                drop(manager);
                let (current_epoch, reason) = sess.wait_interrupt_snapshot();
                if current_epoch != initial_wait_epoch {
                    let message = match reason {
                        Some(WaitInterruptReason::UserMessage) => {
                            "wait ended due to new user message".to_string()
                        }
                        _ => "wait ended because the session was interrupted".to_string(),
                    };
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: message,
                            success: Some(false),
                        },
                    };
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
                }
                Err(e) => ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: format!("Invalid agent arguments for action=wait: {}", e),
                        success: None,
                    },
                },
            }
        },
    ).await
}

async fn handle_list_agents(
    sess: &Session,
    ctx: &ToolCallCtx,
    arguments: String,
    event_payload: serde_json::Value,
) -> ResponseInputItem {
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();
    execute_custom_tool(
        sess,
        ctx,
        "agent".to_string(),
        Some(event_payload),
        || async move {
    match serde_json::from_str::<ListAgentsParams>(&arguments_clone) {
        Ok(params) => {
            let manager = AGENT_MANAGER.read().await;

            let batch_id = match params.batch_id.clone() {
                Some(batch) if !batch.trim().is_empty() => batch,
                _ => {
                    return ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: "action=list requires 'list.batch_id'".to_string(),
                            success: Some(false),
                        },
                    };
                }
            };

            let status_filter =
                params
                    .status_filter
                    .and_then(|s| match s.to_lowercase().as_str() {
                        "pending" => Some(AgentStatus::Pending),
                        "running" => Some(AgentStatus::Running),
                        "completed" => Some(AgentStatus::Completed),
                        "failed" => Some(AgentStatus::Failed),
                        "cancelled" => Some(AgentStatus::Cancelled),
                        _ => None,
                    });

            let agents = manager.list_agents(
                status_filter,
                Some(batch_id.clone()),
                params.recent_only.unwrap_or(false),
            );

            // Count running agents for status update
            let running_count = agents
                .iter()
                .filter(|a| a.status == AgentStatus::Running)
                .count();
            if running_count > 0 {
                let status_msg = format!(
                    "🤖 {} agent{} currently running",
                    running_count,
                    if running_count != 1 { "s" } else { "" }
                );
                let event = sess.make_event(
                    "agent-status",
                    EventMsg::BackgroundEvent(BackgroundEventEvent { message: status_msg }),
                );
                let _ = sess.tx_event.send(event).await;
            }

            // Add status counts to summary
            let pending_count = agents
                .iter()
                .filter(|a| a.status == AgentStatus::Pending)
                .count();
            let running_count = agents
                .iter()
                .filter(|a| a.status == AgentStatus::Running)
                .count();
            let completed_count = agents
                .iter()
                .filter(|a| a.status == AgentStatus::Completed)
                .count();
            let failed_count = agents
                .iter()
                .filter(|a| a.status == AgentStatus::Failed)
                .count();
            let cancelled_count = agents
                .iter()
                .filter(|a| a.status == AgentStatus::Cancelled)
                .count();

            let summary = serde_json::json!({
                "total_agents": agents.len(),
                "status_counts": {
                    "pending": pending_count,
                    "running": running_count,
                    "completed": completed_count,
                    "failed": failed_count,
                    "cancelled": cancelled_count,
                },
                "batch_id": batch_id,
                "agents": agents.iter().map(|t| {
                    serde_json::json!({
                        "id": t.id,
                        "name": t.name.clone(),
                        "model": t.model,
                        "status": t.status,
                        "created_at": t.created_at,
                        "batch_id": t.batch_id,
                        "worktree_path": t.worktree_path,
                        "branch_name": t.branch_name,
                    })
                }).collect::<Vec<_>>(),
            });

            ResponseInputItem::FunctionCallOutput {
                call_id: call_id_clone,
                output: FunctionCallOutputPayload {
                    content: summary.to_string(),
                    success: Some(true),
                },
            }
        }
        Err(e) => ResponseInputItem::FunctionCallOutput {
            call_id: call_id_clone,
            output: FunctionCallOutputPayload {
                content: format!("Invalid agent arguments for action=list: {}", e),
                success: None,
            },
        },
    }
        },
    ).await
}

async fn handle_container_exec_with_params(
    params: ExecParams,
    sess: &Session,
    turn_diff_tracker: &mut TurnDiffTracker,
    sub_id: String,
    call_id: String,
    seq_hint: Option<u64>,
    output_index: Option<u32>,
    attempt_req: u64,
) -> ResponseInputItem {
    // Intercept risky git commands and require an explicit confirm prefix.
    // We support a simple convention: prefix the script with `confirm:` to proceed.
    // The prefix is stripped before execution.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    enum SensitiveGitKind {
        BranchChange,
        PathCheckout,
        Reset,
        Revert,
    }

    fn detect_sensitive_git(script: &str) -> Option<SensitiveGitKind> {
        // Goal: detect sensitive git invocations (branch changes, resets) while
        // avoiding false positives from commit messages or other quoted strings.
        // We do a lightweight scan that strips quoted regions before token analysis.

        // 1) Strip quote characters but preserve content inside quotes, while
        // neutralizing control separators to avoid over-splitting tokens.
        let mut cleaned = String::with_capacity(script.len());
        let mut in_squote = false;
        let mut in_dquote = false;
        let mut prev_was_backslash = false;
        for ch in script.chars() {
            let mut emit_space = false;
            match ch {
                '\\' => {
                    // Track escapes inside double quotes; in single quotes, backslash has no special meaning in POSIX sh.
                    prev_was_backslash = !prev_was_backslash;
                }
                '\'' if !in_dquote => {
                    in_squote = !in_squote;
                    emit_space = true; // token boundary at quote edges
                    prev_was_backslash = false;
                }
                '"' if !in_squote && !prev_was_backslash => {
                    in_dquote = !in_dquote;
                    emit_space = true; // token boundary at quote edges
                    prev_was_backslash = false;
                }
                _ => {
                    prev_was_backslash = false;
                }
            }
            if emit_space {
                cleaned.push(' ');
                continue;
            }
            if in_squote || in_dquote {
                if matches!(ch, '|' | '&' | ';' | '\n' | '\r') {
                    cleaned.push(' ');
                } else {
                    cleaned.push(ch);
                }
            } else {
                cleaned.push(ch);
            }
        }

        // 2) Split into simple commands at common separators.
        for chunk in cleaned.split(|c| matches!(c, ';' | '\n' | '\r')) {
            // Further split on conditional operators while keeping order.
            for part in chunk.split(|c| matches!(c, '|' | '&')) {
                let s = part.trim();
                if s.is_empty() { continue; }
                // Tokenize on whitespace, skip wrappers and git globals to find the real subcommand.
                let raw_tokens: Vec<&str> = s.split_whitespace().collect();
                if raw_tokens.is_empty() { continue; }
                fn strip_tok(t: &str) -> &str { t.trim_matches(|c| matches!(c, '(' | ')' | '{' | '}' | '\'' | '"')) }
                let mut i = 0usize;
                // Skip env assignments and lightweight wrappers/keywords.
                loop {
                    if i >= raw_tokens.len() { break; }
                    let tok = strip_tok(raw_tokens[i]);
                    if tok.is_empty() { i += 1; continue; }
                    // Skip KEY=val assignments.
                    if tok.contains('=') && !tok.starts_with('=') && !tok.starts_with('-') {
                        i += 1; continue;
                    }
                    // Skip simple wrappers and control keywords.
                    if matches!(tok, "env" | "sudo" | "command" | "time" | "nohup" | "nice" | "then" | "do" | "{" | "(") {
                        // Best-effort: skip immediate option-like flags after some wrappers.
                        i += 1;
                        while i < raw_tokens.len() {
                            let peek = strip_tok(raw_tokens[i]);
                            if peek.starts_with('-') { i += 1; } else { break; }
                        }
                        continue;
                    }
                    break;
                }
                if i >= raw_tokens.len() { continue; }
                let cmd = strip_tok(raw_tokens[i]);
                let is_git = cmd.ends_with("/git") || cmd == "git";
                if !is_git { continue; }
                i += 1; // advance past git
                // Skip git global options to find the real subcommand.
                while i < raw_tokens.len() {
                    let t = strip_tok(raw_tokens[i]);
                    if t.is_empty() { i += 1; continue; }
                    if matches!(t, "-C" | "--git-dir" | "--work-tree" | "-c") {
                        i += 1; // skip option key
                        if i < raw_tokens.len() { i += 1; } // skip its value
                        continue;
                    }
                    if t.starts_with("--git-dir=") || t.starts_with("--work-tree=") || t.starts_with("-c") {
                        i += 1; continue;
                    }
                    if t.starts_with('-') { i += 1; continue; }
                    break;
                }
                if i >= raw_tokens.len() { continue; }
                let sub = strip_tok(raw_tokens[i]);
                i += 1;
                match sub {
                    "checkout" => {
                        let args: Vec<&str> = raw_tokens[i..].iter().map(|t| strip_tok(t)).collect();
                        let has_path_delimiter = args.iter().any(|a| *a == "--");
                        if has_path_delimiter {
                            return Some(SensitiveGitKind::PathCheckout);
                        }

                        // If any of the strong branch-changing flags are present, flag it.
                        let mut saw_branch_change_flag = false;
                        for a in &args {
                            if matches!(*a, "-b" | "-B" | "--orphan" | "--detach") {
                                saw_branch_change_flag = true;
                                break;
                            }
                        }
                        if saw_branch_change_flag { return Some(SensitiveGitKind::BranchChange); }

                        // `git checkout -` switches to previous branch.
                        if args.first().copied() == Some("-") {
                            return Some(SensitiveGitKind::BranchChange);
                        }

                        // Heuristic: a single non-flag argument likely denotes a branch.
                        if let Some(first_arg) = args.first() {
                            let a = *first_arg;
                            if !a.starts_with('-') && a != "." && a != ".." {
                                return Some(SensitiveGitKind::BranchChange);
                            }
                        }
                    }
                    "switch" => {
                        // `git switch -c <name>` creates; `git switch <name>` changes.
                        let mut saw_c = false;
                        let mut saw_detach = false;
                        let mut first_non_flag: Option<&str> = None;
                        for a in &raw_tokens[i..] {
                            let a = strip_tok(a);
                            if a == "-c" { saw_c = true; break; }
                            if a == "--detach" { saw_detach = true; break; }
                            if a.starts_with('-') { continue; }
                            first_non_flag = Some(a);
                            break;
                        }
                        if saw_c || saw_detach || first_non_flag.is_some() { return Some(SensitiveGitKind::BranchChange); }
                    }
                    "reset" => {
                        // Any form of git reset is considered sensitive.
                        return Some(SensitiveGitKind::Reset);
                    }
                    "revert" => {
                        // Any form of git revert is considered sensitive.
                        return Some(SensitiveGitKind::Revert);
                    }
                    // Future: consider `git branch -D/-m` as branch‑modifying, but keep
                    // this minimal to avoid over‑blocking normal workflows.
                    _ => {}
                }
            }
        }
        None
    }

    fn strip_leading_confirm_prefix(argv: &mut Vec<String>) -> bool {
        if argv.is_empty() {
            return false;
        }

        let first = argv[0].trim().to_string();
        for prefix in ["confirm:", "CONFIRM:"] {
            if first == prefix {
                argv.remove(0);
                return true;
            }
            if let Some(rest) = first.strip_prefix(prefix) {
                let trimmed = rest.trim_start();
                if trimmed.is_empty() {
                    argv.remove(0);
                } else {
                    argv[0] = trimmed.to_string();
                }
                return true;
            }
        }

        false
    }

    fn guidance_for_sensitive_git(kind: SensitiveGitKind, original_label: &str, original_value: &str, suggested: &str) -> String {
        match kind {
            SensitiveGitKind::BranchChange => format!(
                "Blocked git checkout/switch on a branch. Switching branches can discard or hide in-progress changes. Only continue if the user explicitly requested this branch change. Resend with 'confirm:' if you intend to proceed.\n\n{}: {}\nresend_exact_argv: {}",
                original_label,
                original_value,
                suggested
            ),
            SensitiveGitKind::PathCheckout => format!(
                "Blocked git checkout -- <paths>. This command overwrites local modifications to the specified files. Consider backing up the files first. If you intentionally want to discard those edits, resend the exact command prefixed with 'confirm:'.\n\n{}: {}\nresend_exact_argv: {}",
                original_label,
                original_value,
                suggested
            ),
            SensitiveGitKind::Reset => format!(
                "Blocked git reset. Reset rewrites the working tree/index and may delete local work. Consider backing up the files first. If backups exist and this was explicitly requested, resend prefixed with 'confirm:'.\n\n{}: {}\nresend_exact_argv: {}",
                original_label,
                original_value,
                suggested
            ),
            SensitiveGitKind::Revert => format!(
                "Blocked git revert. Reverting commits alters history and should only happen when the user asks for it. If that’s the case, resend the command with 'confirm:'.\n\n{}: {}\nresend_exact_argv: {}",
                original_label,
                original_value,
                suggested
            ),
        }
    }

    fn guidance_for_dry_run_guard(
        analysis: &DryRunAnalysis,
        original_label: &str,
        original_value: &str,
        resend_exact_argv: Vec<String>,
    ) -> String {
        let suggested_confirm = serde_json::to_string(&resend_exact_argv)
            .unwrap_or_else(|_| "<failed to serialize suggested argv>".to_string());
        let suggested_dry_run = analysis
            .suggested_dry_run()
            .unwrap_or_else(|| "<no canonical dry-run variant; remove mutating flags or use confirm:>".to_string());
        format!(
            "Blocked {} without a prior dry run. Run the dry-run variant first or resend with 'confirm:' if explicitly requested.\n\n{}: {}\nresend_exact_argv: {}\nsuggested_dry_run: {}",
            analysis.display_name(),
            original_label,
            original_value,
            suggested_confirm,
            suggested_dry_run
        )
    }


    // If the argv is a shell wrapper, analyze and optionally strip `confirm:`.
    let mut params = params;
    let seq_hint_for_exec = seq_hint;
    let otel_event_manager = sess.client.get_otel_event_manager();
    let tool_name = "local_shell";
    if let Some((script_index, script)) = extract_shell_script_from_wrapper(&params.command) {
        let trimmed = script.trim_start();
        let confirm_prefixes = ["confirm:", "CONFIRM:"];
        let has_confirm_prefix = confirm_prefixes
            .iter()
            .any(|p| trimmed.starts_with(p));

        // If no confirm prefix and it looks like a sensitive git command, reject with guidance.
        if !has_confirm_prefix {
            if let Some(pattern) = if sess.confirm_guard.is_empty() {
                None
            } else {
                sess.confirm_guard.matched_pattern(trimmed)
            } {
                let mut argv_confirm = params.command.clone();
                argv_confirm[script_index] = format!("confirm: {}", script.trim_start());
                let suggested = serde_json::to_string(&argv_confirm)
                    .unwrap_or_else(|_| "<failed to serialize suggested argv>".to_string());
                let guidance = pattern.guidance("original_script", &script, &suggested);

                let order = sess.next_background_order(&sub_id, attempt_req, output_index);
                sess
                    .notify_background_event_with_order(
                        &sub_id,
                        order,
                        format!("Command guard: {}", guidance),
                    )
                    .await;

                return ResponseInputItem::FunctionCallOutput {
                    call_id,
                    output: FunctionCallOutputPayload { content: guidance, success: None },
                };
            }

            if let Some(kind) = detect_sensitive_git(trimmed) {
                // Provide the exact argv the model should resend with the confirm prefix.
                let mut argv_confirm = params.command.clone();
                argv_confirm[script_index] = format!("confirm: {}", script.trim_start());
                let suggested = serde_json::to_string(&argv_confirm)
                    .unwrap_or_else(|_| "<failed to serialize suggested argv>".to_string());

                let guidance = guidance_for_sensitive_git(kind, "original_script", &script, &suggested);

                let order = sess.next_background_order(&sub_id, attempt_req, output_index);
                sess
                    .notify_background_event_with_order(
                        &sub_id,
                        order,
                        format!("Command guard: {}", guidance.clone()),
                    )
                    .await;

                return ResponseInputItem::FunctionCallOutput {
                    call_id,
                    output: FunctionCallOutputPayload { content: guidance, success: None },
                };
            }
        }

        // If confirm prefix present, strip it before execution.
        if has_confirm_prefix {
            let without_prefix = confirm_prefixes
                .iter()
                .find_map(|p| {
                    let t = trimmed.strip_prefix(p)?;
                    Some(t.trim_start().to_string())
                })
                .unwrap_or_else(|| trimmed.to_string());
            params.command[script_index] = without_prefix;
        }

        let dry_run_analysis = analyze_command(&params.command);
        if !has_confirm_prefix {
            if let Some(analysis) = dry_run_analysis.as_ref() {
                if analysis.disposition == DryRunDisposition::Mutating {
                    let needs_dry_run = {
                        let state = sess.state.lock().unwrap();
                        !state.dry_run_guard.has_recent_dry_run(analysis.key)
                    };
                    if needs_dry_run {
                        let mut argv_confirm = params.command.clone();
                        argv_confirm[script_index] = format!("confirm: {}", params.command[script_index].trim_start());
                        let guidance = guidance_for_dry_run_guard(
                            analysis,
                            "original_script",
                            &params.command[script_index],
                            argv_confirm,
                        );

                        let order = sess.next_background_order(&sub_id, attempt_req, output_index);
                        sess
                            .notify_background_event_with_order(
                                &sub_id,
                                order,
                                format!("Command guard: {}", guidance.clone()),
                            )
                            .await;

                        return ResponseInputItem::FunctionCallOutput {
                            call_id,
                            output: FunctionCallOutputPayload { content: guidance, success: None },
                        };
                    }
                }
            }
        }
    }

    strip_leading_confirm_prefix(&mut params.command);

    if let Some(redundant) = detect_redundant_cd(&params.command, &params.cwd) {
        let guidance = guidance_for_redundant_cd(&redundant);
        let order = sess.next_background_order(&sub_id, attempt_req, output_index);
        sess
            .notify_background_event_with_order(
                &sub_id,
                order,
                format!("Command guard: {}", guidance.clone()),
            )
            .await;

        return ResponseInputItem::FunctionCallOutput {
            call_id,
            output: FunctionCallOutputPayload {
                content: guidance,
                success: None,
            },
        };
    }

    if let Some(cat_guard) = detect_cat_write(&params.command) {
        let guidance = guidance_for_cat_write(&cat_guard);
        let order = sess.next_background_order(&sub_id, attempt_req, output_index);
        sess
            .notify_background_event_with_order(
                &sub_id,
                order,
                format!("Command guard: {}", guidance.clone()),
            )
            .await;

        return ResponseInputItem::FunctionCallOutput {
            call_id,
            output: FunctionCallOutputPayload {
                content: guidance,
                success: None,
            },
        };
    }

    if let Some(python_guard) = detect_python_write(&params.command) {
        let guidance = guidance_for_python_write(&python_guard);
        let order = sess.next_background_order(&sub_id, attempt_req, output_index);
        sess
            .notify_background_event_with_order(
                &sub_id,
                order,
                format!("Command guard: {}", guidance.clone()),
            )
            .await;

        return ResponseInputItem::FunctionCallOutput {
            call_id,
            output: FunctionCallOutputPayload {
                content: guidance,
                success: None,
            },
        };
    }

    // If no shell wrapper, perform a lightweight argv inspection for sensitive git commands.
    if extract_shell_script_from_wrapper(&params.command).is_none() {
        let joined = params.command.join(" ");
        if !sess.confirm_guard.is_empty() {
            if let Some(pattern) = sess.confirm_guard.matched_pattern(&joined) {
                let suggested = serde_json::to_string(&vec![
                    "bash".to_string(),
                    "-lc".to_string(),
                    format!("confirm: {}", joined),
                ])
                .unwrap_or_else(|_| "<failed to serialize suggested argv>".to_string());
                let guidance = pattern.guidance(
                    "original_argv",
                    &format!("{:?}", params.command),
                    &suggested,
                );

                let order = sess.next_background_order(&sub_id, attempt_req, output_index);
                sess
                    .notify_background_event_with_order(
                        &sub_id,
                        order,
                        format!("Command guard: {}", guidance.clone()),
                    )
                    .await;

                return ResponseInputItem::FunctionCallOutput {
                    call_id,
                    output: FunctionCallOutputPayload { content: guidance, success: None },
                };
            }
        }

        if let Some(analysis) = analyze_command(&params.command) {
            if analysis.disposition == DryRunDisposition::Mutating {
                let needs_dry_run = {
                    let state = sess.state.lock().unwrap();
                    !state.dry_run_guard.has_recent_dry_run(analysis.key)
                };
                if needs_dry_run {
                    let resend = vec![
                        "bash".to_string(),
                        "-lc".to_string(),
                        format!("confirm: {}", joined),
                    ];
                    let guidance = guidance_for_dry_run_guard(
                        &analysis,
                        "original_argv",
                        &format!("{:?}", params.command),
                        resend,
                    );

                    let order = sess.next_background_order(&sub_id, attempt_req, output_index);
                    sess
                        .notify_background_event_with_order(
                            &sub_id,
                            order,
                            format!("Command guard: {}", guidance.clone()),
                        )
                        .await;

                    return ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload { content: guidance, success: None },
                    };
                }
            }
        }

        fn strip_tok2(t: &str) -> &str { t.trim_matches(|c| matches!(c, '(' | ')' | '{' | '}' | '\'' | '"')) }
        let mut i = 0usize;
        // Skip env assignments and simple wrappers at the front
        while i < params.command.len() {
            let tok = strip_tok2(&params.command[i]);
            if tok.is_empty() { i += 1; continue; }
            if tok.contains('=') && !tok.starts_with('=') && !tok.starts_with('-') { i += 1; continue; }
            if matches!(tok, "env" | "sudo" | "command" | "time" | "nohup" | "nice") {
                i += 1;
                while i < params.command.len() && strip_tok2(&params.command[i]).starts_with('-') { i += 1; }
                continue;
            }
            break;
        }
        if i < params.command.len() {
            let cmd = strip_tok2(&params.command[i]);
            if cmd.ends_with("/git") || cmd == "git" {
                i += 1;
                while i < params.command.len() {
                    let t = strip_tok2(&params.command[i]);
                    if t.is_empty() { i += 1; continue; }
                    if matches!(t, "-C" | "--git-dir" | "--work-tree" | "-c") {
                        i += 1; if i < params.command.len() { i += 1; }
                        continue;
                    }
                    if t.starts_with("--git-dir=") || t.starts_with("--work-tree=") || t.starts_with("-c") { i += 1; continue; }
                    if t.starts_with('-') { i += 1; continue; }
                    break;
                }
                if i < params.command.len() {
                    let sub = strip_tok2(&params.command[i]);
                    let args: Vec<&str> = params.command[i + 1..].iter().map(|t| strip_tok2(t)).collect();
                    let kind = match sub {
                        "checkout" => {
                            if args.iter().any(|a| *a == "--") {
                                Some(SensitiveGitKind::PathCheckout)
                            } else if args.iter().any(|a| matches!(*a, "-b" | "-B" | "--orphan" | "--detach")) {
                                Some(SensitiveGitKind::BranchChange)
                            } else if args.first().copied() == Some("-") {
                                Some(SensitiveGitKind::BranchChange)
                            } else if let Some(first_arg) = args.first() {
                                let a = *first_arg;
                                if !a.starts_with('-') && a != "." && a != ".." {
                                    Some(SensitiveGitKind::BranchChange)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        "switch" => Some(SensitiveGitKind::BranchChange),
                        "reset" => Some(SensitiveGitKind::Reset),
                        "revert" => Some(SensitiveGitKind::Revert),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        let suggested = serde_json::to_string(&vec![
                            "bash".to_string(),
                            "-lc".to_string(),
                            format!("confirm: {}", params.command.join(" ")),
                        ]).unwrap_or_else(|_| "<failed to serialize suggested argv>".to_string());

                        let guidance = guidance_for_sensitive_git(kind, "original_argv", &format!("{:?}", params.command), &suggested);

                        let order = sess.next_background_order(&sub_id, attempt_req, output_index);
                        sess
                            .notify_background_event_with_order(
                                &sub_id,
                                order,
                                format!("Command guard: {}", guidance.clone()),
                            )
                            .await;

                        return ResponseInputItem::FunctionCallOutput { call_id, output: FunctionCallOutputPayload { content: guidance, success: None } };
                    }
                }
            }
        }
    }

    // Check if this was a patch, and apply it in-process if so.
    match sess
        .maybe_parse_apply_patch_verified(&params.command, &params.cwd)
        .await
    {
        MaybeApplyPatchVerified::Body(action) => {
            if let Some(branch_root) = git_worktree::branch_worktree_root(sess.get_cwd()) {
                if let Some(guidance) = guard_apply_patch_outside_branch(&branch_root, &action) {
                    let order = sess.next_background_order(&sub_id, attempt_req, output_index);
                    sess
                        .notify_background_event_with_order(
                            &sub_id,
                            order,
                            format!("Command guard: {}", guidance.clone()),
                        )
                        .await;

                    return ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload { content: guidance, success: None },
                    };
                }
            }
            let changes = convert_apply_patch_to_protocol(&action);
            turn_diff_tracker.on_patch_begin(&changes);

            match apply_patch::apply_patch(
                sess,
                &sub_id,
                &call_id,
                attempt_req,
                output_index,
                action,
            )
            .await
            {
                ApplyPatchResult::Reply(item) => return item,
                ApplyPatchResult::Applied(run) => {
                    let order_begin = crate::protocol::OrderMeta {
                        request_ordinal: attempt_req,
                        output_index,
                        sequence_number: seq_hint,
                    };
                    let begin_event = EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                        call_id: call_id.clone(),
                        auto_approved: run.auto_approved,
                        changes,
                    });
                    let event = sess.make_event_with_order(&sub_id, begin_event, order_begin, seq_hint);
                    let _ = sess.tx_event.send(event).await;

                    let order_end = crate::protocol::OrderMeta {
                        request_ordinal: attempt_req,
                        output_index,
                        sequence_number: seq_hint.map(|h| h.saturating_add(1)),
                    };
                    let end_event = EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                        call_id: call_id.clone(),
                        stdout: run.stdout.clone(),
                        stderr: run.stderr.clone(),
                        success: run.success,
                    });
                    let event = sess.make_event_with_order(
                        &sub_id,
                        end_event,
                        order_end,
                        seq_hint.map(|h| h.saturating_add(1)),
                    );
                    let _ = sess.tx_event.send(event).await;

                    if let Ok(Some(unified_diff)) = turn_diff_tracker.get_unified_diff() {
                        let diff_event = sess.make_event(
                            &sub_id,
                            EventMsg::TurnDiff(TurnDiffEvent { unified_diff }),
                        );
                        let _ = sess.tx_event.send(diff_event).await;
                    }

                    let mut content = run.stdout;
                    if !run.success && !run.stderr.is_empty() {
                        if !content.is_empty() {
                            content.push('\n');
                        }
                        content.push_str(&format!("stderr: {}", run.stderr));
                    }
                    if let Some(summary) = run.harness_summary_json {
                        if !summary.is_empty() {
                            if !content.is_empty() {
                                content.push('\n');
                            }
                            content.push_str(&summary);
                        }
                    }

                    return ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload {
                            content,
                            success: Some(run.success),
                        },
                    };
                }
            }
        }
        MaybeApplyPatchVerified::CorrectnessError(parse_error) => {
            return ResponseInputItem::FunctionCallOutput {
                call_id,
                output: FunctionCallOutputPayload {
                    content: format!("error: {parse_error:#}"),
                    success: None,
                },
            };
        }
        MaybeApplyPatchVerified::ShellParseError(error) => {
            trace!("Failed to parse shell command, {error:?}");
        }
        MaybeApplyPatchVerified::NotApplyPatch => {}
    }

    let safety = {
        let state = sess.state.lock().unwrap();
        assess_command_safety(
            &params.command,
            sess.approval_policy,
            &sess.sandbox_policy,
            &state.approved_commands,
            params.with_escalated_permissions.unwrap_or(false),
        )
    };
    let command_for_display = params.command.clone();
    let harness_summary_json: Option<String> = None;

    let sandbox_type = match safety {
        SafetyCheck::AutoApprove {
            sandbox_type,
            user_explicitly_approved,
        } => {
            if let Some(manager) = otel_event_manager.as_ref() {
                let (decision_for_log, source) = if user_explicitly_approved {
                    (
                        ReviewDecision::ApprovedForSession,
                        ToolDecisionSource::User,
                    )
                } else {
                    (ReviewDecision::Approved, ToolDecisionSource::Config)
                };
                manager.tool_decision(
                    tool_name,
                    call_id.as_str(),
                    to_proto_review_decision(decision_for_log),
                    source,
                );
            }
            sandbox_type
        }
        SafetyCheck::AskUser => {
            let rx_approve = sess
                .request_command_approval(
                    sub_id.clone(),
                    call_id.clone(),
                    params.command.clone(),
                    params.cwd.clone(),
                    params.justification.clone(),
                )
                .await;

            let decision = rx_approve.await.unwrap_or_default();
            if let Some(manager) = otel_event_manager.as_ref() {
                manager.tool_decision(
                    tool_name,
                    call_id.as_str(),
                    to_proto_review_decision(decision),
                    ToolDecisionSource::User,
                );
            }

            match decision {
                ReviewDecision::Approved => {}
                ReviewDecision::ApprovedForSession => {
                    sess.add_approved_command(ApprovedCommandPattern::new(
                        params.command.clone(),
                        ApprovedCommandMatchKind::Exact,
                        None,
                    ));
                }
                ReviewDecision::Denied | ReviewDecision::Abort => {
                    return ResponseInputItem::FunctionCallOutput {
                        call_id,
                        output: FunctionCallOutputPayload {
                            content: "exec command rejected by user".to_string(),
                            success: None,
                        },
                    };
                }
            }
            // No sandboxing is applied because the user has given
            // explicit approval. Often, we end up in this case because
            // the command cannot be run in a sandbox, such as
            // installing a new dependency that requires network access.
            SandboxType::None
        }
        SafetyCheck::Reject { reason } => {
            return ResponseInputItem::FunctionCallOutput {
                call_id,
                output: FunctionCallOutputPayload {
                    content: format!("exec command rejected: {reason}"),
                    success: None,
                },
            };
        }
    };

    let exec_command_context = ExecCommandContext {
        sub_id: sub_id.clone(),
        call_id: call_id.clone(),
        command_for_display: command_for_display.clone(),
        cwd: params.cwd.clone(),
        apply_patch: None,
    };

    let display_label = crate::util::strip_bash_lc_and_escape(&exec_command_context.command_for_display);
    let params = maybe_run_with_user_profile(params, sess);

    // Prepare tail buffer and background registry entry
    let tail_buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let notify = std::sync::Arc::new(tokio::sync::Notify::new());
    let result_cell: std::sync::Arc<std::sync::Mutex<Option<ExecToolCallOutput>>> = std::sync::Arc::new(std::sync::Mutex::new(None));
    let backgrounded = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let suppress_event_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let order_meta_for_end = crate::protocol::OrderMeta {
        request_ordinal: attempt_req,
        output_index,
        sequence_number: seq_hint_for_exec.map(|h| h.saturating_add(1)),
    };
    let order_meta_for_deltas = crate::protocol::OrderMeta {
        request_ordinal: attempt_req,
        output_index,
        sequence_number: None,
    };
    {
        let mut st = sess.state.lock().unwrap();
        st.background_execs.insert(
            call_id.clone(),
            BackgroundExecState {
                notify: notify.clone(),
                result_cell: result_cell.clone(),
                tail_buf: Some(tail_buf.clone()),
                cmd_display: display_label.clone(),
                suppress_event: suppress_event_flag.clone(),
                task_handle: None,
                order_meta_for_end: order_meta_for_end.clone(),
                sub_id: sub_id.clone(),
            },
        );
    }

    // Emit BEGIN event using the normal path so the TUI shows a running cell
    sess
        .on_exec_command_begin(
            turn_diff_tracker,
            exec_command_context.clone(),
            seq_hint_for_exec,
            output_index,
            attempt_req,
        )
        .await;

    // Spawn the runner that streams output and, on completion, emits END and records result.
    let tx_event = sess.tx_event.clone();
    let sub_id_for_events = sub_id.clone();
    let call_id_for_events = call_id.clone();
    let sandbox_policy = sess.sandbox_policy.clone();
    let sandbox_cwd = sess.get_cwd().to_path_buf();
    let code_linux_sandbox_exe = sess.code_linux_sandbox_exe.clone();
    let result_cell_for_task = result_cell.clone();
    let notify_task = notify.clone();
    let tail_buf_task = tail_buf.clone();
    let backgrounded_task = backgrounded.clone();
    let suppress_event_flag_task = suppress_event_flag.clone();
    let display_label_task = display_label.clone();
    let task_handle = tokio::spawn(async move {
        // Build stdout stream with tail capture. We cannot stamp via `Session` here,
        // but deltas will be delivered with neutral ordering which the UI tolerates.
        let stdout_stream = if exec_command_context.apply_patch.is_some() {
            None
        } else {
            Some(StdoutStream {
                sub_id: sub_id_for_events.clone(),
                call_id: call_id_for_events.clone(),
                tx_event: tx_event.clone(),
                session: None,
                tail_buf: Some(tail_buf_task.clone()),
                order: Some(order_meta_for_deltas.clone()),
            })
        };

        let start = std::time::Instant::now();
        let res = crate::exec::process_exec_tool_call(
            params.clone(),
            sandbox_type,
            &sandbox_policy,
            &sandbox_cwd,
            &code_linux_sandbox_exe,
            stdout_stream,
        )
        .await;

        // Normalize to ExecToolCallOutput
        let (out, exit_code) = match res {
            Ok(o) => { let exit = o.exit_code; (o, exit) },
            Err(CodexErr::Sandbox(SandboxErr::Timeout { output })) => (output.as_ref().clone(), 124),
            Err(e) => {
                let msg = get_error_message_ui(&e);
                (
                    ExecToolCallOutput {
                        exit_code: -1,
                        stdout: StreamOutput::new(String::new()),
                        stderr: StreamOutput::new(msg.clone()),
                        aggregated_output: StreamOutput::new(msg),
                        duration: start.elapsed(),
                        timed_out: false,
                    },
                    -1,
                )
            }
        };

        // Emit END event directly
        let end_msg = EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: call_id_for_events.clone(),
            stdout: out.stdout.text.clone(),
            stderr: out.stderr.text.clone(),
            exit_code,
            duration: out.duration,
        });
        let ev = Event { id: sub_id_for_events.clone(), event_seq: 0, msg: end_msg, order: Some(order_meta_for_end) };
        let _ = tx_event.send(ev).await;

        // Store result for waiters
        {
            let mut slot = result_cell_for_task.lock().unwrap();
            *slot = Some(out.clone());
        }
        // Only emit background completion notifications if the command actually backgrounded
        if backgrounded_task.load(std::sync::atomic::Ordering::Relaxed) {
            if !suppress_event_flag_task.load(std::sync::atomic::Ordering::Relaxed) {
                let label = display_label_task.trim();
                let message = if label.is_empty() {
                    format!("Background shell '{}' completed.", call_id_for_events)
                } else {
                    format!("{label} completed in background")
                };
                let bg_event = EventMsg::BackgroundEvent(BackgroundEventEvent { message });
                let ev = Event { id: sub_id_for_events.clone(), event_seq: 0, msg: bg_event, order: None };
                let _ = tx_event.send(ev).await;

                if let Some(tx) = TX_SUB_GLOBAL.get() {
                    let header_label = if label.is_empty() {
                        format!("call_id={}", call_id_for_events)
                    } else {
                        display_label_task.clone()
                    };
                    let header = format!("Background shell completed ({header_label}), exit_code={}, duration={:?}.", out.exit_code, out.duration);
                    let full_body = format_exec_output_str(&out);
                    let body = truncate_exec_output_for_storage(&sandbox_cwd, &sub_id_for_events, &call_id_for_events, &full_body);
                    let dev_text = format!("{}\n\n{}", header, body);
                    let _ = tx
                        .send(Submission { id: uuid::Uuid::new_v4().to_string(), op: Op::AddPendingInputDeveloper { text: dev_text } })
                        .await;
                }
            }
            if let Some(n) = ANY_BG_NOTIFY.get() { n.notify_waiters(); }
        }
        notify_task.notify_waiters();
    });

    {
        let mut st = sess.state.lock().unwrap();
        if let Some(bg) = st.background_execs.get_mut(&call_id) {
            bg.task_handle = Some(task_handle);
        }
    }

    // Wait up to 10 seconds for completion
    let waited = tokio::time::timeout(std::time::Duration::from_secs(10), notify.notified()).await;
    if waited.is_ok() {
        // Completed within 10s - return the real output and drop the background entry.
        let done_opt = {
            let mut st = sess.state.lock().unwrap();
            st.background_execs
                .remove(&call_id)
                .and_then(|bg| bg.result_cell.lock().unwrap().clone())
                .or_else(|| {
                    st.background_execs
                        .iter()
                        .find_map(|(k, v)| {
                            if v.result_cell.lock().unwrap().is_some() {
                                Some(k.clone())
                            } else {
                                None
                            }
                        })
                        .and_then(|k| st.background_execs.remove(&k))
                        .and_then(|bg| bg.result_cell.lock().unwrap().clone())
                })
        };
        if let Some(done) = done_opt {
            let is_success = done.exit_code == 0;
            let mut content = format_exec_output_with_limit(sess, &sub_id, &call_id, &done);
            if let Some(harness) = harness_summary_json.as_ref() {
                if !harness.is_empty() {
                    content.push('\n');
                    content.push_str(harness);
                }
            }
            return ResponseInputItem::FunctionCallOutput { call_id: call_id.clone(), output: FunctionCallOutputPayload { content, success: Some(is_success) } };
        } else {
            // Fallback (should not happen): indicate completion without detail
            let msg = format!("Command completed.");
            return ResponseInputItem::FunctionCallOutput { call_id: call_id.clone(), output: FunctionCallOutputPayload { content: msg, success: Some(true) } };
        }
    }

    // Still running: mark as backgrounded and return background notice + tail and instructions
    backgrounded.store(true, std::sync::atomic::Ordering::Relaxed);
    let tail = String::from_utf8_lossy(&tail_buf.lock().unwrap()).to_string();
    let header = format!(
        "Command running in background (call_id={}).\nTo wait: wait(call_id=\"{}\")\nYou can continue other work or wait. You'll be notified when the command completes.",
        call_id,
        call_id
    );
    let msg = if tail.is_empty() {
        header
    } else {
        format!("{}\n\nOutput so far (tail):\n{}", header, tail)
    };
    ResponseInputItem::FunctionCallOutput { call_id: call_id.clone(), output: FunctionCallOutputPayload { content: msg, success: Some(true) } }
}

#[allow(dead_code)]
async fn handle_sandbox_error(
    turn_diff_tracker: &mut TurnDiffTracker,
    params: ExecParams,
    exec_command_context: ExecCommandContext,
    error: SandboxErr,
    sandbox_type: SandboxType,
    sess: &Session,
    attempt_req: u64,
) -> ResponseInputItem {
    let call_id = exec_command_context.call_id.clone();
    let sub_id = exec_command_context.sub_id.clone();
    let cwd = exec_command_context.cwd.clone();
    let otel_event_manager = sess.client.get_otel_event_manager();
    let tool_name = "local_shell";

    // Early out if either the user never wants to be asked for approval, or
    // we're letting the model manage escalation requests. Otherwise, continue
    match sess.approval_policy {
        AskForApproval::Never | AskForApproval::OnRequest => {
            // Clarify when Read Only mode is the reason a command cannot proceed.
            let content = if matches!(sess.sandbox_policy, SandboxPolicy::ReadOnly) {
                format!("command blocked by Read Only mode: {error}")
            } else {
                format!("failed in sandbox {sandbox_type:?} with execution error: {error}")
            };
            return ResponseInputItem::FunctionCallOutput {
                call_id,
                output: FunctionCallOutputPayload { content, success: Some(false) },
            };
        }
        AskForApproval::UnlessTrusted | AskForApproval::OnFailure => (),
    }

    // similarly, if the command timed out, we can simply return this failure to the model
    if matches!(error, SandboxErr::Timeout { .. }) {
        return ResponseInputItem::FunctionCallOutput {
            call_id,
            output: FunctionCallOutputPayload {
                content: "command timed out".to_string(),
                success: Some(false),
            },
        };
    }

    // Note that when `error` is `SandboxErr::Denied`, it could be a false
    // positive. That is, it may have exited with a non-zero exit code, not
    // because the sandbox denied it, but because that is its expected behavior,
    // i.e., a grep command that did not match anything. Ideally we would
    // include additional metadata on the command to indicate whether non-zero
    // exit codes merit a retry.

    // For now, we categorically ask the user to retry without sandbox and
    // emit the raw error as a background event.
    let failure_order = sess.next_background_order(&sub_id, attempt_req, None);
    sess
        .notify_background_event_with_order(
            &sub_id,
            failure_order,
            format!("Execution failed: {error}"),
        )
        .await;

    let rx_approve = sess
        .request_command_approval(
            sub_id.clone(),
            call_id.clone(),
            params.command.clone(),
            cwd.clone(),
            Some("command failed; retry without sandbox?".to_string()),
        )
        .await;

    let decision = rx_approve.await.unwrap_or_default();
    if let Some(manager) = otel_event_manager.as_ref() {
        manager.tool_decision(
            tool_name,
            call_id.as_str(),
            to_proto_review_decision(decision),
            ToolDecisionSource::User,
        );
    }

    match decision {
        ReviewDecision::Approved => {}
        ReviewDecision::ApprovedForSession => {
            // Persist this command as pre‑approved for the
            // remainder of the session so future executions skip the sandbox directly.
            sess.add_approved_command(ApprovedCommandPattern::new(
                params.command.clone(),
                ApprovedCommandMatchKind::Exact,
                None,
            ));
        }
        ReviewDecision::Denied | ReviewDecision::Abort => {
            // Fall through to original failure handling.
            return ResponseInputItem::FunctionCallOutput {
                call_id,
                output: FunctionCallOutputPayload {
                    content: "exec command rejected by user".to_string(),
                    success: None,
                },
            };
        }
    };

    // Inform UI we are retrying without sandbox.
    let retry_order = sess.next_background_order(&sub_id, attempt_req, None);
    sess
        .notify_background_event_with_order(
            &sub_id,
            retry_order,
            "retrying command without sandbox",
        )
        .await;

    // This is an escalated retry; the policy will not be examined and the sandbox has been set to `None`.
    // Use the same attempt_req as the tool call that failed; this retry is still part of the current provider attempt.
    let retry_output_result = sess
        .run_exec_with_events(
            turn_diff_tracker,
            exec_command_context.clone(),
            ExecInvokeArgs {
                params,
                sandbox_type: SandboxType::None,
                sandbox_policy: &sess.sandbox_policy,
                sandbox_cwd: sess.get_cwd(),
                code_linux_sandbox_exe: &sess.code_linux_sandbox_exe,
                stdout_stream: if exec_command_context.apply_patch.is_some() {
                    None
                } else {
                    Some(StdoutStream {
                        sub_id: sub_id.clone(),
                        call_id: call_id.clone(),
                        tx_event: sess.tx_event.clone(),
                        session: None,
                        tail_buf: None,
                        order: Some(crate::protocol::OrderMeta { request_ordinal: attempt_req, output_index: None, sequence_number: None }),
                    })
                },
            },
            None,
            None,
            attempt_req,
        )
        .await;

    match retry_output_result {
        Ok(retry_output) => {
            let ExecToolCallOutput { exit_code, .. } = &retry_output;

            let is_success = *exit_code == 0;
            let content = format_exec_output_with_limit(sess, &sub_id, &call_id, &retry_output);

            ResponseInputItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output: FunctionCallOutputPayload {
                    content,
                    success: Some(is_success),
                },
            }
        }
        Err(e) => ResponseInputItem::FunctionCallOutput {
            call_id: call_id.clone(),
            output: FunctionCallOutputPayload {
                content: format!("retry failed: {e}"),
                success: None,
            },
        },
    }
}

// Limit extremely large tool outputs before sending to the model to avoid
// context overflows. Keep this conservative because multiple tool outputs
// can appear in a single turn. The limit is in bytes (on the UTF‑8 string).
const MAX_TOOL_OUTPUT_BYTES_FOR_MODEL: usize = 32 * 1024; // 32 KiB
const TRUNCATION_MARKER: &str = "…truncated…\n";

fn truncate_middle_bytes(s: &str, max_bytes: usize) -> (String, bool, usize, usize) {
    if s.len() <= max_bytes {
        return (s.to_string(), false, s.len(), s.len());
    }
    if max_bytes == 0 {
        return (TRUNCATION_MARKER.trim_end().to_string(), true, 0, s.len());
    }

    // Try to keep some head/tail, favoring newline boundaries when possible.
    let keep = max_bytes.saturating_sub("…truncated…\n".len());
    let left_budget = keep / 2;
    let right_budget = keep - left_budget;

    // Safe prefix end on a char boundary, prefer last newline within budget.
    let prefix_end = {
        let mut end = left_budget.min(s.len());
        if let Some(head) = s.get(..end) {
            if let Some(i) = head.rfind('\n') { end = i + 1; }
        }
        while end > 0 && !s.is_char_boundary(end) { end -= 1; }
        end
    };

    // Safe suffix start on a char boundary, prefer first newline within budget.
    let suffix_start = {
        let mut start = s.len().saturating_sub(right_budget);
        if let Some(tail) = s.get(start..) {
            if let Some(i) = tail.find('\n') { start += i + 1; }
        }
        while start < s.len() && !s.is_char_boundary(start) { start += 1; }
        start
    };

    let mut out = String::with_capacity(max_bytes);
    out.push_str(&s[..prefix_end]);
    out.push_str(TRUNCATION_MARKER);
    out.push_str(&s[suffix_start..]);
    (out, true, prefix_end, suffix_start)
}

fn format_exec_output_str(exec_output: &ExecToolCallOutput) -> String {
    let ExecToolCallOutput {
        aggregated_output,
        ..
    } = exec_output;

    // Always use the aggregated (stdout + stderr interleaved) stream so the
    // model sees the full build log regardless of which stream a tool used.
    let mut formatted_output = aggregated_output.text.clone();
    if let Some(truncated_before_bytes) = aggregated_output.truncated_before_bytes {
        let note = format!(
            "… clipped {} from the start of command output (showing last {}).\n\n",
            format_bytes(truncated_before_bytes),
            format_bytes(EXEC_CAPTURE_MAX_BYTES),
        );
        formatted_output = format!("{note}{formatted_output}");
    }
    if let Some(truncated_after_lines) = aggregated_output.truncated_after_lines {
        formatted_output.push_str(&format!(
            "\n\n[Output truncated after {truncated_after_lines} lines: too many lines or bytes.]",
        ));
    }

    formatted_output
}

fn truncate_exec_output_for_storage(
    cwd: &Path,
    sub_id: &str,
    call_id: &str,
    full: &str,
) -> String {
    let (maybe_truncated, was_truncated, _, _) = truncate_middle_bytes(full, MAX_TOOL_OUTPUT_BYTES_FOR_MODEL);
    if !was_truncated {
        return maybe_truncated;
    }

    let file_note = match ensure_agent_dir(cwd, sub_id)
        .and_then(|dir| write_agent_file(&dir, &format!("exec-{call_id}.txt"), full))
    {
        Ok(path) => format!("\n\n[Full output saved to: {}]", path.display()),
        Err(e) => format!("\n\n[Full output was too large and truncation applied; failed to save file: {e}]")
    };
    let mut truncated = maybe_truncated;
    truncated.push_str(&file_note);
    truncated
}

/// Exec output serialized for the model. If the payload is too large,
/// write the full output to a file and include a truncated preview here.
fn format_exec_output_with_limit(
    sess: &Session,
    sub_id: &str,
    call_id: &str,
    exec_output: &ExecToolCallOutput,
) -> String {
    let ExecToolCallOutput {
        exit_code,
        duration,
        ..
    } = exec_output;

    #[derive(Serialize)]
    struct ExecMetadata {
        exit_code: i32,
        duration_seconds: f32,
    }

    #[derive(Serialize)]
    struct ExecOutput<'a> { output: &'a str, metadata: ExecMetadata }

    // round to 1 decimal place
    let duration_seconds = ((duration.as_secs_f32()) * 10.0).round() / 10.0;

    let cwd = sess.get_cwd().to_path_buf();
    let full = format_exec_output_str(exec_output);
    let final_output = truncate_exec_output_for_storage(&cwd, sub_id, call_id, &full);

    let payload = ExecOutput {
        output: &final_output,
        metadata: ExecMetadata {
            exit_code: *exit_code,
            duration_seconds,
        },
    };

    #[expect(clippy::expect_used)]
    serde_json::to_string(&payload).expect("serialize ExecOutput")
}

fn format_bytes(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let bytes_f = bytes as f64;
    if bytes >= GIB as usize {
        format!("{:.1} GiB", bytes_f / GIB)
    } else if bytes >= MIB as usize {
        format!("{:.1} MiB", bytes_f / MIB)
    } else if bytes >= KIB as usize {
        format!("{:.1} KiB", bytes_f / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn get_last_assistant_message_from_turn(responses: &[ResponseItem]) -> Option<String> {
    responses.iter().rev().find_map(|item| {
        if let ResponseItem::Message { role, content, .. } = item {
            if role == "assistant" {
                content.iter().rev().find_map(|ci| {
                    if let ContentItem::OutputText { text } = ci {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        } else {
            None
        }
    })
}

/// Capture a screenshot from the browser and store it for the next model request
async fn capture_browser_screenshot(_sess: &Session) -> Result<(PathBuf, String), String> {
    let browser_manager = code_browser::global::get_browser_manager()
        .await
        .ok_or_else(|| "No browser manager available".to_string())?;

    if !browser_manager.is_enabled().await {
        return Err("Browser manager is not enabled".to_string());
    }

    // Get current URL first
    let url = browser_manager
        .get_current_url()
        .await
        .unwrap_or_else(|| "Browser".to_string());
    tracing::debug!("Attempting to capture screenshot at URL: {}", url);

    match browser_manager.capture_screenshot().await {
        Ok(screenshots) => {
            if let Some(first_screenshot) = screenshots.first() {
                tracing::info!(
                    "Captured browser screenshot: {} at URL: {}",
                    first_screenshot.display(),
                    url
                );
                Ok((first_screenshot.clone(), url))
            } else {
                let msg = format!("Screenshot capture returned empty results at URL: {}", url);
                tracing::warn!("{}", msg);
                Err(msg)
            }
        }
        Err(e) => {
            let msg = format!("Failed to capture screenshot at {}: {}", url, e);
            tracing::warn!("{}", msg);
            Err(msg)
        }
    }
}

/// Send agent status update event to the TUI
async fn send_agent_status_update(sess: &Session) {
    let manager = AGENT_MANAGER.read().await;

    // Collect all agents; include completed/failed so HUD can show final messages
    let now = Utc::now();
    let agents: Vec<crate::protocol::AgentInfo> = manager
        .get_all_agents()
        .map(|agent| {
            let start = agent.started_at.unwrap_or(agent.created_at);
            let end = agent.completed_at.unwrap_or(now);
            let elapsed_ms = match end.signed_duration_since(start).num_milliseconds() {
                value if value >= 0 => Some(value as u64),
                _ => None,
            };

            crate::protocol::AgentInfo {
                id: agent.id.clone(),
                name: agent.model.clone(), // Use model name as the display name
                status: match agent.status {
                    AgentStatus::Pending => "pending".to_string(),
                    AgentStatus::Running => "running".to_string(),
                    AgentStatus::Completed => "completed".to_string(),
                    AgentStatus::Failed => "failed".to_string(),
                    AgentStatus::Cancelled => "cancelled".to_string(),
                },
                batch_id: agent.batch_id.clone(),
                model: Some(agent.model.clone()),
                last_progress: agent.progress.last().cloned(),
                result: agent.result.clone(),
                error: agent.error.clone(),
                elapsed_ms,
                token_count: None,
            }
        })
        .collect();

    let event = sess.make_event(
        "agent_status",
        EventMsg::AgentStatusUpdate(AgentStatusUpdateEvent {
            agents,
            context: None,
            task: None,
        }),
    );

    // Send event asynchronously
    let tx_event = sess.tx_event.clone();
    tokio::spawn(async move {
        if let Err(e) = tx_event.send(event).await {
            tracing::error!("Failed to send agent status update event: {}", e);
        }
    });
}

/// Add a screenshot to pending screenshots for the next model request
fn add_pending_screenshot(sess: &Session, screenshot_path: PathBuf, url: String) {
    // Do not queue screenshots for next turn anymore; we inject fresh per-turn.
    tracing::info!("Captured screenshot; updating UI and using per-turn injection");

    // Also send an immediate event to update the TUI display
    let event = sess.make_event(
        "browser_screenshot",
        EventMsg::BrowserScreenshotUpdate(BrowserScreenshotUpdateEvent {
            screenshot_path,
            url,
        }),
    );

    // Send event asynchronously to avoid blocking
    let tx_event = sess.tx_event.clone();
    tokio::spawn(async move {
        if let Err(e) = tx_event.send(event).await {
            tracing::error!("Failed to send browser screenshot update event: {}", e);
        }
    });
}

/// Consume pending screenshots and return them as ResponseInputItems
#[allow(dead_code)]
fn consume_pending_screenshots(sess: &Session) -> Vec<ResponseInputItem> {
    let mut pending = sess.pending_browser_screenshots.lock().unwrap();
    let screenshots = pending.drain(..).collect::<Vec<_>>();

    screenshots
        .into_iter()
        .map(|path| {
            let metadata = format!(
                "[EPHEMERAL:browser_screenshot] Browser screenshot at {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
            );

            // Read the screenshot file and create an ephemeral image
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let mime = mime_guess::from_path(&path)
                        .first()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| "image/png".to_string());
                    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);

                    ResponseInputItem::Message {
                        role: "user".to_string(),
                        content: vec![
                            ContentItem::InputText { text: metadata },
                            ContentItem::InputImage {
                                image_url: format!("data:{mime};base64,{encoded}"),
                            },
                        ],
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to read screenshot {}: {}", path.display(), e);
                    ResponseInputItem::Message {
                        role: "user".to_string(),
                        content: vec![ContentItem::InputText {
                            text: format!("Failed to load browser screenshot: {}", e),
                        }],
                    }
                }
            }
        })
        .collect()
}

/// Helper function to wrap custom tool calls with events
async fn execute_custom_tool<F, Fut>(
    sess: &Session,
    ctx: &ToolCallCtx,
    tool_name: String,
    parameters: Option<serde_json::Value>,
    tool_fn: F,
) -> ResponseInputItem
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ResponseInputItem>,
{
    use crate::protocol::{CustomToolCallBeginEvent, CustomToolCallEndEvent};
    use std::time::Instant;

    // Send begin event with ordering
    let begin_msg = EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
        call_id: ctx.call_id.clone(),
        tool_name: tool_name.clone(),
        parameters: parameters.clone(),
    });
    let begin_order = ctx.order_meta(sess.current_request_ordinal());
    let begin_event = sess.make_event_with_order(&ctx.sub_id, begin_msg, begin_order, ctx.seq_hint);
    sess.send_event(begin_event).await;

    // Execute the tool
    let start = Instant::now();
    let result = tool_fn().await;
    let duration = start.elapsed();

    // Extract success/failure from result. Prefer explicit success flag when available.
    let (success, message) = match &result {
        ResponseInputItem::FunctionCallOutput { output, .. } => {
            let content = &output.content;
            let success_flag = output.success;
            (success_flag.unwrap_or(true), content.clone())
        }
        _ => (true, String::from("Tool completed")),
    };

    // Send end event with ordering
    let end_msg = EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
        call_id: ctx.call_id.clone(),
        tool_name,
        parameters,
        duration,
        result: if success { Ok(message) } else { Err(message) },
    });
    let end_order = ctx.order_meta(sess.current_request_ordinal());
    let end_event = sess.make_event_with_order(&ctx.sub_id, end_msg, end_order, ctx.seq_hint);
    sess.send_event(end_event).await;

    result
}

async fn handle_browser_tool(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    use serde_json::Value;

    let parsed_value = match serde_json::from_str::<Value>(&arguments) {
        Ok(value) => value,
        Err(e) => {
            return ResponseInputItem::FunctionCallOutput {
                call_id: ctx.call_id.clone(),
                output: FunctionCallOutputPayload {
                    content: format!("Invalid browser arguments: {e}"),
                    success: Some(false),
                },
            };
        }
    };

    let mut object = match parsed_value {
        Value::Object(map) => map,
        _ => {
            return ResponseInputItem::FunctionCallOutput {
                call_id: ctx.call_id.clone(),
                output: FunctionCallOutputPayload {
                    content: "Invalid browser arguments: expected an object".to_string(),
                    success: Some(false),
                },
            };
        }
    };

    let action_value = object.remove("action");
    let action = match action_value.and_then(|v| v.as_str().map(|s| s.to_string())) {
        Some(value) => value,
        None => {
            return ResponseInputItem::FunctionCallOutput {
                call_id: ctx.call_id.clone(),
                output: FunctionCallOutputPayload {
                    content: "Invalid browser arguments: missing 'action'".to_string(),
                    success: Some(false),
                },
            };
        }
    };

    let payload_value = Value::Object(object.clone());
    let payload_string = if object.is_empty() {
        "{}".to_string()
    } else {
        serde_json::to_string(&payload_value).unwrap_or_else(|_| "{}".to_string())
    };

    let action_lower = action.to_lowercase();

    match action_lower.as_str() {
        "open" => handle_browser_open(sess, ctx, payload_string.clone()).await,
        "close" => handle_browser_close(sess, ctx).await,
        "status" => handle_browser_status(sess, ctx).await,
        "click" => handle_browser_click(sess, ctx, payload_string.clone()).await,
        "move" => handle_browser_move(sess, ctx, payload_string.clone()).await,
        "type" => handle_browser_type(sess, ctx, payload_string.clone()).await,
        "key" => handle_browser_key(sess, ctx, payload_string.clone()).await,
        "javascript" => handle_browser_javascript(sess, ctx, payload_string.clone()).await,
        "scroll" => handle_browser_scroll(sess, ctx, payload_string.clone()).await,
        "history" => handle_browser_history(sess, ctx, payload_string.clone()).await,
        "inspect" => handle_browser_inspect(sess, ctx, payload_string.clone()).await,
        "console" => handle_browser_console(sess, ctx, payload_string.clone()).await,
        "cdp" => handle_browser_cdp(sess, ctx, payload_string.clone()).await,
        "cleanup" => handle_browser_cleanup(sess, ctx).await,
        "fetch" => handle_web_fetch(sess, ctx, payload_string.clone()).await,
        _ => ResponseInputItem::FunctionCallOutput {
            call_id: ctx.call_id.clone(),
            output: FunctionCallOutputPayload {
                content: format!("Unknown browser action: {}", action),
                success: Some(false),
            },
        },
    }
}

async fn handle_browser_open(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    // Parse arguments as JSON for the event
    let params = serde_json::from_str(&arguments).ok();

    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_open".to_string(),
        params,
        || async move {
            // Parse the URL from arguments
            let args: Result<Value, _> = serde_json::from_str(&arguments_clone);

            match args {
                Ok(json) => {
                    let url = json
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("about:blank");

                    // Use the global browser manager (create if needed)
                    let browser_manager = {
                        let existing_global = code_browser::global::get_browser_manager().await;
                        if let Some(existing) = existing_global {
                            tracing::info!("Using existing global browser manager");
                            Some(existing)
                        } else {
                            tracing::info!("Creating new browser manager");
                            let new_manager =
                                code_browser::global::get_or_create_browser_manager().await;
                            Some(new_manager)
                        }
                    };

                    if let Some(browser_manager) = browser_manager {
                        // Ensure the browser manager is marked enabled so status reflects reality
                        browser_manager.set_enabled_sync(true);
                        // Clear any lingering node highlight from previous commands
                        let _ = browser_manager
                            .execute_cdp("Overlay.hideHighlight", serde_json::json!({}))
                            .await;
                        // Navigate to the URL with detailed timing logs
                        let step_start = std::time::Instant::now();
                        tracing::info!("[browser_open] begin goto: {}", url);
                        match browser_manager.goto(url).await {
                            Ok(_) => {
                                tracing::info!(
                                    "[browser_open] goto success: {} in {:?}",
                                    url,
                                    step_start.elapsed()
                                );
                                ResponseInputItem::FunctionCallOutput {
                                    call_id: call_id_clone.clone(),
                                    output: FunctionCallOutputPayload {
                                        content: format!("Browser opened to: {}", url),
                                        success: Some(true),
                                    },
                                }
                            }
                            Err(e) => ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone.clone(),
                                output: FunctionCallOutputPayload {
                                    content: format!(
                                        "Failed to navigate browser to {}: {}",
                                        url, e
                                    ),
                                    success: Some(false),
                                },
                            },
                        }
                    } else {
                        ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone.clone(),
                            output: FunctionCallOutputPayload {
                                content: "Failed to initialize browser manager.".to_string(),
                                success: Some(false),
                            },
                        }
                    }
                }
                Err(e) => ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: format!("Failed to parse browser_open arguments: {}", e),
                        success: Some(false),
                    },
                },
            }
        },
    )
    .await
}

/// Get the browser manager for the session (always uses global)
async fn get_browser_manager_for_session(
    _sess: &Session,
) -> Option<Arc<code_browser::BrowserManager>> {
    // Always use the global browser manager
    code_browser::global::get_browser_manager().await
}

async fn handle_browser_close(sess: &Session, ctx: &ToolCallCtx) -> ResponseInputItem {
    let sess_clone = sess;
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_close".to_string(),
        None,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;
            if let Some(browser_manager) = browser_manager {
                // Clear any lingering highlight before closing
                let _ = browser_manager
                    .execute_cdp("Overlay.hideHighlight", serde_json::json!({}))
                    .await;
                match browser_manager.stop().await {
                    Ok(_) => {
                        // Clear the browser manager from global
                        code_browser::global::clear_browser_manager().await;
                        ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone.clone(),
                            output: FunctionCallOutputPayload {
                                content: "Browser closed. Screenshot capture disabled.".to_string(),
                                success: Some(true),
                            },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone.clone(),
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to close browser: {}", e),
                            success: Some(false),
                        },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: "Browser is not currently open.".to_string(),
                        success: Some(false),
                    },
                }
            }
        },
    )
    .await
}

async fn handle_browser_status(sess: &Session, ctx: &ToolCallCtx) -> ResponseInputItem {
    let sess_clone = sess;
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_status".to_string(),
        None,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;
            if let Some(browser_manager) = browser_manager {
                let _ = browser_manager
                    .execute_cdp("Overlay.hideHighlight", serde_json::json!({}))
                    .await;
                let status = browser_manager.get_status().await;
                let status_msg = if status.enabled {
                    if let Some(url) = status.current_url {
                        format!("Browser status: Enabled, currently at {}", url)
                    } else {
                        "Browser status: Enabled, no page loaded".to_string()
                    }
                } else {
                    "Browser status: Disabled".to_string()
                };

                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone.clone(),
                    output: FunctionCallOutputPayload {
                        content: status_msg,
                        success: Some(true),
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content:
                            "Browser is not initialized. Use browser_open to start the browser."
                                .to_string(),
                        success: Some(false),
                    },
                }
            }
        },
    )
    .await
}

async fn handle_browser_click(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    let params = serde_json::from_str::<serde_json::Value>(&arguments).ok();
    let sess_clone = sess;
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_click".to_string(),
        params.clone(),
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;

            if let Some(browser_manager) = browser_manager {
                let _ = browser_manager
                    .execute_cdp("Overlay.hideHighlight", serde_json::json!({}))
                    .await;
                // Determine click type: default 'click', or 'mousedown'/'mouseup'
                let click_type = params
                    .as_ref()
                    .and_then(|v| v.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("click")
                    .to_lowercase();

                // Optional absolute coordinates
                let (mut target_x, mut target_y) = (None, None);
                if let Some(p) = params.as_ref() {
                    if let Some(vx) = p.get("x").and_then(|v| v.as_f64()) {
                        target_x = Some(vx);
                    }
                    if let Some(vy) = p.get("y").and_then(|v| v.as_f64()) {
                        target_y = Some(vy);
                    }
                }

                // If x or y provided, resolve missing coord from current position, then move
                if target_x.is_some() || target_y.is_some() {
                    // get current cursor for missing values
                    match browser_manager.get_cursor_position().await {
                        Ok((cx, cy)) => {
                            let x = target_x.unwrap_or(cx);
                            let y = target_y.unwrap_or(cy);
                            if let Err(e) = browser_manager.move_mouse(x, y).await {
                                return ResponseInputItem::FunctionCallOutput {
                                    call_id: call_id_clone.clone(),
                                    output: FunctionCallOutputPayload {
                                        content: format!("Failed to move before click: {}", e),
                                        success: Some(false),
                                    },
                                };
                            }
                        }
                        Err(e) => {
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone.clone(),
                                output: FunctionCallOutputPayload {
                                    content: format!("Failed to get current cursor position: {}", e),
                                    success: Some(false),
                                },
                            };
                        }
                    }
                }

                // Perform the action at current (possibly moved) position
                let action_result = match click_type.as_str() {
                    "mousedown" => match browser_manager.mouse_down_at_current().await {
                        Ok((x, y)) => Ok((x, y, "Mouse down".to_string())),
                        Err(e) => Err(e),
                    },
                    "mouseup" => match browser_manager.mouse_up_at_current().await {
                        Ok((x, y)) => Ok((x, y, "Mouse up".to_string())),
                        Err(e) => Err(e),
                    },
                    "click" | _ => match browser_manager.click_at_current().await {
                        Ok((x, y)) => Ok((x, y, "Clicked".to_string())),
                        Err(e) => Err(e),
                    },
                };

                match action_result {
                    Ok((x, y, label)) => {
                        ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone.clone(),
                            output: FunctionCallOutputPayload {
                                content: format!("{} at ({}, {})", label, x, y),
                                success: Some(true),
                            },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone.clone(),
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to perform mouse action: {}", e),
                            success: Some(false),
                        },
                    },
                }
    } else {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id_clone,
            output: FunctionCallOutputPayload {
                content: "Browser is not initialized. Use browser_open to start the browser."
                    .to_string(),
                success: Some(false),
            },
        }
    }
        },
    )
    .await
}

async fn handle_browser_move(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    let params = serde_json::from_str(&arguments).ok();
    let sess_clone = sess;
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_move".to_string(),
        params,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;

            if let Some(browser_manager) = browser_manager {
                let _ = browser_manager
                    .execute_cdp("Overlay.hideHighlight", serde_json::json!({}))
                    .await;
                let args: Result<Value, _> = serde_json::from_str(&arguments_clone);
                match args {
                    Ok(json) => {
                        // Check if we have relative movement (dx, dy) or absolute (x, y)
                        let has_dx = json.get("dx").is_some();
                        let has_dy = json.get("dy").is_some();
                        let has_x = json.get("x").is_some();
                        let has_y = json.get("y").is_some();

                        let result = if has_dx || has_dy {
                            // Relative movement
                            let dx = json.get("dx").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let dy = json.get("dy").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            browser_manager.move_mouse_relative(dx, dy).await
                        } else if has_x || has_y {
                            // Absolute movement
                            let x = json.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let y = json.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            browser_manager.move_mouse(x, y).await.map(|_| (x, y))
                        } else {
                            // No parameters provided, just return current position
                            browser_manager.get_cursor_position().await
                        };

                        match result {
                            Ok((x, y)) => {
                                ResponseInputItem::FunctionCallOutput {
                                    call_id: call_id_clone.clone(),
                                    output: FunctionCallOutputPayload {
                                        content: format!("Moved mouse position to ({}, {})", x, y),
                                        success: Some(true),
                                    },
                                }
                            }
                            Err(e) => ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone.clone(),
                                output: FunctionCallOutputPayload {
                                    content: format!("Failed to move mouse: {}", e),
                                    success: Some(false),
                                },
                            },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone.clone(),
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to parse browser_move arguments: {}", e),
                            success: Some(false),
                        },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: "Browser is not initialized. Use browser_open to start the browser."
                            .to_string(),
                        success: Some(false),
                    },
                }
            }
        },
    )
    .await
}

async fn handle_browser_type(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    let params = serde_json::from_str(&arguments).ok();
    let sess_clone = sess;
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_type".to_string(),
        params,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;
            if let Some(browser_manager) = browser_manager {
                let _ = browser_manager
                    .execute_cdp("Overlay.hideHighlight", serde_json::json!({}))
                    .await;
                let args: Result<Value, _> = serde_json::from_str(&arguments_clone);
                match args {
                    Ok(json) => {
                        let text = json.get("text").and_then(|v| v.as_str()).unwrap_or("");

                        match browser_manager.type_text(text).await {
                            Ok(_) => {
                                ResponseInputItem::FunctionCallOutput {
                                    call_id: call_id_clone.clone(),
                                    output: FunctionCallOutputPayload {
                                        content: format!("Typed: {}", text),
                                        success: Some(true),
                                    },
                                }
                            }
                            Err(e) => ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone.clone(),
                                output: FunctionCallOutputPayload {
                                    content: format!("Failed to type text: {}", e),
                                    success: Some(false),
                                },
                            },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone.clone(),
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to parse browser_type arguments: {}", e),
                            success: Some(false),
                        },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content:
                            "Browser is not initialized. Use browser_open to start the browser."
                                .to_string(),
                        success: Some(false),
                    },
                }
            }
        },
    )
    .await
}

async fn handle_browser_key(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    let params = serde_json::from_str(&arguments).ok();
    let sess_clone = sess;
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_key".to_string(),
        params,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;
            if let Some(browser_manager) = browser_manager {
                let _ = browser_manager
                    .execute_cdp("Overlay.hideHighlight", serde_json::json!({}))
                    .await;
                let args: Result<Value, _> = serde_json::from_str(&arguments_clone);
                match args {
                    Ok(json) => {
                        let key = json.get("key").and_then(|v| v.as_str()).unwrap_or("");

                        match browser_manager.press_key(key).await {
                            Ok(_) => {
                                ResponseInputItem::FunctionCallOutput {
                                    call_id: call_id_clone.clone(),
                                    output: FunctionCallOutputPayload {
                                        content: format!("Pressed key: {}", key),
                                        success: Some(true),
                                    },
                                }
                            }
                            Err(e) => ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone.clone(),
                                output: FunctionCallOutputPayload {
                                    content: format!("Failed to press key: {}", e),
                                    success: Some(false),
                                },
                            },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to parse browser_key arguments: {}", e),
                            success: Some(false),
                        },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content:
                            "Browser is not initialized. Use browser_open to start the browser."
                                .to_string(),
                        success: Some(false),
                    },
                }
            }
        },
    )
    .await
}

async fn handle_browser_javascript(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    let params = serde_json::from_str(&arguments).ok();
    let sess_clone = sess;
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_javascript".to_string(),
        params,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;
            if let Some(browser_manager) = browser_manager {
                let _ = browser_manager
                    .execute_cdp("Overlay.hideHighlight", serde_json::json!({}))
                    .await;
                let args: Result<Value, _> = serde_json::from_str(&arguments_clone);
                match args {
                    Ok(json) => {
                        let code = json.get("code").and_then(|v| v.as_str()).unwrap_or("");

                        match browser_manager.execute_javascript(code).await {
                            Ok(result) => {
                                // Log the JavaScript execution result
                                tracing::info!("JavaScript execution returned: {:?}", result);

                                // Format the result for the LLM
                                let formatted_result = if let Some(obj) = result.as_object() {
                                    // Check if it's our wrapped result format
                                    if let (Some(success), Some(value)) =
                                        (obj.get("success"), obj.get("value"))
                                    {
                                        let logs = obj.get("logs").and_then(|v| v.as_array());
                                        let mut output = String::new();

                                        if let Some(logs) = logs {
                                            if !logs.is_empty() {
                                                output.push_str("Console logs:\n");
                                                for log in logs {
                                                    if let Some(log_str) = log.as_str() {
                                                        output
                                                            .push_str(&format!("  {}\n", log_str));
                                                    }
                                                }
                                                output.push_str("\n");
                                            }
                                        }

                                        if success.as_bool().unwrap_or(false) {
                                            output.push_str("Result: ");
                                            output.push_str(
                                                &serde_json::to_string_pretty(value)
                                                    .unwrap_or_else(|_| "null".to_string()),
                                            );
                                        } else if let Some(error) = obj.get("error") {
                                            output.push_str("Error: ");
                                            output.push_str(&error.to_string());
                                        }

                                        output
                                    } else {
                                        // Fallback to raw JSON if not in expected format
                                        serde_json::to_string_pretty(&result)
                                            .unwrap_or_else(|_| "null".to_string())
                                    }
                                } else {
                                    // Not an object, return as-is
                                    serde_json::to_string_pretty(&result)
                                        .unwrap_or_else(|_| "null".to_string())
                                };

                                tracing::info!("Returning to LLM: {}", formatted_result);

                                ResponseInputItem::FunctionCallOutput {
                                    call_id: call_id_clone.clone(),
                                    output: FunctionCallOutputPayload {
                                        content: formatted_result,
                                        success: Some(true),
                                    },
                                }
                            }
                            Err(e) => ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone.clone(),
                                output: FunctionCallOutputPayload {
                                    content: format!("Failed to execute JavaScript: {}", e),
                                    success: Some(false),
                                },
                            },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to parse browser_javascript arguments: {}", e),
                            success: Some(false),
                        },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content:
                            "Browser is not initialized. Use browser_open to start the browser."
                                .to_string(),
                        success: Some(false),
                    },
                }
            }
        },
    )
    .await
}

async fn handle_browser_scroll(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    let params = serde_json::from_str(&arguments).ok();
    let sess_clone = sess;
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_scroll".to_string(),
        params,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;
            if let Some(browser_manager) = browser_manager {
                let _ = browser_manager
                    .execute_cdp("Overlay.hideHighlight", serde_json::json!({}))
                    .await;
                let args: Result<Value, _> = serde_json::from_str(&arguments_clone);
                match args {
                    Ok(json) => {
                        let dx = json.get("dx").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let dy = json.get("dy").and_then(|v| v.as_f64()).unwrap_or(0.0);

                        match browser_manager.scroll_by(dx, dy).await {
                    Ok(_) => {
                        ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone.clone(),
                            output: FunctionCallOutputPayload {
                                content: format!("Scrolled by ({}, {})", dx, dy),
                                success: Some(true),
                            },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone.clone(),
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to scroll: {}", e),
                            success: Some(false),
                        },
                    },
                }
            }
            Err(e) => ResponseInputItem::FunctionCallOutput {
                call_id: call_id_clone,
                output: FunctionCallOutputPayload {
                    content: format!("Failed to parse browser_scroll arguments: {}", e),
                    success: Some(false),
                },
            },
        }
    } else {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id_clone,
            output: FunctionCallOutputPayload {
                content: "Browser is not initialized. Use browser_open to start the browser.".to_string(),
                success: Some(false),
            },
        }
    }
        },
    )
    .await
}

async fn handle_browser_console(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    let params = serde_json::from_str(&arguments).ok();
    let sess_clone = sess;
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_console".to_string(),
        params,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;
            if let Some(browser_manager) = browser_manager {
                let args: Result<Value, _> = serde_json::from_str(&arguments_clone);
                let lines = match args {
                    Ok(json) => json.get("lines").and_then(|v| v.as_u64()).map(|n| n as usize),
                    Err(_) => None,
                };

                match browser_manager.get_console_logs(lines).await {
                    Ok(logs) => {
                        // Format the logs for display
                        let formatted = if let Some(logs_array) = logs.as_array() {
                            if logs_array.is_empty() {
                                "No console logs captured.".to_string()
                            } else {
                                let mut output = String::new();
                                output.push_str("Console logs:\n");
                                for log in logs_array {
                                    if let Some(log_obj) = log.as_object() {
                                        let timestamp = log_obj.get("timestamp")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        let level = log_obj.get("level")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("log");
                                        let message = log_obj.get("message")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");

                                        output.push_str(&format!("[{}] [{}] {}\n", timestamp, level.to_uppercase(), message));
                                    }
                                }
                                output
                            }
                        } else {
                            "No console logs captured.".to_string()
                        };

                        ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone,
                            output: FunctionCallOutputPayload {
                                content: formatted,
                                success: Some(true),
                            },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to get console logs: {}", e),
                            success: Some(false),
                        },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: "Browser is not enabled. Use browser_open to enable it first.".to_string(),
                        success: Some(false),
                    },
                }
            }
        },
    )
    .await
}

async fn handle_browser_cdp(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    let params = serde_json::from_str(&arguments).ok();
    let sess_clone = sess;
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_cdp".to_string(),
        params,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;
            if let Some(browser_manager) = browser_manager {
                let _ = browser_manager
                    .execute_cdp("Overlay.hideHighlight", serde_json::json!({}))
                    .await;
                let args: Result<Value, _> = serde_json::from_str(&arguments_clone);
                match args {
                    Ok(json) => {
                        let method = json
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let params = json.get("params").cloned().unwrap_or_else(|| Value::Object(serde_json::Map::new()));
                        let target = json
                            .get("target")
                            .and_then(|v| v.as_str())
                            .unwrap_or("page");

                        if method.is_empty() {
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload {
                                    content: "Missing required field: method".to_string(),
                                    success: Some(false),
                                },
                            };
                        }

                        let exec_res = if target == "browser" {
                            browser_manager.execute_cdp_browser(&method, params).await
                        } else {
                            browser_manager.execute_cdp(&method, params).await
                        };

                        match exec_res {
                            Ok(result) => {
                                let pretty = serde_json::to_string_pretty(&result)
                                    .unwrap_or_else(|_| "<non-serializable result>".to_string());
                                ResponseInputItem::FunctionCallOutput {
                                    call_id: call_id_clone,
                                    output: FunctionCallOutputPayload {
                                        content: pretty,
                                        success: Some(true),
                                    },
                                }
                            }
                            Err(e) => ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload {
                                    content: format!("Failed to execute CDP command: {}", e),
                                    success: Some(false),
                                },
                            },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to parse browser_cdp arguments: {}", e),
                            success: Some(false),
                        },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: "Browser is not initialized. Use browser_open to start the browser.".to_string(),
                        success: Some(false),
                    },
                }
            }
        },
    )
    .await
}

async fn handle_browser_inspect(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    use serde_json::json;
    let params = serde_json::from_str(&arguments).ok();
    let sess_clone = sess;
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_inspect".to_string(),
        params,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;
            if let Some(browser_manager) = browser_manager {
                let args: Result<Value, _> = serde_json::from_str(&arguments_clone);
                match args {
                    Ok(json) => {
                        // Determine target element: by id, by coords, or by cursor
                        let id_attr = json.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let mut x = json.get("x").and_then(|v| v.as_f64());
                        let mut y = json.get("y").and_then(|v| v.as_f64());

                        if (x.is_none() || y.is_none()) && id_attr.is_none() {
                            // No coords provided; use current cursor
                            if let Ok((cx, cy)) = browser_manager.get_cursor_position().await {
                                x = Some(cx);
                                y = Some(cy);
                            }
                        }

                        // Resolve nodeId
                        let node_id_value = if let Some(id_attr) = id_attr.clone() {
                            // Use DOM.getDocument -> DOM.querySelector with selector `#id`
                            let doc = browser_manager
                                .execute_cdp("DOM.getDocument", json!({}))
                                .await
                                .map_err(|e| e);
                            let root_id = match doc {
                                Ok(v) => v.get("root").and_then(|r| r.get("nodeId")).and_then(|n| n.as_u64()),
                                Err(_) => None,
                            };
                            if let Some(root_node_id) = root_id {
                                let sel = format!("#{}", id_attr);
                                let q = browser_manager
                                    .execute_cdp(
                                        "DOM.querySelector",
                                        json!({"nodeId": root_node_id, "selector": sel}),
                                    )
                                    .await;
                                match q {
                                    Ok(v) => v.get("nodeId").cloned(),
                                    Err(_) => None,
                                }
                            } else {
                                None
                            }
                        } else if let (Some(x), Some(y)) = (x, y) {
                            // Use DOM.getNodeForLocation
                            let res = browser_manager
                                .execute_cdp(
                                    "DOM.getNodeForLocation",
                                    json!({
                                        "x": x,
                                        "y": y,
                                        "includeUserAgentShadowDOM": true
                                    }),
                                )
                                .await;
                            match res {
                                Ok(v) => {
                                    // Prefer nodeId; if absent, push backendNodeId
                                    if let Some(n) = v.get("nodeId").cloned() {
                                        Some(n)
                                    } else if let Some(backend) = v.get("backendNodeId").and_then(|b| b.as_u64()) {
                                        let pushed = browser_manager
                                            .execute_cdp(
                                                "DOM.pushNodesByBackendIdsToFrontend",
                                                json!({ "backendNodeIds": [backend] }),
                                            )
                                            .await
                                            .ok();
                                        pushed
                                            .and_then(|pv| pv.get("nodeIds").and_then(|arr| arr.as_array().cloned()))
                                            .and_then(|arr| arr.first().cloned())
                                    } else {
                                        None
                                    }
                                }
                                Err(_) => None,
                            }
                        } else {
                            None
                        };

                        let node_id = match node_id_value.and_then(|v| v.as_u64()) {
                            Some(id) => id,
                            None => {
                                return ResponseInputItem::FunctionCallOutput {
                                    call_id: call_id_clone,
                                    output: FunctionCallOutputPayload {
                                        content: "Failed to resolve target node for inspection".to_string(),
                                        success: Some(false),
                                    },
                                };
                            }
                        };

                        // Enable CSS domain to get matched rules
                        let _ = browser_manager.execute_cdp("CSS.enable", json!({})).await;

                        // Gather details
                        let attrs = browser_manager
                            .execute_cdp("DOM.getAttributes", json!({"nodeId": node_id}))
                            .await
                            .unwrap_or_else(|_| json!({}));
                        let outer = browser_manager
                            .execute_cdp("DOM.getOuterHTML", json!({"nodeId": node_id}))
                            .await
                            .unwrap_or_else(|_| json!({}));
                        let box_model = browser_manager
                            .execute_cdp("DOM.getBoxModel", json!({"nodeId": node_id}))
                            .await
                            .unwrap_or_else(|_| json!({}));
                        let styles = browser_manager
                            .execute_cdp("CSS.getMatchedStylesForNode", json!({"nodeId": node_id}))
                            .await
                            .unwrap_or_else(|_| json!({}));

                        // Highlight the inspected node using Overlay domain (no screenshot capture here)
                        let _ = browser_manager.execute_cdp("Overlay.enable", json!({})).await;
                        let highlight_config = json!({
                            "showInfo": true,
                            "showStyles": false,
                            "showRulers": false,
                            "contentColor": {"r": 111, "g": 168, "b": 220, "a": 0.20},
                            "paddingColor": {"r": 147, "g": 196, "b": 125, "a": 0.55},
                            "borderColor": {"r": 255, "g": 229, "b": 153, "a": 0.60},
                            "marginColor": {"r": 246, "g": 178, "b": 107, "a": 0.60}
                        });
                        let _ = browser_manager.execute_cdp(
                            "Overlay.highlightNode",
                            json!({ "nodeId": node_id, "highlightConfig": highlight_config })
                        ).await;
                        // Do not hide here; keep highlight until the next browser command.

                        // Format output
                        let mut out = String::new();
                        if let (Some(ix), Some(iy)) = (x, y) {
                            out.push_str(&format!("Target: coordinates ({}, {})\n", ix, iy));
                        }
                        if let Some(id_attr) = id_attr {
                            out.push_str(&format!("Target: id '#{}'\n", id_attr));
                        }
                        out.push_str(&format!("NodeId: {}\n", node_id));

                        // Attributes
                        if let Some(arr) = attrs.get("attributes").and_then(|v| v.as_array()) {
                            out.push_str("Attributes:\n");
                            let mut it = arr.iter();
                            while let (Some(k), Some(v)) = (it.next(), it.next()) {
                                out.push_str(&format!("  {}=\"{}\"\n", k.as_str().unwrap_or(""), v.as_str().unwrap_or("")));
                            }
                        }

                        // Outer HTML
                        if let Some(html) = outer.get("outerHTML").and_then(|v| v.as_str()) {
                            let one = html.replace('\n', " ");
                            let snippet: String = one.chars().take(800).collect();
                            out.push_str("\nOuterHTML (truncated):\n");
                            out.push_str(&snippet);
                            if one.len() > snippet.len() { out.push_str("…"); }
                            out.push('\n');
                        }

                        // Box Model summary
                        if box_model.get("model").is_some() {
                            out.push_str("\nBoxModel: available (content/padding/border/margin)\n");
                        }

                        // Matched styles summary
                        if let Some(rules) = styles.get("matchedCSSRules").and_then(|v| v.as_array()) {
                            out.push_str(&format!("Matched CSS rules: {}\n", rules.len()));
                        }

                        // No inline screenshot capture; result reflects DOM details only.

                        ResponseInputItem::FunctionCallOutput {
                            call_id: call_id_clone,
                            output: FunctionCallOutputPayload { content: out, success: Some(true) },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to parse browser_inspect arguments: {}", e),
                            success: Some(false),
                        },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content: "Browser is not initialized. Use browser_open to start the browser.".to_string(),
                        success: Some(false),
                    },
                }
            }
        },
    )
    .await
}

async fn handle_browser_history(sess: &Session, ctx: &ToolCallCtx, arguments: String) -> ResponseInputItem {
    let params = serde_json::from_str(&arguments).ok();
    let sess_clone = sess;
    let arguments_clone = arguments.clone();
    let call_id_clone = ctx.call_id.clone();

    execute_custom_tool(
        sess,
        ctx,
        "browser_history".to_string(),
        params,
        || async move {
            let browser_manager = get_browser_manager_for_session(sess_clone).await;
            if let Some(browser_manager) = browser_manager {
                let args: Result<Value, _> = serde_json::from_str(&arguments_clone);
                match args {
                    Ok(json) => {
                        let direction =
                            json.get("direction").and_then(|v| v.as_str()).unwrap_or("");

                        if direction != "back" && direction != "forward" {
                            return ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone,
                                output: FunctionCallOutputPayload {
                                    content: format!(
                                        "Unsupported direction: {} (expected 'back' or 'forward')",
                                        direction
                                    ),
                                    success: Some(false),
                                },
                            };
                        }

                        let action_res = if direction == "back" {
                            browser_manager.history_back().await
                        } else {
                            browser_manager.history_forward().await
                        };

                        match action_res {
                            Ok(_) => {
                                ResponseInputItem::FunctionCallOutput {
                                    call_id: call_id_clone.clone(),
                                    output: FunctionCallOutputPayload {
                                        content: format!("History {} triggered", direction),
                                        success: Some(true),
                                    },
                                }
                            }
                            Err(e) => ResponseInputItem::FunctionCallOutput {
                                call_id: call_id_clone.clone(),
                                output: FunctionCallOutputPayload {
                                    content: format!("Failed to navigate history: {}", e),
                                    success: Some(false),
                                },
                            },
                        }
                    }
                    Err(e) => ResponseInputItem::FunctionCallOutput {
                        call_id: call_id_clone,
                        output: FunctionCallOutputPayload {
                            content: format!("Failed to parse browser_history arguments: {}", e),
                            success: Some(false),
                        },
                    },
                }
            } else {
                ResponseInputItem::FunctionCallOutput {
                    call_id: call_id_clone,
                    output: FunctionCallOutputPayload {
                        content:
                            "Browser is not initialized. Use browser_open to start the browser."
                                .to_string(),
                        success: Some(false),
                    },
                }
            }
        },
    )
    .await
}

fn extract_shell_script_from_wrapper(argv: &[String]) -> Option<(usize, String)> {
    // Return (index_of_script, script) if argv matches: <shell> (-lc|-c) <script>
    if argv.len() == 3 {
        let shell = std::path::Path::new(&argv[0])
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let is_shell = matches!(shell, "bash" | "sh" | "zsh");
        let is_flag = matches!(argv[1].as_str(), "-lc" | "-c");
        if is_shell && is_flag {
            return Some((2, argv[2].clone()));
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CatWriteSuggestion {
    label: &'static str,
    original_value: String,
}

fn detect_cat_write(argv: &[String]) -> Option<CatWriteSuggestion> {
    if let Some((_, script)) = extract_shell_script_from_wrapper(argv) {
        if script_contains_cat_write(&script) {
            return Some(CatWriteSuggestion {
                label: "original_script",
                original_value: script,
            });
        }
    }

    None
}

fn script_contains_cat_write(script: &str) -> bool {
    script
        .lines()
        .any(|line| line_contains_cat_heredoc_write(line))
}

fn line_contains_cat_heredoc_write(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return false;
    }

    let lower = line.to_ascii_lowercase();
    if !lower.contains("<<") || !lower.contains('>') {
        return false;
    }

    let bytes = lower.as_bytes();
    let mut idx = 0;
    while idx + 3 <= bytes.len() {
        if bytes[idx..].starts_with(b"cat") {
            if idx > 0 {
                let prev = bytes[idx - 1];
                if prev.is_ascii_alphanumeric() || prev == b'_' {
                    idx += 1;
                    continue;
                }
            }

            let after = &lower[idx + 3..];
            let after_trimmed = after.trim_start();
            if after_trimmed.starts_with("<<") {
                let heredoc_offset_in_after = after.find("<<").unwrap_or(0);
                let heredoc_offset = idx + 3 + heredoc_offset_in_after;
                let redirect_section = &lower[heredoc_offset..];
                if let Some(rel_redirect_idx) = redirect_section.find('>') {
                    let redirect_idx = heredoc_offset + rel_redirect_idx;
                    if redirect_idx > heredoc_offset {
                        let redirect_slice = &lower[redirect_idx..];
                        if redirect_slice.starts_with(">&") {
                            idx += 1;
                            continue;
                        }
                        let after_gt = redirect_slice[1..].trim_start();
                        if after_gt.starts_with('&') {
                            idx += 1;
                            continue;
                        }
                        if after_gt.starts_with('(') {
                            idx += 1;
                            continue;
                        }
                        return true;
                    }
                }
            }
        }
        idx += 1;
    }

    false
}

fn guard_apply_patch_outside_branch(branch_root: &Path, action: &ApplyPatchAction) -> Option<String> {
    let branch_norm = match normalize_absolute(branch_root) {
        Some(path) => path,
        None => {
            return Some(format!(
                "apply_patch blocked: failed to resolve /branch worktree root {}. Stay inside the worktree until you finish with `/merge`.",
                branch_root.display()
            ));
        }
    };
    let action_cwd_norm = match normalize_absolute(&action.cwd) {
        Some(path) => path,
        None => {
            return Some(format!(
                "apply_patch blocked: the command resolved outside the /branch worktree (cwd {}). Stay inside {} until you finish with `/merge`.",
                action.cwd.display(),
                branch_root.display()
            ));
        }
    };
    if !path_within(&action_cwd_norm, &branch_norm) {
        return Some(format!(
            "apply_patch blocked: the active /branch worktree is {} but the command tried to run from {}. Stay inside the worktree until you finish with `/merge`.",
            branch_root.display(),
            action.cwd.display()
        ));
    }

    for path in action.changes().keys() {
        let normalized = match normalize_absolute(path) {
            Some(value) => value,
            None => {
                return Some(format!(
                    "apply_patch blocked: could not resolve patch target {} inside worktree {}. Keep edits within the /branch directory.",
                    path.display(),
                    branch_root.display()
                ));
            }
        };
        if !path_within(&normalized, &branch_norm) {
            return Some(format!(
                "apply_patch blocked: patch would modify {} outside the active /branch worktree {}. Apply changes from within the worktree before `/merge`.",
                path.display(),
                branch_root.display()
            ));
        }
    }

    None
}

fn normalize_absolute(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => result.push(prefix.as_os_str()),
            Component::RootDir => result.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    return None;
                }
            }
            Component::Normal(part) => result.push(part),
        }
    }
    if result.as_os_str().is_empty() {
        None
    } else {
        Some(result)
    }
}

fn path_within(path: &Path, base: &Path) -> bool {
    path.starts_with(base)
}

fn guidance_for_cat_write(suggestion: &CatWriteSuggestion) -> String {
    format!(
        "Blocked cat heredoc that writes files directly. Use apply_patch to edit files so changes stay reviewable.\n\n{}: {}",
        suggestion.label,
        suggestion.original_value
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PythonWriteSuggestion {
    label: &'static str,
    original_value: String,
}

fn detect_python_write(argv: &[String]) -> Option<PythonWriteSuggestion> {
    if let Some((_, script)) = extract_shell_script_from_wrapper(argv) {
        if script_contains_python_write(&script) {
            return Some(PythonWriteSuggestion {
                label: "original_script",
                original_value: script,
            });
        }
    }

    detect_python_write_in_argv(argv)
}

fn detect_python_write_in_argv(argv: &[String]) -> Option<PythonWriteSuggestion> {
    if argv.is_empty() {
        return None;
    }

    if !is_python_command(&argv[0]) {
        return None;
    }

    if argv.len() >= 3 && argv[1] == "-c" {
        let code = &argv[2];
        if python_code_writes_files(code) {
            return Some(PythonWriteSuggestion {
                label: "python_inline_script",
                original_value: code.clone(),
            });
        }
    }

    None
}

fn script_contains_python_write(script: &str) -> bool {
    let lower = script.to_ascii_lowercase();
    if !(lower.contains("python ")
        || lower.contains("python3")
        || lower.contains("python\n"))
    {
        return false;
    }
    contains_python_write_keywords(&lower)
}

fn python_code_writes_files(code: &str) -> bool {
    contains_python_write_keywords(&code.to_ascii_lowercase())
}

fn contains_python_write_keywords(lower: &str) -> bool {
    const KEYWORDS: &[&str] = &["write_text(", "write_bytes(", ".write_text(", ".write_bytes("];
    KEYWORDS.iter().any(|needle| lower.contains(needle))
}

fn is_python_command(cmd: &str) -> bool {
    std::path::Path::new(cmd)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|name| {
            let lower = name.to_ascii_lowercase();
            matches!(lower.as_str(), "python" | "python3" | "python2")
        })
        .unwrap_or(false)
}

fn guidance_for_python_write(suggestion: &PythonWriteSuggestion) -> String {
    format!(
        "Blocked python command that writes files directly. Use apply_patch to edit files so changes stay reviewable.\n\n{}: {}",
        suggestion.label,
        suggestion.original_value
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RedundantCdSuggestion {
    label: &'static str,
    original_value: String,
    suggested: Vec<String>,
    target_arg: String,
    cwd: PathBuf,
}

fn detect_redundant_cd(argv: &[String], cwd: &Path) -> Option<RedundantCdSuggestion> {
    let normalized_cwd = normalize_path(cwd);
    if let Some((script_index, script)) = extract_shell_script_from_wrapper(argv) {
        if let Some(suggestion) = detect_redundant_cd_in_shell(
            argv,
            script_index,
            &script,
            cwd,
            &normalized_cwd,
        ) {
            return Some(suggestion);
        }
    }
    detect_redundant_cd_in_argv(argv, cwd, &normalized_cwd)
}

fn detect_redundant_cd_in_shell(
    argv: &[String],
    script_index: usize,
    script: &str,
    cwd: &Path,
    normalized_cwd: &Path,
) -> Option<RedundantCdSuggestion> {
    let trimmed = script.trim_start();
    let tokens = shlex_split(trimmed)?;
    if tokens.len() < 3 {
        return None;
    }
    if tokens.first().map(String::as_str) != Some("cd") {
        return None;
    }
    let target = tokens.get(1)?.clone();
    if !is_simple_cd_target(&target) {
        return None;
    }
    let resolved_target = resolve_cd_target(&target, cwd)?;
    if resolved_target != normalized_cwd {
        return None;
    }

    let mut idx = 2;
    let mut saw_connector = false;
    while idx < tokens.len() && is_connector(&tokens[idx]) {
        saw_connector = true;
        idx += 1;
    }
    if !saw_connector || idx >= tokens.len() {
        return None;
    }

    let remainder_tokens = tokens[idx..].to_vec();
    let suggested_script = shlex_try_join(remainder_tokens.iter().map(|s| s.as_str()))
        .unwrap_or_else(|_| remainder_tokens.join(" "));
    if suggested_script.trim().is_empty() {
        return None;
    }

    let mut suggested = argv.to_vec();
    suggested[script_index] = suggested_script;

    Some(RedundantCdSuggestion {
        label: "original_script",
        original_value: script.to_string(),
        suggested,
        target_arg: target,
        cwd: normalized_cwd.to_path_buf(),
    })
}

fn detect_redundant_cd_in_argv(
    argv: &[String],
    cwd: &Path,
    normalized_cwd: &Path,
) -> Option<RedundantCdSuggestion> {
    if argv.len() < 4 {
        return None;
    }
    if argv.first().map(String::as_str) != Some("cd") {
        return None;
    }
    let target = argv.get(1)?.clone();
    if !is_simple_cd_target(&target) {
        return None;
    }
    let resolved_target = resolve_cd_target(&target, cwd)?;
    if resolved_target != normalized_cwd {
        return None;
    }

    let mut idx = 2;
    let mut saw_connector = false;
    while idx < argv.len() && is_connector(&argv[idx]) {
        saw_connector = true;
        idx += 1;
    }
    if !saw_connector || idx >= argv.len() {
        return None;
    }

    let suggested = argv[idx..].to_vec();
    if suggested.is_empty() {
        return None;
    }

    Some(RedundantCdSuggestion {
        label: "original_argv",
        original_value: format!("{:?}", argv),
        suggested,
        target_arg: target,
        cwd: normalized_cwd.to_path_buf(),
    })
}

fn resolve_cd_target(target: &str, cwd: &Path) -> Option<PathBuf> {
    if target.is_empty() {
        return None;
    }
    let candidate = if Path::new(target).is_absolute() {
        PathBuf::from(target)
    } else {
        cwd.join(target)
    };
    Some(normalize_path(candidate.as_path()))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::Prefix(prefix) => {
                normalized = PathBuf::from(prefix.as_os_str());
            }
            Component::RootDir => {
                normalized.push(component.as_os_str());
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn is_simple_cd_target(target: &str) -> bool {
    if target.is_empty() || target == "-" {
        return false;
    }
    !target.chars().any(|ch| matches!(ch, '$' | '`' | '*' | '?' | '[' | ']' | '{' | '}' | '(' | ')' | '|' | '>' | '<' | '!'))
}

fn is_connector(token: &str) -> bool {
    matches!(token, "&&" | ";" | "||")
}

fn guidance_for_redundant_cd(suggestion: &RedundantCdSuggestion) -> String {
    let suggested = serde_json::to_string(&suggestion.suggested)
        .unwrap_or_else(|_| "<failed to serialize suggested argv>".to_string());
    let target_display = shlex_try_join(std::iter::once(suggestion.target_arg.as_str()))
        .unwrap_or_else(|_| suggestion.target_arg.clone());
    format!(
        "Leading cd {target_display} is redundant because the command already runs in {}. Drop the prefix before retrying.\n\n{}: {}\nresend_exact_argv: {}",
        suggestion.cwd.display(),
        suggestion.label,
        suggestion.original_value,
        suggested
    )
}

#[cfg(test)]
mod command_guard_detection_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_shell_redundant_cd() {
        let cwd = PathBuf::from("/tmp/project");
        let argv = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cd /tmp/project && ls".to_string(),
        ];

        let suggestion = detect_redundant_cd(&argv, &cwd).expect("should flag redundant cd");
        assert_eq!(suggestion.label, "original_script");
        assert_eq!(suggestion.suggested, vec!["bash".to_string(), "-lc".to_string(), "ls".to_string()]);
    }

    #[test]
    fn ignores_cd_to_different_directory() {
        let cwd = PathBuf::from("/tmp/project");
        let argv = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cd /tmp/project/src && ls".to_string(),
        ];

        assert!(detect_redundant_cd(&argv, &cwd).is_none());
    }

    #[test]
    fn skips_dynamic_cd_targets() {
        let cwd = PathBuf::from("/tmp/project");
        let argv = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cd $PWD && ls".to_string(),
        ];

        assert!(detect_redundant_cd(&argv, &cwd).is_none());
    }

    #[test]
    fn detects_cat_heredoc_write() {
        let argv = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cat <<'EOF' > code-rs/git-tooling/Cargo.toml\n[package]\nname = \"demo\"\nEOF".to_string(),
        ];

        let suggestion = detect_cat_write(&argv).expect("should flag cat write");
        assert_eq!(suggestion.label, "original_script");
        assert!(suggestion
            .original_value
            .contains("cat <<'EOF' > code-rs/git-tooling/Cargo.toml"));
    }

    #[test]
    fn allows_cat_heredoc_without_redirect() {
        let argv = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cat <<'EOF'\nhello\nEOF".to_string(),
        ];

        assert!(detect_cat_write(&argv).is_none());
    }

    #[test]
    fn allows_cat_redirect_to_fd() {
        let argv = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "cat <<'EOF' >&2\nwarn\nEOF".to_string(),
        ];

        assert!(detect_cat_write(&argv).is_none());
    }

    #[test]
    fn detects_python_here_doc_write() {
        let argv = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "python3 - <<'PY'\nfrom pathlib import Path\nPath('docs.txt').write_text('hello')\nPY".to_string(),
        ];

        let suggestion = detect_python_write(&argv).expect("should flag python write");
        assert_eq!(suggestion.label, "original_script");
        assert!(suggestion.original_value.contains("write_text"));
    }

    #[test]
    fn detects_python_inline_write() {
        let argv = vec![
            "python3".to_string(),
            "-c".to_string(),
            "from pathlib import Path; Path('foo.txt').write_text('hi')".to_string(),
        ];

        let suggestion = detect_python_write(&argv).expect("should flag inline python write");
        assert_eq!(suggestion.label, "python_inline_script");
        assert!(suggestion.original_value.contains("write_text"));
    }

    #[test]
    fn allows_read_only_python() {
        let argv = vec![
            "python3".to_string(),
            "-c".to_string(),
            "print('hello world')".to_string(),
        ];

        assert!(detect_python_write(&argv).is_none());
    }
}

fn debug_history(label: &str, items: &[ResponseItem]) {
    let preview: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(idx, item)| match item {
            ResponseItem::Message { role, content, .. } => {
                let text = content
                    .iter()
                    .filter_map(|c| match c {
                        ContentItem::InputText { text }
                        | ContentItem::OutputText { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let snippet: String = text.chars().take(80).collect();
                format!("{idx}:{role}:{snippet}")
            }
            _ => format!("{idx}:{:?}", item),
        })
        .collect();
    let rendered = preview.join(" | ");
    if std::env::var_os("CODEX_COMPACT_TRACE").is_some() {
        eprintln!("[compact_history] {} => [{}]", label, rendered);
    }
    info!(target = "code_core::compact_history", "{} => [{}]", label, rendered);
}
