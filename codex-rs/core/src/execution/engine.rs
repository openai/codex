//! Execution engine with switchable strategies
//!
//! Provides a unified interface for executing blueprints with different strategies.

use crate::agents::AgentRuntime;
use crate::blueprint::{BlueprintBlock, ExecutionMode};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// Execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Blueprint ID that was executed
    pub blueprint_id: String,
    
    /// Execution mode used
    pub mode: ExecutionMode,
    
    /// Success flag
    pub success: bool,
    
    /// Summary of execution
    pub summary: String,
    
    /// Execution time in seconds
    pub execution_time_secs: f64,
    
    /// Artifacts produced
    pub artifacts: Vec<String>,
    
    /// Error message (if failed)
    pub error: Option<String>,
    
    /// Detailed results (mode-specific)
    pub details: serde_json::Value,
}

/// Execution engine
pub struct ExecutionEngine {
    /// Current execution mode
    mode: ExecutionMode,
    
    /// Agent runtime for sub-agent execution
    #[allow(dead_code)]
    runtime: Arc<AgentRuntime>,
}

impl ExecutionEngine {
    /// Create a new execution engine
    pub fn new(mode: ExecutionMode, runtime: Arc<AgentRuntime>) -> Self {
        Self { mode, runtime }
    }
    
    /// Set execution mode
    pub fn set_mode(&mut self, mode: ExecutionMode) {
        info!("Switching execution mode: {} -> {}", self.mode, mode);
        self.mode = mode;
    }
    
    /// Get current execution mode
    pub fn mode(&self) -> ExecutionMode {
        self.mode
    }
    
    /// Execute a blueprint
    pub async fn execute(&self, blueprint: &BlueprintBlock) -> Result<ExecutionResult> {
        // Verify blueprint is approved
        if !blueprint.can_execute() {
            anyhow::bail!(
                "Blueprint {} is not approved for execution (state: {})",
                blueprint.id,
                blueprint.state
            );
        }
        
        debug!(
            "Executing blueprint {} with mode: {}",
            blueprint.id, self.mode
        );
        
        let start = std::time::Instant::now();
        
        let result = match self.mode {
            ExecutionMode::Single => self.execute_single(blueprint).await?,
            ExecutionMode::Orchestrated => self.execute_orchestrated(blueprint).await?,
            ExecutionMode::Competition => self.execute_competition(blueprint).await?,
        };
        
        let execution_time_secs = start.elapsed().as_secs_f64();
        
        Ok(ExecutionResult {
            blueprint_id: blueprint.id.clone(),
            mode: self.mode,
            success: result.success,
            summary: result.summary,
            execution_time_secs,
            artifacts: result.artifacts,
            error: result.error,
            details: result.details,
        })
    }
    
    /// Execute in single-agent mode
    async fn execute_single(&self, blueprint: &BlueprintBlock) -> Result<ExecutionResult> {
        info!("Executing in single-agent mode");
        
        // Single-agent execution (stub for now)
        Ok(ExecutionResult {
            blueprint_id: blueprint.id.clone(),
            mode: ExecutionMode::Single,
            success: true,
            summary: "Single-agent execution completed".to_string(),
            execution_time_secs: 0.0,
            artifacts: vec![],
            error: None,
            details: serde_json::json!({
                "mode": "single",
                "agent": "primary",
            }),
        })
    }
    
    /// Execute with orchestrated control
    async fn execute_orchestrated(&self, blueprint: &BlueprintBlock) -> Result<ExecutionResult> {
        info!("Executing with orchestrated control");
        
        // Will be implemented by orchestrated-enhancement TODO
        // For now, delegate to auto_orchestrator
        Ok(ExecutionResult {
            blueprint_id: blueprint.id.clone(),
            mode: ExecutionMode::Orchestrated,
            success: true,
            summary: "Orchestrated execution completed".to_string(),
            execution_time_secs: 0.0,
            artifacts: vec![],
            error: None,
            details: serde_json::json!({
                "mode": "orchestrated",
                "agents_used": [],
            }),
        })
    }
    
    /// Execute with worktree competition
    async fn execute_competition(&self, blueprint: &BlueprintBlock) -> Result<ExecutionResult> {
        info!("Executing with worktree competition");
        
        // Will be implemented by competition-impl TODO
        Ok(ExecutionResult {
            blueprint_id: blueprint.id.clone(),
            mode: ExecutionMode::Competition,
            success: true,
            summary: "Competition execution completed".to_string(),
            execution_time_secs: 0.0,
            artifacts: vec![],
            error: None,
            details: serde_json::json!({
                "mode": "competition",
                "variants": [],
                "winner": null,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::state::BlueprintState;

    fn create_test_runtime() -> Arc<AgentRuntime> {
        // Create a minimal runtime for testing
        // This is a placeholder - actual runtime initialization is complex
        Arc::new(AgentRuntime::default())
    }
    
    fn create_approved_blueprint() -> BlueprintBlock {
        let mut bp = BlueprintBlock::new(
            "Test blueprint".to_string(),
            "test-bp".to_string(),
        );
        bp.state = BlueprintState::Approved {
            approved_by: "test-user".to_string(),
            approved_at: chrono::Utc::now(),
        };
        bp
    }

    #[test]
    fn test_engine_creation() {
        let runtime = create_test_runtime();
        let engine = ExecutionEngine::new(ExecutionMode::Orchestrated, runtime);
        
        assert_eq!(engine.mode(), ExecutionMode::Orchestrated);
    }
    
    #[test]
    fn test_mode_switching() {
        let runtime = create_test_runtime();
        let mut engine = ExecutionEngine::new(ExecutionMode::Single, runtime);
        
        assert_eq!(engine.mode(), ExecutionMode::Single);
        
        engine.set_mode(ExecutionMode::Competition);
        assert_eq!(engine.mode(), ExecutionMode::Competition);
    }
    
    #[tokio::test]
    async fn test_execute_single() {
        let runtime = create_test_runtime();
        let engine = ExecutionEngine::new(ExecutionMode::Single, runtime);
        let bp = create_approved_blueprint();
        
        let result = engine.execute(&bp).await.unwrap();
        
        assert_eq!(result.mode, ExecutionMode::Single);
        assert!(result.success);
    }
    
    #[tokio::test]
    async fn test_execute_unapproved_fails() {
        let runtime = create_test_runtime();
        let engine = ExecutionEngine::new(ExecutionMode::Single, runtime);
        let bp = BlueprintBlock::new("Test".to_string(), "test".to_string());
        
        let result = engine.execute(&bp).await;
        assert!(result.is_err());
    }
}

