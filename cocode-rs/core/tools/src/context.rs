//! Tool execution context.
//!
//! This module provides [`ToolContext`] which contains all the context
//! needed for tool execution, including permissions, event channels,
//! and cancellation support.

use crate::permission_rules::PermissionRuleEvaluator;
use async_trait::async_trait;
use cocode_hooks::HookRegistry;
use cocode_lsp::LspServerManager;
use cocode_protocol::ApprovalRequest;
use cocode_protocol::LoopEvent;
use cocode_protocol::PermissionMode;
use cocode_protocol::RoleSelections;
use cocode_shell::BackgroundTaskRegistry;
use cocode_skill::SkillManager;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use std::time::SystemTime;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Input for spawning a subagent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentInput {
    /// The agent type to spawn.
    pub agent_type: String,
    /// The task prompt for the agent.
    pub prompt: String,
    /// Optional model override.
    pub model: Option<String>,
    /// Optional turn limit override.
    pub max_turns: Option<i32>,
    /// Whether to run in background.
    pub run_in_background: bool,
    /// Optional tool filter override.
    pub allowed_tools: Option<Vec<String>>,
    /// Parent's role selections (snapshot at spawn time for isolation).
    ///
    /// When present, the spawned subagent will use these selections,
    /// ensuring it's unaffected by subsequent changes to the parent's settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_selections: Option<RoleSelections>,
    /// Permission mode override for the subagent.
    ///
    /// When set (from `AgentDefinition.permission_mode`), the subagent uses
    /// this mode instead of inheriting from the parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<cocode_protocol::PermissionMode>,
}

/// Result of spawning a subagent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAgentResult {
    /// The unique agent ID.
    pub agent_id: String,
    /// The agent output (foreground only).
    pub output: Option<String>,
    /// Background agent output file path.
    pub output_file: Option<PathBuf>,
}

/// Type alias for the agent spawn callback function.
///
/// This callback is provided by the executor layer to enable tools
/// to spawn subagents without creating circular dependencies.
pub type SpawnAgentFn = Arc<
    dyn Fn(
            SpawnAgentInput,
        )
            -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<SpawnAgentResult>> + Send>>
        + Send
        + Sync,
>;

/// Trait for requesting user permission approval.
///
/// This trait decouples the tools crate from the executor crate,
/// allowing `WorkerPermissionQueue` (in cocode-executor) to be used
/// without creating a circular dependency.
#[async_trait]
pub trait PermissionRequester: Send + Sync {
    /// Request permission for an operation.
    ///
    /// Returns `true` if approved, `false` if denied or timed out.
    async fn request_permission(&self, request: ApprovalRequest, worker_id: &str) -> bool;
}

/// Information about an invoked skill.
///
/// Tracks skills that have been invoked during the session for hook cleanup.
#[derive(Debug, Clone)]
pub struct InvokedSkill {
    /// The skill name.
    pub name: String,
    /// When the skill was invoked.
    pub started_at: Instant,
}

/// Stored approvals for tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApprovalStore {
    /// Approved tool patterns.
    approved_patterns: HashSet<String>,
    /// Session-wide approvals.
    session_approvals: HashSet<String>,
}

impl ApprovalStore {
    /// Create a new empty approval store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a tool action is approved.
    pub fn is_approved(&self, tool_name: &str, pattern: &str) -> bool {
        let key = format!("{tool_name}:{pattern}");
        self.approved_patterns.contains(&key) || self.session_approvals.contains(tool_name)
    }

    /// Add an approval for a specific pattern.
    pub fn approve_pattern(&mut self, tool_name: &str, pattern: &str) {
        let key = format!("{tool_name}:{pattern}");
        self.approved_patterns.insert(key);
    }

    /// Add a session-wide approval for a tool.
    pub fn approve_session(&mut self, tool_name: &str) {
        self.session_approvals.insert(tool_name.to_string());
    }

