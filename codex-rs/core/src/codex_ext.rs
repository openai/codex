//! Extension functions for Codex lifecycle management.
//!
//! This module provides extension functions that hook into Codex lifecycle events
//! without modifying core files directly, minimizing upstream merge conflicts.

use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use std::path::Path;
use std::path::PathBuf;

use crate::config::system_reminder::LspDiagnosticsMinSeverity;
use crate::config::system_reminder::SystemReminderConfig;
use crate::shell_background::get_global_shell_store;
use crate::subagent::cleanup_session_state;
use crate::subagent::expect_session_state;
use crate::system_reminder::FileTracker;
use crate::system_reminder::PlanState;
use crate::system_reminder::SystemReminderOrchestrator;
use crate::system_reminder::generator::BackgroundTaskType;
use crate::system_reminder_inject::build_generator_context;
use crate::system_reminder_inject::inject_system_reminders;
use crate::tools::spec::ToolsConfig;
use crate::tools::spec_ext::ToolFilter;
use codex_lsp::DiagnosticsStore;
use std::sync::Arc;

/// Clean up session-scoped resources when conversation ends.
///
/// Called from `handlers::shutdown()` in `codex.rs` to ensure proper cleanup
/// of subagent stores (AgentRegistry, BackgroundTaskStore, TranscriptStore),
/// background shells, and session-scoped hooks.
///
/// This prevents memory leaks in long-running server deployments where
/// conversations accumulate without cleanup.
pub fn cleanup_session_resources(conversation_id: &ThreadId) {
    cleanup_session_state(conversation_id);

    // Clean up session-scoped hooks for this conversation
    crate::hooks_ext::clear_session_hooks(&conversation_id.to_string());

    // Clean up background shells for this conversation
    // This kills running shells and removes all shells associated with this session
    let store = get_global_shell_store();
    store.cleanup_by_conversation(conversation_id);

    // Also clean up old shells from other conversations (time-based fallback)
    // This ensures shells are cleaned even if cleanup_by_conversation missed any
    store.cleanup_old(std::time::Duration::from_secs(3600)); // 1 hour
}

/// Apply plan mode and subagent tool filters, then log loaded tools.
///
/// Called from `make_turn_context()` in `codex.rs` to minimize upstream changes.
///
/// Filter priority:
/// 1. Plan Mode filter (if active) - restricts to read-only tools + plan file
/// 2. Subagent filter (if configured) - overrides plan mode filter
pub(crate) fn apply_tool_filter(
    tools_config: &mut ToolsConfig,
    ext_tool_filter: Option<&ToolFilter>,
    conversation_id: ThreadId,
    model: &str,
) {
    // Apply Plan Mode tool filter if active (read-only tools + plan file write)
    let stores = expect_session_state(&conversation_id);
    if stores.is_plan_mode_active().unwrap_or(false) {
        if let Ok(plan_state) = stores.get_plan_mode_state() {
            tools_config.tool_filter = Some(ToolFilter::for_plan_mode(
                plan_state.plan_file_path.as_deref(),
            ));
        }
    }

    // Apply tool filter if configured (for subagent sessions)
    // Note: subagent filter overrides plan mode filter if both are set
    if let Some(filter) = ext_tool_filter {
        tools_config.tool_filter = Some(filter.clone());
    }

    // Log loaded tools
    let (tools, _) = crate::tools::spec::build_specs(tools_config, None).build();
    crate::tools::log_loaded_tools(&tools, model);
}

