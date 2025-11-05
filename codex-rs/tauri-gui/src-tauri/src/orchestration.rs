use codex_core::orchestration::parallel_execution::{
    AgentProgress, AgentResult, AgentTask, AgentType, ComparisonResult, ParallelOrchestrator,
};
use codex_core::orchestration::resource_manager::{ResourceCapacity, SystemStats};
use codex_core::orchestration::worktree_manager::{WorktreeInfo, WorktreeManager};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{State, command};
use tokio::sync::RwLock;

pub struct OrchestrationState {
    pub orchestrator: Arc<RwLock<ParallelOrchestrator>>,
}

impl OrchestrationState {
    pub fn new() -> Self {
        Self::with_repo_path(".")
    }

    pub fn with_repo_path(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            orchestrator: Arc::new(RwLock::new(ParallelOrchestrator::with_repo_path(repo_path))),
        }
    }
}

#[command]
pub async fn orchestrate_parallel(
    state: State<'_, crate::OrchestratorState>,
    tasks: Vec<AgentTask>,
) -> Result<Vec<AgentResult>, String> {
    let orchestrator = state.orchestration.orchestrator.read().await;
    orchestrator
        .execute_parallel(tasks)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn get_orchestration_progress(
    state: State<'_, crate::OrchestratorState>,
) -> Result<Vec<AgentProgress>, String> {
    let orchestrator = state.orchestration.orchestrator.read().await;
    let progress = orchestrator.get_progress().await;
    Ok(progress.into_values().collect())
}

#[command]
pub async fn compare_agent_results(results: Vec<AgentResult>) -> Result<ComparisonResult, String> {
    Ok(codex_core::orchestration::parallel_execution::compare_results(&results))
}

#[command]
pub fn create_worktree(
    repo_path: String,
    agent_name: String,
    task_id: String,
) -> Result<WorktreeInfo, String> {
    let manager = WorktreeManager::new(&repo_path).map_err(|e| e.to_string())?;
    manager
        .create_worktree(&agent_name, &task_id)
        .map_err(|e| e.to_string())
}

#[command]
pub fn list_worktrees(repo_path: String) -> Result<Vec<WorktreeInfo>, String> {
    let manager = WorktreeManager::new(&repo_path).map_err(|e| e.to_string())?;
    manager.list_worktrees().map_err(|e| e.to_string())
}

#[command]
pub fn remove_worktree(repo_path: String, worktree_name: String) -> Result<(), String> {
    let manager = WorktreeManager::new(&repo_path).map_err(|e| e.to_string())?;
    manager
        .remove_worktree(&worktree_name)
        .map_err(|e| e.to_string())
}

#[command]
pub fn merge_worktree(
    repo_path: String,
    worktree_info: WorktreeInfo,
    target_branch: String,
) -> Result<(), String> {
    let manager = WorktreeManager::new(&repo_path).map_err(|e| e.to_string())?;
    manager
        .merge_worktree(&worktree_info, &target_branch)
        .map_err(|e| e.to_string())
}

#[command]
pub async fn get_resource_capacity(
    state: State<'_, crate::OrchestratorState>,
) -> Result<ResourceCapacity, String> {
    let orchestrator = state.orchestration.orchestrator.read().await;
    let resource_manager = orchestrator.get_resource_manager();

    Ok(resource_manager.get_capacity().await)
}

#[command]
pub async fn get_system_stats(
    state: State<'_, crate::OrchestratorState>,
) -> Result<SystemStats, String> {
    let orchestrator = state.orchestration.orchestrator.read().await;
    let resource_manager = orchestrator.get_resource_manager();

    resource_manager
        .get_system_stats()
        .await
        .map_err(|e| e.to_string())
}
