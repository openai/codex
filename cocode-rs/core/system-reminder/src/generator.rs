//! Generator trait and context for system reminders.
//!
//! This module defines the [`AttachmentGenerator`] trait that all reminder
//! generators must implement, and the [`GeneratorContext`] that provides
//! the runtime state needed for generation.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::file_tracker::FileTracker;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::ReminderTier;
use crate::types::SystemReminder;

/// Trait for attachment generators.
///
/// Each generator is responsible for producing a specific type of system
/// reminder based on the current context. Generators are run in parallel
/// with timeout protection.
#[async_trait]
pub trait AttachmentGenerator: Send + Sync + Debug {
    /// Unique name for this generator.
    fn name(&self) -> &str;

    /// The type of attachment this generator produces.
    fn attachment_type(&self) -> AttachmentType;

    /// The tier this generator belongs to.
    fn tier(&self) -> ReminderTier {
        self.attachment_type().tier()
    }

    /// Generate the reminder content.
    ///
    /// Returns `Ok(Some(reminder))` if content was generated,
    /// `Ok(None)` if there's nothing to generate this turn,
    /// or `Err` if generation failed.
    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>>;

    /// Check if this generator is enabled in the config.
    fn is_enabled(&self, config: &SystemReminderConfig) -> bool;

    /// Get the throttle configuration for this generator.
    fn throttle_config(&self) -> ThrottleConfig {
        ThrottleConfig::default()
    }
}

/// Background task information.
#[derive(Debug, Clone)]
pub struct BackgroundTaskInfo {
    /// Unique task identifier.
    pub task_id: String,
    /// Type of background task.
    pub task_type: BackgroundTaskType,
    /// Command or description.
    pub command: String,
    /// Current status.
    pub status: BackgroundTaskStatus,
    /// Exit code if completed.
    pub exit_code: Option<i32>,
    /// Whether there's new output since last check.
    pub has_new_output: bool,
}

/// Type of background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundTaskType {
    /// Shell command running in background.
    Shell,
    /// Async agent task.
    AsyncAgent,
    /// Remote session.
    RemoteSession,
}

/// Status of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundTaskStatus {
    /// Task is still running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed.
    Failed,
}

/// Plan state information.
#[derive(Debug, Clone, Default)]
pub struct PlanState {
    /// Whether the plan is empty.
    pub is_empty: bool,
    /// Turn count when plan was last updated.
    pub last_update_turn: i32,
    /// Plan steps.
    pub steps: Vec<PlanStep>,
}

/// A step in the plan.
#[derive(Debug, Clone)]
pub struct PlanStep {
    /// Step description.
    pub step: String,
    /// Step status (pending, in_progress, completed).
    pub status: String,
}

/// Approved plan information (one-time injection after ExitPlanMode).
#[derive(Debug, Clone)]
pub struct ApprovedPlanInfo {
    /// The approved plan content.
    pub content: String,
    /// Turn when the plan was approved.
    pub approved_turn: i32,
}

/// Restored plan information (after compaction recovery).
#[derive(Debug, Clone)]
pub struct RestoredPlanInfo {
    /// The plan file content.
    pub content: String,
    /// Path to the plan file.
    pub file_path: PathBuf,
}

/// LSP diagnostic information.
#[derive(Debug, Clone)]
pub struct DiagnosticInfo {
    /// File path.
    pub file_path: PathBuf,
    /// Line number (1-based).
    pub line: i32,
    /// Column number (1-based).
    pub column: i32,
    /// Severity (error, warning, info, hint).
    pub severity: String,
    /// Diagnostic message.
    pub message: String,
    /// Diagnostic code.
    pub code: Option<String>,
}

/// Todo/task item information.
#[derive(Debug, Clone)]
pub struct TodoItem {
    /// Task ID.
    pub id: String,
    /// Task subject/title.
    pub subject: String,
    /// Task status.
    pub status: TodoStatus,
    /// Whether this task is blocked.
    pub is_blocked: bool,
}

/// Status of a todo item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoStatus {
    /// Task is pending.
    Pending,
    /// Task is in progress.
    InProgress,
    /// Task is completed.
    Completed,
}

impl std::fmt::Display for TodoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TodoStatus::Pending => write!(f, "pending"),
            TodoStatus::InProgress => write!(f, "in_progress"),
            TodoStatus::Completed => write!(f, "completed"),
        }
    }
}

/// Information about a delegated agent.
#[derive(Debug, Clone)]
pub struct DelegatedAgentInfo {
    /// Agent identifier.
    pub agent_id: String,
    /// Agent type (e.g., "Explore", "Plan").
    pub agent_type: String,
    /// Current status.
    pub status: String,
    /// Brief description of what the agent is doing.
    pub description: String,
}

