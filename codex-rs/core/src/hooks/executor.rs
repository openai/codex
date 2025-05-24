//! Hook execution framework and base executor.

use std::time::Duration;

use async_trait::async_trait;

use crate::hooks::context::HookContext;
use crate::hooks::types::{HookError, HookResult};

/// Result type for hook executor operations.
pub type HookExecutorResult = Result<HookResult, HookError>;

/// Trait for hook executors that can execute different types of hooks.
#[async_trait]
pub trait HookExecutor: Send + Sync {
    /// Execute a hook with the given context.
    async fn execute(&self, context: &HookContext) -> HookExecutorResult;
    
    /// Get the name/type of this executor for logging and debugging.
    fn executor_type(&self) -> &'static str;
    
    /// Validate that this executor can handle the given context.
    fn can_execute(&self, context: &HookContext) -> bool;
    
    /// Get the estimated execution time for this hook (for timeout planning).
    fn estimated_duration(&self) -> Option<Duration> {
        None
    }
}

// Placeholder implementations - these will be implemented in Phase 2
pub struct ScriptExecutor;
pub struct WebhookExecutor;
pub struct McpToolExecutor;
pub struct ExecutableExecutor;

#[async_trait]
impl HookExecutor for ScriptExecutor {
    async fn execute(&self, _context: &HookContext) -> HookExecutorResult {
        // TODO: Implement in Phase 2.2
        Err(HookError::Execution("ScriptExecutor not yet implemented".to_string()))
    }
    
    fn executor_type(&self) -> &'static str {
        "script"
    }
    
    fn can_execute(&self, _context: &HookContext) -> bool {
        // TODO: Implement in Phase 2.2
        false
    }
}

#[async_trait]
impl HookExecutor for WebhookExecutor {
    async fn execute(&self, _context: &HookContext) -> HookExecutorResult {
        // TODO: Implement in Phase 2.3
        Err(HookError::Execution("WebhookExecutor not yet implemented".to_string()))
    }
    
    fn executor_type(&self) -> &'static str {
        "webhook"
    }
    
    fn can_execute(&self, _context: &HookContext) -> bool {
        // TODO: Implement in Phase 2.3
        false
    }
}

#[async_trait]
impl HookExecutor for McpToolExecutor {
    async fn execute(&self, _context: &HookContext) -> HookExecutorResult {
        // TODO: Implement in Phase 2.3
        Err(HookError::Execution("McpToolExecutor not yet implemented".to_string()))
    }
    
    fn executor_type(&self) -> &'static str {
        "mcp_tool"
    }
    
    fn can_execute(&self, _context: &HookContext) -> bool {
        // TODO: Implement in Phase 2.3
        false
    }
}

#[async_trait]
impl HookExecutor for ExecutableExecutor {
    async fn execute(&self, _context: &HookContext) -> HookExecutorResult {
        // TODO: Implement in Phase 2.3
        Err(HookError::Execution("ExecutableExecutor not yet implemented".to_string()))
    }
    
    fn executor_type(&self) -> &'static str {
        "executable"
    }
    
    fn can_execute(&self, _context: &HookContext) -> bool {
        // TODO: Implement in Phase 2.3
        false
    }
}