    /// Clear all approvals.
    pub fn clear(&mut self) {
        self.approved_patterns.clear();
        self.session_approvals.clear();
    }
}

/// State of a file that has been read.
///
/// Tracks content, timestamps, and access patterns for read-before-edit validation.
#[derive(Debug, Clone)]
pub struct FileReadState {
    /// File content at time of read (None if partial or too large).
    pub content: Option<String>,
    /// When this read state was recorded.
    pub timestamp: SystemTime,
    /// File modification time at time of read.
    pub file_mtime: Option<SystemTime>,
    /// Line offset of the read (None if from start).
    pub offset: Option<i32>,
    /// Line limit of the read (None if no limit).
    pub limit: Option<i32>,
    /// Whether the entire file was read.
    pub is_complete_read: bool,
    /// Number of times this file has been accessed.
    pub access_count: i32,
}

impl FileReadState {
    /// Create a new read state for a complete file read.
    pub fn complete(content: String, file_mtime: Option<SystemTime>) -> Self {
        Self {
            content: Some(content),
            timestamp: SystemTime::now(),
            file_mtime,
            offset: None,
            limit: None,
            is_complete_read: true,
            access_count: 1,
        }
    }

    /// Create a new read state for a partial file read.
    pub fn partial(offset: i32, limit: i32, file_mtime: Option<SystemTime>) -> Self {
        Self {
            content: None,
            timestamp: SystemTime::now(),
            file_mtime,
            offset: Some(offset),
            limit: Some(limit),
            is_complete_read: false,
            access_count: 1,
        }
    }
}

/// Tracks files that have been read or modified.
#[derive(Debug, Clone, Default)]
pub struct FileTracker {
    /// Files that have been read, with their read state.
    read_files: HashMap<PathBuf, FileReadState>,
    /// Files that have been modified.
    modified_files: HashSet<PathBuf>,
}

impl FileTracker {
    /// Create a new file tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all read files with their state for syncing to another tracker.
    ///
    /// This is used to sync file read state to the system-reminder's FileTracker
    /// for change detection.
    pub fn read_files_with_state(&self) -> Vec<(PathBuf, &FileReadState)> {
        self.read_files
            .iter()
            .map(|(k, v)| (k.clone(), v))
            .collect()
    }

    /// Record a file read (simple — backward-compatible).
    pub fn record_read(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        if let Some(state) = self.read_files.get_mut(&path) {
            state.access_count += 1;
            state.timestamp = SystemTime::now();
        } else {
            self.read_files.insert(
                path,
                FileReadState {
                    content: None,
                    timestamp: SystemTime::now(),
                    file_mtime: None,
                    offset: None,
                    limit: None,
                    is_complete_read: false,
                    access_count: 1,
                },
            );
        }
    }

    /// Record a file read with full state.
    pub fn record_read_with_state(&mut self, path: impl Into<PathBuf>, state: FileReadState) {
        self.read_files.insert(path.into(), state);
    }

    /// Record a file modification.
    pub fn record_modified(&mut self, path: impl Into<PathBuf>) {
        self.modified_files.insert(path.into());
    }

    /// Check if a file has been read.
    pub fn was_read(&self, path: &PathBuf) -> bool {
        self.read_files.contains_key(path)
    }

    /// Get the read state for a file.
    pub fn read_state(&self, path: &PathBuf) -> Option<&FileReadState> {
        self.read_files.get(path)
    }

    /// Check if a file has been modified.
    pub fn was_modified(&self, path: &PathBuf) -> bool {
        self.modified_files.contains(path)
    }

    /// Get all read file paths.
    pub fn read_files(&self) -> Vec<&PathBuf> {
        self.read_files.keys().collect()
    }

    /// Get all modified files.
    pub fn modified_files(&self) -> &HashSet<PathBuf> {
        &self.modified_files
    }
}

