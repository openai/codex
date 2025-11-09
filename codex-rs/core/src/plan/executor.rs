//! Plan execution engine
//!
//! Handles the execution of approved Plans with progress tracking,
//! rollback capabilities, and integration with PlanOrchestrator.

use crate::orchestration::{OrchestratedResult, PlanOrchestrator};
use crate::plan::PlanBlock;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

/// Execution progress event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEvent {
    /// Execution started
    Started {
        execution_id: String,
        Plan_id: String,
        timestamp: DateTime<Utc>,
    },

    /// Step completed
    StepCompleted {
        execution_id: String,
        step_name: String,
        timestamp: DateTime<Utc>,
    },

    /// File changed
    FileChanged {
        execution_id: String,
        file_path: String,
        change_type: String, // "created", "modified", "deleted"
        timestamp: DateTime<Utc>,
    },

    /// Test passed
    TestPassed {
        execution_id: String,
        test_name: String,
        timestamp: DateTime<Utc>,
    },

    /// Test failed
    TestFailed {
        execution_id: String,
        test_name: String,
        error: String,
        timestamp: DateTime<Utc>,
    },

    /// Progress update
    Progress {
        execution_id: String,
        current_step: usize,
        total_steps: usize,
        message: String,
        timestamp: DateTime<Utc>,
    },

    /// Execution completed
    Completed {
        execution_id: String,
        success: bool,
        message: String,
        timestamp: DateTime<Utc>,
    },

    /// Execution failed
    Failed {
        execution_id: String,
        error: String,
        timestamp: DateTime<Utc>,
    },
}

/// Execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Execution ID
    pub execution_id: String,

    /// Plan ID
    pub Plan_id: String,

    /// Success flag
    pub success: bool,

    /// Orchestrated result
    pub orchestrated_result: Option<OrchestratedResult>,

    /// Files changed
    pub files_changed: Vec<String>,

    /// Tests run
    pub tests_run: Vec<TestResult>,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Start time
    pub started_at: DateTime<Utc>,

    /// End time
    pub completed_at: Option<DateTime<Utc>>,

    /// Duration in seconds
    pub duration_secs: Option<f64>,
}

/// Test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
}

/// Plan executor
pub struct PlanExecutor {
    /// Plan orchestrator
    orchestrator: Arc<PlanOrchestrator>,

    /// Event broadcaster
    event_tx: Arc<RwLock<Option<broadcast::Sender<ExecutionEvent>>>>,

    /// Execution log directory
    log_dir: PathBuf,
}

impl PlanExecutor {
    /// Create a new Plan executor
    pub fn new(orchestrator: Arc<PlanOrchestrator>, log_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&log_dir).ok();