/// Inject system reminders into conversation history.
///
/// This is called before the turn is sent to the model to:
/// - Notify about completed/updated background tasks (shells, agents)
/// - Inject plan mode instructions
/// - Notify about changed files
/// - Include critical instructions
///
/// Returns the task IDs that were notified.
pub async fn run_system_reminder_injection(
    history: &mut Vec<ResponseItem>,
    agent_id: &str,
    is_main_agent: bool,
    cwd: &Path,
    is_plan_mode: bool,
    plan_file_path: Option<&str>,
    conversation_id: Option<&ThreadId>,
    critical_instruction: Option<&str>,
    diagnostics_store: Option<Arc<DiagnosticsStore>>,
) -> Vec<String> {
    let shell_store = get_global_shell_store();

    // Get or create stores (also provides cached orchestrator)
    // Use get_or_create to ensure orchestrator is available even for new conversations
    let agent_stores = conversation_id.map(|id| expect_session_state(id));

    // Increment inject call count for main agent only
    // This is used by PlanReminderGenerator to determine if reminder should fire
    let current_count = if is_main_agent {
        agent_stores
            .as_ref()
            .map(|s| s.increment_inject_count())
            .unwrap_or(0)
    } else {
        agent_stores
            .as_ref()
            .map(|s| s.get_inject_count())
            .unwrap_or(0)
    };

    // Collect shell tasks (filtered by conversation)
    let mut background_tasks = shell_store.list_for_reminder(conversation_id);

    // Collect subagent tasks (if stores exist for this conversation)
    if let Some(ref stores) = agent_stores {
        background_tasks.extend(stores.background_store.list_for_reminder());
    }

    // NOTE: Do NOT early return here even if background_tasks is empty!
    // Other generators (PlanReminder, ChangedFiles, etc.) need to run regardless.

    // Collect task IDs for marking as notified (grouped by type)
    let notified_ids: Vec<String> = background_tasks
        .iter()
        .filter(|t| !t.notified)
        .map(|t| t.task_id.clone())
        .collect();

    // Use cached orchestrator from stores, or create fallback for edge cases
    let fallback_orchestrator;
    let orchestrator: &SystemReminderOrchestrator = match &agent_stores {
        Some(stores) => &stores.reminder_orchestrator,
        None => {
            fallback_orchestrator =
                SystemReminderOrchestrator::new(SystemReminderConfig::default());
            &fallback_orchestrator
        }
    };

    // Use file tracker from stores for change detection, or fallback to empty
    let fallback_file_tracker;
    let file_tracker: &FileTracker = match &agent_stores {
        Some(stores) => &stores.file_tracker,
        None => {
            fallback_file_tracker = FileTracker::new();
            &fallback_file_tracker
        }
    };

    // Use plan state from stores for reminder tracking, or fallback to empty
    let fallback_plan_state;
    let plan_state: PlanState = match &agent_stores {
        Some(stores) => stores.get_plan_state().unwrap_or_default(),
        None => {
            fallback_plan_state = PlanState::default();
            fallback_plan_state
        }
    };

    // diagnostics_store is passed from caller (from session.services.lsp_manager)

    // Detect re-entry: user re-enters Plan Mode with existing plan file from previous session
    let is_plan_reentry = if is_plan_mode && is_main_agent {
        agent_stores
            .as_ref()
            .and_then(|s| s.get_plan_mode_state().ok())
            .map(|state| state.is_reentry())
            .unwrap_or(false)
    } else {
        false
    };

    // Take approved plan for one-time injection (if pending)
    // Convert to ApprovedPlanInfo for generator context
    let approved_plan = agent_stores
        .as_ref()
        .and_then(|s| s.take_approved_plan())
        .map(|p| crate::system_reminder::ApprovedPlanInfo {
            content: p.content,
            file_path: p.file_path,
        });

    let ctx = build_generator_context(
        current_count,
        agent_id,
        is_main_agent,
        true, // has_user_input
        None, // user_prompt - TODO: pass actual user prompt for @mention parsing
        cwd,
        is_plan_mode,
        plan_file_path,
        is_plan_reentry,
        file_tracker,
        &plan_state,
        &background_tasks,
        critical_instruction,
        diagnostics_store,
        LspDiagnosticsMinSeverity::default(), // Use default severity filtering (errors only)
        None,                                 // output_style - TODO: load from config
        approved_plan,                        // approved plan for one-time injection
        None, // restored_plan - TODO: integrate with compaction flow for plan file restoration
    );

    inject_system_reminders(history, orchestrator, &ctx).await;

    // Clear re-entry flag after first reminder injection to avoid repeated re-entry prompts
    if is_plan_reentry {
        if let Some(stores) = &agent_stores {
            if let Err(e) = stores.clear_plan_reentry() {
                tracing::warn!("failed to clear plan reentry flag: {e}");
            }
        }
    }

    // Mark tasks as notified using batch methods for efficiency
    // Group task IDs by type to reduce lock contention
    let shell_ids: Vec<String> = background_tasks
        .iter()
        .filter(|t| !t.notified && t.task_type == BackgroundTaskType::Shell)
        .map(|t| t.task_id.clone())
        .collect();

    let agent_ids: Vec<String> = background_tasks
        .iter()
        .filter(|t| !t.notified && t.task_type == BackgroundTaskType::AsyncAgent)
        .map(|t| t.task_id.clone())
        .collect();

    // Batch mark shells as notified
    if !shell_ids.is_empty() {
        shell_store.mark_all_notified(&shell_ids);
    }

    // Batch mark agents as notified
    if !agent_ids.is_empty() {
        if let Some(ref stores) = agent_stores {
            stores.background_store.mark_all_notified(&agent_ids);
        }
    }

    notified_ids
}

