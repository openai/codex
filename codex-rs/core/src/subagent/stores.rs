//! Session-scoped subagent stores with global registry.
//!
//! This module provides a global registry pattern for managing subagent stores
//! keyed by conversation_id. This avoids modifying Session/codex.rs while
//! ensuring stores persist across turns within a session.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use tokio::sync::oneshot;

// Note: inject_call_count uses Ordering::Relaxed because:
// 1. We only need monotonic increment, not synchronization
// 2. Exact count doesn't need to be synchronized across threads

use dashmap::DashMap;

use super::AgentRegistry;
use super::BackgroundTaskStore;
use super::TranscriptStore;
use crate::config::system_reminder::SystemReminderConfig;
use crate::error::CodexErr;
use crate::plan_mode::PlanModeState;
use crate::plan_mode::cleanup_plan_slug;
use crate::system_reminder::FileTracker;
use crate::system_reminder::PlanState;
use crate::system_reminder::PlanStep;
use crate::system_reminder::SystemReminderOrchestrator;
use codex_protocol::ConversationId;
use codex_protocol::plan_tool::UpdatePlanArgs;
use codex_protocol::protocol_ext::PlanExitPermissionMode;

/// Approved plan content after ExitPlanMode approval.
///
/// Used for one-time injection into context via PlanApprovedGenerator.
#[derive(Debug, Clone)]
pub struct ApprovedPlan {
    /// Full plan content
    pub content: String,
    /// Path to the plan file
    pub file_path: String,
}

/// Session-scoped subagent stores.
///
/// These stores maintain state that must persist across turns within a session:
/// - AgentRegistry: Caches loaded agent definitions
/// - BackgroundTaskStore: Tracks background subagent tasks
/// - TranscriptStore: Records agent transcripts for resume functionality
/// - ReminderOrchestrator: Cached system reminder orchestrator (avoids per-turn allocation)
/// - FileTracker: Tracks file reads for change detection
/// - PlanState: Tracks plan state for reminder generation
/// - PlanModeState: Tracks plan mode state (active, file path, re-entry)
/// - inject_call_count: Tracks main agent reminder injection calls
/// - pending_user_answers: Stores pending answers from AskUserQuestion tool
/// - approved_plan: Approved plan content for post-ExitPlanMode injection
/// - permission_mode: Post-plan permission mode for auto-approval
#[derive(Debug)]
pub struct SubagentStores {
    pub registry: Arc<AgentRegistry>,
    pub background_store: Arc<BackgroundTaskStore>,
    pub transcript_store: Arc<TranscriptStore>,
    pub reminder_orchestrator: Arc<SystemReminderOrchestrator>,
    pub file_tracker: Arc<FileTracker>,
    pub plan_state: Arc<RwLock<PlanState>>,
    /// Plan mode state (is_active, plan_file_path, re-entry detection).
    pub plan_mode: Arc<RwLock<PlanModeState>>,
    /// Counter for main agent reminder injection calls.
    /// Used by PlanReminderGenerator to determine if reminder should fire.
    inject_call_count: AtomicI32,
    /// Pending answers from AskUserQuestion tool, keyed by tool_call_id.
    pending_user_answers: Arc<RwLock<std::collections::HashMap<String, String>>>,
    /// Oneshot channels for AskUserQuestion answer injection.
    /// Key: tool_call_id, Value: oneshot sender for receiving user answer.
    /// This enables the handler to block until the user responds.
    user_answer_channels: Arc<RwLock<std::collections::HashMap<String, oneshot::Sender<String>>>>,
    /// Approved plan content after ExitPlanMode approval.
    /// Consumed by PlanApprovedGenerator for one-time injection.
    approved_plan: Arc<RwLock<Option<ApprovedPlan>>>,
    /// Post-plan permission mode for auto-approval.
    /// Set when user approves ExitPlanMode with a permission mode.
    permission_mode: Arc<RwLock<Option<PlanExitPermissionMode>>>,
}