/// Token usage statistics.
#[derive(Debug, Clone, Default)]
pub struct TokenUsageStats {
    /// Input tokens consumed.
    pub input_tokens: i64,
    /// Output tokens generated.
    pub output_tokens: i64,
    /// Cache read tokens (if applicable).
    pub cache_read_tokens: i64,
    /// Cache write tokens (if applicable).
    pub cache_write_tokens: i64,
    /// Total tokens used in session.
    pub total_session_tokens: i64,
    /// Context window capacity.
    pub context_capacity: i64,
    /// Percentage of context used.
    pub context_usage_percent: f64,
}

/// Budget information.
#[derive(Debug, Clone)]
pub struct BudgetInfo {
    /// Total budget in USD.
    pub total_usd: f64,
    /// Used budget in USD.
    pub used_usd: f64,
    /// Remaining budget in USD.
    pub remaining_usd: f64,
    /// Whether budget is low (< 10% remaining).
    pub is_low: bool,
}

/// Collaboration notification from another agent.
#[derive(Debug, Clone)]
pub struct CollabNotification {
    /// Source agent identifier.
    pub from_agent: String,
    /// Notification type (e.g., "completed", "needs_input", "error").
    pub notification_type: String,
    /// Notification message.
    pub message: String,
    /// Turn when notification was received.
    pub received_turn: i32,
}

/// Information about a queued command (real-time steering).
///
/// Queued commands are entered by the user via Enter during streaming.
/// They serve dual purpose:
/// 1. Injected as `<system-reminder>User sent: {prompt}</system-reminder>` for real-time steering
/// 2. Executed as new user turns after the current turn completes
#[derive(Debug, Clone)]
pub struct QueuedCommandInfo {
    /// Unique identifier for this command.
    pub id: String,
    /// The user's prompt/message.
    pub prompt: String,
    /// When the command was queued (Unix millis).
    pub queued_at: i64,
}

/// Context passed to generators during execution.
///
/// This provides all the runtime state needed for generators to
/// determine what content to produce.
#[derive(Debug)]
pub struct GeneratorContext<'a> {
    /// Current configuration.
    pub config: &'a SystemReminderConfig,

    // === Turn tracking ===
    /// Current turn number.
    pub turn_number: i32,
    /// Whether this is the main agent (not a subagent).
    pub is_main_agent: bool,
    /// Whether there's user input this turn.
    pub has_user_input: bool,
    /// Context window size in tokens.
    /// Used for token-aware decisions in generators.
    pub context_window: i32,

    // === User input ===
    /// The user's prompt text (if any).
    pub user_prompt: Option<&'a str>,
    /// Files mentioned via @file syntax.
    pub user_mentioned_files: Vec<PathBuf>,
    /// Agents mentioned via @agent-type syntax.
    pub user_mentioned_agents: Vec<String>,

    // === File state ===
    /// File tracker for change detection.
    pub file_tracker: Option<&'a FileTracker>,
    /// Current working directory.
    pub cwd: PathBuf,
    /// Plan file path (if in plan mode).
    pub plan_file_path: Option<PathBuf>,

    // === Plan state ===
    /// Whether plan mode is active.
    pub is_plan_mode: bool,
    /// Whether this is a re-entry into plan mode.
    pub is_plan_reentry: bool,
    /// Current plan state.
    pub plan_state: Option<PlanState>,
    /// Approved plan (one-time, after ExitPlanMode).
    pub approved_plan: Option<ApprovedPlanInfo>,
    /// Restored plan (after compaction).
    pub restored_plan: Option<RestoredPlanInfo>,

    // === Background tasks ===
    /// Currently running background tasks.
    pub background_tasks: Vec<BackgroundTaskInfo>,

    // === Diagnostics ===
    /// LSP diagnostics.
    pub diagnostics: Vec<DiagnosticInfo>,

    // === Todo/Tasks ===
    /// Current todo items.
    pub todos: Vec<TodoItem>,

    // === Nested memory ===
    /// Paths that trigger nested memory lookup.
    pub nested_memory_triggers: HashSet<PathBuf>,

    // === Extension data ===
    /// Additional data that generators can use.
    pub extension_data: HashMap<String, Arc<dyn std::any::Any + Send + Sync>>,

    // === Delegate mode state ===
    /// Whether delegate mode is active.
    pub is_delegate_mode: bool,
    /// Whether exiting delegate mode this turn.
    pub delegate_mode_exiting: bool,
    /// Information about delegated agents.
    pub delegated_agents: Vec<DelegatedAgentInfo>,

    // === Token/budget tracking ===
    /// Token usage statistics.
    pub token_usage: Option<TokenUsageStats>,
    /// Budget information.
    pub budget: Option<BudgetInfo>,

    // === Collaboration notifications ===
    /// Pending collaboration notifications from other agents.
    pub collab_notifications: Vec<CollabNotification>,

    // === Real-time steering ===
    /// Queued commands from user (Enter during streaming).
    /// These are injected as "User sent: {message}" to steer the model.
    pub queued_commands: Vec<QueuedCommandInfo>,

    // === Global state flags ===
    /// Whether plan mode exit is pending (triggers one-time exit instructions).
    pub plan_mode_exit_pending: bool,
}

