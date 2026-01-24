//! Session-scoped state with global registry.
//!
//! This module provides a global registry pattern for managing session-scoped state
//! keyed by conversation_id. This avoids modifying Session/codex.rs while
//! ensuring state persists across turns within a session.
//!
//! The `SessionScopedState` stores various subsystem states:
//! - Agent subsystem: registry, background tasks, transcripts
//! - Plan Mode subsystem: plan state, mode state, approved plans
//! - System Reminder subsystem: orchestrator, file tracker
//! - User Interaction: pending answers, answer channels

use std::path::Path;
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
use codex_protocol::ThreadId;
use codex_protocol::config_types::PlanModeApprovalPolicy;
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

/// Session-scoped state container.
///
/// Maintains state that must persist across turns within a session:
///
/// ## Agent Subsystem
/// - `registry`: Caches loaded agent definitions
/// - `background_store`: Tracks background subagent tasks
/// - `transcript_store`: Records agent transcripts for resume functionality
///
/// ## System Reminder Subsystem
/// - `reminder_orchestrator`: Cached system reminder orchestrator
/// - `file_tracker`: Tracks file reads for change detection
/// - `inject_call_count`: Tracks main agent reminder injection calls
///
/// ## Plan Mode Subsystem
/// - `plan_state`: Tracks todo/plan state for reminder generation
/// - `plan_mode`: Tracks plan mode state (active, file path, re-entry)
/// - `approved_plan`: Approved plan content for post-ExitPlanMode injection
/// - `permission_mode`: Post-plan permission mode for auto-approval
/// - `plan_mode_approval_policy`: Controls EnterPlanMode/ExitPlanMode approval
///
/// ## User Interaction
/// - `pending_user_answers`: Stores pending answers from AskUserQuestion tool
/// - `user_answer_channels`: Oneshot channels for blocking on user response
#[derive(Debug)]
pub struct SessionScopedState {
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
    /// Plan mode approval policy (controls whether EnterPlanMode/ExitPlanMode need user approval).
    /// Independent from permission_mode which controls subsequent tool approval.
    plan_mode_approval_policy: Arc<RwLock<PlanModeApprovalPolicy>>,
    /// Codex home directory (for plan files, todos, etc.).
    codex_home: PathBuf,
}

/// Build search paths for custom agent discovery.
///
/// Search order:
/// 1. `~/.config/codex/agents/` - User config directory
/// 2. `{codex_home}/agents/` - User codex home directory
/// 3. `{cwd}/.codex/agents/` - Project local directory
fn build_search_paths(codex_home: &Path, cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. User config directory (~/.config/codex/agents/ on Linux/macOS)
    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("codex").join("agents"));
    }

    // 2. User codex home directory (passed via config.codex_home)
    paths.push(codex_home.join("agents"));

    // 3. Project local directory (passed via config.cwd)
    paths.push(cwd.join(".codex").join("agents"));

    paths
}

impl SessionScopedState {
    /// Initialize plugins and register plugin components (agents, hooks).
    ///
    /// This should be called after the stores are created to load
    /// plugin components from the plugin registry.
    ///
    /// # Arguments
    ///
    /// * `codex_home` - Path to the codex home directory (~/.codex)
    /// * `project_path` - Optional project path for project-scoped plugins
    pub async fn init_plugins(
        &self,
        codex_home: &std::path::Path,
        project_path: Option<&std::path::Path>,
    ) {
        // Initialize plugin service
        match codex_plugin::get_or_init_plugin_service(codex_home).await {
            Ok(service) => {
                // Load all enabled plugins
                if let Err(e) = service.load_all(project_path).await {
                    tracing::warn!("Failed to load plugins: {e}");
                    return;
                }

                // Register plugin agents
                let agents = service.get_agents().await;
                if !agents.is_empty() {
                    let count = self.registry.register_plugin_agents(agents).await;
                    tracing::info!("Initialized {count} plugin agents");
                }

                // Register plugin hooks
                let hooks = service.get_hooks().await;
                if !hooks.is_empty() {
                    let count = crate::hooks_ext::register_plugin_hooks(hooks);
                    tracing::info!("Initialized {count} plugin hooks");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to initialize plugin service: {e}");
            }
        }
    }

    pub fn new(codex_home: &Path, cwd: &Path) -> Self {
        let search_paths = build_search_paths(codex_home, cwd);
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
            plan_mode_approval_policy: Arc::new(RwLock::new(PlanModeApprovalPolicy::default())),
            codex_home: codex_home.to_path_buf(),
        }
    }