/// Build default search paths for custom agent discovery.
///
/// Search order:
/// 1. `~/.config/codex/agents/` - User config directory
/// 2. `~/.codex/agents/` - User home directory
/// 3. `.codex/agents/` - Project local directory
fn build_default_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. User config directory (~/.config/codex/agents/ on Linux/macOS)
    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("codex").join("agents"));
    }

    // 2. User home directory (~/.codex/agents/)
    if let Some(home_dir) = dirs::home_dir() {
        paths.push(home_dir.join(".codex").join("agents"));
    }

    // 3. Project local directory (.codex/agents/)
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join(".codex").join("agents"));
    }

    paths
}

impl SubagentStores {
    pub fn new() -> Self {
        let search_paths = build_default_search_paths();
        Self {
            registry: Arc::new(AgentRegistry::with_search_paths(search_paths)),
            background_store: Arc::new(BackgroundTaskStore::new()),
            transcript_store: Arc::new(TranscriptStore::new()),
            reminder_orchestrator: Arc::new(SystemReminderOrchestrator::new(
                SystemReminderConfig::default(),
            )),
            file_tracker: Arc::new(FileTracker::new()),
            plan_state: Arc::new(RwLock::new(PlanState::default())),
            plan_mode: Arc::new(RwLock::new(PlanModeState::new())),
            inject_call_count: AtomicI32::new(0),
            pending_user_answers: Arc::new(RwLock::new(std::collections::HashMap::new())),
            user_answer_channels: Arc::new(RwLock::new(std::collections::HashMap::new())),
            approved_plan: Arc::new(RwLock::new(None)),
            permission_mode: Arc::new(RwLock::new(None)),
        }
    }