impl<'a> GeneratorContext<'a> {
    /// Create a builder for constructing generator context.
    pub fn builder() -> GeneratorContextBuilder<'a> {
        GeneratorContextBuilder::default()
    }

    /// Check if plan mode is active.
    pub fn in_plan_mode(&self) -> bool {
        self.is_plan_mode
    }

    /// Check if there are any background tasks.
    pub fn has_background_tasks(&self) -> bool {
        !self.background_tasks.is_empty()
    }

    /// Check if there are any diagnostics.
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Check if there are any todos.
    pub fn has_todos(&self) -> bool {
        !self.todos.is_empty()
    }

    /// Get pending todos.
    pub fn pending_todos(&self) -> impl Iterator<Item = &TodoItem> {
        self.todos
            .iter()
            .filter(|t| t.status == TodoStatus::Pending)
    }

    /// Get in-progress todos.
    pub fn in_progress_todos(&self) -> impl Iterator<Item = &TodoItem> {
        self.todos
            .iter()
            .filter(|t| t.status == TodoStatus::InProgress)
    }

    /// Check if delegate mode is active.
    pub fn in_delegate_mode(&self) -> bool {
        self.is_delegate_mode
    }

    /// Check if there are pending collaboration notifications.
    pub fn has_collab_notifications(&self) -> bool {
        !self.collab_notifications.is_empty()
    }

    /// Check if context usage is high (> 80%).
    pub fn is_context_usage_high(&self) -> bool {
        self.token_usage
            .as_ref()
            .map(|t| t.context_usage_percent > 80.0)
            .unwrap_or(false)
    }

    /// Check if budget is low (< 10% remaining).
    pub fn is_budget_low(&self) -> bool {
        self.budget.as_ref().map(|b| b.is_low).unwrap_or(false)
    }

    /// Check if full reminders should be used this turn.
    ///
    /// Full reminders are used on turn 1 and every 5th turn thereafter
    /// (i.e., turns 1, 6, 11, 16, ...). This follows Claude Code's steering
    /// pattern to reduce token usage while maintaining model guidance.
    pub fn should_use_full_reminders(&self) -> bool {
        self.turn_number == 1 || self.turn_number % 5 == 1
    }

    /// Check if sparse reminders should be used this turn.
    ///
    /// Sparse reminders are brief summaries used on turns where full
    /// reminders are not needed. This is the inverse of `should_use_full_reminders()`.
    pub fn should_use_sparse_reminders(&self) -> bool {
        !self.should_use_full_reminders()
    }
}

/// Builder for [`GeneratorContext`].
#[derive(Default)]
pub struct GeneratorContextBuilder<'a> {
    config: Option<&'a SystemReminderConfig>,
    turn_number: i32,
    is_main_agent: bool,
    has_user_input: bool,
    context_window: i32,
    user_prompt: Option<&'a str>,
    user_mentioned_files: Vec<PathBuf>,
    user_mentioned_agents: Vec<String>,
    file_tracker: Option<&'a FileTracker>,
    cwd: Option<PathBuf>,
    plan_file_path: Option<PathBuf>,
    is_plan_mode: bool,
    is_plan_reentry: bool,
    plan_state: Option<PlanState>,
    approved_plan: Option<ApprovedPlanInfo>,
    restored_plan: Option<RestoredPlanInfo>,
    background_tasks: Vec<BackgroundTaskInfo>,
    diagnostics: Vec<DiagnosticInfo>,
    todos: Vec<TodoItem>,
    nested_memory_triggers: HashSet<PathBuf>,
    extension_data: HashMap<String, Arc<dyn std::any::Any + Send + Sync>>,
    // New fields
    is_delegate_mode: bool,
    delegate_mode_exiting: bool,
    delegated_agents: Vec<DelegatedAgentInfo>,
    token_usage: Option<TokenUsageStats>,
    budget: Option<BudgetInfo>,
    collab_notifications: Vec<CollabNotification>,
    queued_commands: Vec<QueuedCommandInfo>,
    plan_mode_exit_pending: bool,
}