/// Context for tool execution.
///
/// This provides everything a tool needs during execution:
/// - Call identification (call_id, turn_id, session_id, agent_id)
/// - Working directory and additional directories
/// - Permission mode and approvals
/// - Event channel for progress updates
/// - Cancellation support
/// - File tracking with content/timestamp validation
/// - Subagent spawning capability
/// - Plan mode state for Write/Edit permission checks
/// - Background task registry for Bash background execution
/// - LSP server manager for language intelligence
/// - Session directory for persisting large tool results
#[derive(Clone)]
pub struct ToolContext {
    /// Unique call ID for this execution.
    pub call_id: String,
    /// Session ID.
    pub session_id: String,
    /// Turn ID for the current conversation turn.
    pub turn_id: String,
    /// Agent ID (set when running inside a sub-agent).
    pub agent_id: Option<String>,
    /// Current working directory.
    pub cwd: PathBuf,
    /// Additional working directories (e.g., for multi-root workspaces).
    pub additional_working_directories: Vec<PathBuf>,
    /// Permission mode for this execution.
    pub permission_mode: PermissionMode,
    /// Channel for emitting loop events.
    pub event_tx: Option<mpsc::Sender<LoopEvent>>,
    /// Cancellation token for aborting execution.
    pub cancel_token: CancellationToken,
    /// Stored approvals.
    pub approval_store: Arc<Mutex<ApprovalStore>>,
    /// File tracker.
    pub file_tracker: Arc<Mutex<FileTracker>>,
    /// Optional callback for spawning subagents.
    pub spawn_agent_fn: Option<SpawnAgentFn>,
    /// Whether plan mode is currently active.
    pub is_plan_mode: bool,
    /// Path to the current plan file (if in plan mode).
    pub plan_file_path: Option<PathBuf>,
    /// Background task registry for managing background shell commands.
    pub background_registry: BackgroundTaskRegistry,
    /// Optional LSP server manager for language intelligence tools.
    pub lsp_manager: Option<Arc<LspServerManager>>,
    /// Optional skill manager for executing named skills.
    pub skill_manager: Option<Arc<SkillManager>>,
    /// Optional hook registry for skill hook integration.
    pub hook_registry: Option<Arc<HookRegistry>>,
    /// Skills that have been invoked (for hook cleanup).
    pub invoked_skills: Arc<Mutex<Vec<InvokedSkill>>>,
    /// Session directory for storing tool results.
    ///
    /// Large tool results (>400K chars by default) are persisted here with only
    /// a preview kept in context. Typical path: `~/.cocode/sessions/{session_id}/`
    pub session_dir: Option<PathBuf>,
    /// Parent's role selections (snapshot for subagent isolation).
    ///
    /// When set, spawned subagents will inherit these selections,
    /// ensuring they're unaffected by subsequent changes to the parent's settings.
    pub parent_selections: Option<RoleSelections>,
    /// Optional permission requester for interactive approval flow.
    ///
    /// When set, the executor can route `NeedsApproval` results to the
    /// UI/TUI for user confirmation instead of denying immediately.
    pub permission_requester: Option<Arc<dyn PermissionRequester>>,
    /// Optional permission rule evaluator for pre-configured rules.
    ///
    /// When set, rules are evaluated before the tool's own `check_permission()`
    /// to allow, deny, or delegate based on project/user/policy configuration.
    pub permission_evaluator: Option<PermissionRuleEvaluator>,
}

