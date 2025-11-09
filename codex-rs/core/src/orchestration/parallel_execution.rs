//! Parallel AI execution orchestration
//!
//! Executes multiple AI agents (Codex, GeminiCLI, Claudecode) in parallel
//! for competition-style development with worktree isolation.

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::resource_manager::ResourceGuard;
use super::resource_manager::ResourceManager;
use super::worktree_manager::WorktreeInfo;
use super::worktree_manager::WorktreeManager;
use codex_protocol::config_types::ReasoningEffort;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::Verbosity;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    Codex,
    GeminiCLI,
    Claudecode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    pub effort: ReasoningEffort,
    pub summary: ReasoningSummary,
    pub verbosity: Verbosity,
}

impl Default for ReasoningConfig {
    fn default() -> Self {
        Self {
            effort: ReasoningEffort::default(),
            summary: ReasoningSummary::default(),
            verbosity: Verbosity::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub agent: AgentType,
    pub prompt: String,
    pub worktree_path: Option<String>,
    pub timeout_seconds: Option<u64>,
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub agent: AgentType,
    pub success: bool,
    pub output: String,
    pub elapsed_seconds: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProgress {
    pub agent: AgentType,
    pub status: AgentStatus,
    pub progress_percent: f32,
    pub current_step: Option<String>,
}

pub struct ParallelOrchestrator {
    agent_states: Arc<RwLock<HashMap<AgentType, AgentProgress>>>,
    resource_manager: Arc<ResourceManager>,
    repo_path: PathBuf,
    worktree_cleanup: Arc<RwLock<Vec<WorktreeInfo>>>,
    reasoning_config: ReasoningConfig,
}

impl ParallelOrchestrator {
    pub fn new() -> Self {
        Self::with_repo_path(".")
    }

    pub fn with_repo_path(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            agent_states: Arc::new(RwLock::new(HashMap::new())),
            resource_manager: Arc::new(ResourceManager::new()),
            repo_path: repo_path.into(),
            worktree_cleanup: Arc::new(RwLock::new(Vec::new())),
            reasoning_config: ReasoningConfig::default(),
        }
    }

    pub fn with_reasoning_config(mut self, config: ReasoningConfig) -> Self {
        self.reasoning_config = config;
        self
    }

    /// Execute multiple AI agents in parallel
    pub async fn execute_parallel(&self, tasks: Vec<AgentTask>) -> Result<Vec<AgentResult>> {
        let mut handles: Vec<JoinHandle<AgentResult>> = Vec::new();
        let mut guards: Vec<ResourceGuard> = Vec::new();

        // Acquire resource slots for all tasks
        for _ in 0..tasks.len() {
            let guard = self.resource_manager.acquire_slot().await?;
            guards.push(guard);
        }

        // Create worktrees for each agent task
        let worktree_manager = WorktreeManager::new(&self.repo_path)?;
        let mut worktrees: Vec<WorktreeInfo> = Vec::new();

        for task in &tasks {
            let task_id = Uuid::new_v4().to_string();
            let agent_name = format!("{:?}", task.agent).to_lowercase();

            match worktree_manager.create_worktree(&agent_name, &task_id) {
                Ok(worktree) => {
                    worktrees.push(worktree);
                }
                Err(e) => {
                    tracing::warn!("Failed to create worktree for {}: {}", agent_name, e);
                    // Use repo_path as fallback
                    worktrees.push(WorktreeInfo {
                        name: format!("{}_{}", agent_name, task_id),
                        path: self.repo_path.clone(),
                        branch: "main".to_string(),
                        agent: agent_name,
                    });
                }
            }
        }

        // Store worktrees for cleanup
        {
            let mut cleanup = self.worktree_cleanup.write().await;
            cleanup.extend(worktrees.iter().cloned());
        }

        for (i, task) in tasks.into_iter().enumerate() {
            // Initialize agent state
            {
                let mut states = self.agent_states.write().await;
                states.insert(
                    task.agent,
                    AgentProgress {
                        agent: task.agent,
                        status: AgentStatus::Pending,
                        progress_percent: 0.0,
                        current_step: None,
                    },
                );
            }

            let states = Arc::clone(&self.agent_states);
            let worktree = worktrees.get(i).cloned();

            let handle =
                tokio::spawn(async move { Self::execute_agent(states, task, worktree).await });

            handles.push(handle);
        }

        // Wait for all agents to complete
        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    eprintln!("Agent task panicked: {}", e);
                }
            }
        }

        // Cleanup worktrees
        self.cleanup_worktrees().await?;

        // Drop guards to release resource slots
        drop(guards);