impl<'a> GeneratorContextBuilder<'a> {
    pub fn config(mut self, config: &'a SystemReminderConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn turn_number(mut self, turn: i32) -> Self {
        self.turn_number = turn;
        self
    }

    pub fn is_main_agent(mut self, is_main: bool) -> Self {
        self.is_main_agent = is_main;
        self
    }

    pub fn has_user_input(mut self, has_input: bool) -> Self {
        self.has_user_input = has_input;
        self
    }

    pub fn context_window(mut self, tokens: i32) -> Self {
        self.context_window = tokens;
        self
    }

    pub fn user_prompt(mut self, prompt: &'a str) -> Self {
        self.user_prompt = Some(prompt);
        self
    }

    pub fn user_mentioned_files(mut self, files: Vec<PathBuf>) -> Self {
        self.user_mentioned_files = files;
        self
    }

    pub fn user_mentioned_agents(mut self, agents: Vec<String>) -> Self {
        self.user_mentioned_agents = agents;
        self
    }

    pub fn file_tracker(mut self, tracker: &'a FileTracker) -> Self {
        self.file_tracker = Some(tracker);
        self
    }

    pub fn cwd(mut self, cwd: PathBuf) -> Self {
        self.cwd = Some(cwd);
        self
    }

    pub fn plan_file_path(mut self, path: PathBuf) -> Self {
        self.plan_file_path = Some(path);
        self
    }

    pub fn is_plan_mode(mut self, is_plan: bool) -> Self {
        self.is_plan_mode = is_plan;
        self
    }

    pub fn is_plan_reentry(mut self, is_reentry: bool) -> Self {
        self.is_plan_reentry = is_reentry;
        self
    }

    pub fn plan_state(mut self, state: PlanState) -> Self {
        self.plan_state = Some(state);
        self
    }

    pub fn approved_plan(mut self, plan: ApprovedPlanInfo) -> Self {
        self.approved_plan = Some(plan);
        self
    }

    pub fn restored_plan(mut self, plan: RestoredPlanInfo) -> Self {
        self.restored_plan = Some(plan);
        self
    }

    pub fn background_tasks(mut self, tasks: Vec<BackgroundTaskInfo>) -> Self {
        self.background_tasks = tasks;
        self
    }

    pub fn diagnostics(mut self, diags: Vec<DiagnosticInfo>) -> Self {
        self.diagnostics = diags;
        self
    }

    pub fn todos(mut self, todos: Vec<TodoItem>) -> Self {
        self.todos = todos;
        self
    }

    pub fn nested_memory_triggers(mut self, triggers: HashSet<PathBuf>) -> Self {
        self.nested_memory_triggers = triggers;
        self
    }