    /// Get the codex home directory.
    pub fn codex_home(&self) -> &Path {
        &self.codex_home
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
        conversation_id: ThreadId,
    ) -> Result<std::path::PathBuf, CodexErr> {
        let mut state = self
            .plan_mode
            .write()
            .map_err(|_| CodexErr::Fatal("plan_mode lock poisoned".to_string()))?;
        state.enter(&self.codex_home, conversation_id)
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

    // ========================================================================
    // Plan Mode Approval Policy helpers
    // ========================================================================

    /// Set the plan mode approval policy.
    ///
    /// Called during SpawnAgent initialization to configure silent approval.
    pub fn set_plan_mode_approval_policy(&self, policy: PlanModeApprovalPolicy) {
        if let Ok(mut p) = self.plan_mode_approval_policy.write() {
            *p = policy;
        }
    }

    /// Get the current plan mode approval policy.
    pub fn get_plan_mode_approval_policy(&self) -> PlanModeApprovalPolicy {
        self.plan_mode_approval_policy
            .read()
            .map(|p| *p)
            .unwrap_or_default()
    }

    /// Check if plan mode should be auto-approved (EnterPlanMode/ExitPlanMode).
    ///
    /// Returns true if `AutoApprove` policy is set.
    pub fn should_auto_approve_plan_mode(&self) -> bool {
        matches!(
            self.get_plan_mode_approval_policy(),
            PlanModeApprovalPolicy::AutoApprove
        )
    }
}

/// Global registry mapping conversation_id to session-scoped stores.
///
/// Using LazyLock + DashMap for thread-safe lazy initialization with
/// concurrent access support.
static SESSION_STATE_REGISTRY: LazyLock<DashMap<ThreadId, Arc<SessionScopedState>>> =
    LazyLock::new(DashMap::new);

/// Initialize stores for a conversation.
///
/// Should be called once when a session is created (in Codex::spawn).
/// Subsequent access should use `expect_session_state()` or `get_session_state()`.
///
/// # Arguments
///
/// * `conversation_id` - The conversation/thread ID
/// * `codex_home` - Path to the codex home directory (from config.codex_home)
/// * `cwd` - Current working directory (from config.cwd)
///
/// # Example
/// ```ignore
/// // In Codex::spawn after Session::new
/// init_session_state(thread_id, &config.codex_home, &config.cwd);
/// ```
pub fn init_session_state(
    conversation_id: ThreadId,
    codex_home: &Path,
    cwd: &Path,
) -> Arc<SessionScopedState> {
    SESSION_STATE_REGISTRY
        .entry(conversation_id)
        .or_insert_with(|| Arc::new(SessionScopedState::new(codex_home, cwd)))
        .clone()
}

/// Get stores, panicking if not initialized.
///
/// Use this when you're certain the stores have been initialized
/// (e.g., in tool handlers after session creation).
///
/// # Panics
///
/// Panics if stores haven't been initialized for this conversation.
pub fn expect_session_state(conversation_id: &ThreadId) -> Arc<SessionScopedState> {
    get_session_state(conversation_id)
        .expect("SessionScopedState should be initialized for this conversation")
}

/// Cleanup stores when session ends.
///
/// Should be called when a session is terminated to free memory.
/// Not calling this won't cause memory leaks for short-lived processes,
/// but long-running servers should call this on session cleanup.
///
/// Also cleans up plan slug cache to ensure new sessions get fresh slugs.
pub fn cleanup_session_state(conversation_id: &ThreadId) {
    SESSION_STATE_REGISTRY.remove(conversation_id);
    cleanup_plan_slug(conversation_id);
}

/// Get stores if they exist (without creating new ones).
///
/// Useful for operations that should only work on existing sessions.
pub fn get_session_state(conversation_id: &ThreadId) -> Option<Arc<SessionScopedState>> {
    SESSION_STATE_REGISTRY
        .get(conversation_id)
        .map(|r| r.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_search_paths() {
        let codex_home = PathBuf::from("/tmp/codex_home");
        let cwd = PathBuf::from("/tmp/project");
        let paths = build_search_paths(&codex_home, &cwd);

        // Should have at least 3 paths (config_dir may or may not exist)
        assert!(paths.len() >= 2);

        // All paths should end with "agents"
        for path in &paths {
            assert!(
                path.ends_with("agents"),
                "Path should end with 'agents': {path:?}"
            );
        }

        // Check codex_home path is included
        assert!(paths.contains(&codex_home.join("agents")));

        // Check project path is included
        assert!(paths.contains(&cwd.join(".codex").join("agents")));
    }

    #[test]
    fn test_init_and_expect_session_state() {
        let conv_id = ThreadId::new();
        let codex_home = PathBuf::from("/tmp/codex_home");
        let cwd = PathBuf::from("/tmp/project");

        // First access creates stores
        let stores1 = init_session_state(conv_id, &codex_home, &cwd);

        // expect_session_state returns same stores
        let stores2 = expect_session_state(&conv_id);

        // Both should point to same Arc
        assert!(Arc::ptr_eq(&stores1, &stores2));

        // Cleanup
        cleanup_session_state(&conv_id);

        // After cleanup, get_session_state returns None
        assert!(get_session_state(&conv_id).is_none());
    }

    #[test]
    fn test_different_sessions_have_different_stores() {
        let conv_id1 = ThreadId::new();
        let conv_id2 = ThreadId::new();
        let codex_home = PathBuf::from("/tmp/codex_home");
        let cwd = PathBuf::from("/tmp/project");

        let stores1 = init_session_state(conv_id1, &codex_home, &cwd);
        let stores2 = init_session_state(conv_id2, &codex_home, &cwd);

        // Different sessions should have different stores
        assert!(!Arc::ptr_eq(&stores1, &stores2));

        // Cleanup
        cleanup_session_state(&conv_id1);
        cleanup_session_state(&conv_id2);
    }
}