/// Simplified injection for use in codex.rs with minimal parameters.
///
/// This wraps `run_system_reminder_injection` for easier integration.
/// Called on each main agent turn to inject system reminders.
pub async fn maybe_inject_system_reminders(
    history: &mut Vec<ResponseItem>,
    cwd: &Path,
    conversation_id: Option<&ThreadId>,
    critical_instruction: Option<&str>,
    diagnostics_store: Option<Arc<DiagnosticsStore>>,
) {
    // Get plan mode state from stores
    let (is_plan_mode, plan_file_path) = conversation_id
        .map(|id| {
            let stores = expect_session_state(id);
            match stores.get_plan_mode_state() {
                Ok(state) => (
                    state.is_active,
                    state
                        .plan_file_path
                        .map(|p| p.to_string_lossy().to_string()),
                ),
                Err(e) => {
                    tracing::warn!("failed to get plan mode state: {e}");
                    (false, None)
                }
            }
        })
        .unwrap_or((false, None));

    let _ = run_system_reminder_injection(
        history,
        "main",
        true, // is_main_agent
        cwd,
        is_plan_mode,
        plan_file_path.as_deref(),
        conversation_id,
        critical_instruction,
        diagnostics_store,
    )
    .await;
}

// =============================================================================
// Plan Mode Handlers (called from codex.rs submission_loop)
// =============================================================================

use async_channel::Sender;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol_ext::ExtEventMsg;
use codex_protocol::protocol_ext::PlanExitPermissionMode;
use codex_protocol::protocol_ext::PlanModeEnteredEvent;
use codex_protocol::protocol_ext::PlanModeExitedEvent;

/// Unified handler for all extension Ops.
///
/// Dispatch happens here to minimize changes to codex.rs.
/// When adding new extension ops, add them to the match pattern in codex.rs
/// (single line) and add the handler logic here.
pub async fn handle_ext_op(conversation_id: ThreadId, tx_event: &Sender<Event>, op: Op) {
    match op {
        Op::SetPlanMode {
            active,
            plan_file_path,
        } => {
            handle_set_plan_mode(conversation_id, tx_event, active, plan_file_path.as_deref())
                .await;
        }
        Op::PlanModeApproval {
            approved,
            permission_mode,
        } => {
            handle_plan_mode_approval(conversation_id, tx_event, approved, permission_mode).await;
        }
        Op::EnterPlanModeApproval { approved } => {
            handle_enter_plan_mode_approval(conversation_id, tx_event, approved).await;
        }
        Op::UserQuestionAnswer {
            tool_call_id,
            answers,
        } => {
            handle_user_question_answer(conversation_id, tx_event, tool_call_id, answers).await;
        }
        _ => {} // Non-ext ops handled in codex.rs
    }
}

/// Handle Op::SetPlanMode - enter or configure plan mode.
///
/// This is called from `codex.rs` submission_loop to minimize changes to that file.
///
/// Plan file path uses cached slug (aligned with Claude Code):
/// - Same session = same plan file regardless of how many times /plan is called
/// - This enables proper re-entry detection
pub async fn handle_set_plan_mode(
    conversation_id: ThreadId,
    tx_event: &Sender<Event>,
    active: bool,
    _plan_file_path: Option<&str>, // Ignored - core generates with cached slug
) {
    if active {
        let stores = expect_session_state(&conversation_id);
        let path = match stores.enter_plan_mode(conversation_id) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("failed to enter plan mode: {e}");
                return;
            }
        };
        // Use path from enter_plan_mode (cached slug)
        let path_str = path.to_string_lossy().to_string();

        let event = Event {
            id: String::new(),
            msg: EventMsg::Ext(ExtEventMsg::PlanModeEntered(PlanModeEnteredEvent {
                plan_file_path: path_str,
            })),
        };
        if let Err(e) = tx_event.send(event).await {
            tracing::error!("failed to send PlanModeEntered event: {e}");
        }
    }
    // Note: exit is handled via Op::PlanModeApproval, not SetPlanMode { active: false }
}