    /// Increment and return the new inject call count.
    /// Only call this for main agent turns.
    pub fn increment_inject_count(&self) -> i32 {
        self.inject_call_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Get the current inject call count.
    pub fn get_inject_count(&self) -> i32 {
        self.inject_call_count.load(Ordering::Relaxed)
    }

    /// Update plan state from UpdatePlanArgs.
    ///
    /// Called by the update_plan handler to track plan state for reminder generation.
    pub fn update_plan_state(
        &self,
        args: &UpdatePlanArgs,
        current_count: i32,
    ) -> Result<(), CodexErr> {
        let mut state = self
            .plan_state
            .write()
            .map_err(|_| CodexErr::Fatal("plan_state lock poisoned".to_string()))?;
        state.steps = args
            .plan
            .iter()
            .map(|item| PlanStep {
                step: item.step.clone(),
                status: format!("{:?}", item.status).to_lowercase(),
            })
            .collect();
        state.is_empty = state.steps.is_empty();
        state.last_update_count = current_count;
        Ok(())
    }

    /// Get a snapshot of the current plan state.
    pub fn get_plan_state(&self) -> Result<PlanState, CodexErr> {
        self.plan_state
            .read()
            .map_err(|_| CodexErr::Fatal("plan_state lock poisoned".to_string()))
            .map(|state| state.clone())
    }

    // ========================================================================
    // Plan Mode helpers
    // ========================================================================

    /// Enter Plan Mode and return the plan file path.
    pub fn enter_plan_mode(
        &self,
        conversation_id: ConversationId,
    ) -> Result<std::path::PathBuf, CodexErr> {
        let mut state = self
            .plan_mode
            .write()
            .map_err(|_| CodexErr::Fatal("plan_mode lock poisoned".to_string()))?;
        state.enter(conversation_id)
    }

    /// Exit Plan Mode.
    pub fn exit_plan_mode(&self, approved: bool) -> Result<(), CodexErr> {
        let mut state = self
            .plan_mode
            .write()
            .map_err(|_| CodexErr::Fatal("plan_mode lock poisoned".to_string()))?;
        state.exit(approved);
        Ok(())
    }

    /// Get a snapshot of the current plan mode state.
    pub fn get_plan_mode_state(&self) -> Result<PlanModeState, CodexErr> {
        self.plan_mode
            .read()
            .map_err(|_| CodexErr::Fatal("plan_mode lock poisoned".to_string()))
            .map(|state| state.clone())
    }

    /// Check if Plan Mode is currently active.
    pub fn is_plan_mode_active(&self) -> Result<bool, CodexErr> {
        self.plan_mode
            .read()
            .map_err(|_| CodexErr::Fatal("plan_mode lock poisoned".to_string()))
            .map(|state| state.is_active)
    }

    /// Clear the re-entry flag after re-entry prompt is shown.
    ///
    /// Called after the first reminder injection when re-entering Plan Mode
    /// to avoid repeated re-entry prompts on subsequent turns.
    pub fn clear_plan_reentry(&self) -> Result<(), CodexErr> {
        let mut state = self
            .plan_mode
            .write()
            .map_err(|_| CodexErr::Fatal("plan_mode lock poisoned".to_string()))?;
        state.clear_reentry();
        Ok(())
    }

    // ========================================================================
    // AskUserQuestion helpers
    // ========================================================================

    /// Store a pending user answer for an AskUserQuestion tool call.
    pub fn set_pending_user_answer(&self, tool_call_id: String, answer: String) {
        if let Ok(mut answers) = self.pending_user_answers.write() {
            answers.insert(tool_call_id, answer);
        }
    }

    /// Get and remove a pending user answer for an AskUserQuestion tool call.
    pub fn take_pending_user_answer(&self, tool_call_id: &str) -> Option<String> {
        if let Ok(mut answers) = self.pending_user_answers.write() {
            answers.remove(tool_call_id)
        } else {
            None
        }
    }

    /// Create a oneshot channel for receiving user answer.
    ///
    /// The handler calls this to get a receiver that it will await.
    /// When the user responds, `send_user_answer` is called to send
    /// the answer through the channel, unblocking the handler.
    ///
    /// Returns the receiver that the handler should await.
    pub fn create_answer_channel(&self, tool_call_id: &str) -> oneshot::Receiver<String> {
        let (tx, rx) = oneshot::channel();
        if let Ok(mut channels) = self.user_answer_channels.write() {
            channels.insert(tool_call_id.to_string(), tx);
        }
        rx
    }

    /// Send user answer through the oneshot channel.
    ///
    /// Called by codex_ext.rs when the user responds to an AskUserQuestion.
    /// This unblocks the handler's await and allows it to return the answer
    /// as the tool result.
    ///
    /// Returns true if the answer was sent successfully, false if the channel
    /// was not found or was already closed.
    pub fn send_user_answer(&self, tool_call_id: &str, answer: String) -> bool {
        if let Ok(mut channels) = self.user_answer_channels.write() {
            if let Some(tx) = channels.remove(tool_call_id) {
                return tx.send(answer).is_ok();
            }
        }
        false
    }

    // ========================================================================
    // Approved Plan helpers (for post-ExitPlanMode injection)
    // ========================================================================

    /// Set the approved plan content for one-time injection.
    ///
    /// Called by codex_ext.rs when user approves ExitPlanMode.
    pub fn set_approved_plan(&self, plan: ApprovedPlan) {
        if let Ok(mut approved) = self.approved_plan.write() {
            *approved = Some(plan);
        }
    }

    /// Take (consume) the approved plan for injection.
    ///
    /// Returns the plan and clears the stored value.
    /// Called by PlanApprovedGenerator for one-time injection.
    pub fn take_approved_plan(&self) -> Option<ApprovedPlan> {
        if let Ok(mut approved) = self.approved_plan.write() {
            approved.take()
        } else {
            None
        }
    }

    /// Check if there's an approved plan pending injection.
    pub fn has_approved_plan(&self) -> bool {
        self.approved_plan
            .read()
            .map(|p| p.is_some())
            .unwrap_or(false)
    }

    /// Get the plan file path from plan mode state.
    pub fn get_plan_file_path(&self) -> Option<std::path::PathBuf> {
        self.plan_mode
            .read()
            .ok()
            .and_then(|state| state.plan_file_path.clone())
    }

    // ========================================================================
    // Permission Mode helpers (for post-plan auto-approval)
    // ========================================================================

    /// Set the permission mode for post-plan auto-approval.
    ///
    /// Called by codex_ext.rs when user approves ExitPlanMode with a permission mode.
    pub fn set_permission_mode(&self, mode: PlanExitPermissionMode) {
        if let Ok(mut pm) = self.permission_mode.write() {
            *pm = Some(mode);
        }
    }

    /// Get the current permission mode.
    pub fn get_permission_mode(&self) -> Option<PlanExitPermissionMode> {
        self.permission_mode.read().ok().and_then(|pm| pm.clone())
    }

    /// Clear the permission mode (e.g., on session end or new plan mode).
    pub fn clear_permission_mode(&self) {
        if let Ok(mut pm) = self.permission_mode.write() {
            *pm = None;
        }
    }

    /// Check if a tool should be auto-approved based on permission mode.
    ///
    /// - `BypassPermissions`: Auto-approve all tools
    /// - `AcceptEdits`: Auto-approve file edit tools only
    /// - `Default`: No auto-approval
    pub fn should_auto_approve(&self, tool_name: &str) -> bool {
        match self.get_permission_mode() {
            Some(PlanExitPermissionMode::BypassPermissions) => true,
            Some(PlanExitPermissionMode::AcceptEdits) => {
                matches!(
                    tool_name,
                    "write_file" | "smart_edit" | "str_replace_based_edit_tool"
                )
            }
            Some(PlanExitPermissionMode::Default) | None => false,
        }
    }
}

impl Default for SubagentStores {
    fn default() -> Self {
        Self::new()
    }
}

/// Global registry mapping conversation_id to session-scoped stores.
///
/// Using LazyLock + DashMap for thread-safe lazy initialization with
/// concurrent access support.
static STORES_REGISTRY: LazyLock<DashMap<ConversationId, Arc<SubagentStores>>> =
    LazyLock::new(DashMap::new);

/// Get or create stores for a session by conversation_id.
///
/// This is the main entry point for handlers to access session-scoped stores.
/// The stores are created on first access and reused for subsequent calls
/// with the same conversation_id.
///
/// # Example
/// ```ignore
/// let stores = get_or_create_stores(session.conversation_id);
/// // Use stores.background_store, stores.transcript_store, etc.
/// ```
pub fn get_or_create_stores(conversation_id: ConversationId) -> Arc<SubagentStores> {
    STORES_REGISTRY
        .entry(conversation_id)
        .or_insert_with(|| Arc::new(SubagentStores::new()))
        .clone()
}

/// Cleanup stores when session ends.
///
/// Should be called when a session is terminated to free memory.
/// Not calling this won't cause memory leaks for short-lived processes,
/// but long-running servers should call this on session cleanup.
///
/// Also cleans up plan slug cache to ensure new sessions get fresh slugs.
pub fn cleanup_stores(conversation_id: &ConversationId) {
    STORES_REGISTRY.remove(conversation_id);
    cleanup_plan_slug(conversation_id);
}

/// Get stores if they exist (without creating new ones).
///
/// Useful for operations that should only work on existing sessions.
pub fn get_stores(conversation_id: &ConversationId) -> Option<Arc<SubagentStores>> {
    STORES_REGISTRY.get(conversation_id).map(|r| r.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_default_search_paths() {
        let paths = build_default_search_paths();

        // Should have at least project local path
        assert!(!paths.is_empty());

        // All paths should end with "agents"
        for path in &paths {
            assert!(
                path.ends_with("agents"),
                "Path should end with 'agents': {path:?}"
            );
        }
    }

    #[test]
    fn test_get_or_create_stores() {
        let conv_id = ConversationId::new();

        // First access creates stores
        let stores1 = get_or_create_stores(conv_id);

        // Second access returns same stores
        let stores2 = get_or_create_stores(conv_id);

        // Both should point to same Arc
        assert!(Arc::ptr_eq(&stores1, &stores2));

        // Cleanup
        cleanup_stores(&conv_id);

        // After cleanup, get_stores returns None
        assert!(get_stores(&conv_id).is_none());
    }

    #[test]
    fn test_different_sessions_have_different_stores() {
        let conv_id1 = ConversationId::new();
        let conv_id2 = ConversationId::new();

        let stores1 = get_or_create_stores(conv_id1);
        let stores2 = get_or_create_stores(conv_id2);

        // Different sessions should have different stores
        assert!(!Arc::ptr_eq(&stores1, &stores2));

        // Cleanup
        cleanup_stores(&conv_id1);
        cleanup_stores(&conv_id2);
    }
}