        Self {
            orchestrator,
            event_tx: Arc::new(RwLock::new(None)),
            log_dir,
        }
    }

    /// Subscribe to execution events
    pub async fn subscribe(&self) -> broadcast::Receiver<ExecutionEvent> {
        let mut tx_guard = self.event_tx.write().await;

        if tx_guard.is_none() {
            let (tx, _) = broadcast::channel(100);
            *tx_guard = Some(tx);
        }

        tx_guard.as_ref().unwrap().subscribe()
    }

    /// Emit an execution event
    async fn emit_event(&self, event: ExecutionEvent) {
        let tx_guard = self.event_tx.read().await;

        if let Some(tx) = tx_guard.as_ref() {
            if let Err(e) = tx.send(event.clone()) {
                warn!("Failed to broadcast execution event: {}", e);
            }
        }

        debug!("Execution event: {:?}", event);
    }

    /// Execute a Plan
    pub async fn execute(&self, mut Plan: PlanBlock) -> Result<ExecutionResult> {
        // Verify Plan is approved
        if !Plan.state.can_execute() {
            anyhow::bail!("Plan {} is not approved (state: {})", Plan.id, Plan.state);
        }

        // Generate execution ID
        let execution_id = Uuid::new_v4().to_string();
        let started_at = Utc::now();

        info!("Starting execution {} for Plan {}", execution_id, Plan.id);

        // Transition to Executing state
        Plan.state = Plan
            .state
            .clone()
            .start_execution(execution_id.clone())
            .context("Failed to transition to Executing state")?;

        Plan.updated_at = Utc::now();

        // Emit started event
        self.emit_event(ExecutionEvent::Started {
            execution_id: execution_id.clone(),
            Plan_id: Plan.id.clone(),
            timestamp: started_at,
        })
        .await;

        // Execute the Plan
        let result = match self.execute_internal(&execution_id, &Plan).await {
            Ok(orchestrated_result) => {
                info!("Execution {} completed successfully", execution_id);

                // Transition to Completed state
                Plan.state = Plan
                    .state
                    .clone()
                    .complete_execution()
                    .context("Failed to transition to Completed state")?;

                Plan.updated_at = Utc::now();

                let completed_at = Utc::now();
                let duration_secs = (completed_at - started_at).num_seconds() as f64;

                // Emit completed event
                self.emit_event(ExecutionEvent::Completed {
                    execution_id: execution_id.clone(),
                    success: true,
                    message: "Plan executed successfully".to_string(),
                    timestamp: completed_at,
                })
                .await;

                ExecutionResult {
                    execution_id: execution_id.clone(),
                    Plan_id: Plan.id.clone(),
                    success: true,
                    orchestrated_result: Some(orchestrated_result),
                    files_changed: vec![],
                    tests_run: vec![],
                    error: None,
                    started_at,
                    completed_at: Some(completed_at),
                    duration_secs: Some(duration_secs),
                }
            }
            Err(e) => {
                error!("Execution {} failed: {}", execution_id, e);

                // Transition to Failed state
                Plan.state = Plan
                    .state
                    .clone()
                    .fail_execution(e.to_string())
                    .context("Failed to transition to Failed state")?;

                Plan.updated_at = Utc::now();

                let completed_at = Utc::now();
                let duration_secs = (completed_at - started_at).num_seconds() as f64;

                // Emit failed event
                self.emit_event(ExecutionEvent::Failed {
                    execution_id: execution_id.clone(),
                    error: e.to_string(),
                    timestamp: completed_at,
                })
                .await;

                ExecutionResult {
                    execution_id: execution_id.clone(),
                    Plan_id: Plan.id.clone(),
                    success: false,
                    orchestrated_result: None,
                    files_changed: vec![],
                    tests_run: vec![],
                    error: Some(e.to_string()),
                    started_at,
                    completed_at: Some(completed_at),
                    duration_secs: Some(duration_secs),
                }
            }
        };

        // Save execution log
        self.save_execution_log(&result)?;

        Ok(result)
    }

    /// Internal execution logic
    async fn execute_internal(
        &self,
        execution_id: &str,
        Plan: &PlanBlock,
    ) -> Result<OrchestratedResult> {
        // Emit progress for work items
        let total_steps = Plan.work_items.len().max(1);

        for (i, work_item) in Plan.work_items.iter().enumerate() {
            self.emit_event(ExecutionEvent::Progress {
                execution_id: execution_id.to_string(),
                current_step: i + 1,
                total_steps,
                message: format!("Processing: {}", work_item.name),
                timestamp: Utc::now(),
            })
            .await;

            self.emit_event(ExecutionEvent::StepCompleted {
                execution_id: execution_id.to_string(),
                step_name: work_item.name.clone(),
                timestamp: Utc::now(),
            })
            .await;
        }

        // Execute via orchestrator
        let result = self
            .orchestrator
            .execute_Plan(Plan)
            .await
            .context("Orchestrator execution failed")?;

        Ok(result)
    }

    /// Save execution log to disk
    fn save_execution_log(&self, result: &ExecutionResult) -> Result<()> {
        let log_file = self.log_dir.join(format!("{}.json", result.execution_id));

        let json =
            serde_json::to_string_pretty(result).context("Failed to serialize execution result")?;

        std::fs::write(&log_file, json).context("Failed to write execution log")?;

        info!("Saved execution log to {}", log_file.display());

        Ok(())
    }

    /// Load execution log from disk
    pub fn load_execution_log(&self, execution_id: &str) -> Result<ExecutionResult> {
        let log_file = self.log_dir.join(format!("{}.json", execution_id));

        if !log_file.exists() {
            anyhow::bail!("Execution log not found: {}", execution_id);
        }

        let json = std::fs::read_to_string(&log_file).context("Failed to read execution log")?;

        let result: ExecutionResult =
            serde_json::from_str(&json).context("Failed to deserialize execution log")?;

        Ok(result)
    }

    /// List all execution logs
    pub fn list_executions(&self) -> Result<Vec<ExecutionResult>> {
        let mut results = Vec::new();

        if !self.log_dir.exists() {
            return Ok(results);
        }

        for entry in std::fs::read_dir(&self.log_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(json) = std::fs::read_to_string(&path) {
                    if let Ok(result) = serde_json::from_str::<ExecutionResult>(&json) {
                        results.push(result);
                    }
                }
            }
        }

        // Sort by start time (newest first)
        results.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execution_id_generation() {
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();
        assert_ne!(id1, id2);
    }
}