impl ToolContext {
    /// Create a new tool context.
    pub fn new(call_id: impl Into<String>, session_id: impl Into<String>, cwd: PathBuf) -> Self {
        Self {
            call_id: call_id.into(),
            session_id: session_id.into(),
            turn_id: String::new(),
            agent_id: None,
            cwd,
            additional_working_directories: Vec::new(),
            permission_mode: PermissionMode::Default,
            event_tx: None,
            cancel_token: CancellationToken::new(),
            approval_store: Arc::new(Mutex::new(ApprovalStore::new())),
            file_tracker: Arc::new(Mutex::new(FileTracker::new())),
            spawn_agent_fn: None,
            is_plan_mode: false,
            plan_file_path: None,
            background_registry: BackgroundTaskRegistry::new(),
            lsp_manager: None,
            skill_manager: None,
            hook_registry: None,
            invoked_skills: Arc::new(Mutex::new(Vec::new())),
            session_dir: None,
            parent_selections: None,
            permission_requester: None,
            permission_evaluator: None,
        }
    }

    /// Set the permission mode.
    pub fn with_permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_mode = mode;
        self
    }

    /// Set the event channel.
    pub fn with_event_tx(mut self, tx: mpsc::Sender<LoopEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Set the cancellation token.
    pub fn with_cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = token;
        self
    }

    /// Set the approval store.
    pub fn with_approval_store(mut self, store: Arc<Mutex<ApprovalStore>>) -> Self {
        self.approval_store = store;
        self
    }

    /// Set the file tracker.
    pub fn with_file_tracker(mut self, tracker: Arc<Mutex<FileTracker>>) -> Self {
        self.file_tracker = tracker;
        self
    }

    /// Set the turn ID.
    pub fn with_turn_id(mut self, turn_id: impl Into<String>) -> Self {
        self.turn_id = turn_id.into();
        self
    }

    /// Set the agent ID.
    pub fn with_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Set additional working directories.
    pub fn with_additional_working_directories(mut self, dirs: Vec<PathBuf>) -> Self {
        self.additional_working_directories = dirs;
        self
    }

    /// Set the spawn agent callback.
    pub fn with_spawn_agent_fn(mut self, f: SpawnAgentFn) -> Self {
        self.spawn_agent_fn = Some(f);
        self
    }

    /// Set plan mode state.
    pub fn with_plan_mode(mut self, is_active: bool, plan_file_path: Option<PathBuf>) -> Self {
        self.is_plan_mode = is_active;
        self.plan_file_path = plan_file_path;
        self
    }

    /// Set the background task registry.
    pub fn with_background_registry(mut self, registry: BackgroundTaskRegistry) -> Self {
        self.background_registry = registry;
        self
    }

    /// Set the LSP server manager.
    pub fn with_lsp_manager(mut self, manager: Arc<LspServerManager>) -> Self {
        self.lsp_manager = Some(manager);
        self
    }

    /// Set the skill manager.
    pub fn with_skill_manager(mut self, manager: Arc<SkillManager>) -> Self {
        self.skill_manager = Some(manager);
        self
    }

    /// Set the hook registry.
    pub fn with_hook_registry(mut self, registry: Arc<HookRegistry>) -> Self {
        self.hook_registry = Some(registry);
        self
    }

    /// Set the session directory for persisting large tool results.
    pub fn with_session_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.session_dir = Some(dir.into());
        self
    }

    /// Set the permission requester for interactive approval flow.
    pub fn with_permission_requester(mut self, requester: Arc<dyn PermissionRequester>) -> Self {
        self.permission_requester = Some(requester);
        self
    }

    /// Set the permission rule evaluator.
    pub fn with_permission_evaluator(mut self, evaluator: PermissionRuleEvaluator) -> Self {
        self.permission_evaluator = Some(evaluator);
        self
    }

    /// Spawn a subagent using the configured callback.
    ///
    /// Returns an error if no spawn callback is configured.
    pub async fn spawn_agent(&self, input: SpawnAgentInput) -> anyhow::Result<SpawnAgentResult> {
        let spawn_fn = self
            .spawn_agent_fn
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No spawn_agent_fn configured"))?;
        spawn_fn(input).await
    }

    /// Check if agent spawning is available.
    pub fn can_spawn_agent(&self) -> bool {
        self.spawn_agent_fn.is_some()
    }

    /// Emit a loop event.
    pub async fn emit_event(&self, event: LoopEvent) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(event).await;
        }
    }

    /// Emit tool progress.
    pub async fn emit_progress(&self, message: impl Into<String>) {
        self.emit_event(LoopEvent::ToolProgress {
            call_id: self.call_id.clone(),
            progress: cocode_protocol::ToolProgressInfo {
                message: Some(message.into()),
                percentage: None,
                bytes_processed: None,
                total_bytes: None,
            },
        })
        .await;
    }

    /// Emit tool progress with percentage.
    pub async fn emit_progress_percent(&self, message: impl Into<String>, percentage: i32) {
        self.emit_event(LoopEvent::ToolProgress {
            call_id: self.call_id.clone(),
            progress: cocode_protocol::ToolProgressInfo {
                message: Some(message.into()),
                percentage: Some(percentage),
                bytes_processed: None,
                total_bytes: None,
            },
        })
        .await;
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// Wait for cancellation or completion.
    pub async fn cancelled(&self) {
        self.cancel_token.cancelled().await
    }

    /// Record a file read (simple — backward-compatible).
    pub async fn record_file_read(&self, path: impl Into<PathBuf>) {
        self.file_tracker.lock().await.record_read(path);
    }

    /// Record a file read with full state tracking.
    pub async fn record_file_read_with_state(
        &self,
        path: impl Into<PathBuf>,
        state: FileReadState,
    ) {
        self.file_tracker
            .lock()
            .await
            .record_read_with_state(path, state);
    }

    /// Record a file modification.
    pub async fn record_file_modified(&self, path: impl Into<PathBuf>) {
        self.file_tracker.lock().await.record_modified(path);
    }

    /// Check if a file was read.
    pub async fn was_file_read(&self, path: &PathBuf) -> bool {
        self.file_tracker.lock().await.was_read(path)
    }

    /// Get the read state for a file.
    pub async fn file_read_state(&self, path: &PathBuf) -> Option<FileReadState> {
        self.file_tracker.lock().await.read_state(path).cloned()
    }

    /// Check if a file was modified.
    pub async fn was_file_modified(&self, path: &PathBuf) -> bool {
        self.file_tracker.lock().await.was_modified(path)
    }

    /// Check if an action is approved.
    pub async fn is_approved(&self, tool_name: &str, pattern: &str) -> bool {
        self.approval_store
            .lock()
            .await
            .is_approved(tool_name, pattern)
    }

    /// Approve a specific pattern.
    pub async fn approve_pattern(&self, tool_name: &str, pattern: &str) {
        self.approval_store
            .lock()
            .await
            .approve_pattern(tool_name, pattern);
    }

    /// Approve a tool for the session.
    pub async fn approve_session(&self, tool_name: &str) {
        self.approval_store.lock().await.approve_session(tool_name);
    }

    /// Resolve a path relative to the working directory.
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        }
    }
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("call_id", &self.call_id)
            .field("session_id", &self.session_id)
            .field("turn_id", &self.turn_id)
            .field("agent_id", &self.agent_id)
            .field("cwd", &self.cwd)
            .field("permission_mode", &self.permission_mode)
            .field("is_cancelled", &self.is_cancelled())
            .field("is_plan_mode", &self.is_plan_mode)
            .field("plan_file_path", &self.plan_file_path)
            .field("lsp_manager", &self.lsp_manager.is_some())
            .field("skill_manager", &self.skill_manager.is_some())
            .field("session_dir", &self.session_dir)
            .field("permission_requester", &self.permission_requester.is_some())
            .field("permission_evaluator", &self.permission_evaluator.is_some())
            .finish_non_exhaustive()
    }
}