/// Handle Op::PlanModeApproval - user approved or rejected the plan.
///
/// This is called from `codex.rs` submission_loop to minimize changes to that file.
///
/// # Arguments
/// * `permission_mode` - If approved, determines post-plan permission behavior:
///   - `BypassPermissions`: Auto-approve all tools
///   - `AcceptEdits`: Auto-approve file edits only
///   - `Default`: Manual approval for everything
pub async fn handle_plan_mode_approval(
    conversation_id: ThreadId,
    tx_event: &Sender<Event>,
    approved: bool,
    permission_mode: Option<PlanExitPermissionMode>,
) {
    use crate::subagent::ApprovedPlan;

    let stores = expect_session_state(&conversation_id);
    if let Err(e) = stores.exit_plan_mode(approved) {
        tracing::error!("failed to exit plan mode: {e}");
        // Continue to send event anyway to keep TUI in sync
    }

    if approved {
        // Read plan file and store for one-time injection via PlanApprovedGenerator
        if let Some(plan_path) = stores.get_plan_file_path() {
            match std::fs::read_to_string(&plan_path) {
                Ok(content) => {
                    stores.set_approved_plan(ApprovedPlan {
                        content,
                        file_path: plan_path.display().to_string(),
                    });
                    tracing::info!("Plan content stored for injection: {}", plan_path.display());
                }
                Err(e) => {
                    tracing::warn!("Failed to read plan file for injection: {e}");
                }
            }
        }

        // Apply permission mode for post-plan auto-approval
        if let Some(mode) = permission_mode {
            tracing::info!("Setting permission mode: {:?}", mode);
            stores.set_permission_mode(mode);
        }
    }

    let event = Event {
        id: String::new(),
        msg: EventMsg::Ext(ExtEventMsg::PlanModeExited(PlanModeExitedEvent {
            approved,
        })),
    };
    if let Err(e) = tx_event.send(event).await {
        tracing::error!("failed to send PlanModeExited event: {e}");
    }
}

/// Handle Op::EnterPlanModeApproval - user approved or rejected entering plan mode.
///
/// This is called from `codex.rs` submission_loop when the LLM requests to enter plan mode.
pub async fn handle_enter_plan_mode_approval(
    conversation_id: ThreadId,
    tx_event: &Sender<Event>,
    approved: bool,
) {
    if approved {
        let stores = expect_session_state(&conversation_id);
        match stores.enter_plan_mode(conversation_id) {
            Ok(plan_file_path) => {
                let event = Event {
                    id: String::new(),
                    msg: EventMsg::Ext(ExtEventMsg::PlanModeEntered(PlanModeEnteredEvent {
                        plan_file_path: plan_file_path.display().to_string(),
                    })),
                };
                if let Err(e) = tx_event.send(event).await {
                    tracing::error!("failed to send PlanModeEntered event: {e}");
                }
            }
            Err(e) => {
                tracing::error!("failed to enter plan mode: {e}");
            }
        }
    }
    // If not approved, no action needed - the tool will receive the rejection via normal flow
}