        Ok(results)
    }

    async fn execute_agent(
        states: Arc<RwLock<HashMap<AgentType, AgentProgress>>>,
        task: AgentTask,
        worktree: Option<WorktreeInfo>,
    ) -> AgentResult {
        let start = std::time::Instant::now();

        // Update status to Running
        {
            let mut state_map = states.write().await;
            if let Some(state) = state_map.get_mut(&task.agent) {
                state.status = AgentStatus::Running;
                state.current_step = Some("Initializing".to_string());
            }
        }

        // Execute agent with worktree isolation
        let result = Self::run_agent(&task, states.clone(), worktree).await;

        let elapsed = start.elapsed().as_secs_f64();

        // Update final status
        {
            let mut state_map = states.write().await;
            if let Some(state) = state_map.get_mut(&task.agent) {
                state.status = if result.is_ok() {
                    AgentStatus::Completed
                } else {
                    AgentStatus::Failed
                };
                state.progress_percent = 100.0;
                state.current_step = None;
            }
        }

        match result {
            Ok(output) => AgentResult {
                agent: task.agent,
                success: true,
                output,
                elapsed_seconds: elapsed,
                error: None,
            },
            Err(e) => AgentResult {
                agent: task.agent,
                success: false,
                output: String::new(),
                elapsed_seconds: elapsed,
                error: Some(e.to_string()),
            },
        }
    }

    async fn run_agent(
        task: &AgentTask,
        states: Arc<RwLock<HashMap<AgentType, AgentProgress>>>,
        worktree: Option<WorktreeInfo>,
    ) -> Result<String> {
        match task.agent {
            AgentType::Codex => Self::run_codex(task, Arc::clone(&states), worktree).await,
            AgentType::GeminiCLI => Self::run_gemini(task, Arc::clone(&states), worktree).await,
            AgentType::Claudecode => Self::run_claude(task, Arc::clone(&states), worktree).await,
        }
    }

    async fn run_codex(
        task: &AgentTask,
        states: Arc<RwLock<HashMap<AgentType, AgentProgress>>>,
        worktree: Option<WorktreeInfo>,
    ) -> Result<String> {
        Self::update_progress(Arc::clone(&states), task.agent, 10.0, "Starting Codex").await;

        let default_path = PathBuf::from(".");
        let working_dir = worktree.as_ref().map(|w| &w.path).unwrap_or(&default_path);

        Self::update_progress(Arc::clone(&states), task.agent, 30.0, "Executing command").await;

        let mut cmd = Command::new("codex");
        cmd.arg("exec")
            .arg(&task.prompt)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Apply timeout if specified
        let output = if let Some(timeout_secs) = task.timeout_seconds {
            tokio::time::timeout(tokio::time::Duration::from_secs(timeout_secs), cmd.output())
                .await
                .context("Codex execution timed out")?
        } else {
            cmd.output().await
        }
        .context("Failed to execute codex command")?;

        Self::update_progress(Arc::clone(&states), task.agent, 80.0, "Processing output").await;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            Self::update_progress(Arc::clone(&states), task.agent, 100.0, "Completed").await;

            Ok(if !stdout.is_empty() {
                stdout
            } else if !stderr.is_empty() {
                stderr
            } else {
                "Codex completed successfully (no output)".to_string()
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            anyhow::bail!("Codex failed: {}", stderr);
        }
    }

    async fn run_gemini(
        task: &AgentTask,
        states: Arc<RwLock<HashMap<AgentType, AgentProgress>>>,
        worktree: Option<WorktreeInfo>,
    ) -> Result<String> {
        Self::update_progress(Arc::clone(&states), task.agent, 10.0, "Starting GeminiCLI").await;

        let default_path = PathBuf::from(".");
        let working_dir = worktree.as_ref().map(|w| &w.path).unwrap_or(&default_path);

        Self::update_progress(Arc::clone(&states), task.agent, 30.0, "Executing command").await;

        // Try gemini-cli first, fall back to gemini
        let gemini_cmd = if which::which("gemini-cli").is_ok() {
            "gemini-cli"
        } else {
            "gemini"
        };

        let mut cmd = Command::new(gemini_cmd);
        cmd.arg(&task.prompt)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Apply timeout if specified
        let output = if let Some(timeout_secs) = task.timeout_seconds {
            tokio::time::timeout(tokio::time::Duration::from_secs(timeout_secs), cmd.output())
                .await
                .context("GeminiCLI execution timed out")?
        } else {
            cmd.output().await
        }
        .context("Failed to execute gemini command")?;

        Self::update_progress(Arc::clone(&states), task.agent, 80.0, "Processing output").await;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            Self::update_progress(Arc::clone(&states), task.agent, 100.0, "Completed").await;

            Ok(if !stdout.is_empty() {
                stdout
            } else if !stderr.is_empty() {
                stderr
            } else {
                "GeminiCLI completed successfully (no output)".to_string()
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            anyhow::bail!("GeminiCLI failed: {}", stderr);
        }
    }

    async fn run_claude(
        task: &AgentTask,
        states: Arc<RwLock<HashMap<AgentType, AgentProgress>>>,
        worktree: Option<WorktreeInfo>,
    ) -> Result<String> {
        Self::update_progress(Arc::clone(&states), task.agent, 10.0, "Starting Claudecode").await;

        let default_path = PathBuf::from(".");
        let working_dir = worktree.as_ref().map(|w| &w.path).unwrap_or(&default_path);

        Self::update_progress(Arc::clone(&states), task.agent, 30.0, "Executing command").await;

        // Try claudecode first, fall back to claude
        let claude_cmd = if which::which("claudecode").is_ok() {
            "claudecode"
        } else {
            "claude"
        };

        let mut cmd = Command::new(claude_cmd);
        cmd.arg(&task.prompt)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Apply timeout if specified
        let output = if let Some(timeout_secs) = task.timeout_seconds {
            tokio::time::timeout(tokio::time::Duration::from_secs(timeout_secs), cmd.output())
                .await
                .context("Claudecode execution timed out")?
        } else {
            cmd.output().await
        }
        .context("Failed to execute claude command")?;

        Self::update_progress(Arc::clone(&states), task.agent, 80.0, "Processing output").await;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            Self::update_progress(Arc::clone(&states), task.agent, 100.0, "Completed").await;

            Ok(if !stdout.is_empty() {
                stdout
            } else if !stderr.is_empty() {
                stderr
            } else {
                "Claudecode completed successfully (no output)".to_string()
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            anyhow::bail!("Claudecode failed: {}", stderr);
        }
    }

    async fn update_progress(
        states: Arc<RwLock<HashMap<AgentType, AgentProgress>>>,
        agent: AgentType,
        progress: f32,
        step: &str,
    ) {
        let mut state_map = states.write().await;
        if let Some(state) = state_map.get_mut(&agent) {
            state.progress_percent = progress;
            state.current_step = Some(step.to_string());
        }
    }

    /// Get current progress of all agents
    pub async fn get_progress(&self) -> HashMap<AgentType, AgentProgress> {
        self.agent_states.read().await.clone()
    }

    /// Get progress for a specific agent
    pub async fn get_agent_progress(&self, agent: AgentType) -> Option<AgentProgress> {
        self.agent_states.read().await.get(&agent).cloned()
    }

    /// Get the resource manager
    pub fn get_resource_manager(&self) -> Arc<ResourceManager> {
        Arc::clone(&self.resource_manager)
    }

    /// Cleanup all worktrees
    async fn cleanup_worktrees(&self) -> Result<()> {
        let worktrees = self.worktree_cleanup.read().await;

        if worktrees.is_empty() {
            return Ok(());
        }

        let worktree_manager = WorktreeManager::new(&self.repo_path)?;

        for worktree in worktrees.iter() {
            if let Err(e) = worktree_manager.remove_worktree(&worktree.name) {
                tracing::warn!("Failed to cleanup worktree {}: {}", worktree.name, e);
            }
        }

        Ok(())
    }
}

impl Drop for ParallelOrchestrator {
    fn drop(&mut self) {
        // Spawn a task to cleanup worktrees
        let worktree_cleanup = Arc::clone(&self.worktree_cleanup);
        let repo_path = self.repo_path.clone();

        tokio::spawn(async move {
            let worktrees = worktree_cleanup.read().await;

            if !worktrees.is_empty() {
                if let Ok(manager) = WorktreeManager::new(&repo_path) {
                    for worktree in worktrees.iter() {
                        let _ = manager.remove_worktree(&worktree.name);
                    }
                }
            }
        });
    }
}

impl Default for ParallelOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Compare and merge results from multiple agents
pub fn compare_results(results: &[AgentResult]) -> ComparisonResult {
    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.iter().filter(|r| !r.success).count();

    let fastest = results
        .iter()
        .filter(|r| r.success)
        .min_by(|a, b| a.elapsed_seconds.partial_cmp(&b.elapsed_seconds).unwrap());

    ComparisonResult {
        total_agents: results.len(),
        successful,
        failed,
        fastest_agent: fastest.map(|r| r.agent),
        fastest_time: fastest.map(|r| r.elapsed_seconds),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub total_agents: usize,
    pub successful: usize,
    pub failed: usize,
    pub fastest_agent: Option<AgentType>,
    pub fastest_time: Option<f64>,
}