/// Builder for creating tool contexts.
pub struct ToolContextBuilder {
    call_id: String,
    session_id: String,
    turn_id: String,
    agent_id: Option<String>,
    cwd: PathBuf,
    additional_working_directories: Vec<PathBuf>,
    permission_mode: PermissionMode,
    event_tx: Option<mpsc::Sender<LoopEvent>>,
    cancel_token: CancellationToken,
    approval_store: Arc<Mutex<ApprovalStore>>,
    file_tracker: Arc<Mutex<FileTracker>>,
    spawn_agent_fn: Option<SpawnAgentFn>,
    is_plan_mode: bool,
    plan_file_path: Option<PathBuf>,
    background_registry: BackgroundTaskRegistry,
    lsp_manager: Option<Arc<LspServerManager>>,
    skill_manager: Option<Arc<SkillManager>>,
    hook_registry: Option<Arc<HookRegistry>>,
    invoked_skills: Arc<Mutex<Vec<InvokedSkill>>>,
    session_dir: Option<PathBuf>,
    parent_selections: Option<RoleSelections>,
    permission_requester: Option<Arc<dyn PermissionRequester>>,
    permission_evaluator: Option<PermissionRuleEvaluator>,
}

impl ToolContextBuilder {
    /// Create a new builder.
    pub fn new(call_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            session_id: session_id.into(),
            turn_id: String::new(),
            agent_id: None,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            additional_working_directories: Vec::new(),
            permission_mode: PermissionMode::Default,
            event_tx: None,
            cancel_token: CancellationToken::new(),
            approval_store: Arc::new(Mutex::new(ApprovalStore::new())),
            file_tracker: Arc::new(Mutex::new(FileTracker::new())),
            spawn_agent_fn: None,
            is_plan_mode: false,
            plan_file_path: None,
            background_registry: BackgroundTaskRegistry::new(),
            lsp_manager: None,
            skill_manager: None,
            hook_registry: None,
            invoked_skills: Arc::new(Mutex::new(Vec::new())),
            session_dir: None,
            parent_selections: None,
            permission_requester: None,
            permission_evaluator: None,
        }
    }

    /// Set the working directory.
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = cwd.into();
        self
    }

    /// Set the turn ID.
    pub fn turn_id(mut self, turn_id: impl Into<String>) -> Self {
        self.turn_id = turn_id.into();
        self
    }

    /// Set the agent ID.
    pub fn agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Set additional working directories.
    pub fn additional_working_directories(mut self, dirs: Vec<PathBuf>) -> Self {
        self.additional_working_directories = dirs;
        self
    }

    /// Set the permission mode.
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_mode = mode;
        self
    }

    /// Set the event channel.
    pub fn event_tx(mut self, tx: mpsc::Sender<LoopEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Set the cancellation token.
    pub fn cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = token;
        self
    }

    /// Set the approval store.
    pub fn approval_store(mut self, store: Arc<Mutex<ApprovalStore>>) -> Self {
        self.approval_store = store;
        self
    }

    /// Set the file tracker.
    pub fn file_tracker(mut self, tracker: Arc<Mutex<FileTracker>>) -> Self {
        self.file_tracker = tracker;
        self
    }

    /// Set the spawn agent callback.
    pub fn spawn_agent_fn(mut self, f: SpawnAgentFn) -> Self {
        self.spawn_agent_fn = Some(f);
        self
    }

    /// Set plan mode state.
    pub fn plan_mode(mut self, is_active: bool, plan_file_path: Option<PathBuf>) -> Self {
        self.is_plan_mode = is_active;
        self.plan_file_path = plan_file_path;
        self
    }

    /// Set the background task registry.
    pub fn background_registry(mut self, registry: BackgroundTaskRegistry) -> Self {
        self.background_registry = registry;
        self
    }

    /// Set the LSP server manager.
    pub fn lsp_manager(mut self, manager: Arc<LspServerManager>) -> Self {
        self.lsp_manager = Some(manager);
        self
    }

    /// Set the skill manager.
    pub fn skill_manager(mut self, manager: Arc<SkillManager>) -> Self {
        self.skill_manager = Some(manager);
        self
    }

    /// Set the hook registry.
    pub fn hook_registry(mut self, registry: Arc<HookRegistry>) -> Self {
        self.hook_registry = Some(registry);
        self
    }

    /// Set the session directory for persisting large tool results.
    pub fn session_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.session_dir = Some(dir.into());
        self
    }

    /// Set parent selections for subagent isolation.
    ///
    /// When spawning subagents via the Task tool, these selections will be
    /// passed to the subagent, ensuring it's unaffected by subsequent
    /// changes to the parent's model settings.
    pub fn parent_selections(mut self, selections: RoleSelections) -> Self {
        self.parent_selections = Some(selections);
        self
    }

    /// Set the permission requester for interactive approval flow.
    pub fn permission_requester(mut self, requester: Arc<dyn PermissionRequester>) -> Self {
        self.permission_requester = Some(requester);
        self
    }

    /// Set the permission rule evaluator.
    pub fn permission_evaluator(mut self, evaluator: PermissionRuleEvaluator) -> Self {
        self.permission_evaluator = Some(evaluator);
        self
    }

    /// Build the context.
    pub fn build(self) -> ToolContext {
        ToolContext {
            call_id: self.call_id,
            session_id: self.session_id,
            turn_id: self.turn_id,
            agent_id: self.agent_id,
            cwd: self.cwd,
            additional_working_directories: self.additional_working_directories,
            permission_mode: self.permission_mode,
            event_tx: self.event_tx,
            cancel_token: self.cancel_token,
            approval_store: self.approval_store,
            file_tracker: self.file_tracker,
            spawn_agent_fn: self.spawn_agent_fn,
            is_plan_mode: self.is_plan_mode,
            plan_file_path: self.plan_file_path,
            background_registry: self.background_registry,
            lsp_manager: self.lsp_manager,
            skill_manager: self.skill_manager,
            hook_registry: self.hook_registry,
            invoked_skills: self.invoked_skills,
            session_dir: self.session_dir,
            parent_selections: self.parent_selections,
            permission_requester: self.permission_requester,
            permission_evaluator: self.permission_evaluator,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_store() {
        let mut store = ApprovalStore::new();

        assert!(!store.is_approved("Bash", "git status"));
        store.approve_pattern("Bash", "git status");
        assert!(store.is_approved("Bash", "git status"));

        store.approve_session("Read");
        assert!(store.is_approved("Read", "any_pattern"));
    }

    #[test]
    fn test_file_tracker() {
        let mut tracker = FileTracker::new();

        let path = PathBuf::from("/test/file.txt");
        assert!(!tracker.was_read(&path));

        tracker.record_read(&path);
        assert!(tracker.was_read(&path));
        assert!(!tracker.was_modified(&path));

        tracker.record_modified(&path);
        assert!(tracker.was_modified(&path));
    }

    #[tokio::test]
    async fn test_tool_context() {
        let ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"));

        assert_eq!(ctx.call_id, "call-1");
        assert_eq!(ctx.session_id, "session-1");
        assert!(!ctx.is_cancelled());
    }

    #[test]
    fn test_resolve_path() {
        let ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/home/user/project"));

        // Relative path
        assert_eq!(
            ctx.resolve_path("src/main.rs"),
            PathBuf::from("/home/user/project/src/main.rs")
        );

        // Absolute path
        assert_eq!(
            ctx.resolve_path("/etc/passwd"),
            PathBuf::from("/etc/passwd")
        );
    }

    #[tokio::test]
    async fn test_context_builder() {
        let ctx = ToolContextBuilder::new("call-1", "session-1")
            .cwd("/tmp")
            .permission_mode(PermissionMode::Plan)
            .build();

        assert_eq!(ctx.cwd, PathBuf::from("/tmp"));
        assert_eq!(ctx.permission_mode, PermissionMode::Plan);
    }
}