/// Handle Op::UserQuestionAnswer - user answered the LLM's question.
///
/// This is called from `codex.rs` submission_loop when the user answers an AskUserQuestion tool.
/// The tool_call_id is used to correlate the answer with the original tool call.
///
/// ## Answer Injection Mechanism
///
/// The handler (ask_user_question.rs) is blocking on a oneshot channel waiting
/// for the user's answer. When this function is called, it:
/// 1. Formats the user's answers
/// 2. Sends the answer through the channel via `stores.send_user_answer()`
/// 3. This unblocks the handler, which returns the answer as the tool result
/// 4. The LLM receives the actual user answer (not "Waiting for response...")
///
/// This matches Claude Code's `onAllow(updatedInput with answers)` callback mechanism.
#[allow(unused_variables)]
pub async fn handle_user_question_answer(
    conversation_id: ThreadId,
    tx_event: &Sender<Event>,
    tool_call_id: String,
    answers: std::collections::HashMap<String, String>,
) {
    // Log the user's answers for debugging
    tracing::info!(
        "Received user question answer for tool_call_id={}: {:?}",
        tool_call_id,
        answers
    );

    // Format answers for injection into conversation
    let formatted_answers: Vec<String> = answers
        .iter()
        .map(|(header, answer)| format!("{}: {}", header, answer))
        .collect();
    let answers_text = if formatted_answers.is_empty() {
        "User cancelled or provided no answer.".to_string()
    } else {
        formatted_answers.join("\n")
    };

    // Send the answer through the oneshot channel.
    // This unblocks the handler which is awaiting on the receiver.
    let stores = expect_session_state(&conversation_id);
    let sent = stores.send_user_answer(&tool_call_id, answers_text);

    if sent {
        tracing::debug!("Successfully sent answer through channel for tool_call_id={tool_call_id}");
    } else {
        // Channel was not found or already closed.
        // This can happen if the handler timed out or the session ended.
        tracing::warn!("Failed to send answer - channel not found for tool_call_id={tool_call_id}");
        // Fallback: store in pending_user_answers for potential retry
        stores.set_pending_user_answer(
            tool_call_id,
            "Answer received but channel was closed.".to_string(),
        );
    }
}

// =============================================================================
// Retrieval Initialization (called from entry points: exec, tui, app-server)
// =============================================================================

/// Initialize retrieval service for a workdir (non-blocking).
///
/// This function:
/// 1. Checks if code_search feature is enabled
/// 2. Spawns background task to initialize service
/// 3. Returns immediately - service initializes asynchronously
///
/// The initialized service is automatically cached globally by `RetrievalService`.
/// Tool handlers access it via `RetrievalService::for_workdir()` which returns
/// the same cached instance.
///
/// # Arguments
/// * `cwd` - Working directory for the retrieval service
/// * `include_code_search` - Whether code_search feature is enabled in config
///
/// # Example
/// ```ignore
/// // In entry point (exec, tui, app-server) after cwd is known:
/// let default_cwd = config.cwd.to_path_buf();
/// codex_core::spawn_retrieval_init(&default_cwd, config.include_code_search);
/// ```
pub fn spawn_retrieval_init(cwd: &Path, include_code_search: bool) {
    if !include_code_search {
        tracing::debug!("Retrieval init skipped: code_search feature not enabled");
        return;
    }

    let cwd: PathBuf = cwd.to_path_buf();
    tokio::spawn(async move {
        match codex_retrieval::RetrievalFacade::for_workdir(&cwd).await {
            Ok(service) => {
                // Start worker pool for background indexing
                if let Err(e) = service.start_pipeline().await {
                    tracing::warn!("Failed to start retrieval workers: {e}");
                    return;
                }

                // Trigger file scanning + indexing
                if let Err(e) = service.index_service().trigger_session_start().await {
                    tracing::warn!("Failed to trigger retrieval indexing: {e}");
                }

                // Pre-warm BM25 for fast first search
                if let Err(e) = service.search_service().warmup().await {
                    tracing::debug!("BM25 warmup skipped: {e}");
                }

                tracing::info!(
                    workdir = %cwd.display(),
                    "Retrieval service initialized"
                );
            }
            Err(codex_retrieval::RetrievalErr::NotEnabled) => {
                tracing::debug!(
                    workdir = %cwd.display(),
                    "Retrieval not configured (no retrieval.toml)"
                );
            }
            Err(e) => {
                tracing::warn!(
                    workdir = %cwd.display(),
                    error = %e,
                    "Retrieval init failed"
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagent::get_session_state;
    use crate::subagent::init_session_state;
    use std::path::PathBuf;

    #[test]
    fn test_cleanup_session_resources() {
        let conv_id = ThreadId::new();
        let codex_home = PathBuf::from("/tmp/codex_home");
        let cwd = PathBuf::from("/tmp/project");

        // Create stores
        let _ = init_session_state(conv_id, &codex_home, &cwd);
        assert!(get_session_state(&conv_id).is_some());

        // Cleanup
        cleanup_session_resources(&conv_id);

        // Verify cleanup
        assert!(get_session_state(&conv_id).is_none());
    }
}