    pub fn extension<T: Send + Sync + 'static>(mut self, key: &str, value: T) -> Self {
        self.extension_data.insert(key.to_string(), Arc::new(value));
        self
    }

    pub fn is_delegate_mode(mut self, is_delegate: bool) -> Self {
        self.is_delegate_mode = is_delegate;
        self
    }

    pub fn delegate_mode_exiting(mut self, exiting: bool) -> Self {
        self.delegate_mode_exiting = exiting;
        self
    }

    pub fn delegated_agents(mut self, agents: Vec<DelegatedAgentInfo>) -> Self {
        self.delegated_agents = agents;
        self
    }

    pub fn token_usage(mut self, usage: TokenUsageStats) -> Self {
        self.token_usage = Some(usage);
        self
    }

    pub fn budget(mut self, budget: BudgetInfo) -> Self {
        self.budget = Some(budget);
        self
    }

    pub fn collab_notifications(mut self, notifications: Vec<CollabNotification>) -> Self {
        self.collab_notifications = notifications;
        self
    }

    pub fn queued_commands(mut self, commands: Vec<QueuedCommandInfo>) -> Self {
        self.queued_commands = commands;
        self
    }

    pub fn plan_mode_exit_pending(mut self, pending: bool) -> Self {
        self.plan_mode_exit_pending = pending;
        self
    }

    /// Build the generator context.
    ///
    /// # Panics
    ///
    /// Panics if `config` or `cwd` is not set.
    pub fn build(self) -> GeneratorContext<'a> {
        GeneratorContext {
            config: self.config.expect("config is required"),
            turn_number: self.turn_number,
            is_main_agent: self.is_main_agent,
            has_user_input: self.has_user_input,
            context_window: self.context_window,
            user_prompt: self.user_prompt,
            user_mentioned_files: self.user_mentioned_files,
            user_mentioned_agents: self.user_mentioned_agents,
            file_tracker: self.file_tracker,
            cwd: self.cwd.expect("cwd is required"),
            plan_file_path: self.plan_file_path,
            is_plan_mode: self.is_plan_mode,
            is_plan_reentry: self.is_plan_reentry,
            plan_state: self.plan_state,
            approved_plan: self.approved_plan,
            restored_plan: self.restored_plan,
            background_tasks: self.background_tasks,
            diagnostics: self.diagnostics,
            todos: self.todos,
            nested_memory_triggers: self.nested_memory_triggers,
            extension_data: self.extension_data,
            // New fields
            is_delegate_mode: self.is_delegate_mode,
            delegate_mode_exiting: self.delegate_mode_exiting,
            delegated_agents: self.delegated_agents,
            token_usage: self.token_usage,
            budget: self.budget,
            collab_notifications: self.collab_notifications,
            queued_commands: self.queued_commands,
            plan_mode_exit_pending: self.plan_mode_exit_pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[test]
    fn test_context_builder() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(5)
            .is_main_agent(true)
            .has_user_input(true)
            .cwd(PathBuf::from("/tmp/test"))
            .build();

        assert_eq!(ctx.turn_number, 5);
        assert!(ctx.is_main_agent);
        assert!(ctx.has_user_input);
        assert!(!ctx.in_plan_mode());
        assert!(!ctx.has_background_tasks());
    }

    #[test]
    fn test_context_plan_mode() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .is_plan_mode(true)
            .plan_file_path(PathBuf::from("/tmp/plan.md"))
            .build();

        assert!(ctx.in_plan_mode());
        assert_eq!(ctx.plan_file_path, Some(PathBuf::from("/tmp/plan.md")));
    }

    #[test]
    fn test_todo_filtering() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .todos(vec![
                TodoItem {
                    id: "1".to_string(),
                    subject: "Task 1".to_string(),
                    status: TodoStatus::Pending,
                    is_blocked: false,
                },
                TodoItem {
                    id: "2".to_string(),
                    subject: "Task 2".to_string(),
                    status: TodoStatus::InProgress,
                    is_blocked: false,
                },
                TodoItem {
                    id: "3".to_string(),
                    subject: "Task 3".to_string(),
                    status: TodoStatus::Completed,
                    is_blocked: false,
                },
            ])
            .build();

        assert!(ctx.has_todos());
        assert_eq!(ctx.pending_todos().count(), 1);
        assert_eq!(ctx.in_progress_todos().count(), 1);
    }

    #[test]
    fn test_background_task_info() {
        let task = BackgroundTaskInfo {
            task_id: "task-1".to_string(),
            task_type: BackgroundTaskType::Shell,
            command: "npm test".to_string(),
            status: BackgroundTaskStatus::Running,
            exit_code: None,
            has_new_output: true,
        };

        assert_eq!(task.task_type, BackgroundTaskType::Shell);
        assert_eq!(task.status, BackgroundTaskStatus::Running);
        assert!(task.has_new_output);
    }

    #[test]
    fn test_should_use_full_reminders() {
        let config = test_config();

        // Turn 1 - should be full
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .build();
        assert!(ctx.should_use_full_reminders());
        assert!(!ctx.should_use_sparse_reminders());

        // Turn 2, 3, 4, 5 - should be sparse
        for turn in [2, 3, 4, 5] {
            let ctx = GeneratorContext::builder()
                .config(&config)
                .turn_number(turn)
                .cwd(PathBuf::from("/tmp"))
                .build();
            assert!(
                !ctx.should_use_full_reminders(),
                "Turn {turn} should be sparse"
            );
            assert!(
                ctx.should_use_sparse_reminders(),
                "Turn {turn} should be sparse"
            );
        }

        // Turn 6 (5+1) - should be full
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(6)
            .cwd(PathBuf::from("/tmp"))
            .build();
        assert!(ctx.should_use_full_reminders());
        assert!(!ctx.should_use_sparse_reminders());

        // Turn 11 (10+1) - should be full
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(11)
            .cwd(PathBuf::from("/tmp"))
            .build();
        assert!(ctx.should_use_full_reminders());

        // Turn 16 (15+1) - should be full
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(16)
            .cwd(PathBuf::from("/tmp"))
            .build();
        assert!(ctx.should_use_full_reminders());
    }
}
